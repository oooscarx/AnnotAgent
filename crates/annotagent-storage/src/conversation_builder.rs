//! Idempotent orchestration receipt; does not create another workflow executor.
use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationBuilderOperation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub request_hash: String,
    pub status: String,
    pub evidence: Option<serde_json::Value>,
}
fn owned(db: &rusqlite::Connection, project: &str, task: Uuid) -> Result<(), StorageError> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2)",params![task.to_string(),project],|row|row.get(0))?;
    if !exists {
        return Err(StorageError::InvalidConversation(
            "Builder task belongs to another Project".into(),
        ));
    }
    Ok(())
}
fn read(
    db: &rusqlite::Connection,
    id: Uuid,
) -> Result<Option<ConversationBuilderOperation>, StorageError> {
    let row: Option<(String,String,String,Option<String>)> = db.query_row("SELECT task_id,request_hash,status,evidence_json FROM conversation_builder_operations WHERE id=?1",[id.to_string()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    row.map(|(task, request_hash, status, evidence)| {
        Ok(ConversationBuilderOperation {
            id,
            task_id: Uuid::parse_str(&task)
                .map_err(|_| StorageError::InvalidConversation("Invalid task ID".into()))?,
            request_hash,
            status,
            evidence: evidence
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
        })
    })
    .transpose()
}

fn schema_pair(evidence: Option<&serde_json::Value>) -> Result<Option<(Uuid, u64)>, StorageError> {
    let Some(evidence) = evidence else {
        return Ok(None);
    };
    match (evidence.get("schema_id"), evidence.get("schema_revision")) {
        (None, None) => Ok(None),
        (Some(id), Some(revision)) => {
            let id = id.as_str().and_then(|value| Uuid::parse_str(value).ok());
            let revision = revision.as_u64();
            match (id, revision) {
                (Some(id), Some(revision)) if !id.is_nil() && revision > 0 => {
                    Ok(Some((id, revision)))
                }
                _ => Err(StorageError::InvalidConversation(
                    "Invalid Builder Schema identity".into(),
                )),
            }
        }
        _ => Err(StorageError::InvalidConversation(
            "Builder evidence requires a complete Schema identity pair".into(),
        )),
    }
}

fn merge_evidence(
    saved: Option<&serde_json::Value>,
    incoming: &serde_json::Value,
) -> Result<serde_json::Value, StorageError> {
    if let Some(source) = saved.and_then(|value| value.get("repair_source")) {
        if !incoming.is_object()
            || incoming
                .get("repair_source")
                .is_some_and(|value| value != source)
        {
            return Err(StorageError::InvalidConversation(
                "Builder settlement cannot replace its admitted repair source".into(),
            ));
        }
    }
    let original = schema_pair(saved)?;
    let proposed = schema_pair(Some(incoming))?;
    if original.is_some() && (proposed.is_some() && proposed != original || !incoming.is_object()) {
        return Err(StorageError::InvalidConversation(
            "Builder settlement cannot replace its admitted Schema identity".into(),
        ));
    }
    if let Some(incoming) = incoming.as_object() {
        let mut merged = saved
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        merged.extend(incoming.clone());
        Ok(serde_json::Value::Object(merged))
    } else {
        // Legacy unscoped operations historically accepted arbitrary evidence.
        Ok(incoming.clone())
    }
}

impl SqliteStore {
    /// Preserve the admitted source before a session/working Draft exists.
    pub fn reserve_conversation_builder_with_source(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        hash: &str,
        schema: (Uuid, u64),
        source: &serde_json::Value,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        if id.is_nil()
            || schema.0.is_nil()
            || schema.1 == 0
            || (!source.is_null()
                && (!source.is_object() || serde_json::to_vec(source)?.len() > 8192))
        {
            return Err(StorageError::InvalidConversation(
                "Invalid bounded Builder source".into(),
            ));
        }
        self.reserve_conversation_builder_scoped(
            project,
            task,
            id,
            hash,
            Some(schema),
            Some(source),
        )
    }
    /// Freeze the exact owned Schema in the same transaction that admits the Builder.
    /// Its identity is readable even before an Agent session or seed Draft exists.
    pub fn reserve_conversation_builder_with_schema(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        hash: &str,
        schema_id: Uuid,
        schema_revision: u64,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        if id.is_nil() || schema_id.is_nil() || schema_revision == 0 {
            return Err(StorageError::InvalidConversation(
                "Builder admission requires a stable operation and Schema identity".into(),
            ));
        }
        self.reserve_conversation_builder_scoped(
            project,
            task,
            id,
            hash,
            Some((schema_id, schema_revision)),
            None,
        )
    }
    /// Only `None` admits new execution. A saved receipt must never be dispatched again.
    pub fn reserve_conversation_builder(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        hash: &str,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        self.reserve_conversation_builder_scoped(project, task, id, hash, None, None)
    }
    fn reserve_conversation_builder_scoped(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        hash: &str,
        schema: Option<(Uuid, u64)>,
        source: Option<&serde_json::Value>,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StorageError::InvalidConversation(
                "Invalid Builder request hash".into(),
            ));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?; owned(&tx,project,task)?;
            if let Some(saved) = read(&tx,id)? {
                if saved.task_id != task || saved.request_hash != hash { return Err(StorageError::InvalidConversation("Builder request key conflicts".into())); }
                if schema.is_some() && schema_pair(saved.evidence.as_ref())?!=schema {return Err(StorageError::InvalidConversation("Builder request changed its admitted Schema identity".into()));}
                if let (Some(expected), Some(original)) = (source, saved.evidence.as_ref().and_then(|value|value.get("repair_source"))) {
                    if expected != original {return Err(StorageError::InvalidConversation("Builder retry changed its admitted repair source".into()));}
                }
                return Ok(Some(saved));
            }
            crate::conversation_stop::require_admission_clear(&tx,task,&id.to_string(),true)?;
            if let Some((schema_id,revision))=schema {
                let saved=crate::conversation_schema::read(&tx,project,schema_id,Some(revision))?;
                if saved.task_id!=task {return Err(StorageError::InvalidConversation("Builder Schema belongs to another task".into()));}
            }
            let collision: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE id=?1 UNION ALL SELECT 1 FROM agent_sessions WHERE id=?1 UNION ALL SELECT 1 FROM conversation_model_calls WHERE id=?1)",[id.to_string()],|row|row.get(0))?;
            if collision { return Err(StorageError::InvalidConversation("Builder operation ID is already used by another object".into())); }
            let evidence=schema.map(|(schema_id,revision)| {
                let mut evidence=serde_json::json!({"schema_id":schema_id,"schema_revision":revision});
                if let Some(source)=source {evidence["repair_source"]=source.clone();}
                evidence.to_string()
            });
            tx.execute("INSERT INTO conversation_builder_operations(id,task_id,request_hash,status,evidence_json) VALUES(?1,?2,?3,'reserved',?4)",params![id.to_string(),task.to_string(),hash,evidence])?;
            tx.commit()?; Ok(None)
        })
    }
    pub fn conversation_builder_history(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Vec<ConversationBuilderOperation>, StorageError> {
        self.with_connection(|db| {
            owned(db,project,task)?;
            let mut statement=db.prepare("SELECT id FROM conversation_builder_operations WHERE task_id=?1 ORDER BY rowid DESC LIMIT 32")?;
            let ids=statement.query_map([task.to_string()],|row|row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id|read(db,Uuid::parse_str(&id).map_err(|_|StorageError::InvalidConversation("Invalid operation ID".into()))?)?.ok_or_else(||StorageError::InvalidConversation("Operation missing".into()))).collect()
        })
    }
    pub fn conversation_builder_operation(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let saved = read(db, id)?;
            if saved.as_ref().is_some_and(|saved| saved.task_id != task) {
                return Err(StorageError::InvalidConversation(
                    "Builder belongs to another task".into(),
                ));
            }
            Ok(saved)
        })
    }
    /// Filter by the admitted source before applying the history window.
    pub fn conversation_image_class_builder_history(
        &self,
        project: &str,
        task: Uuid,
        review: Uuid,
    ) -> Result<Vec<ConversationBuilderOperation>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let mut statement = db.prepare("SELECT id FROM conversation_builder_operations WHERE task_id=?1 AND json_extract(evidence_json,'$.repair_source.kind')='image_class_review' AND json_extract(evidence_json,'$.repair_source.reference.review_id')=?2 ORDER BY rowid DESC LIMIT 32")?;
            let ids = statement.query_map(params![task.to_string(),review.to_string()], |row|row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id| read(db,Uuid::parse_str(&id).map_err(|_|StorageError::InvalidConversation("Invalid operation ID".into()))?)?.ok_or_else(||StorageError::InvalidConversation("Operation missing".into()))).collect()
        })
    }
    pub fn conversation_human_builder_history(
        &self,
        project: &str,
        task: Uuid,
        request: Uuid,
        legacy_draft: Option<&str>,
    ) -> Result<Vec<ConversationBuilderOperation>, StorageError> {
        self.with_connection(|db| {
            owned(db,project,task)?;
            let mut statement=db.prepare("SELECT b.id FROM conversation_builder_operations b LEFT JOIN agent_sessions s ON s.id=b.id WHERE b.task_id=?1 AND ((json_extract(b.evidence_json,'$.repair_source.kind')='human_request' AND json_extract(b.evidence_json,'$.repair_source.reference.request_id')=?2) OR (json_extract(b.evidence_json,'$.repair_source') IS NULL AND ?3 IS NOT NULL AND json_extract(s.session_json,'$.working_draft.draft_id')=?3 AND json_extract(s.session_json,'$.working_draft.build_mode.kind')='repair_draft')) ORDER BY b.rowid DESC LIMIT 32")?;
            let ids=statement.query_map(params![task.to_string(),request.to_string(),legacy_draft],|row|row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id|read(db,Uuid::parse_str(&id).map_err(|_|StorageError::InvalidConversation("Invalid operation ID".into()))?)?.ok_or_else(||StorageError::InvalidConversation("Operation missing".into()))).collect()
        })
    }
    pub fn settle_conversation_builder(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        completed: bool,
        evidence: &serde_json::Value,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?; owned(&tx,project,task)?;
            let Some(saved)=read(&tx,id)? else {return Ok(())};
            if saved.task_id!=task {return Err(StorageError::InvalidConversation("Builder belongs to another task".into()));}
            let evidence=merge_evidence(saved.evidence.as_ref(),evidence)?;
            if saved.status!="reserved" {return Ok(())}
            tx.execute("UPDATE conversation_builder_operations SET status=?3,evidence_json=?4 WHERE id=?1 AND task_id=?2 AND status='reserved'",params![id.to_string(),task.to_string(),if completed {"completed"} else {"interrupted"},serde_json::to_string(&evidence)?])?;
            tx.commit()?; Ok(())
        })
    }
    pub fn recover_conversation_builders(&self) -> Result<(), StorageError> {
        self.with_connection(|db| { db.execute("UPDATE conversation_builder_operations SET status='interrupted',evidence_json=json_patch(COALESCE(evidence_json,'{}'),?1) WHERE status='reserved'",[serde_json::json!({"error":"Server restarted; saved Draft and model-call receipts remain. No automatic re-execution."}).to_string()])?; Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(store: &SqliteStore) -> (String, Uuid, crate::ConversationSchemaDraft) {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST stable Builder Schema".into(),
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
        let schema=store.create_human_conversation_schema_draft(&project,task,Uuid::new_v4(),&crate::ConversationSchemaDefinition{goal:message.text,task:serde_json::from_value(json!({"id":format!("annotation_{}",task.simple()),"kind":"bounding_box","labels":["cup"],"required":true})).unwrap(),boundary_rules:vec![]}).unwrap();
        (project, task, schema)
    }

    #[test]
    fn repair_source_is_durable_before_seed_and_cannot_change_on_retry_or_settlement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-builder-source.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, task, schema) = schema(&store);
        let id = Uuid::new_v4();
        let hash = "b".repeat(64);
        let source = json!({"kind":"image_class_review","reference":{
            "review_id":Uuid::new_v4(),"draft_id":"TEST-repair","revision":2,
            "content_hash":"c".repeat(64),"schema_id":schema.id,"schema_revision":1,
            "scope_digest":"d".repeat(64),"feedback_digest":"e".repeat(64)}});
        assert!(
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &source
                )
                .unwrap()
                .is_none()
        );
        let receipt = store
            .conversation_builder_operation(&project, task, id)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status, "reserved");
        assert_eq!(receipt.evidence.as_ref().unwrap()["repair_source"], source);
        assert!(
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &source
                )
                .unwrap()
                .is_some()
        );
        for changed in [
            json!(null),
            json!({"kind":"human_request","reference":source["reference"]}),
            {
                let mut changed = source.clone();
                changed["reference"]["revision"] = json!(3);
                changed
            },
        ] {
            assert!(
                store
                    .reserve_conversation_builder_with_source(
                        &project,
                        task,
                        id,
                        &hash,
                        (schema.id, 1),
                        &changed
                    )
                    .is_err()
            );
            assert!(
                store
                    .settle_conversation_builder(
                        &project,
                        task,
                        id,
                        true,
                        &json!({"repair_source":changed})
                    )
                    .is_err()
            );
        }
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        store.recover_conversation_builders().unwrap();
        let recovered = store
            .conversation_builder_operation(&project, task, id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "interrupted");
        assert_eq!(recovered.evidence.unwrap()["repair_source"], source);
        assert!(
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &source
                )
                .unwrap()
                .is_some(),
            "Recovery must not admit another execution"
        );

        let completed = Uuid::new_v4();
        store
            .reserve_conversation_builder_with_source(
                &project,
                task,
                completed,
                &hash,
                (schema.id, 1),
                &source,
            )
            .unwrap();
        store
            .settle_conversation_builder(
                &project,
                task,
                completed,
                true,
                &json!({"draft_id":"TEST-repair"}),
            )
            .unwrap();
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, completed)
                .unwrap()
                .unwrap()
                .evidence
                .unwrap()["repair_source"],
            source
        );
    }

    #[test]
    fn exact_operation_recovery_is_not_limited_to_recent_history() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, task, schema) = schema(&store);
        let review = Uuid::new_v4();
        let source = json!({"kind":"image_class_review","reference":{"review_id":review}});
        let first = Uuid::new_v4();
        let hash = "a".repeat(64);
        store
            .reserve_conversation_builder_with_source(
                &project,
                task,
                first,
                &hash,
                (schema.id, 1),
                &source,
            )
            .unwrap();
        store
            .settle_conversation_builder(&project, task, first, true, &json!({}))
            .unwrap();
        for _ in 0..33 {
            let id = Uuid::new_v4();
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &json!(null),
                )
                .unwrap();
            store
                .settle_conversation_builder(&project, task, id, true, &json!({}))
                .unwrap();
        }
        let history = store.conversation_builder_history(&project, task).unwrap();
        assert_eq!(history.len(), 32);
        assert!(!history.iter().any(|entry| entry.id == first));
        let scoped = store
            .conversation_image_class_builder_history(&project, task, review)
            .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].id, first);
        assert!(
            store
                .conversation_image_class_builder_history(&project, task, Uuid::new_v4())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, first)
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        assert!(
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    first,
                    &hash,
                    (schema.id, 1),
                    &source
                )
                .unwrap()
                .is_some()
        );
        let (other_project, other_task, _) = self::schema(&store);
        assert!(
            store
                .conversation_builder_operation(&other_project, other_task, first)
                .is_err()
        );
    }

    #[test]
    fn human_repair_history_filters_before_limit_and_preserves_only_legacy_fallback() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, task, schema) = schema(&store);
        let request = Uuid::new_v4();
        let modern = Uuid::new_v4();
        let legacy = Uuid::new_v4();
        let conflicting = Uuid::new_v4();
        let hash = "a".repeat(64);
        for (id, source) in [
            (
                modern,
                json!({"kind":"human_request","reference":{"request_id":request}}),
            ),
            (legacy, json!(null)),
            (
                conflicting,
                json!({"kind":"image_class_review","reference":{"review_id":request}}),
            ),
        ] {
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &source,
                )
                .unwrap();
            store
                .settle_conversation_builder(&project, task, id, true, &json!({}))
                .unwrap();
            if id != modern {
                store.with_connection(|db| {
                db.execute("INSERT INTO agent_sessions(id,project_id,kind,status,session_json,created_at,updated_at) VALUES(?1,?2,'pipeline_builder','completed',?3,?4,?4)",params![id.to_string(),project,json!({"working_draft":{"draft_id":"TEST-legacy-repair","build_mode":{"kind":"repair_draft"}}}).to_string(),chrono::Utc::now().to_rfc3339()])?;Ok(())
            }).unwrap();
            }
        }
        for _ in 0..33 {
            let id = Uuid::new_v4();
            store
                .reserve_conversation_builder_with_source(
                    &project,
                    task,
                    id,
                    &hash,
                    (schema.id, 1),
                    &json!(null),
                )
                .unwrap();
            store
                .settle_conversation_builder(&project, task, id, true, &json!({}))
                .unwrap();
        }
        assert!(
            !store
                .conversation_builder_history(&project, task)
                .unwrap()
                .iter()
                .any(|item| item.id == modern || item.id == legacy)
        );
        let found = store
            .conversation_human_builder_history(&project, task, request, Some("TEST-legacy-repair"))
            .unwrap();
        assert_eq!(
            found.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![legacy, modern]
        );
        assert_eq!(
            store
                .conversation_human_builder_history(&project, task, request, None)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![modern]
        );
        assert!(
            store
                .conversation_human_builder_history(&project, task, Uuid::new_v4(), None)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_human_builder_history("TEST-foreign", task, request, None)
                .is_err()
        );
    }

    #[test]
    fn schema_identity_is_visible_before_builder_seed_and_survives_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-builder-admission.db");
        let store = SqliteStore::open(&path).unwrap();
        let (project, task, schema) = schema(&store);
        let id = Uuid::new_v4();
        let hash = "b".repeat(64);
        assert!(
            store
                .reserve_conversation_builder_with_schema(&project, task, id, &hash, schema.id, 1)
                .unwrap()
                .is_none()
        );
        let saved = store
            .conversation_builder_operation(&project, task, id)
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.evidence,
            Some(json!({"schema_id":schema.id,"schema_revision":1}))
        );
        assert_eq!(
            store.conversation_builder_history(&project, task).unwrap(),
            vec![saved.clone()]
        );
        assert!(store.get_agent_session(id).is_err());
        assert_eq!(
            store
                .reserve_conversation_builder_with_schema(&project, task, id, &hash, schema.id, 1)
                .unwrap(),
            Some(saved)
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        store.recover_conversation_builders().unwrap();
        let recovered = store
            .conversation_builder_operation(&project, task, id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "interrupted");
        assert_eq!(
            recovered.evidence.as_ref().unwrap()["schema_id"],
            schema.id.to_string()
        );
        assert_eq!(recovered.evidence.as_ref().unwrap()["schema_revision"], 1);
        assert_eq!(
            store
                .reserve_conversation_builder_with_schema(&project, task, id, &hash, schema.id, 1)
                .unwrap(),
            Some(recovered)
        );
    }

    #[test]
    fn builder_schema_retry_and_settlement_reject_identity_substitution_and_guard_keeps_pair() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, task, schema) = schema(&store);
        let id = Uuid::new_v4();
        let hash = "b".repeat(64);
        store
            .reserve_conversation_builder_with_schema(&project, task, id, &hash, schema.id, 1)
            .unwrap();
        let saved = store
            .conversation_builder_operation(&project, task, id)
            .unwrap();
        for (candidate, revision) in [
            (Uuid::new_v4(), 1),
            (schema.id, 2),
            (Uuid::nil(), 1),
            (schema.id, 0),
        ] {
            assert!(
                store
                    .reserve_conversation_builder_with_schema(
                        &project, task, id, &hash, candidate, revision
                    )
                    .is_err()
            );
        }
        for evidence in [
            json!({"schema_id":Uuid::new_v4(),"schema_revision":1}),
            json!({"schema_id":schema.id,"schema_revision":2}),
            json!({"schema_id":schema.id}),
            json!("TEST wrong shape"),
        ] {
            assert!(
                store
                    .settle_conversation_builder(&project, task, id, true, &evidence)
                    .is_err()
            );
            assert_eq!(
                store
                    .conversation_builder_operation(&project, task, id)
                    .unwrap(),
                saved
            );
        }
        store
            .settle_conversation_builder(
                &project,
                task,
                id,
                false,
                &json!({"error":"TEST guard drop"}),
            )
            .unwrap();
        let interrupted = store
            .conversation_builder_operation(&project, task, id)
            .unwrap()
            .unwrap();
        assert_eq!(interrupted.status, "interrupted");
        assert_eq!(
            interrupted.evidence,
            Some(json!({"schema_id":schema.id,"schema_revision":1,"error":"TEST guard drop"}))
        );
        store
            .settle_conversation_builder(
                &project,
                task,
                id,
                true,
                &json!({"schema_id":schema.id,"schema_revision":1,"result":"TEST too late"}),
            )
            .unwrap();
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, id)
                .unwrap(),
            Some(interrupted)
        );
        let success = Uuid::new_v4();
        store
            .reserve_conversation_builder_with_schema(&project, task, success, &hash, schema.id, 1)
            .unwrap();
        store
            .settle_conversation_builder(
                &project,
                task,
                success,
                true,
                &json!({"schema_id":schema.id,"schema_revision":1,"draft_id":"TEST draft"}),
            )
            .unwrap();
        let finished = store
            .conversation_builder_operation(&project, task, success)
            .unwrap()
            .unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(
            finished.evidence.unwrap()["schema_id"],
            schema.id.to_string()
        );
        let finished = store
            .conversation_builder_operation(&project, task, success)
            .unwrap()
            .unwrap();
        store
            .settle_conversation_builder(
                &project,
                task,
                success,
                true,
                &json!({"schema_id":schema.id,"schema_revision":1,"draft_id":"TEST draft"}),
            )
            .unwrap();
        store
            .settle_conversation_builder(
                &project,
                task,
                success,
                false,
                &json!({"error":"TEST late duplicate failure"}),
            )
            .unwrap();
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, success)
                .unwrap()
                .unwrap(),
            finished,
            "duplicate completion delivery cannot rewrite a terminal Builder receipt"
        );
    }

    #[test]
    fn builder_schema_admission_requires_owned_saved_revision_and_preserves_legacy_receipts() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, task, schema) = schema(&store);
        let (_, _, foreign) = self::schema(&store);
        let hash = "b".repeat(64);
        for (candidate, revision) in [
            (foreign.id, 1),
            (Uuid::new_v4(), 1),
            (schema.id, 2),
            (schema.id, 0),
            (Uuid::nil(), 1),
        ] {
            assert!(
                store
                    .reserve_conversation_builder_with_schema(
                        &project,
                        task,
                        Uuid::new_v4(),
                        &hash,
                        candidate,
                        revision
                    )
                    .is_err()
            );
        }
        assert!(
            store
                .conversation_builder_history(&project, task)
                .unwrap()
                .is_empty()
        );
        let legacy = Uuid::new_v4();
        store
            .reserve_conversation_builder(&project, task, legacy, &hash)
            .unwrap();
        assert!(
            store
                .reserve_conversation_builder_with_schema(
                    &project, task, legacy, &hash, schema.id, 1
                )
                .is_err()
        );
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, legacy)
                .unwrap()
                .unwrap()
                .evidence,
            None
        );
    }
    #[test]
    fn admission_and_restart_keep_one_operation_without_reexecuting_or_rewriting_result() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-builder.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = crate::ConversationMessageInput {
            reference: None,
            id: Uuid::new_v4(),
            text: "TEST target".into(),
            image: None,
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
        let hash = "b".repeat(64);
        assert!(
            store
                .reserve_conversation_builder(&project, task, id, &hash)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .reserve_conversation_builder(&project, task, id, &hash)
                .unwrap()
                .unwrap()
                .status,
            "reserved"
        );
        assert!(
            store
                .reserve_conversation_builder(&project, task, id, &"c".repeat(64))
                .is_err()
        );
        assert!(
            store
                .conversation_builder_operation("foreign", task, id)
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        store.recover_conversation_builders().unwrap();
        let interrupted = store
            .reserve_conversation_builder(&project, task, id, &hash)
            .unwrap()
            .unwrap();
        assert_eq!(interrupted.status, "interrupted");
        store
            .settle_conversation_builder(
                &project,
                task,
                id,
                true,
                &serde_json::json!({"fake":"late result"}),
            )
            .unwrap();
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, id)
                .unwrap()
                .unwrap(),
            interrupted
        );
    }
}
