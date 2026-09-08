//! Bounded detached work over the existing exporter; GET never dispatches or repairs.
use super::{
    ApiError, ApiResult, AxumPath, ExportConversation, Json, ServerState, State, Value, json,
};
use axum::{
    http::HeaderMap,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::{Stream, stream};
use std::{collections::VecDeque, convert::Infallible, time::Duration};

pub(super) async fn events(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    headers: HeaderMap,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let after = headers
        .get("last-event-id")
        .map(|value| {
            value
                .to_str()
                .ok()
                .and_then(|text| text.parse::<i64>().ok())
                .filter(|id| *id >= 0)
                .ok_or_else(|| {
                    ApiError::bad_request(anyhow::anyhow!("Invalid export event cursor"))
                })
        })
        .transpose()?;
    let (head, _) = state
        .application
        .conversation_export_events(&project, conversation, task, after.unwrap_or(0))
        .map_err(ApiError::bad_request)?;
    let permit = state.security.try_acquire_sse().ok_or_else(|| {
        ApiError::too_many_requests("the local SSE client limit has been reached")
    })?;
    let snapshot = after.is_none_or(|cursor| cursor > head);
    let cursor = if snapshot { head } else { after.unwrap_or(0) };
    let mut queue = VecDeque::new();
    if snapshot {
        queue.push_back(
            Event::default()
                .event("export_snapshot")
                .id(head.to_string())
                .data("{}"),
        );
    }
    let stream = stream::unfold(
        (
            state.application,
            project,
            conversation,
            task,
            cursor,
            queue,
            permit,
        ),
        |(application, project, conversation, task, mut cursor, mut queue, permit)| async move {
            loop {
                if let Some(event) = queue.pop_front() {
                    return Some((
                        Ok(event),
                        (
                            application,
                            project,
                            conversation,
                            task,
                            cursor,
                            queue,
                            permit,
                        ),
                    ));
                }
                let (_, events) = application
                    .conversation_export_events(&project, conversation, task, cursor)
                    .ok()?;
                for event in events {
                    cursor = event.sequence;
                    queue.push_back(
                        Event::default()
                            .event("export_changed")
                            .id(cursor.to_string())
                            .json_data(&event)
                            .ok()?,
                    );
                }
                if queue.is_empty() {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

pub(super) async fn status(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    let jobs = state.export_jobs.lock().await;
    let active = jobs.get(&id).is_some_and(|job| !job.is_finished());
    let receipt = state
        .application
        .conversation_export_status(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"job":receipt,"active":active})))
}

pub(super) async fn start(
    state: ServerState,
    project: String,
    source: ExportConversation,
    format: String,
) -> ApiResult<Json<Value>> {
    let mut jobs = state.export_jobs.lock().await;
    jobs.retain(|_, job| !job.is_finished());
    // Capacity is checked before admitting new work, not after writing a stranded receipt.
    let existing = state
        .application
        .conversation_export_status(&project, source.conversation_id, source.task_id, source.id)
        .ok();
    if existing.is_none() {
        let permit = state
            .export_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                ApiError::bad_request(anyhow::anyhow!(
                    "Export capacity is full. No export was admitted; retry later."
                ))
            })?;
        if state
            .application
            .admit_conversation_export(
                &project,
                source.conversation_id,
                source.task_id,
                source.id,
                &format,
            )
            .map_err(ApiError::bad_request)?
        {
            let application = state.application.clone();
            let worker_project = project.clone();
            let runtime = tokio::runtime::Handle::current();
            let job = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                // A panic leaves an unconfirmed durable receipt, not a fabricated success.
                let _ = runtime.block_on(application.execute_admitted_export(
                    &worker_project,
                    source.conversation_id,
                    source.task_id,
                    source.id,
                    &format,
                ));
            });
            jobs.insert(source.id, job);
        }
    } else {
        // Validate the exact immutable request even when it is already active or complete.
        state
            .application
            .admit_conversation_export(
                &project,
                source.conversation_id,
                source.task_id,
                source.id,
                &format,
            )
            .map_err(ApiError::bad_request)?;
        if !jobs.contains_key(&source.id) {
            // Explicit POST can recover verified completed files; never re-run a missing job.
            let _ = state
                .application
                .export_from_conversation(
                    &project,
                    source.conversation_id,
                    source.task_id,
                    source.id,
                    &format,
                )
                .await;
        }
    }
    let active = jobs.get(&source.id).is_some_and(|job| !job.is_finished());
    let receipt = state
        .application
        .conversation_export_status(&project, source.conversation_id, source.task_id, source.id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"job":receipt,"active":active})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse as _;
    use futures::StreamExt as _;
    #[tokio::test]
    async fn background_export_capacity_and_detached_failure_are_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let application = std::sync::Arc::new(
            annotagent_application::LocalApplication::new(temp.path()).unwrap(),
        );
        application.create_project("test-export","version: 1\nproject:\n  name: TEST export\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let owner =
            crate::stable_project_id(&temp.path().join("test-export").canonicalize().unwrap())
                .to_string();
        let conversation = application.store().create_conversation(&owner).unwrap();
        let message = annotagent_storage::ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "TEST export".into(),
            image: None,
            reference: None,
        };
        application
            .store()
            .append_conversation_message(&owner, conversation, &message)
            .unwrap();
        let task = uuid::Uuid::new_v4();
        application
            .store()
            .begin_conversation_task(
                &owner,
                conversation,
                &annotagent_storage::BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let state = ServerState::new(application.clone()).await.unwrap();
        let id = uuid::Uuid::new_v4();
        let source = || ExportConversation {
            id,
            conversation_id: conversation,
            task_id: task,
        };
        let capacity = state
            .export_workers
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        assert!(
            start(
                state.clone(),
                "test-export".into(),
                source(),
                "native".into()
            )
            .await
            .is_err()
        );
        assert!(
            application
                .conversation_export_history("test-export", conversation, task)
                .unwrap()
                .is_empty()
        );
        drop(capacity);
        let Json(admission) = start(
            state.clone(),
            "test-export".into(),
            source(),
            "native".into(),
        )
        .await
        .unwrap();
        assert_eq!(admission["job"]["id"], json!(id));
        drop(admission); // Ending response ownership does not own/cancel the worker.
        let worker = state.export_jobs.lock().await.remove(&id).unwrap();
        worker.await.unwrap();
        let Json(result) = status(
            State(state.clone()),
            AxumPath(("test-export".into(), conversation, task, id)),
        )
        .await
        .unwrap();
        assert_eq!(result["active"], false);
        assert!(
            result["job"]["error"].is_string(),
            "empty TEST Project fails with a real saved error"
        );
        assert!(result["job"]["result"].is_null());
        let Json(retry) = start(
            state.clone(),
            "test-export".into(),
            source(),
            "native".into(),
        )
        .await
        .unwrap();
        assert_eq!(retry["job"], result["job"]);
        assert!(
            state.export_jobs.lock().await.is_empty(),
            "failed operation was not dispatched again"
        );
        assert!(
            status(
                State(state.clone()),
                AxumPath(("test-export".into(), conversation, uuid::Uuid::new_v4(), id))
            )
            .await
            .is_err()
        );
        let (_, saved) = application
            .conversation_export_events("test-export", conversation, task, 0)
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "last-event-id",
            saved[0].sequence.to_string().parse().unwrap(),
        );
        let replay = events(
            State(state.clone()),
            AxumPath(("test-export".into(), conversation, task)),
            headers,
        )
        .await
        .unwrap();
        let mut body = replay.into_response().into_body().into_data_stream();
        let frame = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = std::str::from_utf8(&frame).unwrap();
        assert!(text.contains("export_changed") && text.contains("failed"));
        assert!(!text.contains("requested"));
        assert!(text.contains(&format!("id: {}", saved[1].sequence)));
        drop(body);
        let snapshot = events(
            State(state),
            AxumPath(("test-export".into(), conversation, task)),
            HeaderMap::new(),
        )
        .await
        .unwrap();
        let mut body = snapshot.into_response().into_body().into_data_stream();
        let frame = tokio::time::timeout(Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            std::str::from_utf8(&frame)
                .unwrap()
                .contains("export_snapshot")
        );
    }
}
