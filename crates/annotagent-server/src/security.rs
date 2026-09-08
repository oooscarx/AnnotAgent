use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use url::Url;
use uuid::Uuid;

#[cfg(test)]
mod control_tests {
    use super::*;
    use axum::{Router, body::Body, middleware, routing::post};
    use tower::ServiceExt;

    fn request(security: &LocalSecurity, path: &str, authorized: bool) -> Request {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::HOST, "127.0.0.1:8791")
            .header(header::COOKIE, security.session_cookie());
        if authorized {
            request = request.header(CSRF_HEADER, security.csrf_token.as_str());
        }
        request.body(Body::empty()).unwrap()
    }
    #[test]
    fn control_allowlist_rejects_resume_and_suffix_impostors() {
        let id = Uuid::new_v4();
        for path in [
            format!("/api/batches/{id}/pause"),
            format!("/api/agent-sessions/{id}/cancel"),
            format!("/api/projects/test/sample-operations/{id}/cancel"),
            format!("/api/projects/test/conversations/{id}/tasks/{id}/calls/{id}/cancel"),
            format!(
                "/api/projects/test/conversations/{id}/tasks/{id}/calls/{id}/clarification/cancel"
            ),
            format!("/api/projects/test/conversations/{id}/tasks/{id}/human-requests/{id}/cancel"),
        ] {
            assert!(is_execution_control(&Method::POST, &path), "{path}");
            assert!(!is_execution_control(&Method::DELETE, &path));
        }
        for path in [
            format!("/api/runs/{id}/resume"),
            "/api/runs/not-an-id/cancel".into(),
            format!("/api/models/{id}/cancel"),
            format!("/api/runs/{id}/cancel/extra"),
            format!("/api/runs/{id}/cancel/"),
            format!("/api/projects/test/conversations/{id}/tasks/{id}/builder-operations"),
            format!("/api/projects/test/conversations/{id}/tasks/{id}/human-requests/{id}/resume"),
        ] {
            assert!(!is_execution_control(&Method::POST, &path), "{path}");
        }
    }
    #[tokio::test]
    async fn control_lane_has_its_own_rate_and_concurrency_bounds() {
        let security = LocalSecurity::default();
        let path = format!("/api/runs/{}/cancel", Uuid::new_v4());
        let router = Router::new()
            .route(
                "/api/runs/{id}/cancel",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .layer(middleware::from_fn_with_state(
                security.clone(),
                protect_local_api,
            ));
        let held = security
            .control_limit
            .clone()
            .acquire_many_owned(MAX_CONCURRENT_CONTROL_ACTIONS as u32)
            .await
            .unwrap();
        let limited = router
            .clone()
            .oneshot(request(&security, &path, true))
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = axum::body::to_bytes(limited.into_body(), 4096)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains("control_action_concurrency_limited"));
        drop(held);
        for _ in 1..MAX_CONTROL_ACTIONS_PER_MINUTE {
            assert_eq!(
                router
                    .clone()
                    .oneshot(request(&security, &path, true))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::NO_CONTENT
            );
        }
        let limited = router
            .oneshot(request(&security, &path, true))
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = axum::body::to_bytes(limited.into_body(), 4096)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains("control_action_rate_limited"));
        assert!(security.mutation_times.lock().await.is_empty());
    }
    #[tokio::test]
    async fn stop_survives_mutation_saturation_without_bypassing_auth_or_start_limits() {
        let security = LocalSecurity::default();
        for _ in 0..MAX_MUTATIONS_PER_MINUTE {
            assert!(security.mutation_rate_available().await);
        }
        let _held = security
            .mutation_limit
            .clone()
            .acquire_many_owned(MAX_CONCURRENT_MUTATIONS as u32)
            .await
            .unwrap();
        let router = Router::new()
            .route(
                "/api/runs/{id}/cancel",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/runs/{id}/resume",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .layer(middleware::from_fn_with_state(
                security.clone(),
                protect_local_api,
            ));
        let id = Uuid::new_v4();
        let cancel = format!("/api/runs/{id}/cancel");
        assert_eq!(
            router
                .clone()
                .oneshot(request(&security, &cancel, false))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            router
                .clone()
                .oneshot(request(&security, &cancel, true))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            router
                .clone()
                .oneshot(request(&security, &format!("/api/runs/{id}/resume"), true))
                .await
                .unwrap()
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        let mut cross = request(&security, &cancel, true);
        cross.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("https://foreign.invalid"),
        );
        assert_eq!(
            router.oneshot(cross).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
}

pub(crate) const MAX_JSON_BODY_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const SESSION_COOKIE: &str = "annotagent_session";
pub(crate) const CSRF_HEADER: &str = "x-annotagent-csrf";
pub(crate) const PRIVILEGED_CONFIRMATION_HEADER: &str = "x-annotagent-privileged-confirmation";
pub(crate) const REQUEST_ID_HEADER: &str = "x-request-id";

const MAX_MUTATIONS_PER_MINUTE: usize = 120;
const MAX_CONCURRENT_MUTATIONS: usize = 16;
const MAX_CONTROL_ACTIONS_PER_MINUTE: usize = 30;
const MAX_CONCURRENT_CONTROL_ACTIONS: usize = 4;
const MAX_CONCURRENT_EXPENSIVE_ACTIONS: usize = 4;
const MAX_SSE_CLIENTS: usize = 8;
const PRIVILEGED_GRANT_LIFETIME: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct PrivilegedGrant {
    action: String,
    expires_at: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalSecurity {
    session_id: Arc<String>,
    csrf_token: Arc<String>,
    privileged_grants: Arc<Mutex<HashMap<String, PrivilegedGrant>>>,
    mutation_times: Arc<Mutex<VecDeque<Instant>>>,
    mutation_limit: Arc<Semaphore>,
    control_times: Arc<Mutex<VecDeque<Instant>>>,
    control_limit: Arc<Semaphore>,
    expensive_action_limit: Arc<Semaphore>,
    sse_limit: Arc<Semaphore>,
}

impl Default for LocalSecurity {
    fn default() -> Self {
        Self {
            session_id: Arc::new(Uuid::new_v4().to_string()),
            csrf_token: Arc::new(Uuid::new_v4().to_string()),
            privileged_grants: Arc::new(Mutex::new(HashMap::new())),
            mutation_times: Arc::new(Mutex::new(VecDeque::new())),
            mutation_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_MUTATIONS)),
            control_times: Arc::new(Mutex::new(VecDeque::new())),
            control_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_CONTROL_ACTIONS)),
            expensive_action_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_EXPENSIVE_ACTIONS)),
            sse_limit: Arc::new(Semaphore::new(MAX_SSE_CLIENTS)),
        }
    }
}

impl LocalSecurity {
    pub(crate) fn session_payload(&self) -> serde_json::Value {
        json!({
            "csrf_token": self.csrf_token.as_str(),
            "expires": "server_session",
            "same_site": "strict"
        })
    }

    pub(crate) fn session_cookie(&self) -> String {
        format!(
            "{SESSION_COOKIE}={}; HttpOnly; SameSite=Strict; Path=/",
            self.session_id
        )
    }

    pub(crate) async fn issue_privileged_grant(
        &self,
        action: &str,
    ) -> Result<String, &'static str> {
        if !is_privileged_action_string(action) {
            return Err("the requested action is not a privileged API operation");
        }
        let token = Uuid::new_v4().to_string();
        let now = Instant::now();
        let mut grants = self.privileged_grants.lock().await;
        grants.retain(|_, grant| grant.expires_at > now);
        grants.insert(
            token.clone(),
            PrivilegedGrant {
                action: action.to_owned(),
                expires_at: now + PRIVILEGED_GRANT_LIFETIME,
            },
        );
        Ok(token)
    }

    async fn consume_privileged_grant(&self, token: &str, action: &str) -> bool {
        let now = Instant::now();
        let mut grants = self.privileged_grants.lock().await;
        grants.retain(|_, grant| grant.expires_at > now);
        grants
            .remove(token)
            .is_some_and(|grant| grant.action == action && grant.expires_at > now)
    }

    async fn mutation_rate_available(&self) -> bool {
        Self::rate_available(&self.mutation_times, MAX_MUTATIONS_PER_MINUTE).await
    }

    async fn rate_available(history: &Mutex<VecDeque<Instant>>, maximum: usize) -> bool {
        let now = Instant::now();
        let cutoff = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
        let mut times = history.lock().await;
        while times.front().is_some_and(|time| *time < cutoff) {
            times.pop_front();
        }
        if times.len() >= maximum {
            return false;
        }
        times.push_back(now);
        true
    }

    pub(crate) fn try_acquire_sse(&self) -> Option<OwnedSemaphorePermit> {
        self.sse_limit.clone().try_acquire_owned().ok()
    }
}

/// Only known, non-starting controls may use the bounded stop lane. A suffix or
/// arbitrary request body cannot turn a start/resume/delete action into a control.
fn is_execution_control(method: &Method, path: &str) -> bool {
    if method != Method::POST {
        return false;
    }
    let segments: Vec<_> = path.split('/').collect();
    let uuid = |id: &str| Uuid::parse_str(id).is_ok();
    match segments.as_slice() {
        ["", "api", "runs" | "batches", id, "cancel" | "pause"]
        | ["", "api", "agent-sessions", id, "cancel"] => uuid(id),
        [
            "",
            "api",
            "projects",
            project,
            "sample-operations",
            id,
            "cancel",
        ] => !project.is_empty() && uuid(id),
        [
            "",
            "api",
            "projects",
            project,
            "conversations",
            conversation,
            "tasks",
            task,
            "calls" | "human-requests",
            id,
            "cancel",
        ]
        | [
            "",
            "api",
            "projects",
            project,
            "conversations",
            conversation,
            "tasks",
            task,
            "calls",
            id,
            "clarification",
            "cancel",
        ] => !project.is_empty() && uuid(conversation) && uuid(task) && uuid(id),
        _ => false,
    }
}

pub(crate) fn privileged_action(method: &Method, path: &str) -> Option<String> {
    is_privileged_action(method, path).then(|| format!("{} {path}", method.as_str()))
}

fn is_privileged_action_string(action: &str) -> bool {
    let Some((method, path)) = action.split_once(' ') else {
        return false;
    };
    let Ok(method) = Method::from_bytes(method.as_bytes()) else {
        return false;
    };
    is_privileged_action(&method, path)
}

fn is_privileged_action(method: &Method, path: &str) -> bool {
    if method == Method::DELETE {
        return true;
    }
    if !is_mutation(method) {
        return false;
    }
    path == "/api/settings"
        || path.contains("/credential")
        || path.ends_with("/active-probe")
        || path == "/api/model-bundles/install"
        || path == "/api/model-bundles/import"
        || path == "/api/model-bundles/gc"
        || path == "/api/model-installations"
        || path.ends_with("/management/actions")
        || (path.starts_with("/api/model-bundles/")
            && (path.ends_with("/verify")
                || path.ends_with("/test")
                || path.ends_with("/enable")
                || path.ends_with("/disable")
                || path.ends_with("/license-acceptance")))
        || (path.starts_with("/api/model-instances/") && path.ends_with("/test"))
        || path == "/api/plugins/packages/install"
        || (path.starts_with("/api/plugins/")
            && (path.ends_with("/test")
                || path.ends_with("/enable")
                || path.ends_with("/disable")
                || path.ends_with("/weights")
                || path.ends_with("/legacy-model-bundle")))
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn is_expensive_action(path: &str) -> bool {
    path.ends_with("/active-probe")
        || path.ends_with("/schema-proposals")
        || path.ends_with("/builder-operations")
        || path.ends_with("/sample-operations")
        || path.ends_with("/discover-models")
        || path.ends_with("/sample-test")
        || path.ends_with("/dry-run")
        || path.ends_with("/suggest")
        || path.ends_with("/test")
        || path.ends_with("/runs")
        || path.ends_with("/batches")
        || path == "/api/model-installations"
}

fn loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn request_host(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
}

fn host_name(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    authority.split(':').next().unwrap_or(authority)
}

fn host_is_allowed(headers: &HeaderMap) -> bool {
    request_host(headers).is_none_or(|authority| loopback_host(host_name(authority)))
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(origin) = Url::parse(origin) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https") || !origin.host_str().is_some_and(loopback_host)
    {
        return false;
    }
    request_host(headers).is_none_or(|host| {
        origin
            .host_str()
            .is_some_and(|origin_host| origin_host == host_name(host))
            && origin.port_or_known_default()
                == host
                    .rsplit_once(':')
                    .and_then(|(_, port)| port.parse::<u16>().ok())
                    .or_else(|| (origin.scheme() == "http").then_some(80))
    })
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn json_error(status: StatusCode, code: &str, message: &str, request_id: &str) -> Response {
    let mut response = (
        status,
        Json(json!({
            "error": message,
            "code": code,
            "status": status.as_u16(),
            "request_id": request_id
        })),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response
}

pub(crate) async fn protect_local_api(
    State(security): State<LocalSecurity>,
    request: Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let path = request.uri().path().to_owned();
    if !path.starts_with("/api/") {
        return next.run(request).await;
    }
    if !host_is_allowed(request.headers()) {
        return json_error(
            StatusCode::FORBIDDEN,
            "untrusted_host",
            "the local API accepts only loopback Host values",
            &request_id,
        );
    }
    if !origin_is_allowed(request.headers()) {
        return json_error(
            StatusCode::FORBIDDEN,
            "untrusted_origin",
            "the request Origin does not match the local GUI",
            &request_id,
        );
    }
    if request.method() == Method::OPTIONS {
        return json_error(
            StatusCode::FORBIDDEN,
            "cross_origin_preflight_rejected",
            "cross-origin API preflight is not supported",
            &request_id,
        );
    }

    let mut mutation_permit = None;
    let mut expensive_permit = None;
    if is_mutation(request.method()) {
        if cookie_value(request.headers(), SESSION_COOKIE) != Some(security.session_id.as_str()) {
            return json_error(
                StatusCode::UNAUTHORIZED,
                "local_session_required",
                "initialize a local GUI session before changing workspace state",
                &request_id,
            );
        }
        if request
            .headers()
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            != Some(security.csrf_token.as_str())
        {
            return json_error(
                StatusCode::FORBIDDEN,
                "csrf_token_invalid",
                "a valid local-session CSRF token is required",
                &request_id,
            );
        }
        if request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"))
            && request
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<usize>().ok())
                .is_some_and(|length| length > MAX_JSON_BODY_BYTES)
        {
            return json_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_body_too_large",
                "JSON request body exceeds the 2 MiB local API limit",
                &request_id,
            );
        }
        let control = is_execution_control(request.method(), &path);
        let available = if control {
            LocalSecurity::rate_available(&security.control_times, MAX_CONTROL_ACTIONS_PER_MINUTE)
                .await
        } else {
            security.mutation_rate_available().await
        };
        if !available {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                if control {
                    "control_action_rate_limited"
                } else {
                    "mutation_rate_limited"
                },
                if control {
                    "too many stop or pause actions were requested in this local session; ordinary work cannot consume this reserved allowance"
                } else {
                    "too many workspace changes were requested in this local session"
                },
                &request_id,
            );
        }
        mutation_permit = if control {
            security.control_limit.clone()
        } else {
            security.mutation_limit.clone()
        }
        .try_acquire_owned()
        .ok();
        if mutation_permit.is_none() {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                if control {
                    "control_action_concurrency_limited"
                } else {
                    "mutation_concurrency_limited"
                },
                if control {
                    "too many stop or pause actions are already in progress"
                } else {
                    "too many workspace changes are already in progress"
                },
                &request_id,
            );
        }
        if is_expensive_action(&path) {
            expensive_permit = security
                .expensive_action_limit
                .clone()
                .try_acquire_owned()
                .ok();
            if expensive_permit.is_none() {
                return json_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "expensive_action_concurrency_limited",
                    "too many model, Agent, or execution actions are already in progress",
                    &request_id,
                );
            }
        }
        if let Some(action) = privileged_action(request.method(), &path) {
            let token = request
                .headers()
                .get(PRIVILEGED_CONFIRMATION_HEADER)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            if !security.consume_privileged_grant(token, &action).await {
                return json_error(
                    StatusCode::FORBIDDEN,
                    "privileged_confirmation_required",
                    "request a fresh one-time confirmation for this privileged action",
                    &request_id,
                );
            }
        }
    }

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    drop(expensive_permit);
    drop(mutation_permit);
    response
}
