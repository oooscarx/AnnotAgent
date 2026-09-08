import { useEffect, useState } from "react";
import { api } from "../api";
import type { ImageItem, WorkflowSampleTestRecord, ConversationMessageInput } from "../types";
import { SampleFeedbackEditor } from "./SampleFeedbackEditor";
import type { OpenConversationSample } from "./ConversationSampleCard";
import type { HumanRequest } from "../conversation-human-api";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { sampleAnnotations } from "../sampleAnnotations";
import type { ImageClassReview } from "../conversation-image-class-api";
import { ConversationImageClassReview } from "./ConversationImageClassReview";

export function ConversationSampleCanvas({project, draft, test, image, onDirtyChange, onOpen, humanRequest, onAnswered, referenceTask, onReference, messageReference, imageClassReview, onClassChanged, onClassReturn, onClassRevision}: {
  messageReference?:ConversationMessageInput;
  referenceTask?:{id:string;schema_revision:string};
  onReference?:(input:Pick<ConversationMessageInput,"image"|"reference">,name:string)=>void;
  project: string; draft: string; test: string; image?: ImageItem;
  onDirtyChange: (dirty:boolean)=>void; onOpen:OpenConversationSample;
  humanRequest?:HumanRequest; onAnswered?:(value:HumanRequest)=>void;
  imageClassReview?:ImageClassReview; onClassChanged?:(value:ImageClassReview)=>void; onClassReturn?:()=>void; onClassRevision?:(draft:string)=>void;
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
  if(humanRequest && (humanRequest.input.sample_test_id!==test || humanRequest.input.image_id!==image.image_id || humanRequest.input.content_hash!==image.content_hash))return <p role="alert">The human request does not match this image and Sample Test.</p>;
  if(!sample)return <section aria-label="Sample test failure"><h2>Sample execution did not produce a result</h2><ul>{record.report.validation.issues.map((issue,index)=><li key={index}>{issue.message}</li>)}</ul><figure className="conversation-image"><img src={image.url} alt={image.name}/><figcaption>Original image · No result was substituted</figcaption></figure></section>;
  if(!sample.projection)return <section><p role="alert">This older test has no terminal-result projection. Intermediate boxes are not final results.</p><figure className="conversation-image"><img src={image.url} alt={image.name}/></figure></section>;
  const projection=sample.projection;
  if(messageReference){
    const ref=messageReference.reference?.scope === "sample_candidate" ? messageReference.reference : undefined;
    const candidate=[...projection.final_candidates,...projection.review_candidates.map(item=>item.candidate)].filter(item=>item.outcome.id===ref?.candidate_id && item.source_artifact_id===ref?.source_artifact_id);
    if(!ref || ref.draft_revision!==record.draft_revision || ref.draft_id!==draft || ref.sample_test_id!==test || messageReference.image?.sha256!==source.content_hash || candidate.length!==1)return <p role="alert">The referenced candidate, Artifact or revision is not available in this saved sample. No replacement was selected.</p>;
    return <section className="conversation-sample-canvas" aria-label="Referenced sample candidate"><header><h2>{image.name}</h2><p>Original saved candidate · Draft revision {ref.draft_revision}</p></header><p>{messageReference.text}</p><p>This historical prediction is read-only. Later sample corrections and formal annotations remain separate.</p><AnnotationCanvas imageUrl={image.url} annotations={sampleAnnotations([candidate[0].outcome],image.image_id,test)} selectedId={ref.candidate_id} readOnly compactList onSelect={()=>{}} onChange={()=>{}}/><button onClick={()=>onOpen(draft,test,image.image_id)}>View current sample corrections</button></section>;
  }
  const outcomes=[...projection.final_candidates.map(candidate=>candidate.outcome),...projection.review_candidates.map(item=>item.candidate.outcome)];
  const referenceId=humanRequest?.input.addition_id;
  if(humanRequest && Boolean(humanRequest.input.outcome_id)===Boolean(referenceId))return <p role="alert">This request does not identify exactly one candidate or new reference target. No replacement was selected.</p>;
  if(referenceId && (!schema || !["bounding_box","classification"].includes(schema.kind)))return <p role="alert">Reference editing requires the saved box or image-category Schema. No model candidate was substituted.</p>;
  if(humanRequest?.input.outcome_id && !outcomes.some(outcome=>outcome.id===humanRequest.input.outcome_id))return <p role="alert">The requested candidate is not in this sample's terminal results. No replacement was selected.</p>;
  const requestedId=referenceId ? `human-sample:${referenceId}` : humanRequest?.input.outcome_id;
  const terminal={...sample,outcomes:outcomes.filter((item,index,items)=>items.findIndex(other=>other.id===item.id)===index)};
  if(imageClassReview){
    const scope=imageClassReview.scope;
    if(humanRequest || messageReference || scope.draft_id!==draft || scope.sample_test_id!==test || scope.draft_revision!==record.draft_revision || scope.draft_content_hash!==record.draft_content_hash || scope.image_id!==image.image_id || scope.content_hash!==image.content_hash || !onClassChanged || !onClassReturn || !onClassRevision || scope.members.some(member=>![...projection.final_candidates,...projection.review_candidates.map(item=>item.candidate)].some(item=>item.outcome.id===member.outcome.id && item.source_artifact_id===member.source_artifact_id)))return <p role="alert">The image-class review does not match this saved terminal evidence. No ordinary editor was substituted.</p>;
    return <ConversationImageClassReview key={imageClassReview.id} project={project} review={imageClassReview} image={image} outcomes={terminal.outcomes} onChanged={onClassChanged} onDirtyChange={onDirtyChange} onReturn={onClassReturn} onRevision={onClassRevision}/>;
  }
  if(humanRequest?.deferred)return <section className="conversation-sample-canvas" aria-label="Deferred sample request"><h2>{image.name}</h2><p role="status">Deferred · not reviewed or completed. Reopen the request in the conversation before submitting a correction.</p><SampleFeedbackEditor key={`deferred:${humanRequest.input.id}`} readOnly projectId={project} draftId={draft} testId={test} sample={terminal} image={image} goalOverride={schema} initialOutcomeId={requestedId} onDirtyChange={onDirtyChange} onKeepOriginal={onOpen}/></section>;
  return <section className="conversation-sample-canvas" aria-label="Saved sample results">
    <header><h2>{image.name}</h2><p>Sample {index+1}/{record.inputs.length} · Evaluation only · Not a published dataset annotation</p></header>
    {humanRequest && <p role="status">{humanRequest.resume_draft_id ? "Correction saved and revision Draft prepared without model calls. Later repairs and tests have separate operation records." : humanRequest.status==="pending" ? humanRequest.input.question : humanRequest.status==="answered" ? "Correction saved. Task continuation is pending; no new model call was started." : `Human request: ${humanRequest.status}`}</p>}
    <SampleFeedbackEditor key={`${test}:${image.image_id}:${humanRequest?.input.id ?? ""}:${humanRequest?.status ?? ""}`} projectId={project} draftId={draft} testId={test} sample={terminal} image={image} goalOverride={schema} onDirtyChange={onDirtyChange} onKeepOriginal={onOpen}
      onReferenceOutcome={referenceTask && onReference ? id=>{
        const candidates=[...projection.final_candidates,...projection.review_candidates.map(item=>item.candidate)].filter(item=>item.outcome.id===id);
        if(candidates.length!==1){setError("The selection does not identify one saved terminal candidate.");return;}
        onReference({image:{image_id:image.image_id,sha256:source.content_hash},reference:{scope:"sample_candidate",task_id:referenceTask.id,project_schema_revision:referenceTask.schema_revision,draft_id:record.draft_id,draft_revision:record.draft_revision,sample_test_id:record.id,candidate_id:id,source_artifact_id:candidates[0].source_artifact_id}},image.name);
      }:undefined}
      initialOutcomeId={requestedId}
      humanSubmission={humanRequest?.status==="pending" ? {...(referenceId ? {additionId:referenceId} : {outcomeId:humanRequest.input.outcome_id!}),save:async revision=>{const saved=await api.answerHumanRequest(project,humanRequest,revision);if(!saved.answer)throw new Error("Server did not return a saved correction");onAnswered?.(saved);return saved.answer;}} : undefined}
      navigation={<nav className="button-row" aria-label="Tested images"><button disabled={index===0} onClick={()=>onOpen(draft,test,record.inputs[index-1].image_id)}>Previous image</button><span>{index+1}/{record.inputs.length}</span><button disabled={index===record.inputs.length-1} onClick={()=>onOpen(draft,test,record.inputs[index+1].image_id)}>Next image</button></nav>} />
  </section>;
}
