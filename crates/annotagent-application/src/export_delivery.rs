//! Immutable generated-file delivery; never serves caller-provided filesystem paths.
use crate::{LocalApplication, ProjectExportResult};
use annotagent_core::ExportReport;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDelivery {
    pub id: Uuid,
    pub bytes: u64,
    pub sha256: String,
}

fn digest(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16384];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(format!("{:x}", hash.finalize()))
}
pub(crate) fn package(root: &Path, id: Uuid, report: &ExportReport) -> Result<ExportDelivery> {
    let archive = root.join("dataset.zip");
    let file = File::options()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&archive)?;
    let mut zip = zip::ZipWriter::new(file);
    let report_path = root.join("export-report.json");
    let root = root.canonicalize()?;
    let mut total = 0_u64;
    for path in &report.output_files {
        // The result report is written only after delivery succeeds. Its summary
        // is included separately without a self-referential archive checksum.
        if path == &report_path {
            continue;
        }
        let resolved = path.canonicalize()?;
        if path.is_symlink() || !resolved.starts_with(&root) || !resolved.is_file() {
            bail!("Export produced an unsafe delivery file");
        }
        let name = resolved
            .strip_prefix(&root)?
            .to_str()
            .context("Export filename is not UTF-8")?;
        if name.contains('\\') {
            bail!("Ambiguous export filename");
        }
        let source = File::open(&resolved)?;
        let expected = source.metadata()?.len();
        total = total
            .checked_add(expected)
            .context("Export size overflow")?;
        if total > MAX_BYTES {
            bail!("Export archive exceeds the 512 MiB delivery limit");
        }
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )?;
        if std::io::copy(&mut source.take(expected + 1), &mut zip)? != expected {
            bail!("Export file changed during packaging");
        }
    }
    zip.start_file(
        "delivery-report.json",
        zip::write::SimpleFileOptions::default(),
    )?;
    zip.write_all(&serde_json::to_vec_pretty(&serde_json::json!({"export_id":id,"exported_count":report.exported_count,"skipped_count":report.skipped_count,"warnings":report.warnings,"scope":"Generated annotation files only; original images are not included unless produced by the exporter."}))?)?;
    let mut file = zip.finish()?;
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;
    let bytes = file.metadata()?.len();
    if bytes > MAX_BYTES {
        bail!("Export archive exceeds the delivery limit");
    }
    Ok(ExportDelivery {
        id,
        bytes,
        sha256: digest(&mut file)?,
    })
}

impl LocalApplication {
    pub fn conversation_export_history(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<annotagent_storage::ConversationExport>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .conversation_exports(&owner, conversation, task)?)
    }
    pub fn conversation_export_history_page(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<annotagent_storage::ConversationExport>> {
        let owner = self.conversation_project_identity(project)?;
        Ok(self
            .store
            .conversation_exports_page(&owner, conversation, task, before, limit)?)
    }
    pub async fn export_from_conversation(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        format: &str,
    ) -> Result<ProjectExportResult> {
        let owner = self.conversation_project_identity(project)?;
        if !self
            .store
            .begin_conversation_export(&owner, conversation, task, id, format)?
        {
            if let Some(result) = self.store.completed_conversation_export(id)? {
                return Ok(serde_json::from_value(result)?);
            }
            bail!(
                "This export request is pending, interrupted or failed. Inspect its saved status; it was not executed again."
            );
        }
        match self
            .export_project_dataset_with_id(project, format, id)
            .await
        {
            Ok(result) => {
                self.store.finish_conversation_export(
                    id,
                    Some(&serde_json::to_value(&result)?),
                    None,
                )?;
                Ok(result)
            }
            Err(error) => {
                self.store
                    .finish_conversation_export(id, None, Some(&error.to_string()))?;
                Err(error)
            }
        }
    }
    pub(crate) fn export_delivery_directory(
        &self,
        project: &str,
        id: Uuid,
        create: bool,
    ) -> Result<PathBuf> {
        let project_path = self.project_path(project)?;
        let root = project_path
            .parent()
            .context("Project root missing")?
            .canonicalize()?;
        let mut directory = root.clone();
        for component in [
            "exports".to_owned(),
            "deliveries".to_owned(),
            id.to_string(),
        ] {
            directory.push(&component);
            if directory.is_symlink() {
                bail!("Export delivery directory must not be a symlink");
            }
            if create && component == id.to_string() && directory.exists() {
                bail!("Export generation already exists; refusing to overwrite it");
            }
            if create && !directory.exists() {
                std::fs::create_dir(&directory)?;
            }
            if !directory.canonicalize()?.starts_with(&root) {
                bail!("Export delivery escaped the Project");
            }
        }
        Ok(directory)
    }
    pub fn open_export_delivery(&self, project: &str, id: Uuid) -> Result<(File, ExportDelivery)> {
        let root = self.export_delivery_directory(project, id, false)?;
        let report = root.join("export-report.json");
        let archive = root.join("dataset.zip");
        if report.is_symlink() || archive.is_symlink() {
            bail!("Export delivery cannot use symlinks");
        }
        if std::fs::metadata(&report)?.len() > 1024 * 1024 {
            bail!("Export delivery manifest exceeds the size limit");
        }
        let result: ProjectExportResult = serde_json::from_slice(&std::fs::read(report)?)?;
        let metadata = result
            .delivery
            .context("This legacy export has no saved download; export again explicitly")?;
        if metadata.id != id || metadata.bytes > MAX_BYTES {
            bail!("Invalid export delivery identity or size");
        }
        let mut file = File::open(archive)?;
        if !file.metadata()?.is_file()
            || file.metadata()?.len() != metadata.bytes
            || digest(&mut file)? != metadata.sha256
        {
            bail!("Export archive changed after completion; refusing substituted data");
        }
        Ok((file, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_delivery_rejects_files_outside_generation_and_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("generation");
        std::fs::create_dir(&root).unwrap();
        let outside = temp.path().join("secret.json");
        std::fs::write(&outside, b"not an export").unwrap();
        let report: ExportReport = serde_json::from_value(serde_json::json!({
            "exported_count": 1, "skipped_count": 0, "warnings": [], "unsupported_task_kinds": [], "output_files": [outside]
        }))
        .unwrap();
        assert!(package(&root, Uuid::new_v4(), &report).is_err());
        // A failed generation cannot be overwritten or silently repurposed.
        assert!(package(&root, Uuid::new_v4(), &report).is_err());
    }
}
