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
    #[test]
    fn clone_command_owner_hash_replay_restart_and_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let d = draft(&store);
        let frozen = publish(&store, &d, &command(&d)).unwrap();
        let default_before: (String,i64,String) = store.with_connection(|db| Ok(db.query_row("SELECT workflow_id,version,updated_at FROM project_workflow_defaults WHERE project_id=?1", [&d.project_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)).unwrap();
        let c = crate::WorkflowCloneCommand {
            command_id: Uuid::new_v4(),
            project_id: d.project_id.clone(),
            workflow_id: frozen.workflow_id.clone(),
            version: frozen.version,
            source_snapshot_hash: frozen.content_hash.clone(),
        };
        let mut wrong = c.clone();
        wrong.project_id = "TEST-other".into();
        assert!(store.clone_workflow_version_command(&wrong).is_err());
        wrong = c.clone();
        wrong.source_snapshot_hash = "wrong".into();
        assert!(store.clone_workflow_version_command(&wrong).is_err());
        let copy = store.clone_workflow_version_command(&c).unwrap();
        assert_eq!(copy.revision, 1);
        let mut edited = copy.clone();
        edited.name = "TEST human edit".into();
        store.save_workflow_draft(&edited).unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            serde_json::to_value(store.clone_workflow_version_command(&c).unwrap()).unwrap(),
            serde_json::to_value(&copy).unwrap()
        );
        assert_eq!(
            store.get_workflow_draft(&copy.id).unwrap().name,
            "TEST human edit"
        );
        let default_after: (String,i64,String) = store.with_connection(|db| Ok(db.query_row("SELECT workflow_id,version,updated_at FROM project_workflow_defaults WHERE project_id=?1", [&d.project_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)).unwrap();
        assert_eq!(default_before, default_after);
        wrong = c.clone();
        wrong.version += 1;
        assert!(store.clone_workflow_version_command(&wrong).is_err());
        assert_eq!(
            serde_json::to_value(
                store
                    .get_published_workflow_version(&frozen.workflow_id, frozen.version)
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(frozen).unwrap()
        );
    }

    #[test]
    fn clone_command_concurrent_writers_and_receipt_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let d = draft(&store);
        let frozen = publish(&store, &d, &command(&d)).unwrap();
        let c = crate::WorkflowCloneCommand {
            command_id: Uuid::new_v4(),
            project_id: d.project_id.clone(),
            workflow_id: frozen.workflow_id,
            version: frozen.version,
            source_snapshot_hash: frozen.content_hash,
        };
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_clone_receipt_failure BEFORE INSERT ON workflow_clone_commands BEGIN SELECT RAISE(ABORT, 'TEST rollback'); END;")?;Ok(())}).unwrap();
        assert!(store.clone_workflow_version_command(&c).is_err());
        store
            .with_connection(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM workflow_drafts", [], |r| r
                        .get::<_, i64>(0))?,
                    1
                );
                db.execute_batch("DROP TRIGGER TEST_clone_receipt_failure")?;
                Ok(())
            })
            .unwrap();
        let other = SqliteStore::open(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b = barrier.clone();
        let second = c.clone();
        let thread = std::thread::spawn(move || {
            b.wait();
            other.clone_workflow_version_command(&second).unwrap()
        });
        barrier.wait();
        let first = store.clone_workflow_version_command(&c).unwrap();
        let second = thread.join().unwrap();
        assert_eq!(first.id, second.id);
        store
            .with_connection(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM workflow_drafts", [], |r| r
                        .get::<_, i64>(0))?,
                    2
                );
                Ok(())
            })
            .unwrap();
    }
}
