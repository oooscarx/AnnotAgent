import { useEffect, useRef, useState } from "react";
import "./delivery-intake.css";
import { DeliveryReview } from "./DeliveryReview";
import type { DeliveryService } from "./deliveryService";
import { Disclosure } from "./Disclosure";
import { DeliveryPackage } from "./DeliveryPackage";

export type DeliveryLabel = { stable_id: string; display_name: string; aliases: string[]; include: string; exclude: string };
type Target = { annotation_kind: string; framework: string; export_profile: string; profile_revision: number };
type Split = { train_percent: number; seed: number; preserve_existing: boolean; keep_known_groups_together: boolean };
export type IntakeInput = { command_id: string; expected_revision: number; image_ids: string[] | null; label_spec: DeliveryLabel[] | null; training_target: Target | null; split_policy: Split };
export type IntakeView = { saved: null | { revision: number; content_sha256: string; intent: { dataset_scope: null | { image_id: string }[]; label_spec: DeliveryLabel[] | null; training_target: Target | null; split_policy: Split } }; missing_slots: string[]; blockers: string[]; maximum_sample_images: number; execution_authorized: boolean };
export interface DeliveryIntakeService {
  read(project: string, task: string, signal?: AbortSignal): Promise<IntakeView>;
  save(project: string, task: string, input: IntakeInput): Promise<IntakeView>;
  prepare?(project: string, task: string, input: {command_id:string;expected_revision:number;expected_sha256:string}): Promise<{id:string;revision:number}>;
}
const target: Target = { annotation_kind: "bounding_box", framework: "ultralytics", export_profile: "ultralytics_yolo_detection", profile_revision: 1 };
const split: Split = { train_percent: 80, seed: 0, preserve_existing: true, keep_known_groups_together: true };
/** Preserve explicit IDs/semantics for unchanged names; ordering is the export class order. */
export function intakeLabels(text: string, previous: DeliveryLabel[], createId: () => string = () => crypto.randomUUID()): DeliveryLabel[] {
  return text.split("\n").map(s => s.trim()).filter(Boolean).map(name => previous.find(p => p.display_name === name) || { stable_id: createId(), display_name: name, aliases: [], include: "", exclude: "" });
}

export function DeliveryIntake({ service, delivery, project, task, images, locked = false }: { service: DeliveryIntakeService; delivery?:DeliveryService; project: string; task: string; images: { id: string; name: string; src?:string }[]; locked?: boolean }) {
  const [view, setView] = useState<IntakeView>();
  const [ids, setIds] = useState<string[]>([]);
  const [labels, setLabels] = useState<DeliveryLabel[]>([]);
  const [newLabels,setNewLabels]=useState("");
  const [splitPolicy,setSplitPolicy]=useState<Split>(split);
  const [selectedTarget, setTarget] = useState<Target | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [reviewOpen,setReviewOpen]=useState(()=>new URL(location.href).searchParams.has("delivery_image"));
  const [reviewVisited,setReviewVisited]=useState(reviewOpen);
  const [objectEditing,setObjectEditing]=useState(false);
  const retry = useRef<{ signature: string; input: IntakeInput } | null>(null);
  const inFlight = useRef(false);
  const preparation = useRef<{command_id:string;expected_revision:number;expected_sha256:string} | null>(null);
  const [prepared,setPrepared]=useState("");
  const supportedTarget = selectedTarget && selectedTarget.annotation_kind === target.annotation_kind && selectedTarget.framework === target.framework && selectedTarget.export_profile === target.export_profile && selectedTarget.profile_revision === target.profile_revision;
  const apply = (value: IntakeView) => {
    setPrepared("");
    setView(value); setIds(value.saved?.intent.dataset_scope?.map(i => i.image_id) || []);
    setLabels(value.saved?.intent.label_spec || []);setNewLabels("");
    setSplitPolicy(value.saved?.intent.split_policy || split);
    setTarget(value.saved?.intent.training_target || null); setDirty(false);
  };
  useEffect(() => {
    const ctrl = new AbortController();
    void service.read(project, task, ctrl.signal).then(value => { if (!ctrl.signal.aborted) apply(value); }).catch(e => { if (!ctrl.signal.aborted) setError(e.message); });
    return () => ctrl.abort();
  }, [service, project, task]);
  useEffect(() => {
    const guard = (e: Event) => { if (dirty && !window.confirm("离开会丢失尚未保存的交付信息，继续？")) e.preventDefault(); };
    const unload = (e: BeforeUnloadEvent) => { if (dirty) { e.preventDefault(); e.returnValue = ""; } };
    window.addEventListener("ui-preview:before-navigate", guard); window.addEventListener("beforeunload", unload);
    return () => { window.removeEventListener("ui-preview:before-navigate", guard); window.removeEventListener("beforeunload", unload); };
  }, [dirty]);
  const save = async () => {
    if (!view || inFlight.current || locked || objectEditing) return;
    inFlight.current = true; setBusy(true); setError("");
    const signature = JSON.stringify([view.saved?.revision || 0, ids, labels, newLabels, selectedTarget,splitPolicy]);
    if (retry.current?.signature !== signature) {const next=[...labels,...intakeLabels(newLabels,[])];retry.current = { signature, input: { command_id: crypto.randomUUID(), expected_revision: view.saved?.revision || 0, image_ids: ids.length ? ids : null, label_spec: next.length?next:null, training_target: selectedTarget, split_policy:splitPolicy } };}
    try { apply(await service.save(project, task, retry.current.input)); retry.current = null; }
    catch (e) { setError((e as Error).message); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const prepare=async()=>{
    if(!view?.saved || !service.prepare || busy || locked || dirty || inFlight.current)return;
    inFlight.current=true;setBusy(true);setError("");setPrepared("");
    if(preparation.current?.expected_sha256!==view.saved.content_sha256 || preparation.current.expected_revision!==view.saved.revision)preparation.current={command_id:crypto.randomUUID(),expected_revision:view.saved.revision,expected_sha256:view.saved.content_sha256};
    try {await service.prepare(project,task,preparation.current);setPrepared("目标规范已准备。接下来查看方案生成授权；本操作未调用模型、发布或处理图片。");}
    catch(e){setError((e as Error).message);}finally{inFlight.current=false;setBusy(false);}
  };
  return <section className="delivery-intake" aria-label="训练数据交付信息">
    <div className="delivery-intake-heading"><strong>训练数据包</strong><button type="button" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>{expanded ? "收起信息" : view?.saved ? "查看交付信息" : "定义交付目标"}</button></div>
    {error && <p role="alert" className="error">{error}</p>}
    {!view && !error && <p role="status">读取已保存信息…</p>}
    {view?.saved && !expanded && <p>{view.saved.intent.dataset_scope?.length || 0} 张图片 · {view.saved.intent.label_spec?.map(l => l.display_name).join("、") || "类别待确定"} · {view.missing_slots.length ? "仍有信息待补齐" : "信息已保存，尚未批准执行"}</p>}
    {view?.saved && !view.missing_slots.length && !view.blockers.length && service.prepare && <div className="delivery-intake-actions"><button type="button" disabled={busy||locked||dirty} onClick={()=>void prepare()}>{busy?"保存中…":"确认目标并准备方案"}</button><small>复用已填写的类别和任务类型，不再要求填写内部 ID。模型执行仍需授权。</small></div>}
    {prepared && <p role="status">{prepared}</p>}
    {expanded && view && <form aria-disabled={locked} onSubmit={e => { e.preventDefault(); void save(); }}>
      {objectEditing&&<p role="status">请先保存或撤销对象修改，再调整交付范围；收起审核面板不会丢失编辑。</p>}
      <fieldset disabled={objectEditing}>
      {locked && <p role="status">任务执行中，暂时不能保存交付信息；已有输入保留。</p>}
      <fieldset disabled={busy}><legend>用哪些图片？</legend><div className="delivery-intake-selection"><button type="button" onClick={() => { setIds(images.map(i => i.id)); setDirty(true); }}>选择当前 {images.length} 张图片</button><span>已选 {ids.length} 张</span></div>
        {!images.length && <p>先使用输入框的图片按钮上传图片。未上传的文件不属于已保存范围。</p>}
        <div className="delivery-intake-images">{images.map(image => <label key={image.id}><input type="checkbox" checked={ids.includes(image.id)} onChange={e => { setIds(e.target.checked ? [...ids, image.id] : ids.filter(id => id !== image.id)); setDirty(true); }} /><span>{image.name}</span></label>)}</div>
      </fieldset>
      <fieldset disabled={busy}><legend>标注哪些类别？</legend>
        {labels.map((label,index)=><div className="delivery-label-edit" key={label.stable_id}>
          <label>类别 {index+1} 名称<input required value={label.display_name} onChange={e=>{setLabels(current=>current.map(l=>l.stable_id===label.stable_id?{...l,display_name:e.target.value}:l));setDirty(true);}}/></label>
          <Disclosure title={`类别 ${index+1} 规则与排序`}>
            {(["aliases","include","exclude"] as const).map(field=><label key={field}>{field==="aliases"?"别名（每行一个）":field==="include"?"包含规则":"排除规则"}<textarea value={field==="aliases"?label.aliases.join("\n"):label[field]} onChange={e=>{const value=e.target.value;setLabels(current=>current.map(l=>l.stable_id===label.stable_id?{...l,[field]:field==="aliases"?value.split("\n"):value}:l));setDirty(true);}}/></label>)}
            <div className="delivery-intake-actions"><button type="button" disabled={index===0} onClick={()=>{setLabels(current=>{const next=[...current];[next[index-1],next[index]]=[next[index],next[index-1]];return next;});setDirty(true);}}>上移类别 {index+1}</button><button type="button" onClick={()=>{setLabels(current=>current.filter(l=>l.stable_id!==label.stable_id));setDirty(true);}}>从新交付版本移除类别 {index+1}</button></div>
          </Disclosure>
        </div>)}
        <label>{labels.length?"添加类别（每行一个）":"类别名称（每行一个）"}<textarea rows={3} value={newLabels} placeholder={"杯子\n瓶子"} onChange={e=>{setNewLabels(e.target.value);setDirty(true);}}/></label>
        <small>改名称和规则保留类别身份；排序决定新包的 class_id。保存后产生新交付版本，不更改旧 Run 或旧数据包。</small>
      </fieldset>
      <label>训练什么任务？<select disabled={busy} value={selectedTarget ? supportedTarget ? target.export_profile : "unsupported" : ""} onChange={e => { setTarget(e.target.value ? target : null); setDirty(true); }}><option value="">请选择任务和训练格式</option><option value="ultralytics_yolo_detection">框出目标 · Ultralytics YOLO Object Detection</option>{selectedTarget && !supportedTarget && <option disabled value="unsupported">{selectedTarget.annotation_kind} · {selectedTarget.export_profile}（尚无完整交付预设）</option>}</select></label>
      <p>本预设交付目标框，不会把分类或分割需求自动改成检测。建议按图片组划分训练/验证 {splitPolicy.train_percent}/{100-splitPolicy.train_percent}；整图需人工确认，模型未检出不等于确认负样本。</p>
      <Disclosure title="调整数据划分"><label>训练集比例（百分比）<input disabled={busy} type="number" min={1} max={99} step={1} required value={splitPolicy.train_percent} onChange={e=>{setSplitPolicy(current=>({...current,train_percent:Number(e.target.value)}));setDirty(true);}}/></label><p>保留已有划分和已知来源组，不拆组凑比例；实际数量在打包检查报告中展示。仅修改比例，保留当前 seed 与其他约束。</p></Disclosure>
      {view.blockers.map((b, i) => <p role="alert" key={i}>{b}</p>)}
      <div className="delivery-intake-actions"><button type="button" disabled={busy || !dirty} onClick={() => { apply(view); setError(""); retry.current = null; }}>取消修改</button><button type="submit" disabled={busy || locked || !dirty}>{busy ? "保存中…" : "保存交付信息"}</button><span role="status">{dirty ? "尚未保存" : view.saved ? "已保存到服务器" : "等待填写"}</span></div>
      <small>保存不会调用模型或开始处理。样例最多 {view.maximum_sample_images} 张，执行前需要确认模型、数据目的地和费用范围。</small>
      </fieldset>
    </form>}
    {delivery && view?.saved && <>
      {!view.missing_slots.length && !view.blockers.length && <Disclosure title="检查正式训练图片（整图审核）" open={reviewOpen} onToggle={e=>{setReviewOpen(e.currentTarget.open);if(e.currentTarget.open)setReviewVisited(true);}}>{(reviewVisited||reviewOpen)&&<DeliveryReview key={`${task}:${view.saved.revision}`} service={delivery} project={project} task={task} labels={view.saved.intent.label_spec||[]} locked={locked||dirty} onEditingState={setObjectEditing} images={(view.saved.intent.dataset_scope || []).flatMap(i=>{const image=images.find(a=>a.id===i.image_id);return image?[image]:[{id:i.image_id,name:"原图不可用"}];})}/>}</Disclosure>}
      <DeliveryPackage key={`${task}:${view.saved.revision}`} service={delivery} project={project} task={task} locked={locked||dirty||!!view.missing_slots.length||!!view.blockers.length} scope={{revision:view.saved.revision,content_sha256:view.saved.content_sha256,image_ids:(view.saved.intent.dataset_scope||[]).map(i=>i.image_id)}} onInspect={id=>{const url=new URL(location.href);url.searchParams.set("delivery_image",id);url.searchParams.delete("delivery_run");history.pushState(history.state,"",url);window.dispatchEvent(new PopStateEvent("popstate"));setReviewOpen(true);}}/>
    </>}
  </section>;
}
