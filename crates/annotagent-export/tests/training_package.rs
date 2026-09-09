use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, ImageId,
    LabelId, NormalizedRect, ReviewStatus, dataset_delivery::*,
};
use annotagent_export::training_package::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};

fn fixture(root: &Path) -> (TaskDeliveryIntent, Vec<PackageImage>) {
    let mut scope = Vec::new();
    let mut sources = Vec::new();
    for i in 0..12_u8 {
        let id = ImageId::new();
        let dir = root.join(format!("original-{i}"));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("same-name.png");
        image::RgbImage::from_pixel(1000, 500, image::Rgb([i, 40, 80]))
            .save(&source)
            .unwrap();
        scope.push(DeliveryImage {
            image_id: id,
            content_sha256: format!("{:x}", Sha256::digest(fs::read(&source).unwrap())),
            content_revision: "TEST-1".into(),
            existing_split: None,
            group_ids: vec![],
        });
        let annotations = if i < 10 {
            ["stable-cup", "stable-ball"]
                .iter()
                .map(|label| Annotation {
                    id: AnnotationId::new(),
                    image_id: id,
                    task_id: "objects".into(),
                    label: Some(LabelId::from(*label)),
                    value: AnnotationValue::BoundingBox {
                        rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
                    },
                    attributes: BTreeMap::new(),
                    confidence: None,
                    source: AnnotationSource::Human,
                    review_status: ReviewStatus::HumanAccepted,
                    provenance: AnnotationProvenance::default(),
                    created_at: chrono::Utc::now(),
                })
                .collect()
        } else {
            vec![]
        };
        let confirmation = match i {
            10 => ImageConfirmation::NegativeConfirmed {
                confirmation_id: "TEST-negative-confirmation".into(),
            },
            11 => ImageConfirmation::Excluded {
                confirmation_id: "TEST-exclusion-consent".into(),
                reason: "TEST unresolved image explicitly excluded".into(),
            },
            _ => ImageConfirmation::PositiveComplete {
                confirmation_id: format!("TEST-human-image-{i}"),
            },
        };
        sources.push(PackageImage {
            image_id: id,
            source,
            annotations,
            confirmation,
        });
    }
    let intent = TaskDeliveryIntent {
        version: 1,
        project_id: ImageId::new().to_string(),
        conversation_id: ImageId::new().0,
        task_id: ImageId::new().0,
        dataset_scope: Some(scope),
        label_spec: Some(
            [("stable-cup", "杯子"), ("stable-ball", "球")]
                .map(|(id, name)| DeliveryLabel {
                    stable_id: id.into(),
                    display_name: name.into(),
                    aliases: vec![],
                    include: String::new(),
                    exclude: String::new(),
                })
                .to_vec(),
        ),
        training_target: Some(TrainingTarget {
            annotation_kind: annotagent_core::TaskKind::BoundingBox,
            framework: "ultralytics".into(),
            export_profile: DETECTION_PROFILE.into(),
            profile_revision: 1,
        }),
        split_policy: DeliverySplitPolicy::default(),
        review_policy: DeliveryReviewPolicy::HumanWholeImage,
    };
    (intent, sources)
}

#[test]
fn real_zip_contains_originals_negative_labels_mapping_and_portable_yaml() {
    let temp = tempfile::tempdir().unwrap();
    let (intent, mut sources) = fixture(temp.path());
    let expected = sources
        .iter()
        .map(|s| (s.image_id, fs::read(&s.source).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let destination = temp.path().join("training.zip");
    let receipt = write_training_package(intent.clone(), 1, &sources, &destination).unwrap();
    sources.reverse();
    let repeated =
        write_training_package(intent, 1, &sources, &temp.path().join("repeated.zip")).unwrap();
    assert_eq!(
        receipt.sha256, repeated.sha256,
        "same frozen input must produce identical bytes regardless of source enumeration"
    );
    assert_eq!(
        (
            receipt.images,
            receipt.objects,
            receipt.negatives,
            receipt.excluded
        ),
        (11, 20, 1, 1)
    );
    assert_eq!(
        receipt.sha256,
        format!("{:x}", Sha256::digest(fs::read(&destination).unwrap()))
    );
    validate_training_package(&destination).unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(&destination).unwrap()).unwrap();
    let manifest: PackageManifest =
        serde_json::from_reader(archive.by_name("annotagent/manifest.json").unwrap()).unwrap();
    let mut readme = String::new();
    archive
        .by_name("README.md")
        .unwrap()
        .read_to_string(&mut readme)
        .unwrap();
    assert!(readme.contains("0: 杯子 (stable label: stable-cup)"));
    assert!(readme.contains("1: 球 (stable label: stable-ball)"));
    assert!(readme.contains("Included originals: 11; objects: 20; explicitly confirmed negative images: 1; excluded images: 1."));
    for (split, name) in [(DatasetSplit::Train, "train"), (DatasetSplit::Val, "val")] {
        let count = manifest
            .images
            .iter()
            .filter(|image| image.split == Some(split))
            .count();
        assert!(readme.contains(&format!("- {name}: {count} original images")));
    }
    assert!(readme.contains("official framework loader smoke test was not executed"));
    assert!(!readme.contains(temp.path().to_str().unwrap()));
    for record in &manifest.images {
        let Some(path) = &record.image else {
            continue;
        };
        let mut bytes = Vec::new();
        archive
            .by_name(path)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, expected[&record.image_id]);
        let mut text = String::new();
        archive
            .by_name(record.label.as_ref().unwrap())
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        if matches!(
            record.confirmation,
            ImageConfirmation::NegativeConfirmed { .. }
        ) {
            assert!(text.is_empty());
        } else {
            let row = text
                .lines()
                .nth(1)
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            assert_eq!(row[0], "1");
            for value in &row[1..] {
                assert!((value.parse::<f64>().unwrap() - 0.2).abs() < 0.000_001);
            }
        }
    }
    for folder in ["extract-one", "含 空格的数据包"] {
        let directory = temp.path().join(folder);
        fs::create_dir(&directory).unwrap();
        archive.extract(&directory).unwrap();
        let yaml: serde_yaml::Value =
            serde_yaml::from_slice(&fs::read(directory.join("data.yaml")).unwrap()).unwrap();
        assert!(yaml.get("path").is_none());
        for split in ["train", "val"] {
            let path = directory.join(yaml[split].as_str().unwrap());
            assert!(path.is_dir());
            assert!(fs::read_dir(path).unwrap().next().is_some());
        }
    }
    assert!(!temp.path().join("training.zip.partial").exists());
}

#[test]
fn unresolved_image_or_changed_original_never_publishes_zip() {
    let temp = tempfile::tempdir().unwrap();
    let (intent, mut sources) = fixture(temp.path());
    sources[0].confirmation = ImageConfirmation::NeedsReview;
    let destination = temp.path().join("blocked.zip");
    assert!(write_training_package(intent.clone(), 1, &sources, &destination).is_err());
    assert!(!destination.exists());
    let (intent, sources) = fixture(temp.path());
    fs::write(&sources[0].source, b"not an image").unwrap();
    assert!(write_training_package(intent, 1, &sources, &destination).is_err());
    assert!(!destination.exists());
}

#[test]
fn cancellation_during_copy_or_validation_never_publishes_or_removes_other_files() {
    let temp = tempfile::tempdir().unwrap();
    let (intent, sources) = fixture(temp.path());
    let unrelated = temp.path().join("unrelated.txt");
    fs::write(&unrelated, b"TEST preserve").unwrap();
    for (name, stage, occurrence) in [
        ("during-copy", PackageProgress::Exporting, 1),
        ("before-validation", PackageProgress::Validating, 1),
        ("after-validation", PackageProgress::Validating, 2),
    ] {
        let destination = temp.path().join(format!("{name}.zip"));
        let partial = temp.path().join(format!("{name}.zip.partial"));
        let mut count = 0;
        let error = write_training_package_controlled(
            intent.clone(),
            1,
            &sources,
            &destination,
            |current| {
                if current == stage && partial.exists() {
                    count += 1;
                    if count == occurrence {
                        anyhow::bail!("TEST cancelled at checkpoint");
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("TEST cancelled"));
        assert!(!destination.exists());
        assert!(!partial.exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"TEST preserve");
    }
}

#[test]
fn group_conflicts_and_object_only_acceptance_do_not_imply_image_readiness() {
    let temp = tempfile::tempdir().unwrap();
    let (mut intent, mut sources) = fixture(temp.path());
    let scope = intent.dataset_scope.as_mut().unwrap();
    scope[0].group_ids = vec!["same-capture".into()];
    scope[1].group_ids = vec!["same-capture".into()];
    scope[0].existing_split = Some(DatasetSplit::Train);
    scope[1].existing_split = Some(DatasetSplit::Val);
    let destination = temp.path().join("must-not-exist.zip");
    assert!(
        write_training_package(intent.clone(), 1, &sources, &destination)
            .unwrap_err()
            .to_string()
            .contains("Existing splits conflict")
    );
    let scope = intent.dataset_scope.as_mut().unwrap();
    scope[0].existing_split = None;
    scope[1].existing_split = None;
    sources[0].confirmation = ImageConfirmation::Incomplete;
    assert!(write_training_package(intent.clone(), 1, &sources, &destination).is_err());
    sources[0].confirmation = ImageConfirmation::PositiveComplete {
        confirmation_id: "TEST-image-confirmation".into(),
    };
    sources[0].annotations[0].review_status = ReviewStatus::NeedsReview;
    assert!(write_training_package(intent, 1, &sources, &destination).is_err());
    assert!(!destination.exists());
}

// Repair checksums after tampering so semantic validation, not hashing, must reject it.
fn rewrite_with_matching_hashes(
    source: &Path,
    target: &Path,
    change: impl FnOnce(&mut BTreeMap<String, Vec<u8>>, &mut PackageManifest),
) {
    let mut archive = zip::ZipArchive::new(fs::File::open(source).unwrap()).unwrap();
    let mut files = BTreeMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        files.insert(name, bytes);
    }
    let mut manifest: PackageManifest =
        serde_json::from_slice(&files.remove("annotagent/manifest.json").unwrap()).unwrap();
    change(&mut files, &mut manifest);
    manifest.intent_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&manifest.intent).unwrap())
    );
    files.insert(
        "annotagent/split-manifest.json".into(),
        serde_json::to_vec(&manifest.images).unwrap(),
    );
    files.insert(
        "annotagent/exclusions.json".into(),
        serde_json::to_vec(
            &manifest
                .images
                .iter()
                .filter(|i| i.image.is_none())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    );
    manifest.files = files
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                FileEvidence {
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                    bytes: bytes.len() as u64,
                },
            )
        })
        .collect();
    files.insert(
        "annotagent/manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut output = zip::ZipWriter::new(fs::File::create(target).unwrap());
    for (name, bytes) in files {
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap();
}

#[test]
fn package_lineage_is_explicit_and_independently_checked() {
    let temp = tempfile::tempdir().unwrap();
    let (intent, mut sources) = fixture(temp.path());
    let mut images = BTreeMap::new();
    for source in &mut sources {
        let id = ImageId::new().to_string();
        match &mut source.confirmation {
            ImageConfirmation::PositiveComplete { confirmation_id }
            | ImageConfirmation::NegativeConfirmed { confirmation_id }
            | ImageConfirmation::Excluded {
                confirmation_id, ..
            } => *confirmation_id = id.clone(),
            _ => unreachable!(),
        }
        images.insert(
            source.image_id,
            PackageImageLineage {
                schema_sha256: Some("d".repeat(64)),
                workflow_sha256: None,
                model_binding_sha256: Some("e".repeat(64)),
                original_name: "same-name.png".into(),
                source_run_id: Some(annotagent_core::RunId::new()),
                confirmation_id: id,
                confirmation_revision: 1,
                annotation_revision_ids: source
                    .annotations
                    .iter()
                    .map(|_| annotagent_core::AnnotationRevisionId::new().to_string())
                    .collect(),
                annotation_snapshot_sha256: "a".repeat(64),
                source_evidence_sha256: Some("b".repeat(64)),
            },
        );
    }
    let lineage = PackageLineage {
        package_id: ImageId::new().to_string(),
        package_version: 1,
        frozen_snapshot_sha256: "c".repeat(64),
        label_id_to_class_id: BTreeMap::from([("stable-cup".into(), 0), ("stable-ball".into(), 1)]),
        exporter_version: "TEST".into(),
        validator_version: 1,
        images,
    };
    let original = temp.path().join("lineage.zip");
    write_training_package_with_lineage(intent, 1, &sources, &original, Some(lineage), |_| Ok(()))
        .unwrap();
    validate_training_package(&original).unwrap();
    for case in [
        "missing",
        "classes",
        "private-path",
        "confirmation",
        "source",
        "revision",
    ] {
        let target = temp.path().join(format!("bad-lineage-{case}.zip"));
        rewrite_with_matching_hashes(&original, &target, |_, manifest| {
            if case == "missing" {
                manifest.lineage = None;
                return;
            }
            let lineage = manifest.lineage.as_mut().unwrap();
            match case {
                "classes" => {
                    lineage.label_id_to_class_id.insert("stable-cup".into(), 41);
                }
                "private-path" => {
                    lineage.images.values_mut().next().unwrap().original_name =
                        "/private/user/image.png".into();
                }
                "confirmation" => {
                    lineage.images.values_mut().next().unwrap().confirmation_id =
                        ImageId::new().to_string();
                }
                "source" => {
                    lineage
                        .images
                        .values_mut()
                        .next()
                        .unwrap()
                        .source_evidence_sha256 = None;
                }
                "revision" => lineage
                    .images
                    .values_mut()
                    .find(|i| !i.annotation_revision_ids.is_empty())
                    .unwrap()
                    .annotation_revision_ids
                    .clear(),
                _ => unreachable!(),
            }
        });
        assert!(
            validate_training_package(&target).is_err(),
            "must reject {case}"
        );
    }
}

#[test]
fn independent_validator_rejects_semantic_tampering_even_with_matching_hashes() {
    let temp = tempfile::tempdir().unwrap();
    let (intent, sources) = fixture(temp.path());
    let original = temp.path().join("valid.zip");
    write_training_package(intent, 1, &sources, &original).unwrap();
    let rewritten = temp.path().join("rewritten-valid.zip");
    rewrite_with_matching_hashes(&original, &rewritten, |_, _| {});
    validate_training_package(&rewritten).unwrap();
    for (case, row) in [
        ("nan", "0 NaN 0.2 0.2 0.2\n"),
        ("outside", "0 0.9 0.2 0.8 0.2\n"),
        ("class", "99 0.2 0.2 0.2 0.2\n"),
        ("empty", "0 0.2 0.2 0 0.2\n"),
    ] {
        let target = temp.path().join(format!("{case}.zip"));
        rewrite_with_matching_hashes(&original, &target, |files, manifest| {
            let image = manifest
                .images
                .iter()
                .find(|i| matches!(i.confirmation, ImageConfirmation::PositiveComplete { .. }))
                .unwrap();
            files.insert(image.label.clone().unwrap(), row.as_bytes().to_vec());
        });
        assert!(
            validate_training_package(&target).is_err(),
            "must reject {case} with matching hashes"
        );
    }
    let target = temp.path().join("unsafe-yaml.zip");
    rewrite_with_matching_hashes(&original, &target, |files, _| {
        files.insert("data.yaml".into(),b"train: /private/images\nval: /private/images\ndownload: unsafe\nnames: {0: cup, 1: ball}\n".to_vec());
    });
    assert!(validate_training_package(&target).is_err());
    let target = temp.path().join("leaked-secret.zip");
    rewrite_with_matching_hashes(&original, &target, |files, _| {
        files.insert(
            "credentials.json".into(),
            b"TEST fixture, not a real credential".to_vec(),
        );
    });
    assert!(
        validate_training_package(&target)
            .unwrap_err()
            .to_string()
            .contains("Unexpected files")
    );
    let target = temp.path().join("group-leak.zip");
    rewrite_with_matching_hashes(&original, &target, |_, manifest| {
        for split in [DatasetSplit::Train, DatasetSplit::Val] {
            let id = manifest
                .images
                .iter()
                .find(|i| i.split == Some(split))
                .unwrap()
                .image_id;
            manifest
                .intent
                .dataset_scope
                .as_mut()
                .unwrap()
                .iter_mut()
                .find(|i| i.image_id == id)
                .unwrap()
                .group_ids = vec!["TEST-capture".into()];
        }
    });
    assert!(
        validate_training_package(&target)
            .unwrap_err()
            .to_string()
            .contains("leaks across splits")
    );
    let target = temp.path().join("corrupt-pixels.zip");
    rewrite_with_matching_hashes(&original, &target, |files, manifest| {
        let image = manifest.images.iter().find(|i| i.image.is_some()).unwrap();
        let bytes = files.get_mut(image.image.as_ref().unwrap()).unwrap();
        let idat = bytes.windows(4).position(|v| v == b"IDAT").unwrap();
        bytes[idat + 8] ^= 0xff;
        manifest
            .intent
            .dataset_scope
            .as_mut()
            .unwrap()
            .iter_mut()
            .find(|i| i.image_id == image.image_id)
            .unwrap()
            .content_sha256 = format!("{:x}", Sha256::digest(bytes));
    });
    assert!(validate_training_package(&target).is_err());
}

#[test]
fn empty_positive_training_set_is_blocked_and_missing_class_has_a_warning() {
    let temp = tempfile::tempdir().unwrap();
    let (mut intent, mut sources) = fixture(temp.path());
    intent.label_spec.as_mut().unwrap().push(DeliveryLabel {
        stable_id: "unused".into(),
        display_name: "未出现类别".into(),
        aliases: vec![],
        include: String::new(),
        exclude: String::new(),
    });
    let path = temp.path().join("rare-class.zip");
    write_training_package(intent.clone(), 1, &sources, &path).unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let manifest: PackageManifest =
        serde_json::from_reader(archive.by_name("annotagent/manifest.json").unwrap()).unwrap();
    assert!(
        manifest
            .warnings
            .iter()
            .any(|warning| warning.contains("未出现类别") && warning.contains("only 0"))
    );
    for source in &mut sources {
        source.annotations.clear();
        if !matches!(source.confirmation, ImageConfirmation::Excluded { .. }) {
            source.confirmation = ImageConfirmation::NegativeConfirmed {
                confirmation_id: "TEST-negative-reviewed".into(),
            };
        }
    }
    let empty = temp.path().join("all-negative.zip");
    assert!(
        write_training_package(intent, 1, &sources, &empty)
            .unwrap_err()
            .to_string()
            .contains("positive training example")
    );
    assert!(!empty.exists());
}
