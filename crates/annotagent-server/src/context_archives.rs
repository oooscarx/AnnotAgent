use super::*;
use annotagent_storage::context_archives::{
    ConfirmContextImport, ContextArchive, PreviewContextImport,
};
fn archive_error(error: anyhow::Error) -> ApiError {
    if let Some(storage) = error.downcast_ref::<StorageError>() {
        match storage {
            StorageError::Management { .. } => return ApiError::management(error),
            StorageError::ConversationContract { code, .. }
                if *code == "owner_mismatch" || *code == "not_found" =>
            {
                return ApiError::not_found("Conversation not found");
            }
            _ => {}
        }
    }
    ApiError::not_found("Project or archive unavailable")
}
pub(super) async fn export(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<ContextArchive>> {
    state
        .application
        .export_context_archive(&project, conversation)
        .map(Json)
        .map_err(archive_error)
}
pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Json(input): Json<PreviewContextImport>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .preview_context_import(&project, &input.archive)
        .map(Json)
        .map_err(archive_error)
}
pub(super) async fn confirm(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Json(input): Json<ConfirmContextImport>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    state
        .application
        .confirm_context_import(&project, &input)
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(archive_error)
}
pub(super) async fn receipt(
    State(state): State<ServerState>,
    AxumPath((project, command)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .context_import_receipt(&project, command)
        .map(Json)
        .map_err(archive_error)
}
pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, id)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .archived_context(&project, id)
        .map(Json)
        .map_err(archive_error)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    #[serde(default)]
    after: i64,
    #[serde(default = "page_size")]
    limit: u32,
}
fn page_size() -> u32 {
    50
}
pub(super) async fn list(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .archived_contexts(&project, page.after, page.limit)
        .map(Json)
        .map_err(archive_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_provider::InMemorySecretStore;
    use annotagent_storage::context_archives::archive_payload_hash;
    #[tokio::test]
    async fn context_archive_http_owned_preview_confirm_recovery_and_no_dispatch() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        for project in ["TEST-archive", "TEST-foreign"] {
            app.create_project(
                project,
                include_str!(
                    "../../../examples/label-pipelines/whole-image-classification/project.yaml"
                ),
            )
            .unwrap();
        }
        let c = app.create_project_conversation("TEST-archive").unwrap();
        let message = annotagent_storage::ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "TEST actual user text".into(),
            image: None,
            reference: None,
        };
        app.append_project_conversation_message("TEST-archive", c, &message)
            .unwrap();
        let router = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let p = "/api/projects/TEST-archive";
        let get = axum::http::Method::GET;
        let post = axum::http::Method::POST;
        let archive_response = request(
            &router,
            get.clone(),
            &format!("{p}/conversations/{c}/context-archive"),
            None,
        )
        .await;
        assert_eq!(archive_response.status(), StatusCode::OK);
        let mut archive = response_json(archive_response).await;
        assert_eq!(
            archive["payload"]["records"][0]["data"]["input"]["text"],
            "TEST actual user text"
        );
        // Add fixture-only observable numeric data, hash its Rust f64 representation, then
        // submit the browser JSON.stringify representation through the real HTTP parser.
        archive["payload"]["resources"] = json!([{"kind":"TEST_numeric","id":"TEST-numeric","embedded":false,"availability":"not_verified","values":[-0.0,0.0,1.0,0.25]}]);
        let payload: annotagent_storage::context_archives::ArchivePayload =
            serde_json::from_value(archive["payload"].clone()).unwrap();
        archive["archive_hash"] = json!(archive_payload_hash(&payload).unwrap());
        archive["payload"]["resources"][0]["values"] = json!([0, 0, 1, 0.25]);
        let wrong = request(
            &router,
            get.clone(),
            &format!("/api/projects/TEST-foreign/conversations/{c}/context-archive"),
            None,
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::NOT_FOUND);
        let preview_response = request(
            &router,
            post.clone(),
            &format!("{p}/context-imports/preview"),
            Some(json!({"archive":archive})),
        )
        .await;
        let preview_status = preview_response.status();
        let preview = response_json(preview_response).await;
        assert_eq!(preview_status, StatusCode::OK, "{preview}");
        assert_eq!(preview["continuation"]["can_resume"], false);
        let list = response_json(
            request(
                &router,
                get.clone(),
                &format!("{p}/archived-contexts"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(list["items"], json!([]));
        let command = uuid::Uuid::new_v4();
        let body = json!({"command_id":command,"preview_hash":preview["preview_hash"],"archive":archive,"confirm_archive_only":true});
        let missing = request(
            &router,
            get.clone(),
            &format!("{p}/context-imports/{command}"),
            None,
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let first = request(
            &router,
            post.clone(),
            &format!("{p}/context-imports"),
            Some(body.clone()),
        )
        .await;
        assert_eq!(first.status(), StatusCode::CREATED);
        let first = response_json(first).await;
        let second = response_json(
            request(
                &router,
                post.clone(),
                &format!("{p}/context-imports"),
                Some(body.clone()),
            )
            .await,
        )
        .await;
        assert_eq!(first, second);
        let recovered = response_json(
            request(
                &router,
                get.clone(),
                &format!("{p}/context-imports/{command}"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(first, recovered);
        let context = first["context_id"].as_str().unwrap();
        let loaded = response_json(
            request(
                &router,
                get.clone(),
                &format!("{p}/archived-contexts/{context}"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(loaded["archive"], archive);
        println!(
            "UIAPI018_EXAMPLE={}",
            json!({"archive":archive,"preview":preview,"receipt":first,"loaded":loaded})
        );
        assert_eq!(loaded["trust"], "untrusted_import");
        assert_eq!(
            request(
                &router,
                get.clone(),
                &format!("/api/projects/TEST-foreign/archived-contexts/{context}"),
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let mut stale = body.clone();
        stale["preview_hash"] = json!("stale");
        let response = request(
            &router,
            post.clone(),
            &format!("{p}/context-imports"),
            Some(stale),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(response).await["code"],
            "archive_preview_conflict"
        );
        let mut unknown = body;
        unknown["run"] = json!(true);
        assert_eq!(
            request(
                &router,
                post,
                &format!("{p}/context-imports"),
                Some(unknown)
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        // Active journal is untouched; imported context IDs are not live conversations.
        assert_eq!(app.create_project_conversation("TEST-archive").unwrap(), c);
        assert_eq!(
            app.project_conversation_messages("TEST-archive", c, 0, 100)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            request(
                &router,
                get,
                &format!("{p}/conversations/{context}/messages"),
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }

    async fn wire_request(
        address: std::net::SocketAddr,
        method: &str,
        path: &str,
        extra: &str,
        body: Option<Value>,
    ) -> (u16, String, Value) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let body = body.map(|v| v.to_string()).unwrap_or_default();
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let (headers, body) = text.split_once("\r\n\r\n").unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, headers.into(), serde_json::from_str(body).unwrap())
    }
    #[tokio::test]
    async fn context_archive_loopback_http_session_and_exact_import() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        app.create_project(
            "TEST-loopback",
            include_str!(
                "../../../examples/label-pipelines/whole-image-classification/project.yaml"
            ),
        )
        .unwrap();
        let c = app.create_project_conversation("TEST-loopback").unwrap();
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, service).await.unwrap();
        });
        let p = "/api/projects/TEST-loopback";
        let (status, _, a) = wire_request(
            address,
            "GET",
            &format!("{p}/conversations/{c}/context-archive"),
            "",
            None,
        )
        .await;
        assert_eq!(status, 200);
        let (status, headers, session) =
            wire_request(address, "GET", "/api/session", "", None).await;
        assert_eq!(status, 200);
        let cookie = headers
            .lines()
            .find_map(|l| l.strip_prefix("set-cookie: "))
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let extra = format!(
            "Cookie: {cookie}\r\n{}: {}\r\n",
            security::CSRF_HEADER,
            session["csrf_token"].as_str().unwrap()
        );
        let (status, _, _) = wire_request(
            address,
            "POST",
            &format!("{p}/context-imports/preview"),
            "",
            Some(json!({"archive":a})),
        )
        .await;
        assert_eq!(status, 401);
        let (status, _, preview) = wire_request(
            address,
            "POST",
            &format!("{p}/context-imports/preview"),
            &extra,
            Some(json!({"archive":a})),
        )
        .await;
        assert_eq!(status, 200);
        let command = uuid::Uuid::new_v4();
        let body = json!({"command_id":command,"preview_hash":preview["preview_hash"],"archive":a,"confirm_archive_only":true});
        let (status, _, first) = wire_request(
            address,
            "POST",
            &format!("{p}/context-imports"),
            &extra,
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, 201);
        let (status, _, second) = wire_request(
            address,
            "POST",
            &format!("{p}/context-imports"),
            &extra,
            Some(body),
        )
        .await;
        assert_eq!(status, 201);
        assert_eq!(first, second);
        let (status, _, receipt) = wire_request(
            address,
            "GET",
            &format!("{p}/context-imports/{command}"),
            "",
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(first, receipt);
        server.abort();
        let _ = server.await;
    }
}
