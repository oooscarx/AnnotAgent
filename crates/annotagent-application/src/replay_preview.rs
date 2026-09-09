use super::{LocalApplication, sha256, stable_project_id};
use annotagent_core::{PublishedWorkflowVersion, RunId, RunStatus};
use annotagent_runtime::{DagCheckpoint, DagNodeStatus};
use anyhow::{Result, anyhow, ensure};
use std::collections::{BTreeMap, BTreeSet};

impl LocalApplication {
    /// Passive exact scope. Live binding substitution is deliberately refused.
    pub fn preview_node_replay(
        &self,
        project_id: &str,
        run_id: RunId,
        node_id: &str,
    ) -> Result<serde_json::Value> {
        self.preview_node_replay_bindings(project_id, run_id, node_id, &BTreeMap::new())
            .map(|v| v.0)
    }
    pub fn preview_node_replay_bindings(
        &self,
        project_id: &str,
        run_id: RunId,
        node_id: &str,
        selections: &BTreeMap<String, String>,
    ) -> Result<(serde_json::Value, super::ReplayOverlay)> {
        let path = self.project_path(project_id)?;
        let owner = stable_project_id(path.parent().unwrap_or(&self.workspace));
        let history = self
            .store
            .list_runs()?
            .into_iter()
            .find(|r| r.id == run_id && r.project_id == Some(owner))
            .ok_or_else(|| anyhow!("Run was not found in this Project"))?;
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .workflow_snapshot_json
                .as_deref()
                .ok_or_else(|| anyhow!("Run has no Workflow checkpoint"))?,
        )?;
        let workflow: PublishedWorkflowVersion =
            serde_json::from_value(snapshot["selected_workflow"].clone())?;
        ensure!(
            workflow.project_id == project_id,
            "Run Workflow belongs to another Project"
        );
        let checkpoint: DagCheckpoint = serde_json::from_value(snapshot["checkpoint"].clone())?;
        let draft = workflow.snapshot.draft.as_ref().unwrap_or(&workflow.draft);
        ensure!(
            draft.nodes.iter().any(|n| n.id == node_id),
            "Unknown replay node"
        );
        let mut downstream = BTreeSet::from([node_id.to_owned()]);
        loop {
            let next = draft
                .edges
                .iter()
                .filter(|e| downstream.contains(&e.from_node))
                .map(|e| e.to_node.clone())
                .collect::<Vec<_>>();
            let before = downstream.len();
            downstream.extend(next);
            if before == downstream.len() {
                break;
            }
        }
        let nodes=draft.nodes.iter().filter(|n|downstream.contains(&n.id)).map(|n|serde_json::json!({"node_id":n.id,"kind":n.kind,"model_profile_binding":n.model_profile_binding,"model_binding":n.model_binding})).collect::<Vec<_>>();
        let mut blockers = Vec::new();
        let overlay = self
            .replay_overlay(&workflow, &downstream, selections)
            .unwrap_or_else(|error| {
                blockers.push(match error.to_string().as_str() {
                    "Replay supports one current Provider connection" => {
                        "multiple_current_providers_unsupported"
                    }
                    "Replay adapter is unsupported for this model operation" => {
                        "model_operation_unsupported"
                    }
                    _ => "current_binding_unavailable_or_unsupported",
                });
                super::ReplayOverlay {
                    nodes: BTreeMap::new(),
                    profiles: vec![],
                    plugins: vec![],
                    bindings: vec![],
                }
            });
        if matches!(
            history.status,
            RunStatus::Pending | RunStatus::Running | RunStatus::Paused | RunStatus::AwaitingReview
        ) {
            blockers.push("source_run_not_terminal");
        }
        if sha256(&workflow.snapshot.content_hash_material()?) != workflow.content_hash {
            blockers.push("snapshot_integrity_mismatch");
        }
        if checkpoint.workflow_content_hash != workflow.content_hash {
            blockers.push("checkpoint_snapshot_mismatch");
        }
        if draft.nodes != workflow.draft.nodes
            || draft.edges != workflow.draft.edges
            || draft.label_pipeline != workflow.draft.label_pipeline
        {
            blockers.push("legacy_snapshot_draft_mismatch");
        }
        if checkpoint.node_statuses.iter().any(|(id, status)| {
            !downstream.contains(id)
                && matches!(
                    status,
                    DagNodeStatus::Pending | DagNodeStatus::Running | DagNodeStatus::AwaitingReview
                )
        }) {
            blockers.push("upstream_checkpoint_not_settled");
        }
        let image_hash = snapshot
            .pointer("/image/sha256")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if image_hash.is_empty()
            || self
                .image_index_by_sha256(project_id, image_hash)?
                .is_none()
        {
            blockers.push("source_image_unavailable");
        }
        let preserved = checkpoint
            .node_outputs
            .keys()
            .filter(|id| !downstream.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        let mut scope = serde_json::json!({"project_id":project_id,"source_run_id":run_id,"node_id":node_id,"source_record_hash":sha256(history.workflow_snapshot_json.as_deref().unwrap_or("").as_bytes()),"source_snapshot_hash":workflow.content_hash,"checkpoint_hash":sha256(&serde_json::to_vec(&checkpoint)?),"image_hash":image_hash,"downstream_nodes":nodes,"preserved_upstream_nodes":preserved,"destinations":{"sandbox":true,"formal_annotations":false,"source_checkpoint_write":false,"published_write":false},"limits":{"maximum_model_requests":if overlay.uses_external_calls(){12}else{0},"timeout_seconds":30,"unknown_cost":overlay.uses_external_calls(),"parallel_model_nodes":1,"provider_retries":0,"http_redirects":false},"current_bindings":overlay.bindings,"binding_selections":selections,"available":blockers.is_empty(),"refusal_reasons":blockers});
        let hash = sha256(&serde_json::to_vec(&scope)?);
        scope["scope_hash"] = serde_json::json!(hash);
        Ok((scope, overlay))
    }
}
