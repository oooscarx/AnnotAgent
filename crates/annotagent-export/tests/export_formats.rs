use std::{collections::BTreeMap, path::PathBuf};

use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationRevision, AnnotationRevisionId,
    AnnotationSource, AnnotationValue, DatasetExporter, DatasetImporter, ExportRequest, ImageId,
    ImageMetadata, ImportRequest, LabelId, NormalizedPoint, NormalizedRect, ProjectSchema,
    ProjectSnapshot, ReviewStatus, RevisionActor, SnapshotImage, TaskId,
};
use annotagent_export::{
    CocoExporter, CocoImporter, LabelMeExporter, LabelMeImporter, NativeExporter, NativeImporter,
    YoloDetectionExporter, YoloDetectionImporter, YoloSegmentationExporter,
    YoloSegmentationImporter,
};
use chrono::Utc;

fn snapshot() -> ProjectSnapshot {
    let image_id = ImageId::new();
    let annotation = |label: &str, value| Annotation {
        id: AnnotationId::new(),
        image_id,
        task_id: TaskId::from("objects"),
        label: Some(LabelId::from(label)),
        value,
        attributes: BTreeMap::new(),
        confidence: Some(0.99),
        source: AnnotationSource::Human,
        review_status: ReviewStatus::HumanAccepted,
        provenance: AnnotationProvenance::default(),
        created_at: Utc::now(),
    };
    let mut annotations = vec![
        annotation(
            "ball",
            AnnotationValue::BoundingBox {
                rect: NormalizedRect::new(0.5, 0.6, 0.05, 0.08).expect("bbox"),
            },
        ),
        annotation(
            "field",
            AnnotationValue::Polygon {
                rings: vec![vec![
                    NormalizedPoint::new(0.0, 0.0).expect("point"),
                    NormalizedPoint::new(1.0, 0.0).expect("point"),
                    NormalizedPoint::new(1.0, 1.0).expect("point"),
                ]],
            },
        ),
    ];
    annotations[1].task_id = TaskId::from("field_region");
    let revision = AnnotationRevision {
        revision_id: AnnotationRevisionId::new(),
        annotation_id: annotations[0].id,
        parent_revision_id: None,
        before: None,
        after: Some(annotations[0].snapshot()),
        actor: RevisionActor::Human,
        reason: Some("fixture_review".to_owned()),
        created_at: Utc::now(),
    };
    ProjectSnapshot {
        schema: ProjectSchema::from_yaml(include_str!("../../../examples/robocup/project.yaml"))
            .expect("project"),
        images: vec![SnapshotImage {
            id: image_id,
            relative_path: PathBuf::from("images/demo.png"),
            metadata: ImageMetadata {
                width: 640,
                height: 400,
                mime_type: "image/png".to_owned(),
                sha256: "fixture".to_owned(),
            },
        }],
        annotations,
        revisions: vec![revision],
    }
}

fn import_request(project: &ProjectSnapshot, source: PathBuf, dry_run: bool) -> ImportRequest {
    ImportRequest {
        project_schema: project.schema.clone(),
        known_images: project.images.clone(),
        source,
        label_mapping: BTreeMap::new(),
        dry_run,
    }
}

#[tokio::test]
async fn coco_and_native_emit_versioned_json_and_report_skips() {
    let temporary = tempfile::tempdir().expect("temporary output");
    let native = NativeExporter
        .export(ExportRequest {
            project: snapshot(),
            output: temporary.path().join("native"),
        })
        .await
        .expect("native export");
    assert_eq!(native.exported_count, 2);
    let native_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&native.output_files[0]).expect("native file"))
            .expect("native JSON");
    assert_eq!(native_json["schema_version"], 1);

    let coco = CocoExporter
        .export(ExportRequest {
            project: snapshot(),
            output: temporary.path().join("coco"),
        })
        .await
        .expect("COCO export");
    assert_eq!(coco.exported_count, 2);
    assert_eq!(coco.skipped_count, 0);
}

#[tokio::test]
async fn yolo_variants_never_silently_drop_incompatible_annotations() {
    let temporary = tempfile::tempdir().expect("temporary output");
    let detection = YoloDetectionExporter
        .export(ExportRequest {
            project: snapshot(),
            output: temporary.path().join("detection"),
        })
        .await
        .expect("YOLO detection");
    assert_eq!((detection.exported_count, detection.skipped_count), (1, 1));
    assert_eq!(detection.warnings.len(), 1);

    let segmentation = YoloSegmentationExporter
        .export(ExportRequest {
            project: snapshot(),
            output: temporary.path().join("segmentation"),
        })
        .await
        .expect("YOLO segmentation");
    assert_eq!(
        (segmentation.exported_count, segmentation.skipped_count),
        (1, 1)
    );
    assert_eq!(segmentation.warnings.len(), 1);
}

#[tokio::test]
async fn native_round_trip_preserves_annotations_provenance_and_revisions() {
    let temporary = tempfile::tempdir().expect("temporary output");
    let original = snapshot();
    let exported = NativeExporter
        .export(ExportRequest {
            project: original.clone(),
            output: temporary.path().join("native"),
        })
        .await
        .expect("native export");
    let imported = NativeImporter
        .import(import_request(
            &original,
            exported.output_files[0].clone(),
            true,
        ))
        .await
        .expect("native import");
    assert!(imported.dry_run);
    assert_eq!(imported.annotations, original.annotations);
    assert_eq!(imported.revisions, original.revisions);
    assert!(imported.issues.is_empty());
}

#[tokio::test]
async fn representable_coco_labelme_and_yolo_data_round_trip_with_reports() {
    let temporary = tempfile::tempdir().expect("temporary output");
    let original = snapshot();
    let coco = CocoExporter
        .export(ExportRequest {
            project: original.clone(),
            output: temporary.path().join("coco"),
        })
        .await
        .expect("COCO export");
    let coco_import = CocoImporter
        .import(import_request(
            &original,
            coco.output_files[0].clone(),
            false,
        ))
        .await
        .expect("COCO import");
    assert_eq!(coco_import.imported_count, 2);
    assert!(
        coco_import
            .warnings
            .iter()
            .any(|warning| warning.contains("provenance"))
    );

    let labelme = LabelMeExporter
        .export(ExportRequest {
            project: original.clone(),
            output: temporary.path().join("labelme"),
        })
        .await
        .expect("LabelMe export");
    let labelme_import = LabelMeImporter
        .import(import_request(
            &original,
            labelme.output_files[0].clone(),
            false,
        ))
        .await
        .expect("LabelMe import");
    assert_eq!(labelme_import.imported_count, 2);

    let detection = YoloDetectionExporter
        .export(ExportRequest {
            project: original.clone(),
            output: temporary.path().join("yolo-detection"),
        })
        .await
        .expect("YOLO detection export");
    let detection_import = YoloDetectionImporter
        .import(import_request(
            &original,
            detection.output_files[0]
                .parent()
                .expect("YOLO directory")
                .to_path_buf(),
            false,
        ))
        .await
        .expect("YOLO detection import");
    assert_eq!(detection_import.imported_count, 1);

    let segmentation = YoloSegmentationExporter
        .export(ExportRequest {
            project: original.clone(),
            output: temporary.path().join("yolo-segmentation"),
        })
        .await
        .expect("YOLO segmentation export");
    let segmentation_import = YoloSegmentationImporter
        .import(import_request(
            &original,
            segmentation.output_files[0]
                .parent()
                .expect("YOLO directory")
                .to_path_buf(),
            false,
        ))
        .await
        .expect("YOLO segmentation import");
    assert_eq!(segmentation_import.imported_count, 1);
}

#[tokio::test]
async fn one_corrupt_labelme_shape_does_not_abort_the_file() {
    let temporary = tempfile::tempdir().expect("temporary output");
    let original = snapshot();
    let file = temporary.path().join("demo.json");
    std::fs::write(
        &file,
        serde_json::to_vec(&serde_json::json!({
            "imagePath": "images/demo.png",
            "imageWidth": 640,
            "imageHeight": 400,
            "shapes": [
                {"label": "ball", "shape_type": "rectangle", "points": [[100, 100], [150, 150]]},
                {"label": "ball", "shape_type": "polygon", "points": [[1, 1]]}
            ]
        }))
        .expect("JSON"),
    )
    .expect("fixture");
    let report = LabelMeImporter
        .import(import_request(&original, file, true))
        .await
        .expect("partial import");
    assert_eq!((report.imported_count, report.skipped_count), (1, 1));
    assert_eq!(report.issues.len(), 1);
}
