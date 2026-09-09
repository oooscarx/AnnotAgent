import { useEffect, useState, useRef } from "react";
import { Icon, IconButton } from "./Icon";
import { Disclosure } from "./Disclosure";
import { ImageBrowser } from "./ImageBrowser";
import { labelColor } from "../annotationVisuals";
import type { WorkspaceAdapter, Task, Box, Snapshot, ImageId } from "./adapter";
import { command } from "./App";
export function ArtifactPane({
  task,
  adapter,
  close,
  onError,
  image,
  onImage,
  onReference,
  assets,
}: {
  task: Task;
  adapter: WorkspaceAdapter;
  close: () => void;
  onError: (s: string) => void;
  image: ImageId;
  onImage: (image: ImageId) => void;
  onReference: (candidate: string, image: ImageId) => void;
  assets: Snapshot["artifacts"];
}) {
  const asset = assets.find((a) => a.id === image);
  const fixture = adapter.kind === "fixture";
  const editable = fixture || (task.actions?.answer?.available === true && task.human?.image === image);
  const [natural, setNatural] = useState({width: 960, height: 760});
  useEffect(() => { if (!asset) return; const img = new Image(); let current = true; img.onload = () => { if (current) setNatural({width: img.naturalWidth, height: img.naturalHeight}); }; img.src = asset.src; return () => { current = false; }; }, [asset?.src]);
  const width = asset?.width || natural.width,
    height = asset?.height || natural.height;
  const savedBoxes = task.boxesByImage?.[image] || (image === 1 ? task.boxes : []);
  const initialImage = image;
  const [boxes, setBoxes] = useState(
    task.editBoxes?.[initialImage] || savedBoxes,
  );
  const [selected, setSelected] = useState(savedBoxes[0]?.id);
  const [classification,setClassification]=useState(task.human?.label || "");
  const [compare, setCompare] = useState(false);
  const [original, setOriginal] = useState(false);
  const [zoom, setZoom] = useState(100);
  const svgRef = useRef<SVGSVGElement>(null);
  const [screenScale, setScreenScale] = useState(1);
  useEffect(()=>{
    const svg=svgRef.current;if(!svg)return;
    const measure=()=>setScreenScale(svg.getScreenCTM()?.a || 1);
    const observer=new ResizeObserver(measure);observer.observe(svg);measure();
    return()=>observer.disconnect();
  },[width,height,zoom]);
  const [history, setHistory] = useState<Box[][]>([]);
  const [saving, setSaving] = useState(false);
  const [drag, setDrag] = useState<{
    id: string;
    x: number;
    y: number;
    box: Box;
    resize: boolean;
  } | null>(null);
  const dirty = JSON.stringify(boxes) !== JSON.stringify(savedBoxes) || classification !== (task.human?.label || "");
  useEffect(() => {
    // Box drafts are already persisted. Classification edits need an explicit guard.
    if(fixture || classification === (task.human?.label || "")) return;
    const leave=(event:Event)=>{if(!window.confirm("当前类别修改尚未保存，放弃修改并离开？"))event.preventDefault();};
    const unload=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    window.addEventListener("ui-preview:before-navigate",leave);
    window.addEventListener("beforeunload",unload);
    return()=>{window.removeEventListener("ui-preview:before-navigate",leave);window.removeEventListener("beforeunload",unload);};
  },[fixture,classification,task.human?.label]);
  useEffect(() => {
    adapter.saveArtifactDraft(task.id, image, boxes);
  }, [boxes, image, task.id, adapter]);
  const update = (id: string, changes: Partial<Box>) => {
    setHistory((h) => [...h, boxes]);
    setBoxes((bs) =>
      bs.map((b) => {
        if (b.id !== id) return b;
        const next = { ...b, ...changes };
        next.x = Math.max(0, Math.min(width - 8, next.x));
        next.y = Math.max(0, Math.min(height - 8, next.y));
        next.w = Math.max(8, Math.min(width - next.x, next.w));
        next.h = Math.max(8, Math.min(height - next.y, next.h));
        return next;
      }),
    );
  };
  const pickImage = (n: ImageId) => {
    onImage(n);
  };
  return (
    <aside className="artifact-pane" aria-label="图片与标注">
      <div className="artifact-toolbar">
        <strong>
          {fixture ? "示意图片" : "图片"} · {assets.findIndex(a => a.id === image) + 1}/{assets.length}
        </strong>
        <button aria-pressed={original} onClick={() => setOriginal(!original)}>
          原图
        </button>
        <button aria-pressed={compare} onClick={() => setCompare(!compare)}>
          对比
        </button>
        <IconButton icon="close" label="关闭图片" onClick={close} />
      </div>
      <div className="artifact-toolbar">
        <small>{asset?.name || "图片不存在"}</small>
        <button
          onClick={() => {
            if (history.length) {
              setBoxes(history.at(-1)!);
              setHistory((h) => h.slice(0, -1));
            }
          }}
          disabled={!history.length}
        >
          <Icon name="undo" size={16} />撤销
        </button>
        <button onClick={() => setZoom(100)} aria-label="适应画布"><Icon name="fit" size={16} />Fit</button>
        <label>
          Zoom{" "}
          <input
            aria-label="图片缩放"
            type="range"
            min="60"
            max="200"
            value={zoom}
            onChange={(e) => setZoom(Number(e.target.value))}
          />
          {zoom}%
        </label>
      </div>
      <div className="canvas-region">
        <svg
          ref={svgRef}
          viewBox={`0 0 ${width} ${height}`}
          aria-label={fixture ? "演示标注画布" : "标注画布"}
          style={{ width: `${zoom}%`, minWidth: `${zoom}%` }}
          onPointerMove={(e) => {
            if (!drag) return;
            const transform = e.currentTarget.getScreenCTM();
            if (!transform) return;
            const dx = (e.clientX - drag.x) / transform.a,
              dy = (e.clientY - drag.y) / transform.d;
            setBoxes((bs) =>
              bs.map((b) =>
                b.id !== drag.id
                  ? b
                  : drag.resize
                    ? {
                        ...b,
                        w: Math.max(8, Math.min(width - b.x, drag.box.w + dx)),
                        h: Math.max(8, Math.min(height - b.y, drag.box.h + dy)),
                      }
                    : {
                        ...b,
                        x: Math.max(0, Math.min(width - b.w, drag.box.x + dx)),
                        y: Math.max(0, Math.min(height - b.h, drag.box.y + dy)),
                      },
              ),
            );
          }}
          onPointerUp={() => setDrag(null)}
        >
          <image href={asset?.src} width={width} height={height} />
          {!original &&
            compare &&
            savedBoxes.map((b) => (
              <rect
                key={b.id}
                x={b.x}
                y={b.y}
                width={b.w}
                height={b.h}
                fill="none"
                stroke={labelColor(b.label)}
                strokeWidth="1"
                strokeDasharray="5 4"
                vectorEffect="non-scaling-stroke"
              />
            ))}
          {!original &&
            boxes.map((b) => (
              <g
                key={b.id}
                onPointerDown={(e) => {
                  setSelected(b.id);
                  if(!editable)return;
                  e.currentTarget.ownerSVGElement?.setPointerCapture(
                    e.pointerId,
                  );
                  setSelected(b.id);
                  onReference(b.id, image);
                  setHistory((h) => [...h, boxes]);
                  setDrag({
                    id: b.id,
                    x: e.clientX,
                    y: e.clientY,
                    box: b,
                    resize: (e.target as SVGElement).tagName === "circle",
                  });
                }}
              >
                <rect
                  x={b.x}
                  y={b.y}
                  width={b.w}
                  height={b.h}
                  fill="transparent"
                  stroke={labelColor(b.label)}
                  strokeWidth="1.25"
                  vectorEffect="non-scaling-stroke"
                />
                <g className="bbox-label" pointerEvents="none" transform={`translate(${Math.max(2/screenScale,Math.min(b.x,width-Math.min(180,width*screenScale-4)/screenScale))} ${Math.max(16/screenScale,b.y-5/screenScale)}) scale(${1/screenScale})`}>
                  <text fontSize="12" paintOrder="stroke" stroke="var(--aa-surface)" strokeWidth="1.5" strokeLinejoin="round" fill={labelColor(b.label)}><title>{b.label}</title>{Array.from(b.label).slice(0,20).join("")}{b.label.length>20?"…":""}{fixture ? " · 演示" : ""}</text>
                </g>
                {selected === b.id && (
                  <circle
                    cx={b.x + b.w}
                    cy={b.y + b.h}
                    r={3 / screenScale}
                    fill="var(--aa-surface)"
                    stroke={labelColor(b.label)}
                    vectorEffect="non-scaling-stroke"
                  />
                )}
              </g>
            ))}
        </svg>
      </div>
      <p className="canvas-warning">
        {fixture ? "示意图与手工框 · 非模型推理。杯柄边界待确认，语义得分不代表几何质量。" : task.imageResults?.[image]?.risks.join("；") || "样例评估结果 · 不是已接受的正式标注。语义分数不等于几何质量。"}
      </p>
      {!fixture && task.imageResults?.[image]?.labels.map((label,i)=><p className="artifact-toolbar" key={i}>分类结果：{label}</p>)}
      {!fixture && task.human?.kind==="classification" && task.human.image===image && <label className="artifact-toolbar">确认类别<select aria-label="确认类别" value={classification} onChange={e=>setClassification(e.target.value)}>{task.human.labels.map(label=><option key={label}>{label}</option>)}</select></label>}
      <ImageBrowser assets={assets} image={image} onSelect={pickImage}/>
      <Disclosure className="annotation-list" title={`标注列表与精确编辑 · ${boxes.length} 个`}>
        {boxes.map((b) => (
          <div key={b.id}>
            <button
              onClick={() => {
                setSelected(b.id);
                onReference(b.id, image);
              }}
              aria-pressed={selected === b.id}
            >
              {b.label}
            </button>
            <label>
              标签
              <input
                value={b.label}
                disabled={!editable}
                onChange={(e) => update(b.id, { label: e.target.value })}
              />
            </label>
            {(["x", "y", "w", "h"] as const).map((k) => (
              <label key={k}>
                {k}
                <input
                  aria-label={`${b.id} ${k}`}
                  type="number"
                  disabled={!editable}
                  value={Math.round(b[k])}
                  onChange={(e) =>
                    update(b.id, { [k]: Math.max(0, Number(e.target.value)) })
                  }
                />
              </label>
            ))}
          </div>
        ))}
        <button
          disabled={!fixture}
          onClick={() => {
            setHistory((h) => [...h, boxes]);
            setBoxes((bs) => [
              ...bs,
              {
                id: crypto.randomUUID(),
                label: "cup",
                x: 300,
                y: 300,
                w: 100,
                h: 100,
              },
            ]);
          }}
        >
          <Icon name="plus" size={16} />添加遗漏目标
        </button>
      </Disclosure>
      <div className="artifact-footer">
        <small>
          {!fixture && !editable ? "只读样例；当前没有待回答的人工问题" : dirty ? "浏览器编辑草稿 · 尚未提交" : fixture ? "演示候选 · 非正式标注" : "样例终端候选 · 非正式标注"}
        </small>
        <button
          className="primary"
          disabled={saving || (fixture ? image !== 1 || task.phase !== "waiting_for_human" : !task.actions?.answer?.available || task.human?.image!==image)}
          onClick={async () => {
            setSaving(true);
            try {
              await adapter.answerHumanRequest(command(task), boxes, classification);
              setHistory([]);
            } catch (e) {
              onError((e as Error).message);
            } finally {
              setSaving(false);
            }
          }}
        >
          {saving ? "保存中…" : fixture ? "提交修正并继续" : "保存当前样例修正"}
        </button>
      </div>
    </aside>
  );
}
