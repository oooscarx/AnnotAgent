import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { api } from "../api";
import { projectWorkPath, projectBuildPath } from "../navigation";
import type { ConversationMessage, ConversationMessageInput, ImageItem, ProjectSummary } from "../types";
import "./conversation-workspace.css";
import { ConversationSchemaCard } from "./ConversationSchemaCard";
import { ConversationSampleCanvas } from "./ConversationSampleCanvas";
import { ConversationRepairCard } from "./ConversationRepairCard";
import { conversationSampleRelation } from "../conversation-context";
import type { HumanRequest } from "../conversation-human-api";

/** The journal and image importer share the existing Project; neither starts inference. */
export function ConversationWorkspace({ project, conversationId, imageId, draftId, sampleTestId, taskId, humanRequestId, onNavigate, onNavigationGuardChange }: {
  project: ProjectSummary; conversationId?: string; imageId?: string; draftId?:string; sampleTestId?:string;
  taskId?:string; humanRequestId?:string;
  onNavigate: (path: string) => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [conversation, setConversation] = useState<string>();
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [images, setImages] = useState<ImageItem[]>([]);
  const [text, setText] = useState("");
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [mobileView, setMobileView] = useState("conversation");
  const [width, setWidth] = useState(32);
  const [requests,setRequests]=useState<HumanRequest[]>([]);
  const [requestsReady,setRequestsReady]=useState(false);
  const [requestRefresh,setRequestRefresh]=useState(0);
  const [repairEditing,setRepairEditing]=useState(false);
  const activeRequest=requests.find(value=>value.input.id===humanRequestId && value.input.task_id===taskId);
  const requestRelation=activeRequest ? conversationSampleRelation(activeRequest,draftId,sampleTestId,imageId) : undefined;
  useEffect(()=>{
    const controller=new AbortController();setRequestsReady(false);
    if(!conversation)return()=>controller.abort();
    void api.conversationTasks(project.id,conversation,controller.signal).then(tasks=>Promise.all(tasks.map(task=>api.conversationHumanRequests(project.id,conversation,task.input.id,controller.signal)))).then(values=>{if(!controller.signal.aborted){setRequests(values.flat());setRequestsReady(true);}}).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return()=>controller.abort();
  },[project.id,conversation,sampleTestId,requestRefresh]);
  const updateRequest=(value:HumanRequest)=>setRequests(items=>items.map(item=>item.input.id===value.input.id ? value : item));
  async function cancelRequest(value:HumanRequest){
    if(value.input.id===humanRequestId && sampleDirty.current && !window.confirm("Discard unsaved correction and cancel this request?"))return;
    try{updateRequest(await api.cancelHumanRequest(project.id,value));}catch(error){setError((error as Error).message);}
  }
  async function retryContinuation(value:HumanRequest){
    if(sampleDirty.current){setError("Save or undo the current correction before retrying continuation.");return;}
    try{updateRequest(await api.resumeHumanRequest(project.id,value));}catch(error){setError((error as Error).message);}
  }
  async function openRequest(value:HumanRequest){
    const ticket=++sampleNavigation.current;
    try{const operation=await api.sampleOperation(project.id,value.input.sample_test_id);
      if(!alive.current || ticket!==sampleNavigation.current)return;
      onNavigate(projectWorkPath(project.id,{conversationId:value.input.conversation_id,taskId:value.input.task_id,humanRequestId:value.input.id,draftId:operation.draft_id,sampleTestId:value.input.sample_test_id,imageId:value.input.image_id}));setMobileView("images");
    }catch(error){if(alive.current)setError((error as Error).message);}
  }
  const root = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const selectingImage = useRef(false);
  const sampleNavigation = useRef(0);
  useEffect(()=>{sampleNavigation.current++;},[imageId,draftId,sampleTestId]);
  const unsent = useRef("");
  const schemaDirty = useRef(false);
  const sampleDirty = useRef(false);
  const sampleDirtyChange = useCallback((dirty:boolean)=>{sampleDirty.current=dirty;setRepairEditing(dirty);},[]);
  const schemaDirtyChange = useCallback((dirty: boolean) => { schemaDirty.current = dirty; }, []);
  const frozen = useRef<ConversationMessageInput | undefined>(undefined);
  const alive = useRef(true);
  const selected = images.find((image) => image.image_id === imageId) ?? (!imageId ? images[0] : undefined);
  const referenceImage = frozen.current ? images.find((image) => image.image_id === frozen.current?.image?.image_id) : selected;
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    const guard = () => selectingImage.current || (!pending.current && (!(unsent.current || schemaDirty.current || sampleDirty.current) || window.confirm("Leave with unsaved message, Schema or sample edits? Saved workspace data remains on the server.")));
    const unload = (event: BeforeUnloadEvent) => { if (pending.current || unsent.current || schemaDirty.current || sampleDirty.current) event.preventDefault(); };
    onNavigationGuardChange(guard);
    window.addEventListener("beforeunload", unload);
    return () => { onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", unload); };
  }, [onNavigationGuardChange]);
  useEffect(() => {
    const controller = new AbortController();
    setReady(false); setError("");
    void (async () => {
      const [current, dataset] = await Promise.all([api.conversation(project.id, controller.signal), api.images(project.id, controller.signal)]);
      if (conversationId && conversationId !== current.conversation_id) throw new Error("This conversation does not belong to this Project or is no longer available.");
      const saved: ConversationMessage[] = [];
      if (current.conversation_id) {
        let page: ConversationMessage[];
        do {
          page = await api.conversationMessages(project.id, current.conversation_id, saved.at(-1)?.sequence ?? 0, controller.signal);
          saved.push(...page);
        } while (page.length === 100);
      }
      if (controller.signal.aborted) return;
      setConversation(current.conversation_id ?? undefined); setImages(dataset.images); setMessages(saved); setReady(true);
    })().catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => controller.abort();
  }, [project.id, conversationId]);
  async function send() {
    if (pending.current || !ready || !text.trim()) return;
    frozen.current ??= { id: crypto.randomUUID(), text, image: selected ? { image_id: selected.image_id, sha256: selected.content_hash } : null };
    const input = frozen.current;
    pending.current = true; setBusy(true); setError(""); setStatus("Saving message…");
    try {
      const id = conversation ?? (await api.createConversation(project.id)).conversation_id;
      const saved = await api.sendConversationMessage(project.id, id, input);
      if (!alive.current) return;
      setConversation(id); setMessages((items) => [...items.filter((item) => item.input.id !== saved.input.id), saved].sort((a, b) => a.sequence - b.sequence));
      frozen.current = undefined; unsent.current = ""; setText(""); setStatus("Message saved. No model has been called.");
    } catch (error) { if (alive.current) { setError((error as Error).message); setStatus("Not confirmed saved. Retry sends the same message and frozen image reference."); } }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function upload(files: File[]) {
    if (pending.current || !files.length) return;
    pending.current = true; setBusy(true); setError("");
    try {
      for (const [index, file] of files.entries()) {
        setStatus(`Uploading ${index + 1}/${files.length} to this AnnotAgent server…`);
        const result = await api.uploadImage(project.id, file);
        if (result.corrupt.length) throw new Error(result.corrupt.map((item) => `${item.name}: ${item.message}`).join("; "));
      }
      const dataset = await api.images(project.id);
      if (alive.current) { setImages(dataset.images); setStatus("Images saved on this server. No model has been called."); }
    } catch (error) { if (alive.current) { setError((error as Error).message); setStatus("Completed uploads are retained. Reselect files to retry; identical content is deduplicated."); } }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  const openImage = (id: string) => {
    sampleNavigation.current++;
    if(sampleDirty.current && !window.confirm("Discard unsaved sample edits and change the image?"))return;
    selectingImage.current = true;
    try { onNavigate(projectWorkPath(project.id, { conversationId: conversation, imageId: id, draftId, sampleTestId, taskId, humanRequestId })); }
    finally { selectingImage.current = false; }
  };
  async function openSample(draft:string,test:string,image?:string){
    const request=++sampleNavigation.current;
    try{const {sample_test}=await api.workflowSampleTest(draft,undefined,test);
      if(!sample_test || sample_test.project_id!==project.id || sample_test.draft_id!==draft || sample_test.id!==test)throw new Error("Sample Test does not belong to this Project.");
      if(!alive.current || request!==sampleNavigation.current)return;
      const keepOrigin=activeRequest && conversationSampleRelation(activeRequest,draft,test,image)!=="unrelated";
      onNavigate(projectWorkPath(project.id,{conversationId:conversation,draftId:draft,sampleTestId:test,imageId:image ?? sample_test.inputs[0]?.image_id,taskId:keepOrigin ? activeRequest.input.task_id : undefined,humanRequestId:keepOrigin ? activeRequest.input.id : undefined}));
      setMobileView("images");
    }catch(error){if(alive.current)setError((error as Error).message);}
  }
  return <section className="conversation-workspace" aria-label="Annotation workspace">
    <nav className="conversation-mobile-tabs" aria-label="Workspace panels">
      <button aria-pressed={mobileView === "conversation"} onClick={() => setMobileView("conversation")}>Conversation</button>
      <button aria-pressed={mobileView === "images"} onClick={() => setMobileView("images")}>Images ({images.length})</button>
    </nav>
    <div ref={root} className="conversation-split" data-mobile-view={mobileView} style={{ "--conversation-width": `${width}%` } as CSSProperties}>
      <section className="conversation-panel" aria-label="Project conversation">
        <h2>What would you like to annotate?</h2>
        <p className="muted">Describe your goal before or after uploading images.</p>
        <p className="conversation-development-note">Workspace integration in progress: label proposals, Pipeline Drafts, sample tests, human corrections and authorized plan revisions share saved server state. Dataset processing is not connected here yet.</p>
        <ol className="conversation-messages" aria-label="Saved messages">
          {messages.map((message) => <li key={message.input.id}><p>{message.input.text}</p>{message.input.image && <button onClick={() => {
            const reference = message.input.image;
            const image = images.find((item) => item.image_id === reference?.image_id);
            if (!reference || !image || image.content_hash !== reference.sha256) { setError("The referenced image was removed or changed. Its historical reference remains saved; current pixels cannot stand in for that evidence."); return; }
            openImage(image.image_id);
          }}>Referenced image · {images.find((image) => image.image_id === message.input.image?.image_id)?.name ?? message.input.image.image_id}</button>}<small>Saved · {message.sequence}</small></li>)}
        </ol>
        {conversation && <section aria-label="Human requests"><h3>Requests for your help</h3><button onClick={()=>{if(sampleDirty.current){setError("Save or undo this correction before refreshing requests.");return;}setRequestRefresh(value=>value+1);}}>Refresh requests</button>{requests.map(value=><article key={value.input.id} className="conversation-consent"><p>{value.input.question}</p><p>{value.resume_draft_id ? "Correction saved · revision Draft available" : value.status==="answered" ? "Correction saved · awaiting task continuation" : value.status}</p>{value.resume_error && <p role="alert">Correction saved, but Draft preparation failed: {value.resume_error}</p>}<button onClick={()=>void openRequest(value)}>Open requested result</button>{value.status==="answered" && <button onClick={()=>void retryContinuation(value)}>Retry Draft preparation</button>}{value.resume_draft_id && <button onClick={()=>onNavigate(projectBuildPath(project.id,"pipeline",{draftId:value.resume_draft_id!}))}>Inspect revision Draft</button>}{value.status==="pending" && <button onClick={()=>void cancelRequest(value)}>Cancel request</button>}</article>)}</section>}
        {conversation && messages[0] && <ConversationSchemaCard key={`${conversation}:${messages[0].input.id}`} project={project.id} conversation={conversation} message={messages[0].input.id} onDirtyChange={schemaDirtyChange} onSample={(draft,test,image)=>void openSample(draft,test,image)} />}
        {activeRequest?.status==="applied" && activeRequest.resume_draft_id && <ConversationRepairCard key={activeRequest.input.id} project={project.id} request={activeRequest} editing={repairEditing} onSample={(draft,test,image)=>void openSample(draft,test,image)} />}
        <form onSubmit={(event) => { event.preventDefault(); void send(); }} className="conversation-composer">
          <label htmlFor="conversation-message">Your message</label>
          <textarea id="conversation-message" value={text} disabled={busy || Boolean(frozen.current)} rows={3} placeholder="Find cups, but not bottles" onChange={(event) => { unsent.current = event.target.value; setText(event.target.value); }} />
          <small>{referenceImage ? `Image reference: ${referenceImage.name}` : "No image reference · Project-level message"}</small>
          <button className="primary" disabled={!ready || busy || !text.trim()} type="submit">{busy ? "Saving…" : frozen.current ? "Retry saving message" : "Save message"}</button>
        </form>
      </section>
      <div className="conversation-divider" role="separator" aria-label="Resize conversation panel" aria-orientation="vertical" tabIndex={0} aria-valuemin={25} aria-valuemax={50} aria-valuenow={width}
        onKeyDown={(event) => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); setWidth((current) => Math.max(25, Math.min(50, current + (event.key === "ArrowRight" ? 2 : -2)))); } }}
        onPointerDown={(event) => event.currentTarget.setPointerCapture(event.pointerId)}
        onPointerMove={(event) => { if (!event.currentTarget.hasPointerCapture(event.pointerId)) return; const bounds = root.current?.getBoundingClientRect(); if (bounds) setWidth(Math.round(Math.max(25, Math.min(50, (event.clientX - bounds.left) / bounds.width * 100)))); }} />
      <section className="conversation-image-panel" aria-label="Project images">
        {activeRequest && (requestRelation==="baseline" || requestRelation==="comparison") && <aside className="conversation-consent" aria-label="Sample origin"><p>{requestRelation==="comparison" ? "Sample from the revised plan. The original correction remains separate; improvement has not been established." : "Another image from the original sample. The requested correction belongs to a different image."}</p><button onClick={()=>void openRequest(activeRequest)}>Return to original correction</button></aside>}
        <div className="conversation-image-tools"><h2>{images.length ? `${images.length} images` : "Your images"}</h2><label className="conversation-upload">Add images<input type="file" accept="image/png,image/jpeg" multiple disabled={busy || !ready} onChange={(event) => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ""; void upload(files); }} /></label></div>
        {humanRequestId && (!activeRequest || requestRelation==="unrelated") ? <p role="status">{requestsReady ? "Human request not found in this task. No other result was substituted." : "Loading saved human request…"}</p> : draftId && sampleTestId ? <ConversationSampleCanvas project={project.id} draft={draftId} test={sampleTestId} image={selected} humanRequest={requestRelation==="subject" ? activeRequest : undefined} onAnswered={updateRequest} onDirtyChange={sampleDirtyChange} onOpen={(draft,test,image)=>void openSample(draft,test,image)} /> : imageId && !selected && ready ? <p role="alert">This image is not available in this Project. Select an existing image below.</p> : selected ? <figure className="conversation-image"><img src={selected.url} alt={selected.name} /><figcaption>{selected.name} · Original image · No annotation overlay</figcaption></figure> : <div className="conversation-image-empty"><h3>Start with your own images</h3><p>PNG or JPEG · up to 25 MB per image. Files are uploaded to this AnnotAgent server, not sent to a model.</p></div>}
        <nav className="conversation-thumbnails" aria-label="Select image">{images.map((image) => <button key={image.image_id} aria-label={image.name} aria-current={image.image_id === selected?.image_id ? "true" : undefined} onClick={() => openImage(image.image_id)}><img loading="lazy" src={image.url} alt="" /><span>{image.name}</span></button>)}</nav>
      </section>
    </div>
    <p className="conversation-status" role="status">{status || (ready ? "Saved workspace loaded" : "Loading saved workspace…")}</p>
    {error && <p role="alert">{error}</p>}
  </section>;
}
