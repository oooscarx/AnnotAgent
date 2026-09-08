//! Server-derived data/Registry evidence for bounded conversation continuations.
//! No model invocation, publication, grant increment or Sample Operation creation.
use crate::LocalApplication;
use annotagent_core::{InputModality, ModelProfileId, ModelProfileSnapshot};
use annotagent_storage::{
    ConversationJourneyConsent, ConversationJourneyRecord, JourneyImageScope, JourneyModelScope,
    JourneySampleScope,
};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JourneyModelDescription {
    pub scope: JourneyModelScope,
    pub display_name: String,
    pub destination: String,
    pub permissions: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConversationJourneyDataScope {
    pub schema_id: Uuid,
    pub schema_revision: u64,
    pub schema_digest: String,
    pub images: Vec<JourneyImageScope>,
    pub models: Vec<JourneyModelDescription>,
}

impl LocalApplication {
    pub fn queue_conversation_journey_answer(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        schema: Uuid,
    ) -> Result<()> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .queue_conversation_journey_answer(&owner, conversation, task, id, schema)?)
    }
    pub fn resolve_initial_journey_schema(
        &self,
        project: &str,
        conversation: Uuid,
        resolved: &ConversationJourneyConsent,
    ) -> Result<ConversationJourneyRecord> {
        let original = self
            .conversation_journey_consent(project, conversation, resolved.task_id, resolved.id)?
            .ok_or_else(|| anyhow!("Journey consent not found"))?;
        self.validate_conversation_journey_data(project, conversation, &original.consent)?;
        self.validate_conversation_journey_data(project, conversation, resolved)?;
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .resolve_conversation_journey_schema(&owner, conversation, resolved)?)
    }
    pub fn conversation_journey_history(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<serde_json::Value>> {
        let owner = self.conversation_project_identity(project)?;
        self.store
            .conversation_journey_ids(&owner, conversation, task)?
            .into_iter()
            .map(|id| self.conversation_journey_execution_status(project, conversation, task, id))
            .collect()
    }
    pub fn conversation_journey_execution_status(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<serde_json::Value> {
        let record = self
            .conversation_journey_consent(project, conversation, task, id)?
            .ok_or_else(|| anyhow!("Journey consent not found"))?;
        let owner = self.conversation_project_identity(project)?;
        let builder = self.store.conversation_builder_operation(
            &owner,
            task,
            record.consent.builder_operation_id,
        )?;
        let sample = self
            .store
            .sample_operation(&record.consent.sample_operation_id.to_string())?;
        if sample.as_ref().is_some_and(|sample| {
            sample.project_id != project
                || sample.request["conversation"]["task_id"] != task.to_string()
                || sample.request["conversation"]["conversation_id"] != conversation.to_string()
        }) {
            bail!("Journey sample identity belongs to another task");
        }
        let dispatch = self
            .store
            .conversation_journey_dispatch(&owner, conversation, task, id)?;
        let schema = record
            .consent
            .schema_proposal
            .as_ref()
            .map(|proposal| {
                self.conversation_call_receipt(project, conversation, task, proposal.call_id)
            })
            .transpose()?
            .flatten();
        let mut sample_value = serde_json::json!(sample);
        let clarification = match schema.as_ref() {
            Some(receipt)
                if serde_json::to_value(receipt)?["evidence"]["decision"]["Ok"]["decision"]
                    == "clarify" =>
            {
                Some(self.schema_clarification(project, conversation, task, receipt.id)?)
            }
            _ => None,
        };
        if let Some(sample) = sample {
            sample_value["assistance"] =
                serde_json::json!(self.store.sample_assistance_status(&sample.id)?);
        }
        Ok(
            serde_json::json!({"record":record,"schema":schema,"clarification":clarification,"builder":builder,"sample":sample_value,"dispatch":dispatch}),
        )
    }

    pub fn claim_conversation_journey_dispatch(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        attempt: Uuid,
    ) -> Result<bool> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self.store.claim_conversation_journey_dispatch(
            &owner,
            conversation,
            task,
            id,
            attempt,
        )?)
    }

    pub fn require_active_conversation_journey(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationJourneyRecord> {
        let record = self
            .conversation_journey_consent(project, conversation, task, id)?
            .ok_or_else(|| anyhow!("Journey consent not found"))?;
        if record.revoked || record.consent.expires_at <= chrono::Utc::now() {
            bail!("Journey consent is revoked or expired");
        }
        Ok(record)
    }

    /// Historical consent reads never resolve models or trigger continuation.
    pub fn conversation_journey_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationJourneyRecord>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .conversation_journey(&owner, conversation, task, id)?)
    }

    /// Caller has validated the exact Builder preview and explicit acceptance.
    /// Original receipts are replayable even if a model later becomes unavailable.
    pub fn save_conversation_journey_consent(
        &self,
        project: &str,
        conversation: Uuid,
        consent: &ConversationJourneyConsent,
    ) -> Result<ConversationJourneyRecord> {
        let owner = self.conversation_project_identity(project)?;
        if self
            .store
            .conversation_journey(&owner, conversation, consent.task_id, consent.id)?
            .is_none()
        {
            self.validate_conversation_journey_data(project, conversation, consent)?;
        }
        Ok(self
            .store
            .save_conversation_journey(&owner, conversation, consent)?)
    }

    pub fn revoke_conversation_journey_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationJourneyRecord> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .revoke_conversation_journey(&owner, conversation, task, id)?)
    }

    fn journey_model_description(&self, selection: &str) -> Result<JourneyModelDescription> {
        if let Some(id) = selection.strip_prefix("model-profile:") {
            let id: ModelProfileId = id.parse()?;
            let model = self.store.get_model_profile(id, None)?;
            let provider = self.store.get_provider_profile(model.provider_id)?;
            let snapshot = ModelProfileSnapshot::frozen(&model, &provider)?;
            if !snapshot.input_modalities.contains(&InputModality::Image) {
                bail!("A sample model must accept images: {}", model.display_name);
            }
            // Routing/permission changes invalidate consent; routine health timestamps
            // and display-name edits do not. No credential bytes are read or returned.
            let permissions = serde_json::json!({"connection_policy":provider.connection_policy});
            let evidence = serde_json::json!({"snapshot":snapshot,"organization":provider.organization,"workspace":provider.workspace,"headers":provider.safe_headers,"credential_reference":provider.credential_ref,"permissions":permissions});
            return Ok(JourneyModelDescription {
                scope: JourneyModelScope {
                    model_id: format!("model-profile:{id}"),
                    binding_digest: annotagent_image_tools::sha256(&serde_json::to_vec(&evidence)?),
                },
                display_name: model.display_name,
                destination: provider.endpoint_summary(),
                permissions,
            });
        }
        if !(selection.starts_with("plugin:") || selection.starts_with("model-instance:")) {
            bail!("Select a real Registry Model Profile or installed Plugin model");
        }
        let snapshots = self.freeze_plugin_model_selections(BTreeSet::from([selection]))?;
        let snapshot = snapshots
            .first()
            .ok_or_else(|| anyhow!("Plugin model snapshot is unavailable"))?;
        let registry = self
            .plugin_registry
            .lock()
            .map_err(|_| anyhow!("Plugin Registry lock is poisoned"))?;
        let installations = registry.list();
        let installation = installations
            .iter()
            .find(|installed| {
                installed.manifest.id.to_string() == snapshot.plugin_id
                    && installed.manifest.version.to_string() == snapshot.plugin_version
                    && installed.package_digest.to_string() == snapshot.plugin_package_sha256
            })
            .ok_or_else(|| anyhow!("Plugin installation changed during scope resolution"))?;
        let permissions = serde_json::to_value(&installation.manifest.permissions)?;
        let evidence = serde_json::json!({"snapshot":snapshot,"permissions":permissions});
        Ok(JourneyModelDescription {
            scope: JourneyModelScope {
                model_id: selection.to_owned(),
                binding_digest: annotagent_image_tools::sha256(&serde_json::to_vec(&evidence)?),
            },
            display_name: format!("{} · {}", snapshot.plugin_id, snapshot.model_id),
            destination:
                "Installed Plugin worker · network access follows the displayed Plugin permissions"
                    .into(),
            permissions,
        })
    }

    /// Read the current first 1–3 images, matching the existing bounded sample API.
    /// Normal image identity reconciliation is reused; no inference or approval occurs.
    pub fn conversation_journey_data_scope(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        schema_id: Uuid,
        schema_revision: u64,
        selections: &[String],
    ) -> Result<ConversationJourneyDataScope> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|value| value.input.id == task)
        {
            bail!("Journey task belongs to another conversation");
        }
        let schema_digest = if schema_id.is_nil() && schema_revision == 0 {
            let task_record = self
                .conversation_tasks(project, conversation)?
                .into_iter()
                .find(|value| value.input.id == task)
                .ok_or_else(|| anyhow!("Task not found"))?;
            if self.project_goal(project)?["revision"] != task_record.input.schema_revision {
                bail!("Project goal changed since this task started");
            }
            task_record.input.schema_revision
        } else {
            let schema = self.conversation_schema_draft(project, schema_id, None)?;
            if schema.task_id != task || schema.revision != schema_revision {
                bail!("Journey Schema changed or belongs to another task; review the current goal");
            }
            annotagent_image_tools::sha256(&serde_json::to_vec(&schema.definition)?)
        };
        if !(1..=32).contains(&selections.len())
            || selections.iter().collect::<BTreeSet<_>>().len() != selections.len()
        {
            bail!("Choose 1–32 distinct registered image model bindings");
        }
        let mut models = selections
            .iter()
            .map(|selection| self.journey_model_description(selection))
            .collect::<Result<Vec<_>>>()?;
        models.sort_by(|a, b| a.scope.model_id.cmp(&b.scope.model_id));
        if models
            .iter()
            .map(|m| &m.scope.model_id)
            .collect::<BTreeSet<_>>()
            .len()
            != models.len()
        {
            bail!("Duplicate canonical model bindings");
        }
        let images = self
            .list_project_image_summaries(project)?
            .into_iter()
            .take(3)
            .map(|image| JourneyImageScope {
                image_id: image.image_id.0,
                content_hash: image.content_hash,
            })
            .collect::<Vec<_>>();
        if images.is_empty() {
            bail!("Upload Project images before authorizing automatic samples");
        }
        Ok(ConversationJourneyDataScope {
            schema_id,
            schema_revision,
            schema_digest,
            images,
            models,
        })
    }

    /// Compare fresh server snapshots before accepting a combined consent. The
    /// caller must separately verify its Builder scope/caps using the existing
    /// Builder preview and require explicit user acceptance before saving it.
    pub fn validate_conversation_journey_data(
        &self,
        project: &str,
        conversation: Uuid,
        consent: &ConversationJourneyConsent,
    ) -> Result<()> {
        if consent.expires_at <= chrono::Utc::now() {
            bail!("Journey authorization expired");
        }
        let selections = consent
            .allowed_models
            .iter()
            .map(|model| model.model_id.clone())
            .collect::<Vec<_>>();
        let current = self.conversation_journey_data_scope(
            project,
            conversation,
            consent.task_id,
            consent.schema_id,
            consent.schema_revision,
            &selections,
        )?;
        if current.images != consent.images
            || current.schema_digest != consent.schema_digest
            || current
                .models
                .iter()
                .map(|m| m.scope.clone())
                .collect::<Vec<_>>()
                != consent.allowed_models
        {
            bail!("Journey images, Schema, model destination or permission scope changed");
        }
        Ok(())
    }

    /// Bind the existing resolved Draft to saved consent. This does not start a
    /// sample: Server must still run static/supported-binding checks, derive the
    /// current guided fingerprint, and use normal sealed Sample Operation admission.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_conversation_journey_draft(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        consent_id: Uuid,
        draft_id: &str,
        authorization_fingerprint: &str,
        maximum_calls: u32,
    ) -> Result<ConversationJourneyRecord> {
        let owner = self.conversation_project_identity(project)?;
        let saved = self
            .store
            .conversation_journey(&owner, conversation, task, consent_id)?
            .ok_or_else(|| anyhow!("Journey consent not found"))?;
        if saved.revoked || saved.consent.expires_at <= chrono::Utc::now() {
            bail!("Journey consent is revoked or expired");
        }
        let builder = self
            .store
            .conversation_builder_operation(&owner, task, saved.consent.builder_operation_id)?
            .ok_or_else(|| anyhow!("Journey Builder receipt is unavailable"))?;
        let evidence = builder
            .evidence
            .as_ref()
            .ok_or_else(|| anyhow!("Journey Builder has no completed result"))?;
        if builder.status != "completed"
            || evidence["outcome"] != "draft_ready_for_human_review"
            || evidence["draft_id"] != draft_id
        {
            bail!("The requested Draft is not this journey's completed Builder result");
        }
        let (draft, profiles) = self.resolved_workflow_draft_model_profiles(draft_id)?;
        if draft.project_id != project {
            bail!("Journey Draft belongs to another Project");
        }
        if evidence["draft_revision"].as_u64() != Some(draft.revision)
            || evidence["draft_content_hash"].as_str() != Some(draft.content_hash.as_str())
        {
            bail!(
                "Builder Draft changed or lacks an exact saved revision; automatic sampling needs a fresh plan authorization"
            );
        }
        self.workflow_project_schema(&draft)?;
        let schema = draft
            .annotation_schema
            .as_ref()
            .ok_or_else(|| anyhow!("Journey Draft has no saved Schema binding"))?;
        let mut selections = profiles
            .iter()
            .map(|profile| format!("model-profile:{}", profile.model_profile_id))
            .collect::<BTreeSet<_>>();
        selections.extend(
            draft
                .nodes
                .iter()
                .filter_map(|node| node.model_binding.as_ref())
                .filter(|binding| {
                    binding.starts_with("plugin:") || binding.starts_with("model-instance:")
                })
                .cloned(),
        );
        let scope = self.conversation_journey_data_scope(
            project,
            conversation,
            task,
            Uuid::parse_str(&schema.schema_draft_id)?,
            schema.revision,
            &selections.into_iter().collect::<Vec<_>>(),
        )?;
        let sample = JourneySampleScope {
            operation_id: saved.consent.sample_operation_id,
            draft_id: Uuid::parse_str(draft_id)?,
            draft_revision: draft.revision,
            authorization_fingerprint: authorization_fingerprint.into(),
            schema_id: scope.schema_id,
            schema_revision: scope.schema_revision,
            schema_digest: scope.schema_digest,
            images: scope.images,
            models: scope.models.into_iter().map(|m| m.scope).collect(),
            maximum_calls,
        };
        Ok(self.store.seal_conversation_journey_sample(
            &owner,
            conversation,
            task,
            consent_id,
            &sample,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::*;
    use annotagent_storage::{BeginConversationTask, ConversationMessageInput};
    use std::collections::BTreeMap;

    #[test]
    fn server_derived_journey_scope_detects_data_recipient_and_schema_changes() {
        let temp = tempfile::tempdir().unwrap();
        let app = LocalApplication::new(temp.path()).unwrap();
        let project = "TEST-journey-scope";
        app.create_project(project,"version: 1\nproject:\n  name: TEST journey scope\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let image = temp.path().join(project).join("images/sample.png");
        std::fs::create_dir_all(image.parent().unwrap()).unwrap();
        annotagent_image_tools::generate_synthetic_inspection(&image).unwrap();
        let conversation = app.create_project_conversation(project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST classify cups".into(),
            image: None,
            reference: None,
        };
        app.append_project_conversation_message(project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        app.begin_conversation_task(
            project,
            conversation,
            &BeginConversationTask {
                id: task,
                source_message_id: message.id,
                schema_revision: app.project_goal(project).unwrap()["revision"]
                    .as_str()
                    .unwrap()
                    .into(),
            },
        )
        .unwrap();
        let owner = app.conversation_project_identity(project).unwrap();
        let definition = annotagent_storage::ConversationSchemaDefinition {
            goal: message.text,
            task: serde_json::from_value(
                serde_json::json!({"id":"objects","kind":"classification","labels":["cup"]}),
            )
            .unwrap(),
            boundary_rules: vec![],
        };
        let schema = app
            .store
            .create_human_conversation_schema_draft(&owner, task, Uuid::new_v4(), &definition)
            .unwrap();
        let now = chrono::Utc::now();
        let mut provider = ProviderProfile {
            id: ProviderId::new(),
            display_name: "TEST no-network provider".into(),
            preset_id: Some("mock".into()),
            adapter: ProviderAdapterKind::Mock,
            base_url: "http://127.0.0.1:8796/v1".parse().unwrap(),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("TEST only".into()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST image model".into(),
            remote_model_id: "TEST-not-called".into(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::ImageClassification]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: vec![],
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        app.store.save_provider_profile(&provider).unwrap();
        app.store.save_model_profile(&model).unwrap();
        let selections = vec![format!("model-profile:{}", model.id)];
        let scope = app
            .conversation_journey_data_scope(project, conversation, task, schema.id, 1, &selections)
            .unwrap();
        assert_eq!(scope.images.len(), 1);
        assert_eq!(scope.models[0].destination, provider.endpoint_summary());
        let consent = ConversationJourneyConsent {
            continue_after_clarification: false,
            schema_proposal: None,
            id: Uuid::new_v4(),
            task_id: task,
            builder_operation_id: Uuid::new_v4(),
            builder_model_id: Some(model.id),
            previous_grant_id: None,
            sample_operation_id: Uuid::new_v4(),
            builder_scope_hash: "a".repeat(64),
            schema_id: schema.id,
            schema_revision: 1,
            schema_digest: scope.schema_digest.clone(),
            images: scope.images.clone(),
            allowed_models: scope.models.iter().map(|m| m.scope.clone()).collect(),
            maximum_builder_calls: 8,
            maximum_sample_calls: 12,
            expires_at: now + chrono::Duration::minutes(20),
            allow_unknown_cost: true,
        };
        app.validate_conversation_journey_data(project, conversation, &consent)
            .unwrap();
        app.store
            .save_conversation_journey(&owner, conversation, &consent)
            .unwrap();
        assert!(
            app.seal_conversation_journey_draft(
                project,
                conversation,
                task,
                consent.id,
                &Uuid::new_v4().to_string(),
                &"b".repeat(64),
                12
            )
            .unwrap_err()
            .to_string()
            .contains("Builder receipt")
        );
        assert!(
            app.conversation_journey_data_scope(
                project,
                Uuid::new_v4(),
                task,
                schema.id,
                1,
                &selections
            )
            .is_err()
        );
        assert!(
            app.conversation_journey_data_scope(
                project,
                conversation,
                task,
                schema.id,
                1,
                &[selections[0].clone(), selections[0].clone()]
            )
            .is_err()
        );
        assert!(
            app.conversation_journey_data_scope(
                project,
                conversation,
                task,
                schema.id,
                1,
                &["unregistered-detector".into()]
            )
            .is_err()
        );
        assert!(
            app.conversation_journey_data_scope(
                project,
                conversation,
                task,
                schema.id,
                1,
                &["model-instance:missing".into()]
            )
            .is_err()
        );
        // Scope-only Draft/receipt fixture: it is deliberately not an executable
        // sample. Static validation and inference remain the existing Server's job.
        let mut draft = app
            .create_workflow_draft(project, &crate::load_settings(None).unwrap(), false)
            .unwrap();
        draft.nodes.push(WorkflowDraftNode {
            id: "TEST-classifier".into(),
            node_type: "classification.classify".into(),
            kind: WorkflowNodeKind::VisionModel,
            model_profile_binding: Some(WorkflowModelBinding {
                model_profile_id: model.id,
                locked: true,
            }),
            ..WorkflowDraftNode::default()
        });
        let draft = app.save_workflow_draft(draft).unwrap();
        let draft = app
            .bind_conversation_schema_to_workflow(project, &draft.id, draft.revision, schema.id, 1)
            .unwrap();
        app.store
            .reserve_conversation_builder(
                &owner,
                task,
                consent.builder_operation_id,
                &"1".repeat(64),
            )
            .unwrap();
        app.store.settle_conversation_builder(&owner,task,consent.builder_operation_id,true,&serde_json::json!({"outcome":"draft_ready_for_human_review","draft_id":draft.id,"draft_revision":draft.revision,"draft_content_hash":draft.content_hash})).unwrap();
        let sealed = app
            .seal_conversation_journey_draft(
                project,
                conversation,
                task,
                consent.id,
                &draft.id,
                &"2".repeat(64),
                12,
            )
            .unwrap();
        assert_eq!(
            sealed.sample.as_ref().unwrap().draft_revision,
            draft.revision
        );
        assert_eq!(
            sealed.sample.as_ref().unwrap().models,
            consent.allowed_models
        );
        let mut edited = draft.clone();
        edited.name = "TEST later edit".into();
        app.save_workflow_draft(edited).unwrap();
        assert!(
            app.seal_conversation_journey_draft(
                project,
                conversation,
                task,
                consent.id,
                &draft.id,
                &"2".repeat(64),
                12
            )
            .unwrap_err()
            .to_string()
            .contains("Draft changed")
        );
        provider.health.checked_at = Some(now + chrono::Duration::seconds(1));
        provider.updated_at = now + chrono::Duration::seconds(1);
        app.store.save_provider_profile(&provider).unwrap();
        app.validate_conversation_journey_data(project, conversation, &consent)
            .unwrap();
        provider.base_url = "http://127.0.0.1:8796/another-recipient".parse().unwrap();
        app.store.save_provider_profile(&provider).unwrap();
        assert!(
            app.validate_conversation_journey_data(project, conversation, &consent)
                .is_err()
        );
        provider.base_url = "http://127.0.0.1:8796/v1".parse().unwrap();
        provider.enabled = false;
        provider.health.status = ProviderHealthStatus::Disabled;
        app.store.save_provider_profile(&provider).unwrap();
        assert!(
            app.validate_conversation_journey_data(project, conversation, &consent)
                .is_err()
        );
        provider.enabled = true;
        provider.health.status = ProviderHealthStatus::Available;
        app.store.save_provider_profile(&provider).unwrap();
        app.validate_conversation_journey_data(project, conversation, &consent)
            .unwrap();
        let bytes = std::fs::read(&image).unwrap();
        let mut changed = bytes.clone();
        changed.extend_from_slice(b"TEST changed image bytes");
        std::fs::write(&image, changed).unwrap();
        assert!(
            app.validate_conversation_journey_data(project, conversation, &consent)
                .is_err()
        );
        std::fs::write(&image, bytes).unwrap();
        app.validate_conversation_journey_data(project, conversation, &consent)
            .unwrap();
        app.store
            .revise_conversation_schema_draft(&owner, schema.id, Uuid::new_v4(), 1, &definition)
            .unwrap();
        assert!(
            app.validate_conversation_journey_data(project, conversation, &consent)
                .is_err()
        );
        assert_eq!(
            app.store
                .conversation_task_budget(&owner, task)
                .unwrap()
                .total_reserved_calls,
            0
        );
    }
}
