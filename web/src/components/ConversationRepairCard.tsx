import { useEffect, useState } from "react";
import { api } from "../api";
import type { HumanRequest } from "../conversation-human-api";
import { ConversationBuilderCard } from "./ConversationBuilderCard";
import type { OpenConversationSample } from "./ConversationSampleCard";
import type { ImageClassReview } from "../conversation-image-class-api";

/** Restore the exact Sandbox Schema; fetching this card never invokes a model. */
export function ConversationRepairCard({project, request, editing, onSample, onAssistance}: {
  project:string; request:HumanRequest; editing:boolean; onAssistance?:()=>void; onSample:OpenConversationSample;
}) {
  return <RepairContext project={project} conversation={request.input.conversation_id} task={request.input.task_id} source={request.input.id} sample={request.input.sample_test_id} draft={request.resume_draft_id!} editing={editing} onSample={onSample} onAssistance={onAssistance} />;
}
export function ConversationClassRepairCard({project,review,editing,onSample,onAssistance}: {
  project:string;review:ImageClassReview;editing:boolean;onSample:OpenConversationSample;onAssistance?:()=>void;
}) {
  if(review.status!=="applied" || !review.repair_draft_id)return null;
  return <RepairContext project={project} conversation={review.conversation_id} task={review.task_id} source={review.id} sample={review.scope.sample_test_id} draft={review.repair_draft_id} imageClass editing={editing} onSample={(draft,test,image)=>onSample(draft,test,image ?? review.scope.image_id)} onAssistance={onAssistance} />;
}
function RepairContext({project,conversation,task,source,sample,draft,imageClass,editing,onSample,onAssistance}: {
  project:string;conversation:string;task:string;source:string;sample:string;draft:string;imageClass?:boolean;editing:boolean;onSample:OpenConversationSample;onAssistance?:()=>void;
}) {
  const [schema,setSchema]=useState<{id:string;revision:number}>();
  const [error,setError]=useState("");
  useEffect(()=>{
    const controller=new AbortController();
    setSchema(undefined);setError("");
    void (async()=>{
      const operation=await api.sampleOperation(project,sample);
      const result=await api.workflowSampleTest(operation.draft_id,controller.signal,sample);
      if(result.sample_test?.project_id!==project || result.sample_test.id!==sample || !result.annotation_schema)throw new Error("The saved sample's Schema is unavailable. No repair request was started.");
      if(!controller.signal.aborted)setSchema({id:result.annotation_schema.schema_draft_id,revision:result.annotation_schema.revision});
    })().catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return()=>controller.abort();
  },[project,source,sample]);
  if(error)return <p role="alert">{error}</p>;
  if(!schema)return <p role="status">Loading the saved correction's plan context…</p>;
  return <ConversationBuilderCard project={project} conversation={conversation} task={task} schema={schema} editing={editing} onSample={onSample} onAssistance={onAssistance} repairRequest={imageClass ? undefined : {id:source,draft}} imageClassRepair={imageClass ? {id:source,draft} : undefined} />;
}
