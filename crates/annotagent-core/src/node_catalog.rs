//! Public, constrained Annotation Workflow node catalog.
//!
//! Executable operation descriptors remain in `vision_backend` for snapshot compatibility. This
//! module is the smaller authoring contract exposed to people and the Pipeline Builder Agent.

use serde::{Deserialize, Serialize};

use crate::{ArtifactKind, ModelCapability};

pub type NodeDefinitionId = String;
pub type JsonSchema = serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    Input,
    ImagePreparation,
    ModelInference,
    ResultTransform,
    EvidenceAndValidation,
    HumanAndOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortCardinality {
    One,
    Many,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCardinality {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSideEffect {
    None,
    HumanSuspension,
    AnnotationCommit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDefinition {
    pub name: String,
    pub artifact_type: ArtifactKind,
    pub required: bool,
    pub cardinality: PortCardinality,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDefinition {
    pub id: NodeDefinitionId,
    pub display_name: String,
    pub category: NodeCategory,
    pub input_ports: Vec<PortDefinition>,
    pub output_ports: Vec<PortDefinition>,
    pub config_schema: JsonSchema,
    pub required_model_capability: Option<ModelCapability>,
    pub cardinality: NodeCardinality,
    pub side_effect: NodeSideEffect,
    pub dry_run_supported: bool,
    pub expert_only: bool,
}

impl NodeDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err("node definition id and display_name cannot be empty".to_owned());
        }
        if self.output_ports.is_empty() {
            return Err(format!(
                "node definition {:?} requires an output port",
                self.id
            ));
        }
        if !self.config_schema.is_object() {
            return Err(format!(
                "node definition {:?} config_schema must be a JSON object",
                self.id
            ));
        }
        for ports in [&self.input_ports, &self.output_ports] {
            let mut names = std::collections::BTreeSet::new();
            for port in ports {
                if port.name.trim().is_empty() || !names.insert(port.name.as_str()) {
                    return Err(format!(
                        "node definition {:?} port names must be non-empty and unique per direction",
                        self.id
                    ));
                }
            }
        }
        if self.side_effect == NodeSideEffect::AnnotationCommit && self.dry_run_supported {
            return Err("annotation commit cannot claim dry-run side effects".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePolicyScope {
    Node,
    Workflow,
    Runtime,
}

/// Cross-cutting execution behavior. These definitions are intentionally not Node Definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimePolicyDefinition {
    pub id: String,
    pub display_name: String,
    pub scope: RuntimePolicyScope,
    pub config_schema: JsonSchema,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_cannot_claim_dry_run_side_effects() {
        let definition = NodeDefinition {
            id: "core.commit".to_owned(),
            display_name: "Commit".to_owned(),
            category: NodeCategory::HumanAndOutput,
            input_ports: Vec::new(),
            output_ports: vec![PortDefinition {
                name: "annotations".to_owned(),
                artifact_type: ArtifactKind::AnnotationCandidateSet,
                required: true,
                cardinality: PortCardinality::Many,
            }],
            config_schema: serde_json::json!({"type": "object"}),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::AnnotationCommit,
            dry_run_supported: true,
            expert_only: false,
        };
        assert!(definition.validate().is_err());
    }
}
