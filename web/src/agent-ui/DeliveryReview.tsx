import { useEffect, useMemo, useRef, useState } from "react";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import type { Annotation } from "../types";
import type { DemoAnnotationOrigin, DeliveryImageView, DeliveryReviewInput, DeliveryReviewSummary, DeliveryService } from "./deliveryService";
import { demoOriginLabel } from "./demoDeliveryPresentation";
import {
  sampleVisualSelection,
  type DeliveryFormalResult,
  type DeliverySampleResult,
  type SampleVisualSelection,
} from "./deliveryVisualSelection";
import type { FormalVisualSelection } from "./mainline";

type Image = { id: string; name: string; src?: string };
type Mode = "sample" | "formal";
type Selection = { mode: Mode; image: string };

export type DeliveryReviewProps = {
  service: DeliveryService;
  project: string;
  task: string;
  images: Image[];
  labels?: { stable_id: string; display_name: string }[];
  sampleResult?: DeliverySampleResult | null;
  formalResult?: DeliveryFormalResult | null;
  locked?: boolean;
  onEditingState?: (active: boolean) => void;
  onVisualSelection?: (selection: SampleVisualSelection) => void;
  onSampleIssue?: (selection: SampleVisualSelection) => void;
  onFormalSelection?: (selection: FormalVisualSelection) => void;
  annotationOrigins?:Record<string,Record<string,DemoAnnotationOrigin>>;
};

const fromUrl = (images: Image[], hasSample: boolean): Selection => {
  const query = new URL(location.href).searchParams;
  return {
    mode: query.get("delivery_view") === "sample" && hasSample ? "sample" : "formal",
    image: images.find((item) => item.id === query.get("delivery_image"))?.id ?? images[0]?.id ?? "",
  };
};

/** Sample feedback and formal review share a canvas, but never a write target. */
export function DeliveryReview({
  service, project, task, images, labels = [], sampleResult = null,
  formalResult: formalResultProp, locked = false, onEditingState,
  onVisualSelection, onSampleIssue, onFormalSelection, annotationOrigins = {},
}: DeliveryReviewProps) {
  const [formalResult, setFormalResult] = useState<DeliveryFormalResult | null | undefined>(
    formalResultProp !== undefined ? formalResultProp : service.formalResult ? undefined : null,
  );
  const [selection, setSelection] = useState(() => fromUrl(images, !!sampleResult));
  const [view, setView] = useState<DeliveryImageView>();
  const [summary, setSummary] = useState<DeliveryReviewSummary>();
  const [summaryItems, setSummaryItems] = useState<DeliveryReviewSummary["items"]>([]);
  const [summaryCursor, setSummaryCursor] = useState<string | null>(null);
  const [selected, setSelected] = useState<string>();
  const [draft, setDraft] = useState<Annotation>();
  const [reason, setReason] = useState("");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [reload, setReload] = useState(0);
  const [readable, setReadable] = useState(false);
  const pending = useRef(false);
  const confirmRetry = useRef<{ signature: string; input: DeliveryReviewInput } | undefined>(undefined);
  const editRetry = useRef<{ signature: string; input: import("./deliveryService").DeliveryObjectEdit } | undefined>(undefined);
  const createRetry = useRef<{ signature: string; input: import("./deliveryService").DeliveryObjectCreate } | undefined>(undefined);

  const image = images.find((item) => item.id === selection.image);
  const sampleImage = sampleResult?.images.find((item) => item.image_id === selection.image);
  const formalImage = formalResult?.images.find((item) => item.image_id === selection.image);
  const formalRun = formalImage?.child_run_id ?? null;
  const original = view?.snapshot.annotations.find((item) => item.id === selected);
  const object = draft ?? original;
  const creating = !!draft && !original;
  const dirty = !!draft && JSON.stringify(draft) !== JSON.stringify(original);
  const activeAnnotations = useMemo(() => {
    if (selection.mode === "sample") return sampleImage?.annotations ?? [];
    if (!formalResult || !formalImage) return [];
    const saved = view?.snapshot.annotations
      .filter((item) => item.review_status !== "rejected")
      .map((item) => draft?.id === item.id ? draft : item) ?? [];
    return creating && draft ? [...saved, draft] : saved;
  }, [creating, draft, formalImage, formalResult, sampleImage, selection.mode, view]);

  useEffect(() => {
    onEditingState?.(dirty || busy);
    return () => onEditingState?.(false);
  }, [busy, dirty, onEditingState]);

  useEffect(() => {
    if (formalResultProp !== undefined) setFormalResult(formalResultProp);
    else if (!service.formalResult) setFormalResult(null);
  }, [formalResultProp, service]);

  useEffect(() => {
    if (sampleResult && (sampleResult.project_id !== project || sampleResult.task_id !== task)) {
      setError("Sample Test 不属于当前 Project/Task，未显示其结果。");
    }
  }, [project, sampleResult, task]);

  useEffect(() => {
    if (formalResultProp !== undefined || !service.formalResult) return;
    const controller = new AbortController();
    setFormalResult(undefined);
    void service.formalResult(project, task, controller.signal)
      .then((result) => { if (!controller.signal.aborted) setFormalResult(result); })
      .catch((cause: Error) => { if (!controller.signal.aborted) setError(cause.message); });
    return () => controller.abort();
  }, [formalResultProp, project, reload, service, task]);

  const readSummary = (cursor?: string) => {
    if (!service.reviewSummary) return;
    const controller = new AbortController();
    void service.reviewSummary(project, task, cursor, controller.signal)
      .then((next) => {
        if (controller.signal.aborted) return;
        setSummary(next);
        setSummaryItems((current) => cursor
          ? [...current, ...next.items.filter((item) => !current.some((saved) => saved.image_id === item.image_id))]
          : next.items);
        setSummaryCursor(next.next_cursor);
      })
      .catch((cause: Error) => { if (!controller.signal.aborted) setError(cause.message); });
    return controller;
  };

  useEffect(() => {
    const controller = readSummary();
    return () => controller?.abort();
    // readSummary intentionally follows the immutable service identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project, reload, service, task]);

  useEffect(() => {
    setReadable(false);
    if (!image?.src) return;
    let current = true;
    const probe = new Image();
    probe.onload = () => { if (current) setReadable(probe.naturalWidth > 0 && probe.naturalHeight > 0); };
    probe.src = image.src;
    return () => { current = false; };
  }, [image?.src]);

  useEffect(() => {
    const restore = () => {
      if (pending.current) return;
      const next = fromUrl(images, !!sampleResult);
      if (next.image !== selection.image || next.mode !== selection.mode) {
        setDraft(undefined); setSelected(undefined); setSelection(next);
      }
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, [images, sampleResult, selection]);

  useEffect(() => {
    setView(undefined); setError("");
    if (selection.mode !== "formal" || !image || formalResult === undefined) return;
    if (!formalResult) { setError("当前任务还没有绑定正式处理结果。不会改用项目最新 Run。"); return; }
    if (formalResult.project_id !== project || formalResult.task_id !== task) {
      setError("正式结果不属于当前 Project/Task，未读取其 child Run。");
      return;
    }
    if (!formalImage) { setError("这张图片不属于当前任务绑定的 Batch。"); return; }
    const controller = new AbortController();
    void service.image(project, task, image.id, formalRun, controller.signal)
      .then((next) => {
        if (controller.signal.aborted) return;
        if (next.snapshot.image_id !== image.id || next.snapshot.source_run_id !== formalRun) {
          throw new Error("服务端返回的图片或 child Run 与当前任务不匹配。");
        }
        setView(next);
        setSelected((current) => next.snapshot.annotations.some((item) => item.id === current) ? current : undefined);
      })
      .catch((cause: Error) => { if (!controller.signal.aborted) setError(cause.message); });
    return () => controller.abort();
  }, [formalImage, formalResult, formalRun, image, project, reload, selection.mode, service, task]);

  useEffect(() => {
    const guard = (event: Event) => {
      if ((dirty || busy) && !window.confirm("有未保存的对象编辑或正在保存。仍要离开？")) event.preventDefault();
    };
    const unload = (event: BeforeUnloadEvent) => {
      if (dirty || busy) { event.preventDefault(); event.returnValue = ""; }
    };
    window.addEventListener("ui-preview:before-navigate", guard);
    window.addEventListener("beforeunload", unload);
    return () => {
      window.removeEventListener("ui-preview:before-navigate", guard);
      window.removeEventListener("beforeunload", unload);
    };
  }, [busy, dirty]);

  const choose = (next: Selection) => {
    if (pending.current || (dirty && !window.confirm("放弃当前尚未保存的对象修改？"))) return;
    setDraft(undefined); setSelected(undefined); setSelection(next); setReason(""); setMessage("");
    const url = new URL(location.href);
    url.searchParams.set("delivery_view", next.mode);
    url.searchParams.set("delivery_image", next.image);
    url.searchParams.delete("delivery_run");
    history.pushState(history.state, "", url);
  };

  const emitSample = (annotation: Annotation) => {
    try {
      const next = sampleResult && sampleVisualSelection(sampleResult, selection.image, annotation);
      if (next) onVisualSelection?.(next);
      return next || undefined;
    } catch (cause) {
      setError((cause as Error).message);
      return undefined;
    }
  };

  const emitFormal = (annotation?: Annotation, imageDecision = false) => {
    try {
      if(imageDecision)return undefined;
      const next=annotation&&summaryItems.find(item=>item.image_id===selection.image)?.formal_selections[annotation.id];
      if(annotation&&!next)throw new Error("正式标注的当前会话引用仍未读取或已经失效，未附加到输入框。");
      if (next) onFormalSelection?.(next);
      return next || undefined;
    } catch (cause) {
      setError((cause as Error).message);
      return undefined;
    }
  };

  const selectObject = (id: string) => {
    if (id === selected || busy || (dirty && !window.confirm("放弃当前对象的未保存修改？"))) return;
    setDraft(undefined); setSelected(id);
    const annotation = activeAnnotations.find((item) => item.id === id);
    if (annotation) {
      if (selection.mode === "sample") emitSample(annotation);
      else emitFormal(annotation);
    }
  };

  const nextImage = () => {
    const index = images.findIndex((item) => item.id === selection.image);
    if (index < 0 || index + 1 >= images.length) return;
    const next = { ...selection, image: images[index + 1].id };
    setDraft(undefined); setSelected(undefined); setSelection(next); setReason("");
    const url = new URL(location.href);
    url.searchParams.set("delivery_view", next.mode);
    url.searchParams.set("delivery_image", next.image);
    url.searchParams.delete("delivery_run");
    history.pushState(history.state, "", url);
  };

  const confirm = async (decision: DeliveryReviewInput["decision"]) => {
    if (!view || !image || !formalResult || !formalImage || locked || dirty || pending.current) return;
    const scope = {
      intent_revision: view.intent_revision, intent_sha256: view.intent_sha256,
      image_id: image.id, source_run_id: formalRun,
      expected_snapshot_sha256: view.snapshot.sha256,
      expected_review_revision: view.review?.revision ?? 0,
      decision, reason: reason.trim() || null, confirmed: true,
    };
    const signature = JSON.stringify(scope);
    if (confirmRetry.current?.signature !== signature) {
      confirmRetry.current = { signature, input: { ...scope, command_id: crypto.randomUUID() } };
    }
    pending.current = true; setBusy(true); setError(""); setMessage("");
    let saved = false;
    try {
      await service.confirmImage(project, task, confirmRetry.current.input);
      confirmRetry.current = undefined; emitFormal(undefined, true);
      setMessage("整图决定已保存。对象审核、样例反馈与整图决定保持独立记录。");
      setReload((value) => value + 1); saved = true;
    } catch (cause) { setError((cause as Error).message); }
    finally {
      pending.current = false; setBusy(false);
      if (saved) nextImage();
    }
  };

  const saveObject = async (status: "needs_review" | "human_accepted" | "rejected") => {
    if (selection.mode !== "formal" || creating || !object || !view || !image || !formalRun || pending.current || locked || !readable) return;
    const body = {
      intent_revision: view.intent_revision, intent_sha256: view.intent_sha256,
      source_run_id: formalRun, annotation_id: object.id,
      expected_snapshot_sha256: view.snapshot.sha256, label: object.label || "",
      value: object.value, review_status: status,
      reason: reason.trim() || `Human object ${status} in delivery review`,
    };
    const signature = JSON.stringify(body);
    if (editRetry.current?.signature !== signature) editRetry.current = { signature, input: { ...body, command_id: crypto.randomUUID() } };
    pending.current = true; setBusy(true); setError("");
    try {
      await service.editObject(project, task, image.id, editRetry.current.input);
      emitFormal(object); editRetry.current = undefined; setDraft(undefined);
      setMessage("对象修改已保存；这不等于整张图已经检查完整。");
      setReload((value) => value + 1);
    } catch (cause) { setError((cause as Error).message); }
    finally { pending.current = false; setBusy(false); }
  };

  const addObject = () => {
    if (selection.mode !== "formal" || dirty || busy || locked || !view || !formalRun || !labels.length) return;
    const id = crypto.randomUUID();
    setSelected(id);
    setDraft({
      id, image_id: selection.image, task_id: task, label: labels[0].stable_id,
      value: { kind: "bounding_box", rect: [0.4, 0.4, 0.15, 0.15] },
      attributes: {}, source: "human", review_status: "needs_review",
      provenance: {}, created_at: new Date().toISOString(),
    });
    setReason("人工检查原图后补充漏标目标");
  };

  const saveNewObject = async () => {
    if (!creating || !object || !view || !image || !formalRun || pending.current || locked || !readable || !service.createObject || !reason.trim()) return;
    const body = {
      intent_revision: view.intent_revision, intent_sha256: view.intent_sha256,
      source_run_id: formalRun, expected_snapshot_sha256: view.snapshot.sha256,
      label: object.label || "", value: object.value, reason: reason.trim(),
    };
    const signature = JSON.stringify(body);
    if (createRetry.current?.signature !== signature) createRetry.current = { signature, input: { ...body, command_id: crypto.randomUUID() } };
    pending.current = true; setBusy(true); setError("");
    try {
      await service.createObject(project, task, image.id, createRetry.current.input);
      createRetry.current = undefined; setDraft(undefined); setSelected(undefined);
      setMessage("新增目标已保存为待审核；接受对象后仍需确认整图。");
      setReload((value) => value + 1);
    } catch (cause) { setError((cause as Error).message); }
    finally { pending.current = false; setBusy(false); }
  };

  const positive = !!formalRun && !!view && view.accepted_objects > 0 && view.unresolved_objects === 0;
  const negative = !!view && view.accepted_objects === 0 && view.unresolved_objects === 0;

  return <section className="delivery-review" aria-label="当前任务图片结果">
    <div className="delivery-review-heading">
      <div><h3>检查当前任务结果</h3><p>样例反馈只修改 Sandbox；正式审核只保存到绑定 Batch 的 child Run。</p></div>
      <div className="delivery-review-tabs" role="tablist" aria-label="结果类型">
        {sampleResult && <button type="button" role="tab" aria-selected={selection.mode === "sample"} onClick={() => choose({ ...selection, mode: "sample" })}>样例结果</button>}
        <button type="button" role="tab" aria-selected={selection.mode === "formal"} onClick={() => choose({ ...selection, mode: "formal" })}>正式 Batch</button>
      </div>
    </div>
    <div className="delivery-review-controls">
      <label>图片<select aria-label="图片" value={selection.image} disabled={busy} onChange={(event) => choose({ ...selection, image: event.target.value })}>
        {images.map((item, index) => <option key={item.id} value={item.id}>{index + 1}/{images.length} · {item.name}</option>)}
      </select></label>
      {selection.mode === "formal" && formalResult && <p className="delivery-source-receipt">
        Batch {formalResult.batch_id.slice(0, 8)} · Workflow {formalResult.workflow_version} · child Run {formalRun?.slice(0, 8) ?? "无目标结果"}
      </p>}
    </div>
    {summary && selection.mode === "formal" && <div className="delivery-review-summary" aria-label="审核摘要">
      <strong>{summary.counts.complete}/{summary.counts.total} 张已完成</strong>
      <span>待处理 {summary.counts.unresolved}</span><span>失败 {summary.counts.failed}</span>
      {!!summaryItems.length && <div className="delivery-review-summary-items">
        {summaryItems.map((item) => <button type="button" key={item.image_id} onClick={() => choose({ mode: "formal", image: item.image_id })}>{item.image_id.slice(0, 8)} · {item.state}</button>)}
        {summaryCursor && <button type="button" onClick={() => readSummary(summaryCursor)}>加载下一页</button>}
      </div>}
    </div>}
    {error && <p role="alert" className="error">{error}</p>}
    {message && <p role="status">{message}</p>}
    {selection.mode === "formal" && formalResult === undefined && !error && <p role="status">读取当前任务绑定的正式结果…</p>}
    {selection.mode === "formal" && formalResult && formalImage && !view && !error && <p role="status">读取 child Run 的正式标注…</p>}
    {selection.mode === "sample" && !sampleImage && <p role="status">这张图片没有当前 Sample Test 结果。</p>}
    {image && <AnnotationCanvas
      imageUrl={image.src}
      labelNames={Object.fromEntries(labels.map((item) => [item.stable_id, item.display_name]))}
      annotations={activeAnnotations}
      selectedId={selected}
      onSelect={selectObject}
      onChange={(next) => {
        if (selection.mode === "formal" && !busy && !locked) { setSelected(next.id); setDraft(next); }
      }}
      readOnly={selection.mode === "sample" || busy || locked || !formalResult || !formalImage || !service.editObject}
      compactList
    />}
    {image&&Object.keys(annotationOrigins[image.id]||{}).length>0&&<div className="delivery-source-receipt" aria-label="当前图片候选来源">
      {Object.entries(annotationOrigins[image.id]).map(([annotationId,origin])=><span key={annotationId}>{annotationId===selected?"当前对象 · ":""}{demoOriginLabel(origin)}</span>)}
    </div>}
    {selection.mode === "sample" && sampleResult && <div className="actions">
      <button type="button" disabled={!selected} onClick={() => {
        const annotation = sampleImage?.annotations.find((item) => item.id === selected);
        const next = annotation && emitSample(annotation);
        if (next) onSampleIssue?.(next);
      }}>这个样例框有问题</button>
      <p>仅创建带 Draft、Sample Test、Artifact 和 feedback revision 的反馈引用，不写正式标注。</p>
    </div>}
    {selection.mode === "formal" && view && formalResult && formalImage && <>
      {service.createObject && <button type="button" disabled={busy || locked || dirty || !formalRun || !readable || !labels.length} onClick={addObject}>新增漏标目标框</button>}
      {creating && <div className="actions">
        <button type="button" disabled={busy} onClick={() => { setDraft(undefined); setSelected(undefined); }}>取消新增框</button>
        <button type="button" disabled={busy || locked || !readable || !reason.trim()} onClick={() => void saveNewObject()}>保存新增目标框</button>
      </div>}
      {object && formalRun && <div className="delivery-object-editor">
        <p>选中对象 · {object.review_status} {dirty ? "· 尚未保存" : ""}</p>
        <label>对象类别<select aria-label="对象类别" value={object.label || ""} disabled={busy || locked} onChange={(event) => setDraft({ ...object, label: event.target.value })}>
          {labels.length ? labels.map((item) => <option key={item.stable_id} value={item.stable_id}>{item.display_name}</option>) : <option value={object.label || ""}>{object.label}</option>}
        </select></label>
        {!creating && <div className="actions">
          <button type="button" disabled={busy || !dirty} onClick={() => setDraft(undefined)}>撤销对象修改</button>
          <button type="button" disabled={busy || locked || !dirty} onClick={() => void saveObject("needs_review")}>保存对象修改</button>
          <button type="button" disabled={busy || locked || !readable || (object.review_status === "human_accepted" && !dirty)} onClick={() => void saveObject("human_accepted")}>接受这个对象</button>
          <button type="button" disabled={busy || locked || !readable} onClick={() => void saveObject("rejected")}>拒绝这个对象</button>
        </div>}
      </div>}
      {!readable && <p role="status">原图尚未成功加载，不能确认整图完整或无目标；仍可填写原因明确排除。</p>}
      <p>{view.confirmation_current ? `此快照已有整图决定：${view.review?.input.decision}` : "当前图片尚未整图确认"} · 已接受对象 {view.accepted_objects} · 未解决对象 {view.unresolved_objects}</p>
      <label>检查备注／排除原因<textarea value={reason} disabled={busy} onChange={(event) => setReason(event.target.value)} placeholder="排除图片必须说明原因" /></label>
      <div className="actions">
        <button type="button" disabled={busy || locked || dirty || !positive || !readable} onClick={() => void confirm("positive_complete")}>确认整张图标注完整并继续</button>
        <button type="button" disabled={busy || locked || dirty || !negative || !readable} onClick={() => void confirm("negative_confirmed")}>确认整张图没有目标并继续</button>
        <button type="button" disabled={busy || locked || dirty || !reason.trim()} onClick={() => void confirm("excluded")}>明确排除此图并继续</button>
        <button type="button" disabled={busy || dirty} onClick={() => setReload((value) => value + 1)}>重新读取服务器状态</button>
      </div>
    </>}
  </section>;
}
