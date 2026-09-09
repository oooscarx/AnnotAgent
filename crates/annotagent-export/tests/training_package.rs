use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, ImageId,
    LabelId, NormalizedRect, ReviewStatus, dataset_delivery::*,
};
use annotagent_export::training_package::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

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
