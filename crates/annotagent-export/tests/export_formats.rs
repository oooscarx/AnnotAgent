use std::{collections::BTreeMap, path::PathBuf};

use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue,
    DatasetExporter, ExportRequest, ImageId, ImageMetadata, LabelId, NormalizedPoint,
    NormalizedRect, ProjectSchema, ProjectSnapshot, ReviewStatus, SnapshotImage, TaskId,
};
use annotagent_export::{
    CocoExporter, NativeExporter, YoloDetectionExporter, YoloSegmentationExporter,
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
        annotations: vec![
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
        ],
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
