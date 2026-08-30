//! Backend-neutral Segmentation Capability.
//!
//! The Alpha advertises the semantic contract without pretending a runnable model exists. A
//! healthy Model Descriptor must provide one of the declared capabilities before an authoring
//! service may bind a segmentation node.

use std::collections::BTreeMap;

use annotagent_core::{
    CoreError, CoreResult, Skill, SkillKind, SkillManifest, SkillProductVisibility, SkillResource,
    SkillResourceRequest,
};

pub const SEGMENTATION_SKILL_ID: &str = "annotagent.segmentation";
pub const SEGMENTATION_SKILL_VERSION: &str = "1";

pub struct SegmentationCapabilitySkill {
    manifest: SkillManifest,
}

impl Default for SegmentationCapabilitySkill {
    fn default() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: SEGMENTATION_SKILL_ID.to_owned(),
                kind: SkillKind::Capability,
                skill_version: SEGMENTATION_SKILL_VERSION.to_owned(),
                display_name: "Segmentation".to_owned(),
                description:
                    "Create semantic, prompted or instance masks with a compatible Model Backend"
                        .to_owned(),
                product_visibility: SkillProductVisibility::Primary,
                deprecated_alias_for: None,
                rust_implementation: Some(
                    "annotagent_skill_segmentation::SegmentationCapabilitySkill".to_owned(),
                ),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: vec![
                    "semantic_segmentation".to_owned(),
                    "prompted_segmentation".to_owned(),
                    "instance_segmentation".to_owned(),
                ],
                requires: annotagent_core::SkillCapabilityRequirements::default(),
                optional_capabilities: Vec::new(),
                nodes: Vec::new(),
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: Vec::new(),
                summary_resources: vec!["segmentation/summary.md".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
            },
        }
    }
}

impl Skill for SegmentationCapabilitySkill {
    fn id(&self) -> &str {
        SEGMENTATION_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None | Some("segmentation/summary.md") => Ok(vec![SkillResource {
                name: "segmentation/summary.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content: "Segmentation is a generic Capability. Bind only a healthy Model Backend that declares semantic_segmentation, prompted_segmentation or instance_segmentation. SAM is one optional prompted-segmentation backend, not a Skill."
                    .to_owned(),
            }]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown Segmentation resource {other:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmentation_is_generic_and_does_not_claim_an_available_backend() {
        let skill = SegmentationCapabilitySkill::default();
        assert_eq!(skill.id(), SEGMENTATION_SKILL_ID);
        assert!(skill.manifest().nodes.is_empty());
        assert!(skill.manifest().templates.is_empty());
        assert!(
            skill
                .manifest()
                .capabilities
                .contains(&"prompted_segmentation".to_owned())
        );
        assert!(!skill.manifest().description.contains("SAM"));
    }
}
