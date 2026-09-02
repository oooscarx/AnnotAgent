use std::{path::Path, time::Duration};

use futures::StreamExt as _;
use sha2::Digest as _;
use tokio::io::AsyncWriteExt as _;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    MAX_CATALOG_BUNDLE_BYTES, MAX_CATALOG_BYTES, ModelCatalog, ModelCatalogEntry,
    ModelCatalogError, validate_https_public_url, validate_public_ip,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionStage {
    Resolving,
    Downloading,
    Verifying,
    Installing,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProvisionProgress {
    pub stage: ProvisionStage,
    pub bytes_completed: u64,
    pub bytes_total: Option<u64>,
    pub detail: String,
}

pub struct ModelCatalogClient {
    client: reqwest::Client,
}

impl ModelCatalogClient {
    pub fn new() -> Result<Self, ModelCatalogError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30 * 60))
                .user_agent("AnnotAgent-ModelCatalog/1")
                .build()?,
        })
    }

    pub async fn fetch_catalog(&self, url: &Url) -> Result<ModelCatalog, ModelCatalogError> {
        validate_network_destination(url).await?;
        let response = self.client.get(url.clone()).send().await?;
        if response.status().is_redirection() {
            return Err(ModelCatalogError::UnsafeUrl(
                "catalog redirects are not followed".to_owned(),
            ));
        }
        let response = response.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_CATALOG_BYTES)
        {
            return Err(ModelCatalogError::DownloadSize);
        }
        let bytes =
            bounded_response(response, MAX_CATALOG_BYTES, &CancellationToken::new(), None).await?;
        ModelCatalog::from_json(&bytes)
    }

    pub async fn download_bundle(
        &self,
        entry: &ModelCatalogEntry,
        destination: &Path,
        cancellation: &CancellationToken,
        progress: Option<&tokio::sync::mpsc::UnboundedSender<ProvisionProgress>>,
    ) -> Result<(), ModelCatalogError> {
        entry.validate()?;
        validate_network_destination(&entry.bundle_url).await?;
        send_progress(
            progress,
            ProvisionStage::Resolving,
            0,
            Some(entry.bundle_size_bytes),
            "Trusted public endpoint resolved",
        );
        let response = self.client.get(entry.bundle_url.clone()).send().await?;
        if response.status().is_redirection() {
            return Err(ModelCatalogError::UnsafeUrl(
                "bundle redirects are not followed".to_owned(),
            ));
        }
        let response = response.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|size| size != entry.bundle_size_bytes)
        {
            return Err(ModelCatalogError::DownloadSize);
        }
        if entry.bundle_size_bytes > MAX_CATALOG_BUNDLE_BYTES {
            return Err(ModelCatalogError::DownloadSize);
        }
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary =
            destination.with_extension(format!("download-partial-{}", uuid::Uuid::new_v4()));
        let result = download_response(response, &temporary, entry, cancellation, progress).await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
            return result;
        }
        tokio::fs::rename(temporary, destination).await?;
        Ok(())
    }
}

async fn validate_network_destination(url: &Url) -> Result<(), ModelCatalogError> {
    validate_https_public_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| ModelCatalogError::UnsafeUrl("URL host is missing".to_owned()))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((host, port)).await?;
    let mut found = false;
    for address in addresses {
        found = true;
        validate_public_ip(address.ip())?;
    }
    if !found {
        return Err(ModelCatalogError::UnsafeUrl(
            "host did not resolve to an address".to_owned(),
        ));
    }
    Ok(())
}

async fn bounded_response(
    response: reqwest::Response,
    maximum: u64,
    cancellation: &CancellationToken,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<ProvisionProgress>>,
) -> Result<Vec<u8>, ModelCatalogError> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    let mut total = 0_u64;
    while let Some(chunk) = tokio::select! {
        () = cancellation.cancelled() => return Err(ModelCatalogError::Cancelled),
        chunk = stream.next() => chunk,
    } {
        let chunk = chunk?;
        total = total.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if total > maximum {
            return Err(ModelCatalogError::DownloadSize);
        }
        bytes.extend_from_slice(&chunk);
        send_progress(
            progress,
            ProvisionStage::Downloading,
            total,
            Some(maximum),
            "Downloading",
        );
    }
    Ok(bytes)
}

async fn download_response(
    response: reqwest::Response,
    temporary: &Path,
    entry: &ModelCatalogEntry,
    cancellation: &CancellationToken,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<ProvisionProgress>>,
) -> Result<(), ModelCatalogError> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .await?;
    let mut stream = response.bytes_stream();
    let mut digest = sha2::Sha256::new();
    let mut total = 0_u64;
    while let Some(chunk) = tokio::select! {
        () = cancellation.cancelled() => return Err(ModelCatalogError::Cancelled),
        chunk = stream.next() => chunk,
    } {
        let chunk = chunk?;
        total = total.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if total > entry.bundle_size_bytes || total > MAX_CATALOG_BUNDLE_BYTES {
            return Err(ModelCatalogError::DownloadSize);
        }
        file.write_all(&chunk).await?;
        digest.update(&chunk);
        send_progress(
            progress,
            ProvisionStage::Downloading,
            total,
            Some(entry.bundle_size_bytes),
            "Downloading model Bundle",
        );
    }
    file.flush().await?;
    file.sync_all().await?;
    if total != entry.bundle_size_bytes {
        return Err(ModelCatalogError::DownloadSize);
    }
    let actual = annotagent_model_bundle::Sha256Digest::parse(format!("{:x}", digest.finalize()))
        .map_err(|error| ModelCatalogError::Provisioning(error.to_string()))?;
    if actual != entry.bundle_sha256 {
        return Err(ModelCatalogError::DownloadChecksum);
    }
    send_progress(
        progress,
        ProvisionStage::Verifying,
        total,
        Some(total),
        "Catalog SHA-256 verified",
    );
    Ok(())
}

fn send_progress(
    sender: Option<&tokio::sync::mpsc::UnboundedSender<ProvisionProgress>>,
    stage: ProvisionStage,
    bytes_completed: u64,
    bytes_total: Option<u64>,
    detail: &str,
) {
    if let Some(sender) = sender {
        let _ = sender.send(ProvisionProgress {
            stage,
            bytes_completed,
            bytes_total,
            detail: detail.to_owned(),
        });
    }
}
