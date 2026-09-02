use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use annotagent_core::{
    CoreError, CoreResult, PipelineInferenceRequest, PipelineInferenceResponse,
    PipelineModelBackend, VisionCapability,
};
use annotagent_plugin_api::{
    CancelRequest, CancelResponse, PluginHealth, PluginManifest, PluginReadyHandshake,
    PluginStatus, PluginTestReport, ShutdownRequest,
};
use annotagent_plugin_sdk::{PluginStartupConfig, run_conformance};
use reqwest::Url;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader},
    process::{Child, Command},
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
const MAX_LOG_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("plugin host configuration is invalid: {0}")]
    InvalidConfiguration(String),
    #[error("plugin process io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("plugin handshake timed out")]
    StartupTimeout,
    #[error("plugin handshake is invalid: {0}")]
    InvalidHandshake(String),
    #[error("plugin request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("plugin process is no longer running")]
    Crashed,
}

#[derive(Debug, Clone)]
pub struct PluginProcessConfig {
    pub executable: PathBuf,
    pub installation_root: PathBuf,
    pub state_dir: PathBuf,
    pub weights_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temporary_dir: PathBuf,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
}

pub struct HostedPlugin {
    manifest: PluginManifest,
    base_url: Url,
    session_token: String,
    child: Arc<Mutex<Child>>,
    stdout_log: Arc<Mutex<Vec<u8>>>,
    stderr_log: Arc<Mutex<Vec<u8>>>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    client: reqwest::Client,
}

impl HostedPlugin {
    pub async fn start(
        manifest: PluginManifest,
        config: PluginProcessConfig,
    ) -> Result<Self, PluginHostError> {
        validate_process_config(&config)?;
        let session_token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let session_nonce = uuid::Uuid::new_v4().to_string();
        for path in [&config.state_dir, &config.cache_dir, &config.temporary_dir] {
            tokio::fs::create_dir_all(path).await?;
        }
        if !config.weights_dir.is_dir() {
            return Err(PluginHostError::InvalidConfiguration(
                "weights directory must exist".to_owned(),
            ));
        }
        let mut command = Command::new(&config.executable);
        command
            .env_clear()
            .env("RUST_BACKTRACE", "0")
            .current_dir(&config.state_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let startup = PluginStartupConfig {
            session_token: session_token.clone(),
            session_nonce: session_nonce.clone(),
            state_dir: config.state_dir,
            weights_dir: config.weights_dir,
            cache_dir: config.cache_dir,
            temporary_dir: config.temporary_dir,
            listen_port: 0,
            max_request_bytes: config.max_request_bytes,
            max_response_bytes: config.max_response_bytes,
        };
        let mut stdin = child.stdin.take().ok_or_else(|| {
            PluginHostError::InvalidConfiguration("child stdin is unavailable".to_owned())
        })?;
        stdin
            .write_all(
                &serde_json::to_vec(&startup)
                    .map_err(|error| PluginHostError::InvalidConfiguration(error.to_string()))?,
            )
            .await?;
        stdin.shutdown().await?;
        drop(stdin);
        let stdout = child.stdout.take().ok_or_else(|| {
            PluginHostError::InvalidConfiguration("child stdout is unavailable".to_owned())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            PluginHostError::InvalidConfiguration("child stderr is unavailable".to_owned())
        })?;
        let mut stdout = BufReader::new(stdout);
        let mut handshake_line = String::new();
        let read = tokio::time::timeout(
            Duration::from_secs(manifest.runtime.startup_timeout_seconds),
            stdout.read_line(&mut handshake_line),
        )
        .await
        .map_err(|_| PluginHostError::StartupTimeout)??;
        if read == 0 || handshake_line.len() > MAX_HANDSHAKE_BYTES {
            return Err(PluginHostError::InvalidHandshake(
                "handshake line is empty or too large".to_owned(),
            ));
        }
        let handshake: PluginReadyHandshake = serde_json::from_str(handshake_line.trim())
            .map_err(|error| PluginHostError::InvalidHandshake(error.to_string()))?;
        validate_handshake(&manifest, &handshake, &session_nonce)?;
        let base_url = Url::parse(&format!("http://{}", handshake.listen))
            .map_err(|error| PluginHostError::InvalidHandshake(error.to_string()))?;
        if base_url.host_str().is_none_or(|host| host != "127.0.0.1") {
            return Err(PluginHostError::InvalidHandshake(
                "plugin did not bind IPv4 loopback".to_owned(),
            ));
        }
        let stdout_log = Arc::new(Mutex::new(Vec::new()));
        let stderr_log = Arc::new(Mutex::new(Vec::new()));
        let stdout_task = capture_bounded(stdout, stdout_log.clone());
        let stderr_task = capture_bounded(BufReader::new(stderr), stderr_log.clone());
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(
                manifest.runtime.startup_timeout_seconds,
            ))
            .build()?;
        Ok(Self {
            manifest,
            base_url,
            session_token,
            child: Arc::new(Mutex::new(child)),
            stdout_log,
            stderr_log,
            stdout_task,
            stderr_task,
            client,
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &Url {
        &self.base_url
    }

    pub async fn status(&self) -> Result<PluginStatus, PluginHostError> {
        if self.child.lock().await.try_wait()?.is_some() {
            Ok(PluginStatus::Crashed)
        } else {
            Ok(PluginStatus::Ready)
        }
    }

    pub async fn health(&self) -> Result<PluginHealth, PluginHostError> {
        self.ensure_running().await?;
        Ok(self
            .client
            .get(self.base_url.join("health").expect("static endpoint"))
            .bearer_auth(&self.session_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn infer(
        &self,
        request: &PipelineInferenceRequest,
    ) -> Result<PipelineInferenceResponse, PluginHostError> {
        self.ensure_running().await?;
        Ok(self
            .client
            .post(self.base_url.join("v1/infer").expect("static endpoint"))
            .bearer_auth(&self.session_token)
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn cancel_request(
        &self,
        request_id: &str,
    ) -> Result<CancelResponse, PluginHostError> {
        self.ensure_running().await?;
        Ok(self
            .client
            .post(self.base_url.join("v1/cancel").expect("static endpoint"))
            .bearer_auth(&self.session_token)
            .json(&CancelRequest {
                request_id: request_id.to_owned(),
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn test(
        &self,
        sample: Option<&PipelineInferenceRequest>,
    ) -> Result<PluginTestReport, PluginHostError> {
        self.ensure_running().await?;
        Ok(run_conformance(
            self.base_url.as_str().trim_end_matches('/'),
            &self.session_token,
            &self.manifest,
            sample,
        )
        .await)
    }

    pub async fn logs(&self) -> (String, String) {
        (
            redact_log(&self.stdout_log.lock().await, &self.session_token),
            redact_log(&self.stderr_log.lock().await, &self.session_token),
        )
    }

    pub async fn stop(&self) -> Result<(), PluginHostError> {
        if self.child.lock().await.try_wait()?.is_some() {
            return Ok(());
        }
        let _ = self
            .client
            .post(self.base_url.join("v1/shutdown").expect("static endpoint"))
            .bearer_auth(&self.session_token)
            .json(&ShutdownRequest {
                reason: "host shutdown".to_owned(),
            })
            .send()
            .await;
        let timeout = Duration::from_secs(self.manifest.runtime.shutdown_timeout_seconds);
        let mut child = self.child.lock().await;
        if tokio::time::timeout(timeout, child.wait()).await.is_err() {
            child.kill().await?;
            let _ = child.wait().await;
        }
        Ok(())
    }

    pub async fn kill_for_test(&self) -> Result<(), PluginHostError> {
        self.child.lock().await.kill().await?;
        Ok(())
    }

    async fn ensure_running(&self) -> Result<(), PluginHostError> {
        if self.child.lock().await.try_wait()?.is_some() {
            Err(PluginHostError::Crashed)
        } else {
            Ok(())
        }
    }
}

/// Core-facing adapter for one already hosted, exact plugin model.
pub struct PluginPipelineBackend {
    id: String,
    capability: VisionCapability,
    plugin: Arc<HostedPlugin>,
}

impl PluginPipelineBackend {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        capability: VisionCapability,
        plugin: Arc<HostedPlugin>,
    ) -> Self {
        Self {
            id: id.into(),
            capability,
            plugin,
        }
    }
}

#[async_trait::async_trait]
impl PipelineModelBackend for PluginPipelineBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        self.capability
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        let request_id = request.request_id.clone();
        tokio::select! {
            result = self.plugin.infer(&request) => {
                result.map_err(|error| CoreError::Provider(error.to_string()))
            }
            () = cancellation.cancelled() => {
                let _ = self.plugin.cancel_request(&request_id).await;
                Err(CoreError::Provider("plugin inference cancelled".to_owned()))
            }
        }
    }
}

impl Drop for HostedPlugin {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
        self.stdout_task.abort();
        self.stderr_task.abort();
    }
}

fn validate_process_config(config: &PluginProcessConfig) -> Result<(), PluginHostError> {
    let root = config.installation_root.canonicalize()?;
    let executable = config.executable.canonicalize()?;
    if !executable.starts_with(&root) || !executable.is_file() {
        return Err(PluginHostError::InvalidConfiguration(
            "executable must be a regular file inside its installation root".to_owned(),
        ));
    }
    if config.max_request_bytes == 0 || config.max_response_bytes == 0 {
        return Err(PluginHostError::InvalidConfiguration(
            "transport limits must be greater than zero".to_owned(),
        ));
    }
    Ok(())
}

fn validate_handshake(
    manifest: &PluginManifest,
    handshake: &PluginReadyHandshake,
    nonce: &str,
) -> Result<(), PluginHostError> {
    if handshake.status != "ready"
        || handshake.plugin_id != manifest.id
        || handshake.plugin_api != manifest.plugin_api
        || handshake.protocol_version != annotagent_plugin_api::PLUGIN_PROTOCOL_VERSION
        || handshake.session_nonce != nonce
    {
        return Err(PluginHostError::InvalidHandshake(
            "runtime identity, protocol or nonce does not match".to_owned(),
        ));
    }
    let address = handshake
        .listen
        .parse::<std::net::SocketAddr>()
        .map_err(|error| PluginHostError::InvalidHandshake(error.to_string()))?;
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err(PluginHostError::InvalidHandshake(
            "plugin address must be a selected loopback port".to_owned(),
        ));
    }
    Ok(())
}

fn capture_bounded<R>(reader: R, destination: Arc<Mutex<Vec<u8>>>) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = reader
            .take(u64::try_from(MAX_LOG_BYTES).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .await;
        *destination.lock().await = bytes;
    })
}

fn redact_log(bytes: &[u8], token: &str) -> String {
    String::from_utf8_lossy(bytes)
        .replace(token, "[redacted]")
        .chars()
        .filter(|character| *character != '\0')
        .take(MAX_LOG_BYTES)
        .collect()
}

#[must_use]
pub fn process_directories(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (
        root.join("state"),
        root.join("cache"),
        root.join("temporary"),
    )
}
