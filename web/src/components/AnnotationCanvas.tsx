import { t } from "../i18n";
import { workspaceShortcutAllowed } from "../workspaceKeyboard";
import { useEffect, useMemo, useRef, useState } from "react";
import { annotationColor, annotationVisual } from "../annotationVisuals";
import type { AnnotationVisualContext } from "../annotationVisuals";
import { zoomAroundPoint } from "../canvasViewport";
import { keyboardBox, keyboardPoint } from "../bboxKeyboard";
import type { Annotation, Point } from "../types";

interface Props {
  imageUrl?: string;
  annotations: Annotation[];
  selectedId?: string;
  visualContext?: AnnotationVisualContext;
  onSelect: (id: string) => void;
  onChange: (annotation: Annotation) => void;
  onEditStart?: () => void;
  readOnly?: boolean;
  compactList?: boolean;
}

const DEFAULT_WIDTH = 1000;
const DEFAULT_HEIGHT = 650;
const pointText = (points: Point[], width: number, height: number) =>
  points.map(([x, y]) => `${x * width},${y * height}`).join(" ");

export function AnnotationCanvas({
  imageUrl,
  annotations,
  selectedId,
  visualContext,
  onSelect,
  onChange,
  onEditStart,
  readOnly = false,
  compactList = false,
}: Props) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [listOpen, setListOpen] = useState(!compactList);
  const [measuredImageUrl, setMeasuredImageUrl] = useState<string>();
  const [canvasSize, setCanvasSize] = useState<[number, number]>([
    DEFAULT_WIDTH,
    DEFAULT_HEIGHT,
  ]);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<Point>([0, 0]);
  const [drag, setDrag] = useState<
    | { type: "vertex"; id: string; ring: number; index: number }
    | { type: "bbox"; id: string; start: Point; original: [number, number, number, number] }
    | {
        type: "bbox_resize";
        id: string;
        corner: "nw" | "ne" | "sw" | "se";
        original: [number, number, number, number];
      }
    | { type: "pan"; start: Point; original: Point }
  >();

  const selected = useMemo(
    () => annotations.find((annotation) => annotation.id === selectedId),
    [annotations, selectedId],
  );
  const editingHint = readOnly ? "Read-only geometry · you can still zoom and select results" : !selected
    ? "Select an annotation to edit it"
    : selected.value.kind === "classification"
      ? "Image-level classification · use the label control to edit"
    : selected.value.kind === "bounding_box"
      ? "Drag the box to move it · drag a corner handle to resize"
      : selected.value.kind === "keypoints"
        ? "Drag a keypoint to move it · Delete removes the selected point"
        : "Drag vertices to move them · double-click to add · Delete removes the selected vertex";

  useEffect(() => {
    setCanvasSize([DEFAULT_WIDTH, DEFAULT_HEIGHT]);
  }, [imageUrl]);

  const [width, height] = canvasSize;

  const canvasPoint = (clientX: number, clientY: number): Point => {
    const matrix = svgRef.current?.getScreenCTM();
    if (!matrix) return [width / 2, height / 2];
    const point = new DOMPoint(clientX, clientY).matrixTransform(matrix.inverse());
    return [point.x, point.y];
  };

  const applyZoom = (requestedZoom: number, anchor: Point = [width / 2, height / 2]) => {
    const next = zoomAroundPoint(zoom, pan, anchor, requestedZoom);
    setPan(next.pan);
    setZoom(next.zoom);
  };

  const localPoint = (event: React.PointerEvent<Element>): Point => {
    const point = canvasPoint(event.clientX, event.clientY);
    return [
      Math.max(0, Math.min(1, (point[0] - pan[0]) / zoom / width)),
      Math.max(0, Math.min(1, (point[1] - pan[1]) / zoom / height)),
    ];
  };

  const moveVertex = (annotation: Annotation, ring: number, index: number, point: Point) => {
    if (readOnly) return;
    const value = structuredClone(annotation.value);
    if (value.kind === "polyline") value.points[index] = point;
    if (value.kind === "polygon") value.rings[ring][index] = point;
    if (value.kind === "keypoints") value.points[index].point = point;
    if ((value.kind === "instance_mask" || value.kind === "semantic_mask") && value.mask.encoding === "polygon") {
      value.mask.rings[ring][index] = point;
    }
    onChange({ ...annotation, value });
  };

  const onPointerMove = (event: React.PointerEvent<SVGSVGElement>) => {
    if (!drag) return;
    // A save can make the canvas read-only between pointerdown and pointerup.
    // Selection/zoom remain available, but an old drag cannot mutate its draft.
    if (readOnly && drag.type !== "pan") return;
    const point = localPoint(event);
    if (drag.type === "vertex") {
      const annotation = annotations.find((item) => item.id === drag.id);
      if (annotation) moveVertex(annotation, drag.ring, drag.index, point);
    } else if (drag.type === "bbox") {
      const annotation = annotations.find((item) => item.id === drag.id);
      if (annotation && annotation.value.kind === "bounding_box") {
        const dx = point[0] - drag.start[0];
        const dy = point[1] - drag.start[1];
        const [, , width, height] = drag.original;
        const rect: [number, number, number, number] = [
          Math.max(0, Math.min(1 - width, drag.original[0] + dx)),
          Math.max(0, Math.min(1 - height, drag.original[1] + dy)),
          width,
          height,
        ];
        onChange({ ...annotation, value: { kind: "bounding_box", rect } });
      }
    } else if (drag.type === "bbox_resize") {
      const annotation = annotations.find((item) => item.id === drag.id);
      if (annotation && annotation.value.kind === "bounding_box") {
        const [x, y, originalWidth, originalHeight] = drag.original;
        const opposite: Point = [
          drag.corner.includes("w") ? x + originalWidth : x,
          drag.corner.includes("n") ? y + originalHeight : y,
        ];
        const width = Math.max(0.002, Math.abs(point[0] - opposite[0]));
        const height = Math.max(0.002, Math.abs(point[1] - opposite[1]));
        const left = Math.min(Math.min(point[0], opposite[0]), 1 - width);
        const top = Math.min(Math.min(point[1], opposite[1]), 1 - height);
        const rect: [number, number, number, number] = [
          left,
          top,
          width,
          height,
        ];
        onChange({ ...annotation, value: { kind: "bounding_box", rect } });
      }
    } else {
      setPan([
        drag.original[0] + (point[0] - drag.start[0]) * width,
        drag.original[1] + (point[1] - drag.start[1]) * height,
      ]);
    }
  };

  const addVertex = (event: React.MouseEvent<SVGSVGElement>) => {
    if (readOnly || !selected || event.detail !== 2) return;
    onEditStart?.();
    const point = localPoint(event as unknown as React.PointerEvent<SVGSVGElement>);
    const value = structuredClone(selected.value);
    if (value.kind === "polyline") value.points.push(point);
    if (value.kind === "polygon") value.rings[0]?.push(point);
    if ((value.kind === "instance_mask" || value.kind === "semantic_mask") && value.mask.encoding === "polygon") {
      value.mask.rings[0]?.push(point);
    }
    onChange({ ...selected, value });
  };

  const deleteVertex = (annotation: Annotation, ring: number, index: number) => {
    if (readOnly) return;
    const value = structuredClone(annotation.value);
    if (value.kind === "polyline" && value.points.length > 2) value.points.splice(index, 1);
    else if (value.kind === "polygon" && value.rings[ring]?.length > 3)
      value.rings[ring].splice(index, 1);
    else if (value.kind === "keypoints" && value.points.length > 1)
      value.points.splice(index, 1);
    else if (
      (value.kind === "instance_mask" || value.kind === "semantic_mask") &&
      value.mask.encoding === "polygon" &&
      value.mask.rings[ring]?.length > 3
    )
      value.mask.rings[ring].splice(index, 1);
    else return;
    onEditStart?.();
    onChange({ ...annotation, value });
  };

  return (
    <div className="canvas-shell">
      <div className="canvas-tools">
        <span>{t("Canvas")}</span>
        <div className="canvas-zoom-controls" role="group" aria-label={t("Canvas zoom")}>
          <button aria-label={t("Zoom out")} title={t("Zoom out")} onClick={() => applyZoom(zoom - 0.1)}>−</button>
          <strong aria-live="polite">{Math.round(zoom * 100)}%</strong>
          <button aria-label={t("Zoom in")} title={t("Zoom in")} onClick={() => applyZoom(zoom + 0.1)}>+</button>
        </div>
        <button className="canvas-fit-button" aria-label={t("Fit image")} title={t("Fit image")} onClick={() => { setZoom(1); setPan([0, 0]); }}>{t("Fit")}</button>
        {compactList && <button aria-expanded={listOpen} onClick={() => setListOpen(!listOpen)}>{t("Annotation list")} · {annotations.length}</button>}
      </div>
      {!readOnly && imageUrl && measuredImageUrl === imageUrl && selected?.value.kind === "bounding_box" && <div className="canvas-keyboard-tools" role="group" aria-label={t("Keyboard box editing")}>
        {[false, true].map((resize) => <button key={String(resize)} onKeyDown={(event) => {
          // These explicit local controls also work in the sample dialog. Do not
          // bubble arrow keys into Review queue navigation or consume IME input.
          if (!workspaceShortcutAllowed(event.nativeEvent, false, false) || event.metaKey || event.ctrlKey || event.altKey || selected.value.kind !== "bounding_box") return;
          const value = keyboardBox(selected.value.rect, event.key, resize, event.shiftKey ? 10 : 1, width, height);
          if (!value) return;
          event.preventDefault(); event.stopPropagation();
          if (JSON.stringify(value) === JSON.stringify(selected.value)) return;
          onEditStart?.(); onChange({ ...selected, value });
        }}>{t(resize ? "Resize box with arrow keys" : "Move box with arrow keys")}</button>)}
        <small>{t("Focus a control, then use arrow keys: 1 image pixel, or Shift for 10. Resize keeps the top-left corner fixed.")}</small>
      </div>}
      {annotations.some((annotation) => annotation.value.kind === "classification") && <div className="canvas-classification-results" aria-label={t("Image classification results")}>{annotations.filter((annotation) => annotation.value.kind === "classification").map((annotation) => <span key={annotation.id}>{annotation.label ?? annotation.task_id}</span>)}</div>}
      {listOpen && <ul className="canvas-annotation-list" aria-label={t("Annotations on canvas")}>
        {annotations.map((annotation) => {
          const visual = annotationVisual(annotation, visualContext);
          return <li key={annotation.id}><button aria-pressed={annotation.id === selectedId} onClick={() => onSelect(annotation.id)}><i aria-hidden="true" style={{ borderColor: annotationColor(visual.slot) }} /><span><strong>{annotation.label ?? annotation.task_id}</strong><small>{annotation.value.kind.replaceAll("_", " ")} · {annotation.confidence == null ? t("Score not provided") : `${Math.round(annotation.confidence * 100)}%`}</small>{typeof annotation.provenance.addition_id === "string" ? <small>{t("Human sample example")}</small> : annotation.provenance.human_corrected === true && <small>{t("Human sample correction")}</small>}</span></button></li>;
        })}
        {annotations.length === 0 && <li className="canvas-annotation-empty">{t("No annotations selected")}</li>}
      </ul>}
      {imageUrl && (
        <img
          className="canvas-dimension-probe"
          src={imageUrl}
          alt=""
          aria-hidden="true"
          onLoad={(event) => {
            const { naturalWidth, naturalHeight } = event.currentTarget;
            if (naturalWidth > 0 && naturalHeight > 0) {
              setCanvasSize([naturalWidth, naturalHeight]);
              setMeasuredImageUrl(imageUrl);
            }
          }}
        />
      )}
      <svg
        ref={svgRef}
        className="annotation-canvas"
        style={{ aspectRatio: `${width} / ${height}` }}
        role="img"
        aria-label={`${annotations.length} annotations over the active image`}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="xMidYMid meet"
        onDoubleClick={addVertex}
        onPointerMove={onPointerMove}
        onPointerUp={() => setDrag(undefined)}
        onPointerLeave={() => setDrag(undefined)}
        onPointerDown={(event) => {
          if (event.target === event.currentTarget) {
            setDrag({ type: "pan", start: localPoint(event), original: pan });
          }
        }}
      >
        <defs>
          {([1, 2, 3, 4, 5, 6, 7, 8] as const).map((slot) => (
            <pattern key={slot} id={`aa-diagonal-${slot}`} width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
              <line x1="0" y1="0" x2="0" y2="12" stroke={annotationColor(slot)} strokeWidth="4" />
            </pattern>
          ))}
        </defs>
        <g transform={`translate(${pan[0]} ${pan[1]}) scale(${zoom})`}>
          <rect width={width} height={height} fill="var(--aa-surface-muted)" />
          {imageUrl ? (
            <image href={imageUrl} width={width} height={height} />
          ) : (
            <text x={width / 2} y={height / 2} textAnchor="middle" fill="var(--aa-text-muted)" fontSize="22">{t("Select an image or review item")}</text>
          )}
          {annotations.map((annotation) => (
            <AnnotationShape
              key={annotation.id}
              annotation={annotation}
              canvasWidth={width}
              canvasHeight={height}
              visual={annotationVisual(annotation, visualContext)}
              selected={annotation.id === selectedId}
              readOnly={readOnly}
              onKeyboardVertex={(ring, index, point, event) => {
                if (readOnly || !imageUrl || measuredImageUrl !== imageUrl || !workspaceShortcutAllowed(event.nativeEvent, false, false) || event.metaKey || event.ctrlKey || event.altKey) return;
                const next = keyboardPoint(point, event.key, event.shiftKey ? 10 : 1, width, height);
                if (!next) return;
                event.preventDefault(); event.stopPropagation();
                if (next[0] === point[0] && next[1] === point[1]) return;
                onEditStart?.(); moveVertex(annotation, ring, index, next);
              }}
              onSelect={() => onSelect(annotation.id)}
              onVertex={(ring, index, event) => {
                if (readOnly) return;
                event.stopPropagation();
                onSelect(annotation.id);
                onEditStart?.();
                setDrag({ type: "vertex", id: annotation.id, ring, index });
              }}
              onDeleteVertex={(ring, index) => deleteVertex(annotation, ring, index)}
              onBbox={(event) => {
                if (readOnly || annotation.value.kind !== "bounding_box") return;
                event.stopPropagation();
                onSelect(annotation.id);
                onEditStart?.();
                setDrag({
                  type: "bbox",
                  id: annotation.id,
                  start: localPoint(event),
                  original: annotation.value.rect,
                });
              }}
              onBboxResize={(corner, event) => {
                if (readOnly || annotation.value.kind !== "bounding_box") return;
                event.stopPropagation();
                onSelect(annotation.id);
                onEditStart?.();
                setDrag({
                  type: "bbox_resize",
                  id: annotation.id,
                  corner,
                  original: annotation.value.rect,
                });
              }}
            />
          ))}
        </g>
      </svg>
      <p className="canvas-hint">{t(editingHint)}</p>
    </div>
  );
}

function AnnotationShape({
  annotation,
  canvasWidth,
  canvasHeight,
  visual,
  selected,
  readOnly,
  onSelect,
  onVertex,
  onDeleteVertex,
  onKeyboardVertex,
  onBbox,
  onBboxResize,
}: {
  annotation: Annotation;
  canvasWidth: number;
  canvasHeight: number;
  visual: ReturnType<typeof annotationVisual>;
  selected: boolean;
  readOnly: boolean;
  onSelect: () => void;
  onVertex: (ring: number, index: number, event: React.PointerEvent) => void;
  onDeleteVertex: (ring: number, index: number) => void;
  onKeyboardVertex: (ring: number, index: number, point: Point, event: React.KeyboardEvent<SVGCircleElement>) => void;
  onBbox: (event: React.PointerEvent) => void;
  onBboxResize: (
    corner: "nw" | "ne" | "sw" | "se",
    event: React.PointerEvent,
  ) => void;
}) {
  const color = annotationColor(visual.slot);
  const strokeWidth = selected ? 2.6 : 2;
  const strokeDasharray = !selected && visual.pattern === "dashed-box" ? "12 8" : undefined;
  const fill = visual.pattern === "diagonal-fill" ? `url(#aa-diagonal-${visual.slot})` : color;
  const label = `${annotation.label ?? annotation.task_id} ${annotation.confidence ? `${Math.round(annotation.confidence * 100)}%` : ""}`;
  const vertices = (points: Point[], ring = 0) => readOnly ? [] :
    points.map(([x, y], index) => (
      <circle
        key={`${ring}-${index}`}
        cx={x * canvasWidth}
        cy={y * canvasHeight}
        r={selected ? 8 : 6}
        fill="var(--aa-bg)"
        stroke={color}
        strokeWidth={3}
        className="annotation-control"
        role="button"
        tabIndex={selected ? 0 : -1}
        aria-label={t("Vertex {index}: arrows move, Shift moves faster, Delete removes", { index: index + 1 })}
        onPointerDown={(event) => onVertex(ring, index, event)}
        onKeyDown={(event) => {
          if (!workspaceShortcutAllowed(event.nativeEvent, false, false) || event.metaKey || event.ctrlKey || event.altKey) return;
          if (event.key === "Delete" || event.key === "Backspace") {
            event.preventDefault(); event.stopPropagation();
            const group = event.currentTarget.parentElement;
            onDeleteVertex(ring, index);
            requestAnimationFrame(() => {
              const handles = group?.querySelectorAll<SVGCircleElement>('circle[role="button"]');
              handles?.[Math.min(index, handles.length - 1)]?.focus();
            });
          } else {
            onKeyboardVertex(ring, index, [x, y], event);
          }
        }}
      />
    ));

  if (annotation.value.kind === "bounding_box") {
    const [x, y, width, height] = annotation.value.rect;
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        <rect
          x={x * canvasWidth}
          y={y * canvasHeight}
          width={width * canvasWidth}
          height={height * canvasHeight}
          fill={fill}
          fillOpacity={selected ? 0.13 : 0.07}
          stroke={color}
          strokeWidth={strokeWidth}
          strokeDasharray={strokeDasharray}
          className="aa-annotation-shape"
          onPointerDown={onBbox}
        />
        <ShapeLabel x={x * canvasWidth} y={y * canvasHeight} text={label} color={color} />
        {selected && !readOnly &&
          ([
            ["nw", x, y],
            ["ne", x + width, y],
            ["sw", x, y + height],
            ["se", x + width, y + height],
          ] as const).map(([corner, cx, cy]) => (
            <g
              key={corner}
              className="bbox-resize-control"
              data-corner={corner}
              role="button"
              tabIndex={0}
              aria-label={`Resize bounding box from ${corner} corner`}
              onPointerDown={(event) => onBboxResize(corner, event)}
            >
              <circle
                cx={cx * canvasWidth}
                cy={cy * canvasHeight}
                r={12}
                className="bbox-resize-hit-area"
              />
              <circle
                cx={cx * canvasWidth}
                cy={cy * canvasHeight}
                r={5.5}
                fill="var(--aa-bg)"
                stroke={color}
                strokeWidth={2}
                className="annotation-control bbox-resize-dot"
              />
            </g>
          ))}
      </g>
    );
  }
  if (annotation.value.kind === "polyline") {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        <polyline points={pointText(annotation.value.points, canvasWidth, canvasHeight)} fill="none" stroke={color} strokeWidth={strokeWidth} strokeDasharray={strokeDasharray} className="aa-annotation-shape" />
        {vertices(annotation.value.points)}
        <ShapeLabel x={annotation.value.points[0][0] * canvasWidth} y={annotation.value.points[0][1] * canvasHeight} text={label} color={color} />
      </g>
    );
  }
  const rings = annotation.value.kind === "polygon"
    ? annotation.value.rings
    : (annotation.value.kind === "instance_mask" || annotation.value.kind === "semantic_mask") && annotation.value.mask.encoding === "polygon"
      ? annotation.value.mask.rings
      : undefined;
  if (rings) {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        {rings.map((ring, ringIndex) => (
          <g key={ringIndex}>
            <polygon points={pointText(ring, canvasWidth, canvasHeight)} fill={fill} fillOpacity={selected ? 0.14 : 0.08} stroke={color} strokeWidth={strokeWidth} strokeDasharray={strokeDasharray} className="aa-annotation-shape" />
            {vertices(ring, ringIndex)}
          </g>
        ))}
        {rings[0]?.[0] && <ShapeLabel x={rings[0][0][0] * canvasWidth} y={rings[0][0][1] * canvasHeight} text={label} color={color} />}
      </g>
    );
  }
  if (annotation.value.kind === "keypoints") {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        {annotation.value.points.map((keypoint, index) => (
          <g key={keypoint.name}>
            <circle cx={keypoint.point[0] * canvasWidth} cy={keypoint.point[1] * canvasHeight} r={selected ? 10 : 7} fill={color} className="aa-annotation-shape" onPointerDown={(event) => onVertex(0, index, event)}
              role={readOnly ? undefined : "button"} tabIndex={!readOnly && selected ? 0 : undefined}
              aria-label={t("Keypoint {name}: arrows move, Shift moves faster, Delete removes", { name: keypoint.name })}
              onKeyDown={(event) => {
                if (readOnly || !workspaceShortcutAllowed(event.nativeEvent, false, false) || event.metaKey || event.ctrlKey || event.altKey) return;
                if (event.key === "Delete" || event.key === "Backspace") {
                  event.preventDefault(); event.stopPropagation();
                  const group = event.currentTarget.closest(".annotation-shape");
                  onDeleteVertex(0, index);
                  requestAnimationFrame(() => {
                    const handles = group?.querySelectorAll<SVGCircleElement>('circle[role="button"]');
                    handles?.[Math.min(index, handles.length - 1)]?.focus();
                  });
                } else onKeyboardVertex(0, index, keypoint.point, event);
              }} />
            <ShapeLabel x={keypoint.point[0] * canvasWidth} y={keypoint.point[1] * canvasHeight} text={keypoint.name} color={color} />
          </g>
        ))}
      </g>
    );
  }
  return null;
}

function ShapeLabel({ x, y, text, color }: { x: number; y: number; text: string; color: string }) {
  return (
    <g transform={`translate(${x} ${Math.max(16, y - 8)})`}>
      <rect x="0" y="-20" rx="4" width={Math.max(90, text.length * 8)} height="24" fill="var(--aa-surface)" fillOpacity=".94" />
      <text className="aa-annotation-label" x="6" y="-4" fill={color} fontSize="14" fontWeight="700">{text}</text>
    </g>
  );
}
