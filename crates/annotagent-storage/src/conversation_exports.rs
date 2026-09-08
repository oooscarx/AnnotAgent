//! Links the existing project exporter to its requesting task, not a second exporter.
use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationExport {
    pub id: Uuid,
    pub format: String,
    pub created_at: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationExportEvent {
    pub sequence: i64,
    pub export_id: Uuid,
    pub kind: String,
}

fn export_event(db: &rusqlite::Connection, id: Uuid, kind: &str) -> Result<(), StorageError> {
    db.execute("INSERT INTO conversation_export_events(project_id,conversation_id,task_id,export_id,kind) SELECT project_id,conversation_id,task_id,id,?2 FROM conversation_exports WHERE id=?1",params![id.to_string(),kind])?;
    Ok(())
}

impl SqliteStore {
    pub fn conversation_export(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationExport, StorageError> {
        self.with_connection(|db|{
            let row:Option<(String,String,Option<String>,Option<String>)>=db.query_row("SELECT e.format,e.created_at,e.result_json,e.error FROM conversation_exports e JOIN conversation_tasks t ON t.id=e.task_id JOIN project_conversations c ON c.id=t.conversation_id WHERE e.id=?1 AND e.project_id=?2 AND e.conversation_id=?3 AND e.task_id=?4 AND c.id=e.conversation_id AND c.project_id=e.project_id",params![id.to_string(),project,conversation.to_string(),task.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            let(format,created_at,result,error)=row.ok_or_else(||StorageError::InvalidConversation("Export is unavailable in this task".into()))?;
            Ok(ConversationExport{id,format,created_at,result:result.map(|value|serde_json::from_str(&value)).transpose()?,error})
        })
    }
    pub fn completed_conversation_export(
        &self,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.with_connection(|db| {
            let result: Option<Option<String>> = db
                .query_row(
                    "SELECT result_json FROM conversation_exports WHERE id=?1",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            result
                .flatten()
                .map(|value| serde_json::from_str(&value).map_err(Into::into))
                .transpose()
        })
    }
    pub fn begin_conversation_export(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        format: &str,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            let owned:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE c.project_id=?1 AND c.id=?2 AND t.id=?3)",params![project,conversation.to_string(),task.to_string()],|row|row.get(0))?;
            if !owned {return Err(StorageError::InvalidConversation("Export task does not belong to this Project and conversation".into()));}
            let existing:Option<(String,String,String,String)>=tx.query_row("SELECT project_id,conversation_id,task_id,format FROM conversation_exports WHERE id=?1",[id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
            if let Some(existing)=existing {
                if existing!=(project.into(),conversation.to_string(),task.to_string(),format.into()) {return Err(StorageError::InvalidConversation("Export operation ID conflicts with its saved scope".into()));}
                return Ok(false);
            }
            tx.execute("INSERT INTO conversation_exports(id,project_id,conversation_id,task_id,format,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id.to_string(),project,conversation.to_string(),task.to_string(),format,chrono::Utc::now().to_rfc3339()])?;
            export_event(&tx,id,"requested")?;
            tx.commit()?;
            Ok(true)
        })
    }
    pub fn finish_conversation_export(
        &self,
        id: Uuid,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        if result.is_some() == error.is_some() {
            return Err(StorageError::InvalidConversation(
                "Export completion requires exactly one result or error".into(),
            ));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            let changed=tx.execute("UPDATE conversation_exports SET result_json=?2,error=?3 WHERE id=?1 AND result_json IS NULL AND error IS NULL",params![id.to_string(),result.map(serde_json::to_string).transpose()?,error])?;
            if changed==1{export_event(&tx,id,if result.is_some(){"completed"}else{"failed"})?;}
            tx.commit()?;Ok(())
        })
    }
    pub fn conversation_export_events(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        after: i64,
    ) -> Result<(i64, Vec<ConversationExportEvent>), StorageError> {
        if after < 0 {
            return Err(StorageError::InvalidConversation(
                "Export event cursor must be nonnegative".into(),
            ));
        }
        self.with_connection(|db|{
            let owned:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE c.project_id=?1 AND c.id=?2 AND t.id=?3)",params![project,conversation.to_string(),task.to_string()],|row|row.get(0))?;
            if !owned{return Err(StorageError::InvalidConversation("Export task is unavailable".into()));}
            let head=db.query_row("SELECT COALESCE(MAX(sequence),0) FROM conversation_export_events WHERE project_id=?1 AND conversation_id=?2 AND task_id=?3",params![project,conversation.to_string(),task.to_string()],|row|row.get(0))?;
            let mut query=db.prepare("SELECT sequence,export_id,kind FROM conversation_export_events WHERE project_id=?1 AND conversation_id=?2 AND task_id=?3 AND sequence>?4 ORDER BY sequence LIMIT 100")?;
            let rows=query.query_map(params![project,conversation.to_string(),task.to_string(),after],|row|Ok((row.get(0)?,row.get::<_,String>(1)?,row.get(2)?)))?;
            let events=rows.map(|row|{let(sequence,id,kind)=row?;Ok(ConversationExportEvent{sequence,export_id:Uuid::parse_str(&id).map_err(|_|StorageError::InvalidConversation("Invalid export event ID".into()))?,kind})}).collect::<Result<Vec<_>,StorageError>>()?;
            Ok((head,events))
        })
    }
    pub fn conversation_exports(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<ConversationExport>, StorageError> {
        self.conversation_exports_page(project, conversation, task, None, 100)
    }
    pub fn conversation_exports_page(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<ConversationExport>, StorageError> {
        self.with_connection(|db| {
            let owned:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE c.project_id=?1 AND c.id=?2 AND t.id=?3)",params![project,conversation.to_string(),task.to_string()],|row|row.get(0))?;
            if !owned {return Err(StorageError::InvalidConversation("Export task is unavailable in this Project".into()));}
            let cursor=before.map(|id|id.to_string());
            let timestamp:Option<String>=if let Some(id)=&cursor {Some(db.query_row("SELECT created_at FROM conversation_exports WHERE id=?1 AND project_id=?2 AND conversation_id=?3 AND task_id=?4",params![id,project,conversation.to_string(),task.to_string()],|row|row.get(0)).optional()?.ok_or_else(||StorageError::InvalidConversation("Export cursor is unavailable in this task".into()))?)}else{None};
            let mut query=db.prepare("SELECT id,format,created_at,result_json,error FROM conversation_exports WHERE project_id=?1 AND conversation_id=?2 AND task_id=?3 AND (?4 IS NULL OR (created_at,id)<(?5,?4)) ORDER BY created_at DESC,id DESC LIMIT ?6")?;
            let rows=query.query_map(params![project,conversation.to_string(),task.to_string(),cursor,timestamp,limit.clamp(1,100)],|row|Ok((row.get::<_,String>(0)?,row.get(1)?,row.get(2)?,row.get::<_,Option<String>>(3)?,row.get(4)?)))?;
            rows.map(|row|{let(id,format,created_at,result,error)=row?;Ok(ConversationExport{id:Uuid::parse_str(&id).map_err(|_|StorageError::InvalidConversation("Invalid export ID".into()))?,format,created_at,result:result.map(|value|serde_json::from_str(&value)).transpose()?,error})}).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversation_export_receipts_are_owned_idempotent_and_durable() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-exports.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST export".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &crate::BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let id = Uuid::new_v4();
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER fail_export_admission_event BEFORE INSERT ON conversation_export_events WHEN NEW.kind='requested' BEGIN SELECT RAISE(ABORT,'TEST admission event unavailable'); END;")?;Ok(())}).unwrap();
        assert!(
            store
                .begin_conversation_export(&project, conversation, task, id, "native")
                .is_err()
        );
        assert!(
            store
                .conversation_export(&project, conversation, task, id)
                .is_err(),
            "admission rolls back with its event"
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_export_admission_event;")?;
                Ok(())
            })
            .unwrap();
        assert!(
            store
                .begin_conversation_export(&project, conversation, task, id, "native")
                .unwrap()
        );
        assert!(
            !store
                .begin_conversation_export(&project, conversation, task, id, "native")
                .unwrap()
        );
        assert!(
            store
                .begin_conversation_export(&project, conversation, task, id, "coco")
                .is_err()
        );
        assert!(
            store
                .begin_conversation_export("foreign", conversation, task, Uuid::new_v4(), "native")
                .is_err()
        );
        assert!(
            store
                .conversation_exports(&project, conversation, Uuid::new_v4())
                .is_err()
        );
        let result = serde_json::json!({"TEST":"immutable result"});
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER fail_export_event BEFORE INSERT ON conversation_export_events WHEN NEW.kind='completed' BEGIN SELECT RAISE(ABORT,'TEST event unavailable'); END;")?;Ok(())}).unwrap();
        assert!(
            store
                .finish_conversation_export(id, Some(&result), None)
                .is_err()
        );
        assert!(
            store.completed_conversation_export(id).unwrap().is_none(),
            "completion rolls back with its event"
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_export_event;")?;
                Ok(())
            })
            .unwrap();
        store
            .finish_conversation_export(id, Some(&result), None)
            .unwrap();
        store
            .finish_conversation_export(id, None, Some("late failure"))
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store.completed_conversation_export(id).unwrap(),
            Some(result)
        );
        let history = store
            .conversation_exports(&project, conversation, task)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].error.is_none());
        let (head, events) = store
            .conversation_export_events(&project, conversation, task, 0)
            .unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["requested", "completed"]
        );
        assert_eq!(events[1].sequence, head);
        assert_eq!(
            store
                .conversation_export_events(&project, conversation, task, events[0].sequence)
                .unwrap()
                .1
                .len(),
            1,
            "reconnect replays only events after the cursor"
        );
        assert!(
            store
                .conversation_export_events(&project, conversation, Uuid::new_v4(), 0)
                .is_err()
        );
        for _ in 0..124 {
            store
                .begin_conversation_export(&project, conversation, task, Uuid::new_v4(), "native")
                .unwrap();
        }
        // Equal timestamps must still produce stable pages via the UUID tie-breaker.
        store.with_connection(|db|{db.execute("UPDATE conversation_exports SET created_at='2000-01-01T00:00:00Z' WHERE task_id=?1",[task.to_string()])?;Ok(())}).unwrap();
        let first = store
            .conversation_exports_page(&project, conversation, task, None, 17)
            .unwrap();
        assert_eq!(first.len(), 17);
        let newest = Uuid::new_v4();
        store
            .begin_conversation_export(&project, conversation, task, newest, "native")
            .unwrap();
        let mut ids = std::collections::BTreeSet::new();
        let mut page = first;
        while let Some(last) = page.last().map(|row| row.id) {
            for row in &page {
                assert!(ids.insert(row.id), "no duplicate across pages");
            }
            page = store
                .conversation_exports_page(&project, conversation, task, Some(last), 17)
                .unwrap();
        }
        assert_eq!(
            ids.len(),
            125,
            "older history remains reachable beyond the original 100 limit"
        );
        assert!(
            !ids.contains(&newest),
            "new inserts do not shift an existing cursor"
        );
        assert!(
            store
                .conversation_exports_page(&project, conversation, task, Some(Uuid::new_v4()), 17)
                .is_err()
        );
        let other_message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            ..message
        };
        store
            .append_conversation_message(&project, conversation, &other_message)
            .unwrap();
        let other = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &crate::BeginConversationTask {
                    id: other,
                    source_message_id: other_message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        assert!(
            store
                .conversation_exports_page(&project, conversation, other, Some(id), 17)
                .is_err(),
            "cursor cannot cross tasks"
        );
    }
}
