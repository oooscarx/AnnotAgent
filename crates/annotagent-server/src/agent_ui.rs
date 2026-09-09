//! Thin read-only UI projections. All commands use their existing routes.
use super::*;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    cursor: Option<String>,
    limit: Option<u32>,
}
pub(super) async fn navigation(State(state): State<ServerState>, Query(page): Query<Page>) -> ApiResult<Json<Value>> {
    state.application.agent_ui_navigation(page.cursor.as_deref(),page.limit.unwrap_or(50) as usize).map(Json).map_err(ApiError::bad_request)
}
pub(super) async fn tasks(State(state): State<ServerState>, AxumPath((project,conversation)): AxumPath<(String,uuid::Uuid)>, Query(page): Query<Page>) -> ApiResult<Json<Value>> {
    let after=page.cursor.as_deref().unwrap_or("0").parse().map_err(ApiError::bad_request)?;
    state.application.agent_ui_tasks(&project,conversation,after,page.limit.unwrap_or(50)).map(Json).map_err(ApiError::bad_request)
}
pub(super) async fn thread(State(state): State<ServerState>, AxumPath((project,conversation,task)): AxumPath<(String,uuid::Uuid,uuid::Uuid)>, Query(page): Query<Page>) -> ApiResult<Json<Value>> {
    let after=page.cursor.as_deref().unwrap_or("0").parse().map_err(ApiError::bad_request)?;
    state.application.agent_ui_thread(&project,conversation,task,after,page.limit.unwrap_or(50)).map(Json).map_err(ApiError::bad_request)
}
pub(super) async fn snapshot(State(state): State<ServerState>, AxumPath((project,conversation,task)): AxumPath<(String,uuid::Uuid,uuid::Uuid)>) -> ApiResult<Json<Value>> {
    state.application.agent_ui_snapshot(&project,conversation,task).map(Json).map_err(ApiError::bad_request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{test_state,request,response_json};
    use annotagent_provider::InMemorySecretStore;
    #[tokio::test]
    async fn navigation_snapshot_thread_keep_real_ownership_without_execution() {
        let dir=tempfile::tempdir().unwrap();
        let app=Arc::new(LocalApplication::new(dir.path()).unwrap());
        for route in ["TEST-a","TEST-b"] {
            app.create_project(route,include_str!("../../../examples/label-pipelines/whole-image-classification/project.yaml")).unwrap();
        }
        let c=app.create_project_conversation("TEST-a").unwrap();
        let goal=app.project_goal("TEST-a").unwrap();
        let sent=app.send_project_conversation_message("TEST-a",c,&serde_json::from_value(json!({"message":{"id":uuid::Uuid::new_v4(),"text":"TEST goal","image":null},"task_id":null,"schema_revision":goal["revision"],"mode":"plan"})).unwrap()).unwrap();
        let service=router(test_state(app,Arc::new(InMemorySecretStore::default())).await,None);
        let nav=response_json(request(&service,axum::http::Method::GET,"/api/navigation?limit=1",None).await).await;
        assert_eq!(nav["items"].as_array().unwrap().len(),1);
        assert!(nav["next_cursor"].is_string());
        assert!(!nav.to_string().contains(&dir.path().display().to_string()));
        let next=response_json(request(&service,axum::http::Method::GET,&format!("/api/navigation?limit=1&cursor={}",nav["next_cursor"].as_str().unwrap()),None).await).await;
        assert_ne!(next["items"][0]["project_owner_id"],nav["items"][0]["project_owner_id"]);
        let root=format!("/api/projects/TEST-a/conversations/{c}/tasks/{}",sent.task_id);
        let thread=response_json(request(&service,axum::http::Method::GET,&format!("{root}/thread"),None).await).await;
        assert_eq!(thread["items"][0]["role"],"user");
        let snapshot=response_json(request(&service,axum::http::Method::GET,&format!("{root}/workspace"),None).await).await;
        assert_eq!(snapshot["task"]["input"]["id"],sent.task_id.to_string());
        assert!(snapshot["budget"].is_null());
        assert_eq!(request(&service,axum::http::Method::GET,&format!("{}/workspace",root.replace("TEST-a","TEST-b")),None).await.status(),StatusCode::BAD_REQUEST);
    }
}
