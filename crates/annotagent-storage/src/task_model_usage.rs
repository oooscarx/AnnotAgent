//! Durable physical Provider-attempt usage owned by one Conversation Task.
use crate::{SqliteStore, StorageError};
use annotagent_core::{
    ModelFailure, ModelPricing, ModelPricingSnapshot, ModelProfile, ProviderProfile, TokenUsage,
    UsageSource,
};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskModelAttemptKind {
    Task,
    Probe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskModelAttemptStatus {
    Started,
    Succeeded,
    Failed,
    InDoubt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskModelAttempt {
    pub sequence: i64,
    pub attempt_id: Uuid,
    pub project_id: String,
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    pub call_id: Uuid,
    pub attempt_number: u32,
    pub kind: TaskModelAttemptKind,
    pub status: TaskModelAttemptStatus,
    pub model_profile_id: annotagent_core::ModelProfileId,
    pub model_profile_revision: u64,
    pub provider_id: annotagent_core::ProviderId,
    pub provider_name: String,
    pub request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub image_count: u64,
    pub usage_source: UsageSource,
    pub cost: Option<String>,
    pub currency: Option<String>,
    pub pricing_snapshot: ModelPricingSnapshot,
    pub effective_request: Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub failure: Option<ModelFailure>,
}

#[derive(Debug, Clone)]
pub struct BeginTaskModelAttempt<'a> {
    pub attempt_id: Uuid,
    pub call_id: Uuid,
    pub attempt_number: u32,
    pub kind: TaskModelAttemptKind,
    pub model: &'a ModelProfile,
    pub provider: &'a ProviderProfile,
    pub effective_request: &'a Value,
    pub image_count: u64,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FinishTaskModelAttempt {
    pub status: TaskModelAttemptStatus,
    pub request_id: Option<String>,
    pub usage: TokenUsage,
    pub cached_input_tokens: Option<u64>,
    pub completed_at: DateTime<Utc>,
    pub failure: Option<ModelFailure>,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.to_owned())
}

fn kind_name(kind: TaskModelAttemptKind) -> &'static str {
    match kind {
        TaskModelAttemptKind::Task => "task",
        TaskModelAttemptKind::Probe => "probe",
    }
}

fn status_name(status: TaskModelAttemptStatus) -> &'static str {
    match status {
        TaskModelAttemptStatus::Started => "started",
        TaskModelAttemptStatus::Succeeded => "succeeded",
        TaskModelAttemptStatus::Failed => "failed",
        TaskModelAttemptStatus::InDoubt => "in_doubt",
    }
}

fn usage_source_name(source: UsageSource) -> &'static str {
    match source {
        UsageSource::Actual => "actual",
        UsageSource::Estimated => "estimated",
        UsageSource::Mock => "mock",
        UsageSource::Unknown => "unknown",
    }
}

fn parse_kind(value: &str) -> Result<TaskModelAttemptKind, StorageError> {
    match value {
        "task" => Ok(TaskModelAttemptKind::Task),
        "probe" => Ok(TaskModelAttemptKind::Probe),
        _ => Err(invalid("invalid stored model attempt kind")),
    }
}

fn parse_status(value: &str) -> Result<TaskModelAttemptStatus, StorageError> {
    match value {
        "started" => Ok(TaskModelAttemptStatus::Started),
        "succeeded" => Ok(TaskModelAttemptStatus::Succeeded),
        "failed" => Ok(TaskModelAttemptStatus::Failed),
        "in_doubt" => Ok(TaskModelAttemptStatus::InDoubt),
        _ => Err(invalid("invalid stored model attempt status")),
    }
}

fn parse_usage_source(value: &str) -> Result<UsageSource, StorageError> {
    match value {
        "actual" => Ok(UsageSource::Actual),
        "estimated" => Ok(UsageSource::Estimated),
        "mock" => Ok(UsageSource::Mock),
        "unknown" => Ok(UsageSource::Unknown),
        _ => Err(invalid("invalid stored usage source")),
    }
}

fn cost(
    pricing: &ModelPricing,
    usage: &TokenUsage,
    cached_input_tokens: Option<u64>,
    image_count: u64,
) -> Option<Decimal> {
    let input = usage.input_tokens?;
    let output = usage.output_tokens?;
    let cached = cached_input_tokens?;
    if cached > input {
        return None;
    }
    let ordinary = input - cached;
    let input_cost = if ordinary == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(ordinary) * pricing.input_per_million_tokens? / Decimal::from(1_000_000_u64)
    };
    let cached_cost = if cached == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(cached) * pricing.cached_input_per_million_tokens?
            / Decimal::from(1_000_000_u64)
    };
    let output_cost = if output == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(output) * pricing.output_per_million_tokens? / Decimal::from(1_000_000_u64)
    };
    let image_cost = if image_count == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(image_count) * pricing.per_image?
    };
    Some(
        input_cost
            + cached_cost
            + output_cost
            + image_cost
            + pricing.per_request.unwrap_or_default(),
    )
}

impl SqliteStore {
    pub fn begin_task_model_attempt(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &BeginTaskModelAttempt<'_>,
    ) -> Result<TaskModelAttempt, StorageError> {
        if input.attempt_id.is_nil() || input.attempt_number == 0 {
            return Err(invalid(
                "attempt ID and positive attempt number are required",
            ));
        }
        if input.model.provider_id != input.provider.id {
            return Err(invalid("model attempt Provider binding changed"));
        }
        input
            .model
            .validate()
            .map_err(|error| invalid(&error.to_string()))?;
        input
            .provider
            .validate()
            .map_err(|error| invalid(&error.to_string()))?;
        let price = ModelPricingSnapshot::capture(input.model, input.started_at);
        self.with_connection(|db| {
            let model_revision = i64::try_from(input.model.revision)
                .map_err(|_| invalid("model revision exceeds SQLite range"))?;
            let owned: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.id=?2 AND c.project_id=?3)",
                params![task.to_string(), conversation.to_string(), project],
                |row| row.get(0),
            )?;
            if !owned {
                return Err(invalid("model attempt Task does not belong to this Project"));
            }
            let call_owned: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE id=?1 AND task_id=?2 AND status='reserved')",
                params![input.call_id.to_string(), task.to_string()],
                |row| row.get(0),
            )?;
            if !call_owned {
                return Err(invalid("a current reserved Task call is required before Provider attempt"));
            }
            let existing: Option<String> = db
                .query_row(
                    "SELECT attempt_id FROM task_model_attempts WHERE call_id=?1 AND attempt_number=?2",
                    params![input.call_id.to_string(), input.attempt_number],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(existing) = existing {
                if existing != input.attempt_id.to_string() {
                    return Err(invalid("attempt number already belongs to another identity"));
                }
                let saved=attempt(db,input.attempt_id)?.ok_or_else(||invalid("attempt missing"))?;
                if saved.project_id!=project||saved.conversation_id!=conversation||saved.task_id!=task||saved.call_id!=input.call_id||saved.kind!=input.kind||saved.model_profile_id!=input.model.id||saved.model_profile_revision!=input.model.revision||saved.provider_id!=input.provider.id||saved.pricing_snapshot!=price||saved.effective_request!=*input.effective_request||saved.image_count!=input.image_count||saved.started_at!=input.started_at {
                    return Err(invalid("attempt retry changed its frozen scope"));
                }
                return Ok(saved);
            }
            let frozen:Option<(String,i64,String,String,String,String,i64)>=db.query_row("SELECT model_profile_id,model_profile_revision,provider_id,kind,pricing_snapshot_json,effective_request_json,image_count FROM task_model_attempts WHERE call_id=?1 ORDER BY attempt_number LIMIT 1",[input.call_id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).optional()?;
            if let Some((model,revision,provider,kind,pricing,effective,image_count))=frozen
                && (model!=input.model.id.to_string()||revision!=model_revision||provider!=input.provider.id.to_string()||kind!=kind_name(input.kind)||serde_json::from_str::<ModelPricingSnapshot>(&pricing)?.pricing!=price.pricing||effective!=serde_json::to_string(input.effective_request)?||image_count.max(0) as u64!=input.image_count) {
                    return Err(invalid("physical retry changed its frozen Model or request scope"));
                }
            db.execute(
                "INSERT INTO task_model_attempts(attempt_id,project_id,conversation_id,task_id,call_id,attempt_number,kind,status,model_profile_id,model_profile_revision,provider_id,provider_name,usage_source,pricing_snapshot_json,effective_request_json,image_count,started_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'started',?8,?9,?10,?11,'unknown',?12,?13,?14,?15)",
                params![
                    input.attempt_id.to_string(),project,conversation.to_string(),task.to_string(),
                    input.call_id.to_string(),input.attempt_number,kind_name(input.kind),
                    input.model.id.to_string(),model_revision,input.provider.id.to_string(),
                    input.provider.display_name,serde_json::to_string(&price)?,
                    serde_json::to_string(input.effective_request)?,i64::try_from(input.image_count).map_err(|_|invalid("image count exceeds SQLite range"))?,input.started_at.to_rfc3339()
                ],
            )?;
            attempt(db, input.attempt_id)?.ok_or_else(|| invalid("attempt insert failed"))
        })
    }

    pub fn finish_task_model_attempt(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        attempt_id: Uuid,
        finish: &FinishTaskModelAttempt,
    ) -> Result<TaskModelAttempt, StorageError> {
        if finish.status == TaskModelAttemptStatus::Started {
            return Err(invalid("terminal attempt status is required"));
        }
        self.with_connection(|db| {
            let saved = attempt(db, attempt_id)?.ok_or_else(|| invalid("attempt not found"))?;
            if saved.project_id != project || saved.conversation_id != conversation || saved.task_id != task {
                return Err(invalid("model attempt does not belong to this Task"));
            }
            let calculated = cost(&saved.pricing_snapshot.pricing, &finish.usage, finish.cached_input_tokens, saved.image_count);
            let currency = calculated.map(|_| saved.pricing_snapshot.pricing.currency.clone());
            if saved.status != TaskModelAttemptStatus::Started {
                let expected = (
                    finish.status, &finish.request_id, finish.usage.input_tokens,
                    finish.cached_input_tokens, finish.usage.output_tokens, finish.usage.source,
                    calculated.map(|value| value.normalize().to_string()), currency.as_ref(),
                    Some(finish.completed_at), &finish.failure,
                );
                let actual = (
                    saved.status, &saved.request_id, saved.input_tokens,
                    saved.cached_input_tokens, saved.output_tokens, saved.usage_source,
                    saved.cost.clone(), saved.currency.as_ref(), saved.completed_at, &saved.failure,
                );
                if expected != actual {
                    return Err(invalid("model attempt terminal evidence is immutable"));
                }
                return Ok(saved);
            }
            let duration = finish
                .completed_at
                .signed_duration_since(saved.started_at)
                .num_milliseconds()
                .max(0) as u64;
            let input_tokens = finish
                .usage
                .input_tokens
                .map(i64::try_from)
                .transpose()
                .map_err(|_| invalid("input token count exceeds SQLite range"))?;
            let cached_input_tokens = finish
                .cached_input_tokens
                .map(i64::try_from)
                .transpose()
                .map_err(|_| invalid("cached token count exceeds SQLite range"))?;
            let output_tokens = finish
                .usage
                .output_tokens
                .map(i64::try_from)
                .transpose()
                .map_err(|_| invalid("output token count exceeds SQLite range"))?;
            let duration = i64::try_from(duration)
                .map_err(|_| invalid("attempt duration exceeds SQLite range"))?;
            db.execute(
                "UPDATE task_model_attempts SET status=?2,request_id=?3,input_tokens=?4,cached_input_tokens=?5,output_tokens=?6,usage_source=?7,cost=?8,currency=?9,completed_at=?10,duration_ms=?11,failure_json=?12 WHERE attempt_id=?1 AND status='started'",
                params![attempt_id.to_string(),status_name(finish.status),finish.request_id,
                    input_tokens,cached_input_tokens,output_tokens,
                    usage_source_name(finish.usage.source),calculated.map(|value|value.normalize().to_string()),
                    currency,finish.completed_at.to_rfc3339(),duration,
                    finish.failure.as_ref().map(serde_json::to_string).transpose()?],
            )?;
            attempt(db, attempt_id)?.ok_or_else(|| invalid("attempt settlement failed"))
        })
    }

    pub fn task_model_usage(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        cursor: i64,
        limit: u32,
    ) -> Result<Value, StorageError> {
        if cursor < 0 || !(1..=100).contains(&limit) {
            return Err(invalid(
                "attempt cursor must be nonnegative and limit must be 1..100",
            ));
        }
        self.with_connection(|db| {
            let owned: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.id=?2 AND c.project_id=?3)",
                params![task.to_string(),conversation.to_string(),project],|row|row.get(0))?;
            if !owned { return Err(invalid("Task does not belong to this Project")); }
            let all = attempts_for_task(db, task, 0, u32::MAX)?;
            let mut costs = std::collections::BTreeMap::<String, Decimal>::new();
            let mut input_tokens=0_u64; let mut cached_input_tokens=0_u64; let mut output_tokens=0_u64;
            let mut unknown=0_u64; let mut token_unknown=0_u64;
            for item in &all {
                input_tokens=input_tokens.saturating_add(item.input_tokens.unwrap_or(0));
                cached_input_tokens=cached_input_tokens.saturating_add(item.cached_input_tokens.unwrap_or(0));
                output_tokens=output_tokens.saturating_add(item.output_tokens.unwrap_or(0));
                if item.input_tokens.is_none()||item.cached_input_tokens.is_none()||item.output_tokens.is_none(){token_unknown=token_unknown.saturating_add(1);}
                if let (Some(cost),Some(currency))=(&item.cost,&item.currency) {
                    if let Ok(value)=cost.parse::<Decimal>() { *costs.entry(currency.clone()).or_default() += value; }
                } else { unknown=unknown.saturating_add(1); }
            }
            let state=if all.is_empty(){"no_model_requests"}else if all.iter().any(|a|matches!(a.status,TaskModelAttemptStatus::Started|TaskModelAttemptStatus::InDoubt)){"unknown"}else if all.iter().all(|a|a.status==TaskModelAttemptStatus::Succeeded)&&unknown==0&&token_unknown==0{"complete"}else{"partial"};
            let page=attempts_for_task(db,task,cursor,limit.saturating_add(1))?;
            let more=page.len()>limit as usize;
            let items=page.into_iter().take(limit as usize).collect::<Vec<_>>();
            let next_cursor=more.then(||items.last().map(|item|item.sequence)).flatten();
            let buckets=costs.iter().map(|(currency,cost)|json!({"currency":currency,"cost":cost.normalize().to_string()})).collect::<Vec<_>>();
            let (known_cost,currency)=if costs.len()==1&&unknown==0 { let (currency,cost)=costs.iter().next().unwrap();(Some(cost.normalize().to_string()),Some(currency.clone())) } else {(None,None)};
            Ok(json!({"scope":{"project_id":project,"conversation_id":conversation,"task_id":task},"state":state,
                "summary":{"attempt_count":all.len(),"known_cost":known_cost,"currency":currency,"costs_by_currency":buckets,"input_tokens":if all.is_empty()||all.iter().all(|a|a.input_tokens.is_some()){Some(input_tokens)}else{None},"cached_input_tokens":if all.is_empty()||all.iter().all(|a|a.cached_input_tokens.is_some()){Some(cached_input_tokens)}else{None},"output_tokens":if all.is_empty()||all.iter().all(|a|a.output_tokens.is_some()){Some(output_tokens)}else{None},"token_unknown_attempt_count":token_unknown,"unknown_cost_attempt_count":unknown},
                "attempts":{"items":items,"next_cursor":next_cursor}}))
        })
    }

    pub fn task_model_attempt(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        attempt_id: Uuid,
    ) -> Result<TaskModelAttempt, StorageError> {
        self.with_connection(|db| {
            let saved = attempt(db, attempt_id)?.ok_or_else(|| invalid("attempt not found"))?;
            if saved.project_id != project
                || saved.conversation_id != conversation
                || saved.task_id != task
            {
                return Err(invalid("attempt does not belong to this Task"));
            }
            Ok(saved)
        })
    }
}

fn attempts_for_task(
    db: &rusqlite::Connection,
    task: Uuid,
    cursor: i64,
    limit: u32,
) -> Result<Vec<TaskModelAttempt>, StorageError> {
    let mut statement=db.prepare("SELECT sequence,attempt_id,project_id,conversation_id,task_id,call_id,attempt_number,kind,status,model_profile_id,model_profile_revision,provider_id,provider_name,request_id,input_tokens,cached_input_tokens,output_tokens,image_count,usage_source,cost,currency,pricing_snapshot_json,effective_request_json,started_at,completed_at,duration_ms,failure_json FROM task_model_attempts WHERE task_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
    statement
        .query_map(params![task.to_string(), cursor, limit], decode)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn attempt(db: &rusqlite::Connection, id: Uuid) -> Result<Option<TaskModelAttempt>, StorageError> {
    db.query_row("SELECT sequence,attempt_id,project_id,conversation_id,task_id,call_id,attempt_number,kind,status,model_profile_id,model_profile_revision,provider_id,provider_name,request_id,input_tokens,cached_input_tokens,output_tokens,image_count,usage_source,cost,currency,pricing_snapshot_json,effective_request_json,started_at,completed_at,duration_ms,failure_json FROM task_model_attempts WHERE attempt_id=?1",[id.to_string()],decode).optional().map_err(StorageError::from)
}

fn decode(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelAttempt> {
    fn invalid_column(error: impl std::fmt::Display) -> rusqlite::Error {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    }
    let parse_uuid = |index| Uuid::parse_str(&row.get::<_, String>(index)?).map_err(invalid_column);
    let u64_value = |index| {
        row.get::<_, Option<i64>>(index)
            .map(|value| value.map(|v| v.max(0) as u64))
    };
    let pricing: String = row.get(21)?;
    let effective: String = row.get(22)?;
    let started: String = row.get(23)?;
    let completed: Option<String> = row.get(24)?;
    let failure: Option<String> = row.get(26)?;
    Ok(TaskModelAttempt {
        sequence: row.get(0)?,
        attempt_id: parse_uuid(1)?,
        project_id: row.get(2)?,
        conversation_id: parse_uuid(3)?,
        task_id: parse_uuid(4)?,
        call_id: parse_uuid(5)?,
        attempt_number: row.get(6)?,
        kind: parse_kind(&row.get::<_, String>(7)?).map_err(invalid_column)?,
        status: parse_status(&row.get::<_, String>(8)?).map_err(invalid_column)?,
        model_profile_id: row.get::<_, String>(9)?.parse().map_err(invalid_column)?,
        model_profile_revision: row.get::<_, i64>(10)?.max(0) as u64,
        provider_id: row.get::<_, String>(11)?.parse().map_err(invalid_column)?,
        provider_name: row.get(12)?,
        request_id: row.get(13)?,
        input_tokens: u64_value(14)?,
        cached_input_tokens: u64_value(15)?,
        output_tokens: u64_value(16)?,
        image_count: row.get::<_, i64>(17)?.max(0) as u64,
        usage_source: parse_usage_source(&row.get::<_, String>(18)?).map_err(invalid_column)?,
        cost: row.get(19)?,
        currency: row.get(20)?,
        pricing_snapshot: serde_json::from_str(&pricing).map_err(invalid_column)?,
        effective_request: serde_json::from_str(&effective).map_err(invalid_column)?,
        started_at: DateTime::parse_from_rfc3339(&started)
            .map_err(invalid_column)?
            .with_timezone(&Utc),
        completed_at: completed
            .map(|v| {
                DateTime::parse_from_rfc3339(&v)
                    .map(|v| v.with_timezone(&Utc))
                    .map_err(invalid_column)
            })
            .transpose()?,
        duration_ms: u64_value(25)?,
        failure: failure
            .map(|v| serde_json::from_str(&v).map_err(invalid_column))
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginConversationTask, ConversationCallGrant, ConversationMessageInput};
    use annotagent_core::{
        CapabilityDeclarationSource, GenerationDefaults, InputModality, ModelCapability,
        ModelLimits, ModelProfileStatus, PricingSource, ProtocolFeatures, ProviderAdapterKind,
        ProviderConnectionPolicy, ProviderHealthSnapshot, ProviderHealthStatus,
    };
    use chrono::Duration;
    use std::collections::{BTreeMap, BTreeSet};
    use url::Url;

    fn setup(store: &SqliteStore) -> (String, Uuid, Uuid) {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST usage".to_owned(),
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
                &BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        (project, conversation, task)
    }

    fn profile(currency: &str) -> (ProviderProfile, ModelProfile) {
        let now = Utc::now();
        let provider = ProviderProfile {
            id: annotagent_core::ProviderId::new(),
            display_name: format!("TEST {currency} Provider"),
            preset_id: Some("TEST".to_owned()),
            adapter: ProviderAdapterKind::Mock,
            base_url: Url::parse("http://127.0.0.1:1").unwrap(),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::default(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("TEST only".to_owned()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let model = ModelProfile {
            id: annotagent_core::ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST priced model".to_owned(),
            remote_model_id: "TEST-model".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text]),
            protocol_features: ProtocolFeatures {
                usage_reporting: true,
                ..Default::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing {
                currency: currency.to_owned(),
                input_per_million_tokens: Some(Decimal::from(2)),
                output_per_million_tokens: Some(Decimal::from(8)),
                cached_input_per_million_tokens: Some(Decimal::ONE),
                per_image: None,
                per_request: None,
                source: PricingSource::UserConfigured,
                updated_at: Some(now),
            },
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        (provider, model)
    }

    fn reserve(store: &SqliteStore, project: &str, task: Uuid, maximum: u32) -> Uuid {
        let call = Uuid::new_v4();
        let grant = ConversationCallGrant {
            id: call,
            task_id: task,
            scope_hash: "b".repeat(64),
            maximum_calls: maximum,
            expires_at: Utc::now() + Duration::hours(1),
        };
        store.authorize_conversation_calls(project, &grant).unwrap();
        assert!(matches!(
            store
                .reserve_conversation_call(project, task, call, &grant.scope_hash, &"c".repeat(64))
                .unwrap(),
            crate::ConversationCallAdmission::Admitted
        ));
        call
    }

    #[test]
    fn physical_attempt_cost_snapshot_unknown_owner_and_pagination() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (project, conversation, task) = setup(&store);
        let empty = store
            .task_model_usage(&project, conversation, task, 0, 50)
            .unwrap();
        assert_eq!(empty["state"], "no_model_requests");
        assert_eq!(empty["summary"]["attempt_count"], 0);

        let (provider, mut model) = profile("USD");
        let call = reserve(&store, &project, task, 3);
        let started = Utc::now();
        let id = Uuid::new_v4();
        let begin = BeginTaskModelAttempt {
            attempt_id: id,
            call_id: call,
            attempt_number: 1,
            kind: TaskModelAttemptKind::Task,
            model: &model,
            provider: &provider,
            effective_request: &json!({"snapshot_sha256":"d".repeat(64)}),
            image_count: 0,
            started_at: started,
        };
        assert_eq!(
            store
                .begin_task_model_attempt(&project, conversation, task, &begin)
                .unwrap()
                .status,
            TaskModelAttemptStatus::Started
        );
        assert_eq!(
            store
                .begin_task_model_attempt(&project, conversation, task, &begin)
                .unwrap()
                .attempt_id,
            id
        );
        let completed = store
            .finish_task_model_attempt(
                &project,
                conversation,
                task,
                id,
                &FinishTaskModelAttempt {
                    status: TaskModelAttemptStatus::Succeeded,
                    request_id: Some("TEST-request".to_owned()),
                    usage: TokenUsage::known(1_500, 500, UsageSource::Actual),
                    cached_input_tokens: Some(0),
                    completed_at: started + Duration::milliseconds(4),
                    failure: None,
                },
            )
            .unwrap();
        assert_eq!(completed.cost.as_deref(), Some("0.007"));
        assert_eq!(completed.currency.as_deref(), Some("USD"));

        let frozen_model = model.clone();
        model.pricing.input_per_million_tokens = Some(Decimal::from(200));
        let after_price_edit = store
            .task_model_usage(&project, conversation, task, 0, 1)
            .unwrap();
        assert_eq!(after_price_edit["state"], "complete");
        assert_eq!(after_price_edit["summary"]["known_cost"], "0.007");
        assert_eq!(
            after_price_edit["attempts"]["items"][0]["pricing_snapshot"]["pricing"]["input_per_million_tokens"],
            "2"
        );
        assert!(
            store
                .task_model_usage("foreign", conversation, task, 0, 50)
                .is_err()
        );
        assert!(
            store
                .task_model_attempt("foreign", conversation, task, id)
                .is_err()
        );

        let id2 = Uuid::new_v4();
        store
            .begin_task_model_attempt(
                &project,
                conversation,
                task,
                &BeginTaskModelAttempt {
                    attempt_id: id2,
                    call_id: call,
                    attempt_number: 2,
                    kind: TaskModelAttemptKind::Task,
                    model: &frozen_model,
                    provider: &provider,
                    effective_request: &json!({"snapshot_sha256":"d".repeat(64)}),
                    image_count: 0,
                    started_at: Utc::now(),
                },
            )
            .unwrap();
        let unknown = store
            .task_model_usage(&project, conversation, task, 0, 1)
            .unwrap();
        assert_eq!(unknown["state"], "unknown");
        assert_eq!(unknown["attempts"]["items"].as_array().unwrap().len(), 1);
        assert!(unknown["attempts"]["next_cursor"].as_i64().is_some());
    }
}
