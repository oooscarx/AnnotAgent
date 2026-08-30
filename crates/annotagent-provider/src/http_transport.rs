//! Shared, bounded transport rules for local HTTP Vision Workers.

use std::{net::IpAddr, time::Duration};

use annotagent_core::{CoreError, CoreResult};
use reqwest::{Client, Response, Url, redirect::Policy};

pub(crate) const MAX_CONFIGURED_WORKER_RESPONSE_BYTES: usize = 16_000_000;
pub(crate) const MAX_WORKER_RETRIES: u32 = 3;

pub(crate) fn validate_worker_base_url(endpoint: &str, allow_remote: bool) -> CoreResult<Url> {
    let mut url = Url::parse(endpoint)
        .map_err(|error| CoreError::Validation(format!("invalid HTTP Worker endpoint: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CoreError::Validation(
            "HTTP Worker endpoint must be an http(s) URL without credentials, query, or fragment"
                .to_owned(),
        ));
    }
    let host = url.host_str().unwrap_or_default();
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback && !allow_remote {
        return Err(CoreError::Validation(
            "remote HTTP Worker endpoints require explicit allow_remote=true".to_owned(),
        ));
    }
    if !loopback && url.scheme() != "https" {
        return Err(CoreError::Validation(
            "explicitly enabled remote HTTP Workers must use HTTPS".to_owned(),
        ));
    }
    if !url.path().ends_with('/') {
        let mut path = url.path().to_owned();
        path.push('/');
        url.set_path(&path);
    }
    Ok(url)
}

pub(crate) fn endpoint(base_url: &Url, path: &str) -> CoreResult<Url> {
    base_url
        .join(path.trim_start_matches('/'))
        .map_err(|error| CoreError::Validation(format!("invalid HTTP Worker route: {error}")))
}

pub(crate) fn build_worker_client(timeout: Duration) -> CoreResult<Client> {
    if timeout.is_zero() {
        return Err(CoreError::Validation(
            "HTTP Worker timeout must be greater than zero".to_owned(),
        ));
    }
    Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(Policy::none())
        .build()
        .map_err(|error| CoreError::Provider(format!("cannot build HTTP Worker client: {error}")))
}

pub(crate) fn validate_transport_limits(
    max_response_bytes: usize,
    max_retries: u32,
) -> CoreResult<()> {
    if max_response_bytes == 0 || max_response_bytes > MAX_CONFIGURED_WORKER_RESPONSE_BYTES {
        return Err(CoreError::Validation(format!(
            "HTTP Worker max_response_bytes must be within 1..={MAX_CONFIGURED_WORKER_RESPONSE_BYTES}"
        )));
    }
    if max_retries > MAX_WORKER_RETRIES {
        return Err(CoreError::Validation(format!(
            "HTTP Worker max_retries cannot exceed {MAX_WORKER_RETRIES}"
        )));
    }
    Ok(())
}

pub(crate) async fn bounded_response_body(
    mut response: Response,
    max_bytes: usize,
) -> CoreResult<(reqwest::StatusCode, Vec<u8>)> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(CoreError::Provider(format!(
            "HTTP Worker response exceeds {max_bytes} bytes"
        )));
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(max_bytes),
    );
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        CoreError::Provider(format!("cannot read HTTP Worker response: {error}"))
    })? {
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(CoreError::Provider(format!(
                "HTTP Worker response exceeds {max_bytes} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((status, body))
}
