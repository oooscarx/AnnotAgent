use std::{collections::BTreeMap, fs, path::Path};

use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, CoreError,
    CoreResult, DatasetImporter, ImageId, ImportIssue, ImportReport, ImportRequest, Keypoint,
    LabelId, MaskEncoding, NormalizedPoint, NormalizedRect, ReviewStatus, SnapshotImage,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

pub struct NativeImporter;
pub struct CocoImporter;
pub struct LabelMeImporter;
pub struct YoloDetectionImporter;
pub struct YoloSegmentationImporter;

#[async_trait]
impl DatasetImporter for NativeImporter {
    fn format_id(&self) -> &str {
        "native"
    }

    async fn import(&self, request: ImportRequest) -> CoreResult<ImportReport> {
        let root = read_json(&request.source)?;
        if root["schema_version"].as_u64() != Some(1) {
            return Err(CoreError::Validation(
                "native import schema_version must be 1".to_owned(),
            ));
        }
        let project = &root["project"];
        let source_images = project["images"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| serde_json::from_value::<SnapshotImage>(value).ok())
            .collect::<Vec<_>>();
        let image_map = source_images
            .iter()
            .filter_map(|source| {
                find_image(&request.known_images, &source.relative_path)
                    .map(|target| (source.id, target.id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut report = empty_report(self.format_id(), request.dry_run);
        for (index, value) in project["annotations"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            let record = format!("annotations[{index}]");
            match serde_json::from_value::<Annotation>(value)
                .map_err(|error| error.to_string())
                .and_then(|mut annotation| {
                    annotation.image_id =
                        image_map
                            .get(&annotation.image_id)
                            .copied()
                            .ok_or_else(|| {
                                "source image does not exist in target Project".to_owned()
                            })?;
                    map_annotation(&request, &mut annotation)?;
                    annotation.validate().map_err(|error| error.to_string())?;
                    Ok(annotation)
                }) {
                Ok(annotation) => report.annotations.push(annotation),
                Err(message) => report.issues.push(ImportIssue { record, message }),
            }
        }
        let imported_ids = report
            .annotations
            .iter()
            .map(|annotation| annotation.id)
            .collect::<std::collections::BTreeSet<_>>();
        for (index, value) in project["revisions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            match serde_json::from_value::<annotagent_core::AnnotationRevision>(value) {
                Ok(revision) if imported_ids.contains(&revision.annotation_id) => {
                    report.revisions.push(revision);
                }
                Ok(_) => report.warnings.push(format!(
                    "revisions[{index}] was skipped because its annotation was not imported"
                )),
                Err(error) => report.issues.push(ImportIssue {
                    record: format!("revisions[{index}]"),
                    message: error.to_string(),
                }),
            }
        }
        finish(&mut report);
        Ok(report)
    }
}

#[async_trait]
impl DatasetImporter for CocoImporter {
    fn format_id(&self) -> &str {
        "coco"
    }

    async fn import(&self, request: ImportRequest) -> CoreResult<ImportReport> {
        let root = read_json(&request.source)?;
        let images = root["images"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| {
                Some((
                    value["id"].as_i64()?,
                    (
                        value["file_name"].as_str()?.to_owned(),
                        value["width"].as_u64()? as u32,
                        value["height"].as_u64()? as u32,
                    ),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let categories = root["categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| Some((value["id"].as_i64()?, value["name"].as_str()?.to_owned())))
            .collect::<BTreeMap<_, _>>();
        let mut report = empty_report(self.format_id(), request.dry_run);
        for (index, value) in root["annotations"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            let parsed = (|| -> Result<Annotation, String> {
                let image = images
                    .get(&value["image_id"].as_i64().ok_or("missing image_id")?)
                    .ok_or("unknown image_id")?;
                let target = find_image(&request.known_images, Path::new(&image.0))
                    .ok_or("COCO image does not exist in target Project")?;
                let label = categories
                    .get(&value["category_id"].as_i64().ok_or("missing category_id")?)
                    .ok_or("unknown category_id")?;
                let label = mapped_label(&request, label);
                let annotation_value = coco_value(&value, image.1, image.2)?;
                imported_annotation(&request, target.id, &label, annotation_value)
            })();
            match parsed {
                Ok(annotation) => report.annotations.push(annotation),
                Err(message) => report.issues.push(ImportIssue {
                    record: format!("annotations[{index}]"),
                    message,
                }),
            }
        }
        report.warnings.push(
            "COCO cannot preserve AnnotAgent revision chains or full provenance; imported records enter Review"
                .to_owned(),
        );
        finish(&mut report);
        Ok(report)
    }
}

#[async_trait]
impl DatasetImporter for LabelMeImporter {
    fn format_id(&self) -> &str {
        "labelme"
    }

    async fn import(&self, request: ImportRequest) -> CoreResult<ImportReport> {
        let files = json_files(&request.source)?;
        let mut report = empty_report(self.format_id(), request.dry_run);
        for file in files {
            let root = match read_json(&file) {
                Ok(value) => value,
                Err(error) => {
                    report.issues.push(ImportIssue {
                        record: file.display().to_string(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            let image_path = root["imagePath"].as_str().unwrap_or_default();
            let Some(image) = find_image(&request.known_images, Path::new(image_path)) else {
                report.issues.push(ImportIssue {
                    record: file.display().to_string(),
                    message: "LabelMe image does not exist in target Project".to_owned(),
                });
                continue;
            };
            let width = root["imageWidth"]
                .as_f64()
                .unwrap_or(f64::from(image.metadata.width));
            let height = root["imageHeight"]
                .as_f64()
                .unwrap_or(f64::from(image.metadata.height));
            for (index, shape) in root["shapes"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
            {
                let parsed = (|| -> Result<Annotation, String> {
                    let label = mapped_label(
                        &request,
                        shape["label"].as_str().ok_or("shape has no label")?,
                    );
                    let points = pixel_points(&shape["points"], width, height)?;
                    let value = match shape["shape_type"].as_str().unwrap_or("polygon") {
                        "rectangle" if points.len() == 2 => AnnotationValue::BoundingBox {
                            rect: rect_from_corners(points[0], points[1])?,
                        },
                        "point" if !points.is_empty() => AnnotationValue::Keypoints {
                            points: points
                                .into_iter()
                                .enumerate()
                                .map(|(index, point)| Keypoint {
                                    name: format!("point_{index}"),
                                    point,
                                    visible: true,
                                })
                                .collect(),
                        },
                        "line" | "linestrip" if points.len() >= 2 => {
                            AnnotationValue::Polyline { points }
                        }
                        "polygon" if points.len() >= 3 => AnnotationValue::Polygon {
                            rings: vec![points],
                        },
                        other => return Err(format!("unsupported or malformed shape {other:?}")),
                    };
                    imported_annotation(&request, image.id, &label, value)
                })();
                match parsed {
                    Ok(annotation) => report.annotations.push(annotation),
                    Err(message) => report.issues.push(ImportIssue {
                        record: format!("{}:shapes[{index}]", file.display()),
                        message,
                    }),
                }
            }
        }
        report.warnings.push(
            "LabelMe cannot preserve revision chains, provenance, masks, relations, or typed attributes"
                .to_owned(),
        );
        finish(&mut report);
        Ok(report)
    }
}

macro_rules! yolo_importer {
    ($type:ty, $format:literal, $segmentation:literal) => {
        #[async_trait]
        impl DatasetImporter for $type {
            fn format_id(&self) -> &str {
                $format
            }
            async fn import(&self, request: ImportRequest) -> CoreResult<ImportReport> {
                import_yolo(&request, $format, $segmentation)
            }
        }
    };
}

yolo_importer!(YoloDetectionImporter, "yolo_detection", false);
yolo_importer!(YoloSegmentationImporter, "yolo_segmentation", true);

fn import_yolo(
    request: &ImportRequest,
    format: &str,
    segmentation: bool,
) -> CoreResult<ImportReport> {
    let root = request
        .source
        .parent()
        .filter(|_| request.source.is_file())
        .unwrap_or(&request.source);
    let classes = fs::read_to_string(root.join("classes.txt"))
        .map_err(|error| CoreError::Validation(format!("cannot read YOLO classes.txt: {error}")))?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut report = empty_report(format, request.dry_run);
    for image in &request.known_images {
        let Some(stem) = image
            .relative_path
            .file_stem()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        let file = root.join(format!("{stem}.txt"));
        if !file.is_file() {
            continue;
        }
        for (index, line) in fs::read_to_string(&file)
            .map_err(|error| CoreError::Validation(error.to_string()))?
            .lines()
            .enumerate()
        {
            let parsed = (|| -> Result<Annotation, String> {
                let values = line
                    .split_whitespace()
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                let class = *values.first().ok_or("empty YOLO row")? as usize;
                let label = mapped_label(request, classes.get(class).ok_or("unknown YOLO class")?);
                let value = if segmentation {
                    if values.len() < 7 || values.len() % 2 == 0 {
                        return Err("YOLO polygon needs at least three x/y pairs".to_owned());
                    }
                    let points = values[1..]
                        .chunks_exact(2)
                        .map(|pair| {
                            NormalizedPoint::new(pair[0], pair[1])
                                .map_err(|error| error.to_string())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    AnnotationValue::Polygon {
                        rings: vec![points],
                    }
                } else {
                    if values.len() != 5 {
                        return Err("YOLO bbox needs class cx cy width height".to_owned());
                    }
                    AnnotationValue::BoundingBox {
                        rect: NormalizedRect::new(
                            values[1] - values[3] / 2.0,
                            values[2] - values[4] / 2.0,
                            values[3],
                            values[4],
                        )
                        .map_err(|error| error.to_string())?,
                    }
                };
                imported_annotation(request, image.id, &label, value)
            })();
            match parsed {
                Ok(annotation) => report.annotations.push(annotation),
                Err(message) => report.issues.push(ImportIssue {
                    record: format!("{}:{index}", file.display()),
                    message,
                }),
            }
        }
    }
    report.warnings.push(
        "YOLO cannot preserve revision chains, provenance, attributes, or non-geometry task data"
            .to_owned(),
    );
    finish(&mut report);
    Ok(report)
}

fn coco_value(value: &Value, width: u32, height: u32) -> Result<AnnotationValue, String> {
    if let Some(segmentation) = value["segmentation"].as_object() {
        let size = segmentation
            .get("size")
            .and_then(Value::as_array)
            .ok_or("COCO RLE has no size")?;
        let mask_height = size
            .first()
            .and_then(Value::as_u64)
            .ok_or("COCO RLE has no height")? as u32;
        let mask_width = size
            .get(1)
            .and_then(Value::as_u64)
            .ok_or("COCO RLE has no width")? as u32;
        let counts = segmentation
            .get("counts")
            .and_then(Value::as_str)
            .ok_or("only compressed string COCO RLE counts are supported")?;
        return Ok(AnnotationValue::InstanceMask {
            mask: MaskEncoding::CocoRle {
                width: mask_width,
                height: mask_height,
                counts: counts.to_owned(),
            },
        });
    }
    if let Some(rings) = value["segmentation"]
        .as_array()
        .filter(|rings| !rings.is_empty())
    {
        let parsed = rings
            .iter()
            .map(|ring| pixel_points(ring, f64::from(width), f64::from(height)))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(AnnotationValue::Polygon { rings: parsed });
    }
    if let Some(points) = value["keypoints"]
        .as_array()
        .filter(|points| !points.is_empty())
    {
        let values = points.iter().filter_map(Value::as_f64).collect::<Vec<_>>();
        if values.len() % 3 != 0 {
            return Err("COCO keypoints must be x/y/visibility triples".to_owned());
        }
        return Ok(AnnotationValue::Keypoints {
            points: values
                .chunks_exact(3)
                .enumerate()
                .map(|(index, point)| {
                    Ok(Keypoint {
                        name: format!("point_{index}"),
                        point: NormalizedPoint::new(
                            (point[0] / f64::from(width)) as f32,
                            (point[1] / f64::from(height)) as f32,
                        )
                        .map_err(|error| error.to_string())?,
                        visible: point[2] > 0.0,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        });
    }
    let bbox = value["bbox"]
        .as_array()
        .ok_or("COCO annotation has no supported geometry")?
        .iter()
        .filter_map(Value::as_f64)
        .collect::<Vec<_>>();
    if bbox.len() != 4 {
        return Err("COCO bbox must have four values".to_owned());
    }
    Ok(AnnotationValue::BoundingBox {
        rect: NormalizedRect::new(
            (bbox[0] / f64::from(width)) as f32,
            (bbox[1] / f64::from(height)) as f32,
            (bbox[2] / f64::from(width)) as f32,
            (bbox[3] / f64::from(height)) as f32,
        )
        .map_err(|error| error.to_string())?,
    })
}

fn imported_annotation(
    request: &ImportRequest,
    image_id: ImageId,
    label: &str,
    value: AnnotationValue,
) -> Result<Annotation, String> {
    let task = request
        .project_schema
        .tasks
        .iter()
        .find(|task| {
            task.kind == value.task_kind() && task.labels.iter().any(|candidate| candidate == label)
        })
        .or_else(|| {
            request
                .project_schema
                .tasks
                .iter()
                .find(|task| task.kind == value.task_kind())
        })
        .ok_or_else(|| format!("target Project has no {:?} task", value.task_kind()))?;
    let annotation = Annotation {
        id: AnnotationId::new(),
        image_id,
        task_id: task.id.clone(),
        label: Some(LabelId::from(label)),
        value,
        attributes: BTreeMap::new(),
        confidence: None,
        source: AnnotationSource::Imported,
        review_status: ReviewStatus::NeedsReview,
        provenance: AnnotationProvenance::default(),
        created_at: Utc::now(),
    };
    annotation.validate().map_err(|error| error.to_string())?;
    Ok(annotation)
}

fn map_annotation(request: &ImportRequest, annotation: &mut Annotation) -> Result<(), String> {
    if let Some(label) = annotation.label.as_ref() {
        annotation.label = Some(LabelId::from(mapped_label(request, label.as_str())));
    }
    let kind = annotation.value.task_kind();
    let task = request
        .project_schema
        .tasks
        .iter()
        .find(|task| task.id == annotation.task_id && task.kind == kind)
        .or_else(|| {
            request
                .project_schema
                .tasks
                .iter()
                .find(|task| task.kind == kind)
        })
        .ok_or_else(|| format!("target Project has no {kind:?} task"))?;
    annotation.task_id = task.id.clone();
    Ok(())
}

fn mapped_label(request: &ImportRequest, label: &str) -> String {
    request
        .label_mapping
        .get(label)
        .cloned()
        .unwrap_or_else(|| label.to_owned())
}
fn find_image<'a>(images: &'a [SnapshotImage], path: &Path) -> Option<&'a SnapshotImage> {
    let name = path.file_name()?;
    images
        .iter()
        .find(|image| image.relative_path == path || image.relative_path.file_name() == Some(name))
}
fn read_json(path: &Path) -> CoreResult<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|error| {
        CoreError::Validation(format!("cannot read {}: {error}", path.display()))
    })?)
    .map_err(|error| CoreError::Validation(format!("invalid JSON {}: {error}", path.display())))
}
fn json_files(path: &Path) -> CoreResult<Vec<std::path::PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = fs::read_dir(path)
        .map_err(|error| CoreError::Validation(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}
fn pixel_points(value: &Value, width: f64, height: f64) -> Result<Vec<NormalizedPoint>, String> {
    let raw = value.as_array().ok_or("points must be an array")?;
    if raw.first().is_some_and(Value::is_number) {
        let values = raw.iter().filter_map(Value::as_f64).collect::<Vec<_>>();
        return values
            .chunks_exact(2)
            .map(|pair| {
                NormalizedPoint::new((pair[0] / width) as f32, (pair[1] / height) as f32)
                    .map_err(|error| error.to_string())
            })
            .collect();
    }
    raw.iter()
        .map(|point| {
            let pair = point.as_array().ok_or("point must be [x,y]")?;
            NormalizedPoint::new(
                (pair.first().and_then(Value::as_f64).ok_or("missing x")? / width) as f32,
                (pair.get(1).and_then(Value::as_f64).ok_or("missing y")? / height) as f32,
            )
            .map_err(|error| error.to_string())
        })
        .collect()
}
fn rect_from_corners(
    left: NormalizedPoint,
    right: NormalizedPoint,
) -> Result<NormalizedRect, String> {
    let x = left.x().min(right.x());
    let y = left.y().min(right.y());
    NormalizedRect::new(
        x,
        y,
        (left.x() - right.x()).abs(),
        (left.y() - right.y()).abs(),
    )
    .map_err(|error| error.to_string())
}
fn empty_report(format: &str, dry_run: bool) -> ImportReport {
    ImportReport {
        format: format.to_owned(),
        dry_run,
        imported_count: 0,
        skipped_count: 0,
        annotations: Vec::new(),
        revisions: Vec::new(),
        warnings: Vec::new(),
        issues: Vec::new(),
    }
}
fn finish(report: &mut ImportReport) {
    report.imported_count = report.annotations.len() as u64;
    report.skipped_count = report.issues.len() as u64;
}
