//! Dataset exporters with explicit compatibility and skip reports.

mod importers;

pub use importers::*;

use std::{collections::BTreeMap, fs, path::PathBuf};

use annotagent_core::{
    Annotation, AnnotationValue, CoreError, CoreResult, DatasetExporter, ExportCompatibility,
    ExportReport, ExportRequest, ImageId, MaskEncoding, ProjectSnapshot, SnapshotImage, TaskKind,
};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct NativeExporter;

#[async_trait]
impl DatasetExporter for NativeExporter {
    fn format_id(&self) -> &str {
        "native"
    }

    fn compatibility(&self, _project: &ProjectSnapshot) -> ExportCompatibility {
        ExportCompatibility {
            supported: true,
            unsupported_task_kinds: Vec::new(),
            warnings: Vec::new(),
        }
    }

    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport> {
        fs::create_dir_all(&request.output)
            .map_err(|error| CoreError::Export(format!("cannot create output: {error}")))?;
        let path = request.output.join("annotagent-native.json");
        write_json(
            &path,
            &json!({"schema_version": 1, "project": request.project}),
        )?;
        Ok(ExportReport {
            exported_count: request.project.annotations.len() as u64,
            skipped_count: 0,
            warnings: Vec::new(),
            unsupported_task_kinds: Vec::new(),
            output_files: vec![path],
        })
    }
}

pub struct CocoExporter;

#[async_trait]
impl DatasetExporter for CocoExporter {
    fn format_id(&self) -> &str {
        "coco"
    }

    fn compatibility(&self, project: &ProjectSnapshot) -> ExportCompatibility {
        compatibility(
            project,
            &[
                TaskKind::BoundingBox,
                TaskKind::Keypoints,
                TaskKind::Polygon,
                TaskKind::InstanceMask,
            ],
        )
    }

    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport> {
        fs::create_dir_all(&request.output)
            .map_err(|error| CoreError::Export(format!("cannot create output: {error}")))?;
        let categories = categories(&request.project);
        let category_ids: BTreeMap<_, _> = categories
            .iter()
            .enumerate()
            .map(|(index, label)| (label.clone(), index + 1))
            .collect();
        let image_ids: BTreeMap<_, _> = request
            .project
            .images
            .iter()
            .enumerate()
            .map(|(index, image)| (image.id, index + 1))
            .collect();
        let images: Vec<Value> = request
            .project
            .images
            .iter()
            .enumerate()
            .map(|(index, image)| {
                json!({
                    "id": index + 1,
                    "file_name": image.relative_path,
                    "width": image.metadata.width,
                    "height": image.metadata.height,
                })
            })
            .collect();
        let mut annotations = Vec::new();
        let mut skipped = 0_u64;
        let mut warnings = Vec::new();
        for annotation in &request.project.annotations {
            let Some(image) = find_image(&request.project.images, annotation.image_id) else {
                skipped += 1;
                warnings.push(format!(
                    "annotation {} has no image metadata",
                    annotation.id
                ));
                continue;
            };
            let Some(category_id) = annotation
                .label
                .as_ref()
                .and_then(|label| category_ids.get(label.as_str()))
                .copied()
            else {
                skipped += 1;
                warnings.push(format!(
                    "annotation {} has no exportable label",
                    annotation.id
                ));
                continue;
            };
            let Some(coco) = coco_annotation(
                annotation,
                image,
                annotations.len() + 1,
                *image_ids
                    .get(&annotation.image_id)
                    .ok_or_else(|| CoreError::Export("missing image mapping".to_owned()))?,
                category_id,
            ) else {
                skipped += 1;
                warnings.push(format!(
                    "annotation {} kind {:?} is unsupported by COCO export",
                    annotation.id,
                    annotation.value.task_kind()
                ));
                continue;
            };
            annotations.push(coco);
        }
        let path = request.output.join("annotations.coco.json");
        write_json(
            &path,
            &json!({
                "info": {"description": "AnnotAgent export", "version": "1"},
                "images": images,
                "categories": categories.iter().enumerate().map(|(index, name)| {
                    json!({"id": index + 1, "name": name})
                }).collect::<Vec<_>>(),
                "annotations": annotations,
            }),
        )?;
        Ok(report(
            request.project.annotations.len() as u64 - skipped,
            skipped,
            warnings,
            self.compatibility(&request.project).unsupported_task_kinds,
            vec![path],
        ))
    }
}

pub struct YoloDetectionExporter;

#[async_trait]
impl DatasetExporter for YoloDetectionExporter {
    fn format_id(&self) -> &str {
        "yolo_detection"
    }

    fn compatibility(&self, project: &ProjectSnapshot) -> ExportCompatibility {
        compatibility(project, &[TaskKind::BoundingBox])
    }

    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport> {
        export_yolo(&request, false)
    }
}

pub struct YoloSegmentationExporter;

#[async_trait]
impl DatasetExporter for YoloSegmentationExporter {
    fn format_id(&self) -> &str {
        "yolo_segmentation"
    }

    fn compatibility(&self, project: &ProjectSnapshot) -> ExportCompatibility {
        compatibility(project, &[TaskKind::Polygon, TaskKind::InstanceMask])
    }

    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport> {
        export_yolo(&request, true)
    }
}

pub struct LabelMeExporter;

#[async_trait]
impl DatasetExporter for LabelMeExporter {
    fn format_id(&self) -> &str {
        "labelme"
    }

    fn compatibility(&self, project: &ProjectSnapshot) -> ExportCompatibility {
        compatibility(
            project,
            &[
                TaskKind::BoundingBox,
                TaskKind::Keypoints,
                TaskKind::Polyline,
                TaskKind::Polygon,
            ],
        )
    }

    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport> {
        fs::create_dir_all(&request.output)
            .map_err(|error| CoreError::Export(format!("cannot create output: {error}")))?;
        let mut files = Vec::new();
        let mut exported = 0_u64;
        let mut skipped = 0_u64;
        let mut warnings = Vec::new();
        for image in &request.project.images {
            let mut shapes = Vec::new();
            for annotation in request
                .project
                .annotations
                .iter()
                .filter(|annotation| annotation.image_id == image.id)
            {
                if let Some(shape) = labelme_shape(annotation, image) {
                    shapes.push(shape);
                    exported += 1;
                } else {
                    skipped += 1;
                    warnings.push(format!(
                        "annotation {} cannot be represented by LabelMe",
                        annotation.id
                    ));
                }
            }
            let file = request.output.join(format!("{}.json", safe_stem(image)));
            write_json(
                &file,
                &json!({
                    "version": "5.0.0",
                    "flags": {},
                    "shapes": shapes,
                    "imagePath": image.relative_path,
                    "imageData": Value::Null,
                    "imageHeight": image.metadata.height,
                    "imageWidth": image.metadata.width,
                }),
            )?;
            files.push(file);
        }
        Ok(report(
            exported,
            skipped,
            warnings,
            self.compatibility(&request.project).unsupported_task_kinds,
            files,
        ))
    }
}

fn export_yolo(request: &ExportRequest, segmentation: bool) -> CoreResult<ExportReport> {
    fs::create_dir_all(&request.output)
        .map_err(|error| CoreError::Export(format!("cannot create output: {error}")))?;
    let labels = categories(&request.project);
    let label_indices: BTreeMap<_, _> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| (label.as_str(), index))
        .collect();
    let classes = request.output.join("classes.txt");
    fs::write(&classes, labels.join("\n"))
        .map_err(|error| CoreError::Export(format!("cannot write classes: {error}")))?;
    let mut files = vec![classes];
    let mut exported = 0_u64;
    let mut skipped = 0_u64;
    let mut warnings = Vec::new();
    for image in &request.project.images {
        let mut lines = Vec::new();
        for annotation in request
            .project
            .annotations
            .iter()
            .filter(|annotation| annotation.image_id == image.id)
        {
            let Some(class_index) = annotation
                .label
                .as_ref()
                .and_then(|label| label_indices.get(label.as_str()))
                .copied()
            else {
                skipped += 1;
                warnings.push(format!("annotation {} has no YOLO class", annotation.id));
                continue;
            };
            let line = if segmentation {
                yolo_segmentation_line(annotation, class_index)
            } else {
                yolo_detection_line(annotation, class_index)
            };
            if let Some(line) = line {
                lines.push(line);
                exported += 1;
            } else {
                skipped += 1;
                warnings.push(format!(
                    "annotation {} kind {:?} skipped by {}",
                    annotation.id,
                    annotation.value.task_kind(),
                    if segmentation {
                        "YOLO segmentation"
                    } else {
                        "YOLO detection"
                    }
                ));
            }
        }
        let file = request.output.join(format!("{}.txt", safe_stem(image)));
        fs::write(&file, lines.join("\n"))
            .map_err(|error| CoreError::Export(format!("cannot write YOLO labels: {error}")))?;
        files.push(file);
    }
    let unsupported = if segmentation {
        compatibility(
            &request.project,
            &[TaskKind::Polygon, TaskKind::InstanceMask],
        )
        .unsupported_task_kinds
    } else {
        compatibility(&request.project, &[TaskKind::BoundingBox]).unsupported_task_kinds
    };
    Ok(report(exported, skipped, warnings, unsupported, files))
}

fn coco_annotation(
    annotation: &Annotation,
    image: &SnapshotImage,
    id: usize,
    image_id: usize,
    category_id: usize,
) -> Option<Value> {
    let width = f64::from(image.metadata.width);
    let height = f64::from(image.metadata.height);
    let mut value = json!({
        "id": id,
        "image_id": image_id,
        "category_id": category_id,
        "iscrowd": 0,
        "attributes": annotation.attributes,
    });
    match &annotation.value {
        AnnotationValue::BoundingBox { rect } => {
            let bbox = [
                f64::from(rect.x()) * width,
                f64::from(rect.y()) * height,
                f64::from(rect.width()) * width,
                f64::from(rect.height()) * height,
            ];
            value["bbox"] = json!(bbox);
            value["area"] = json!(bbox[2] * bbox[3]);
            value["segmentation"] = json!([]);
        }
        AnnotationValue::Polygon { rings }
        | AnnotationValue::InstanceMask {
            mask: MaskEncoding::Polygon { rings },
        } => {
            let segmentation: Vec<Vec<f64>> = rings
                .iter()
                .map(|ring| {
                    ring.iter()
                        .flat_map(|point| {
                            [f64::from(point.x()) * width, f64::from(point.y()) * height]
                        })
                        .collect()
                })
                .collect();
            value["segmentation"] = json!(segmentation);
            value["area"] = json!(0.0);
            value["bbox"] = json!([]);
        }
        AnnotationValue::Keypoints { points } => {
            value["keypoints"] = json!(
                points
                    .iter()
                    .flat_map(|point| [
                        f64::from(point.point.x()) * width,
                        f64::from(point.point.y()) * height,
                        if point.visible { 2.0 } else { 0.0 }
                    ])
                    .collect::<Vec<_>>()
            );
            value["num_keypoints"] = json!(points.iter().filter(|point| point.visible).count());
            value["bbox"] = json!([]);
            value["area"] = json!(0.0);
        }
        AnnotationValue::InstanceMask {
            mask: MaskEncoding::CocoRle { counts, .. },
        } => {
            value["segmentation"] = json!({
                "size": [image.metadata.height, image.metadata.width],
                "counts": counts
            });
            value["bbox"] = json!([]);
            value["area"] = json!(0.0);
        }
        _ => return None,
    }
    Some(value)
}

fn labelme_shape(annotation: &Annotation, image: &SnapshotImage) -> Option<Value> {
    let label = annotation.label.as_ref()?.as_str();
    let width = image.metadata.width;
    let height = image.metadata.height;
    let point = |point: annotagent_core::NormalizedPoint| {
        let (x, y) = point.to_pixel(width, height);
        json!([x, y])
    };
    let (shape_type, points) = match &annotation.value {
        AnnotationValue::BoundingBox { rect } => (
            "rectangle",
            vec![
                json!([rect.x() * width as f32, rect.y() * height as f32]),
                json!([
                    (rect.x() + rect.width()) * width as f32,
                    (rect.y() + rect.height()) * height as f32
                ]),
            ],
        ),
        AnnotationValue::Keypoints { points } => (
            "point",
            points
                .iter()
                .map(|keypoint| point(keypoint.point))
                .collect(),
        ),
        AnnotationValue::Polyline { points } => (
            "linestrip",
            points.iter().map(|value| point(*value)).collect(),
        ),
        AnnotationValue::Polygon { rings } => (
            "polygon",
            rings.first()?.iter().map(|value| point(*value)).collect(),
        ),
        _ => return None,
    };
    Some(json!({
        "label": label,
        "points": points,
        "group_id": Value::Null,
        "description": "",
        "shape_type": shape_type,
        "flags": {},
    }))
}

fn yolo_detection_line(annotation: &Annotation, class_index: usize) -> Option<String> {
    let AnnotationValue::BoundingBox { rect } = annotation.value else {
        return None;
    };
    Some(format!(
        "{class_index} {:.6} {:.6} {:.6} {:.6}",
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
        rect.width(),
        rect.height()
    ))
}

fn yolo_segmentation_line(annotation: &Annotation, class_index: usize) -> Option<String> {
    let (AnnotationValue::Polygon { rings }
    | AnnotationValue::InstanceMask {
        mask: MaskEncoding::Polygon { rings },
    }) = &annotation.value
    else {
        return None;
    };
    let ring = rings.first()?;
    Some(format!(
        "{class_index} {}",
        ring.iter()
            .map(|point| format!("{:.6} {:.6}", point.x(), point.y()))
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

fn compatibility(project: &ProjectSnapshot, supported: &[TaskKind]) -> ExportCompatibility {
    let unsupported: Vec<String> = project
        .schema
        .tasks
        .iter()
        .filter(|task| !supported.contains(&task.kind))
        .map(|task| format!("{:?}", task.kind).to_ascii_lowercase())
        .collect();
    ExportCompatibility {
        supported: unsupported.is_empty(),
        warnings: if unsupported.is_empty() {
            Vec::new()
        } else {
            vec!["unsupported task kinds will be reported and skipped".to_owned()]
        },
        unsupported_task_kinds: unsupported,
    }
}

fn categories(project: &ProjectSnapshot) -> Vec<String> {
    let mut labels: Vec<String> = project
        .schema
        .tasks
        .iter()
        .flat_map(|task| task.labels.iter().cloned())
        .chain(
            project
                .annotations
                .iter()
                .filter_map(|annotation| annotation.label.as_ref().map(ToString::to_string)),
        )
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

fn find_image(images: &[SnapshotImage], id: ImageId) -> Option<&SnapshotImage> {
    images.iter().find(|image| image.id == id)
}

fn safe_stem(image: &SnapshotImage) -> String {
    image
        .relative_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .map_or_else(|| image.id.to_string(), str::to_owned)
}

fn write_json(path: &PathBuf, value: &Value) -> CoreResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| CoreError::Export(format!("cannot serialize export: {error}")))?;
    fs::write(path, bytes)
        .map_err(|error| CoreError::Export(format!("cannot write {}: {error}", path.display())))
}

fn report(
    exported_count: u64,
    skipped_count: u64,
    warnings: Vec<String>,
    unsupported_task_kinds: Vec<String>,
    output_files: Vec<PathBuf>,
) -> ExportReport {
    ExportReport {
        exported_count,
        skipped_count,
        warnings,
        unsupported_task_kinds,
        output_files,
    }
}

#[cfg(test)]
mod tests {
    use annotagent_core::LabelId;

    use super::*;

    #[test]
    fn bbox_yolo_is_normalized_center_format() {
        let annotation = Annotation {
            id: annotagent_core::AnnotationId::new(),
            image_id: ImageId::new(),
            task_id: "objects".into(),
            label: Some(LabelId::from("item")),
            value: AnnotationValue::BoundingBox {
                rect: annotagent_core::NormalizedRect::new(0.1, 0.2, 0.4, 0.2).expect("rect"),
            },
            attributes: BTreeMap::new(),
            confidence: Some(1.0),
            source: annotagent_core::AnnotationSource::Human,
            review_status: annotagent_core::ReviewStatus::HumanAccepted,
            provenance: annotagent_core::AnnotationProvenance::default(),
            created_at: chrono::Utc::now(),
        };
        assert_eq!(
            yolo_detection_line(&annotation, 2).expect("YOLO bbox"),
            "2 0.300000 0.300000 0.400000 0.200000"
        );
    }
}
