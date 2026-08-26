//! Thin HTTP/SSE adapter over the shared application service.

use std::{
    collections::BTreeMap,
    convert::Infallible,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use annotagent_application::{
    AnnotAgentApplication, LocalApplication, Settings, stable_project_id,
};
use annotagent_core::{
    Annotation, AnnotationId, CorrectionFeatures, CorrectionRecord, DatasetExporter, ExportRequest,
    LabelId, ProjectSnapshot, ReviewStatus, RunId, SnapshotImage,
};
use annotagent_export::{
    CocoExporter, NativeExporter, YoloDetectionExporter, YoloSegmentationExporter,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, patch, post},
};
use chrono::Utc;
use futures::{Stream, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct ServerState {
    application: Arc<LocalApplication>,
    settings: Arc<RwLock<Value>>,
    temporary_api_key: Arc<RwLock<Option<String>>>,
}

impl ServerState {
    pub fn new(application: Arc<LocalApplication>) -> anyhow::Result<Self> {
        let settings = annotagent_application::load_settings(None)?;
        Ok(Self {
            application,
            settings: Arc::new(RwLock::new(serde_json::to_value(settings)?)),
            temporary_api_key: Arc::new(RwLock::new(None)),
        })
    }

    #[must_use]
    pub fn application(&self) -> &Arc<LocalApplication> {
        &self.application
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
        }
    }

    fn not_found(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: error.to_string(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error": self.message, "status": self.status.as_u16()})),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

pub fn router(state: ServerState, web_dist: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/skills", get(list_skills))
        .route("/api/skills/{skill_id}", get(get_skill))
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/{project_id}", get(get_project))
        .route("/api/projects/{project_id}/import", post(import_images))
        .route("/api/projects/{project_id}/images", get(list_images))
        .route(
            "/api/projects/{project_id}/images/{index}/content",
            get(image_content),
        )
        .route("/api/projects/{project_id}/runs", post(start_run))
        .route("/api/projects/{project_id}/export", post(export_dataset))
        .route("/api/runs/{run_id}", get(get_run))
        .route("/api/runs/{run_id}/pause", post(pause_run))
        .route("/api/runs/{run_id}/resume", post(resume_run))
        .route("/api/runs/{run_id}/cancel", post(cancel_run))
        .route("/api/runs/{run_id}/events", get(run_events))
        .route("/api/reviews", get(list_reviews))
        .route("/api/reviews/{review_id}", get(get_review))
        .route("/api/reviews/{review_id}/decision", post(review_decision))
        .route("/api/annotations/{annotation_id}", patch(patch_annotation))
        .route(
            "/api/annotations/{annotation_id}/revisions",
            get(annotation_revisions),
        )
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/events", get(events))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());
    if let Some(web_dist) = web_dist.filter(|path| path.join("index.html").is_file()) {
        api.fallback_service(
            ServeDir::new(web_dist).fallback(ServeFile::new(web_dist.join("index.html"))),
        )
    } else {
        api.fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                "RoboCup AnnotAgent Web build not found; run npm --prefix web run build",
            )
        })
    }
}

pub async fn serve(
    state: ServerState,
    address: SocketAddr,
    web_dist: Option<&Path>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, router(state, web_dist))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ignored = tokio::signal::ctrl_c().await;
}

async fn health(State(state): State<ServerState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "RoboCup AnnotAgent",
        "workspace": state.application.workspace(),
        "database": state.application.database_path(),
    }))
}

#[derive(Debug, Serialize)]
struct SkillDetail {
    id: String,
    display_name: String,
    version: String,
    description: String,
    tasks: Vec<Value>,
    tools: Vec<String>,
    validators: Vec<String>,
    refiners: Vec<String>,
    correction_taxonomy: Vec<String>,
    resources: Vec<String>,
    project_template: Option<String>,
}

fn skill_detail(skill: &dyn annotagent_core::DomainSkill) -> SkillDetail {
    let manifest = skill.manifest();
    SkillDetail {
        id: skill.id().to_owned(),
        display_name: manifest.display_name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        tasks: skill
            .task_templates()
            .into_iter()
            .map(|task| json!({"id": task.id, "description": task.description}))
            .collect(),
        tools: skill
            .tool_factories()
            .into_iter()
            .map(|tool| tool.definition().name)
            .collect(),
        validators: skill
            .validators()
            .into_iter()
            .map(|validator| validator.id().to_owned())
            .collect(),
        refiners: skill
            .refiners()
            .into_iter()
            .map(|refiner| refiner.id().to_owned())
            .collect(),
        correction_taxonomy: skill
            .correction_taxonomy()
            .into_iter()
            .map(|kind| kind.code)
            .collect(),
        resources: manifest
            .summary_resources
            .iter()
            .chain(manifest.task_resources.values().flatten())
            .cloned()
            .collect(),
        project_template: skill.project_template().map(str::to_owned),
    }
}

async fn list_skills(State(state): State<ServerState>) -> Json<Vec<SkillDetail>> {
    Json(
        state
            .application
            .skills()
            .list()
            .iter()
            .map(|skill| skill_detail(skill.as_ref()))
            .collect(),
    )
}

async fn get_skill(
    State(state): State<ServerState>,
    AxumPath(skill_id): AxumPath<String>,
) -> ApiResult<Json<SkillDetail>> {
    let skill = state
        .application
        .skills()
        .get(&skill_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(skill_detail(skill.as_ref())))
}

async fn list_projects(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    let runs = state.application.list_runs().map_err(ApiError::internal)?;
    Ok(Json(json!({"projects": projects, "runs": runs})))
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    id: String,
    yaml: String,
}

async fn create_project(
    State(state): State<ServerState>,
    Json(request): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let project = state
        .application
        .create_project(&request.id, &request.yaml)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(project))))
}

async fn get_project(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let project = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(project)))
}

#[derive(Debug, Deserialize)]
struct ImportRequest {
    source: PathBuf,
}

async fn import_images(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<ImportRequest>,
) -> ApiResult<Json<Value>> {
    let (imported, duplicates) = state
        .application
        .import_images(&project_id, &request.source)
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"imported": imported, "duplicates": duplicates}),
    ))
}

async fn list_images(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let images = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({
        "images": images.iter().enumerate().map(|(index, path)| json!({
            "index": index,
            "name": path.file_name().unwrap_or_default().to_string_lossy(),
            "url": format!("/api/projects/{project_id}/images/{index}/content"),
        })).collect::<Vec<_>>()
    })))
}

async fn image_content(
    State(state): State<ServerState>,
    AxumPath((project_id, index)): AxumPath<(String, usize)>,
) -> ApiResult<Response> {
    let path = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::not_found)?
        .get(index)
        .cloned()
        .ok_or_else(|| ApiError::not_found("image index was not found"))?;
    let content_type = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => "image/png",
        _ => "image/jpeg",
    };
    let bytes = std::fs::read(path).map_err(ApiError::internal)?;
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    Ok(response)
}

#[derive(Debug, Deserialize)]
struct StartRunRequest {
    #[serde(default = "default_provider")]
    provider: String,
}

fn default_provider() -> String {
    "mock".to_owned()
}

async fn start_run(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    payload: Option<Json<StartRunRequest>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let provider = payload.map_or_else(default_provider, |Json(value)| value.provider);
    let project_path = state
        .application
        .project_path(&project_id)
        .map_err(ApiError::not_found)?;
    let settings = serde_json::from_value::<Settings>(state.settings.read().await.clone())
        .map_err(ApiError::internal)?;
    let temporary_api_key = state.temporary_api_key.read().await.clone();
    let started = state
        .application
        .start_run_path_with_settings(&project_path, &provider, settings, temporary_api_key)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::ACCEPTED, Json(json!(started))))
}

fn parse_run_id(value: &str) -> ApiResult<RunId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn get_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let run = state
        .application
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| ApiError::not_found("run was not found"))?;
    let events = state
        .application
        .list_events(run_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"run": run, "event_count": events.len()})))
}

async fn pause_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .pause_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "paused"})))
}

async fn resume_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .resume_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "running"})))
}

async fn cancel_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .cancel_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "cancelled"})))
}

async fn run_events(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let events = state
        .application
        .list_events(run_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"events": events})))
}

fn reviews(state: &ServerState) -> ApiResult<Vec<Value>> {
    let mut reviews = Vec::new();
    for run in state.application.list_runs().map_err(ApiError::internal)? {
        for annotation in state
            .application
            .store()
            .list_annotations(run.id)
            .map_err(ApiError::internal)?
        {
            if annotation.review_status == ReviewStatus::NeedsReview {
                reviews
                    .push(json!({"id": annotation.id, "run_id": run.id, "annotation": annotation}));
            }
        }
    }
    Ok(reviews)
}

async fn list_reviews(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"reviews": reviews(&state)?})))
}

fn parse_annotation_id(value: &str) -> ApiResult<AnnotationId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn get_review(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    let (run_id, annotation) = state
        .application
        .store()
        .find_annotation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    Ok(Json(
        json!({"id": id, "run_id": run_id, "annotation": annotation}),
    ))
}

#[derive(Debug, Deserialize)]
struct AnnotationPatch {
    annotation: Annotation,
    reason: Option<String>,
}

async fn patch_annotation(
    State(state): State<ServerState>,
    AxumPath(annotation_id): AxumPath<String>,
    Json(request): Json<AnnotationPatch>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&annotation_id)?;
    if request.annotation.id != id {
        return Err(ApiError::bad_request(
            "path and annotation ids do not match",
        ));
    }
    let revision = state
        .application
        .store()
        .update_annotation(&request.annotation, request.reason.as_deref())
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"annotation": request.annotation, "revision": revision}),
    ))
}

#[derive(Debug, Deserialize)]
struct ReviewDecisionRequest {
    decision: String,
    project_id: String,
    reason_code: String,
    note: Option<String>,
    corrected_label: Option<LabelId>,
}

async fn review_decision(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
    Json(request): Json<ReviewDecisionRequest>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    let (_, mut annotation) = state
        .application
        .store()
        .find_annotation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    let original = annotation.snapshot();
    annotation.review_status = match request.decision.as_str() {
        "accept" => ReviewStatus::HumanAccepted,
        "reject" | "delete" => ReviewStatus::Rejected,
        other => return Err(ApiError::bad_request(format!("unknown decision {other:?}"))),
    };
    if let Some(label) = request.corrected_label.clone() {
        annotation.label = Some(label);
    }
    let revision = state
        .application
        .store()
        .update_annotation(&annotation, Some(&request.reason_code))
        .map_err(ApiError::bad_request)?;
    let project_path = state
        .application
        .project_path(&request.project_id)
        .map_err(ApiError::bad_request)?;
    let (project, _) =
        annotagent_application::load_project(&project_path).map_err(ApiError::bad_request)?;
    let record = CorrectionRecord {
        id: uuid::Uuid::new_v4(),
        project_id: stable_project_id(
            project_path
                .parent()
                .unwrap_or(state.application.workspace()),
        ),
        skill_id: project.project.skill,
        task_id: annotation.task_id.clone(),
        predicted_label: original.label.clone(),
        corrected_label: annotation.label.clone(),
        reason_code: request.reason_code,
        original_annotation: Some(original),
        corrected_annotation: Some(annotation.snapshot()),
        note: request.note,
        image_features: CorrectionFeatures {
            geometry: BTreeMap::new(),
            colors: BTreeMap::new(),
        },
        created_at: Utc::now(),
    };
    state
        .application
        .store()
        .save_correction(&record)
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"annotation": annotation, "revision": revision, "correction_id": record.id}),
    ))
}

async fn annotation_revisions(
    State(state): State<ServerState>,
    AxumPath(annotation_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&annotation_id)?;
    let revisions = state
        .application
        .store()
        .list_revisions(id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"revisions": revisions})))
}

#[derive(Debug, Deserialize)]
struct ExportBody {
    #[serde(default = "default_export_format")]
    format: String,
}

fn default_export_format() -> String {
    "native".to_owned()
}

async fn export_dataset(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<ExportBody>,
) -> ApiResult<Json<Value>> {
    let project_path = state
        .application
        .project_path(&project_id)
        .map_err(ApiError::not_found)?;
    let (schema, _) =
        annotagent_application::load_project(&project_path).map_err(ApiError::bad_request)?;
    let run = state
        .application
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|run| run.project_name == schema.project.name)
        .ok_or_else(|| ApiError::bad_request("project has no completed run"))?;
    let annotations = state
        .application
        .store()
        .list_annotations(run.id)
        .map_err(ApiError::internal)?;
    let image_path = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::bad_request)?
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::bad_request("project has no image"))?;
    let frame = annotagent_image_tools::load_image(&image_path, 40_000_000)
        .map_err(ApiError::bad_request)?;
    let image_id = annotations
        .first()
        .map(|annotation| annotation.image_id)
        .ok_or_else(|| ApiError::bad_request("run has no annotations"))?;
    let root = project_path
        .parent()
        .unwrap_or(state.application.workspace());
    let snapshot = ProjectSnapshot {
        schema,
        images: vec![SnapshotImage {
            id: image_id,
            relative_path: image_path
                .strip_prefix(root)
                .unwrap_or(&image_path)
                .to_path_buf(),
            metadata: frame.metadata,
        }],
        annotations,
    };
    let output = root.join("exports").join(&request.format);
    let exporter: Box<dyn DatasetExporter> = match request.format.as_str() {
        "native" => Box::new(NativeExporter),
        "coco" => Box::new(CocoExporter),
        "yolo" | "yolo_detection" => Box::new(YoloDetectionExporter),
        "yolo_segmentation" => Box::new(YoloSegmentationExporter),
        other => {
            return Err(ApiError::bad_request(format!(
                "unknown export format {other:?}"
            )));
        }
    };
    let report = exporter
        .export(ExportRequest {
            project: snapshot,
            output,
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!(report)))
}

async fn get_settings(State(state): State<ServerState>) -> Json<Value> {
    let mut settings = state.settings.read().await.clone();
    if let Some(object) = settings.as_object_mut() {
        object.insert(
            "temporary_api_key_configured".to_owned(),
            Value::Bool(state.temporary_api_key.read().await.is_some()),
        );
    }
    Json(settings)
}

async fn put_settings(
    State(state): State<ServerState>,
    Json(mut settings): Json<Value>,
) -> ApiResult<Json<Value>> {
    let temporary_key = settings
        .as_object_mut()
        .and_then(|object| object.remove("temporary_api_key"))
        .and_then(|value| value.as_str().map(str::to_owned));
    serde_json::from_value::<Settings>(settings.clone()).map_err(ApiError::bad_request)?;
    *state.settings.write().await = settings;
    if temporary_key.is_some() {
        *state.temporary_api_key.write().await = temporary_key;
    }
    Ok(get_settings(State(state)).await)
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    run_id: Option<RunId>,
}

async fn events(
    State(state): State<ServerState>,
    Query(query): Query<EventQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.application.subscribe();
    let stream = stream::unfold(
        (receiver, query.run_id),
        |(mut receiver, run_id)| async move {
            loop {
                match receiver.recv().await {
                    Ok(value) if run_id.is_none_or(|filter| filter == value.run_id) => {
                        let event = Event::default()
                            .event(serde_json::to_value(value.kind).ok()?.as_str()?)
                            .json_data(&value)
                            .ok()?;
                        return Some((Ok(event), (receiver, run_id)));
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

#[cfg(test)]
mod tests {
    use annotagent_core::RunStatus;
    use axum::body::to_bytes;
    use futures::StreamExt;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_works_and_traversal_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(ServerState::new(app).expect("state"), None);
        let response = service
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        assert!(String::from_utf8_lossy(&body).contains("RoboCup AnnotAgent"));
        let response = service
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/..%2Fsecret")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ));
    }

    #[tokio::test]
    async fn project_sse_review_revision_and_budget_flow_works_over_http() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let state = ServerState::new(application.clone()).expect("state");
        let service = router(state.clone(), None);
        let skill = application
            .skills()
            .get("robocup")
            .expect("registered test skill");
        let project_yaml = skill
            .project_template()
            .expect("project template")
            .replace("max_retries_per_task: 3", "max_retries_per_task: 0");
        let response = request(
            &service,
            axum::http::Method::POST,
            "/api/projects",
            Some(json!({"id": "review-demo", "yaml": project_yaml})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);

        let sse = request(&service, axum::http::Method::GET, "/api/events", None).await;
        assert_eq!(sse.status(), StatusCode::OK);
        let mut event_stream = sse.into_body().into_data_stream();

        let started = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/runs",
                Some(json!({"provider": "mock"})),
            )
            .await,
        )
        .await;
        let run_id = started["run_id"].as_str().expect("run id");
        let first_event = tokio::time::timeout(Duration::from_secs(2), event_stream.next())
            .await
            .expect("SSE timeout")
            .expect("SSE item")
            .expect("SSE body");
        assert!(String::from_utf8_lossy(&first_event).contains("run_"));

        wait_for_status(&application, run_id, RunStatus::AwaitingReview).await;
        let run_response = request(
            &service,
            axum::http::Method::GET,
            &format!("/api/runs/{run_id}"),
            None,
        )
        .await;
        assert_eq!(run_response.status(), StatusCode::OK);
        let events = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/events"),
                None,
            )
            .await,
        )
        .await;
        assert!(
            events["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );

        let reviews =
            response_json(request(&service, axum::http::Method::GET, "/api/reviews", None).await)
                .await;
        let review_id = reviews["reviews"][0]["id"].as_str().expect("review id");
        let reason_code = skill
            .correction_taxonomy()
            .into_iter()
            .next()
            .expect("correction taxonomy")
            .code;
        let decision = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{review_id}/decision"),
            Some(json!({
                "project_id": "review-demo",
                "decision": "reject",
                "reason_code": reason_code,
                "note": "deterministic server test"
            })),
        )
        .await;
        assert_eq!(decision.status(), StatusCode::OK);
        let revisions = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/annotations/{review_id}/revisions"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(revisions["revisions"].as_array().map(Vec::len), Some(1));

        let mut settings =
            response_json(request(&service, axum::http::Method::GET, "/api/settings", None).await)
                .await;
        settings["budget"]["max_requests"] = json!(0);
        let settings_response = request(
            &service,
            axum::http::Method::PUT,
            "/api/settings",
            Some(settings),
        )
        .await;
        assert_eq!(settings_response.status(), StatusCode::OK);
        let budget_run = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/runs",
                Some(json!({"provider": "mock"})),
            )
            .await,
        )
        .await;
        wait_for_status(
            &application,
            budget_run["run_id"].as_str().expect("budget run id"),
            RunStatus::BudgetExceeded,
        )
        .await;
    }

    async fn request(
        service: &Router,
        method: axum::http::Method,
        uri: &str,
        body: Option<Value>,
    ) -> Response {
        let request = axum::http::Request::builder().method(method).uri(uri);
        let request = if let Some(value) = body {
            request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
        } else {
            request.body(Body::empty())
        }
        .expect("request");
        service.clone().oneshot(request).await.expect("response")
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .expect("response body");
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    async fn wait_for_status(application: &LocalApplication, run_id: &str, expected: RunStatus) {
        let run_id: RunId = run_id.parse().expect("valid run id");
        for _ in 0..100 {
            if application
                .list_runs()
                .expect("runs")
                .into_iter()
                .any(|run| run.id == run_id && run.status == expected)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("run {run_id} did not reach {expected:?}");
    }
}
