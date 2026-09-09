//! Run-scoped SSE uses durable sequence reads, including across process restarts.
use super::*;
pub(super) fn gap(run: RunId) -> ApiError {
    ApiError{status:StatusCode::CONFLICT,body:json!({"status":409,"code":"event_cursor_gap","error":"Event cursor is absent from this Run; reload the snapshot","suggested_action":"reload_snapshot","snapshot_url":format!("/api/runs/{run}"),"history_url":format!("/api/runs/{run}/events")})}
}
pub(super) async fn events(State(state): State<ServerState>, Query(query): Query<EventQuery>, headers: HeaderMap) -> ApiResult<Response> {
    let cursor=headers.get("last-event-id").map(|v|v.to_str().map(str::to_owned)).transpose().map_err(ApiError::bad_request)?.or(query.last_event_id);
    let Some(run)=query.run_id else {
        if cursor.is_some() {return Err(ApiError::bad_request("Reconnect requires exact run_id scope"));}
        return super::events(State(state),Query(EventQuery{run_id:None,last_event_id:None})).await.map(IntoResponse::into_response);
    };
    state.application.store().get_run_summary(run).map_err(ApiError::not_found)?;
    let permit=state.security.try_acquire_sse().ok_or_else(||ApiError::too_many_requests("SSE client limit reached"))?;
    let first=state.application.store().run_events_after(run,cursor.as_deref(),100).map_err(ApiError::internal)?.ok_or_else(||gap(run))?;
    let stream=stream::unfold((state,cursor,std::collections::VecDeque::from(first),permit,false),move |(state,mut cursor,mut pending,permit,done)| async move {
        if done {return None;}
        loop {
            if let Some(value)=pending.pop_front() {
                cursor=Some(value.event_id.to_string());
                let event=Event::default().id(value.event_id.to_string()).event(serde_json::to_value(value.kind).ok()?.as_str()?).json_data(&value).ok()?;
                return Some((Ok::<_,Infallible>(event),(state,cursor,pending,permit,false)));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            match state.application.store().run_events_after(run,cursor.as_deref(),100) {
                Ok(Some(events))=>pending.extend(events),
                _=>{
                    let event=Event::default().event("resync_required").json_data(json!({"code":"event_cursor_gap","snapshot_url":format!("/api/runs/{run}"),"history_url":format!("/api/runs/{run}/events")})).ok()?;
                    return Some((Ok::<_,Infallible>(event),(state,cursor,pending,permit,true)));
                }
            }
        }
    }).boxed();
    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive")).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{test_state,request,response_json};
    use annotagent_provider::InMemorySecretStore;
    use annotagent_runtime::RunRecord;
    #[tokio::test]
    async fn sse_reconnect_replays_exact_run_and_reports_cursor_gap() {
        let dir=tempfile::tempdir().unwrap();
        let app=Arc::new(LocalApplication::new(dir.path()).unwrap());
        let run=RunId::new();
        app.store().create_run(&RunRecord{id:run,project_id:ProjectId::new(),project_name:"TEST".into(),skill_id:"TEST".into(),provider:"mock".into(),model:"mock".into(),status:RunStatus::Pending,project_schema_json:"{}".into(),workflow_snapshot_json:None}).await.unwrap();
        let first=RunEvent::new(run,RunEventKind::RunCreated,RunEventPayload::State{from:None,to:RunStatus::Pending,reason:None});
        let second=RunEvent::new(run,RunEventKind::RunCreated,RunEventPayload::State{from:None,to:RunStatus::Pending,reason:None});
        app.store().record_event(&first).await.unwrap();
        app.store().record_event(&second).await.unwrap();
        let service=router(test_state(app,Arc::new(InMemorySecretStore::default())).await,None);
        let response=request(&service,axum::http::Method::GET,&format!("/api/events?run_id={run}&last_event_id={}",first.event_id),None).await;
        assert_eq!(response.status(),StatusCode::OK);
        let mut body=response.into_body().into_data_stream();
        let bytes=tokio::time::timeout(Duration::from_secs(2),body.next()).await.unwrap().unwrap().unwrap();
        let text=String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains(&format!("id: {}",second.event_id)));
        assert!(!text.contains(&first.event_id.to_string()));
        drop(body);
        let response=request(&service,axum::http::Method::GET,&format!("/api/events?run_id={run}&last_event_id=TEST-missing"),None).await;
        assert_eq!(response.status(),StatusCode::CONFLICT);
        assert_eq!(response_json(response).await["code"],"event_cursor_gap");
    }
}
