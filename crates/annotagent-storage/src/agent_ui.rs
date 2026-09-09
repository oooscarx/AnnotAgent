//! Bounded projections over existing conversation identities; no new executor.
use crate::{SqliteStore, StorageError};
use rusqlite::params;
use serde_json::{Value, json};
use uuid::Uuid;

fn page(items: Vec<Value>, limit: usize) -> Value {
    let more = items.len() > limit;
    let items: Vec<_> = items.into_iter().take(limit).collect();
    let next = more.then(|| items.last().unwrap()["sequence"].clone());
    json!({"items":items,"next_cursor":next})
}
fn bounds(after: i64, limit: u32) -> Result<usize, StorageError> {
    if after < 0 || !(1..=100).contains(&limit) {
        return Err(StorageError::InvalidConversation("Invalid cursor or limit (1..100)".into()));
    }
    Ok(limit as usize)
}
impl SqliteStore {
    pub fn agent_ui_active_operations(&self, project: &str, route: &str, conversation: Uuid, task: Uuid) -> Result<Vec<crate::ConversationStopTarget>, StorageError> {
        self.with_connection(|db| {
            crate::conversation_message_queue::require_task(db,project,conversation,task)?;
            crate::conversation_stop::discover(db,route,conversation,Some(task))
        })
    }

    pub fn agent_ui_tasks(&self, project: &str, conversation: Uuid, after: i64, limit: u32) -> Result<Value, StorageError> {
        let limit = bounds(after, limit)?;
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            let mut query = db.prepare("SELECT t.id,t.source_message_id,t.schema_revision,t.created_at,m.sequence,json_extract(m.input_json,'$.text') FROM conversation_tasks t JOIN conversation_messages m ON m.conversation_id=t.conversation_id AND m.message_id=t.source_message_id WHERE t.conversation_id=?1 AND m.sequence>?2 ORDER BY m.sequence LIMIT ?3")?;
            let items = query.query_map(params![conversation.to_string(),after,(limit+1) as i64], |r| Ok(json!({
                "task_id":r.get::<_,String>(0)?,"source_message_id":r.get::<_,String>(1)?,"schema_revision":r.get::<_,String>(2)?,"created_at":r.get::<_,String>(3)?,"sequence":r.get::<_,i64>(4)?,"title":r.get::<_,String>(5)?.chars().take(160).collect::<String>(),"project_owner_id":project,"conversation_id":conversation
            })))?.collect::<Result<Vec<_>,_>>()?;
            Ok(page(items, limit))
        })
    }
    pub fn agent_ui_thread(&self, project: &str, conversation: Uuid, task: Uuid, after: i64, limit: u32) -> Result<Value, StorageError> {
        let limit = bounds(after, limit)?;
        self.with_connection(|db| {
            crate::conversation_message_queue::require_task(db,project,conversation,task)?;
            let mut query=db.prepare("SELECT m.sequence,m.input_json FROM conversation_messages m WHERE m.conversation_id=?1 AND m.sequence>?3 AND (m.message_id=(SELECT source_message_id FROM conversation_tasks WHERE id=?2 AND conversation_id=?1) OR EXISTS(SELECT 1 FROM conversation_send_receipts s WHERE s.conversation_id=m.conversation_id AND s.message_id=m.message_id AND json_extract(s.receipt_json,'$.task_id')=?2) OR json_extract(m.input_json,'$.reference.task_id')=?2) ORDER BY m.sequence LIMIT ?4")?;
            let rows=query.query_map(params![conversation.to_string(),task.to_string(),after,(limit+1) as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            let items=rows.into_iter().map(|(sequence,input)|{
                let input: Value=serde_json::from_str(&input)?;
                Ok(json!({"id":input["id"],"role":"user","source":"persisted_user_message","sequence":sequence,"task_id":task,"conversation_id":conversation,"project_owner_id":project,"message":{"conversation_id":conversation,"sequence":sequence,"input":input}}))
            }).collect::<Result<Vec<_>,StorageError>>()?;
            Ok(page(items,limit))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_navigation_and_thread_are_owned_keyset_pages() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("TEST-navigation.sqlite")).unwrap();
        let conversation = store.create_conversation("00000000-0000-4000-8000-000000000010").unwrap();
        let mut tasks = Vec::new();
        for n in 0..3 {
            let input = crate::ConversationSendInput {
                message: crate::ConversationMessageInput { id: Uuid::new_v4(), text: format!("TEST goal {n}"), image: None, reference: None },
                task_id: None, schema_revision: "a".repeat(64), agent_model: None, mode: Some(crate::ConversationSendMode::Plan),
            };
            tasks.push(store.send_conversation_message("00000000-0000-4000-8000-000000000010", conversation, &input).unwrap());
        }
        let first = store.agent_ui_tasks("00000000-0000-4000-8000-000000000010", conversation, 0, 2).unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 2);
        let cursor = first["next_cursor"].as_i64().unwrap();
        let last = store.agent_ui_tasks("00000000-0000-4000-8000-000000000010", conversation, cursor, 2).unwrap();
        assert_eq!(last["items"].as_array().unwrap().len(), 1);
        assert!(last["next_cursor"].is_null());
        assert!(store.agent_ui_tasks("TEST-foreign", conversation, 0, 2).is_err());
        let thread = store.agent_ui_thread("00000000-0000-4000-8000-000000000010", conversation, tasks[0].task_id, 0, 2).unwrap();
        assert_eq!(thread["items"].as_array().unwrap().len(), 1);
        assert_eq!(thread["items"][0]["role"], "user");
        assert_eq!(thread["items"][0]["message"]["input"]["id"], tasks[0].message.input.id.to_string());
        assert!(store.agent_ui_thread("00000000-0000-4000-8000-000000000010", conversation, Uuid::new_v4(), 0, 2).is_err());
        assert!(store.agent_ui_tasks("00000000-0000-4000-8000-000000000010", conversation, -1, 2).is_err());
    }
}
