//! Real training packages use frozen Task exports, not project-wide annotation aggregation.
use crate::LocalApplication;
use annotagent_core::ReviewStatus;
use annotagent_export::training_package::{
    ImageConfirmation, PackageImage, PackageImageLineage, PackageLineage, PackageProgress,
    PackageReceipt, inspect_training_package_with_lineage, write_training_package_with_lineage,
};
use annotagent_storage::{
    DeliveryImageDecision, DeliveryPackageInput, DeliveryPackageJob, DeliveryPackagePhase,
};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct TrainingPackageStatus {
    pub id: Uuid,
    pub phase: DeliveryPackagePhase,
    pub intent_revision: u32,
    pub snapshot_sha256: String,
    pub result: Option<PackageReceipt>,
    pub error: Option<String>,
}
fn status(job: DeliveryPackageJob) -> Result<TrainingPackageStatus> {
    Ok(TrainingPackageStatus {
        id: job.id,
        phase: job.phase,
        intent_revision: job.snapshot.delivery.revision,
        snapshot_sha256: job.snapshot_sha256,
        result: job.result.map(serde_json::from_value).transpose()?,
        error: job.error,
    })
}
impl LocalApplication {
    pub fn training_package_consents(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<annotagent_storage::DeliveryPackageConsent>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .delivery_package_consents(&owner, conversation, task)?)
    }
    pub fn training_package_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<annotagent_storage::DeliveryPackageConsent> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .delivery_package_consent(&owner, conversation, task, id)?)
    }

    /// Passive consent projection. It checks current review snapshots but never
    /// consumes the consent or creates an Export job.
    pub fn training_package_consent_view(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        consent: &annotagent_storage::DeliveryPackageConsent,
    ) -> Result<serde_json::Value> {
        let owner = self.conversation_project_identity(project)?;
        let saved = self
            .store
            .task_delivery_intent(&owner, conversation, task)?;
        let selected_images = saved
            .as_ref()
            .and_then(|saved| saved.intent.dataset_scope.as_ref())
            .map_or(0, Vec::len);
        let mut confirmed_images = 0usize;
        let mut reasons = Vec::new();
        let stale = saved.as_ref().is_none_or(|saved| {
            saved.revision != consent.input.intent_revision
                || saved.content_sha256 != consent.input.intent_sha256
        });
        if stale {
            reasons.push("delivery_intent_changed");
        } else if let Some(saved) = saved.as_ref() {
            let reviews = self
                .store
                .delivery_image_reviews(&owner, conversation, task)?;
            for image in saved.intent.dataset_scope.as_deref().unwrap_or_default() {
                let review = reviews
                    .iter()
                    .find(|review| review.input.image_id == image.image_id);
                if let Some(review) = review {
                    let current = self.store.delivery_image_snapshot(
                        &owner,
                        conversation,
                        task,
                        image.image_id,
                        review.input.source_run_id,
                    )?;
                    if current.sha256 == review.snapshot.sha256 {
                        confirmed_images += 1;
                    }
                }
            }
            if confirmed_images != selected_images {
                reasons.push("whole_image_review_missing_or_stale");
            }
        }
        let job = self
            .store
            .delivery_package(&owner, conversation, task, consent.input.id)
            .ok()
            .map(status)
            .transpose()?;
        let effective_state = if stale && consent.state == "armed" {
            "stale"
        } else if consent.state == "armed" && confirmed_images != selected_images {
            "blocked"
        } else {
            consent.state.as_str()
        };
        Ok(serde_json::json!({
            "input":consent.input,"state":consent.state,"effective_state":effective_state,
            "readiness":{
                "ready":consent.state=="armed" && !stale && selected_images>0 && confirmed_images==selected_images,
                "selected_images":selected_images,"confirmed_images":confirmed_images,
                "blocked_images":selected_images.saturating_sub(confirmed_images),"reasons":reasons
            },
            "job":job
        }))
    }
    pub fn authorize_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &annotagent_storage::DeliveryPackageConsentInput,
    ) -> Result<annotagent_storage::DeliveryPackageConsent> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .authorize_delivery_package(&owner, conversation, task, input)?)
    }
    pub fn cancel_training_package_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<annotagent_storage::DeliveryPackageConsent> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .cancel_delivery_package_consent(&owner, conversation, task, id)?)
    }
    /// Read-only readiness projection. The storage admission repeats snapshot and permission CAS.
    pub fn automatic_training_package_input(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<DeliveryPackageInput>> {
        let owner = self.conversation_project_identity(project)?;
        let consent = self
            .store
            .delivery_package_consent(&owner, conversation, task, id)?;
        if consent.state != "armed" {
            return Ok(None);
        }
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Delivery intent missing")?;
        ensure!(
            saved.revision == consent.input.intent_revision
                && saved.content_sha256 == consent.input.intent_sha256,
            "Automatic package permission is for an older delivery version; cancel or review it"
        );
        let mut image_reviews = std::collections::BTreeMap::new();
        for image in saved
            .intent
            .dataset_scope
            .as_ref()
            .context("Missing image scope")?
        {
            let mut state =
                self.task_delivery_image(project, conversation, task, image.image_id, None)?;
            if let Some(run) = state.review.as_ref().and_then(|r| r.input.source_run_id) {
                state = self.task_delivery_image(
                    project,
                    conversation,
                    task,
                    image.image_id,
                    Some(run),
                )?;
            }
            if !state.confirmation_current {
                return Ok(None);
            }
            let review = state.review.context("Current review receipt missing")?;
            image_reviews.insert(image.image_id, review.revision);
        }
        Ok(Some(DeliveryPackageInput {
            command_id: id,
            intent_revision: saved.revision,
            intent_sha256: saved.content_sha256,
            image_reviews,
            confirmed: true,
        }))
    }
    pub fn admit_authorized_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryPackageInput,
    ) -> Result<(TrainingPackageStatus, bool)> {
        let owner = self.conversation_project_identity(project)?;
        let (job, created) =
            self.store
                .begin_authorized_delivery_package(&owner, conversation, task, input)?;
        Ok((status(job)?, created))
    }
    pub fn training_package_status(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<TrainingPackageStatus> {
        let owner = self.conversation_project_identity(project)?;
        status(
            self.store
                .delivery_package(&owner, conversation, task, id)?,
        )
    }
    pub fn admit_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryPackageInput,
    ) -> Result<(TrainingPackageStatus, bool)> {
        let owner = self.conversation_project_identity(project)?;
        // A historical exact retry returns its saved immutable job even if today's
        // intake is incomplete or points at newer image bytes. It never dispatches.
        if self
            .store
            .delivery_package(&owner, conversation, task, input.command_id)
            .is_ok()
        {
            let (job, created) =
                self.store
                    .begin_delivery_package(&owner, conversation, task, input)?;
            return Ok((status(job)?, created));
        }
        let saved = self
            .require_delivery_intake(project, conversation, task)?
            .context("Save delivery information before packaging")?;
        let (job, created) = self.store.begin_delivery_package(
            &saved.intent.project_id,
            conversation,
            task,
            input,
        )?;
        Ok((status(job)?, created))
    }
    pub fn cancel_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<TrainingPackageStatus> {
        let owner = self.conversation_project_identity(project)?;
        self.store.finish_delivery_package(
            &owner,
            conversation,
            task,
            id,
            Err("Packaging cancelled by the user; no new model calls were made."),
            true,
        )?;
        self.training_package_status(project, conversation, task, id)
    }
    /// Dispatch only after explicit admission; the CAS claim makes duplicate dispatch a read.
    /// The HTTP caller runs this blocking file work off its request executor.
    pub fn execute_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<TrainingPackageStatus> {
        use DeliveryPackagePhase::{Exporting, Preparing, Validating};
        let owner = self.conversation_project_identity(project)?;
        let phase = self
            .store
            .delivery_package_phase(&owner, conversation, task, id)?;
        if phase == Preparing {
            if !self.store.advance_delivery_package(
                &owner,
                conversation,
                task,
                id,
                Preparing,
                Exporting,
            )? {
                return self.training_package_status(project, conversation, task, id);
            }
        } else if !matches!(phase, Exporting | Validating) {
            return self.training_package_status(project, conversation, task, id);
        }
        let outcome = (|| -> Result<PackageReceipt> {
            let job = self
                .store
                .delivery_package(&owner, conversation, task, id)?;
            let directory = self.export_delivery_directory(project, id, true)?;
            let destination = directory.join("dataset.zip");
            if phase == Validating && destination.is_file() {
                return inspect_training_package_with_lineage(
                    &destination,
                    &id.to_string(),
                    &job.snapshot_sha256,
                    job.snapshot.delivery.revision,
                );
            }
            let mut sources = Vec::with_capacity(job.snapshot.images.len());
            for frozen in &job.snapshot.images {
                let review = &frozen.review;
                let confirmation_id = review.input.command_id.to_string();
                let confirmation = match review.input.decision {
                    DeliveryImageDecision::PositiveComplete => {
                        ImageConfirmation::PositiveComplete { confirmation_id }
                    }
                    DeliveryImageDecision::NegativeConfirmed => {
                        ImageConfirmation::NegativeConfirmed { confirmation_id }
                    }
                    DeliveryImageDecision::Excluded => ImageConfirmation::Excluded {
                        confirmation_id,
                        reason: review
                            .input
                            .reason
                            .clone()
                            .context("Saved exclusion has no reason")?,
                    },
                };
                let source = if review.input.decision == DeliveryImageDecision::Excluded {
                    PathBuf::new()
                } else {
                    self.project_image_path(project, review.input.image_id)?
                };
                let annotations = review
                    .snapshot
                    .annotations
                    .iter()
                    .filter(|a| a.review_status == ReviewStatus::HumanAccepted)
                    .cloned()
                    .collect();
                sources.push(PackageImage {
                    image_id: review.input.image_id,
                    source,
                    annotations,
                    confirmation,
                });
            }
            let lineage = PackageLineage {
                package_id: id.to_string(),
                package_version: 1,
                frozen_snapshot_sha256: job.snapshot_sha256,
                label_id_to_class_id: job
                    .snapshot
                    .delivery
                    .intent
                    .label_spec
                    .as_ref()
                    .context("Frozen labels missing")?
                    .iter()
                    .enumerate()
                    .map(|(n, label)| (label.stable_id.clone(), n))
                    .collect(),
                exporter_version: env!("CARGO_PKG_VERSION").into(),
                validator_version: 1,
                images: job
                    .snapshot
                    .images
                    .iter()
                    .map(|image| {
                        (
                            image.review.input.image_id,
                            PackageImageLineage {
                                original_name: image.original_name.clone(),
                                schema_sha256: image.schema_sha256.clone(),
                                workflow_sha256: image.workflow_sha256.clone(),
                                model_binding_sha256: image.model_binding_sha256.clone(),
                                source_run_id: image.review.input.source_run_id,
                                confirmation_id: image.review.input.command_id.to_string(),
                                confirmation_revision: image.review.revision,
                                annotation_revision_ids: image.annotation_revision_ids.clone(),
                                annotation_snapshot_sha256: image.review.snapshot.sha256.clone(),
                                source_evidence_sha256: image.source_evidence_sha256.clone(),
                            },
                        )
                    })
                    .collect(),
            };
            write_training_package_with_lineage(
                job.snapshot.delivery.intent,
                job.snapshot.delivery.revision,
                &sources,
                &destination,
                Some(lineage),
                |progress| {
                    if progress == PackageProgress::Validating {
                        self.store.advance_delivery_package(
                            &owner,
                            conversation,
                            task,
                            id,
                            Exporting,
                            Validating,
                        )?;
                    }
                    let phase =
                        self.store
                            .delivery_package_phase(&owner, conversation, task, id)?;
                    ensure!(
                        matches!(phase, Exporting | Validating),
                        "Packaging stopped before publication"
                    );
                    Ok(())
                },
            )
        })();
        match outcome {
            Ok(receipt) => {
                self.store.finish_delivery_package(
                    &owner,
                    conversation,
                    task,
                    id,
                    Ok(&serde_json::to_value(receipt)?),
                    false,
                )?;
            }
            Err(error) => {
                self.store.finish_delivery_package(
                    &owner,
                    conversation,
                    task,
                    id,
                    Err(&error.to_string()),
                    false,
                )?;
            }
        }
        self.training_package_status(project, conversation, task, id)
    }
    /// Immutable owned receipt gates download; no client path and no generation on GET.
    pub fn open_training_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<(File, PackageReceipt)> {
        let status = self.training_package_status(project, conversation, task, id)?;
        ensure!(
            status.phase == DeliveryPackagePhase::Ready,
            "Training package is not ready for download"
        );
        let receipt = status.result.context("Ready package has no file receipt")?;
        let path = self
            .export_delivery_directory(project, id, false)?
            .join("dataset.zip");
        ensure!(!path.is_symlink(), "Package archive must not be a symlink");
        let mut file = File::open(path)?;
        ensure!(
            file.metadata()?.is_file() && file.metadata()?.len() == receipt.bytes,
            "Package file size changed after validation"
        );
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 16384];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        if format!("{:x}", hash.finalize()) != receipt.sha256 {
            bail!("Package bytes changed after validation");
        }
        file.seek(SeekFrom::Start(0))?;
        Ok((file, receipt))
    }
}
