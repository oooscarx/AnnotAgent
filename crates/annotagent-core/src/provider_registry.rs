//! Persistent Provider connection and credential-reference contracts.

use std::{collections::BTreeMap, fmt, net::IpAddr};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use zeroize::Zeroizing;

use crate::{ModelProfileId, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdapterKind {
    OpenAiCompatible,
    Mock,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConnectionPolicy {
    pub request_timeout_seconds: u64,
    pub maximum_retries: u32,
    pub maximum_concurrency: u32,
    pub minimum_retry_delay_ms: u64,
    pub maximum_retry_delay_ms: u64,
    pub allow_remote_http: bool,
    pub allowed_redirects: u32,
}

impl Default for ProviderConnectionPolicy {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 120,
            maximum_retries: 2,
            maximum_concurrency: 4,
            minimum_retry_delay_ms: 250,
            maximum_retry_delay_ms: 5_000,
            allow_remote_http: false,
            allowed_redirects: 0,
        }
    }
}

impl ProviderConnectionPolicy {
    pub fn validate(&self) -> Result<(), ProviderProfileValidationError> {
        if !(1..=3_600).contains(&self.request_timeout_seconds) {
            return Err(ProviderProfileValidationError::InvalidConnectionPolicy(
                "request_timeout_seconds must be within 1..=3600".to_owned(),
            ));
        }
        if self.maximum_retries > 10 {
            return Err(ProviderProfileValidationError::InvalidConnectionPolicy(
                "maximum_retries cannot exceed 10".to_owned(),
            ));
        }
        if !(1..=128).contains(&self.maximum_concurrency) {
            return Err(ProviderProfileValidationError::InvalidConnectionPolicy(
                "maximum_concurrency must be within 1..=128".to_owned(),
            ));
        }
        if self.minimum_retry_delay_ms > self.maximum_retry_delay_ms
            || self.maximum_retry_delay_ms > 120_000
        {
            return Err(ProviderProfileValidationError::InvalidConnectionPolicy(
                "retry delays must be ordered and maximum_retry_delay_ms cannot exceed 120000"
                    .to_owned(),
            ));
        }
        if self.allowed_redirects > 5 {
            return Err(ProviderProfileValidationError::InvalidConnectionPolicy(
                "allowed_redirects cannot exceed 5".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    SystemKeyring,
    EnvironmentVariable,
    SessionOnly,
    LegacyWorkspaceFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialReference {
    pub provider_id: ProviderId,
    pub source: CredentialSource,
    /// Opaque lookup key: keyring account, environment variable, session token, or legacy path.
    pub locator: String,
}

impl CredentialReference {
    pub fn validate(&self) -> Result<(), SecretStoreError> {
        if self.locator.trim().is_empty() || self.locator.len() > 1_024 {
            return Err(SecretStoreError::InvalidReference(
                "credential locator must be non-empty and at most 1024 bytes".to_owned(),
            ));
        }
        if self.source == CredentialSource::EnvironmentVariable
            && !valid_environment_name(&self.locator)
        {
            return Err(SecretStoreError::InvalidReference(
                "environment credential locator must be a valid variable name".to_owned(),
            ));
        }
        if self.locator.contains(['\r', '\n']) {
            return Err(SecretStoreError::InvalidReference(
                "credential locator cannot contain line breaks".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretScope {
    pub provider_id: ProviderId,
    pub source: CredentialSource,
    pub locator: String,
}

impl SecretScope {
    #[must_use]
    pub fn reference(&self) -> CredentialReference {
        CredentialReference {
            provider_id: self.provider_id,
            source: self.source,
            locator: self.locator.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretStoreError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SecretStoreError::InvalidSecret(
                "secret value cannot be empty".to_owned(),
            ));
        }
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose_secret(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    #[error("credential was not found")]
    NotFound,
    #[error("credential store is unavailable: {0}")]
    Unavailable(String),
    #[error("invalid credential reference: {0}")]
    InvalidReference(String),
    #[error("invalid secret: {0}")]
    InvalidSecret(String),
    #[error("credential source is read-only: {0}")]
    ReadOnly(String),
    #[error("credential operation failed: {0}")]
    OperationFailed(String),
}

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError>;

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError>;

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError>;

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealthStatus {
    Unknown,
    Configured,
    Available,
    Unreachable,
    InvalidCredential,
    RateLimited,
    IncompatibleProtocol,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthSnapshot {
    pub status: ProviderHealthStatus,
    pub safe_message: Option<String>,
    pub checked_at: Option<DateTime<Utc>>,
}

impl Default for ProviderHealthSnapshot {
    fn default() -> Self {
        Self {
            status: ProviderHealthStatus::Unknown,
            safe_message: None,
            checked_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderProfile {
    pub id: ProviderId,
    pub display_name: String,
    pub preset_id: Option<String>,
    pub adapter: ProviderAdapterKind,
    pub base_url: Url,
    pub organization: Option<String>,
    pub workspace: Option<String>,
    pub credential_ref: Option<CredentialReference>,
    pub safe_headers: BTreeMap<String, String>,
    pub connection_policy: ProviderConnectionPolicy,
    pub enabled: bool,
    pub health: ProviderHealthSnapshot,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderProfile {
    pub fn validate(&self) -> Result<(), ProviderProfileValidationError> {
        if self.display_name.trim().is_empty() || self.display_name.len() > 120 {
            return Err(ProviderProfileValidationError::InvalidDisplayName);
        }
        self.connection_policy.validate()?;
        validate_provider_url(&self.base_url, &self.connection_policy)?;
        for (name, value) in &self.safe_headers {
            validate_safe_header(name, value)?;
        }
        if let Some(reference) = &self.credential_ref {
            if reference.provider_id != self.id {
                return Err(ProviderProfileValidationError::CredentialScopeMismatch);
            }
            reference.validate().map_err(|error| {
                ProviderProfileValidationError::InvalidCredentialReference {
                    safe_message: error.to_string(),
                }
            })?;
        }
        if !self.enabled && self.health.status != ProviderHealthStatus::Disabled {
            return Err(ProviderProfileValidationError::DisabledHealthMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub fn endpoint_summary(&self) -> String {
        let port = self
            .base_url
            .port()
            .map_or_else(String::new, |port| format!(":{port}"));
        format!(
            "{}://{}{}",
            self.base_url.scheme(),
            self.base_url.host_str().unwrap_or("invalid-host"),
            port
        )
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderProfileValidationError {
    #[error("provider display name must be non-empty and at most 120 bytes")]
    InvalidDisplayName,
    #[error("invalid provider endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("invalid provider connection policy: {0}")]
    InvalidConnectionPolicy(String),
    #[error("unsafe provider header {name:?}: {safe_message}")]
    UnsafeHeader { name: String, safe_message: String },
    #[error("credential reference belongs to another provider")]
    CredentialScopeMismatch,
    #[error("invalid credential reference: {safe_message}")]
    InvalidCredentialReference { safe_message: String },
    #[error("a disabled provider must have Disabled health")]
    DisabledHealthMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    InvalidEndpoint,
    MissingCredential,
    InvalidCredential,
    Unreachable,
    Timeout,
    RateLimited,
    IncompatibleProtocol,
    ModelNotFound,
    UnsupportedCapability,
    ResponseTooLarge,
    InvalidResponse,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderErrorDetails {
    pub code: ProviderErrorCode,
    pub provider_id: ProviderId,
    pub model_profile_id: Option<ModelProfileId>,
    pub operation: String,
    pub recoverable: bool,
    pub retry_after_ms: Option<u64>,
    pub safe_message: String,
}

fn validate_provider_url(
    url: &Url,
    policy: &ProviderConnectionPolicy,
) -> Result<(), ProviderProfileValidationError> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ProviderProfileValidationError::InvalidEndpoint(
            "embedded username or password is forbidden".to_owned(),
        ));
    }
    if url.host_str().is_none() || url.fragment().is_some() {
        return Err(ProviderProfileValidationError::InvalidEndpoint(
            "base URL requires a host and cannot contain a fragment".to_owned(),
        ));
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(url.host_str().unwrap_or_default()) => Ok(()),
        "http" if policy.allow_remote_http => Ok(()),
        "http" => Err(ProviderProfileValidationError::InvalidEndpoint(
            "remote HTTP is disabled; use HTTPS or an explicit loopback endpoint".to_owned(),
        )),
        _ => Err(ProviderProfileValidationError::InvalidEndpoint(
            "provider endpoint must use HTTPS, or HTTP for an allowed endpoint".to_owned(),
        )),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn validate_safe_header(name: &str, value: &str) -> Result<(), ProviderProfileValidationError> {
    const ALLOWED: [&str; 7] = [
        "accept",
        "content-type",
        "http-referer",
        "openai-organization",
        "openai-project",
        "user-agent",
        "x-title",
    ];
    let normalized = name.trim().to_ascii_lowercase();
    if !ALLOWED.contains(&normalized.as_str()) {
        return Err(ProviderProfileValidationError::UnsafeHeader {
            name: name.to_owned(),
            safe_message: "header name is not in the Provider metadata allow-list".to_owned(),
        });
    }
    if value.contains(['\r', '\n']) || value.len() > 1_024 {
        return Err(ProviderProfileValidationError::UnsafeHeader {
            name: name.to_owned(),
            safe_message: "header value contains line breaks or exceeds 1024 bytes".to_owned(),
        });
    }
    Ok(())
}

fn valid_environment_name(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(base_url: &str) -> ProviderProfile {
        let now = Utc::now();
        ProviderProfile {
            id: ProviderId::new(),
            display_name: "Qwen Lab".to_owned(),
            preset_id: Some("dashscope".to_owned()),
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: Url::parse(base_url).expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot::default(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn multiple_profiles_may_share_a_vendor_without_sharing_identity() {
        let first = profile("https://dashscope.aliyuncs.com/compatible-mode/v1");
        let mut second = first.clone();
        second.id = ProviderId::new();
        second.display_name = "Qwen Personal".to_owned();
        first.validate().expect("first");
        second.validate().expect("second");
        assert_ne!(first.id, second.id);
        assert_eq!(first.preset_id, second.preset_id);
    }

    #[test]
    fn endpoint_policy_allows_https_and_loopback_but_rejects_unsafe_urls() {
        profile("https://provider.example/v1")
            .validate()
            .expect("HTTPS");
        profile("http://127.0.0.1:8000/v1")
            .validate()
            .expect("loopback");
        assert!(profile("http://provider.example/v1").validate().is_err());
        assert!(
            profile("https://user:password@provider.example/v1")
                .validate()
                .is_err()
        );
        assert!(profile("file:///tmp/provider").validate().is_err());
    }

    #[test]
    fn only_safe_metadata_headers_are_accepted() {
        let mut valid = profile("https://openrouter.ai/api/v1");
        valid.safe_headers.insert(
            "HTTP-Referer".to_owned(),
            "https://annotagent.local".to_owned(),
        );
        valid.validate().expect("safe header");
        valid
            .safe_headers
            .insert("Authorization".to_owned(), "Bearer forbidden".to_owned());
        assert!(valid.validate().is_err());
    }

    #[test]
    fn credential_reference_is_provider_scoped_and_secret_debug_is_masked() {
        let mut candidate = profile("https://provider.example/v1");
        candidate.credential_ref = Some(CredentialReference {
            provider_id: candidate.id,
            source: CredentialSource::EnvironmentVariable,
            locator: "ANNOTAGENT_TEST_KEY".to_owned(),
        });
        candidate.validate().expect("scoped reference");
        let secret = SecretValue::new("top-secret-value").expect("secret");
        assert_eq!(format!("{secret:?}"), "SecretValue([REDACTED])");
        assert!(!format!("{secret:?}").contains(secret.expose_secret()));
        candidate
            .credential_ref
            .as_mut()
            .expect("reference")
            .provider_id = ProviderId::new();
        assert!(candidate.validate().is_err());
    }

    #[test]
    fn disabled_provider_health_is_explicit() {
        let mut candidate = profile("https://provider.example/v1");
        candidate.enabled = false;
        assert!(candidate.validate().is_err());
        candidate.health.status = ProviderHealthStatus::Disabled;
        candidate.validate().expect("disabled health");
    }
}
