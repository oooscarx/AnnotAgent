//! Future-only human annotation semantics, branched from the exact tested Schema.
use crate::{
    ConversationFeedbackAuthorizationRecord, ConversationMessage, ConversationSchemaDefinition,
    ConversationSelectionRef, SqliteStore, StorageError, WorkflowSampleTestInput,
};
use annotagent_core::{AttributeKind, TaskKind, WorkflowSchemaBinding};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFutureSchemaInput {
    pub command_id: Uuid,
    pub feedback_call_id: Uuid,
    pub scope_answer_command_id: Uuid,
    pub context_digest: String,
    pub base_schema_id: Uuid,
    pub base_schema_revision: u64,
    pub definition: ConversationSchemaDefinition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_call_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFutureSchemaRecord {
    pub input: ConversationFutureSchemaInput,
    pub schema_id: Uuid,
    pub created_at: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

fn read(
    db: &Connection,
    conversation: Uuid,
    task: Uuid,
    call: Uuid,
) -> Result<Option<ConversationFutureSchemaRecord>, StorageError> {
    let row:Option<(String,String,String,String,String)>=db.query_row("SELECT task_id,conversation_id,input_json,schema_id,created_at FROM conversation_future_schema_drafts WHERE feedback_call_id=?1",[call.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    row.map(|(owner, conv, input, schema_id, created_at)| {
        if owner != task.to_string() || conv != conversation.to_string() {
            return Err(invalid(
                "Future Schema belongs to another conversation task",
            ));
        }
        Ok(ConversationFutureSchemaRecord {
            input: serde_json::from_str(&input)?,
            schema_id: Uuid::parse_str(&schema_id)
                .map_err(|_| invalid("Invalid saved future Schema ID"))?,
            created_at,
        })
    })
    .transpose()
}

fn bounded(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

struct TestedSample {
    project: String,
    draft: String,
    revision: i64,
    hash: String,
    images: String,
    request: String,
    operation_project: String,
    operation_draft: String,
}

/// Defense in depth for the Application's bounded human Schema constructor.
pub(crate) fn validate_definition(
    definition: &ConversationSchemaDefinition,
    base: &ConversationSchemaDefinition,
) -> Result<(), StorageError> {
    let task = &definition.task;
    if !bounded(&definition.goal, 16_000)
        || serde_json::to_vec(definition)?.len() > 65_536
        || task.id != base.task.id
        || !matches!(task.kind, TaskKind::BoundingBox | TaskKind::Classification)
        || !task.required
        || !task.depends_on.is_empty()
        || !task.validators.is_empty()
        || !task.refiners.is_empty()
        || task.target_task.is_some()
        || !task.target_labels.is_empty()
        || task
            .display_name
            .as_ref()
            .is_some_and(|name| !bounded(name, 128))
        || task.labels.is_empty()
        || task.labels.len() > 32
        || task
            .labels
            .iter()
            .any(|label| !bounded(label, 128) || label != label.trim())
        || task.labels.iter().collect::<BTreeSet<_>>().len() != task.labels.len()
        || definition.boundary_rules.len() > 16
        || definition
            .boundary_rules
            .iter()
            .any(|rule| !bounded(rule, 1_000))
        || task.attributes.len() > 16
    {
        return Err(invalid(
            "Future Schema must retain the task identity and bounded annotation semantics only",
        ));
    }
    for (name, attribute) in &task.attributes {
        if !bounded(name, 128)
            || (attribute.kind == AttributeKind::Enum
                && (attribute.values.is_empty()
                    || attribute.values.len() > 32
                    || attribute.values.iter().any(|value| !bounded(value, 128))
                    || attribute.values.iter().collect::<BTreeSet<_>>().len()
                        != attribute.values.len()))
            || (attribute.kind != AttributeKind::Enum && !attribute.values.is_empty())
        {
            return Err(invalid("Invalid future Schema attribute definition"));
        }
    }
    Ok(())
}

/// Resolve only the exact tested binding in the saved message's Sample seal.
/// Never use the current selection, another draft's latest Schema, or mutate the test.
pub(crate) fn validate_tested_base(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    record: &ConversationFeedbackAuthorizationRecord,
    base_schema_id: Uuid,
    base_schema_revision: u64,
) -> Result<crate::ConversationSchemaDraft, StorageError> {
    let message: ConversationMessage = serde_json::from_value(record.context["message"].clone())?;
    let Some(ConversationSelectionRef::SampleCandidate {
        sample_test_id,
        draft_id,
        draft_revision,
        ..
    }) = &message.input.reference
    else {
        return Err(invalid(
            "Future Schema requires a saved candidate's tested Schema",
        ));
    };
    let sample:Option<TestedSample>=db.query_row(
        "SELECT s.project_id,s.draft_id,s.draft_revision,s.draft_content_hash,s.input_json,o.request_json,o.project_id,o.draft_id FROM workflow_sample_tests s JOIN sample_operations o ON o.id=s.id WHERE s.id=?1",
        [sample_test_id],|r|Ok(TestedSample {project:r.get(0)?,draft:r.get(1)?,revision:r.get(2)?,hash:r.get(3)?,images:r.get(4)?,request:r.get(5)?,operation_project:r.get(6)?,operation_draft:r.get(7)?}),
    ).optional()?;
    let sample = sample
        .ok_or_else(|| invalid("Future Schema requires the original saved Sample Operation"))?;
    let request: serde_json::Value = serde_json::from_str(&sample.request)?;
    let images: Vec<WorkflowSampleTestInput> = serde_json::from_str(&sample.images)?;
    let image = message
        .input
        .image
        .as_ref()
        .ok_or_else(|| invalid("Future Schema message has no frozen image"))?;
    if sample.draft != *draft_id
        || u64::try_from(sample.revision).ok() != Some(*draft_revision)
        || record.context["sample_content_hash"] != sample.hash
        || sample.operation_project != sample.project
        || sample.operation_draft != sample.draft
        || request["conversation"]["conversation_id"] != conversation.to_string()
        || request["conversation"]["task_id"] != task.to_string()
        || images
            .iter()
            .filter(|saved| saved.image_id == image.image_id && saved.content_hash == image.sha256)
            .count()
            != 1
    {
        return Err(invalid(
            "Future Schema sample differs from the frozen conversation reference",
        ));
    }
    let seal: Option<String> = db
        .query_row(
            "SELECT scope_json FROM sample_scope_seals WHERE sample_test_id=?1",
            [sample_test_id],
            |r| r.get(0),
        )
        .optional()?;
    let seal: serde_json::Value = serde_json::from_str(&seal.ok_or_else(|| {
        invalid("Future Schema requires a sealed tested Schema; run a new sample first")
    })?)?;
    let binding: WorkflowSchemaBinding = serde_json::from_value(seal["annotation_schema"].clone())?;
    if binding.schema_draft_id != base_schema_id.to_string()
        || binding.revision != base_schema_revision
    {
        return Err(invalid(
            "Future Schema base is not the exact Schema tested in this sample",
        ));
    }
    let base =
        crate::conversation_schema::read(db, project, base_schema_id, Some(base_schema_revision))?;
    let head = crate::conversation_schema::read(db, project, base_schema_id, None)?;
    if base.task_id != task
        || head.revision != base_schema_revision
        || binding.task != base.definition.task
        || binding.goal != base.definition.goal
        || binding.boundary_rules != base.definition.boundary_rules
    {
        return Err(invalid(
            "Tested Schema binding changed or is no longer its head revision",
        ));
    }
    Ok(base)
}

impl SqliteStore {
    pub fn future_schema_draft(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFutureSchemaRecord>, StorageError> {
        self.with_connection(|db| {
            let conversation = crate::conversation_feedback::owned(db, project, task)?;
            read(db, conversation, task, call)
        })
    }
    /// Explicit local human intent only: creates one independent Schema at revision 1.
    /// Existing exact commands restore their link, never replace later human edits.
    pub fn create_future_schema_draft(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &ConversationFutureSchemaInput,
    ) -> Result<ConversationFutureSchemaRecord, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            if crate::conversation_feedback::owned(&tx,project,task)?!=conversation {return Err(invalid("Future Schema task belongs to another conversation"));}
            if let Some(saved)=read(&tx,conversation,task,input.feedback_call_id)? {
                if saved.input!=*input {return Err(invalid("Future Schema command conflicts with its saved human input"));}
                return Ok(saved);
            }
            if input.command_id.is_nil() || input.feedback_call_id.is_nil() || input.scope_answer_command_id.is_nil() || input.base_schema_id.is_nil() || input.base_schema_revision==0
                || input.context_digest.len()!=64 || !input.context_digest.bytes().all(|b|b.is_ascii_hexdigit()) {
                return Err(invalid("Future Schema requires stable source IDs and an exact context digest"));
            }
            let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_future_schema_drafts WHERE command_id=?1)",[input.command_id.to_string()],|r|r.get(0))?;
            if collision {return Err(invalid("Future Schema command already belongs to another source"));}
            let source=crate::ConversationFutureSchemaProposalSource{feedback_call_id:input.feedback_call_id,scope_answer_command_id:input.scope_answer_command_id,context_digest:input.context_digest.clone(),base_schema_id:input.base_schema_id,base_schema_revision:input.base_schema_revision};
            let (_,base)=crate::conversation_future_schema_proposal::validate_source(&tx,project,conversation,task,&source)?;
            validate_definition(&input.definition,&base.definition)?;
            crate::conversation_future_schema_proposal::validate_provenance(&tx,project,conversation,task,input)?;
            let schema_id=crate::conversation_schema::insert_human_schema(&tx,task,input.command_id,&input.definition)?;
            let saved=ConversationFutureSchemaRecord {input:input.clone(),schema_id,created_at:chrono::Utc::now().to_rfc3339()};
            tx.execute("INSERT INTO conversation_future_schema_drafts(feedback_call_id,task_id,conversation_id,command_id,schema_id,input_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![input.feedback_call_id.to_string(),task.to_string(),conversation.to_string(),input.command_id.to_string(),schema_id.to_string(),serde_json::to_string(input)?,saved.created_at])?;
            tx.commit()?;Ok(saved)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::*;
    use serde_json::json;

    pub(crate) struct Fixture {
        pub(crate) source: crate::conversation_feedback_scope::tests::Fixture,
        pub(crate) input: ConversationFutureSchemaInput,
        pub(crate) base: ConversationSchemaDraft,
    }

    pub(crate) fn fixture(store: &SqliteStore) -> Fixture {
        let mut source = crate::conversation_feedback_scope::tests::fixture(store, "bounding_box");
        source.answer.choice = ConversationFeedbackScopeChoice::ProjectFutureRule;
        store
            .answer_conversation_feedback_scope(
                &source.project,
                source.conversation,
                source.task,
                source.source.id,
                &source.answer,
                &source.source,
            )
            .unwrap();
        let definition = ConversationSchemaDefinition {
            goal: "TEST annotate cups and bottles".into(),
            task: serde_json::from_value(json!({"id":format!("annotation_{}",source.task.simple()),"display_name":"Annotation goal","kind":"bounding_box","labels":["cup","bottle"],"required":true})).unwrap(),
            boundary_rules: vec![],
        };
        let base = store
            .create_human_conversation_schema_draft(
                &source.project,
                source.task,
                Uuid::new_v4(),
                &definition,
            )
            .unwrap();
        let binding = annotagent_core::WorkflowSchemaBinding {
            schema_draft_id: base.id.to_string(),
            revision: base.revision,
            goal: definition.goal.clone(),
            task: definition.task.clone(),
            boundary_rules: definition.boundary_rules.clone(),
        };
        store.with_connection(|db| {
            db.execute("INSERT INTO sample_scope_seals(sample_test_id,scope_json) VALUES('TEST-scoped-sample',?1)",[serde_json::to_string(&json!({"annotation_schema":binding}))?])?;
            db.execute("UPDATE sample_operations SET status='succeeded' WHERE id='TEST-scoped-sample'",[])?;
            Ok(())
        }).unwrap();
        let mut definition = definition;
        definition.goal = "TEST only annotate cups, including partially occluded cups".into();
        definition.task.labels = vec!["cup".into()];
        definition.boundary_rules =
            vec!["TEST include partially occluded cups; annotate visible extent only".into()];
        let input = ConversationFutureSchemaInput {
            command_id: Uuid::new_v4(),
            feedback_call_id: source.source.id,
            scope_answer_command_id: source.answer.command_id,
            context_digest: source.answer.expected_context_digest.clone(),
            base_schema_id: base.id,
            base_schema_revision: base.revision,
            definition,
            proposal_call_id: None,
            proposal_digest: None,
        };
        Fixture {
            source,
            input,
            base,
        }
    }

    impl Fixture {
        fn save(
            &self,
            store: &SqliteStore,
        ) -> Result<ConversationFutureSchemaRecord, StorageError> {
            store.create_future_schema_draft(
                &self.source.project,
                self.source.conversation,
                self.source.task,
                &self.input,
            )
        }
        fn counts(store: &SqliteStore) -> Vec<i64> {
            store
                .with_connection(|db| {
                    [
                        "conversation_schema_drafts",
                        "conversation_schema_revisions",
                        "conversation_future_schema_drafts",
                        "sample_feedback_revisions",
                        "conversation_human_requests",
                        "conversation_resume_results",
                        "annotations",
                        "workflow_drafts",
                    ]
                    .iter()
                    .map(|name| {
                        db.query_row(&format!("SELECT COUNT(*) FROM {name}"), [], |r| r.get(0))
                            .map_err(Into::into)
                    })
                    .collect()
                })
                .unwrap()
        }
    }

    #[test]
    fn future_schema_branches_only_semantics_and_restores_after_reopen_and_human_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-future.db");
        let store = SqliteStore::open(&path).unwrap();
        let f = fixture(&store);
        let budget = store
            .conversation_call_budget(&f.source.project, f.source.task)
            .unwrap();
        let sample = store
            .get_workflow_sample_test_by_id("TEST-scoped-sample")
            .unwrap();
        let counts = Fixture::counts(&store);
        let saved = f.save(&store).unwrap();
        assert_ne!(saved.schema_id, f.base.id);
        let child = store
            .conversation_schema_draft(&f.source.project, saved.schema_id, None)
            .unwrap();
        assert_eq!(child.revision, 1);
        assert_eq!(child.definition, f.input.definition);
        assert_eq!(child.task_id, f.base.task_id);
        assert_eq!(child.definition.task.id, f.base.definition.task.id);
        assert_eq!(child.source_call_id, None);
        assert_eq!(child.source_request_id, Some(f.input.command_id));
        assert_eq!(
            store
                .human_conversation_schema_drafts(&f.source.project, f.source.task)
                .unwrap(),
            vec![f.base.clone()]
        );
        let mut expected = counts;
        expected[0] += 1;
        expected[1] += 1;
        expected[2] += 1;
        assert_eq!(Fixture::counts(&store), expected);
        assert_eq!(
            store
                .conversation_schema_draft(&f.source.project, f.base.id, None)
                .unwrap(),
            f.base
        );
        assert_eq!(
            store
                .conversation_call_budget(&f.source.project, f.source.task)
                .unwrap(),
            budget
        );
        assert_eq!(
            store
                .get_workflow_sample_test_by_id("TEST-scoped-sample")
                .unwrap(),
            sample
        );
        assert_eq!(
            store
                .conversation_call_history(&f.source.project, f.source.task)
                .unwrap(),
            vec![f.source.source.clone()]
        );
        let mut edited = child.definition.clone();
        edited.boundary_rules.push("TEST later human edit".into());
        let newer = store
            .revise_conversation_schema_draft(
                &f.source.project,
                child.id,
                Uuid::new_v4(),
                1,
                &edited,
            )
            .unwrap();
        store
            .request_conversation_call_cancel(
                &f.source.project,
                f.source.task,
                f.input.feedback_call_id,
            )
            .unwrap();
        store
            .with_connection(|db| {
                db.execute(
                    "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                    [&f.source.project],
                )?;
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(f.save(&store).unwrap(), saved);
        assert_eq!(
            store
                .future_schema_draft(&f.source.project, f.source.task, f.input.feedback_call_id)
                .unwrap(),
            Some(saved)
        );
        assert_eq!(
            store
                .conversation_schema_draft(&f.source.project, child.id, None)
                .unwrap(),
            newer
        );
        assert_eq!(
            store
                .human_conversation_schema_drafts(&f.source.project, f.source.task)
                .unwrap(),
            vec![f.base]
        );
    }

    #[test]
    fn future_schema_migration_preserves_ordinary_human_schema_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-future-migration.db");
        let store = SqliteStore::open(&path).unwrap();
        let f = fixture(&store);
        let another = store
            .create_human_conversation_schema_draft(
                &f.source.project,
                f.source.task,
                Uuid::new_v4(),
                &f.base.definition,
            )
            .unwrap();
        let baseline = store
            .human_conversation_schema_drafts(&f.source.project, f.source.task)
            .unwrap();
        assert_eq!(baseline, vec![another, f.base.clone()]);
        // Restore the pre-46 shape in this isolated fixture; no future link exists.
        store.with_connection(|db|{db.execute_batch("DROP TABLE conversation_future_schema_drafts; DELETE FROM schema_migrations WHERE version=46;")?;Ok(())}).unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .human_conversation_schema_drafts(&f.source.project, f.source.task)
                .unwrap(),
            baseline
        );
        let saved = f.save(&store).unwrap();
        assert_eq!(
            store
                .human_conversation_schema_drafts(&f.source.project, f.source.task)
                .unwrap(),
            baseline
        );
        assert_eq!(
            store
                .future_schema_draft(&f.source.project, f.source.task, f.input.feedback_call_id)
                .unwrap(),
            Some(saved.clone())
        );
        assert_eq!(
            store
                .conversation_schema_draft(&f.source.project, saved.schema_id, Some(1))
                .unwrap()
                .definition,
            f.input.definition
        );
    }

    #[test]
    fn future_schema_rejects_invalid_or_stale_sources_without_any_write() {
        for variant in 0..24 {
            let store = SqliteStore::open_in_memory().unwrap();
            let mut f = fixture(&store);
            match variant {
                0 => f.source.conversation = Uuid::new_v4(),
                1 => f.input.scope_answer_command_id = Uuid::new_v4(),
                2 => f.input.context_digest = "0".repeat(64),
                3 => {
                    store
                        .request_conversation_call_cancel(
                            &f.source.project,
                            f.source.task,
                            f.input.feedback_call_id,
                        )
                        .unwrap();
                }
                4 => {
                    store.with_connection(|db|{db.execute("UPDATE conversation_feedback_scope_answers SET input_json=json_set(input_json,'$.choice',json('{\"scope\":\"current_image_class\"}'))",[])?;Ok(())}).unwrap();
                }
                5 => {
                    store
                        .with_connection(|db| {
                            db.execute("DELETE FROM sample_scope_seals", [])?;
                            Ok(())
                        })
                        .unwrap();
                }
                6 => {
                    store.with_connection(|db|{db.execute("UPDATE sample_scope_seals SET scope_json=json_set(scope_json,'$.annotation_schema.goal','TEST tampered')",[])?;Ok(())}).unwrap();
                }
                7 => {
                    store
                        .revise_conversation_schema_draft(
                            &f.source.project,
                            f.base.id,
                            Uuid::new_v4(),
                            1,
                            &f.base.definition,
                        )
                        .unwrap();
                }
                8 => {
                    store
                        .with_connection(|db| {
                            db.execute("UPDATE images SET sha256='TEST changed'", [])?;
                            Ok(())
                        })
                        .unwrap();
                }
                9 => {
                    store.with_connection(|db|{db.execute("UPDATE conversation_messages SET input_json=json_set(input_json,'$.text','TEST altered') WHERE message_id=?1",[f.source.record.consent.message_id.to_string()])?;Ok(())}).unwrap();
                }
                10 => f.input.definition.task.id = "TEST foreign task".into(),
                11 => f
                    .input
                    .definition
                    .task
                    .validators
                    .push("TEST injected permission".into()),
                12 => f.input.definition.task.labels = vec!["cup".into(), "cup".into()],
                13 => {
                    store.with_connection(|db|{db.execute("UPDATE conversation_model_calls SET evidence_json=json_set(evidence_json,'$.response.tool_calls[0].arguments.decision','request_correction')",[])?;Ok(())}).unwrap();
                }
                14 => f.input.definition.goal = "x".repeat(16_001),
                15 => f.input.command_id = f.base.source_request_id.unwrap(),
                16 => {
                    store.with_connection(|db|{db.execute("UPDATE sample_operations SET request_json=json_set(request_json,'$.conversation.task_id',?1) WHERE id='TEST-scoped-sample'",[Uuid::new_v4().to_string()])?;Ok(())}).unwrap();
                }
                17 => {
                    store.with_connection(|db|{db.execute("UPDATE workflow_sample_tests SET draft_content_hash='TEST changed' WHERE id='TEST-scoped-sample'",[])?;Ok(())}).unwrap();
                }
                18 => {
                    store.with_connection(|db|{db.execute("UPDATE conversation_feedback_scope_answers SET input_json=json_set(input_json,'$.choice',json('{\"scope\":\"current_candidate\",\"reason\":\"wrong_target\"}'))",[])?;Ok(())}).unwrap();
                }
                19 => {
                    store
                        .with_connection(|db| {
                            db.execute("DELETE FROM conversation_feedback_scope_answers", [])?;
                            Ok(())
                        })
                        .unwrap();
                }
                20 => {
                    let message: ConversationMessage =
                        serde_json::from_value(f.source.record.context["message"].clone()).unwrap();
                    store
                        .save_sample_feedback(&SampleFeedbackRevision {
                            revision_id: "TEST-future-stale".into(),
                            sample_test_id: "TEST-scoped-sample".into(),
                            image_id: message.input.image.unwrap().image_id,
                            sequence: 1,
                            reason: SampleFeedbackReason::WrongTarget,
                            outcome_id: Some("final-1".into()),
                            corrected_value: None,
                            corrected_label: None,
                            addition_id: None,
                            note: "TEST human answer".into(),
                            created_at: chrono::Utc::now(),
                        })
                        .unwrap();
                }
                21 => {
                    let other = store
                        .create_human_conversation_schema_draft(
                            &f.source.project,
                            f.source.task,
                            Uuid::new_v4(),
                            &f.base.definition,
                        )
                        .unwrap();
                    f.input.base_schema_id = other.id;
                }
                22 => {
                    store.with_connection(|db|{db.execute("UPDATE conversation_model_calls SET evidence_json=json_set(evidence_json,'$.response.tool_calls[0].arguments.coordinates',json('[0,0,1,1]'))",[])?;Ok(())}).unwrap();
                }
                _ => f.input.command_id = Uuid::nil(),
            }
            let before = Fixture::counts(&store);
            assert!(f.save(&store).is_err(), "variant {variant}");
            assert_eq!(Fixture::counts(&store), before, "variant {variant}");
        }
    }

    #[test]
    fn future_schema_exact_retry_conflicts_and_link_failure_rolls_back_schema_insert() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut f = fixture(&store);
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_future_link_failure BEFORE INSERT ON conversation_future_schema_drafts BEGIN SELECT RAISE(ABORT,'TEST rollback'); END;")?;Ok(())}).unwrap();
        let before = Fixture::counts(&store);
        assert!(f.save(&store).is_err());
        assert_eq!(Fixture::counts(&store), before);
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER TEST_future_link_failure;")?;
                Ok(())
            })
            .unwrap();
        let saved = f.save(&store).unwrap();
        let after = Fixture::counts(&store);
        f.input.command_id = Uuid::new_v4();
        assert!(f.save(&store).is_err());
        f.input = saved.input;
        f.input.definition.task.labels.push("bottle".into());
        assert!(f.save(&store).is_err());
        assert_eq!(Fixture::counts(&store), after);
    }

    #[test]
    fn future_schema_contract_rejects_unknown_actions() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store);
        let mut value = serde_json::to_value(&f.input).unwrap();
        value["apply_to_existing_images"] = json!(true);
        assert!(serde_json::from_value::<ConversationFutureSchemaInput>(value).is_err());
    }

    #[test]
    fn future_schema_cancellation_and_creation_have_one_durable_order() {
        for _ in 0..4 {
            let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
            let f = fixture(&store);
            let worker_store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let ready = std::sync::Arc::clone(&barrier);
            let project = f.source.project.clone();
            let task = f.source.task;
            let call = f.input.feedback_call_id;
            let worker = std::thread::spawn(move || {
                ready.wait();
                worker_store.request_conversation_call_cancel(&project, task, call)
            });
            barrier.wait();
            let created = f.save(&store);
            worker.join().unwrap().unwrap();
            let restored = store
                .future_schema_draft(&f.source.project, task, call)
                .unwrap();
            if let Ok(saved) = created {
                assert_eq!(restored, Some(saved.clone()));
                assert_eq!(f.save(&store).unwrap(), saved);
                assert_eq!(Fixture::counts(&store)[0], 2);
            } else {
                assert!(restored.is_none());
                assert!(f.save(&store).is_err());
                assert_eq!(Fixture::counts(&store)[0], 1);
            }
        }
    }

    #[test]
    fn future_schema_concurrent_duplicates_restore_one_draft_and_expired_grants_do_not_spend() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-concurrent-future.db");
        let store = SqliteStore::open(&path).unwrap();
        let f = fixture(&store);
        store.with_connection(|db|{db.execute("UPDATE conversation_call_grants SET expires_at='2000-01-01T00:00:00Z' WHERE task_id=?1",[f.source.task.to_string()])?;Ok(())}).unwrap();
        let other = SqliteStore::open(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let ready = std::sync::Arc::clone(&barrier);
        let input = f.input.clone();
        let project = f.source.project.clone();
        let task = f.source.task;
        let conversation = f.source.conversation;
        let worker = std::thread::spawn(move || {
            ready.wait();
            other.create_future_schema_draft(&project, conversation, task, &input)
        });
        barrier.wait();
        let local = f.save(&store);
        let remote = worker.join().unwrap();
        assert!(local.is_ok() || remote.is_ok());
        let saved = f.save(&store).unwrap();
        for result in [local, remote].into_iter().flatten() {
            assert_eq!(result, saved);
        }
        assert_eq!(Fixture::counts(&store)[0], 2);
        assert_eq!(Fixture::counts(&store)[2], 1);
        assert_eq!(
            store
                .conversation_call_budget(&f.source.project, task)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
        assert!(
            store
                .future_schema_draft("TEST foreign project", task, f.input.feedback_call_id)
                .is_err()
        );
        let foreign = store.create_conversation(&f.source.project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST independent task".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&f.source.project, foreign, &message)
            .unwrap();
        let foreign_task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &f.source.project,
                foreign,
                &BeginConversationTask {
                    id: foreign_task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        assert!(
            store
                .future_schema_draft(&f.source.project, foreign_task, f.input.feedback_call_id)
                .is_err()
        );
        assert!(
            store
                .create_future_schema_draft(&f.source.project, foreign, foreign_task, &f.input)
                .is_err()
        );
    }

    #[test]
    fn future_schema_command_cannot_link_two_owned_feedback_sources() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store);
        let first = f.save(&store).unwrap();
        let mut auth = f.source.record.clone();
        let mut message: ConversationMessage =
            serde_json::from_value(auth.context["message"].clone()).unwrap();
        message.input.id = Uuid::new_v4();
        message.input.text = "TEST another future rule request".into();
        message = store
            .append_conversation_message(&f.source.project, f.source.conversation, &message.input)
            .unwrap();
        let call = Uuid::new_v4();
        auth.context["message"] = json!(message);
        auth.consent.previous_grant_id = Some(auth.grant.id);
        auth.consent.call_id = call;
        auth.consent.message_id = message.input.id;
        auth.consent.scope_hash = "d".repeat(64);
        auth.grant.id = call;
        auth.grant.scope_hash = auth.consent.scope_hash.clone();
        auth.grant.maximum_calls = 2;
        store
            .authorize_conversation_feedback(
                &f.source.project,
                f.source.conversation,
                f.source.task,
                &auth,
            )
            .unwrap();
        store
            .reserve_conversation_call(
                &f.source.project,
                f.source.task,
                call,
                &auth.consent.scope_hash,
                &"e".repeat(64),
            )
            .unwrap();
        let mut evidence = f.source.source.evidence.clone().unwrap();
        evidence["context"]["subject"] = auth.context.clone();
        evidence["context"]["scope_hash"] = json!(auth.consent.scope_hash);
        let source = store
            .finish_conversation_call(
                &f.source.project,
                f.source.task,
                call,
                ConversationCallStatus::Completed,
                evidence,
            )
            .unwrap();
        let answer = ConversationFeedbackScopeAnswerInput {
            command_id: Uuid::new_v4(),
            expected_context_digest: conversation_feedback_context_digest(&auth.context).unwrap(),
            choice: ConversationFeedbackScopeChoice::ProjectFutureRule,
        };
        store
            .answer_conversation_feedback_scope(
                &f.source.project,
                f.source.conversation,
                f.source.task,
                call,
                &answer,
                &source,
            )
            .unwrap();
        let mut input = f.input.clone();
        input.feedback_call_id = call;
        input.scope_answer_command_id = answer.command_id;
        input.context_digest = answer.expected_context_digest;
        let before = Fixture::counts(&store);
        assert!(
            store
                .create_future_schema_draft(
                    &f.source.project,
                    f.source.conversation,
                    f.source.task,
                    &input
                )
                .is_err()
        );
        assert_eq!(Fixture::counts(&store), before);
        assert_eq!(f.save(&store).unwrap(), first);
    }
}
