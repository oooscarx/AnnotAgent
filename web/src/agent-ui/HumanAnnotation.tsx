import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import type {Annotation,ProjectSummary} from "../types";
import {AnnotationCanvas} from "../components/AnnotationCanvas";
import {editableHumanKinds,newHumanAnnotation,relabelHumanAnnotation} from "./humanGeometry";
import {reviewPath} from "./reviewNavigation";
export type HumanAnnotationService=Pick<typeof api,"projectSummary"|"createAnnotation"|"review">;
export function HumanAnnotation({service,projectId,runId,imageId,imageUrl,workspaceId,queueOffset}:{service:HumanAnnotationService;projectId:string;runId:string;imageId:string;imageUrl:string;workspaceId:string;queueOffset:number}) {
  const [project,setProject]=useState<ProjectSummary>();
  const [draft,setDraft]=useState<Annotation>();
  const [undo,setUndo]=useState<Annotation[]>([]);
  const [submitted,setSubmitted]=useState(false);
  const [saved,setSaved]=useState(false);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const lock=useRef(false);
  const key=`annotagent.human-create.${workspaceId}.${projectId}.${runId}.${imageId}`;
  useEffect(()=>{const c=new AbortController();void service.projectSummary(projectId,c.signal).then(({project:p})=>{
    if(c.signal.aborted)return;if(p.project_id!==projectId)throw new Error("标签定义的项目归属不匹配");setProject(p);
    const cached=JSON.parse(localStorage.getItem(key)||"null");
    if(cached){if(cached.draft?.image_id!==imageId||cached.draft?.source!=="human"||!p.annotation_schema.some(t=>t.id===cached.draft.task_id))throw new Error("本地新增标注记录与当前图片或标签定义不匹配，未自动提交");setDraft(cached.draft);setSubmitted(cached.submitted===true);}
  }).catch(e=>{if(!c.signal.aborted)setError(e.message);});return()=>c.abort();},[service,projectId,imageId,key]);
  useEffect(()=>{const guard=(e:Event)=>{if((busy||!!draft&&!saved)&&!confirm("新增标注尚未确认保存，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(busy||!!draft&&!saved){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[busy,draft,saved]);
  const edit=(next:Annotation)=>{if(submitted||busy||saved)return;if(draft)setUndo(items=>[...items.slice(-99),draft]);else setUndo([]);setDraft(next);try{localStorage.setItem(key,JSON.stringify({draft:next,submitted:false}));}catch{setError("本地暂存失败；请勿关闭页面。");}};
  const undoEdit=()=>{if(busy||submitted||saved||!undo.length)return;const next=undo.at(-1)!;try{localStorage.setItem(key,JSON.stringify({draft:next,submitted:false}));setDraft(next);setUndo(items=>items.slice(0,-1));}catch{setError("无法暂存撤销结果；当前编辑保留。");}};
  const acknowledge=(annotation:Annotation)=>{if(!draft||annotation.id!==draft.id||annotation.image_id!==imageId||annotation.source!=="human")throw new Error("保存结果身份不匹配");setDraft(annotation);setSaved(true);try{localStorage.removeItem(key);}catch{/* Confirmed server save is authoritative. */}};
  const perform=async(action:()=>Promise<void>)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await action();}catch(e){setError((e as Error).message);}finally{lock.current=false;setBusy(false);}};
  const save=()=>void perform(async()=>{
    if(!draft||saved)return;
    // Freeze the UUID and exact body before sending; unknown responses never create a new command.
    localStorage.setItem(key,JSON.stringify({draft,submitted:true}));setSubmitted(true);
    const result=await service.createAnnotation(runId,draft);acknowledge(result.annotation);
  });
  const recover=()=>void perform(async()=>{if(!draft)return;const item=await service.review(draft.id,undefined,projectId);if(item.run_id!==runId||item.image_id!==imageId)throw new Error("保存记录不属于当前 Run/图片");acknowledge(item.annotation);});
  return <section className="native-human-annotation"><h2>补充遗漏标注</h2><p>新几何是人工编辑起点，不是模型结果。保存到当前 Run 后仍需单独审核，不会接受原来的对象。</p>{error&&<p role="alert">{error}</p>}
    {!project&&!error&&<p role="status">读取项目标签定义…</p>}
    {!draft&&project&&<><div className="actions">{project.annotation_schema.filter(t=>editableHumanKinds.has(t.kind)&&t.labels.length).map(t=><button key={t.id} onClick={()=>edit(newHumanAnnotation(imageId,t,crypto.randomUUID(),new Date().toISOString()))}>新增 {t.display_name} · {t.kind}</button>)}</div>{!project.annotation_schema.some(t=>editableHumanKinds.has(t.kind)&&t.labels.length)&&<p>当前项目没有可人工绘制的标签组。请先在项目标签管理中定义框、多边形、折线或关键点。</p>}</>}
    {draft&&<><AnnotationCanvas imageUrl={imageUrl} annotations={[draft]} selectedId={draft.id} onSelect={()=>{}} onChange={edit} readOnly={busy||submitted||saved} compactList/>
      <label>新增标注类别<select aria-label="新增标注类别" value={draft.label} disabled={busy||submitted||saved} onChange={e=>edit(relabelHumanAnnotation(draft,e.target.value))}>{project?.annotation_schema.find(t=>t.id===draft.task_id)?.labels.map(label=><option key={label} value={label}>{label}</option>)}</select></label>
      {!saved&&<button disabled={busy||submitted||!undo.length} onClick={undoEdit}>撤销新增标注编辑</button>}
      {saved?<p role="status">新增标注已保存，尚未接受。<a href={reviewPath(projectId,draft.id,queueOffset)}>审核新增对象</a></p>:<div className="actions"><button disabled={busy} onClick={save}>{submitted?"使用原请求重试保存":"保存新增标注"}</button>{submitted?<><p>请求已发出；若响应丢失，先核实服务端。编辑已锁定，重试沿用同一对象和内容。</p><button disabled={busy} onClick={recover}>核实保存结果</button></>:<button disabled={busy} onClick={()=>{try{localStorage.removeItem(key);setDraft(undefined);}catch{setError("无法清除本地编辑，尚未取消。");}}}>取消新增</button>}</div>}
    </>}
  </section>;
}
