//! Strict, source-image YOLO Detection package, separate from permissive flat exports.
//! Callers must resolve/freeze owned annotations and human confirmation receipts.
use annotagent_core::{
    Annotation, AnnotationValue, ImageId, ReviewStatus,
    dataset_delivery::{DatasetSplit, TaskDeliveryIntent},
};
use anyhow::{Context, Result, bail, ensure};
use image::ImageDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ImageConfirmation {
    PositiveComplete {
        confirmation_id: String,
    },
    NegativeConfirmed {
        confirmation_id: String,
    },
    Excluded {
        confirmation_id: String,
        reason: String,
    },
    NeedsReview,
    Incomplete,
    Failed,
}

pub struct PackageImage {
    pub image_id: ImageId,
    /// Server-resolved original file; never serialized into the package.
    pub source: PathBuf,
    pub annotations: Vec<Annotation>,
    pub confirmation: ImageConfirmation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageReceipt {
    pub sha256: String,
    pub bytes: u64,
    pub images: usize,
    pub objects: usize,
    pub negatives: usize,
    pub excluded: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEvidence {
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageEvidence {
    pub image_id: ImageId,
    pub image: Option<String>,
    pub label: Option<String>,
    pub width: u32,
    pub height: u32,
    pub split: Option<DatasetSplit>,
    pub confirmation: ImageConfirmation,
    pub annotation_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageManifest {
    pub format_version: u32,
    pub delivery_revision: u32,
    pub intent_sha256: String,
    pub intent: TaskDeliveryIntent,
    pub license: String,
    pub loader_commit: String,
    pub files: BTreeMap<String, FileEvidence>,
    pub images: Vec<ImageEvidence>,
    pub warnings: Vec<String>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn split_name(split: DatasetSplit) -> &'static str {
    match split {
        DatasetSplit::Train => "train",
        DatasetSplit::Val => "val",
        DatasetSplit::Test => "test",
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Exact-content and known-group union prevents leakage; this is not near-duplicate detection.
fn splits(
    intent: &TaskDeliveryIntent,
    included: &BTreeSet<ImageId>,
) -> Result<BTreeMap<ImageId, DatasetSplit>> {
    let scope = intent
        .dataset_scope
        .as_ref()
        .context("image scope missing")?;
    let images = scope
        .iter()
        .filter(|i| included.contains(&i.image_id))
        .collect::<Vec<_>>();
    let mut parent = (0..images.len()).collect::<Vec<_>>();
    let mut known = BTreeMap::<String, usize>::new();
    for (i, image) in images.iter().enumerate() {
        let keys = std::iter::once(format!("sha:{}", image.content_sha256))
            .chain(image.group_ids.iter().map(|g| format!("group:{g}")));
        for key in keys {
            if let Some(other) = known.insert(key, i) {
                let a = root(&mut parent, i);
                let b = root(&mut parent, other);
                parent[a] = b;
            }
        }
    }
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..images.len() {
        groups.entry(root(&mut parent, i)).or_default().push(i);
    }
    ensure!(
        groups.len() >= 2,
        "At least two independent image groups are required for nonempty train and val"
    );
    let mut ordered = groups
        .into_values()
        .map(|group| {
            let mut ids = group
                .iter()
                .map(|i| images[*i].image_id.to_string())
                .collect::<Vec<_>>();
            ids.sort();
            (
                digest(format!("{}:{}", intent.split_policy.seed, ids.join(",")).as_bytes()),
                group,
            )
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let train_target = (images.len() * usize::from(intent.split_policy.train_percent) / 100)
        .clamp(1, images.len() - 1);
    let mut train_count = 0;
    let mut result = BTreeMap::new();
    let group_count = ordered.len();
    for (position, (_, group)) in ordered.into_iter().enumerate() {
        let explicit = group
            .iter()
            .filter_map(|i| images[*i].existing_split)
            .collect::<Vec<_>>();
        ensure!(
            explicit.iter().all(|s| Some(s) == explicit.first()),
            "Existing splits conflict within a duplicate/source group"
        );
        let split = explicit.first().copied().unwrap_or(
            if train_count < train_target && position + 1 < group_count {
                DatasetSplit::Train
            } else {
                DatasetSplit::Val
            },
        );
        if split == DatasetSplit::Train {
            train_count += group.len();
        }
        for i in group {
            result.insert(images[i].image_id, split);
        }
    }
    ensure!(
        result.values().any(|s| *s == DatasetSplit::Train)
            && result.values().any(|s| *s == DatasetSplit::Val),
        "Split policy cannot provide nonempty train and val without changing existing groups"
    );
    Ok(result)
}

fn label_text(
    annotations: &[Annotation],
    labels: &BTreeMap<&str, usize>,
    image: ImageId,
) -> Result<String> {
    let mut rows = Vec::new();
    for annotation in annotations {
        ensure!(
            annotation.image_id == image && annotation.review_status == ReviewStatus::HumanAccepted,
            "Whole-image package requires owned, human-accepted annotations"
        );
        let label = annotation
            .label
            .as_ref()
            .context("annotation label missing")?;
        let class = labels
            .get(label.as_str())
            .context("annotation has no stable export class mapping")?;
        let AnnotationValue::BoundingBox { rect } = annotation.value else {
            bail!("Detection package cannot silently skip non-bbox annotations")
        };
        let (x, y, w, h) = (rect.x(), rect.y(), rect.width(), rect.height());
        ensure!(
            [x, y, w, h].iter().all(|v| v.is_finite())
                && x >= 0.0
                && y >= 0.0
                && w > 0.0
                && h > 0.0
                && x + w <= 1.0
                && y + h <= 1.0,
            "Invalid detection geometry"
        );
        rows.push(format!(
            "{class} {:.9} {:.9} {:.9} {:.9}",
            f64::from(x) + f64::from(w) / 2.0,
            f64::from(y) + f64::from(h) / 2.0,
            w,
            h
        ));
    }
    Ok(if rows.is_empty() {
        String::new()
    } else {
        format!("{}\n", rows.join("\n"))
    })
}

fn write_entry(
    zip: &mut zip::ZipWriter<File>,
    files: &mut BTreeMap<String, FileEvidence>,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    zip.start_file(
        name,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(bytes)?;
    files.insert(
        name.into(),
        FileEvidence {
            sha256: digest(bytes),
            bytes: bytes.len() as u64,
        },
    );
    Ok(())
}

struct PartialFile(PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Writes a real ZIP to a new destination only. Failures leave no published ZIP.
/// Readiness is image-level: empty detections and rejected objects cannot imply negatives.
pub fn write_training_package(
    intent: TaskDeliveryIntent,
    revision: u32,
    sources: &[PackageImage],
    destination: &Path,
) -> Result<PackageReceipt> {
    let mut sources = sources.iter().collect::<Vec<_>>();
    sources.sort_by_key(|image| image.image_id);
    intent.validate().map_err(anyhow::Error::msg)?;
    ensure!(
        revision > 0 && intent.missing_slots().is_empty(),
        "Delivery intent is incomplete"
    );
    ensure!(
        intent
            .training_target
            .as_ref()
            .is_some_and(annotagent_core::dataset_delivery::TrainingTarget::is_detection_preset),
        "Unsupported delivery preset"
    );
    ensure!(!destination.exists(), "Package destination already exists");
    let scope = intent.dataset_scope.as_ref().context("scope missing")?;
    let source_ids = sources.iter().map(|i| i.image_id).collect::<BTreeSet<_>>();
    ensure!(
        source_ids.len() == sources.len()
            && source_ids == scope.iter().map(|i| i.image_id).collect(),
        "Package images must exactly match frozen intent scope"
    );
    let mut included = BTreeSet::new();
    for source in &sources {
        match &source.confirmation {
            ImageConfirmation::PositiveComplete { confirmation_id } => {
                ensure!(
                    !confirmation_id.trim().is_empty() && !source.annotations.is_empty(),
                    "Positive image lacks confirmation or objects"
                );
                included.insert(source.image_id);
            }
            ImageConfirmation::NegativeConfirmed { confirmation_id } => {
                ensure!(
                    !confirmation_id.trim().is_empty() && source.annotations.is_empty(),
                    "Negative image requires explicit empty-image confirmation"
                );
                included.insert(source.image_id);
            }
            ImageConfirmation::Excluded {
                confirmation_id,
                reason,
            } => ensure!(
                !confirmation_id.trim().is_empty() && !reason.trim().is_empty(),
                "Exclusion requires explicit receipt and reason"
            ),
            _ => bail!("Unresolved, incomplete or failed images block training package delivery"),
        }
    }
    let plan = splits(&intent, &included)?;
    let class_map = intent
        .label_spec
        .as_ref()
        .context("labels missing")?
        .iter()
        .enumerate()
        .map(|(i, l)| (l.stable_id.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let mut prepared = Vec::new();
    let mut train_classes = BTreeSet::new();
    let mut other_classes = BTreeSet::new();
    for source in &sources {
        if !included.contains(&source.image_id) {
            continue;
        }
        let text = label_text(&source.annotations, &class_map, source.image_id)?;
        for annotation in &source.annotations {
            let label = annotation
                .label
                .as_ref()
                .context("label missing")?
                .as_str()
                .to_owned();
            if plan[&source.image_id] == DatasetSplit::Train {
                train_classes.insert(label);
            } else {
                other_classes.insert(label);
            }
        }
        ensure!(
            !source.source.is_symlink() && source.source.is_file(),
            "Original source must be a regular file"
        );
        let reader = image::ImageReader::open(&source.source)?.with_guessed_format()?;
        let extension = match reader.format() {
            Some(image::ImageFormat::Png) => "png",
            Some(image::ImageFormat::Jpeg) => "jpg",
            _ => bail!("Only original PNG/JPEG images are supported"),
        };
        let mut decoder = reader.into_decoder()?;
        ensure!(
            decoder.orientation()? == image::metadata::Orientation::NoTransforms,
            "EXIF orientation requires an explicit geometry transform before delivery"
        );
        let (width, height) = decoder.dimensions();
        ensure!(width > 0 && height > 0, "Invalid source dimensions");
        prepared.push((source, text, extension, width, height));
    }
    ensure!(
        other_classes.is_subset(&train_classes),
        "A class occurs only outside train; revise the split instead of exporting invalid training data"
    );
    ensure!(
        !train_classes.is_empty(),
        "A standalone training package requires at least one confirmed positive training example"
    );
    let temporary = destination.with_extension("zip.partial");
    let file = File::options()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&temporary)?;
    let _cleanup = PartialFile(temporary.clone());
    let mut zip = zip::ZipWriter::new(file);
    let mut files = BTreeMap::new();
    let mut evidence = Vec::new();
    let mut objects = 0;
    let mut negatives = 0;
    for (source, text, extension, width, height) in prepared {
        let split = plan[&source.image_id];
        let image_path = format!(
            "images/{}/{}.{}",
            split_name(split),
            source.image_id,
            extension
        );
        let label_path = format!("labels/{}/{}.txt", split_name(split), source.image_id);
        zip.start_file(
            &image_path,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )?;
        let mut original = File::open(&source.source)?;
        let mut hash = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 16384];
        loop {
            let n = original.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
            bytes += n as u64;
        }
        let sha256 = format!("{:x}", hash.finalize());
        ensure!(
            scope
                .iter()
                .any(|i| i.image_id == source.image_id && i.content_sha256 == sha256),
            "Original bytes changed since delivery scope was confirmed"
        );
        files.insert(image_path.clone(), FileEvidence { sha256, bytes });
        write_entry(&mut zip, &mut files, &label_path, text.as_bytes())?;
        objects += source.annotations.len();
        if source.annotations.is_empty() {
            negatives += 1;
        }
        evidence.push(ImageEvidence {
            image_id: source.image_id,
            image: Some(image_path),
            label: Some(label_path),
            width,
            height,
            split: Some(split),
            confirmation: source.confirmation.clone(),
            annotation_ids: source
                .annotations
                .iter()
                .map(|a| a.id.to_string())
                .collect(),
        });
    }
    for source in sources.iter().filter(|s| !included.contains(&s.image_id)) {
        evidence.push(ImageEvidence {
            image_id: source.image_id,
            image: None,
            label: None,
            width: 0,
            height: 0,
            split: None,
            confirmation: source.confirmation.clone(),
            annotation_ids: vec![],
        });
    }
    evidence.sort_by_key(|e| e.image_id);
    let names = intent
        .label_spec
        .as_ref()
        .context("labels missing")?
        .iter()
        .enumerate()
        .map(|(i, l)| (i, l.display_name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut yaml = serde_json::json!({"train":"images/train","val":"images/val","names":names});
    if plan.values().any(|s| *s == DatasetSplit::Test) {
        yaml["test"] = serde_json::json!("images/test");
    }
    write_entry(
        &mut zip,
        &mut files,
        "data.yaml",
        serde_yaml::to_string(&yaml)?.as_bytes(),
    )?;
    write_entry(&mut zip,&mut files,"README.md",b"# AnnotAgent YOLO Object Detection dataset\n\nPass the absolute path of data.yaml after extracting this archive. Image paths are relative to that YAML. Original image bytes are preserved. Geometry and format validation do not establish model accuracy. Source license is unknown; verify usage rights.\n")?;
    write_entry(
        &mut zip,
        &mut files,
        "annotagent/split-manifest.json",
        &serde_json::to_vec_pretty(&evidence)?,
    )?;
    write_entry(
        &mut zip,
        &mut files,
        "annotagent/exclusions.json",
        &serde_json::to_vec_pretty(
            &evidence
                .iter()
                .filter(|e| e.image.is_none())
                .collect::<Vec<_>>(),
        )?,
    )?;
    write_entry(
        &mut zip,
        &mut files,
        "annotagent/validation-report.json",
        &serde_json::to_vec_pretty(&serde_json::json!({
            "validator_version":1,"scope":"Structural checks only, not model accuracy or official loader smoke",
            "checks":["exact_scope","explicit_image_confirmation","stable_class_mapping","normalized_bbox","nonempty_grouped_splits","source_sha256","zip_entry_hashes","image_label_pairing"],
            "publication":"This archive is published only after all listed checks pass"
        }))?,
    )?;
    let mut warnings = vec!["Near-duplicate visual similarity was not checked; exact content and known groups stay together.".into()];
    for label in intent.label_spec.as_ref().context("labels missing")? {
        let count = sources
            .iter()
            .filter(|source| {
                included.contains(&source.image_id)
                    && source.annotations.iter().any(|a| {
                        a.label
                            .as_ref()
                            .is_some_and(|id| id.as_str() == label.stable_id)
                    })
            })
            .count();
        if count < 2 {
            warnings.push(format!("Class {} appears in only {count} included images; useful train/val class coverage is not established.", label.display_name));
        }
    }
    let manifest = PackageManifest {
        format_version: 1,
        delivery_revision: revision,
        intent_sha256: digest(&serde_json::to_vec(&intent)?),
        intent,
        license: "unknown".into(),
        loader_commit: "6e43d1e1e5db72afbf686dee6745669bcb124b0a".into(),
        files,
        images: evidence,
        warnings,
    };
    zip.start_file(
        "annotagent/manifest.json",
        zip::write::SimpleFileOptions::default(),
    )?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    let file = zip.finish()?;
    file.sync_all()?;
    // Independent ZIP verification is required before atomic publication (implemented below).
    validate_training_package(&temporary)?;
    let mut file = File::open(&temporary)?;
    let mut hash = Sha256::new();
    let bytes = std::io::copy(&mut file, &mut hash)?;
    fs::hard_link(&temporary, destination)
        .context("Cannot atomically publish package without overwriting")?;
    Ok(PackageReceipt {
        sha256: format!("{:x}", hash.finalize()),
        bytes,
        images: included.len(),
        objects,
        negatives,
        excluded: sources.len() - included.len(),
    })
}

/// Reopens the finished archive and checks entry paths, pairing and every payload hash.
pub fn validate_training_package(path: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(path)?)?;
    let manifest_entry = zip.by_name("annotagent/manifest.json")?;
    ensure!(
        manifest_entry.size() <= 32 * 1024 * 1024,
        "Package manifest exceeds validation limit"
    );
    let manifest: PackageManifest =
        serde_json::from_reader(manifest_entry.take(32 * 1024 * 1024 + 1))?;
    ensure!(
        zip.len() == manifest.files.len() + 1,
        "Unexpected or missing package entries"
    );
    let mut seen = BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        ensure!(
            entry.enclosed_name().is_some()
                && !entry.is_dir()
                && entry
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170_000 != 0o120_000)
                && !entry.name().contains('\\')
                && seen.insert(entry.name().to_owned()),
            "Unsafe or duplicate ZIP entry"
        );
        if entry.name() == "annotagent/manifest.json" {
            continue;
        }
        let expected = manifest
            .files
            .get(entry.name())
            .context("Unlisted file in package")?;
        let mut hash = Sha256::new();
        let bytes = std::io::copy(&mut entry, &mut hash)?;
        ensure!(
            bytes == expected.bytes && format!("{:x}", hash.finalize()) == expected.sha256,
            "Package file hash mismatch"
        );
    }
    for image in &manifest.images {
        if let (Some(path), Some(label)) = (&image.image, &image.label) {
            ensure!(
                seen.contains(path) && seen.contains(label),
                "Image/label pair is incomplete"
            );
        } else {
            ensure!(
                matches!(image.confirmation, ImageConfirmation::Excluded { .. }),
                "Only explicit exclusions may omit image bytes"
            );
        }
    }
    crate::training_package_validation::validate_contents(&mut zip, &manifest, &seen)
}
