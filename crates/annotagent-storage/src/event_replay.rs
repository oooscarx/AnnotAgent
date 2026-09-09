//! Bounded replay over the existing persistent event sequence.
use crate::{SqliteStore, StorageError};
use annotagent_core::{RunEvent, RunId};
use rusqlite::{OptionalExtension, params};
impl SqliteStore {
    /// None means a cursor missing from this exact Run, requiring a snapshot.
    pub fn run_events_after(
        &self,
        run: RunId,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<Option<Vec<RunEvent>>, StorageError> {
        self.with_connection(|db| {
            let after = if let Some(cursor)=cursor {
                let seq:Option<i64>=db.query_row("SELECT sequence FROM run_events WHERE run_id=?1 AND event_id=?2",params![run.to_string(),cursor],|r|r.get(0)).optional()?;
                let Some(seq)=seq else {return Ok(None)};
                seq
            } else {0};
            let mut query=db.prepare("SELECT event_json FROM run_events WHERE run_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
            let events=query.query_map(params![run.to_string(),after,limit.clamp(1,100)],|r|r.get::<_,String>(0))?.map(|row| Ok(serde_json::from_str(&row?)?)).collect::<Result<Vec<_>,StorageError>>()?;
            Ok(Some(events))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{ProjectId, RunEventKind, RunEventPayload, RunStatus};
    use annotagent_runtime::{RunRecord, RuntimeStore};
    #[tokio::test]
    async fn replay_is_ordered_bounded_owned_and_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-replay.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let run = RunId::new();
        store
            .create_run(&RunRecord {
                id: run,
                project_id: ProjectId::new(),
                project_name: "TEST".into(),
                skill_id: "TEST".into(),
                provider: "mock".into(),
                model: "mock".into(),
                status: RunStatus::Pending,
                project_schema_json: "{}".into(),
                workflow_snapshot_json: None,
            })
            .await
            .unwrap();
        let mut ids = Vec::new();
        for _ in 0..3 {
            let event = RunEvent::new(
                run,
                RunEventKind::RunCreated,
                RunEventPayload::State {
                    from: None,
                    to: RunStatus::Pending,
                    reason: None,
                },
            );
            ids.push(event.event_id.to_string());
            store.record_event(&event).await.unwrap();
        }
        assert_eq!(
            store.run_events_after(run, None, 2).unwrap().unwrap().len(),
            2
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        let page = store
            .run_events_after(run, Some(&ids[1]), 2)
            .unwrap()
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].event_id.to_string(), ids[2]);
        assert!(
            store
                .run_events_after(RunId::new(), Some(&ids[1]), 2)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .run_events_after(run, Some("TEST-missing"), 2)
                .unwrap()
                .is_none()
        );
    }
}
