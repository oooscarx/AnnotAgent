use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use annotagent_core::{
    ArtifactValidationState, CoreError, CoreResult, ImageId, ModelImage, ModelRegistry,
    NodeRegistry, RunId, TaskId, VisionArtifact, VisionInferenceRequest,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HybridNodeAction {
    Model { model_id: String },
    StaticValidator { validator_id: String },
    ReviewGate,
    Commit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridWorkflowNode {
    pub id: String,
    pub node_type: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub action: HybridNodeAction,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridWorkflowPlan {
    pub id: String,
    pub nodes: Vec<HybridWorkflowNode>,
}

pub trait VisionArtifactValidator: Send + Sync {
    fn id(&self) -> &str;
    fn validate(&self, artifacts: &[VisionArtifact]) -> Vec<String>;
}

pub struct HybridExecutionRequest {
    pub run_id: RunId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub image: Option<ModelImage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HybridExecutionResult {
    pub artifacts: Vec<VisionArtifact>,
    pub committed: Vec<VisionArtifact>,
    pub validation_issues: Vec<String>,
    pub needs_review: bool,
    pub trace: Vec<String>,
}

pub struct HybridWorkflowExecutor<'a> {
    models: &'a ModelRegistry,
    nodes: &'a NodeRegistry,
    validators: BTreeMap<String, Arc<dyn VisionArtifactValidator>>,
}

impl<'a> HybridWorkflowExecutor<'a> {
    #[must_use]
    pub fn new(models: &'a ModelRegistry, nodes: &'a NodeRegistry) -> Self {
        Self {
            models,
            nodes,
            validators: BTreeMap::new(),
        }
    }

    pub fn register_validator(
        &mut self,
        validator: Arc<dyn VisionArtifactValidator>,
    ) -> CoreResult<()> {
        let id = validator.id().to_owned();
        if self.validators.insert(id.clone(), validator).is_some() {
            return Err(CoreError::Validation(format!(
                "hybrid validator {id:?} is already registered"
            )));
        }
        Ok(())
    }

    pub async fn execute(
        &self,
        plan: &HybridWorkflowPlan,
        request: HybridExecutionRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<HybridExecutionResult> {
        let order = hybrid_order(plan)?;
        let mut outputs = BTreeMap::<String, Vec<VisionArtifact>>::new();
        let mut result = HybridExecutionResult::default();
        for node_id in order {
            if cancellation.is_cancelled() {
                return Err(CoreError::Provider(
                    "hybrid workflow execution cancelled".to_owned(),
                ));
            }
            let node = plan
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .ok_or_else(|| CoreError::Validation("hybrid node disappeared".to_owned()))?;
            let descriptor = self.nodes.get(&node.node_type).ok_or_else(|| {
                CoreError::Validation(format!(
                    "hybrid node {:?} uses unknown type {:?}",
                    node.id, node.node_type
                ))
            })?;
            let mut inputs = node
                .depends_on
                .iter()
                .flat_map(|dependency| outputs.get(dependency).cloned().unwrap_or_default())
                .collect::<Vec<_>>();
            deduplicate_artifacts(&mut inputs);
            let node_output = match &node.action {
                HybridNodeAction::Model { model_id } => {
                    let (model, backend) = self.models.resolve(model_id)?;
                    for capability in &descriptor.required_capabilities {
                        if !model.capabilities.contains(capability) {
                            return Err(CoreError::Validation(format!(
                                "model {model_id:?} lacks required capability {capability:?}"
                            )));
                        }
                    }
                    let response = backend
                        .infer(
                            VisionInferenceRequest {
                                run_id: request.run_id,
                                image_id: request.image_id,
                                task_id: request.task_id.clone(),
                                node_id: node.id.clone(),
                                model_id: model_id.clone(),
                                image: request.image.clone(),
                                input_artifacts: inputs,
                                prompt: None,
                                parameters: node.parameters.clone(),
                            },
                            cancellation.clone(),
                        )
                        .await?;
                    result.artifacts.extend(response.artifacts.clone());
                    response.artifacts
                }
                HybridNodeAction::StaticValidator { validator_id } => {
                    let validator = self.validators.get(validator_id).ok_or_else(|| {
                        CoreError::Validation(format!("unknown hybrid validator {validator_id:?}"))
                    })?;
                    result.validation_issues.extend(validator.validate(&inputs));
                    inputs
                }
                HybridNodeAction::ReviewGate => {
                    result.needs_review |= !result.validation_issues.is_empty();
                    inputs
                }
                HybridNodeAction::Commit => {
                    if !result.needs_review {
                        for artifact in &mut inputs {
                            artifact.validation_state = ArtifactValidationState::Valid;
                        }
                        result.committed.extend(inputs.clone());
                    }
                    inputs
                }
            };
            result.trace.push(format!(
                "{}:{}:{} artifact(s)",
                node.id,
                node.node_type,
                node_output.len()
            ));
            outputs.insert(node.id.clone(), node_output);
        }
        Ok(result)
    }
}

fn hybrid_order(plan: &HybridWorkflowPlan) -> CoreResult<Vec<String>> {
    let ids = plan
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() != plan.nodes.len() {
        return Err(CoreError::Validation(
            "hybrid workflow node ids must be unique".to_owned(),
        ));
    }
    let mut remaining = plan
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                node.depends_on.iter().cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if let Some(unknown) = remaining
        .values()
        .flatten()
        .find(|dependency| !ids.contains(*dependency))
    {
        return Err(CoreError::Validation(format!(
            "unknown hybrid dependency {unknown:?}"
        )));
    }
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.iter().all(|id| order.contains(id)))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(CoreError::Validation(
                "hybrid workflow contains a dependency cycle".to_owned(),
            ));
        }
        for id in ready {
            remaining.remove(&id);
            order.push(id);
        }
    }
    Ok(order)
}

fn deduplicate_artifacts(artifacts: &mut Vec<VisionArtifact>) {
    let mut ids = BTreeSet::new();
    artifacts.retain(|artifact| ids.insert(artifact.id));
}
