//! Versioned, inert evidence archives. Never insert imported rows into execution tables.
use crate::{SqliteStore, StorageError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const MAX_BYTES: usize = 1_048_576;
const MAX_RECORDS: usize = 10_000;
const MAX_SAFE_JSON_INTEGER: f64 = 9_007_199_254_740_991.0;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextArchive {
    pub format: String,
    pub version: u32,
    pub payload: ArchivePayload,
    pub archive_hash: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivePayload {
    pub source: ArchiveSource,
    pub captured_at: String,
    pub records: Vec<ArchiveRecord>,
    pub resources: Vec<Value>,
    pub integrity: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveSource {
    pub project_id: String,
    pub project_owner_id: String,
    pub conversation_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveRecord {
    pub kind: String,
    pub id: String,
    pub data: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewContextImport {
    pub archive: ContextArchive,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmContextImport {
    pub command_id: Uuid,
    pub preview_hash: String,
    pub archive: ContextArchive,
    pub confirm_archive_only: bool,
}
fn error(code: &str, message: &str) -> StorageError {
    StorageError::Management {
        code: code.into(),
        message: message.into(),
    }
}
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(o) => {
            let sorted: BTreeMap<_, _> = o.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
            serde_json::to_value(sorted).expect("JSON map")
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        Value::Number(number) => {
            // Browsers parse every JSON number as an IEEE-754 Number. JSON.stringify therefore
            // writes 0.0, 1.0, and -0.0 as 0, 1, and 0. Treat those spellings as the same
            // semantic value before hashing so an exported archive survives a browser
            // parse/stringify round trip. Existing integer values are already canonical.
            if number.as_i64().is_some() || number.as_u64().is_some() {
                return value.clone();
            }
            let Some(float) = number.as_f64() else {
                return value.clone();
            };
            if float == 0.0 {
                return Value::Number(0.into());
            }
            if !float.is_finite() || float.fract() != 0.0 {
                return value.clone();
            }
            // Stay inside the common exact integer domain of serde_json and JavaScript Number.
            // Outside this range a browser may already have lost integer identity while parsing.
            if (-MAX_SAFE_JSON_INTEGER..=MAX_SAFE_JSON_INTEGER).contains(&float) {
                return Value::Number((float as i64).into());
            }
            value.clone()
        }
        other => other.clone(),
    }
}

fn legacy_canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object
                .iter()
                .map(|(key, value)| (key.clone(), legacy_canonical(value)))
                .collect();
            serde_json::to_value(sorted).expect("JSON map")
        }
        Value::Array(items) => Value::Array(items.iter().map(legacy_canonical).collect()),
        other => other.clone(),
    }
}

fn hash(value: &impl Serialize) -> Result<String, StorageError> {
    Ok(annotagent_image_tools::sha256(&serde_json::to_vec(
        &canonical(&serde_json::to_value(value)?),
    )?))
}

fn legacy_hash(value: &impl Serialize) -> Result<String, StorageError> {
    Ok(annotagent_image_tools::sha256(&serde_json::to_vec(
        &legacy_canonical(&serde_json::to_value(value)?),
    )?))
}

/// Integrity digest used by the version-1 archive envelope.
pub fn archive_payload_hash(payload: &ArchivePayload) -> Result<String, StorageError> {
    hash(payload)
}
fn continuation() -> Value {
    json!({"can_resume":false,"available_actions":["view","export"],"reasons":["imported_history_is_inert","new_live_task_and_fresh_authorization_required"]})
}
// No Provider/config/grant tables are exported. Scrub nested persisted tool payloads too.
fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    [
        "secret",
        "password",
        "credential",
        "api_key",
        "apikey",
        "apitoken",
        "api_token",
        "accesstoken",
        "refreshtoken",
        "privatekey",
        "base64",
        "authorization",
        "approval_token",
        "access_token",
        "refresh_token",
        "private_key",
        "cookie",
        "headers",
        "reasoning_content",
        "chain_of_thought",
        "thinking",
        "local_path",
        "absolute_path",
    ]
    .iter()
    .any(|s| key.contains(s))
        || matches!(
            key.as_str(),
            "grant"
                | "grant_id"
                | "grant_token"
                | "reasoning"
                | "path"
                | "file_path"
                | "image_path"
        )
}
fn scrub(value: &mut Value, count: &mut usize) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if sensitive_key(key) {
                    if *value != json!("[REDACTED]") {
                        *count += 1;
                        *value = json!("[REDACTED]");
                    }
                } else {
                    scrub(value, count);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, count);
            }
        }
        Value::String(s) => {
            let lower = s.to_ascii_lowercase();
            if lower.contains("bearer ")
                || s.split_whitespace().any(|word| word.starts_with("sk-"))
                || lower.contains("api_key=")
                || lower.contains("apikey=")
                || lower.contains("authorization:")
                || s.contains("://") && (s.contains('@') || s.contains('?'))
                || s.starts_with('/')
                || s.starts_with("data:")
                || s.contains(":\\")
            {
                *count += 1;
                *s = "[REDACTED]".into();
            }
        }
        _ => {}
    }
}
struct Spec {
    kind: &'static str,
    table: &'static str,
    columns: &'static str,
    id: &'static str,
    scope: &'static str,
}
fn specs() -> Vec<Spec> {
    let mut out = Vec::new();
    macro_rules! add {
        ($kind:expr,$table:expr,$cols:expr,$id:expr,$scope:expr) => {
            out.push(Spec {
                kind: $kind,
                table: $table,
                columns: $cols,
                id: $id,
                scope: $scope,
            });
        };
    }
    add!(
        "messages",
        "conversation_messages",
        "conversation_id,sequence,message_id,input_json,created_at",
        "message_id",
        "conversation_id=?1"
    );
    add!(
        "tasks",
        "conversation_tasks",
        "id,conversation_id,source_message_id,schema_revision,created_at",
        "id",
        "conversation_id=?1"
    );
    add!(
        "calls",
        "conversation_model_calls",
        "id,task_id,request_hash,status,evidence_json,created_at",
        "id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "call_progress",
        "conversation_call_progress",
        "call_id,stage,completed_at,failure_json",
        "call_id",
        "call_id IN (SELECT id FROM conversation_model_calls WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "builders",
        "conversation_builder_operations",
        "id,task_id,request_hash,status,evidence_json",
        "id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "agent_sessions",
        "agent_sessions",
        "id,project_id,kind,status,session_json,created_at,updated_at",
        "id",
        "id IN (SELECT id FROM conversation_builder_operations WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)) AND project_id=?2"
    );
    add!(
        "schema_drafts",
        "conversation_schema_drafts",
        "id,task_id,source_call_id,source_request_id,base_schema_revision",
        "id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "schema_revisions",
        "conversation_schema_revisions",
        "draft_id,revision,request_id,definition_json",
        "draft_id || ':' || revision",
        "draft_id IN (SELECT id FROM conversation_schema_drafts WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "human_requests",
        "conversation_human_requests",
        "id,task_id,request_json,status,answer_json,created_at",
        "id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "resume_results",
        "conversation_resume_results",
        "request_id,draft_id,error",
        "request_id",
        "request_id IN (SELECT id FROM conversation_human_requests WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "send_receipts",
        "conversation_send_receipts",
        "conversation_id,message_id,input_json,receipt_json",
        "message_id",
        "conversation_id=?1"
    );
    add!(
        "queue",
        "conversation_message_queue",
        "conversation_id,message_id,task_id,sequence,cancelled_at",
        "message_id",
        "conversation_id=?1"
    );
    add!(
        "queued_planning",
        "conversation_queued_planning",
        "call_id,conversation_id,task_id,message_id,record_json",
        "call_id",
        "conversation_id=?1"
    );
    add!(
        "queued_copies",
        "conversation_queued_workflow_copies",
        "copy_id,conversation_id,task_id,message_id,request_json",
        "copy_id",
        "conversation_id=?1"
    );
    add!(
        "stops",
        "conversation_stop_requests",
        "message_id,conversation_id,record_json,created_at",
        "message_id",
        "conversation_id=?1"
    );
    add!(
        "feedback",
        "conversation_feedback_authorizations",
        "call_id,task_id,conversation_id,message_id,record_json",
        "call_id",
        "conversation_id=?1"
    );
    add!(
        "feedback_answers",
        "conversation_feedback_scope_answers",
        "call_id,task_id,conversation_id,command_id,input_json,created_at",
        "call_id",
        "conversation_id=?1"
    );
    add!(
        "image_reviews",
        "conversation_image_class_reviews",
        "id,feedback_call_id,task_id,conversation_id,sample_test_id,image_id,status,answer_command_id,record_json",
        "id",
        "conversation_id=?1"
    );
    add!(
        "future_schemas",
        "conversation_future_schema_drafts",
        "feedback_call_id,task_id,conversation_id,command_id,schema_id,input_json,created_at",
        "feedback_call_id",
        "conversation_id=?1"
    );
    add!(
        "journeys",
        "conversation_journey_consents",
        "id,task_id,builder_operation_id,sample_operation_id,input_json,revoked,sample_scope_json,created_at",
        "id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "journey_dispatch",
        "conversation_journey_dispatch",
        "consent_id,attempt_id,status,error,updated_at",
        "consent_id",
        "consent_id IN (SELECT id FROM conversation_journey_consents WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "journey_schema",
        "conversation_journey_schema_resolution",
        "consent_id,resolved_json",
        "consent_id",
        "consent_id IN (SELECT id FROM conversation_journey_consents WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "sample_operations",
        "sample_operations",
        "id,project_id,draft_id,request_json,status,error,created_at,updated_at",
        "id",
        "project_id=?2 AND json_extract(request_json,'$.conversation.task_id') IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "processing_operations",
        "processing_operations",
        "id,project_id,request_json,state_json,created_at,updated_at",
        "id",
        "project_id=?2 AND json_extract(state_json,'$.authorization.conversation.task_id') IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "exports",
        "conversation_exports",
        "id,conversation_id,task_id,format,result_json,error,created_at",
        "id",
        "conversation_id=?1"
    );

    add!(
        "cancellations",
        "conversation_call_cancellations",
        "call_id,task_id,requested_at",
        "call_id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "clarifications",
        "conversation_schema_clarification_answers",
        "call_id,request_id,schema_draft_id",
        "call_id",
        "call_id IN (SELECT id FROM conversation_model_calls WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "deferrals",
        "conversation_human_deferrals",
        "command_id,request_id,revision,deferred",
        "command_id",
        "request_id IN (SELECT id FROM conversation_human_requests WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "resume_evidence",
        "conversation_resume_outbox",
        "request_id,task_id,checkpoint_ref,feedback_revision_id,applied_at",
        "request_id",
        "task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1)"
    );
    add!(
        "answer_delivery",
        "conversation_answer_delivery",
        "request_id,consent_id,feedback_revision_id,status,error,created_at",
        "request_id",
        "request_id IN (SELECT id FROM conversation_human_requests WHERE task_id IN (SELECT id FROM conversation_tasks WHERE conversation_id=?1))"
    );
    add!(
        "agent_model",
        "conversation_agent_models",
        "conversation_id,revision,model_profile_id",
        "conversation_id",
        "conversation_id=?1"
    );
    add!(
        "export_events",
        "conversation_export_events",
        "sequence,conversation_id,task_id,export_id,kind",
        "CAST(sequence AS TEXT)",
        "conversation_id=?1"
    );
    out
}
fn rows(
    db: &Connection,
    spec: &Spec,
    conversation: &str,
    project: &str,
) -> Result<Vec<ArchiveRecord>, StorageError> {
    // All identifiers/expressions come from the private allowlist, never imported JSON.
    let order = if spec.kind == "messages" {
        "sequence"
    } else {
        "archive_id"
    };
    let sql = format!(
        "SELECT {},{} AS archive_id FROM {} WHERE {} AND ?2=?2 ORDER BY {order} LIMIT {}",
        spec.columns,
        spec.id,
        spec.table,
        spec.scope,
        MAX_RECORDS + 1
    );
    let mut stmt = db.prepare(&sql)?;
    let names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut query = stmt.query(params![conversation, project])?;
    let mut out = Vec::new();
    while let Some(row) = query.next()? {
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate().take(names.len() - 1) {
            let value = match row.get_ref(index)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => json!(n),
                rusqlite::types::ValueRef::Real(n) => json!(n),
                rusqlite::types::ValueRef::Text(s) => {
                    let s = std::str::from_utf8(s)
                        .map_err(|_| error("archive_invalid_storage", "Non UTF-8 record"))?;
                    if name.ends_with("_json") {
                        serde_json::from_str(s)?
                    } else {
                        json!(s)
                    }
                }
                rusqlite::types::ValueRef::Blob(_) => {
                    return Err(error(
                        "archive_invalid_storage",
                        "Binary record not supported",
                    ));
                }
            };
            object.insert(name.strip_suffix("_json").unwrap_or(name).into(), value);
        }
        out.push(ArchiveRecord {
            kind: spec.kind.into(),
            id: row.get(names.len() - 1)?,
            data: Value::Object(object),
        });
        if out.len() > MAX_RECORDS {
            return Err(error("archive_too_large", "Archive record limit exceeded"));
        }
    }
    Ok(out)
}
fn collect_refs(value: &Value, refs: &mut BTreeSet<(String, String)>) {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                if sensitive_key(k) {
                    continue;
                }
                if matches!(
                    k.as_str(),
                    "draft_id"
                        | "copy_id"
                        | "sample_test_id"
                        | "image_id"
                        | "artifact_id"
                        | "source_artifact_id"
                        | "run_id"
                        | "workflow_id"
                        | "model_id"
                        | "model_profile_id"
                        | "instance_id"
                ) && let Some(id) = v.as_str()
                    && !id.is_empty()
                {
                    refs.insert((k.clone(), id.into()));
                }
                collect_refs(v, refs);
            }
        }
        Value::Array(a) => {
            for v in a {
                collect_refs(v, refs);
            }
        }
        _ => {}
    }
}
impl SqliteStore {
    pub fn export_context_archive(
        &self,
        owner: &str,
        project: &str,
        conversation: Uuid,
    ) -> Result<ContextArchive, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            crate::conversations::require_owner(&tx,owner,conversation)?;
            let mut records=Vec::new();
            for spec in specs() {records.extend(rows(&tx,&spec,&conversation.to_string(),project)?);}
            for record in records.iter_mut().filter(|r|r.kind=="tasks") {
                let state:String=tx.query_row("SELECT CASE
                WHEN EXISTS(SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='in_doubt') THEN 'outcome_unknown'
                WHEN EXISTS(SELECT 1 FROM conversation_call_cancellations x JOIN conversation_model_calls c ON c.id=x.call_id WHERE c.task_id=?1 AND c.status='reserved') OR EXISTS(SELECT 1 FROM sample_operations WHERE json_extract(request_json,'$.conversation.task_id')=?1 AND status='cancelling') THEN 'stopping'
                WHEN EXISTS(SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='reserved') OR EXISTS(SELECT 1 FROM conversation_builder_operations WHERE task_id=?1 AND status='reserved') OR EXISTS(SELECT 1 FROM sample_operations WHERE json_extract(request_json,'$.conversation.task_id')=?1 AND status IN ('queued','running')) THEN 'running'
                WHEN EXISTS(SELECT 1 FROM conversation_human_requests WHERE task_id=?1 AND status='pending') THEN 'waiting_for_human'
                WHEN EXISTS(SELECT 1 FROM conversation_message_queue q LEFT JOIN conversation_queued_planning p USING(conversation_id,message_id) WHERE q.task_id=?1 AND q.cancelled_at IS NULL AND p.call_id IS NULL) THEN 'awaiting_approval'
                ELSE 'idle' END",[&record.id],|r|r.get(0))?;
                record.data["state"]=json!(state);
                record.data["state_scope"]=json!("task_activity_at_capture");
            }
            let mut visited=BTreeSet::new();
            let mut missing=BTreeSet::new();
            loop {
                let mut refs=BTreeSet::new();
                for record in &records {collect_refs(&record.data,&mut refs);}
                let pending:Vec<_>=refs.difference(&visited).cloned().collect();
                if pending.is_empty() {break;}
                for (kind,id) in pending {
                    visited.insert((kind.clone(),id.clone()));
                    let spec=match kind.as_str() {
                        "draft_id"|"copy_id"=>Some(Spec{kind:"workflow_drafts",table:"workflow_drafts",columns:"id,project_id,status,draft_json,created_at,updated_at,revision,content_hash",id:"id",scope:"id=?1 AND project_id=?2"}),
                        "sample_test_id"=>Some(Spec{kind:"sample_tests",table:"workflow_sample_tests",columns:"id,draft_id,project_id,draft_revision,request_revision,draft_content_hash,image_set_hash,model_snapshot_hash,status,input_json,model_bindings_json,report_json,started_at,completed_at",id:"id",scope:"id=?1 AND project_id=?2"}),
                        _=>None,
                    };
                    if let Some(spec)=spec {
                        if spec.kind=="workflow_drafts" && records.iter().any(|r|r.kind=="schema_drafts" && r.id==id) {continue;}
                        let found=rows(&tx,&spec,&id,project)?;
                        if found.is_empty() {missing.insert((kind.clone(),id.clone()));}
                        else {
                            if spec.kind=="workflow_drafts" {
                                let samples=Spec{kind:"sample_tests",table:"workflow_sample_tests",columns:"id,draft_id,project_id,draft_revision,request_revision,draft_content_hash,image_set_hash,model_snapshot_hash,status,input_json,model_bindings_json,report_json,started_at,completed_at",id:"id",scope:"draft_id=?1 AND project_id=?2"};
                                records.extend(rows(&tx,&samples,&id,project)?);
                                let plan=Spec{kind:"plan_revisions",table:"sample_plan_revisions",columns:"draft_id,project_id,sample_test_id,feedback_json,created_at",id:"draft_id",scope:"draft_id=?1 AND project_id=?2"};
                                records.extend(rows(&tx,&plan,&id,project)?);
                            }
                            records.extend(found);
                        }
                    }
                }
                let sample_ids:Vec<_>=records.iter().filter(|r|r.kind=="sample_tests").map(|r|r.id.clone()).collect();
                for id in sample_ids {
                    if visited.insert(("feedback_for_sample".into(),id.clone())) {
                        let feedback=Spec{kind:"sample_feedback",table:"sample_feedback_revisions",columns:"revision_id,sample_test_id,image_id,sequence,feedback_json",id:"revision_id",scope:"sample_test_id=?1"};
                        records.extend(rows(&tx,&feedback,&id,project)?);
                    }
                }
                let mut seen=BTreeSet::new();
                records.retain(|r|seen.insert((r.kind.clone(),r.id.clone())));
                if records.len()>MAX_RECORDS {return Err(error("archive_too_large","Archive record limit exceeded"));}
            }
            // External references are identities only; existence is deliberately not checked
            // by opening files, plugins, Providers, or another project's records.
            let resources:Vec<_>=visited.iter().filter(|(kind,_)|kind!="feedback_for_sample").map(|(kind,id)| {
                let embedded=records.iter().any(|r|r.id==*id && matches!(r.kind.as_str(),"workflow_drafts"|"sample_tests"|"schema_drafts"));
                json!({"kind":kind,"id":id,"embedded":embedded,"availability":if embedded {"embedded"} else if missing.contains(&(kind.clone(),id.clone())) {"missing_or_not_owned"} else {"not_verified"}})
            }).collect();
            let mut redacted=0;
            for record in &mut records {scrub(&mut record.data,&mut redacted);}
            let payload=ArchivePayload{
                source:ArchiveSource{project_id:project.into(),project_owner_id:owner.into(),conversation_id:conversation},
                captured_at:chrono::Utc::now().to_rfc3339(),records,resources,
                integrity:json!({"snapshot":"sqlite_transaction","all_messages":true,"all_tasks":true,"truncated":false,"redacted_fields":redacted,"missing_resources":missing.iter().map(|(kind,id)|json!({"kind":kind,"id":id,"reason":"missing_or_not_owned"})).collect::<Vec<_>>(),"limitations":["raw_model_http_transcripts_not_persisted","hidden_reasoning_not_exported","external_resources_not_embedded","historical_authority_not_transferable"]}),
            };
            let archive=ContextArchive{format:"annotagent.context".into(),version:1,archive_hash:archive_payload_hash(&payload)?,payload};
            validate(&archive)?;
            tx.commit()?;
            Ok(archive)
        })
    }
    pub fn preview_context_import(
        &self,
        owner: &str,
        project: &str,
        archive: &ContextArchive,
    ) -> Result<Value, StorageError> {
        validate(archive)?;
        preview(owner, project, archive)
    }
    pub fn confirm_context_import(
        &self,
        owner: &str,
        project: &str,
        input: &ConfirmContextImport,
    ) -> Result<Value, StorageError> {
        validate(&input.archive)?;
        let preview = preview(owner, project, &input.archive)?;
        if !input.confirm_archive_only || preview["preview_hash"] != input.preview_hash {
            return Err(error(
                "archive_preview_conflict",
                "Confirm the exact archive-only preview",
            ));
        }
        self.with_connection(|db| {
            let tx=rusqlite::Transaction::new_unchecked(db,rusqlite::TransactionBehavior::Immediate)?;
            let old:Option<(String,String,String)>=tx.query_row("SELECT project_owner_id,preview_hash,receipt_json FROM context_archives WHERE command_id=?1",[input.command_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            if let Some((old_owner,old_hash,receipt))=old {
                if old_owner!=owner || old_hash!=input.preview_hash {return Err(error("archive_command_conflict","Command already belongs to a different import scope"));}
                return Ok(serde_json::from_str(&receipt)?);
            }
            let context=Uuid::new_v4();
            let mut map=vec![json!({"kind":"conversation","source_id":input.archive.payload.source.conversation_id,"id":context})];
            map.extend(input.archive.payload.records.iter().map(|r|json!({"kind":r.kind,"source_id":r.id,"id":Uuid::new_v4()})));
            let mut receipt=json!({"command_id":input.command_id,"context_id":context,"project_id":project,"archive_hash":input.archive.archive_hash,"preview_hash":input.preview_hash,"sequence":0,"mode":"archive_only","created_at":chrono::Utc::now().to_rfc3339(),"id_map":map,"continuation":continuation()});
            tx.execute("INSERT INTO context_archives(id,project_owner_id,command_id,preview_hash,archive_json,receipt_json) VALUES(?1,?2,?3,?4,?5,?6)",params![context.to_string(),owner,input.command_id.to_string(),input.preview_hash,serde_json::to_string(&input.archive)?,receipt.to_string()])?;
            receipt["sequence"]=json!(tx.last_insert_rowid());
            tx.execute("UPDATE context_archives SET receipt_json=?2 WHERE id=?1",params![context.to_string(),receipt.to_string()])?;
            tx.commit()?;
            Ok(receipt)
        })
    }
    pub fn context_import_receipt(
        &self,
        owner: &str,
        command: Uuid,
    ) -> Result<Value, StorageError> {
        self.with_connection(|db| {
            let raw:Option<String>=db.query_row("SELECT receipt_json FROM context_archives WHERE command_id=?1 AND project_owner_id=?2",params![command.to_string(),owner],|r|r.get(0)).optional()?;
            serde_json::from_str(&raw.ok_or_else(||error("foreign_project_object","Import receipt not found"))?).map_err(Into::into)
        })
    }
    pub fn archived_context(&self, owner: &str, id: Uuid) -> Result<Value, StorageError> {
        self.with_connection(|db| {
            let raw:Option<(String,String)>=db.query_row("SELECT receipt_json,archive_json FROM context_archives WHERE id=?1 AND project_owner_id=?2",params![id.to_string(),owner],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let (receipt,archive)=raw.ok_or_else(||error("foreign_project_object","Archived context not found"))?;
            let receipt:Value=serde_json::from_str(&receipt)?;
            let archive:ContextArchive=serde_json::from_str(&archive)?;
            let objects:Vec<_>=archive.payload.records.iter().zip(receipt["id_map"].as_array().expect("stored map").iter().skip(1)).map(|(r,m)|json!({"id":m["id"],"kind":r.kind,"source_id":r.id,"data":r.data})).collect();
            Ok(json!({"receipt":receipt,"archive":archive,"objects":objects,"trust":"untrusted_import"}))
        })
    }
    pub fn archived_contexts(
        &self,
        owner: &str,
        after: i64,
        limit: u32,
    ) -> Result<Value, StorageError> {
        if after < 0 || !(1..=100).contains(&limit) {
            return Err(error(
                "archive_invalid_page",
                "Cursor must be nonnegative; limit is 1..100",
            ));
        }
        self.with_connection(|db| {
            let mut stmt=db.prepare("SELECT receipt_json FROM context_archives WHERE project_owner_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
            let raw=stmt.query_map(params![owner,after,limit+1],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            let more=raw.len()>limit as usize;
            let items:Vec<Value>=raw.into_iter().take(limit as usize).map(|s|serde_json::from_str(&s)).collect::<Result<_,_>>()?;
            let cursor=if more {items.last().map(|v|v["sequence"].clone())} else {None};
            Ok(json!({"items":items,"next_cursor":cursor}))
        })
    }
}
fn preview(owner: &str, project: &str, archive: &ContextArchive) -> Result<Value, StorageError> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for r in &archive.payload.records {
        *counts.entry(&r.kind).or_default() += 1;
    }
    Ok(
        json!({"valid":true,"preview_hash":hash(&json!({"target_project_owner_id":owner,"target_project_id":project,"archive_hash":archive.archive_hash,"mode":"archive_only","version":1}))?,"archive_hash":archive.archive_hash,"target_project_id":project,"counts":counts,"integrity":archive.payload.integrity,"continuation":continuation(),"mode":"archive_only","trust":"untrusted_import"}),
    )
}
fn validate(archive: &ContextArchive) -> Result<(), StorageError> {
    if archive.format != "annotagent.context" || archive.version != 1 {
        return Err(error(
            "archive_unsupported_version",
            "Expected annotagent.context version 1",
        ));
    }
    if serde_json::to_vec(archive)?.len() > MAX_BYTES || archive.payload.records.len() > MAX_RECORDS
    {
        return Err(error(
            "archive_too_large",
            "Archive exceeds 1 MiB or 10000 records",
        ));
    }
    if archive_payload_hash(&archive.payload)? != archive.archive_hash
        && legacy_hash(&archive.payload)? != archive.archive_hash
    {
        return Err(error(
            "archive_hash_mismatch",
            "Archive payload integrity check failed",
        ));
    }
    if chrono::DateTime::parse_from_rfc3339(&archive.payload.captured_at).is_err()
        || archive.payload.source.project_id.is_empty()
    {
        return Err(error(
            "archive_invalid",
            "Invalid archive source or timestamp",
        ));
    }
    let integrity = &archive.payload.integrity;
    if integrity["snapshot"] != "sqlite_transaction"
        || integrity["all_messages"] != true
        || integrity["all_tasks"] != true
        || integrity["truncated"] != false
        || integrity["redacted_fields"].as_u64().is_none()
        || !integrity["missing_resources"].is_array()
        || !integrity["limitations"].is_array()
    {
        return Err(error(
            "archive_invalid",
            "Invalid integrity manifest; truncated imports are not supported",
        ));
    }
    for resource in &archive.payload.resources {
        if resource["kind"].as_str().is_none()
            || resource["id"].as_str().is_none()
            || resource["embedded"].as_bool().is_none()
            || !matches!(
                resource["availability"].as_str(),
                Some("embedded" | "missing_or_not_owned" | "not_verified")
            )
        {
            return Err(error("archive_invalid", "Invalid resource manifest"));
        }
    }
    let conversation = archive.payload.source.conversation_id.to_string();
    let messages: BTreeSet<_> = archive
        .payload
        .records
        .iter()
        .filter(|r| r.kind == "messages")
        .map(|r| r.id.as_str())
        .collect();
    let mut sequences = BTreeSet::new();
    for record in &archive.payload.records {
        if matches!(record.kind.as_str(), "messages" | "tasks")
            && record.data["conversation_id"] != conversation
        {
            return Err(error(
                "archive_invalid",
                "Message or task belongs to another conversation",
            ));
        }
        if record.kind == "messages"
            && (record.data["message_id"] != record.id
                || record.data["input"]["id"] != record.id
                || record.data["input"]["text"].as_str().is_none()
                || !record.data["sequence"]
                    .as_u64()
                    .is_some_and(|n| n > 0 && sequences.insert(n)))
        {
            return Err(error(
                "archive_invalid",
                "Invalid message identity, sequence or text",
            ));
        }
        if record.kind == "tasks"
            && (record.data["id"] != record.id
                || !record.data["source_message_id"]
                    .as_str()
                    .is_some_and(|id| messages.contains(id)))
        {
            return Err(error("archive_invalid", "Task source message is missing"));
        }
    }
    let mut kinds: BTreeSet<_> = specs().iter().map(|s| s.kind.to_string()).collect();
    kinds.extend(
        [
            "workflow_drafts",
            "sample_tests",
            "sample_feedback",
            "plan_revisions",
        ]
        .map(String::from),
    );
    let mut seen = BTreeSet::new();
    for record in &archive.payload.records {
        if !kinds.contains(&record.kind)
            || record.id.is_empty()
            || !record.data.is_object()
            || !seen.insert((&record.kind, &record.id))
        {
            return Err(error(
                "archive_invalid",
                "Unknown kind, duplicate identity or invalid record",
            ));
        }
    }
    // A caller may edit an archive and recompute its public hash. Never store obvious
    // secrets or authority in an imported archive merely because its hash matches.
    let mut redacted = 0;
    let mut copy = serde_json::to_value(&archive.payload)?;
    scrub(&mut copy, &mut redacted);
    if redacted > 0 {
        return Err(error(
            "archive_sensitive_content",
            "Remove credential-bearing fields or strings before import",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser_number_round_trip(value: &Value) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), browser_number_round_trip(value)))
                    .collect(),
            ),
            Value::Array(items) => {
                Value::Array(items.iter().map(browser_number_round_trip).collect())
            }
            Value::Number(number) => {
                let Some(float) = number.as_f64() else {
                    return value.clone();
                };
                if float == 0.0 {
                    Value::Number(0.into())
                } else if number.as_i64().is_none()
                    && number.as_u64().is_none()
                    && float.is_finite()
                    && float.fract() == 0.0
                    && (-MAX_SAFE_JSON_INTEGER..=MAX_SAFE_JSON_INTEGER).contains(&float)
                {
                    Value::Number((float as i64).into())
                } else {
                    value.clone()
                }
            }
            _ => value.clone(),
        }
    }

    #[test]
    fn archive_hash_is_stable_across_browser_integral_number_round_trip() {
        let rust = json!({
            "bounds": [-0.0, 0.0, 1.0, -2.0, 0.25],
            "nested": {"revision": 3.0, "unchanged": 4},
        });
        let browser = browser_number_round_trip(&rust);
        assert_eq!(browser["bounds"], json!([0, 0, 1, -2, 0.25]));
        assert_eq!(browser["nested"]["revision"], 3);
        assert_eq!(hash(&rust).unwrap(), hash(&browser).unwrap());

        let mut changed = browser;
        changed["bounds"][4] = json!(0.5);
        assert_ne!(hash(&rust).unwrap(), hash(&changed).unwrap());
    }

    #[test]
    fn archive_validation_keeps_exact_legacy_v1_float_hash_compatible() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, conversation, _) = seed(&store);
        let mut archive = store
            .export_context_archive(&owner, "TEST-P", conversation)
            .unwrap();
        archive.payload.resources.push(json!({
            "kind": "TEST_numeric",
            "id": "TEST-legacy",
            "embedded": false,
            "availability": "not_verified",
            "values": [-0.0, 0.0, 1.0, 0.25]
        }));
        archive.archive_hash = legacy_hash(&archive.payload).unwrap();
        assert_ne!(
            archive.archive_hash,
            archive_payload_hash(&archive.payload).unwrap()
        );
        validate(&archive).unwrap();

        let legacy = archive.payload.resources.last_mut().unwrap();
        legacy["values"][3] = json!(0.5);
        assert!(
            matches!(validate(&archive),Err(StorageError::Management{code,..}) if code=="archive_hash_mismatch")
        );
    }

    fn seed(store: &SqliteStore) -> (String, Uuid, Uuid) {
        let owner = Uuid::new_v4().to_string();
        let c = store.create_conversation(&owner).unwrap();
        let mut first = Uuid::nil();
        for i in 0..121 {
            let id = Uuid::new_v4();
            if i == 0 {
                first = id;
            }
            store
                .append_conversation_message(
                    &owner,
                    c,
                    &crate::ConversationMessageInput {
                        id,
                        text: format!("TEST message {i}"),
                        image: None,
                        reference: None,
                    },
                )
                .unwrap();
        }
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &owner,
                c,
                &crate::BeginConversationTask {
                    id: task,
                    source_message_id: first,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        store.with_connection(|db| {
            let call=Uuid::new_v4().to_string();
            let builder=Uuid::new_v4().to_string();
            db.execute("INSERT INTO conversation_call_grants(task_id,id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,'TEST',4,'2099-01-01')",params![task.to_string(),Uuid::new_v4().to_string()])?;
            db.execute("INSERT INTO conversation_model_calls(id,task_id,request_hash,status,evidence_json,created_at) VALUES(?1,?2,'TEST','in_doubt',?3,'2026-09-10T00:00:00Z')",params![call,task.to_string(),json!({"failure":{"category":"transport"},"api_key":"TEST-secret"}).to_string()])?;
            db.execute("INSERT INTO conversation_call_progress(call_id,stage) VALUES(?1,'request_started')",[&call])?;
            db.execute("INSERT INTO conversation_builder_operations(id,task_id,request_hash,status) VALUES(?1,?2,'TEST','completed')",params![builder,task.to_string()])?;
            db.execute("INSERT INTO agent_sessions(id,project_id,kind,status,session_json,created_at,updated_at) VALUES(?1,'TEST-P','pipeline_builder','completed',?2,'now','now')",params![builder,json!({"steps":[{"tool_name":"inspect","arguments":{"image_id":"TEST-image"},"result":{"ok":true}}],"builder_proposal":{"title":"Actual saved proposal"},"draft_id":"TEST-missing-draft","working_memory":{"reasoning_content":"not an exported transcript"}}).to_string()])?;
            Ok(())
        }).unwrap();
        (owner, c, task)
    }
    fn confirm(
        store: &SqliteStore,
        owner: &str,
        archive: ContextArchive,
        id: Uuid,
    ) -> ConfirmContextImport {
        let p = store
            .preview_context_import(owner, "TEST-P", &archive)
            .unwrap();
        ConfirmContextImport {
            command_id: id,
            archive,
            preview_hash: p["preview_hash"].as_str().unwrap().into(),
            confirm_archive_only: true,
        }
    }
    #[test]
    fn archive_all_pages_observable_evidence_redaction_and_missing_resources() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, c, _) = seed(&store);
        let a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        assert_eq!(
            a.payload
                .records
                .iter()
                .filter(|r| r.kind == "messages")
                .count(),
            121
        );
        assert_eq!(
            a.payload
                .records
                .iter()
                .filter(|r| r.kind == "tasks")
                .count(),
            1
        );
        assert_eq!(a.payload.integrity["truncated"], false);
        let sequences: Vec<_> = a
            .payload
            .records
            .iter()
            .filter(|r| r.kind == "messages")
            .map(|r| r.data["sequence"].as_u64().unwrap())
            .collect();
        assert_eq!(sequences, (1..=121).collect::<Vec<_>>());
        assert_eq!(a.payload.integrity["redacted_fields"], 2);
        assert_eq!(
            a.payload.integrity["missing_resources"][0]["id"],
            "TEST-missing-draft"
        );
        let session = &a
            .payload
            .records
            .iter()
            .find(|r| r.kind == "agent_sessions")
            .unwrap()
            .data["session"];
        assert_eq!(session["steps"][0]["result"]["ok"], true);
        assert_eq!(
            session["builder_proposal"]["title"],
            "Actual saved proposal"
        );
        assert!(!serde_json::to_string(&a).unwrap().contains("TEST-secret"));
        assert!(store.export_context_archive("wrong", "TEST-P", c).is_err());
        let before = store.with_connection(|db| Ok(db.total_changes())).unwrap();
        store.preview_context_import(&owner, "TEST-P", &a).unwrap();
        store.archived_contexts(&owner, 0, 50).unwrap();
        assert_eq!(
            before,
            store.with_connection(|db| Ok(db.total_changes())).unwrap()
        );
    }
    #[test]
    fn archive_import_lost_response_restart_new_ids_and_no_live_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, c, _) = seed(&store);
        let archive = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        let input = confirm(&store, &owner, archive.clone(), Uuid::new_v4());
        let before = store
            .with_connection(|db| {
                let mut counts = Vec::new();
                for table in [
                    "project_conversations",
                    "conversation_tasks",
                    "conversation_model_calls",
                    "conversation_call_grants",
                    "conversation_message_queue",
                    "conversation_resume_outbox",
                    "workflow_versions",
                    "annotations",
                ] {
                    counts.push(db.query_row(
                        &format!("SELECT count(*) FROM {table}"),
                        [],
                        |r| r.get::<_, i64>(0),
                    )?);
                }
                Ok(counts)
            })
            .unwrap();
        let receipt = store
            .confirm_context_import(&owner, "TEST-P", &input)
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            receipt,
            store
                .confirm_context_import(&owner, "TEST-P", &input)
                .unwrap()
        );
        assert_eq!(
            receipt,
            store
                .context_import_receipt(&owner, input.command_id)
                .unwrap()
        );
        let context = Uuid::parse_str(receipt["context_id"].as_str().unwrap()).unwrap();
        assert_ne!(context, c);
        let loaded = store.archived_context(&owner, context).unwrap();
        assert_eq!(loaded["archive"], serde_json::to_value(archive).unwrap());
        for m in receipt["id_map"].as_array().unwrap() {
            assert_ne!(m["source_id"], m["id"]);
        }
        assert!(store.archived_context("foreign", context).is_err());
        assert!(
            store
                .context_import_receipt("foreign", input.command_id)
                .is_err()
        );
        assert_eq!(
            store.archived_contexts("foreign", 0, 1).unwrap()["items"],
            json!([])
        );
        let after = store
            .with_connection(|db| {
                let mut counts = Vec::new();
                for table in [
                    "project_conversations",
                    "conversation_tasks",
                    "conversation_model_calls",
                    "conversation_call_grants",
                    "conversation_message_queue",
                    "conversation_resume_outbox",
                    "workflow_versions",
                    "annotations",
                ] {
                    counts.push(db.query_row(
                        &format!("SELECT count(*) FROM {table}"),
                        [],
                        |r| r.get::<_, i64>(0),
                    )?);
                }
                Ok(counts)
            })
            .unwrap();
        assert_eq!(before, after);
    }
    #[test]
    fn archive_rejects_tamper_unknown_version_secret_and_changed_command_scope() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, c, _) = seed(&store);
        let a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        let mut bad = a.clone();
        bad.payload.records[0].data["text"] = json!("tampered");
        assert!(validate(&bad).is_err());
        bad = a.clone();
        bad.version = 2;
        assert!(validate(&bad).is_err());
        bad = a.clone();
        bad.payload.records[0].data["password"] = json!("TEST-password");
        bad.archive_hash = hash(&bad.payload).unwrap();
        assert!(validate(&bad).is_err());
        bad = a.clone();
        bad.payload.records.push(bad.payload.records[0].clone());
        bad.archive_hash = hash(&bad.payload).unwrap();
        assert!(validate(&bad).is_err());
        let command = Uuid::new_v4();
        let mut input = confirm(&store, &owner, a.clone(), command);
        store
            .confirm_context_import(&owner, "TEST-P", &input)
            .unwrap();
        input.confirm_archive_only = false;
        assert!(
            store
                .confirm_context_import(&owner, "TEST-P", &input)
                .is_err()
        );
        let other = confirm(&store, "other", a.clone(), command);
        assert!(
            store
                .confirm_context_import("other", "TEST-P", &other)
                .is_err()
        );
        bad = a;
        bad.payload.records[0].data["note"] = json!("changed");
        bad.archive_hash = hash(&bad.payload).unwrap();
        let changed = confirm(&store, &owner, bad, command);
        assert!(
            store
                .confirm_context_import(&owner, "TEST-P", &changed)
                .is_err()
        );
    }
    #[test]
    fn archive_two_writers_one_context_and_owner_pagination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, c, _) = seed(&store);
        let a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        let command = Uuid::new_v4();
        let other = SqliteStore::open(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b = barrier.clone();
        let o = owner.clone();
        let a2 = a.clone();
        let thread = std::thread::spawn(move || {
            let input = confirm(&other, &o, a2, command);
            b.wait();
            other.confirm_context_import(&o, "TEST-P", &input).unwrap()
        });
        let input = confirm(&store, &owner, a.clone(), command);
        barrier.wait();
        let first = store
            .confirm_context_import(&owner, "TEST-P", &input)
            .unwrap();
        assert_eq!(first, thread.join().unwrap());
        store
            .confirm_context_import(
                &owner,
                "TEST-P",
                &confirm(&store, &owner, a, Uuid::new_v4()),
            )
            .unwrap();
        let page = store.archived_contexts(&owner, 0, 1).unwrap();
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        let next = store
            .archived_contexts(&owner, page["next_cursor"].as_i64().unwrap(), 1)
            .unwrap();
        assert_eq!(next["next_cursor"], Value::Null);
        assert_ne!(
            page["items"][0]["context_id"],
            next["items"][0]["context_id"]
        );
    }
    #[test]
    fn archive_related_draft_sample_feedback_and_foreign_reference_boundary() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, c, task) = seed(&store);
        store.with_connection(|db|{
            db.execute("INSERT INTO workflow_drafts(id,project_id,status,draft_json,created_at,updated_at) VALUES('TEST-missing-draft','TEST-P','editing',?1,'now','now')",[json!({"id":"TEST-missing-draft","title":"Saved Draft","label_pipeline":{"nodes":[]}}).to_string()])?;
            db.execute("INSERT INTO workflow_drafts(id,project_id,status,draft_json,created_at,updated_at) VALUES('TEST-foreign-draft','FOREIGN','editing',?1,'now','now')",[json!({"title":"MUST-NOT-EXPORT"}).to_string()])?;
            db.execute("INSERT INTO workflow_sample_tests(id,draft_id,project_id,draft_revision,request_revision,draft_content_hash,image_set_hash,model_snapshot_hash,status,input_json,model_bindings_json,report_json,started_at,completed_at) VALUES('TEST-sample','TEST-missing-draft','TEST-P',1,1,'hash','images','models','succeeded','{}','[]',?1,'now','now')",[json!({"candidates":[{"id":"TEST-candidate","image_id":"TEST-image","bbox":[0.1,0.2,0.3,0.4]}]}).to_string()])?;
            db.execute("INSERT INTO sample_feedback_revisions(revision_id,sample_test_id,image_id,sequence,feedback_json) VALUES('TEST-feedback','TEST-sample','TEST-image',1,?1)",[json!({"draft_id":"TEST-foreign-draft","decision":"review"}).to_string()])?;
            db.execute("INSERT INTO conversation_schema_drafts(id,task_id,source_request_id,base_schema_revision) VALUES('TEST-schema',?1,'TEST-command','hash')",[task.to_string()])?;
            db.execute("INSERT INTO conversation_schema_revisions(draft_id,revision,request_id,definition_json) VALUES('TEST-schema',1,'TEST-command',?1)",[json!({"labels":["TEST-label"]}).to_string()])?;
            Ok(())
        }).unwrap();
        let a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        assert!(
            !a.payload.integrity["missing_resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == "TEST-schema")
        );
        for kind in [
            "workflow_drafts",
            "sample_tests",
            "sample_feedback",
            "schema_drafts",
            "schema_revisions",
        ] {
            assert_eq!(
                a.payload.records.iter().filter(|r| r.kind == kind).count(),
                1,
                "{kind}"
            );
        }
        assert!(
            !serde_json::to_string(&a)
                .unwrap()
                .contains("MUST-NOT-EXPORT")
        );
        assert!(
            a.payload.integrity["missing_resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == "TEST-foreign-draft")
        );
        assert!(
            a.payload
                .resources
                .iter()
                .any(|r| r["id"] == "TEST-sample" && r["embedded"] == true)
        );
    }
    #[test]
    fn archive_size_limit_is_explicit_not_truncation() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, c, _) = seed(&store);
        let mut a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
        a.payload.records[0].data["large"] = json!("x".repeat(MAX_BYTES));
        a.archive_hash = hash(&a.payload).unwrap();
        assert!(
            matches!(validate(&a),Err(StorageError::Management{code,..}) if code=="archive_too_large")
        );
    }
    #[test]
    fn archive_snapshot_does_not_mix_concurrent_message_task_commits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, c, _) = seed(&store);
        let writer = SqliteStore::open(&path).unwrap();
        let thread = std::thread::spawn(move || {
            for i in 0..20 {
                writer.with_connection(|db|{
                let tx=db.unchecked_transaction()?;let id=Uuid::new_v4().to_string();
                tx.execute("INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES(?1,?2,?3,?4,'now')",params![c.to_string(),122+i,id,json!({"id":id,"text":"TEST concurrent"}).to_string()])?;
                tx.execute("INSERT INTO conversation_tasks(id,conversation_id,source_message_id,schema_revision,created_at) VALUES(?1,?2,?3,'hash','now')",params![Uuid::new_v4().to_string(),c.to_string(),id])?;tx.commit()?;Ok(())
            }).unwrap();
            }
        });
        for _ in 0..10 {
            let a = store.export_context_archive(&owner, "TEST-P", c).unwrap();
            let messages = a
                .payload
                .records
                .iter()
                .filter(|r| r.kind == "messages")
                .count();
            let tasks = a
                .payload
                .records
                .iter()
                .filter(|r| r.kind == "tasks")
                .count();
            assert_eq!(messages - 120, tasks);
        }
        thread.join().unwrap();
    }
}
