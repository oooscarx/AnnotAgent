//! Explicit permission for one local package after current whole-image reviews.
use crate::delivery_image_review::intent;
use crate::{SqliteStore, StorageError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryPackageConsentInput {
    /// Also the single future Export command ID.
    pub id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryPackageConsent {
    pub input: DeliveryPackageConsentInput,
    pub state: String,
}
fn invalid(s: &str) -> StorageError {
    StorageError::InvalidConversation(s.into())
}

pub(crate) fn read(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    id: Uuid,
) -> Result<DeliveryPackageConsent, StorageError> {
    intent(db, project, conversation, task)?;
    let row:Option<(String,String)>=db.query_row("SELECT input_json,state FROM delivery_package_consents WHERE id=?1 AND project_id=?2 AND conversation_id=?3 AND task_id=?4",params![id.to_string(),project,conversation.to_string(),task.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let (json, state) = row.ok_or_else(|| invalid("Owned packaging permission not found"))?;
    Ok(DeliveryPackageConsent {
        input: serde_json::from_str(&json)?,
        state,
    })
}

impl SqliteStore {
    pub fn delivery_package_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<DeliveryPackageConsent, StorageError> {
        self.with_connection(|db| read(db, project, conversation, task, id))
    }
    pub fn authorize_delivery_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryPackageConsentInput,
    ) -> Result<DeliveryPackageConsent, StorageError> {
        if !input.confirmed || input.id.is_nil() {
            return Err(invalid("Explicit local package permission required"));
        }
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            let saved=intent(&tx,project,conversation,task)?;
            let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM delivery_package_consents WHERE id=?1)",[input.id.to_string()],|r|r.get(0))?;
            if exists{let old=read(&tx,project,conversation,task,input.id)?;if old.input!=*input{return Err(invalid("Package permission retries cannot change scope"));}return Ok(old);}
            if saved.revision!=input.intent_revision||saved.content_sha256!=input.intent_sha256||!saved.intent.missing_slots().is_empty()||!saved.intent.training_target.as_ref().is_some_and(annotagent_core::dataset_delivery::TrainingTarget::is_detection_preset){return Err(invalid("Package permission requires the exact complete supported delivery version"));}
            let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM delivery_package_consents WHERE task_id=?1 AND state='armed')",[task.to_string()],|r|r.get(0))?;
            if active{return Err(invalid("A package permission is already armed; inspect or cancel it before replacing"));}
            tx.execute("INSERT INTO delivery_package_consents(id,project_id,conversation_id,task_id,input_json,state,created_at) VALUES(?1,?2,?3,?4,?5,'armed',?6)",params![input.id.to_string(),project,conversation.to_string(),task.to_string(),serde_json::to_string(input)?,chrono::Utc::now().to_rfc3339()])?;
            let result=read(&tx,project,conversation,task,input.id)?;tx.commit()?;Ok(result)
        })
    }
    pub fn cancel_delivery_package_consent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<DeliveryPackageConsent, StorageError> {
        self.with_connection(|db|{
        let tx=db.unchecked_transaction()?;let old=read(&tx,project,conversation,task,id)?;
        if old.state=="consumed"{return Err(invalid("Packaging already admitted; cancel its Export job instead"));}
        tx.execute("UPDATE delivery_package_consents SET state='cancelled' WHERE id=?1 AND state='armed'",[id.to_string()])?;
        let value=read(&tx,project,conversation,task,id)?;tx.commit()?;Ok(value)
    })
    }
}
