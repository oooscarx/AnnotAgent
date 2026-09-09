import { useEffect, useState } from "react";
import type { WorkspaceAdapter, Task, Box } from "./adapter";
import { command } from "./App";
export function ArtifactPane({
  task,
  adapter,
  close,
  onError,
  image,
  onImage,
  onReference,
}: {
  task: Task;
  adapter: WorkspaceAdapter;
  close: () => void;
  onError: (s: string) => void;
  image: number;
  onImage: (image: number) => void;
  onReference: (candidate: string, image: number) => void;
}) {
  const initialImage = image;
  const [boxes, setBoxes] = useState(
    task.editBoxes?.[initialImage] || (initialImage === 1 ? task.boxes : []),
  );
  const [selected, setSelected] = useState(task.boxes[0]?.id);
  const [compare, setCompare] = useState(false);
  const [original, setOriginal] = useState(false);
  const [zoom, setZoom] = useState(100);
  const [history, setHistory] = useState<Box[][]>([]);
  const [saving, setSaving] = useState(false);
  const [drag, setDrag] = useState<{
    id: string;
    x: number;
    y: number;
    box: Box;
    resize: boolean;
  } | null>(null);
  const dirty = JSON.stringify(boxes) !== JSON.stringify(task.boxes);
  useEffect(() => {
    adapter.saveArtifactDraft(task.id, image, boxes);
  }, [boxes, image, task.id, adapter]);
  const update = (id: string, changes: Partial<Box>) => {
    setHistory((h) => [...h, boxes]);
    setBoxes((bs) =>
      bs.map((b) => {
        if (b.id !== id) return b;
        const next = { ...b, ...changes };
        next.x = Math.max(0, Math.min(952, next.x));
        next.y = Math.max(0, Math.min(752, next.y));
        next.w = Math.max(8, Math.min(960 - next.x, next.w));
        next.h = Math.max(8, Math.min(760 - next.y, next.h));
        return next;
      }),
    );
  };
  const pickImage = (n: number) => {
    onImage(n);
  };
  return (
    <aside className="artifact-pane" aria-label="图片与标注">
      <div className="artifact-toolbar">
        <strong>示意图片 · {image}/3</strong>
        <button aria-pressed={original} onClick={() => setOriginal(!original)}>
          原图
        </button>
        <button aria-pressed={compare} onClick={() => setCompare(!compare)}>
          对比
        </button>
        <button
          aria-label="关闭图片"
          onClick={() => {
            close();
          }}
        >
          ×
        </button>
      </div>
      <div className="artifact-toolbar">
        <small>still-life-{image}.png</small>
        <button
          onClick={() => {
            if (history.length) {
              setBoxes(history.at(-1)!);
              setHistory((h) => h.slice(0, -1));
            }
          }}
          disabled={!history.length}
        >
          撤销
        </button>
        <button onClick={() => setZoom(100)}>Fit</button>
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
          viewBox="0 0 960 760"
          aria-label="演示标注画布"
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
                        w: Math.max(8, Math.min(960 - b.x, drag.box.w + dx)),
                        h: Math.max(8, Math.min(760 - b.y, drag.box.h + dy)),
                      }
                    : {
                        ...b,
                        x: Math.max(0, Math.min(960 - b.w, drag.box.x + dx)),
                        y: Math.max(0, Math.min(760 - b.h, drag.box.y + dy)),
                      },
              ),
            );
          }}
          onPointerUp={() => setDrag(null)}
        >
          <image
            href={`/assets/still-life-${image}.png`}
            width="960"
            height="760"
          />
          {!original &&
            compare &&
            (image === 1 ? task.boxes : []).map((b) => (
              <rect
                key={b.id}
                x={b.x}
                y={b.y}
                width={b.w}
                height={b.h}
                fill="none"
                stroke="var(--aa-annotation-4)"
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
                  stroke={
                    b.label === "bottle"
                      ? "var(--aa-annotation-4)"
                      : "var(--aa-annotation-1)"
                  }
                  strokeWidth="1.25"
                  vectorEffect="non-scaling-stroke"
                />
                <text
                  x={b.x + 3}
                  y={b.y - 9}
                  fontSize="20"
                  fill="var(--aa-annotation-1)"
                >
                  {b.label} · 演示
                </text>
                {selected === b.id && (
                  <circle
                    cx={b.x + b.w}
                    cy={b.y + b.h}
                    r="4"
                    fill="var(--aa-surface)"
                    stroke="var(--aa-annotation-1)"
                    vectorEffect="non-scaling-stroke"
                  />
                )}
              </g>
            ))}
        </svg>
      </div>
      <p className="canvas-warning">
        示意图与手工框 · 非模型推理。杯柄边界待确认，语义得分不代表几何质量。
      </p>
      <div className="thumbnails">
        {[1, 2, 3].map((n) => (
          <button
            key={n}
            aria-label={`查看图片 ${n}`}
            aria-pressed={n === image}
            onClick={() => pickImage(n)}
          >
            <img src={`/assets/still-life-${n}.png`} alt="" />
          </button>
        ))}
      </div>
      <details className="annotation-list">
        <summary>标注列表与精确编辑 · {boxes.length} 个</summary>
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
                onChange={(e) => update(b.id, { label: e.target.value })}
              />
            </label>
            {(["x", "y", "w", "h"] as const).map((k) => (
              <label key={k}>
                {k}
                <input
                  aria-label={`${b.id} ${k}`}
                  type="number"
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
          ＋ 添加遗漏目标
        </button>
      </details>
      <div className="artifact-footer">
        <small>
          {dirty ? "浏览器编辑草稿已保存 · 尚未提交" : "演示候选 · 非正式标注"}
        </small>
        <button
          className="primary"
          disabled={saving || image !== 1 || task.phase !== "waiting_for_human"}
          onClick={async () => {
            setSaving(true);
            try {
              await adapter.answerHumanRequest(command(task), boxes);
              setHistory([]);
            } catch (e) {
              onError((e as Error).message);
            } finally {
              setSaving(false);
            }
          }}
        >
          {saving ? "保存中…" : "提交修正并继续"}
        </button>
      </div>
    </aside>
  );
}
