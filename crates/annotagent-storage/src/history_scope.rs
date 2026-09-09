use crate::{SqliteStore, StorageError};
use annotagent_core::{ManagementObjectKind, ManagementObjectRef};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HISTORY_POLICY: &str = "exclude_existing_history_v1";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryScope {
    pub id: Uuid,
    pub revision: u64,
    pub established_at: String,
    pub policy: String,
    pub establishment_command_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstablishHistoryScope {
    pub command_id: Uuid,
    pub expected_scope_revision: Option<u64>,
    pub expected_snapshot_hash: String,
    pub policy: String,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct HistoryScopePreview {
    pub policy: String,
    pub expected_snapshot_hash: String,
    pub excluded_counts: std::collections::BTreeMap<String, usize>,
    pub preserves_published_versions: bool,
    pub preserves_annotations: bool,
    pub preserves_direct_references: bool,
}
// Use each identity table's actual persisted project key (Run uses stable ProjectId).
const IDENTITIES: &str = "SELECT COALESCE(project_id,''), 'run', id, 0 FROM runs
 UNION ALL SELECT project_id, 'batch', id, 0 FROM dataset_batches
 UNION ALL SELECT project_id, 'pipeline', workflow_id, 0 FROM workflow_pipelines
 UNION ALL SELECT project_id, 'workflow_draft', id, 0 FROM workflow_drafts
 UNION ALL SELECT project_id, 'workflow_version', workflow_id, version FROM workflow_versions
 ORDER BY 1,2,3,4";
type Identity = (String, String, String, u32);
pub(crate) fn scope_error(code: &str) -> StorageError {
    StorageError::Management {
        code: code.into(),
        message: code.replace('_', " "),
    }
}
pub(crate) fn read_scope(c: &Connection) -> Result<Option<HistoryScope>, StorageError> {
    let json: Option<String> = c
        .query_row(
            "SELECT scope_json FROM history_scopes WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    json.map(|s| serde_json::from_str(&s).map_err(Into::into))
        .transpose()
}
pub(crate) fn validate_scope(c: &Connection, id: &str) -> Result<(), StorageError> {
    let scope = read_scope(c)?.ok_or_else(|| scope_error("history_scope_not_established"))?;
    if scope.id.to_string() != id {
        return Err(scope_error("history_scope_mismatch"));
    }
    Ok(())
}
fn identities(c: &Connection) -> Result<Vec<Identity>, StorageError> {
    Ok(c.prepare(IDENTITIES)?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?)
}
fn make_preview(ids: &[Identity]) -> Result<HistoryScopePreview, StorageError> {
    let mut counts = std::collections::BTreeMap::from_iter(
        [
            "run",
            "batch",
            "pipeline",
            "workflow_draft",
            "workflow_version",
        ]
        .map(|s| (s.to_owned(), 0)),
    );
    for (_, kind, _, _) in ids {
        *counts.entry(kind.clone()).or_default() += 1;
    }
    Ok(HistoryScopePreview {
        policy: HISTORY_POLICY.into(),
        expected_snapshot_hash: annotagent_image_tools::sha256(&serde_json::to_vec(ids)?),
        excluded_counts: counts,
        preserves_published_versions: true,
        preserves_annotations: true,
        preserves_direct_references: true,
    })
}
pub(crate) fn check_object(
    c: &Connection,
    id: &str,
    project: &str,
    object: &ManagementObjectRef,
) -> Result<(), StorageError> {
    let kind = serde_json::to_value(object.kind)?
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let excluded: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM history_scope_exclusions WHERE scope_id=?1 AND project_id=?2 AND kind=?3 AND object_id=?4 AND version=?5)",params![id,project,kind,object.id,object.version.unwrap_or(0)],|r|r.get(0))?;
    // Versions created later under a hidden pipeline do not expose that pipeline through Trash.
    let hidden_parent: bool = if object.kind == ManagementObjectKind::WorkflowVersion {
        c.query_row("SELECT EXISTS(SELECT 1 FROM history_scope_exclusions WHERE scope_id=?1 AND project_id=?2 AND kind='pipeline' AND object_id=?3)",params![id,project,object.id],|r|r.get(0))?
    } else {
        false
    };
    if excluded || hidden_parent {
        return Err(scope_error("history_object_out_of_scope"));
    }
    Ok(())
}
impl SqliteStore {
    pub fn history_scope(&self) -> Result<Option<HistoryScope>, StorageError> {
        self.with_connection(read_scope)
    }
    pub fn preview_history_scope(&self) -> Result<HistoryScopePreview, StorageError> {
        self.with_connection(|c| {
            let tx = c.unchecked_transaction()?;
            make_preview(&identities(&tx)?)
        })
    }
    pub fn establish_history_scope(
        &self,
        request: &EstablishHistoryScope,
    ) -> Result<(HistoryScope, bool), StorageError> {
        self.with_connection(|c| {
            let tx=rusqlite::Transaction::new_unchecked(c,TransactionBehavior::Immediate)?;
            let incoming=serde_json::to_string(request)?;
            if let Some(existing)=read_scope(&tx)? {
                if existing.establishment_command_id != request.command_id { return Err(scope_error("history_scope_already_established")); }
                let stored:String=tx.query_row("SELECT request_json FROM history_scopes WHERE singleton=1",[],|r|r.get(0))?;
                if stored != incoming { return Err(scope_error("history_scope_command_conflict")); }
                return Ok((existing,false));
            }
        if !request.confirmed
            || request.policy != HISTORY_POLICY
            || request.expected_scope_revision.is_some()
        {
            return Err(scope_error("invalid_history_scope_request"));
        }
            let ids=identities(&tx)?;
            if make_preview(&ids)?.expected_snapshot_hash != request.expected_snapshot_hash { return Err(scope_error("history_scope_snapshot_changed")); }
            let scope=HistoryScope { id:Uuid::new_v4(),revision:1,established_at:Utc::now().to_rfc3339(),policy:HISTORY_POLICY.into(),establishment_command_id:request.command_id };
            tx.execute("INSERT INTO history_scopes(singleton,id,scope_json,request_json) VALUES(1,?1,?2,?3)",params![scope.id.to_string(),serde_json::to_string(&scope)?,incoming])?;
            for (project,kind,object,version) in ids {
                tx.execute("INSERT INTO history_scope_exclusions(scope_id,project_id,kind,object_id,version) VALUES(?1,?2,?3,?4,?5)",params![scope.id.to_string(),project,kind,object,version])?;
            }
            tx.commit()?;
            Ok((scope,true))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(store: &SqliteStore) -> EstablishHistoryScope {
        EstablishHistoryScope {
            command_id: Uuid::new_v4(),
            expected_scope_revision: None,
            expected_snapshot_hash: store
                .preview_history_scope()
                .unwrap()
                .expected_snapshot_hash,
            policy: HISTORY_POLICY.into(),
            confirmed: true,
        }
    }
    #[test]
    fn history_scope_passive_replay_restart_and_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        for _ in 0..2 {
            assert!(store.history_scope().unwrap().is_none());
            store.preview_history_scope().unwrap();
        }
        store
            .with_connection(|c| {
                assert_eq!(
                    c.query_row("SELECT COUNT(*) FROM history_scope_exclusions", [], |r| r
                        .get::<_, i64>(
                        0
                    ))?,
                    0
                );
                Ok(())
            })
            .unwrap();
        let mut req = request(&store);
        let mut invalid = req.clone();
        invalid.confirmed = false;
        assert!(store.establish_history_scope(&invalid).is_err());
        let (scope, created) = store.establish_history_scope(&req).unwrap();
        assert!(created);
        assert_eq!(
            store.establish_history_scope(&req).unwrap(),
            (scope.clone(), false)
        );
        req.confirmed = false;
        assert!(
            matches!(store.establish_history_scope(&req),Err(StorageError::Management{code,..}) if code=="history_scope_command_conflict")
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(store.history_scope().unwrap(), Some(scope));
        assert!(
            matches!(store.establish_history_scope(&request(&store)),Err(StorageError::Management{code,..}) if code=="history_scope_already_established")
        );
    }
    #[test]
    fn history_scope_two_connections_have_one_establishment_winner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.sqlite");
        let first = SqliteStore::open(&path).unwrap();
        let second = SqliteStore::open(&path).unwrap();
        let req = request(&first);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = [first, second]
            .into_iter()
            .map(|store| {
                let barrier = barrier.clone();
                let mut req = req.clone();
                req.command_id = Uuid::new_v4();
                std::thread::spawn(move || {
                    barrier.wait();
                    store.establish_history_scope(&req)
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|r|matches!(r,Err(StorageError::Management{code,..}) if code=="history_scope_already_established")).count(),1);
    }
}
