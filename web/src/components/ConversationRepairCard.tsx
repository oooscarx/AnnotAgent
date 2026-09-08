import { useEffect, useState } from "react";
import { api } from "../api";
import type { HumanRequest } from "../conversation-human-api";
import { ConversationBuilderCard } from "./ConversationBuilderCard";
import type { OpenConversationSample } from "./ConversationSampleCard";

/** Restore the exact Sandbox Schema; fetching this card never invokes a model. */
export function ConversationRepairCard({project, request, editing, onSample, onAssistance}: {
  project:string; request:HumanRequest; editing:boolean; onAssistance?:()=>void; onSample:OpenConversationSample;
}) {
  const [schema,setSchema]=useState<{id:string;revision:number}>();
  const [error,setError]=useState("");
  useEffect(()=>{
    const controller=new AbortController();
    setSchema(undefined);setError("");
    void (async()=>{
      const operation=await api.sampleOperation(project,request.input.sample_test_id);
      const result=await api.workflowSampleTest(operation.draft_id,controller.signal,request.input.sample_test_id);
      if(result.sample_test?.project_id!==project || result.sample_test.id!==request.input.sample_test_id || !result.annotation_schema)throw new Error("The saved sample's Schema is unavailable. No repair request was started.");
      if(!controller.signal.aborted)setSchema({id:result.annotation_schema.schema_draft_id,revision:result.annotation_schema.revision});
    })().catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return()=>controller.abort();
  },[project,request.input.id,request.input.sample_test_id]);
  if(error)return <p role="alert">{error}</p>;
  if(!schema)return <p role="status">Loading the saved correction's plan context…</p>;
  return <ConversationBuilderCard project={project} conversation={request.input.conversation_id} task={request.input.task_id} schema={schema} editing={editing} onSample={onSample} onAssistance={onAssistance} repairRequest={{id:request.input.id,draft:request.resume_draft_id!}} />;
}
