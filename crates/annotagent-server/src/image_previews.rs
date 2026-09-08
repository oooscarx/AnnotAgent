//! Read-only, bounded browse previews. Annotation canvases retain the original content route.
use super::{
    ApiError, ApiResult, AxumPath, Body, HeaderValue, Response, ServerState, State, header,
    resolve_project_image_reference,
};

static PREVIEW_WORKERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub(super) async fn thumbnail(
    State(state): State<ServerState>,
    AxumPath((project_id, image_reference)): AxumPath<(String, String)>,
) -> ApiResult<Response> {
    let permit = PREVIEW_WORKERS
        .acquire()
        .await
        .map_err(ApiError::internal)?;
    let bytes = tokio::task::spawn_blocking(move || -> ApiResult<Vec<u8>> {
        // Keep filesystem lookup/ownership checks and decoding off the async executor.
        // The permit is held by the worker even if the HTTP client disconnects.
        let _permit = permit;
        let image_id = resolve_project_image_reference(&state, &project_id, &image_reference)?;
        let path = state
            .application
            .project_image_path(&project_id, image_id)
            .map_err(ApiError::not_found)?;
        if std::fs::metadata(&path).map_err(ApiError::internal)?.len() > 32 * 1024 * 1024 {
            return Err(ApiError::bad_request(
                "image exceeds the 32 MiB browse preview limit",
            ));
        }
        let frame =
            annotagent_image_tools::load_image(&path, 16_000_000).map_err(ApiError::bad_request)?;
        let preview =
            annotagent_image_tools::thumbnail(&frame, 256).map_err(ApiError::bad_request)?;
        annotagent_image_tools::encode_png(&preview).map_err(ApiError::internal)
    })
    .await
    .map_err(ApiError::internal)??;
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    // No stale cross-revision preview or persistent derivative files in the workspace.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}
