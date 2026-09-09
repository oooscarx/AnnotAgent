//! Bounded real Rust packaging. Reads/downloads never dispatch work.
use super::*;
use annotagent_storage::{DeliveryPackageInput, DeliveryPackagePhase};

pub(super) async fn start(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<DeliveryPackageInput>,
) -> ApiResult<Json<Value>> {
    let mut jobs = state.export_jobs.lock().await;
    jobs.retain(|_, job| !job.is_finished());
    let existing = state
        .application
        .training_package_status(&project, conversation, task, input.command_id)
        .is_ok();
    let permit = if existing {
        None
    } else {
        Some(
            state
                .export_workers
                .clone()
                .try_acquire_owned()
                .map_err(|_| {
                    ApiError::bad_request(anyhow::anyhow!(
                        "Export capacity is full. No package was admitted; retry later."
                    ))
                })?,
        )
    };
    let (receipt, created) = state
        .application
        .admit_training_package(&project, conversation, task, &input)
        .map_err(ApiError::conversation)?;
    if created {
        let application = state.application.clone();
        let worker_project = project.clone();
        let id = input.command_id;
        let handle = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _ = application.execute_training_package(&worker_project, conversation, task, id);
        });
        jobs.insert(id, handle);
    }
    let active = jobs
        .get(&input.command_id)
        .is_some_and(|j| !j.is_finished());
    Ok(Json(
        json!({"job":receipt,"active":active,"dispatched":created}),
    ))
}

pub(super) async fn status(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    let jobs = state.export_jobs.lock().await;
    let active = jobs.get(&id).is_some_and(|j| !j.is_finished());
    let receipt = state
        .application
        .training_package_status(&project, conversation, task, id)
        .map_err(ApiError::conversation)?;
    let interrupted = !active
        && matches!(
            receipt.phase,
            DeliveryPackagePhase::Preparing
                | DeliveryPackagePhase::Exporting
                | DeliveryPackagePhase::Validating
        );
    Ok(Json(
        json!({"job":receipt,"active":active,"interrupted":interrupted}),
    ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CancelPackage {
    confirmed: bool,
}
pub(super) async fn cancel(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(input): Json<CancelPackage>,
) -> ApiResult<Json<annotagent_application::TrainingPackageStatus>> {
    if !input.confirmed {
        return Err(ApiError::conversation(anyhow::anyhow!(
            "Explicit cancellation is required"
        )));
    }
    state
        .application
        .cancel_training_package(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn download(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<axum::response::Response> {
    let application = state.application.clone();
    let (file, receipt) = tokio::task::spawn_blocking(move || {
        application.open_training_package(&project, conversation, task, id)
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::conversation)?;
    axum::response::Response::builder()
        .header("content-type", "application/zip")
        .header(
            "content-disposition",
            format!("attachment; filename=\"annotagent-training-{id}.zip\""),
        )
        .header("content-length", receipt.bytes.to_string())
        .header("cache-control", "private, no-store")
        .header("x-content-type-options", "nosniff")
        .body(axum::body::Body::from_stream(
            tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file)),
        ))
        .map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_core::{
        Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue,
        NormalizedRect, ReviewStatus, RunId, RunStatus,
    };
    use annotagent_provider::InMemorySecretStore;
    use annotagent_runtime::{RunRecord, RuntimeStore};
    use axum::http::Method;
    use std::{collections::BTreeMap, io::Read};

    #[tokio::test]
    async fn training_package_http_downloads_real_frozen_originals_and_rejects_foreign_or_changed_files()
     {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let schema = "version: 1\nproject:\n  name: TEST delivery\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target-a, target-b]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
        app.create_project("TEST-package", schema).unwrap();
        app.create_project("TEST-foreign", schema).unwrap();
        let incoming = temp.path().join("TEST incoming");
        std::fs::create_dir(&incoming).unwrap();
        for n in 0..12_u8 {
            image::RgbImage::from_pixel(1000, 500, image::Rgb([n, 40, 80]))
                .save(incoming.join(format!("original-{n}.png")))
                .unwrap();
        }
        std::fs::write(
            incoming.join("credentials.json"),
            b"TEST must not be packaged",
        )
        .unwrap();
        app.import_images("TEST-package", &incoming).unwrap();
        let images = app.list_project_image_summaries("TEST-package").unwrap();
        assert_eq!(images.len(), 12);
        let conversation = app.create_project_conversation("TEST-package").unwrap();
        let sent=app.send_project_conversation_message("TEST-package",conversation,&serde_json::from_value(json!({"message":{"id":uuid::Uuid::new_v4(),"text":"TEST two targets for a detection training package","image":null},"task_id":null,"schema_revision":app.project_goal("TEST-package").unwrap()["revision"],"mode":"plan"})).unwrap()).unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let root = format!(
            "/api/projects/TEST-package/conversations/{conversation}/tasks/{}",
            sent.task_id
        );
        let intent=response_json(request(&service,Method::POST,&format!("{root}/delivery-intent"),Some(json!({"command_id":uuid::Uuid::new_v4(),"expected_revision":0,"image_ids":images.iter().map(|i|i.image_id).collect::<Vec<_>>(),"label_spec":[{"stable_id":"target-a","display_name":"杯子","aliases":[],"include":"","exclude":""},{"stable_id":"target-b","display_name":"球","aliases":[],"include":"","exclude":""}],"training_target":{"annotation_kind":"bounding_box","framework":"ultralytics","export_profile":"ultralytics_yolo_detection","profile_revision":1},"split_policy":{"train_percent":80,"seed":12,"preserve_existing":true,"keep_known_groups_together":true}}))).await).await;
        let owner = intent["saved"]["intent"]["project_id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let run = RunId::new();
        app.store()
            .create_run(&RunRecord {
                id: run,
                project_id: owner,
                project_name: "TEST-package".into(),
                skill_id: "TEST manually seeded formal annotations".into(),
                provider: "TEST".into(),
                model: "TEST no inference".into(),
                status: RunStatus::Completed,
                project_schema_json: serde_json::to_string(
                    &annotagent_core::ProjectSchema::from_yaml(schema).unwrap(),
                )
                .unwrap(),
                workflow_snapshot_json: None,
            })
            .await
            .unwrap();
        let mut reviews = BTreeMap::new();
        let mut originals = BTreeMap::new();
        for (n, image) in images.iter().enumerate() {
            app.store()
                .register_run_image(run, image.image_id, "completed")
                .unwrap();
            originals.insert(
                image.image_id.to_string(),
                std::fs::read(
                    app.project_image_path("TEST-package", image.image_id)
                        .unwrap(),
                )
                .unwrap(),
            );
            if n < 10 {
                for label in ["target-a", "target-b"] {
                    let annotation = Annotation {
                        id: AnnotationId::new(),
                        image_id: image.image_id,
                        task_id: "objects".into(),
                        label: Some(label.into()),
                        value: AnnotationValue::BoundingBox {
                            rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
                        },
                        attributes: BTreeMap::new(),
                        confidence: None,
                        source: AnnotationSource::Human,
                        review_status: ReviewStatus::HumanAccepted,
                        provenance: AnnotationProvenance::default(),
                        created_at: chrono::Utc::now(),
                    };
                    app.store()
                        .commit_annotation(run, &annotation)
                        .await
                        .unwrap();
                }
            }
            let uri = format!("{root}/delivery-images/{}", image.image_id);
            let view = response_json(
                request(
                    &service,
                    Method::GET,
                    &format!("{uri}?source_run_id={run}"),
                    None,
                )
                .await,
            )
            .await;
            let response=request(&service,Method::POST,&uri,Some(json!({"command_id":uuid::Uuid::new_v4(),"intent_revision":view["intent_revision"],"intent_sha256":view["intent_sha256"],"image_id":image.image_id,"source_run_id":run,"expected_snapshot_sha256":view["snapshot"]["sha256"],"expected_review_revision":0,"decision":if n==11 {"excluded"} else if n==10 {"negative_confirmed"} else {"positive_complete"},"reason":if n==11 {Some("TEST incomplete image explicitly excluded")} else {None},"confirmed":true}))).await;
            assert_eq!(response.status(), StatusCode::OK);
            reviews.insert(image.image_id, 1);
        }
        let command = uuid::Uuid::new_v4();
        let input = json!({"command_id":command,"intent_revision":intent["saved"]["revision"],"intent_sha256":intent["saved"]["content_sha256"],"image_reviews":reviews,"confirmed":true});
        let response = request(
            &service,
            Method::POST,
            &format!("{root}/delivery-packages"),
            Some(input.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["dispatched"], true);
        let uri = format!("{root}/delivery-packages/{command}");
        let mut finished = None;
        for _ in 0..500 {
            let value = response_json(request(&service, Method::GET, &uri, None).await).await;
            if !value["active"].as_bool().unwrap() {
                finished = Some(value);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let finished = finished.expect("TEST worker must settle");
        assert_eq!(finished["job"]["phase"], "ready", "{finished}");
        assert_eq!(finished["job"]["result"]["images"], 11);
        assert_eq!(finished["job"]["result"]["objects"], 20);
        assert_eq!(finished["job"]["result"]["negatives"], 1);
        assert_eq!(finished["job"]["result"]["excluded"], 1);
        assert_eq!(
            response_json(
                request(
                    &service,
                    Method::POST,
                    &format!("{root}/delivery-packages"),
                    Some(input.clone())
                )
                .await
            )
            .await["dispatched"],
            false
        );
        let response = request(&service, Method::GET, &format!("{uri}/download"), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/zip");
        let bytes = axum::body::to_bytes(response.into_body(), 20 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(
            annotagent_image_tools::sha256(&bytes),
            finished["job"]["result"]["sha256"]
        );
        let downloaded = temp.path().join("TEST-downloaded.zip");
        std::fs::write(&downloaded, &bytes).unwrap();
        annotagent_export::training_package::validate_training_package(&downloaded).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut image_count = 0;
        for n in 0..zip.len() {
            let mut file = zip.by_index(n).unwrap();
            assert!(!file.name().contains("credentials"));
            if file.name().starts_with("images/") {
                let id = std::path::Path::new(file.name())
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                assert_eq!(&bytes, &originals[&id]);
                image_count += 1;
            }
        }
        assert_eq!(image_count, 11);
        for name in ["different-location", "含 空格的目录"] {
            let target = temp.path().join(name);
            std::fs::create_dir(&target).unwrap();
            zip.extract(&target).unwrap();
            assert!(target.join("data.yaml").is_file());
            assert!(target.join("images/train").is_dir());
            assert!(target.join("images/val").is_dir());
        }
        assert!(
            !request(
                &service,
                Method::GET,
                &format!("{uri}/download").replacen("TEST-package", "TEST-foreign", 1),
                None
            )
            .await
            .status()
            .is_success()
        );
        // Admit without dispatch to verify interrupted reads/retries never create a worker.
        let mut pending = input.clone();
        let pending_id = uuid::Uuid::new_v4();
        pending["command_id"] = json!(pending_id);
        app.admit_training_package(
            "TEST-package",
            conversation,
            sent.task_id,
            &serde_json::from_value(pending.clone()).unwrap(),
        )
        .unwrap();
        let pending_uri = format!("{root}/delivery-packages/{pending_id}");
        let stopped = response_json(request(&service, Method::GET, &pending_uri, None).await).await;
        assert_eq!(stopped["interrupted"], true);
        assert_eq!(stopped["job"]["phase"], "preparing");
        assert_eq!(
            response_json(
                request(
                    &service,
                    Method::POST,
                    &format!("{root}/delivery-packages"),
                    Some(pending)
                )
                .await
            )
            .await["dispatched"],
            false
        );
        let cancelled = response_json(
            request(
                &service,
                Method::POST,
                &format!("{pending_uri}/cancel"),
                Some(json!({"confirmed":true})),
            )
            .await,
        )
        .await;
        assert_eq!(cancelled["phase"], "cancelled");
        assert!(
            !request(
                &service,
                Method::GET,
                &format!("{pending_uri}/download"),
                None
            )
            .await
            .status()
            .is_success()
        );
        assert!(
            !temp
                .path()
                .join(format!("TEST-package/exports/deliveries/{pending_id}"))
                .exists()
        );
        let restarted = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let restored = router(
            test_state(restarted, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        assert_eq!(
            request(&restored, Method::GET, &format!("{uri}/download"), None)
                .await
                .status(),
            StatusCode::OK
        );
        std::fs::write(
            temp.path().join(format!(
                "TEST-package/exports/deliveries/{command}/dataset.zip"
            )),
            b"TEST tampered",
        )
        .unwrap();
        assert!(
            !request(&restored, Method::GET, &format!("{uri}/download"), None)
                .await
                .status()
                .is_success()
        );
        assert!(
            app.conversation_schema_calls("TEST-package", conversation, sent.task_id)
                .unwrap()
                .is_empty()
        );
    }
}
