import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { api, type ProcessingReceipt } from "../api";
import { projectWorkPath, projectBuildPath, projectBatchPath, parseWorkspaceRoute, conversationSettingsPath, type ConversationResultsContext } from "../navigation";
import type { ConversationMessage, ConversationMessageInput, ConversationTask, ImageItem, ProjectSummary } from "../types";
import "./conversation-workspace.css";
import { ConversationSchemaCard } from "./ConversationSchemaCard";
import { ConversationSampleCanvas } from "./ConversationSampleCanvas";
import { ConversationRepairCard } from "./ConversationRepairCard";
import { JourneyConfirm } from "./JourneyConfirm";
import { ConversationBatchStatus } from "./ConversationBatchStatus";
import { ConversationBatchResults } from "./ConversationBatchResults";
import { conversationSampleRelation } from "../conversation-context";
import { ConversationProjectBudget } from "./ConversationProjectBudget";
import type { HumanRequest } from "../conversation-human-api";
import { ConversationHumanRequests } from "./ConversationHumanRequests";
import { ConversationFeedbackCard } from "./ConversationFeedbackCard";
import { feedbackNavigationStillCurrent } from "../conversation-feedback";
import { composerIntent, isAnnotationGoalMessage, isStopCommand, isStopMessage, makeStopMessage, mergeConversationMessages, parsePendingStop } from "../conversation-control";
import { stopApi, type StopRequestRecord } from "../conversation-stop-api";
import { ConversationStopCard } from "./ConversationStopCard";
import { t } from "../i18n";
import { imageClassApi, type ImageClassReview } from "../conversation-image-class-api";
import { mergeImageClassReview } from "../conversation-image-class";

/** The journal and image importer share the existing Project; neither starts inference. */
export function ConversationWorkspace({ project, conversationId, imageId, draftId, sampleTestId, taskId, humanRequestId, classReviewId, referenceMessageId, processingOperationId, results, onNavigate, onNavigationGuardChange }: {
  project: ProjectSummary; conversationId?: string; imageId?: string; draftId?:string; sampleTestId?:string;
  taskId?:string; humanRequestId?:string; classReviewId?:string; referenceMessageId?:string; processingOperationId?:string;
  results?: ConversationResultsContext;
  onNavigate: (path: string) => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [conversation, setConversation] = useState<string>();
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [stopRecords, setStopRecords] = useState<Record<string, StopRequestRecord>>({});
  const [tasks, setTasks] = useState<ConversationTask[]>([]);
  const [classReview, setClassReview] = useState<ImageClassReview>();
  const [classReviewError, setClassReviewError] = useState("");
  useEffect(() => {
    setClassReview(undefined); setClassReviewError("");
    const controller = new AbortController();
    if (!classReviewId) return () => controller.abort();
    if (!conversationId || !taskId || humanRequestId || referenceMessageId || results) { setClassReviewError("This review requires its saved conversation and task, without another active result scope."); return () => controller.abort(); }
    void imageClassApi.get(project.id, conversationId, taskId, classReviewId, controller.signal).then(value => {
      if (controller.signal.aborted) return;
      if (value.id !== classReviewId || value.conversation_id !== conversationId || value.task_id !== taskId || (draftId && draftId !== value.scope.draft_id) || (sampleTestId && sampleTestId !== value.scope.sample_test_id) || (imageId && imageId !== value.scope.image_id)) throw new Error("The image-class review does not match this task, image or Sample Test. No other result was substituted.");
      setClassReview(value);
    }).catch((reason: Error) => { if (!controller.signal.aborted) setClassReviewError(reason.message); });
    return () => controller.abort();
  }, [project.id, conversationId, taskId, classReviewId, draftId, sampleTestId, imageId, humanRequestId, referenceMessageId, results]);
  const [images, setImages] = useState<ImageItem[]>([]);
  const [text, setText] = useState("");
  const [pinnedSelection,setPinnedSelection]=useState<{input:Pick<ConversationMessageInput,"image"|"reference">;name:string}>();
  const messageInput=useRef<HTMLTextAreaElement>(null);
  const composing = useRef(false);
  useEffect(()=>setPinnedSelection(undefined),[project.id,conversationId,taskId]);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [prepareMessage,setPrepareMessage]=useState<string>();
  const preparingGoal=useRef(false);
  const selectionCommand=useRef<Parameters<typeof api.selectConversationTask>[2]|undefined>(undefined);
  async function rememberTask(conversation:string,task:string){
    if(selectionCommand.current?.task_id!==task){
      const current=await api.conversationTaskSelection(project.id,conversation);
      selectionCommand.current={request_id:crypto.randomUUID(),expected_revision:current.revision,task_id:task};
    }
    const saved=await api.selectConversationTask(project.id,conversation,selectionCommand.current);
    if(saved.task_id!==task)throw new Error("A newer task selection was saved in another view. Reload before selecting the desired goal again; no model was called.");
    selectionCommand.current=undefined;
  }
  const [mobileView, setMobileView] = useState("conversation");
  const [width, setWidth] = useState(32);
  const [requests,setRequests]=useState<HumanRequest[]>([]);
  const [processing,setProcessing]=useState<ProcessingReceipt[]>([]);
  const [requestsReady,setRequestsReady]=useState(false);
  const [requestRefresh,setRequestRefresh]=useState(0);
  const assistanceChanged=useCallback(()=>setRequestRefresh(value=>value+1),[]);
  const [repairEditing,setRepairEditing]=useState(false);
  const activeRequest=requests.find(value=>value.input.id===humanRequestId && value.input.task_id===taskId);
  const requestRelation=activeRequest ? conversationSampleRelation(activeRequest,draftId,sampleTestId,imageId) : undefined;
  useEffect(()=>{
    const controller=new AbortController();setRequestsReady(false);
    if(!conversation)return()=>controller.abort();
    void api.conversationTasks(project.id,conversation,controller.signal).then(tasks=>Promise.all(tasks.map(async task=>({
      task,
      requests:await api.conversationHumanRequests(project.id,conversation,task.input.id,controller.signal),
      processing:await api.conversationProcessing(project.id,conversation,task.input.id,controller.signal),
    })))).then(values=>{if(!controller.signal.aborted){setTasks(values.map(value=>value.task));setRequests(values.flatMap(value=>value.requests));setProcessing(values.flatMap(value=>value.processing));setRequestsReady(true);}}).catch((error:Error)=>{if(!controller.signal.aborted)setError(error.message);});
    return()=>controller.abort();
  },[project.id,conversation,taskId,sampleTestId,processingOperationId,requestRefresh]);
  const updateRequest=(value:HumanRequest)=>setRequests(items=>items.map(item=>item.input.id===value.input.id ? value : item));
  const deferralCommands=useRef(new Map<string,{command_id:string;expected_revision:number;deferred:boolean}>());
  async function deferRequest(value:HumanRequest){
    if(sampleDirty.current){setError("Save or undo the current correction before deferring or reopening a request.");return;}
    const deferred=!value.deferred;
    let command=deferralCommands.current.get(value.input.id);
    if(!command||command.deferred!==deferred){command={command_id:crypto.randomUUID(),expected_revision:value.deferral_revision??0,deferred};deferralCommands.current.set(value.input.id,command);}
    try{
      const saved=await api.deferHumanRequest(project.id,value,command);updateRequest(saved);
      deferralCommands.current.delete(value.input.id);
      if(Boolean(saved.deferred)!==deferred)setError("This request changed in another view. Its latest saved state is shown; no model was called.");
      else setError("");
    }catch(error){setError((error as Error).message);}
  }
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
    }catch(error){if(alive.current && ticket===sampleNavigation.current)setError((error as Error).message);}
  }
  const root = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const selectingImage = useRef(false);
  const sampleNavigation = useRef(0);
  const captureFeedbackNavigation = () => {
    const ticket = ++sampleNavigation.current;
    return (value: HumanRequest) => {
      if (feedbackNavigationStillCurrent(ticket, sampleNavigation.current, alive.current)) void openRequest(value);
    };
  };
  const captureClassNavigation = () => {
    const ticket = ++sampleNavigation.current;
    return (value: ImageClassReview) => {
      if (!feedbackNavigationStillCurrent(ticket, sampleNavigation.current, alive.current)) return;
      onNavigate(projectWorkPath(project.id, { conversationId: value.conversation_id, taskId: value.task_id, classReviewId: value.id, draftId: value.scope.draft_id, sampleTestId: value.scope.sample_test_id, imageId: value.scope.image_id }));
      setMobileView("images");
    };
  };
  const navigationContext=projectWorkPath(project.id,{conversationId,taskId,imageId,draftId,sampleTestId,humanRequestId,classReviewId,referenceMessageId,processingOperationId,results});
  useEffect(()=>{sampleNavigation.current++;},[navigationContext]);
  const unsent = useRef("");
  const schemaDirty = useRef(false);
  const budgetDirty=useRef(false);
  const feedbackScopeDirty=useRef(new Set<string>());
  const budgetDirtyChange=useCallback((dirty:boolean)=>{budgetDirty.current=dirty;},[]);
  const sampleDirty = useRef(false);
  const formalGuard = useRef<(() => boolean) | undefined>(undefined);
  const formalGuardChange = useCallback((guard?:()=>boolean)=>{formalGuard.current=guard;},[]);
  const sampleDirtyChange = useCallback((dirty:boolean)=>{sampleDirty.current=dirty;setRepairEditing(dirty);},[]);
  const schemaDirtyChange = useCallback((dirty: boolean) => { schemaDirty.current = dirty; }, []);
  const frozen = useRef<ConversationMessageInput | undefined>(undefined);
  const stopConversation = useRef<string | null>(null);
  const stopStorageKey = `annotagent.stop-send:${project.id}`;
  const alive = useRef(true);
  const selected = images.find((image) => image.image_id === imageId) ?? (!imageId ? images[0] : undefined);
  const goalMessage = taskId ? messages.find(message=>isAnnotationGoalMessage(message) && message.input.id===tasks.find(task=>task.input.id===taskId)?.input.source_message_id) : messages.find(isAnnotationGoalMessage);
  const stopTaskNames = Object.fromEntries(tasks.map(task => [task.input.id, messages.find(message => message.input.id === task.input.source_message_id)?.input.text ?? task.input.id]));
  const stopComposer = frozen.current ? isStopMessage(frozen.current) : isStopCommand(text);
  const referenceTask=tasks.find(task=>taskId ? task.input.id===taskId : task.input.source_message_id===goalMessage?.input.id)?.input;
  const referencedMessage=messages.find(message=>message.input.id===referenceMessageId);
  const frozenReference=referencedMessage?.input.reference?.scope === "sample_candidate" ? referencedMessage.input.reference : undefined;
  const referenceMatches=Boolean(frozenReference && referencedMessage?.conversation_id===conversation && frozenReference.task_id===taskId && frozenReference.draft_id===draftId && frozenReference.sample_test_id===sampleTestId && referencedMessage?.input.image?.image_id===imageId && !humanRequestId && !results);
  function openMessageReference(message:ConversationMessage){
    const reference=message.input.reference,image=message.input.image;
    if(reference?.scope !== "sample_candidate"||!image)return;
    onNavigate(projectWorkPath(project.id,{conversationId:message.conversation_id,taskId:reference.task_id,draftId:reference.draft_id,sampleTestId:reference.sample_test_id,imageId:image.image_id,referenceMessageId:message.input.id}));
    setMobileView("images");
  }
  const referenceImage = frozen.current ? images.find((image) => image.image_id === frozen.current?.image?.image_id) : results ? images.find(image=>image.image_id===results.imageId) : selected;
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    const guard = () => (!formalGuard.current || formalGuard.current()) && (selectingImage.current || (!pending.current && (!(unsent.current || schemaDirty.current || sampleDirty.current || budgetDirty.current || feedbackScopeDirty.current.size) || window.confirm("Leave with unsaved message, Schema, budget, scope answer or sample edits? Saved workspace data remains on the server."))));
    const unload = (event: BeforeUnloadEvent) => { if (pending.current || unsent.current || schemaDirty.current || sampleDirty.current || budgetDirty.current || feedbackScopeDirty.current.size) event.preventDefault(); };
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
      setConversation(current.conversation_id ?? undefined); setImages(dataset.images); setMessages(previous => current.conversation_id ? mergeConversationMessages(previous, saved, current.conversation_id) : []);
      try {
        const previous = parsePendingStop(sessionStorage.getItem(stopStorageKey));
        if (previous && (!previous.conversation_id || previous.conversation_id === current.conversation_id) && !pending.current) {
          // Retain identity before the status GET: a failed GET is not evidence
          // that the original cancellation was rejected or safe to replace.
          if (!frozen.current && !unsent.current) {
            frozen.current = previous.input; stopConversation.current = previous.conversation_id; preparingGoal.current = false; unsent.current = previous.input.text; setText(previous.input.text);
            setStatus(t("The stop request acknowledgement is unknown. Retry preserves the original task scope; nothing was retried on reload."));
          }
          const receipt = current.conversation_id ? await stopApi.read(project.id, current.conversation_id, previous.input.id, controller.signal) : null;
          if (controller.signal.aborted) return;
          if (receipt) {
            setMessages(items => mergeConversationMessages(items, [receipt.message], receipt.message.conversation_id));
            setStopRecords(items => ({ ...items, [receipt.message.input.id]: receipt }));
            try { sessionStorage.removeItem(stopStorageKey); } catch { /* The saved receipt is enough for recovery. */ }
            if (frozen.current?.id === previous.input.id) { frozen.current = undefined; stopConversation.current = null; unsent.current = ""; setText(""); }
          }
        }
      } catch (reason) { if (!controller.signal.aborted) setError((reason as Error).message); }
      if(current.conversation_id&&!taskId&&!draftId&&!sampleTestId&&!humanRequestId&&!referenceMessageId&&!processingOperationId&&!results){
        const selection=await api.conversationTaskSelection(project.id,current.conversation_id,controller.signal);
        if(controller.signal.aborted)return;
        if(selection.task_id){
          const ownedTasks=await api.conversationTasks(project.id,current.conversation_id,controller.signal);
          if(controller.signal.aborted)return;
          if(!ownedTasks.some(task=>task.input.id===selection.task_id))throw new Error("The saved task selection is unavailable. No other task was substituted.");
          setTasks(ownedTasks);
          if(!unsent.current&&!schemaDirty.current&&!sampleDirty.current&&!budgetDirty.current)
            onNavigate(projectWorkPath(project.id,{conversationId:current.conversation_id,taskId:selection.task_id,imageId}));
        }
      }
      setReady(true);
    })().catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => controller.abort();
  }, [project.id, conversationId, taskId, draftId, sampleTestId, humanRequestId, referenceMessageId, processingOperationId, Boolean(results)]);
  async function send(prepareGoal=false) {
    if (pending.current || !ready || !text.trim() || composing.current) return;
    if(!frozen.current) {
      const intent = composerIntent(text, prepareGoal && !pinnedSelection?.input.reference);
      preparingGoal.current = intent.prepareGoal;
      frozen.current = intent.kind === "stop" ? makeStopMessage(crypto.randomUUID(), text, taskId ?? null) : { id: crypto.randomUUID(), text, image: referenceImage ? { image_id: referenceImage.image_id, sha256: referenceImage.content_hash } : null, ...pinnedSelection?.input };
    }
    const input = frozen.current;
    const stopping = isStopMessage(input);
    const retainStop = (id: string | null) => { stopConversation.current = id; try { sessionStorage.setItem(stopStorageKey, JSON.stringify({ conversation_id: id, input })); } catch { /* The frozen input and existing dirty guard protect in-view retry. */ } };
    const acceptStop = (record: StopRequestRecord) => {
      if (!isStopMessage(record.message.input) || record.message.input.id !== input.id || record.message.input.text !== input.text || record.message.input.reference.task_id !== input.reference?.task_id || record.message.conversation_id !== stopConversation.current) throw new Error("The stop receipt does not match the original command. No other task was selected.");
      setConversation(record.message.conversation_id); setMessages(items => mergeConversationMessages(items, [record.message], record.message.conversation_id)); setStopRecords(items => ({ ...items, [input.id]: record }));
      frozen.current = undefined; stopConversation.current = null; unsent.current = ""; setText(""); preparingGoal.current = false;
      try { sessionStorage.removeItem(stopStorageKey); } catch { /* Refresh can recover the same saved server record. */ }
      setStatus(t("Stop request saved. No LLM was called; saved annotations and the current canvas are unchanged.")); assistanceChanged();
    };
    if (stopping) retainStop(stopConversation.current ?? conversation ?? null);
    pending.current = true; setBusy(true); setError(""); setStatus("Saving message…");
    let messageSaved=false;
    try {
      const id = (stopping ? stopConversation.current : conversation) ?? conversation ?? (await api.createConversation(project.id)).conversation_id;
      if (stopping) {
        retainStop(id);
        const record = await stopApi.begin(project.id, id, input);
        messageSaved = true;
        if (alive.current) acceptStop(record);
        return;
      }
      const saved = await api.sendConversationMessage(project.id, id, input);
      messageSaved=true;
      if (!alive.current) return;
      setConversation(id); setMessages((items) => mergeConversationMessages(items, [saved], id));
      let selectedTask:ConversationTask|undefined;
      if(preparingGoal.current){
        const existing=await api.conversationTasks(project.id,id);
        const goal=await api.projectGoal(project.id);
        selectedTask=existing.find(task=>task.input.source_message_id===saved.input.id) ?? await api.beginConversationTask(project.id,id,{id:crypto.randomUUID(),source_message_id:saved.input.id,schema_revision:goal.revision});
        await rememberTask(id,selectedTask.input.id);
        if(!alive.current)return;
        setTasks([...existing.filter(task=>task.input.id!==selectedTask!.input.id),selectedTask]);
      }
      frozen.current = undefined; unsent.current = ""; setText(""); setPinnedSelection(undefined); setStatus("Message saved. Saving this message did not start inference.");
      if(selectedTask){
        pending.current=false;
        onNavigate(projectWorkPath(project.id,{conversationId:id,taskId:selectedTask.input.id,imageId:input.image?.image_id}));
        setPrepareMessage(saved.input.id);
        setStatus("Goal saved. Preparing model authorization; no model has been called.");
      }
    } catch (error) { if (alive.current) {
      if (stopping && stopConversation.current) {
        try { const record = await stopApi.read(project.id, stopConversation.current, input.id); if (alive.current && record) { acceptStop(record); return; } } catch { /* Preserve exact stop command; do not repeat its POST. */ }
      }
      setError((error as Error).message); setStatus(stopping ? t("The stop request acknowledgement is unknown. Retry preserves the original task scope; nothing was retried on reload.") : messageSaved ? "Message saved; goal preparation is not confirmed. Retry uses the same message and restores any saved task." : "Not confirmed saved. Retry sends the same message and frozen image reference.");
    } }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function useMessageAsGoal(message: ConversationMessage) {
    if (!conversation || pending.current || !isAnnotationGoalMessage(message)) return;
    if (schemaDirty.current || sampleDirty.current || unsent.current) { setError("Save or undo current edits before switching annotation goals."); return; }
    sampleNavigation.current++;
    pending.current=true;setBusy(true);setError("");
    try {
      const existing=await api.conversationTasks(project.id,conversation);
      const goal=await api.projectGoal(project.id);
      const task=existing.find(task=>task.input.source_message_id===message.input.id) ?? await api.beginConversationTask(project.id,conversation,{id:crypto.randomUUID(),source_message_id:message.input.id,schema_revision:goal.revision});
      await rememberTask(conversation,task.input.id);
      if(!alive.current)return;
      setTasks([...existing.filter(value=>value.input.id!==task.input.id),task]);
      // Selection is a journal operation, not permission to call models or modify another task.
      pending.current=false;
      onNavigate(projectWorkPath(project.id,{conversationId:conversation,taskId:task.input.id,imageId}));
      setStatus("Saved annotation goal selected. This selection did not start inference.");
    } catch(error){if(alive.current)setError((error as Error).message);}
    finally {pending.current=false;if(alive.current)setBusy(false);}
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
      if (alive.current) { setImages(dataset.images); setStatus("Images saved on this server. This upload did not start inference."); }
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
  const showResults = (next?:ConversationResultsContext) => {
    sampleNavigation.current++;
    onNavigate(projectWorkPath(project.id,{conversationId:conversation,imageId,draftId,sampleTestId,taskId,humanRequestId,processingOperationId,results:next}));
    setMobileView("images");
  };
  const processingNavigate = (path:string) => {
    const url=new URL(path,window.location.origin);
    const route=parseWorkspaceRoute(url.pathname,url.search);
    if(route.kind==="projectBatch" && route.projectId===project.id)showResults({batchId:route.batchId});
    else onNavigate(path);
  };
  async function openSample(draft:string,test:string,image?:string){
    const request=++sampleNavigation.current;
    try{const {sample_test}=await api.workflowSampleTest(draft,undefined,test);
      if(!sample_test || sample_test.project_id!==project.id || sample_test.draft_id!==draft || sample_test.id!==test)throw new Error("Sample Test does not belong to this Project.");
      if(!alive.current || request!==sampleNavigation.current)return;
      const keepOrigin=activeRequest && conversationSampleRelation(activeRequest,draft,test,image)!=="unrelated";
      onNavigate(projectWorkPath(project.id,{conversationId:conversation,draftId:draft,sampleTestId:test,imageId:image ?? sample_test.inputs[0]?.image_id,taskId:keepOrigin ? activeRequest.input.task_id : taskId,humanRequestId:keepOrigin ? activeRequest.input.id : undefined}));
      setMobileView("images");
    }catch(error){if(alive.current && request===sampleNavigation.current)setError((error as Error).message);}
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
        <p className="conversation-development-note">Samples and corrections are evaluations, not formal annotations. Dataset processing needs your explicit image and budget confirmation. Advanced review and export remain in the saved processing results.</p>
        <ConversationProjectBudget key={project.id} project={project.id} onDirtyChange={budgetDirtyChange}/>
        <ol className="conversation-messages" aria-label="Saved messages">
          {messages.map((message) => <li key={message.input.id}><p>{message.input.text}</p>{message.input.image && <button onClick={() => {
            const reference = message.input.image;
            const image = images.find((item) => item.image_id === reference?.image_id);
            if (!reference || !image || image.content_hash !== reference.sha256) { setError("The referenced image was removed or changed. Its historical reference remains saved; current pixels cannot stand in for that evidence."); return; }
            openImage(image.image_id);
          }}>Referenced image · {images.find((image) => image.image_id === message.input.image?.image_id)?.name ?? message.input.image.image_id}</button>}<small>Saved · {message.sequence}{goalMessage?.input.id===message.input.id ? " · Current annotation goal" : ""}</small>{isAnnotationGoalMessage(message) && <button disabled={busy || !ready} aria-pressed={goalMessage?.input.id===message.input.id} onClick={()=>void useMessageAsGoal(message)}>Use message {message.sequence} as annotation goal</button>}{message.input.reference?.scope === "sample_candidate" && <><button onClick={()=>openMessageReference(message)}>Open referenced candidate</button><small>Only this sample candidate · {message.input.reference.candidate_id} · Draft revision {message.input.reference.draft_revision}. This is not a project-wide goal.</small><ConversationFeedbackCard onSetup={() => onNavigate(conversationSettingsPath(project.id, "providers", navigationContext))} key={`${project.id}:${message.input.id}`} project={project.id} message={message} requests={requests} requestsReady={requestsReady} onAssistance={assistanceChanged} onSample={(draft,test,image)=>void openSample(draft,test,image)} captureCanvasNavigation={captureFeedbackNavigation} captureClassNavigation={captureClassNavigation} onScopeDirtyChange={dirty=>{if(dirty)feedbackScopeDirty.current.add(message.input.id);else feedbackScopeDirty.current.delete(message.input.id);}} onOpen={value=>void openRequest(value)} /></>}{message.input.reference?.scope === "stop_request" && <ConversationStopCard project={project.id} message={message} initial={stopRecords[message.input.id]} taskNames={stopTaskNames} onChanged={assistanceChanged} onDirtyChange={dirty => { if (dirty) feedbackScopeDirty.current.add(message.input.id); else feedbackScopeDirty.current.delete(message.input.id); }} />}</li>)}
        </ol>
        {conversation && draftId && sampleTestId && <section className="conversation-processing" aria-label="Process this dataset">
          {processingOperationId ? <JourneyConfirm key={`${draftId}:${sampleTestId}`} projectId={project.id} draftId={draftId} testId={sampleTestId} imageId={imageId} operationId={processingOperationId==="preview" ? undefined : processingOperationId} expectedConversation={conversation} viewingBatchId={results?.batchId} stayOnReceipt
            confirmationPath={id=>projectWorkPath(project.id,{conversationId:conversation,draftId,sampleTestId,imageId,taskId,humanRequestId,processingOperationId:id})}
            backPath={projectWorkPath(project.id,{conversationId:conversation,draftId,sampleTestId,imageId,taskId,humanRequestId})} onNavigate={processingNavigate} />
          : <button disabled={repairEditing} onClick={()=>onNavigate(projectWorkPath(project.id,{conversationId:conversation,draftId,sampleTestId,imageId,taskId,humanRequestId,processingOperationId:"preview"}))}>Review dataset processing</button>}
        </section>}
        {processing.length>0 && <section className="conversation-consent" aria-label="Saved processing tasks"><h3>Processing tasks</h3>{processing.map(operation=><article className="conversation-consent" key={operation.id}>
          <h4>{operation.authorization.goal.goal || operation.authorization.plan_name}</h4>
          <p>{operation.phase==="started" ? "Processing was started. Open its saved results for current progress." : operation.phase==="published_start_failed" ? "Plan published; processing did not start." : `Saved operation: ${operation.phase}`}</p>
          <p>{operation.authorization.image_count} images · Schema revision {operation.authorization.conversation?.schema.revision} · Plan revision {operation.authorization.revision}</p>
          {operation.error && <p role="alert">{operation.error}</p>}
          {operation.batch_id && operation.id !== processingOperationId && operation.batch_id!==results?.batchId && <ConversationBatchStatus projectId={project.id} batchId={operation.batch_id} />}
          {operation.batch_id && <button onClick={()=>processingNavigate(projectBatchPath(project.id,operation.batch_id!))}>Open processing results</button>}
        </article>)}</section>}
        {conversation && <ConversationHumanRequests requests={requests} taskId={referenceTask?.id} activeId={humanRequestId} ready={requestsReady} onRefresh={()=>{if(sampleDirty.current){setError("Save or undo this correction before refreshing requests.");return;}setRequestRefresh(value=>value+1);}} onOpen={value=>void openRequest(value)} onCancel={cancelRequest} onRetry={retryContinuation} onDefer={deferRequest} onInspect={value=>onNavigate(projectBuildPath(project.id,"pipeline",{draftId:value.resume_draft_id!}))}/>}
        {conversation && goalMessage && <ConversationSchemaCard prepareRequested={prepareMessage===goalMessage.input.id} key={`${conversation}:${goalMessage.input.id}`} project={project.id} conversation={conversation} message={goalMessage.input.id} onDirtyChange={schemaDirtyChange} onAssistance={assistanceChanged} onSample={(draft,test,image)=>void openSample(draft,test,image)} onSetup={selectedTask=>onNavigate(conversationSettingsPath(project.id,"providers",projectWorkPath(project.id,{conversationId:conversation,taskId:selectedTask ?? taskId,imageId,draftId,sampleTestId,humanRequestId,referenceMessageId,processingOperationId,results})))} />}
        {conversation && taskId && !goalMessage && <p role="status">{requestsReady ? "The selected annotation task is not available in this conversation. Select a saved message; no other task was substituted." : "Loading the selected annotation task…"}</p>}
        {activeRequest?.status==="applied" && activeRequest.resume_draft_id && <ConversationRepairCard key={activeRequest.input.id} project={project.id} request={activeRequest} editing={repairEditing} onAssistance={assistanceChanged} onSample={(draft,test,image)=>void openSample(draft,test,image)} />}
        <form onSubmit={(event) => { event.preventDefault(); if (!composing.current) void send(!goalMessage&&!pinnedSelection); }} className="conversation-composer">
          <label htmlFor="conversation-message">Your message</label>
          <textarea ref={messageInput} id="conversation-message" value={text} disabled={!ready || busy || Boolean(frozen.current)} rows={3} placeholder="Find cups, but not bottles" onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }} onChange={(event) => { unsent.current = event.target.value; setText(event.target.value); }} />
          {stopComposer ? <small>{t("Standalone stop control · No LLM or image submission. Multiple active operations require an explicit choice.")}</small> : pinnedSelection?.input.reference?.scope === "sample_candidate" ? <div className="conversation-candidate-reference" aria-label="Message candidate reference"><strong>Only this saved candidate</strong><span>{pinnedSelection.name} · {pinnedSelection.input.reference.candidate_id} · Draft revision {pinnedSelection.input.reference.draft_revision}</span><small>Changing the displayed image does not change this reference. Saving the message does not edit the annotation.</small><button type="button" disabled={busy||Boolean(frozen.current)} onClick={()=>setPinnedSelection(undefined)}>Remove candidate reference</button></div> : <small>{referenceImage ? `Image reference: ${referenceImage.name}` : "No image reference · Project-level message"}</small>}
          {stopComposer ? <button className="danger-button" disabled={!ready || busy} type="submit">{t(busy ? "Saving stop request…" : frozen.current ? "Retry same stop request" : taskId ? "Stop selected task" : "Stop active work")}</button> : !goalMessage&&!pinnedSelection&&!frozen.current ? <><button className="primary" disabled={!ready||busy||!text.trim()} type="submit">Save goal and prepare labels</button><button disabled={!ready||busy||!text.trim()} type="button" onClick={()=>void send(false)}>Save message</button><small>Preparing labels opens the model and data authorization. It does not call a model or publish a workflow.</small></> : <button className="primary" disabled={!ready || busy || !text.trim()} type="submit">{busy ? "Saving…" : frozen.current ? "Retry saving message" : "Save message"}</button>}
        </form>
      </section>
      <div className="conversation-divider" role="separator" aria-label="Resize conversation panel" aria-orientation="vertical" tabIndex={0} aria-valuemin={25} aria-valuemax={50} aria-valuenow={width}
        onKeyDown={(event) => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); setWidth((current) => Math.max(25, Math.min(50, current + (event.key === "ArrowRight" ? 2 : -2)))); } }}
        onPointerDown={(event) => event.currentTarget.setPointerCapture(event.pointerId)}
        onPointerMove={(event) => { if (!event.currentTarget.hasPointerCapture(event.pointerId)) return; const bounds = root.current?.getBoundingClientRect(); if (bounds) setWidth(Math.round(Math.max(25, Math.min(50, (event.clientX - bounds.left) / bounds.width * 100)))); }} />
      <section className="conversation-image-panel" aria-label="Project images">
        {referenceMessageId && !referenceMatches ? <p role={ready ? "alert" : "status"}>{ready ? "The message reference does not match this conversation, task, image or sample. No other result was substituted." : "Loading the saved message reference…"}</p> : results ? <><div className="conversation-image-tools"><h2>Dataset results</h2><button onClick={()=>showResults()}>Return to sample canvas</button></div>{!requestsReady ? <p role="status">Loading saved processing tasks…</p> : processing.some(operation=>operation.batch_id===results.batchId) ? <ConversationBatchResults key={results.batchId} project={project} context={results} returnPath={projectWorkPath(project.id,{conversationId:conversation,imageId,draftId,sampleTestId,taskId,humanRequestId,processingOperationId,results})} onSelect={showResults} onNavigate={onNavigate} onNavigationGuardChange={formalGuardChange} /> : <p role="alert">This Batch is not linked to this conversation. No other result was substituted.</p>}</> : <>
        {activeRequest && (requestRelation==="baseline" || requestRelation==="comparison") && <aside className="conversation-consent" aria-label="Sample origin"><p>{requestRelation==="comparison" ? "Sample from the revised plan. The original correction remains separate; improvement has not been established." : "Another image from the original sample. The requested correction belongs to a different image."}</p><button onClick={()=>void openRequest(activeRequest)}>Return to original correction</button></aside>}
        <div className="conversation-image-tools"><h2>{images.length ? `${images.length} images` : "Your images"}</h2><label className="conversation-upload">Add images<input type="file" accept="image/png,image/jpeg" multiple disabled={busy || !ready} onChange={(event) => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ""; void upload(files); }} /></label></div>
        {classReviewId ? classReviewError ? <p role="alert">{classReviewError}</p> : !classReview || classReview.id !== classReviewId ? <p role="status">Loading saved image-class review…</p> : <ConversationSampleCanvas key={classReview.id} project={project.id} draft={classReview.scope.draft_id} test={classReview.scope.sample_test_id} image={images.find(item=>item.image_id===classReview.scope.image_id)} imageClassReview={classReview} onClassChanged={value=>setClassReview(current=>mergeImageClassReview(current,value))} onClassReturn={()=>onNavigate(projectWorkPath(project.id,{conversationId:classReview.conversation_id,taskId:classReview.task_id,draftId:classReview.scope.draft_id,sampleTestId:classReview.scope.sample_test_id,imageId:classReview.scope.image_id}))} onClassRevision={draft=>onNavigate(projectBuildPath(project.id,"pipeline",{draftId:draft,workspaceReturn:navigationContext}))} onDirtyChange={sampleDirtyChange} onOpen={(draft,test,image)=>void openSample(draft,test,image)} /> : humanRequestId && (!activeRequest || requestRelation==="unrelated") ? <p role="status">{requestsReady ? "Human request not found in this task. No other result was substituted." : "Loading saved human request…"}</p> : draftId && sampleTestId ? <ConversationSampleCanvas messageReference={referenceMessageId && referenceMatches ? referencedMessage?.input : undefined} referenceTask={referenceTask} onReference={(input,name)=>{if(pending.current||frozen.current)return;setPinnedSelection({input,name});setMobileView("conversation");window.requestAnimationFrame(()=>messageInput.current?.focus());}} project={project.id} draft={draftId} test={sampleTestId} image={selected} humanRequest={requestRelation==="subject" ? activeRequest : undefined} onAnswered={updateRequest} onDirtyChange={sampleDirtyChange} onOpen={(draft,test,image)=>void openSample(draft,test,image)} /> : imageId && !selected && ready ? <p role="alert">This image is not available in this Project. Select an existing image below.</p> : selected ? <figure className="conversation-image"><img src={selected.url} alt={selected.name} /><figcaption>{selected.name} · Original image · No annotation overlay</figcaption></figure> : !ready ? <p role="status">Loading saved images…</p> : <div className="conversation-image-empty"><h3>Start with your own images</h3><p>PNG or JPEG · up to 25 MB per image. Files are uploaded to this AnnotAgent server, not sent to a model.</p></div>}
        <nav className="conversation-thumbnails" aria-label="Select image">{images.map((image) => <button key={image.image_id} aria-label={image.name} aria-current={image.image_id === selected?.image_id ? "true" : undefined} onClick={() => openImage(image.image_id)}><img loading="lazy" src={image.url} alt="" /><span>{image.name}</span></button>)}</nav>
        </>}
      </section>
    </div>
    <p className="conversation-status" role="status">{status || (ready ? "Saved workspace loaded" : "Loading saved workspace…")}</p>
    {error && <p role="alert">{error}</p>}
  </section>;
}
