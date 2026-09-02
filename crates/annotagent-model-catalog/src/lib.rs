//! Trusted curated catalog metadata and content-addressed model provisioning.

mod compatibility;
mod download;
mod fixture;
mod registry;
mod smoke;

use std::{collections::BTreeSet, net::IpAddr};

use annotagent_core::ModelCapability;
use annotagent_model_bundle::{
    CommercialUseStatus, ModelBundleId, PluginCompatibilityRequirement, RedistributionStatus,
    Sha256Digest,
};
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::{Host, Url};

pub use compatibility::*;
pub use download::*;
pub use fixture::*;
pub use registry::*;
pub use smoke::*;

pub const MODEL_CATALOG_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Error)]
pub enum ModelCatalogError {
    #[error("invalid model catalog: {0}")]
    Invalid(String),
    #[error("unsafe catalog URL: {0}")]
    UnsafeUrl(String),
    #[error("catalog network operation failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("catalog io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("catalog serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("catalog operation was cancelled")]
    Cancelled,
    #[error("download checksum mismatch")]
    DownloadChecksum,
    #[error("download size does not match the catalog entry")]
    DownloadSize,
    #[error("model bundle provisioning failed: {0}")]
    Provisioning(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSignature {
    pub key_id: String,
    pub algorithm: String,
    pub signature_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherIdentity {
    pub id: String,
    pub display_name: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformRequirement {
    pub target: String,
    pub execution_providers: BTreeSet<String>,
    pub minimum_memory_mb: u64,
    pub minimum_disk_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLicenseSummary {
    pub name: String,
    pub license_url: Option<Url>,
    pub license_digest: Sha256Digest,
    pub redistribution: RedistributionStatus,
    pub commercial_use: CommercialUseStatus,
    pub requires_acceptance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalogEntry {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub display_name: String,
    pub description: String,
    pub capabilities: BTreeSet<ModelCapability>,
    pub compatible_plugins: Vec<PluginCompatibilityRequirement>,
    pub platform_requirements: Vec<PlatformRequirement>,
    pub bundle_url: Url,
    pub bundle_sha256: Sha256Digest,
    pub bundle_size_bytes: u64,
    pub license_summary: ModelLicenseSummary,
    pub publisher: PublisherIdentity,
    pub fixture: bool,
    pub publishable: bool,
}

impl ModelCatalogEntry {
    pub fn validate(&self) -> Result<(), ModelCatalogError> {
        validate_text("display name", &self.display_name)?;
        validate_text("description", &self.description)?;
        validate_https_public_url(&self.bundle_url)?;
        if self.capabilities.is_empty()
            || self.compatible_plugins.is_empty()
            || self.platform_requirements.is_empty()
            || self.bundle_size_bytes == 0
            || self.bundle_size_bytes > MAX_CATALOG_BUNDLE_BYTES
        {
            return invalid(
                "entry capabilities, compatibility, platform and bounded size are required",
            );
        }
        if self.fixture == self.publishable {
            return invalid(
                "fixture entries must be non-publishable and release entries non-fixture",
            );
        }
        if self.license_summary.redistribution == RedistributionStatus::Prohibited
            && self.publishable
        {
            return invalid("a redistribution-prohibited entry cannot be publishable");
        }
        for requirement in &self.compatible_plugins {
            requirement
                .version_requirement()
                .map_err(|error| ModelCatalogError::Invalid(error.to_string()))?;
        }
        for platform in &self.platform_requirements {
            validate_text("platform target", &platform.target)?;
            if platform.execution_providers.is_empty()
                || platform.minimum_memory_mb == 0
                || platform.minimum_disk_bytes < self.bundle_size_bytes
            {
                return invalid("platform requirements are incomplete");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalog {
    pub schema_version: String,
    pub catalog_id: String,
    pub generated_at: DateTime<Utc>,
    pub entries: Vec<ModelCatalogEntry>,
    pub signature: Option<CatalogSignature>,
}

impl ModelCatalog {
    pub fn from_json(bytes: &[u8]) -> Result<Self, ModelCatalogError> {
        let catalog = serde_json::from_slice::<Self>(bytes)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), ModelCatalogError> {
        if self.schema_version != MODEL_CATALOG_SCHEMA_VERSION {
            return invalid("unsupported catalog schema");
        }
        validate_text("catalog id", &self.catalog_id)?;
        let mut identities = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !identities.insert((entry.bundle_id.clone(), entry.bundle_version.clone())) {
                return invalid("catalog contains a duplicate bundle identity");
            }
        }
        if let Some(signature) = &self.signature {
            if signature.algorithm != "ed25519"
                || signature.key_id.trim().is_empty()
                || signature.signature_base64.trim().is_empty()
            {
                return invalid("catalog signature metadata is invalid");
            }
        }
        Ok(())
    }
}

pub const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_CATALOG_BUNDLE_BYTES: u64 = 32 * 1024 * 1024 * 1024;

pub fn validate_https_public_url(url: &Url) -> Result<(), ModelCatalogError> {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(ModelCatalogError::UnsafeUrl(
            "only credential-free HTTPS URLs are allowed".to_owned(),
        ));
    }
    let host = url
        .host()
        .ok_or_else(|| ModelCatalogError::UnsafeUrl("URL host is missing".to_owned()))?;
    match host {
        Host::Domain(domain) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            if domain == "localhost"
                || domain.ends_with(".localhost")
                || domain
                    .rsplit_once('.')
                    .is_some_and(|(_, suffix)| suffix == "local")
            {
                return Err(ModelCatalogError::UnsafeUrl(
                    "local host names are not allowed".to_owned(),
                ));
            }
        }
        Host::Ipv4(address) => validate_public_ip(IpAddr::V4(address))?,
        Host::Ipv6(address) => validate_public_ip(IpAddr::V6(address))?,
    }
    Ok(())
}

pub fn validate_public_ip(address: IpAddr) -> Result<(), ModelCatalogError> {
    let unsafe_address = match address {
        IpAddr::V4(value) => {
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_broadcast()
                || value.is_documentation()
                || value.is_unspecified()
                || value.octets()[0] == 0
        }
        IpAddr::V6(value) => {
            value.is_loopback()
                || value.is_unspecified()
                || value.is_unique_local()
                || value.is_unicast_link_local()
                || value.to_ipv4_mapped().is_some_and(|mapped| {
                    mapped.is_private() || mapped.is_loopback() || mapped.is_link_local()
                })
        }
    };
    if unsafe_address {
        return Err(ModelCatalogError::UnsafeUrl(format!(
            "private, local and non-routable address {address} is not allowed"
        )));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), ModelCatalogError> {
    if value.trim().is_empty() || value.len() > 2_000 || value.contains(['\r', '\n']) {
        return invalid(&format!("{field} is invalid"));
    }
    Ok(())
}

fn invalid<T>(message: &str) -> Result<T, ModelCatalogError> {
    Err(ModelCatalogError::Invalid(message.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_urls_reject_credentials_local_and_private_targets() {
        for value in [
            "http://models.example/bundle.annotmodel",
            "https://user:secret@models.example/bundle.annotmodel",
            "https://localhost/bundle.annotmodel",
            "https://127.0.0.1/bundle.annotmodel",
            "https://10.1.2.3/bundle.annotmodel",
            "https://[::1]/bundle.annotmodel",
        ] {
            assert!(
                validate_https_public_url(&Url::parse(value).expect("URL")).is_err(),
                "{value}"
            );
        }
        assert!(
            validate_https_public_url(
                &Url::parse("https://models.annotagent.example/bundle.annotmodel").expect("URL")
            )
            .is_ok()
        );
    }
}
