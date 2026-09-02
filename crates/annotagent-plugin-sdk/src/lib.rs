//! Server-side Rust SDK for isolated model plugin processes.

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use annotagent_core::{
    ModelCapability, ModelImage, PipelineInferenceRequest, PipelineInferenceResponse,
    VisionBackendError, VisionCapability,
};
use annotagent_plugin_api::{
    CancelRequest, CancelResponse, ModelRuntimeDescriptor, PLUGIN_API_VERSION,
    PLUGIN_PROTOCOL_VERSION, PluginContracts, PluginErrorBody, PluginHealth, PluginManifest,
    PluginReadyHandshake, PluginRuntimeDescriptor, PluginTestCheck, PluginTestReport,
    ShutdownRequest, ShutdownResponse, WarmupRequest, WarmupResponse,
};
use async_trait::async_trait;
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures::FutureExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

const MAX_TOKEN_BYTES: usize = 512;
const MAX_NONCE_BYTES: usize = 256;
const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PluginSdkError {
    #[error("invalid startup configuration: {0}")]
    InvalidStartup(String),
    #[error("plugin server io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("startup configuration could not be decoded: {0}")]
    StartupDecode(#[from] serde_json::Error),
    #[error("image input is invalid: {0}")]
    InvalidImage(String),
    #[error("plugin implementation failed: {0}")]
    Plugin(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStartupConfig {
    pub session_token: String,
    pub session_nonce: String,
    pub state_dir: PathBuf,
    pub weights_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temporary_dir: PathBuf,
    #[serde(default)]
    pub listen_port: u16,
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: usize,
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: usize,
}

const fn default_max_request_bytes() -> usize {
    DEFAULT_MAX_REQUEST_BYTES
}

const fn default_max_response_bytes() -> usize {
    DEFAULT_MAX_RESPONSE_BYTES
}

impl PluginStartupConfig {
    pub fn validate(&self) -> Result<(), PluginSdkError> {
        if self.session_token.len() < 32
            || self.session_token.len() > MAX_TOKEN_BYTES
            || self.session_token.contains(['\r', '\n'])
        {
            return Err(PluginSdkError::InvalidStartup(
                "session token must be bounded and non-empty".to_owned(),
            ));
        }
        if self.session_nonce.is_empty()
            || self.session_nonce.len() > MAX_NONCE_BYTES
            || self.session_nonce.contains(['\r', '\n'])
        {
            return Err(PluginSdkError::InvalidStartup(
                "session nonce must be bounded and single-line".to_owned(),
            ));
        }
        for (name, path) in [
            ("state", &self.state_dir),
            ("weights", &self.weights_dir),
            ("cache", &self.cache_dir),
            ("temporary", &self.temporary_dir),
        ] {
            if !path.is_absolute() || path.components().any(|part| part.as_os_str() == "..") {
                return Err(PluginSdkError::InvalidStartup(format!(
                    "{name} directory must be an absolute normalized path"
                )));
            }
        }
        if self.max_request_bytes == 0 || self.max_response_bytes == 0 {
            return Err(PluginSdkError::InvalidStartup(
                "request and response limits must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WarmupContext {
    pub weights_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone)]
pub struct InferenceContext {
    pub request_id: String,
    pub weights_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temporary_dir: PathBuf,
    pub cancellation: CancellationToken,
}

/// Least-privilege process directories made available once before the server reports ready.
///
/// Session credentials and host application paths are intentionally excluded.
#[derive(Debug, Clone)]
pub struct PluginRuntimeContext {
    pub state_dir: PathBuf,
    pub weights_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temporary_dir: PathBuf,
}

#[async_trait]
pub trait ExpertModelPlugin: Send + Sync + 'static {
    async fn setup(&self, _context: PluginRuntimeContext) -> Result<(), PluginSdkError> {
        Ok(())
    }

    fn descriptor(&self) -> PluginRuntimeDescriptor;

    fn models(&self) -> Vec<ModelRuntimeDescriptor>;

    async fn warmup(&self, model_id: &str, context: WarmupContext) -> Result<(), PluginSdkError>;

    async fn infer(
        &self,
        request: PipelineInferenceRequest,
        context: InferenceContext,
    ) -> Result<PipelineInferenceResponse, PluginSdkError>;

    async fn cancel(&self, request_id: &str) -> Result<(), PluginSdkError>;
}

#[derive(Clone)]
struct ServerState {
    plugin: Arc<dyn ExpertModelPlugin>,
    config: PluginStartupConfig,
    started_at: Instant,
    requests: Arc<RwLock<BTreeMap<String, CancellationToken>>>,
    shutdown: CancellationToken,
    infer_lock: Arc<Mutex<()>>,
}

pub struct RunningPluginServer {
    pub address: SocketAddr,
    shutdown: CancellationToken,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl RunningPluginServer {
    pub async fn shutdown(self) -> Result<(), PluginSdkError> {
        self.shutdown.cancel();
        self.task
            .await
            .map_err(|error| PluginSdkError::Plugin(format!("server task failed: {error}")))??;
        Ok(())
    }
}

pub struct PluginServer;

impl PluginServer {
    pub async fn spawn(
        plugin: Arc<dyn ExpertModelPlugin>,
        config: PluginStartupConfig,
    ) -> Result<RunningPluginServer, PluginSdkError> {
        config.validate()?;
        create_private_directories(&config).await?;
        plugin
            .setup(PluginRuntimeContext {
                state_dir: config.state_dir.clone(),
                weights_dir: config.weights_dir.clone(),
                cache_dir: config.cache_dir.clone(),
                temporary_dir: config.temporary_dir.clone(),
            })
            .await?;
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), config.listen_port);
        let listener = TcpListener::bind(address).await?;
        let address = listener.local_addr()?;
        if !address.ip().is_loopback() {
            return Err(PluginSdkError::InvalidStartup(
                "plugin listener must be loopback-only".to_owned(),
            ));
        }
        let shutdown = CancellationToken::new();
        let state = ServerState {
            plugin,
            config,
            started_at: Instant::now(),
            requests: Arc::new(RwLock::new(BTreeMap::new())),
            shutdown: shutdown.clone(),
            infer_lock: Arc::new(Mutex::new(())),
        };
        let router = router(state.clone());
        let graceful = shutdown.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(graceful.cancelled_owned())
                .await
        });
        Ok(RunningPluginServer {
            address,
            shutdown,
            task,
        })
    }

    pub async fn run_from_stdin(plugin: Arc<dyn ExpertModelPlugin>) -> Result<(), PluginSdkError> {
        let mut input = Vec::new();
        tokio::io::stdin()
            .take(1024 * 1024)
            .read_to_end(&mut input)
            .await?;
        let config: PluginStartupConfig = serde_json::from_slice(&input)?;
        let nonce = config.session_nonce.clone();
        let descriptor = plugin.descriptor();
        let server = Self::spawn(plugin, config).await?;
        let handshake = PluginReadyHandshake {
            status: "ready".to_owned(),
            plugin_api: PLUGIN_API_VERSION.to_owned(),
            protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
            listen: server.address.to_string(),
            plugin_id: descriptor.plugin_id,
            session_nonce: nonce,
        };
        let mut stdout = tokio::io::stdout();
        stdout
            .write_all(format!("{}\n", serde_json::to_string(&handshake)?).as_bytes())
            .await?;
        stdout.flush().await?;
        server
            .task
            .await
            .map_err(|error| PluginSdkError::Plugin(format!("server task failed: {error}")))??;
        Ok(())
    }
}

async fn create_private_directories(config: &PluginStartupConfig) -> Result<(), std::io::Error> {
    for path in [&config.state_dir, &config.cache_dir, &config.temporary_dir] {
        tokio::fs::create_dir_all(path).await?;
    }
    Ok(())
}

fn router(state: ServerState) -> Router {
    let request_limit = state.config.max_request_bytes;
    Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/models", get(models))
        .route("/v1/contracts", get(contracts))
        .route("/v1/infer", post(infer))
        .route("/v1/cancel", post(cancel))
        .route("/v1/warmup", post(warmup))
        .route("/v1/shutdown", post(shutdown))
        .layer(DefaultBodyLimit::max(request_limit))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state)
}

async fn authorize(
    State(state): State<ServerState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let supplied = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if supplied.is_some_and(|value| constant_time_equal(value, &state.config.session_token)) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(PluginErrorBody {
                code: "unauthorized".to_owned(),
                message: "valid plugin session authorization is required".to_owned(),
                retryable: false,
            }),
        )
            .into_response()
    }
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    let left = Sha256::digest(left.as_bytes());
    let right = Sha256::digest(right.as_bytes());
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

async fn health(State(state): State<ServerState>) -> Json<PluginHealth> {
    let descriptor = state.plugin.descriptor();
    Json(PluginHealth {
        status: "ready".to_owned(),
        plugin_id: descriptor.plugin_id,
        plugin_version: descriptor.plugin_version,
        protocol_version: descriptor.protocol_version,
        loaded_models: state
            .plugin
            .models()
            .into_iter()
            .filter(|model| model.loaded)
            .map(|model| model.model.id)
            .collect(),
        device: state
            .plugin
            .models()
            .first()
            .map_or_else(|| "unknown".to_owned(), |model| model.device.clone()),
        uptime_ms: state.started_at.elapsed().as_millis() as u64,
    })
}

async fn capabilities(State(state): State<ServerState>) -> Json<PluginRuntimeDescriptor> {
    Json(state.plugin.descriptor())
}

async fn models(State(state): State<ServerState>) -> Json<Vec<ModelRuntimeDescriptor>> {
    Json(state.plugin.models())
}

async fn contracts(State(state): State<ServerState>) -> Json<PluginContracts> {
    let descriptor = state.plugin.descriptor();
    Json(PluginContracts {
        plugin_id: descriptor.plugin_id,
        plugin_version: descriptor.plugin_version,
        models: state
            .plugin
            .models()
            .into_iter()
            .map(|model| model.model)
            .collect(),
    })
}

async fn warmup(
    State(state): State<ServerState>,
    Json(request): Json<WarmupRequest>,
) -> Json<WarmupResponse> {
    let started = Instant::now();
    let cancellation = CancellationToken::new();
    let result = AssertUnwindSafe(state.plugin.warmup(
        &request.model_id,
        WarmupContext {
            weights_dir: state.config.weights_dir.clone(),
            cache_dir: state.config.cache_dir.clone(),
            cancellation,
        },
    ))
    .catch_unwind()
    .await;
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error_body("warmup_failed", &error)),
        Err(_) => Some(PluginErrorBody {
            code: "plugin_panic".to_owned(),
            message: "plugin warmup panicked".to_owned(),
            retryable: false,
        }),
    };
    Json(WarmupResponse {
        request_id: request.request_id,
        model_id: request.model_id,
        ready: error.is_none(),
        duration_ms: started.elapsed().as_millis() as u64,
        error,
    })
}

async fn infer(
    State(state): State<ServerState>,
    Json(request): Json<PipelineInferenceRequest>,
) -> Response {
    if let Err(error) = validate_inference_request(&state, &request) {
        return (StatusCode::BAD_REQUEST, Json(error)).into_response();
    }
    let cancellation = CancellationToken::new();
    state
        .requests
        .write()
        .await
        .insert(request.request_id.clone(), cancellation.clone());
    let request_id = request.request_id.clone();
    let _guard = state.infer_lock.lock().await;
    let result = AssertUnwindSafe(state.plugin.infer(
        request,
        InferenceContext {
            request_id: request_id.clone(),
            weights_dir: state.config.weights_dir.clone(),
            cache_dir: state.config.cache_dir.clone(),
            temporary_dir: state.config.temporary_dir.clone(),
            cancellation,
        },
    ))
    .catch_unwind()
    .await;
    state.requests.write().await.remove(&request_id);
    let response = match result {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => failure_response(&request_id, "inference_failed", &error),
        Err(_) => failure_response_message(
            &request_id,
            "plugin_panic",
            "plugin inference panicked",
            false,
        ),
    };
    match serde_json::to_vec(&response) {
        Ok(bytes) if bytes.len() <= state.config.max_response_bytes => {
            Json(response).into_response()
        }
        Ok(_) => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(PluginErrorBody {
                code: "response_too_large".to_owned(),
                message: "plugin response exceeded the configured limit".to_owned(),
                retryable: false,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PluginErrorBody {
                code: "invalid_response".to_owned(),
                message: "plugin response could not be serialized".to_owned(),
                retryable: false,
            }),
        )
            .into_response(),
    }
}

fn validate_inference_request(
    state: &ServerState,
    request: &PipelineInferenceRequest,
) -> Result<(), PluginErrorBody> {
    if request.protocol_version != annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION
        || request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.node_id.is_empty()
    {
        return Err(PluginErrorBody {
            code: "invalid_request".to_owned(),
            message: "request protocol and identity fields are invalid".to_owned(),
            retryable: false,
        });
    }
    let model = state
        .plugin
        .models()
        .into_iter()
        .find(|model| model.model.id == request.model_id && model.loaded)
        .ok_or_else(|| PluginErrorBody {
            code: "model_unavailable".to_owned(),
            message: "requested model is not loaded".to_owned(),
            retryable: false,
        })?;
    if operation_capability(request.operation)
        .is_none_or(|capability| !model.model.capabilities.contains(&capability))
    {
        return Err(PluginErrorBody {
            code: "capability_mismatch".to_owned(),
            message: "requested operation is not declared by the model".to_owned(),
            retryable: false,
        });
    }
    for artifact in &request.input_artifacts {
        artifact.validate().map_err(|message| PluginErrorBody {
            code: "invalid_artifact".to_owned(),
            message,
            retryable: false,
        })?;
    }
    Ok(())
}

const fn operation_capability(operation: VisionCapability) -> Option<ModelCapability> {
    match operation {
        VisionCapability::VisionLanguage => Some(ModelCapability::VisionLanguage),
        VisionCapability::OpenVocabularyDetection => Some(ModelCapability::OpenVocabularyDetection),
        VisionCapability::PhraseGrounding => Some(ModelCapability::PhraseGrounding),
        VisionCapability::ObjectDetection => Some(ModelCapability::ObjectDetection),
        VisionCapability::SemanticSegmentation => Some(ModelCapability::SemanticSegmentation),
        VisionCapability::InstanceSegmentation => Some(ModelCapability::InstanceSegmentation),
        VisionCapability::PromptedSegmentation => Some(ModelCapability::PromptedSegmentation),
        VisionCapability::Classification => Some(ModelCapability::ImageClassification),
        VisionCapability::KeypointDetection => Some(ModelCapability::KeypointDetection),
        VisionCapability::Embedding => None,
    }
}

async fn cancel(
    State(state): State<ServerState>,
    Json(request): Json<CancelRequest>,
) -> Json<CancelResponse> {
    let cancelled = if let Some(token) = state.requests.read().await.get(&request.request_id) {
        token.cancel();
        true
    } else {
        false
    };
    let _ = state.plugin.cancel(&request.request_id).await;
    Json(CancelResponse {
        request_id: request.request_id,
        cancelled,
    })
}

async fn shutdown(
    State(state): State<ServerState>,
    Json(request): Json<ShutdownRequest>,
) -> Json<ShutdownResponse> {
    let accepted = !request.reason.trim().is_empty();
    if accepted {
        state.shutdown.cancel();
    }
    Json(ShutdownResponse { accepted })
}

fn error_body(code: &str, error: &PluginSdkError) -> PluginErrorBody {
    PluginErrorBody {
        code: code.to_owned(),
        message: error.to_string(),
        retryable: false,
    }
}

fn failure_response(
    request_id: &str,
    code: &str,
    error: &PluginSdkError,
) -> PipelineInferenceResponse {
    failure_response_message(request_id, code, &error.to_string(), false)
}

fn failure_response_message(
    request_id: &str,
    code: &str,
    message: &str,
    retryable: bool,
) -> PipelineInferenceResponse {
    PipelineInferenceResponse {
        request_id: Some(request_id.to_owned()),
        error: Some(VisionBackendError {
            code: code.to_owned(),
            message: message.to_owned(),
            retryable,
        }),
        ..PipelineInferenceResponse::default()
    }
}

pub fn decode_image(
    image: &ModelImage,
    maximum_bytes: usize,
) -> Result<image::DynamicImage, PluginSdkError> {
    if image.mime_type != "image/png" && image.mime_type != "image/jpeg" {
        return Err(PluginSdkError::InvalidImage(
            "only PNG and JPEG input is supported".to_owned(),
        ));
    }
    let bytes = STANDARD
        .decode(&image.data_base64)
        .map_err(|_| PluginSdkError::InvalidImage("input is not valid base64".to_owned()))?;
    if bytes.len() > maximum_bytes {
        return Err(PluginSdkError::InvalidImage(
            "decoded image exceeds configured limit".to_owned(),
        ));
    }
    image::load_from_memory(&bytes)
        .map_err(|_| PluginSdkError::InvalidImage("input is not a valid image".to_owned()))
}

#[must_use]
pub fn is_path_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root) && path != root.parent().unwrap_or(root)
}

/// Runs the protocol and declaration checks shared by installation, development and CI.
pub async fn run_conformance(
    base_url: &str,
    session_token: &str,
    manifest: &PluginManifest,
    sample_request: Option<&PipelineInferenceRequest>,
) -> PluginTestReport {
    let started_at = chrono::Utc::now();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(
            manifest.runtime.startup_timeout_seconds,
        ))
        .build()
        .expect("bounded conformance client configuration");
    let mut checks = Vec::new();

    let unauthenticated = client.get(format!("{base_url}/health")).send().await;
    checks.push(PluginTestCheck {
        name: "session authentication".to_owned(),
        passed: unauthenticated
            .as_ref()
            .is_ok_and(|response| response.status() == StatusCode::UNAUTHORIZED),
        detail: "requests without the current session token are rejected".to_owned(),
    });

    let health = authenticated(&client, session_token, format!("{base_url}/health"))
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let health = match health {
        Some(response) => response.json::<PluginHealth>().await.ok(),
        None => None,
    };
    checks.push(PluginTestCheck {
        name: "health".to_owned(),
        passed: health.as_ref().is_some_and(|health| {
            health.plugin_id == manifest.id
                && health.plugin_version == manifest.version
                && health.protocol_version == PLUGIN_PROTOCOL_VERSION
                && health.status == "ready"
        }),
        detail: "runtime identity and health match the package declaration".to_owned(),
    });

    let runtime = authenticated(
        &client,
        session_token,
        format!("{base_url}/v1/capabilities"),
    )
    .send()
    .await
    .ok()
    .and_then(|response| response.error_for_status().ok());
    let runtime = match runtime {
        Some(response) => response.json::<PluginRuntimeDescriptor>().await.ok(),
        None => None,
    };
    let declared_capabilities = manifest
        .models
        .iter()
        .flat_map(|model| model.capabilities.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    checks.push(PluginTestCheck {
        name: "capability declaration".to_owned(),
        passed: runtime.as_ref().is_some_and(|runtime| {
            runtime.plugin_id == manifest.id
                && runtime.plugin_version == manifest.version
                && runtime.plugin_api == manifest.plugin_api
                && runtime
                    .capabilities
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    == declared_capabilities
        }),
        detail: "runtime capabilities equal the manifest capabilities".to_owned(),
    });

    let models = authenticated(&client, session_token, format!("{base_url}/v1/models"))
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let models = match models {
        Some(response) => response.json::<Vec<ModelRuntimeDescriptor>>().await.ok(),
        None => None,
    };
    checks.push(PluginTestCheck {
        name: "model discovery".to_owned(),
        passed: models.as_ref().is_some_and(|runtime_models| {
            runtime_models.len() == manifest.models.len()
                && manifest.models.iter().all(|declared| {
                    runtime_models
                        .iter()
                        .any(|runtime| runtime.model == *declared)
                })
        }),
        detail: "runtime model descriptors equal the manifest models".to_owned(),
    });

    let contracts = authenticated(&client, session_token, format!("{base_url}/v1/contracts"))
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let contracts = match contracts {
        Some(response) => response.json::<PluginContracts>().await.ok(),
        None => None,
    };
    checks.push(PluginTestCheck {
        name: "contract discovery".to_owned(),
        passed: contracts.as_ref().is_some_and(|contracts| {
            contracts.plugin_id == manifest.id
                && contracts.plugin_version == manifest.version
                && contracts.models == manifest.models
        }),
        detail: "runtime contracts equal the package contracts".to_owned(),
    });

    if let Some(request) = sample_request {
        let response = client
            .post(format!("{base_url}/v1/infer"))
            .bearer_auth(session_token)
            .json(request)
            .send()
            .await
            .ok()
            .and_then(|response| response.error_for_status().ok());
        let response = match response {
            Some(response) => response.json::<PipelineInferenceResponse>().await.ok(),
            None => None,
        };
        checks.push(PluginTestCheck {
            name: "sample inference".to_owned(),
            passed: response.as_ref().is_some_and(|response| {
                response.request_id.as_deref() == Some(request.request_id.as_str())
                    && response.error.is_none()
                    && !response.artifacts.is_empty()
                    && response
                        .artifacts
                        .iter()
                        .all(|artifact| artifact.validate().is_ok())
            }),
            detail: "sample inference returns scoped, typed and valid artifacts".to_owned(),
        });
    }

    let invalid = PipelineInferenceRequest {
        protocol_version: u32::MAX,
        request_id: "conformance-invalid".to_owned(),
        run_id: annotagent_core::RunId::new(),
        image_id: annotagent_core::ImageId::new(),
        node_id: "conformance".to_owned(),
        model_id: manifest.models[0].id.clone(),
        operation: VisionCapability::ObjectDetection,
        image: None,
        input_artifacts: Vec::new(),
        parameters: BTreeMap::new(),
        timeout_ms: Some(1_000),
    };
    let invalid_status = client
        .post(format!("{base_url}/v1/infer"))
        .bearer_auth(session_token)
        .json(&invalid)
        .send()
        .await
        .ok()
        .map(|response| response.status());
    checks.push(PluginTestCheck {
        name: "invalid request".to_owned(),
        passed: invalid_status == Some(StatusCode::BAD_REQUEST),
        detail: "invalid protocol requests fail closed".to_owned(),
    });

    let passed = checks.iter().all(|check| check.passed);
    PluginTestReport {
        plugin_id: manifest.id.clone(),
        plugin_version: manifest.version.clone(),
        passed,
        checks,
        started_at,
        finished_at: chrono::Utc::now(),
    }
}

fn authenticated(client: &reqwest::Client, token: &str, url: String) -> reqwest::RequestBuilder {
    client.get(url).bearer_auth(token)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{
        ArtifactContract, ArtifactKind, ArtifactRef, ArtifactValidationState,
        DetectionArtifactItem, DetectionScore, DetectionSetArtifact, DetectionSource,
        GeometrySemantics, ImageId, ModelCapability, NormalizedRect, PipelineArtifact, RunId,
        RuntimeRequirements, ScoreSemantics, VisionCapability,
    };
    use annotagent_plugin_api::{
        PLUGIN_MANIFEST_SCHEMA_VERSION, PluginId, PluginModelManifest, PluginVersion, Sha256Digest,
    };
    use tempfile::TempDir;

    use super::*;

    #[derive(Default)]
    struct TestPlugin;

    impl TestPlugin {
        fn model() -> PluginModelManifest {
            PluginModelManifest {
                id: "dummy-detector-v1".to_owned(),
                display_name: "Dummy Detector v1".to_owned(),
                capabilities: vec![ModelCapability::ObjectDetection],
                input_contracts: vec![ArtifactContract::artifact(
                    "image",
                    ArtifactKind::Image,
                    true,
                    false,
                )],
                output_contracts: vec![ArtifactContract::artifact(
                    "detections",
                    ArtifactKind::DetectionSet,
                    true,
                    false,
                )],
                score_semantics: ScoreSemantics::DetectionConfidence,
                geometry_semantics: GeometrySemantics::PredictedGeometry,
                runtime_requirements: RuntimeRequirements {
                    devices: vec!["cpu".to_owned()],
                    supports_batch: false,
                    ..RuntimeRequirements::default()
                },
            }
        }
    }

    #[async_trait]
    impl ExpertModelPlugin for TestPlugin {
        fn descriptor(&self) -> PluginRuntimeDescriptor {
            PluginRuntimeDescriptor {
                plugin_id: PluginId::parse("org.annotagent.dummy-detector").expect("id"),
                plugin_version: PluginVersion::parse("1.0.0").expect("version"),
                plugin_api: PLUGIN_API_VERSION.to_owned(),
                protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
                capabilities: vec![ModelCapability::ObjectDetection],
            }
        }

        fn models(&self) -> Vec<ModelRuntimeDescriptor> {
            vec![ModelRuntimeDescriptor {
                model: Self::model(),
                loaded: true,
                checkpoint_sha256: Some(Sha256Digest::of_bytes(b"fixture")),
                device: "cpu".to_owned(),
            }]
        }

        async fn warmup(
            &self,
            model_id: &str,
            _context: WarmupContext,
        ) -> Result<(), PluginSdkError> {
            if model_id == "dummy-detector-v1" {
                Ok(())
            } else {
                Err(PluginSdkError::Plugin("unknown model".to_owned()))
            }
        }

        async fn infer(
            &self,
            request: PipelineInferenceRequest,
            _context: InferenceContext,
        ) -> Result<PipelineInferenceResponse, PluginSdkError> {
            let artifact_id = format!("detections:{}", request.request_id);
            let reference = ArtifactRef {
                artifact_id: artifact_id.clone(),
                source_node: request.node_id,
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: None,
            };
            let detection = DetectionArtifactItem::from_source(
                "det-1",
                None,
                Some("object".to_owned()),
                None,
                NormalizedRect::new(0.25, 0.25, 0.5, 0.5)
                    .map_err(|error| PluginSdkError::Plugin(error.to_string()))?,
                DetectionScore::new(Some(0.9), ScoreSemantics::DetectionConfidence)
                    .map_err(PluginSdkError::Plugin)?,
                DetectionSource {
                    model_id: request.model_id.clone(),
                    capability: VisionCapability::ObjectDetection,
                    artifact_id,
                },
            )
            .map_err(PluginSdkError::Plugin)?;
            Ok(PipelineInferenceResponse {
                request_id: Some(request.request_id),
                model_identity: Some(request.model_id.clone()),
                artifacts: vec![PipelineArtifact::DetectionSet(DetectionSetArtifact {
                    schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
                    reference,
                    image_id: request.image_id,
                    model_binding: request.model_id,
                    validation_state: ArtifactValidationState::Unvalidated,
                    detections: vec![detection],
                    metadata: BTreeMap::new(),
                })],
                ..PipelineInferenceResponse::default()
            })
        }

        async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
            Ok(())
        }
    }

    fn startup(temp: &TempDir) -> PluginStartupConfig {
        PluginStartupConfig {
            session_token: "a".repeat(64),
            session_nonce: "nonce-1".to_owned(),
            state_dir: temp.path().join("state"),
            weights_dir: temp.path().join("weights"),
            cache_dir: temp.path().join("cache"),
            temporary_dir: temp.path().join("temporary"),
            listen_port: 0,
            max_request_bytes: 1_000_000,
            max_response_bytes: 1_000_000,
        }
    }

    #[tokio::test]
    async fn authenticated_protocol_discovers_contracts_and_returns_typed_artifact() {
        let temp = TempDir::new().expect("temp");
        std::fs::create_dir_all(temp.path().join("weights")).expect("weights");
        let config = startup(&temp);
        let token = config.session_token.clone();
        let server = PluginServer::spawn(Arc::new(TestPlugin), config)
            .await
            .expect("server");
        let base = format!("http://{}", server.address);
        let client = reqwest::Client::new();

        let unauthorized = client
            .get(format!("{base}/health"))
            .send()
            .await
            .expect("request");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let health = client
            .get(format!("{base}/health"))
            .bearer_auth(&token)
            .send()
            .await
            .expect("health")
            .json::<PluginHealth>()
            .await
            .expect("health json");
        assert_eq!(health.status, "ready");

        let descriptor = client
            .get(format!("{base}/v1/capabilities"))
            .bearer_auth(&token)
            .send()
            .await
            .expect("capabilities")
            .json::<PluginRuntimeDescriptor>()
            .await
            .expect("capabilities json");
        assert_eq!(descriptor.plugin_api, PLUGIN_API_VERSION);

        let contracts = client
            .get(format!("{base}/v1/contracts"))
            .bearer_auth(&token)
            .send()
            .await
            .expect("contracts")
            .json::<PluginContracts>()
            .await
            .expect("contracts json");
        assert!(matches!(
            contracts.models[0].output_contracts[0].data_type,
            annotagent_core::ContractDataType::Artifact(ArtifactKind::DetectionSet)
        ));

        let request = PipelineInferenceRequest {
            protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: "request-1".to_owned(),
            run_id: RunId::new(),
            image_id: ImageId::new(),
            node_id: "detector".to_owned(),
            model_id: "dummy-detector-v1".to_owned(),
            operation: VisionCapability::ObjectDetection,
            image: None,
            input_artifacts: Vec::new(),
            parameters: BTreeMap::new(),
            timeout_ms: Some(1_000),
        };
        let response = client
            .post(format!("{base}/v1/infer"))
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .expect("infer")
            .json::<PipelineInferenceResponse>()
            .await
            .expect("infer json");
        assert!(response.error.is_none());
        response.artifacts[0].validate().expect("artifact");
        assert!(matches!(
            response.artifacts[0],
            PipelineArtifact::DetectionSet(_)
        ));

        let manifest = PluginManifest::from_toml(include_str!(
            "../../../plugins/dummy-detector/annotagent-plugin.toml"
        ))
        .expect("manifest");
        let report = run_conformance(&base, &token, &manifest, Some(&request)).await;
        assert!(report.passed, "{:#?}", report.checks);

        server.shutdown().await.expect("shutdown");
        assert_eq!(PLUGIN_MANIFEST_SCHEMA_VERSION, "1");
    }

    #[test]
    fn startup_and_image_boundaries_fail_closed() {
        let temp = TempDir::new().expect("temp");
        let mut config = startup(&temp);
        config.session_token = "short".to_owned();
        assert!(config.validate().is_err());

        let invalid = ModelImage {
            id: "image".to_owned(),
            mime_type: "image/png".to_owned(),
            data_base64: "not-base64".to_owned(),
        };
        assert!(decode_image(&invalid, 100).is_err());
    }
}
