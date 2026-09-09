//! Versioned delivery intent attached to an existing conversation Task.
//! A complete intent is not authorization, image acceptance, or package readiness.
use crate::{ImageId, TaskKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const DETECTION_PROFILE: &str = "ultralytics_yolo_detection";
pub const DETECTION_PROFILE_REVISION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryImage {
    pub image_id: ImageId,
    /// Server-resolved original byte digest, never the displayed thumbnail digest.
    pub content_sha256: String,
    pub content_revision: String,
    pub existing_split: Option<DatasetSplit>,
    /// Known capture/source group IDs; absence does not imply near-duplicate checking.
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSplit {
    Train,
    Val,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryLabel {
    pub stable_id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub include: String,
    pub exclude: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingTarget {
    pub annotation_kind: TaskKind,
    pub framework: String,
    pub export_profile: String,
    pub profile_revision: u32,
}

impl TrainingTarget {
    /// Only the one implemented preset is eligible for subsequent capability checks.
    /// Unsupported intent must be preserved, never silently converted to detection.
    #[must_use]
    pub fn is_detection_preset(&self) -> bool {
        self.annotation_kind == TaskKind::BoundingBox
            && self.framework == "ultralytics"
            && self.export_profile == DETECTION_PROFILE
            && self.profile_revision == DETECTION_PROFILE_REVISION
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliverySplitPolicy {
    pub train_percent: u8,
    pub seed: u64,
    pub preserve_existing: bool,
    pub keep_known_groups_together: bool,
}

impl Default for DeliverySplitPolicy {
    fn default() -> Self {
        Self {
            train_percent: 80,
            seed: 0,
            preserve_existing: true,
            keep_known_groups_together: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryReviewPolicy {
    HumanWholeImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDeliveryIntent {
    pub version: u32,
    pub project_id: String,
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    /// None means unresolved. Empty supplied values are invalid, not resolved slots.
    pub dataset_scope: Option<Vec<DeliveryImage>>,
    /// Vec order freezes the contiguous export class mapping, independently of detector IDs.
    pub label_spec: Option<Vec<DeliveryLabel>>,
    pub training_target: Option<TrainingTarget>,
    pub split_policy: DeliverySplitPolicy,
    pub review_policy: DeliveryReviewPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverySlot {
    DatasetScope,
    LabelSpec,
    TrainingTarget,
}

impl TaskDeliveryIntent {
    #[must_use]
    pub fn missing_slots(&self) -> Vec<DeliverySlot> {
        let mut slots = Vec::new();
        if self.dataset_scope.is_none() {
            slots.push(DeliverySlot::DatasetScope);
        }
        if self.label_spec.is_none() {
            slots.push(DeliverySlot::LabelSpec);
        }
        if self.training_target.is_none() {
            slots.push(DeliverySlot::TrainingTarget);
        }
        slots
    }

    /// Structural validation only: Application must separately resolve all owners,
    /// image hashes, Schema/Workflow/Exporter capabilities and explicit authorization.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1
            || Uuid::parse_str(&self.project_id).map_or(true, |id| id.is_nil())
            || self.conversation_id.is_nil()
            || self.task_id.is_nil()
        {
            return Err("delivery version or ownership identity is invalid");
        }
        if !(1..100).contains(&self.split_policy.train_percent)
            || !self.split_policy.preserve_existing
            || !self.split_policy.keep_known_groups_together
        {
            return Err("delivery requires nonempty train/val and preserved splits/groups");
        }
        if let Some(images) = &self.dataset_scope {
            let mut ids = BTreeSet::new();
            if images.is_empty() {
                return Err("selected image scope is empty");
            }
            for image in images {
                if !ids.insert(image.image_id)
                    || image.content_revision.trim().is_empty()
                    || image.content_sha256.len() != 64
                    || !image.content_sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    || image.group_ids.iter().any(|id| id.trim().is_empty())
                {
                    return Err("image scope has duplicate identities or invalid content evidence");
                }
            }
        }
        if let Some(labels) = &self.label_spec {
            let mut ids = BTreeSet::new();
            let mut names = BTreeSet::new();
            if labels.is_empty() {
                return Err("label specification is empty");
            }
            for label in labels {
                if label.stable_id.trim().is_empty()
                    || label.display_name.trim().is_empty()
                    || !ids.insert(&label.stable_id)
                    || !names.insert(label.display_name.trim())
                {
                    return Err("labels require unique stable IDs and display names");
                }
            }
        }
        if let Some(target) = &self.training_target {
            if target.framework.trim().is_empty()
                || target.export_profile.trim().is_empty()
                || target.profile_revision == 0
            {
                return Err("training target is incomplete; YOLO alone is ambiguous");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn intent() -> TaskDeliveryIntent {
        TaskDeliveryIntent {
            version: 1,
            project_id: Uuid::new_v4().to_string(),
            conversation_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            dataset_scope: None,
            label_spec: None,
            training_target: None,
            split_policy: DeliverySplitPolicy::default(),
            review_policy: DeliveryReviewPolicy::HumanWholeImage,
        }
    }

    #[test]
    fn missing_slots_survive_serialization_without_inference() {
        let mut value = intent();
        value.label_spec = Some(vec![DeliveryLabel {
            stable_id: "stable-ball".into(),
            display_name: "球".into(),
            aliases: vec![],
            include: String::new(),
            exclude: String::new(),
        }]);
        let restored: TaskDeliveryIntent =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(
            restored.missing_slots(),
            vec![DeliverySlot::DatasetScope, DeliverySlot::TrainingTarget]
        );
        assert!(restored.validate().is_ok());
        assert!(
            restored.training_target.is_none(),
            "ball does not imply bbox"
        );
    }

    #[test]
    fn segmentation_is_preserved_and_not_a_detection_preset() {
        let target = TrainingTarget {
            annotation_kind: TaskKind::SemanticMask,
            framework: "ultralytics".into(),
            export_profile: DETECTION_PROFILE.into(),
            profile_revision: 1,
        };
        assert!(!target.is_detection_preset());
        assert_eq!(target.annotation_kind, TaskKind::SemanticMask);
    }

    #[test]
    fn empty_supplied_slots_and_duplicate_image_ids_are_invalid() {
        let mut value = intent();
        value.dataset_scope = Some(vec![]);
        assert!(value.validate().is_err());
        let image = DeliveryImage {
            image_id: ImageId::new(),
            content_sha256: "a".repeat(64),
            content_revision: "1".into(),
            existing_split: None,
            group_ids: vec![],
        };
        value.dataset_scope = Some(vec![image.clone(), image]);
        assert!(value.validate().is_err());
    }

    #[test]
    fn export_label_order_is_not_alphabetically_resorted() {
        let mut value = intent();
        value.label_spec = Some(
            [("id-z", "zebra"), ("id-a", "apple")]
                .map(|(id, name)| DeliveryLabel {
                    stable_id: id.into(),
                    display_name: name.into(),
                    aliases: vec![],
                    include: String::new(),
                    exclude: String::new(),
                })
                .to_vec(),
        );
        assert!(value.validate().is_ok());
        let restored: TaskDeliveryIntent =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(restored.label_spec.unwrap()[0].stable_id, "id-z");
    }
}
