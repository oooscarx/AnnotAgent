use crate::{SqliteStore, StorageError};
use annotagent_core::{PublishedWorkflowVersion, WorkflowDraft};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPublicationCommand {
    pub command_id: Uuid,
    pub project_id: String,
    pub draft_id: String,
    pub expected_revision: u64,
    pub expected_content_hash: String,
}
impl WorkflowPublicationCommand {
    pub fn check_draft(&self, draft: &WorkflowDraft) -> Result<(), StorageError> {
        if self.project_id != draft.project_id || self.draft_id != draft.id {
            return Err(error(
                "foreign_project_object",
                "Workflow Draft was not found in this Project",
            ));
        }
        if draft.status == annotagent_core::WorkflowDraftStatus::Published {
            return Err(error(
                "workflow_already_published",
                "Draft is already published; recover the original command or view its frozen version",
            ));
        }
        if self.expected_revision != draft.revision {
            return Err(StorageError::WorkflowDraftRevisionConflict {
                expected: self.expected_revision,
                current: draft.revision,
            });
        }
        if self.expected_content_hash.is_empty() || self.expected_content_hash != draft.content_hash
        {
            return Err(error(
                "workflow_draft_content_conflict",
                "Draft content differs from the confirmed hash; reload before publishing",
            ));
        }
        Ok(())
    }
}
fn error(code: &str, message: &str) -> StorageError {
    StorageError::Management {
        code: code.into(),
        message: message.into(),
    }
}
pub(crate) fn replay(
    db: &Connection,
    command: &WorkflowPublicationCommand,
) -> Result<Option<PublishedWorkflowVersion>, StorageError> {
    let stored: Option<(String,String)> = db.query_row("SELECT request_json,result_json FROM workflow_publication_commands WHERE command_id=?1", [command.command_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    stored
        .map(|(request, result)| {
            if serde_json::from_str::<WorkflowPublicationCommand>(&request)? != *command {
                return Err(error(
                    "workflow_publication_command_conflict",
                    "Publication command was already used with a different scope",
                ));
            }
            serde_json::from_str(&result).map_err(Into::into)
        })
        .transpose()
}
impl SqliteStore {
    pub fn workflow_publication_result(
        &self,
        command: &WorkflowPublicationCommand,
    ) -> Result<Option<PublishedWorkflowVersion>, StorageError> {
        self.with_connection(|db| replay(db, command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::WorkflowSnapshot;
    use serde_json::json;
    fn draft(store: &SqliteStore) -> WorkflowDraft {
        let draft: WorkflowDraft = serde_json::from_value(json!({"id":"TEST-publish","project_id":"TEST-owner","name":"TEST frozen","status":"editing","nodes":[],"edges":[],"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now()})).unwrap();
        store.save_workflow_draft(&draft).unwrap();
        store.get_workflow_draft(&draft.id).unwrap()
    }
    fn command(d: &WorkflowDraft) -> WorkflowPublicationCommand {
        WorkflowPublicationCommand {
            command_id: Uuid::new_v4(),
            project_id: d.project_id.clone(),
            draft_id: d.id.clone(),
            expected_revision: d.revision,
            expected_content_hash: d.content_hash.clone(),
        }
    }
    fn publish(
        s: &SqliteStore,
        d: &WorkflowDraft,
        c: &WorkflowPublicationCommand,
    ) -> Result<PublishedWorkflowVersion, StorageError> {
        s.publish_workflow_draft_command(
            d,
            "TEST-frozen-hash".into(),
            WorkflowSnapshot {
                draft: Some(d.clone()),
                ..WorkflowSnapshot::default()
            },
            c,
        )
    }
    #[test]
    fn publication_command_atomic_revision_hash_owner_and_replay_after_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let s = SqliteStore::open(&path).unwrap();
        let d = draft(&s);
        let c = command(&d);
        let mut bad = c.clone();
        bad.expected_content_hash = "wrong".into();
        assert!(publish(&s, &d, &bad).is_err());
        bad = c.clone();
        bad.project_id = "TEST-other".into();
        assert!(publish(&s, &d, &bad).is_err());
        assert!(s.list_published_workflow_versions(None).unwrap().is_empty());
        let mut changed = d.clone();
        changed.name = "TEST newer".into();
        let changed = s
            .save_workflow_draft_if_revision(&changed, d.revision)
            .unwrap()
            .unwrap();
        assert!(matches!(
            publish(&s, &d, &c),
            Err(StorageError::WorkflowDraftRevisionConflict { .. })
        ));
        assert!(s.workflow_publication_result(&c).unwrap().is_none());
        let c = command(&changed);
        let first = publish(&s, &changed, &c).unwrap();
        drop(s);
        let s = SqliteStore::open(&path).unwrap();
        assert_eq!(publish(&s, &changed, &c).unwrap(), first);
        bad = c.clone();
        bad.expected_revision += 1;
        assert!(
            matches!(s.workflow_publication_result(&bad),Err(StorageError::Management{code,..}) if code=="workflow_publication_command_conflict")
        );
        assert!(
            matches!(publish(&s,&changed,&command(&changed)),Err(StorageError::Management{code,..}) if code=="workflow_already_published")
        );
        assert_eq!(
            s.list_published_workflow_versions(None).unwrap(),
            vec![first]
        );
    }
    #[test]
    fn publication_command_receipt_failure_rolls_back_version_and_default() {
        let store = SqliteStore::open_in_memory().unwrap();
        let draft = draft(&store);
        let command = command(&draft);
        store.with_connection(|db| {db.execute_batch("CREATE TRIGGER TEST_receipt_failure BEFORE INSERT ON workflow_publication_commands BEGIN SELECT RAISE(ABORT,'TEST receipt failure'); END;")?;Ok(())}).unwrap();
        assert!(publish(&store, &draft, &command).is_err());
        assert!(
            store
                .list_published_workflow_versions(None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.get_workflow_draft(&draft.id).unwrap(), draft);
        assert!(
            store
                .workflow_publication_result(&command)
                .unwrap()
                .is_none()
        );
        store
            .with_connection(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM project_workflow_defaults", [], |r| {
                        r.get::<_, u32>(0)
                    })?,
                    0
                );
                db.execute_batch("DROP TRIGGER TEST_receipt_failure")?;
                Ok(())
            })
            .unwrap();
        assert!(publish(&store, &draft, &command).is_ok());
    }

    #[test]
    fn publication_command_two_connections_publish_once() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let s = SqliteStore::open(&path).unwrap();
        let d = draft(&s);
        let c = command(&d);
        drop(s);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let s = SqliteStore::open(&path).unwrap();
                let d = d.clone();
                let c = c.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    publish(&s, &d, &c).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results[0], results[1]);
        assert_eq!(
            SqliteStore::open(&path)
                .unwrap()
                .list_published_workflow_versions(None)
                .unwrap()
                .len(),
            1
        );
    }
}
