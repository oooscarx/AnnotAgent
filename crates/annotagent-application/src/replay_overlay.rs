use crate::LocalApplication;
use annotagent_core::{
    ModelCapability, ModelProfileSnapshot, PluginModelSnapshot, ProviderAdapterKind,
    PublishedWorkflowVersion, WorkflowDraftNode, WorkflowModelBinding,
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize)]
pub struct ReplayOverlay {
    pub nodes: BTreeMap<String, WorkflowDraftNode>,
    pub profiles: Vec<ModelProfileSnapshot>,
    pub plugins: Vec<PluginModelSnapshot>,
    pub bindings: Vec<serde_json::Value>,
}
impl LocalApplication {
    pub(crate) fn replay_overlay(
        &self,
        workflow: &PublishedWorkflowVersion,
        downstream: &BTreeSet<String>,
        selections: &BTreeMap<String, String>,
    ) -> Result<ReplayOverlay> {
        let mut overlay = ReplayOverlay {
            nodes: BTreeMap::new(),
            profiles: vec![],
            plugins: vec![],
            bindings: vec![],
        };
        for node in &workflow.draft.nodes {
            if !downstream.contains(&node.id) {
                continue;
            }
            let selected = selections
                .get(&node.id)
                .cloned()
                .or_else(|| {
                    node.model_profile_binding
                        .map(|b| format!("model-profile:{}", b.model_profile_id))
                })
                .or_else(|| node.model_binding.clone());
            let Some(selection) = selected else {
                ensure!(
                    !matches!(
                        node.kind,
                        annotagent_core::WorkflowNodeKind::VisionModel
                            | annotagent_core::WorkflowNodeKind::VisionLanguageModel
                    ),
                    "Current binding required for model node"
                );
                continue;
            };
            if selection.starts_with("mock-") {
                continue;
            }
            let capability = match node.node_type.as_str() {
                annotagent_skill_classification::CLASSIFICATION_OPERATION
                | "capability.classify" => ModelCapability::ImageClassification,
                annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION
                | annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION
                | "capability.detect" => ModelCapability::ObjectDetection,
                annotagent_skill_segmentation::PROMPTED_SEGMENTATION_OPERATION => {
                    ModelCapability::PromptedSegmentation
                }
                _ => bail!("Replay adapter is unsupported for this model operation"),
            };
            let description = self.journey_model_description(&selection)?;
            let mut current = node.clone();
            if let Some(id) = selection.strip_prefix("model-profile:") {
                let model = self.store.get_model_profile(id.parse()?, None)?;
                let provider = self.store.get_provider_profile(model.provider_id)?;
                let snapshot = ModelProfileSnapshot::frozen(&model, &provider)?;
                ensure!(
                    capability != ModelCapability::PromptedSegmentation,
                    "Prompted segmentation requires a Plugin binding"
                );
                ensure!(
                    snapshot.task_capabilities.contains(&capability)
                        || snapshot
                            .task_capabilities
                            .contains(&ModelCapability::VisionLanguage),
                    "Current model capability does not match node"
                );
                current.model_profile_binding = Some(WorkflowModelBinding {
                    model_profile_id: model.id,
                    locked: true,
                });
                current.model_binding = None;
                if !overlay
                    .profiles
                    .iter()
                    .any(|p| p.model_profile_id == model.id)
                {
                    overlay.profiles.push(snapshot);
                }
            } else {
                let snapshots =
                    self.freeze_plugin_model_selections(BTreeSet::from([selection.as_str()]))?;
                ensure!(
                    snapshots
                        .iter()
                        .all(|p| p.capabilities.contains(&capability)),
                    "Current Plugin capability does not match node"
                );
                current.model_profile_binding = None;
                current.model_binding = Some(selection.clone());
                for snapshot in snapshots {
                    if !overlay.plugins.contains(&snapshot) {
                        overlay.plugins.push(snapshot);
                    }
                }
            }
            overlay.bindings.push(serde_json::json!({"node_id":node.id,"selection":selection,"binding_digest":description.scope.binding_digest,"destination":description.destination,"permissions":description.permissions,"capability":capability}));
            overlay.nodes.insert(node.id.clone(), current);
        }
        ensure!(
            selections.keys().all(|id| overlay.nodes.contains_key(id)),
            "Binding selection is not a replayable descendant model"
        );
        if let Some(first) = overlay.profiles.first() {
            ensure!(
                overlay
                    .profiles
                    .iter()
                    .all(|p| p.provider_id == first.provider_id
                        && p.provider_adapter == first.provider_adapter),
                "Replay supports one current Provider connection"
            );
        }
        Ok(overlay)
    }
}
impl ReplayOverlay {
    #[must_use]
    pub fn uses_external_calls(&self) -> bool {
        !self.plugins.is_empty()
            || self
                .profiles
                .iter()
                .any(|p| p.provider_adapter != ProviderAdapterKind::Mock)
    }
}
