//! Bounded real Rust packaging. Reads/downloads never dispatch work.
use super::*;
use annotagent_storage::{DeliveryPackageInput, DeliveryPackagePhase};

fn package_worker(
    state: &ServerState,
    project: String,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    id: uuid::Uuid,
) -> tokio::task::JoinHandle<()> {
    let application = state.application.clone();
    let workers = state.export_workers.clone();
    tokio::spawn(async move {
        let Ok(permit) = workers.acquire_owned().await else {
            return;
        };
        let _ = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _ = application.execute_training_package(&project, conversation, task, id);
        })
        .await;
    })
}

async fn dispatch(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    input: &DeliveryPackageInput,
    authorized: bool,
) -> ApiResult<Value> {
    let mut jobs = state.export_jobs.lock().await;
    jobs.retain(|_, job| !job.is_finished());
    let (receipt, created) = if authorized {
        state
            .application
            .admit_authorized_training_package(project, conversation, task, input)
    } else {
        state
            .application
            .admit_training_package(project, conversation, task, input)
    }
    .map_err(ApiError::conversation)?;
    if created {
        let id = input.command_id;
        let handle = package_worker(state, project.to_owned(), conversation, task, id);
        jobs.insert(id, handle);
    }
    let active = jobs
        .get(&input.command_id)
        .is_some_and(|job| !job.is_finished());
    Ok(json!({"job":receipt,"active":active,"dispatched":created}))
}

/// Event-side best effort. Admission is durable and a local worker waits for bounded
/// export capacity; the review request never blocks on file generation.
pub(super) async fn try_dispatch_automatic(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
) {
    let Ok(consents) = state
        .application
        .training_package_consents(project, conversation, task)
    else {
        return;
    };
    let Some(consent) = consents
        .into_iter()
        .find(|consent| consent.state == "armed")
    else {
        return;
    };
    let Ok(Some(input)) = state.application.automatic_training_package_input(
        project,
        conversation,
        task,
        consent.input.id,
    ) else {
        return;
    };
    let _ = dispatch(state, project, conversation, task, &input, true).await;
}

/// Startup recovery for deterministic local package work. It restores already
/// admitted nonterminal jobs from their frozen snapshots, then rechecks armed
/// consents against current whole-image receipts before admitting them.
pub(super) async fn recover_automatic(state: ServerState) {
    let Ok(routes) = state.application.list_project_route_ids() else {
        return;
    };
    let route = |owner: &str| {
        routes
            .iter()
            .find(|(id, _)| id.to_string() == owner)
            .map(|(_, route)| route.clone())
    };
    let incomplete = state
        .application
        .store()
        .incomplete_delivery_packages()
        .unwrap_or_default();
    for (owner, conversation, task, id) in incomplete {
        let Some(project) = route(&owner) else {
            continue;
        };
        let mut jobs = state.export_jobs.lock().await;
        jobs.retain(|_, job| !job.is_finished());
        jobs.entry(id)
            .or_insert_with(|| package_worker(&state, project, conversation, task, id));
    }
    let armed = state
        .application
        .store()
        .armed_delivery_package_consents()
        .unwrap_or_default();
    for (owner, conversation, task, _) in armed {
        if let Some(project) = route(&owner) {
            try_dispatch_automatic(&state, &project, conversation, task).await;
        }
    }
}

pub(super) async fn start(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<DeliveryPackageInput>,
) -> ApiResult<Json<Value>> {
    dispatch(&state, &project, conversation, task, &input, false)
        .await
        .map(Json)
}

pub(super) async fn list_consents(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    let consents = state
        .application
        .training_package_consents(&project, conversation, task)
        .map_err(ApiError::conversation)?;
    let items = consents
        .iter()
        .map(|consent| {
            state
                .application
                .training_package_consent_view(&project, conversation, task, consent)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::conversation)?;
    Ok(Json(json!({"items":items,"next_cursor":null})))
}

pub(super) async fn authorize_consent(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::DeliveryPackageConsentInput>,
) -> ApiResult<Json<Value>> {
    let consent = state
        .application
        .authorize_training_package(&project, conversation, task, &input)
        .map_err(ApiError::conversation)?;
    try_dispatch_automatic(&state, &project, conversation, task).await;
    let restored = state
        .application
        .training_package_consent(&project, conversation, task, consent.input.id)
        .map_err(ApiError::conversation)?;
    state
        .application
        .training_package_consent_view(&project, conversation, task, &restored)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn get_consent(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    let consent = state
        .application
        .training_package_consent(&project, conversation, task, id)
        .map_err(ApiError::conversation)?;
    state
        .application
        .training_package_consent_view(&project, conversation, task, &consent)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn cancel_consent(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(input): Json<CancelPackage>,
) -> ApiResult<Json<Value>> {
    if !input.confirmed {
        return Err(ApiError::conversation(anyhow::anyhow!(
            "Explicit cancellation is required"
        )));
    }
    let consent = state
        .application
        .cancel_training_package_consent(&project, conversation, task, id)
        .map_err(ApiError::conversation)?;
    state
        .application
        .training_package_consent_view(&project, conversation, task, &consent)
        .map(Json)
        .map_err(ApiError::conversation)
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
    async fn package_consent_is_passive_until_final_review_then_admits_one_restart_safe_job() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let schema = "version: 1\nproject:\n  name: TEST automatic package\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
        app.create_project("TEST-auto-package", schema).unwrap();
        let incoming = temp.path().join("TEST auto package input");
        std::fs::create_dir(&incoming).unwrap();
        image::RgbImage::from_pixel(48, 32, image::Rgb([8, 9, 10]))
            .save(incoming.join("negative.png"))
            .unwrap();
        image::RgbImage::from_pixel(48, 32, image::Rgb([11, 12, 13]))
            .save(incoming.join("negative-two.png"))
            .unwrap();
        app.import_images("TEST-auto-package", &incoming).unwrap();
        let images = app
            .list_project_image_summaries("TEST-auto-package")
            .unwrap();
        let conversation = app
            .create_project_conversation("TEST-auto-package")
            .unwrap();
        let sent = app
            .send_project_conversation_message(
                "TEST-auto-package",
                conversation,
                &serde_json::from_value(json!({
                    "message":{"id":uuid::Uuid::new_v4(),"text":"TEST negative package","image":null},
                    "task_id":null,"schema_revision":app.project_goal("TEST-auto-package").unwrap()["revision"],"mode":"plan"
                }))
                .unwrap(),
            )
            .unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let root = format!(
            "/api/projects/TEST-auto-package/conversations/{conversation}/tasks/{}",
            sent.task_id
        );
        let intent = response_json(
            request(
                &service,
                Method::POST,
                &format!("{root}/delivery-intent"),
                Some(json!({
                    "command_id":uuid::Uuid::new_v4(),"expected_revision":0,"image_ids":images.iter().map(|image|image.image_id).collect::<Vec<_>>(),
                    "label_spec":[{"stable_id":"target","display_name":"Target","aliases":[],"include":"","exclude":""}],
                    "training_target":{"annotation_kind":"bounding_box","framework":"ultralytics","export_profile":"ultralytics_yolo_detection","profile_revision":1},
                    "split_policy":{"train_percent":80,"seed":2,"preserve_existing":true,"keep_known_groups_together":true}
                })),
            )
            .await,
        )
        .await;
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
                project_name: "TEST-auto-package".into(),
                skill_id: "TEST formal result".into(),
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
        app.store()
            .register_run_image(run, images[0].image_id, "completed")
            .unwrap();
        app.store()
            .register_run_image(run, images[1].image_id, "completed")
            .unwrap();
        for image in &images {
            app.store()
                .commit_annotation(
                    run,
                    &Annotation {
                        id: AnnotationId::new(),
                        image_id: image.image_id,
                        task_id: "objects".into(),
                        label: Some("target".into()),
                        value: AnnotationValue::BoundingBox {
                            rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
                        },
                        attributes: BTreeMap::new(),
                        confidence: None,
                        source: AnnotationSource::Human,
                        review_status: ReviewStatus::HumanAccepted,
                        provenance: AnnotationProvenance::default(),
                        created_at: chrono::Utc::now(),
                    },
                )
                .await
                .unwrap();
        }
        let consent_id = uuid::Uuid::new_v4();
        let consent_body = json!({
            "id":consent_id,"intent_revision":intent["saved"]["revision"],
            "intent_sha256":intent["saved"]["content_sha256"],"confirmed":true
        });
        let consent = response_json(
            request(
                &service,
                Method::POST,
                &format!("{root}/delivery-package-consents"),
                Some(consent_body.clone()),
            )
            .await,
        )
        .await;
        assert_eq!(consent["state"], "armed");
        assert_eq!(consent["effective_state"], "blocked");
        assert_eq!(consent["readiness"]["ready"], false);
        assert!(consent["job"].is_null());
        for (index, image) in images.iter().enumerate() {
            let image_uri = format!("{root}/delivery-images/{}", image.image_id);
            let view_uri = format!("{image_uri}?source_run_id={run}");
            let view = response_json(request(&service, Method::GET, &view_uri, None).await).await;
            let input = json!({
                "command_id":uuid::Uuid::new_v4(),"intent_revision":intent["saved"]["revision"],
                "intent_sha256":intent["saved"]["content_sha256"],"image_id":image.image_id,
                "source_run_id":run,"expected_snapshot_sha256":view["snapshot"]["sha256"],
                "expected_review_revision":0,"decision":"positive_complete","reason":null,"confirmed":true
            });
            if index + 1 == images.len() {
                // Simulate a crash after the final receipt commits but before the
                // HTTP event hook can inspect the armed consent.
                app.confirm_task_delivery_image(
                    "TEST-auto-package",
                    conversation,
                    sent.task_id,
                    &serde_json::from_value(input).unwrap(),
                )
                .unwrap();
            } else {
                let reviewed = request(&service, Method::POST, &image_uri, Some(input)).await;
                assert_eq!(reviewed.status(), StatusCode::OK);
            }
        }
        assert!(
            app.training_package_status(
                "TEST-auto-package",
                conversation,
                sent.task_id,
                consent_id,
            )
            .is_err(),
            "the pre-hook crash window must not have admitted a package yet"
        );
        drop(service);
        drop(app);
        let restarted = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let restarted_state =
            test_state(restarted.clone(), Arc::new(InMemorySecretStore::default())).await;
        recover_automatic(restarted_state.clone()).await;
        let service = router(restarted_state, None);
        let package_uri = format!("{root}/delivery-packages/{consent_id}");
        let mut finished = None;
        for _ in 0..300 {
            let value =
                response_json(request(&service, Method::GET, &package_uri, None).await).await;
            if !value["active"].as_bool().unwrap() {
                finished = Some(value);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let finished = finished.expect("TEST automatic package worker must settle");
        assert_eq!(finished["job"]["phase"], "ready", "{finished}");
        assert_eq!(finished["job"]["result"]["objects"], 2);
        assert_eq!(finished["job"]["result"]["negatives"], 0);
        let restored = response_json(
            request(
                &service,
                Method::POST,
                &format!("{root}/delivery-package-consents"),
                Some(consent_body),
            )
            .await,
        )
        .await;
        assert_eq!(restored["state"], "consumed");
        assert_eq!(restored["job"]["id"], consent_id.to_string());
        let list = response_json(
            request(
                &service,
                Method::GET,
                &format!("{root}/delivery-package-consents"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(list["items"].as_array().unwrap().len(), 1);
        assert_eq!(list["items"][0]["job"]["id"], consent_id.to_string());
        drop(service);
        let history = restarted
            .training_package_consents("TEST-auto-package", conversation, sent.task_id)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].state, "consumed");
        assert_eq!(
            restarted
                .training_package_status(
                    "TEST-auto-package",
                    conversation,
                    sent.task_id,
                    consent_id,
                )
                .unwrap()
                .phase,
            DeliveryPackagePhase::Ready
        );
    }

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
        let summary = &finished["job"]["result"]["summary"];
        assert_eq!(summary["labels"].as_array().unwrap().len(), 2);
        assert!(summary["splits"]["train"].as_u64().unwrap() > 0);
        assert!(summary["splits"]["val"].as_u64().unwrap() > 0);
        assert_eq!(
            summary["splits"]["train"].as_u64().unwrap()
                + summary["splits"]["val"].as_u64().unwrap(),
            11
        );
        assert_eq!(summary["exclusions"].as_object().unwrap().len(), 1);
        assert!(!summary["warnings"].as_array().unwrap().is_empty());
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
        let manifest: annotagent_export::training_package::PackageManifest =
            serde_json::from_reader(zip.by_name("annotagent/manifest.json").unwrap()).unwrap();
        assert_eq!(manifest.format_version, 2);
        let lineage = manifest.lineage.unwrap();
        assert_eq!(lineage.package_id, command.to_string());
        assert_eq!(
            lineage.frozen_snapshot_sha256,
            finished["job"]["snapshot_sha256"]
        );
        assert_eq!(
            lineage.label_id_to_class_id,
            BTreeMap::from([("target-a".into(), 0), ("target-b".into(), 1)])
        );
        assert_eq!(lineage.images.len(), 12);
        assert_eq!(
            lineage
                .images
                .values()
                .map(|i| i.annotation_revision_ids.len())
                .sum::<usize>(),
            20
        );
        assert!(lineage.images.values().all(|i| i.source_run_id == Some(run)
            && !i.original_name.contains('/')
            && i.source_evidence_sha256.is_some()));
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
        // A separately admitted local package survives a process boundary in
        // Preparing. Startup resumes the frozen request without another POST.
        let mut recovery = input.clone();
        let recovery_id = uuid::Uuid::new_v4();
        recovery["command_id"] = json!(recovery_id);
        app.admit_training_package(
            "TEST-package",
            conversation,
            sent.task_id,
            &serde_json::from_value(recovery).unwrap(),
        )
        .unwrap();
        assert!(
            app.store()
                .incomplete_delivery_packages()
                .unwrap()
                .iter()
                .any(|(_, _, _, id)| *id == recovery_id)
        );
        let restarted = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let restarted_state = test_state(restarted, Arc::new(InMemorySecretStore::default())).await;
        recover_automatic(restarted_state.clone()).await;
        let restored = router(restarted_state, None);
        let recovery_uri = format!("{root}/delivery-packages/{recovery_id}");
        let mut recovered = None;
        for _ in 0..300 {
            let value =
                response_json(request(&restored, Method::GET, &recovery_uri, None).await).await;
            if !value["active"].as_bool().unwrap() {
                recovered = Some(value);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let recovered = recovered.expect("startup must resume admitted local package work");
        assert_eq!(recovered["job"]["phase"], "ready", "{recovered}");
        assert_eq!(recovered["job"]["result"]["images"], 11);
        assert_eq!(
            request(&restored, Method::GET, &format!("{uri}/download"), None)
                .await
                .status(),
            StatusCode::OK
        );
        let image_uri = format!("{root}/delivery-images/{}", images[0].image_id);
        let source_uri = format!("{image_uri}?source_run_id={run}");
        let before = response_json(request(&restored, Method::GET, &source_uri, None).await).await;
        let object = &before["snapshot"]["annotations"][0];
        let edit = json!({"command_id":uuid::Uuid::new_v4(),"intent_revision":before["intent_revision"],"intent_sha256":before["intent_sha256"],"source_run_id":run,"annotation_id":object["id"],"expected_snapshot_sha256":before["snapshot"]["sha256"],"label":object["label"],"value":{"kind":"bounding_box","rect":[0.12,0.12,0.16,0.16]},"review_status":"needs_review","reason":"TEST saved boundary correction"});
        let edit_uri = format!("{image_uri}/objects");
        let edited = request(&restored, Method::POST, &edit_uri, Some(edit.clone())).await;
        assert!(edited.status().is_success());
        let revision = response_json(edited).await;
        assert_eq!(
            response_json(request(&restored, Method::POST, &edit_uri, Some(edit.clone())).await)
                .await,
            revision
        );
        let mut stale = edit.clone();
        stale["command_id"] = json!(uuid::Uuid::new_v4());
        assert!(
            !request(&restored, Method::POST, &edit_uri, Some(stale))
                .await
                .status()
                .is_success()
        );
        let after = response_json(request(&restored, Method::GET, &source_uri, None).await).await;
        assert_eq!(after["confirmation_current"], false);
        assert_eq!(after["unresolved_objects"], 1);
        assert!(
            !request(
                &restored,
                Method::POST,
                &edit_uri.replacen("TEST-package", "TEST-foreign", 1),
                Some(edit)
            )
            .await
            .status()
            .is_success()
        );
        // Newly edited annotations do not mutate an already frozen package.
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
