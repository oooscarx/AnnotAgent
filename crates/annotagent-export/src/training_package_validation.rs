//! Independent semantic checks on reopened ZIP bytes, not writer preflight results.
use crate::training_package::{ImageConfirmation, PackageManifest};
use annotagent_core::dataset_delivery::DatasetSplit;
use anyhow::{Context, Result, ensure};
use image::ImageDecoder;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
};

fn read_json(zip: &mut zip::ZipArchive<File>, name: &str) -> Result<serde_json::Value> {
    let entry = zip.by_name(name)?;
    ensure!(
        entry.size() <= 32 * 1024 * 1024,
        "Metadata exceeds validation limit"
    );
    Ok(serde_json::from_reader(entry.take(32 * 1024 * 1024 + 1))?)
}

pub(crate) fn validate_contents(
    zip: &mut zip::ZipArchive<File>,
    manifest: &PackageManifest,
    entries: &BTreeSet<String>,
) -> Result<()> {
    manifest.intent.validate().map_err(anyhow::Error::msg)?;
    ensure!(
        manifest.format_version == 1
            && manifest.delivery_revision > 0
            && manifest.intent.missing_slots().is_empty(),
        "Invalid manifest identity or incomplete intent"
    );
    ensure!(
        manifest
            .intent
            .training_target
            .as_ref()
            .is_some_and(annotagent_core::dataset_delivery::TrainingTarget::is_detection_preset),
        "Unsupported package preset"
    );
    ensure!(
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&manifest.intent)?)
        ) == manifest.intent_sha256,
        "Intent hash mismatch"
    );
    let labels = manifest
        .intent
        .label_spec
        .as_ref()
        .context("labels missing")?;
    let yaml_entry = zip.by_name("data.yaml")?;
    ensure!(
        yaml_entry.size() <= 1024 * 1024,
        "YAML exceeds validation limit"
    );
    let yaml: serde_yaml::Value = serde_yaml::from_reader(yaml_entry.take(1024 * 1024 + 1))?;
    let mapping = yaml.as_mapping().context("data.yaml is not a mapping")?;
    ensure!(
        mapping
            .keys()
            .all(|k| matches!(k.as_str(), Some("train" | "val" | "test" | "names"))),
        "YAML contains unsafe or unsupported keys"
    );
    ensure!(
        yaml["train"].as_str() == Some("images/train")
            && yaml["val"].as_str() == Some("images/val"),
        "YAML paths must be package-relative train/val directories"
    );
    ensure!(
        yaml.get("test").is_none() || yaml["test"].as_str() == Some("images/test"),
        "Unsafe test directory"
    );
    let names = yaml["names"]
        .as_mapping()
        .context("class names are not an indexed mapping")?;
    ensure!(
        names.len() == labels.len(),
        "Class mapping size differs from intent"
    );
    for (index, label) in labels.iter().enumerate() {
        let value = names
            .get(serde_yaml::Value::String(index.to_string()))
            .or_else(|| names.get(serde_yaml::Value::Number(index.into())));
        ensure!(
            value.and_then(serde_yaml::Value::as_str) == Some(label.display_name.as_str()),
            "Class mapping is not the frozen label order"
        );
    }
    let scope = manifest
        .intent
        .dataset_scope
        .as_ref()
        .context("scope missing")?;
    let scope_ids = scope.iter().map(|i| i.image_id).collect::<BTreeSet<_>>();
    let image_ids = manifest
        .images
        .iter()
        .map(|i| i.image_id)
        .collect::<BTreeSet<_>>();
    ensure!(
        image_ids.len() == manifest.images.len() && scope_ids == image_ids,
        "Image evidence does not match exact intent scope"
    );
    let mut allowed = BTreeSet::from(
        [
            "data.yaml",
            "README.md",
            "annotagent/manifest.json",
            "annotagent/split-manifest.json",
            "annotagent/exclusions.json",
            "annotagent/validation-report.json",
        ]
        .map(str::to_owned),
    );
    let mut groups = BTreeMap::new();
    let mut split_counts = [0_usize; 3];
    let mut train_classes = BTreeSet::new();
    let mut other_classes = BTreeSet::new();
    let mut annotation_ids = BTreeSet::new();
    for image in &manifest.images {
        let selected = scope
            .iter()
            .find(|i| i.image_id == image.image_id)
            .context("missing image scope")?;
        if let ImageConfirmation::Excluded {
            confirmation_id,
            reason,
        } = &image.confirmation
        {
            ensure!(
                !confirmation_id.trim().is_empty()
                    && !reason.trim().is_empty()
                    && image.image.is_none()
                    && image.label.is_none()
                    && image.split.is_none()
                    && image.annotation_ids.is_empty(),
                "Exclusion is not explicit or includes payload"
            );
            continue;
        }
        let split = image.split.context("Included image lacks split")?;
        let (split_index, split_name) = match split {
            DatasetSplit::Train => (0, "train"),
            DatasetSplit::Val => (1, "val"),
            DatasetSplit::Test => (2, "test"),
        };
        split_counts[split_index] += 1;
        ensure!(
            selected
                .existing_split
                .is_none_or(|existing| existing == split),
            "Existing image split changed"
        );
        for group in std::iter::once(format!("sha:{}", selected.content_sha256))
            .chain(selected.group_ids.iter().map(|g| format!("group:{g}")))
        {
            if let Some(previous) = groups.insert(group, split) {
                ensure!(
                    previous == split,
                    "Exact content or known source group leaks across splits"
                );
            }
        }
        let image_path = image
            .image
            .as_deref()
            .context("Included image has no original")?;
        let label_path = image
            .label
            .as_deref()
            .context("Included image has no label")?;
        let base = format!("images/{split_name}/{}", image.image_id);
        ensure!(
            image_path == format!("{base}.png") || image_path == format!("{base}.jpg"),
            "Image path is not its stable package identity"
        );
        ensure!(
            label_path == format!("labels/{split_name}/{}.txt", image.image_id),
            "Label path does not pair with image"
        );
        ensure!(
            allowed.insert(image_path.into()) && allowed.insert(label_path.into()),
            "Repeated image/label path"
        );
        ensure!(
            manifest
                .files
                .get(image_path)
                .is_some_and(|file| file.sha256 == selected.content_sha256),
            "Packaged original differs from frozen source hash"
        );
        let mut original = tempfile::tempfile()?;
        std::io::copy(&mut zip.by_name(image_path)?, &mut original)?;
        original.seek(SeekFrom::Start(0))?;
        let reader = image::ImageReader::new(BufReader::new(original)).with_guessed_format()?;
        let format = reader.format();
        ensure!(
            matches!(
                (format, image_path == format!("{base}.png")),
                (Some(image::ImageFormat::Png), true) | (Some(image::ImageFormat::Jpeg), false)
            ),
            "Image extension does not match bytes"
        );
        let mut decoder = reader.into_decoder()?;
        ensure!(
            decoder.dimensions() == (image.width, image.height)
                && image.width > 0
                && image.height > 0,
            "Decoded image dimensions differ from evidence"
        );
        ensure!(
            decoder.orientation()? == image::metadata::Orientation::NoTransforms,
            "Unapplied EXIF orientation"
        );
        let _decoded = image::DynamicImage::from_decoder(decoder)
            .context("Original image pixels cannot be decoded")?;
        let entry = zip.by_name(label_path)?;
        ensure!(
            entry.size() <= 16 * 1024 * 1024,
            "Labels exceed validation limit"
        );
        let mut count = 0;
        for row in BufReader::new(entry).lines() {
            let row = row?;
            if row.trim().is_empty() {
                continue;
            }
            let fields = row.split_whitespace().collect::<Vec<_>>();
            ensure!(fields.len() == 5, "Detection row must have five fields");
            ensure!(
                fields[0].bytes().all(|c| c.is_ascii_digit()),
                "Class ID is not a nonnegative integer"
            );
            let class = fields[0].parse::<usize>()?;
            ensure!(class < labels.len(), "Class ID outside frozen mapping");
            let values = fields[1..]
                .iter()
                .map(|s| s.parse::<f64>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let (x, y, w, h) = (values[0], values[1], values[2], values[3]);
            // Nine-decimal serialization can introduce sub-nanopixel rounding; never clamp.
            let epsilon = 0.000_000_001;
            ensure!(
                values
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
                    && w > 0.0
                    && h > 0.0
                    && x - w / 2.0 >= -epsilon
                    && y - h / 2.0 >= -epsilon
                    && x + w / 2.0 <= 1.0 + epsilon
                    && y + h / 2.0 <= 1.0 + epsilon,
                "Non-finite, empty or out-of-image detection box"
            );
            if split == DatasetSplit::Train {
                train_classes.insert(class);
            } else {
                other_classes.insert(class);
            }
            count += 1;
        }
        ensure!(
            count == image.annotation_ids.len(),
            "Annotation count differs from label rows"
        );
        for id in &image.annotation_ids {
            ensure!(
                !id.trim().is_empty() && annotation_ids.insert(id),
                "Duplicate or empty annotation identity"
            );
        }
        match &image.confirmation {
            ImageConfirmation::PositiveComplete { confirmation_id } => ensure!(
                !confirmation_id.trim().is_empty() && count > 0,
                "Positive image lacks confirmation or objects"
            ),
            ImageConfirmation::NegativeConfirmed { confirmation_id } => ensure!(
                !confirmation_id.trim().is_empty() && count == 0,
                "Negative image is not explicitly empty"
            ),
            _ => anyhow::bail!("Unresolved image cannot be delivered"),
        }
    }
    ensure!(
        &allowed == entries,
        "Unexpected files (including credentials or scripts) in package"
    );
    ensure!(
        split_counts[0] > 0 && split_counts[1] > 0,
        "Train and val must both be nonempty"
    );
    ensure!(
        yaml.get("test").is_some() == (split_counts[2] > 0),
        "Test YAML and evidence differ"
    );
    ensure!(
        other_classes.is_subset(&train_classes),
        "Class occurs outside train only"
    );
    ensure!(
        !train_classes.is_empty(),
        "No positive training examples in standalone package"
    );
    ensure!(
        read_json(zip, "annotagent/split-manifest.json")?
            == serde_json::to_value(&manifest.images)?,
        "Split report differs from image evidence"
    );
    ensure!(
        read_json(zip, "annotagent/exclusions.json")?
            == serde_json::to_value(
                manifest
                    .images
                    .iter()
                    .filter(|i| i.image.is_none())
                    .collect::<Vec<_>>()
            )?,
        "Exclusions report differs from image evidence"
    );
    Ok(())
}
