//! Read models for the Agent UI. Identity and all execution remain in existing services.
use crate::{LocalApplication, ProjectSchema, stable_project_id};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use uuid::Uuid;

impl LocalApplication {
    pub fn agent_ui_navigation(&self, after: Option<&str>, limit: usize) -> Result<Value> {
        ensure!((1..=100).contains(&limit), "Navigation limit must be 1..100");
        let routes = self.list_project_route_ids()?;
        if let Some(after) = after {
            ensure!(routes.keys().any(|id| id.to_string() == after), "Navigation cursor missing; reload snapshot");
        }
        let mut items = Vec::new();
        for (owner, route) in routes {
            if after.is_some_and(|after| owner.to_string().as_str() <= after) { continue; }
            let schema = ProjectSchema::from_yaml(&std::fs::read_to_string(self.project_path(&route)?)?).map_err(anyhow::Error::msg)?;
            items.push(json!({"project_id":route,"project_owner_id":owner,"title":schema.project.name,"group_id":null,"conversation_id":self.store.project_conversation(&owner.to_string())?}));
            if items.len() > limit { break; }
        }
        let more = items.len() > limit;
        items.truncate(limit);
        let cursor = more.then(|| items.last().unwrap()["project_owner_id"].clone());
        Ok(json!({"workspace_id":stable_project_id(&self.workspace),"items":items,"next_cursor":cursor}))
    }
    pub fn agent_ui_tasks(&self, project: &str, conversation: Uuid, after: i64, limit: u32) -> Result<Value> {
        Ok(self.store.agent_ui_tasks(&self.conversation_project_identity(project)?, conversation, after, limit)?)
    }
    pub fn agent_ui_thread(&self, project: &str, conversation: Uuid, task: Uuid, after: i64, limit: u32) -> Result<Value> {
        Ok(self.store.agent_ui_thread(&self.conversation_project_identity(project)?, conversation, task, after, limit)?)
    }
    pub fn agent_ui_snapshot(&self, project: &str, conversation: Uuid, task: Uuid) -> Result<Value> {
        // Exact owner validation, even for an empty journal; no current-selection fallback.
        self.agent_ui_thread(project, conversation, task, 0, 1)?;
        let task_record = self.conversation_tasks(project, conversation)?.into_iter().find(|v|v.input.id==task).ok_or_else(||anyhow::anyhow!("Task missing"))?;
        let owner = self.conversation_project_identity(project)?;
        let operations = self.store.agent_ui_active_operations(&owner,project,conversation,task)?;
        let can_stop = !operations.is_empty();
        let calls = self.store.conversation_call_history(&owner,task)?;
        Ok(json!({
            "operations":operations,"calls":calls,
            "project_id":project,"project_owner_id":self.conversation_project_identity(project)?,"conversation_id":conversation,"task":task_record,
            "agent_model":self.project_conversation_agent_model(project,conversation)?,
            "queue":self.project_conversation_message_queue(project,conversation,task,0)?,
            "human_requests":self.conversation_human_requests(project,conversation,task)?,
            "builder_operations":self.conversation_builder_history(project,conversation,task)?,
            "processing_operations":self.conversation_processing_history(project,conversation,task)?,
            "journey_consents":self.conversation_journey_history(project,conversation,task)?,
            "budget":self.optional_conversation_builder_budget(project,conversation,task)?,
            "actions":{
                "stop":{"available":can_stop,"reason":if can_stop { "Select an exact operation target" } else { "No active operation" }},
                "send":{"available":true,"reason":"Saves a message; execution requires exact separate consent"},
                "approve":{"available":false,"reason":"Select an exact operation preview and its current scope"},
                "resume":{"available":false,"reason":"Use the specific checkpoint or paused Run/Batch; unknown outcomes cannot be automatically retried"}
            },
            "thread_url":format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}/thread"),
            "consistency":"individually_committed_records"
        }))
    }
}
