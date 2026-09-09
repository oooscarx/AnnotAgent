//! Real training packages use frozen Task exports, not project-wide annotation aggregation.
use crate::LocalApplication;
use annotagent_core::ReviewStatus;
use annotagent_export::training_package::{
    ImageConfirmation, PackageImage, PackageImageLineage, PackageLineage, PackageProgress,
    PackageReceipt, write_training_package_with_lineage,
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
        let outcome = (|| -> Result<PackageReceipt> {
            let job = self
                .store
                .delivery_package(&owner, conversation, task, id)?;
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
            let directory = self.export_delivery_directory(project, id, true)?;
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
                &directory.join("dataset.zip"),
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
