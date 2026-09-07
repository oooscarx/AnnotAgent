import { useEffect, useState } from "react";
import { api } from "../api";
import type { ImageItem, WorkflowSampleTestRecord } from "../types";
import { SampleFeedbackEditor } from "./SampleFeedbackEditor";
import type { OpenConversationSample } from "./ConversationSampleCard";

export function ConversationSampleCanvas({project, draft, test, image, onDirtyChange, onOpen}: {
  project: string; draft: string; test: string; image?: ImageItem;
  onDirtyChange: (dirty:boolean)=>void; onOpen:OpenConversationSample;
}) {
  const [record,setRecord]=useState<WorkflowSampleTestRecord>();
  const [error,setError]=useState("");
  const [schema,setSchema]=useState<{kind:string;labels:string[]}>();
  useEffect(()=>{
    const controller=new AbortController();setRecord(undefined);setError("");
    void api.workflowSampleTest(draft,controller.signal,test).then(({sample_test,annotation_schema})=>{
      if(controller.signal.aborted)return;
      if(!sample_test || sample_test.project_id!==project || sample_test.draft_id!==draft || sample_test.id!==test)throw new Error("The saved Sample Test does not belong to this Project and Draft.");
      setRecord(sample_test);
      setSchema(annotation_schema?.task);
    }).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return()=>controller.abort();
  },[project,draft,test]);
  const index=record?.inputs.findIndex(input=>input.image_id===image?.image_id) ?? -1;
  const source=record?.inputs[index]; const sample=record?.report.samples[index];
  if(error)return <p role="alert">{error}</p>;
  if(!record || record.id!==test || record.draft_id!==draft)return <p role="status">Loading saved sample evidence…</p>;
  if(!image || !source)return <p>This image was not in the selected Sample Test. Select a tested image; no result has been substituted.</p>;
  if(source.content_hash!==image.content_hash)return <p role="alert">This image changed after the test. Current pixels cannot replace the saved evidence.</p>;
  if(!sample)return <section aria-label="Sample test failure"><h2>Sample execution did not produce a result</h2><ul>{record.report.validation.issues.map((issue,index)=><li key={index}>{issue.message}</li>)}</ul><figure className="conversation-image"><img src={image.url} alt={image.name}/><figcaption>Original image · No result was substituted</figcaption></figure></section>;
  if(!sample.projection)return <section><p role="alert">This older test has no terminal-result projection. Intermediate boxes are not final results.</p><figure className="conversation-image"><img src={image.url} alt={image.name}/></figure></section>;
  const outcomes=[...sample.projection.final_candidates.map(candidate=>candidate.outcome),...sample.projection.review_candidates.map(item=>item.candidate.outcome)];
  const terminal={...sample,outcomes:outcomes.filter((item,index,items)=>items.findIndex(other=>other.id===item.id)===index)};
  return <section className="conversation-sample-canvas" aria-label="Saved sample results">
    <header><h2>{image.name}</h2><p>Sample {index+1}/{record.inputs.length} · Evaluation only · Not a published dataset annotation</p></header>
    <SampleFeedbackEditor key={`${test}:${image.image_id}`} projectId={project} draftId={draft} testId={test} sample={terminal} image={image} goalOverride={schema} onDirtyChange={onDirtyChange} onKeepOriginal={onOpen}
      navigation={<nav className="button-row" aria-label="Tested images"><button disabled={index===0} onClick={()=>onOpen(draft,test,record.inputs[index-1].image_id)}>Previous image</button><span>{index+1}/{record.inputs.length}</span><button disabled={index===record.inputs.length-1} onClick={()=>onOpen(draft,test,record.inputs[index+1].image_id)}>Next image</button></nav>} />
  </section>;
}
