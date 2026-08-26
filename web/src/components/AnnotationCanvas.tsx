import { useMemo, useRef, useState } from "react";
import { annotationColor, annotationVisual } from "../annotationVisuals";
import type { AnnotationVisualContext } from "../annotationVisuals";
import type { Annotation, Point } from "../types";

interface Props {
  imageUrl?: string;
  annotations: Annotation[];
  selectedId?: string;
  visualContext?: AnnotationVisualContext;
  onSelect: (id: string) => void;
  onChange: (annotation: Annotation) => void;
}

const WIDTH = 1000;
const HEIGHT = 650;
const pointText = (points: Point[]) =>
  points.map(([x, y]) => `${x * WIDTH},${y * HEIGHT}`).join(" ");

export function AnnotationCanvas({
  imageUrl,
  annotations,
  selectedId,
  visualContext,
  onSelect,
  onChange,
}: Props) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<Point>([0, 0]);
  const [drag, setDrag] = useState<
    | { type: "vertex"; id: string; ring: number; index: number }
    | { type: "bbox"; id: string; start: Point; original: [number, number, number, number] }
    | { type: "pan"; start: Point; original: Point }
  >();

  const selected = useMemo(
    () => annotations.find((annotation) => annotation.id === selectedId),
    [annotations, selectedId],
  );

  const localPoint = (event: React.PointerEvent<Element>): Point => {
    const bounds = svgRef.current?.getBoundingClientRect();
    if (!bounds) return [0, 0];
    return [
      Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
      Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)),
    ];
  };

  const moveVertex = (annotation: Annotation, ring: number, index: number, point: Point) => {
    const value = structuredClone(annotation.value);
    if (value.kind === "polyline") value.points[index] = point;
    if (value.kind === "polygon") value.rings[ring][index] = point;
    if (value.kind === "keypoints") value.points[index].point = point;
    if (value.kind === "instance_mask" && value.mask.kind === "polygon") {
      value.mask.rings[ring][index] = point;
    }
    onChange({ ...annotation, value });
  };

  const onPointerMove = (event: React.PointerEvent<SVGSVGElement>) => {
    if (!drag) return;
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
    } else {
      setPan([
        drag.original[0] + (point[0] - drag.start[0]) * WIDTH,
        drag.original[1] + (point[1] - drag.start[1]) * HEIGHT,
      ]);
    }
  };

  const addVertex = (event: React.MouseEvent<SVGSVGElement>) => {
    if (!selected || event.detail !== 2) return;
    const point = localPoint(event as unknown as React.PointerEvent<SVGSVGElement>);
    const value = structuredClone(selected.value);
    if (value.kind === "polyline") value.points.push(point);
    if (value.kind === "polygon") value.rings[0]?.push(point);
    if (value.kind === "instance_mask" && value.mask.kind === "polygon") {
      value.mask.rings[0]?.push(point);
    }
    onChange({ ...selected, value });
  };

  return (
    <div className="canvas-shell aa-dark">
      <div className="canvas-tools">
        <span>Annotation workspace</span>
        <button aria-label="Zoom out" title="Zoom out" onClick={() => setZoom((value) => Math.max(0.5, value - 0.2))}>−</button>
        <strong>{Math.round(zoom * 100)}%</strong>
        <button aria-label="Zoom in" title="Zoom in" onClick={() => setZoom((value) => Math.min(4, value + 0.2))}>+</button>
        <button aria-label="Reset canvas view" title="Reset canvas view" onClick={() => { setZoom(1); setPan([0, 0]); }}>Reset</button>
      </div>
      <ul className="canvas-annotation-list" aria-label="Annotations on canvas">
        {annotations.map((annotation) => {
          const visual = annotationVisual(annotation, visualContext);
          return <li key={annotation.id}><button aria-pressed={annotation.id === selectedId} onClick={() => onSelect(annotation.id)}><i aria-hidden="true" style={{ borderColor: annotationColor(visual.slot) }} /><span><strong>{annotation.label ?? annotation.task_id}</strong><small>{annotation.value.kind.replaceAll("_", " ")} · {Math.round((annotation.confidence ?? 0) * 100)}%</small></span></button></li>;
        })}
        {annotations.length === 0 && <li className="canvas-annotation-empty">No annotations selected</li>}
      </ul>
      <svg
        ref={svgRef}
        className="annotation-canvas"
        role="img"
        aria-label={`${annotations.length} annotations over the active image`}
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        onDoubleClick={addVertex}
        onPointerMove={onPointerMove}
        onPointerUp={() => setDrag(undefined)}
        onPointerLeave={() => setDrag(undefined)}
        onPointerDown={(event) => {
          if (event.target === event.currentTarget) {
            setDrag({ type: "pan", start: localPoint(event), original: pan });
          }
        }}
        onWheel={(event) => {
          event.preventDefault();
          setZoom((value) => Math.max(0.5, Math.min(4, value - event.deltaY * 0.001)));
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
          <rect width={WIDTH} height={HEIGHT} fill="var(--aa-bg)" />
          {imageUrl ? (
            <image href={imageUrl} width={WIDTH} height={HEIGHT} preserveAspectRatio="xMidYMid meet" />
          ) : (
            <text x="500" y="325" textAnchor="middle" fill="var(--aa-text-muted)" fontSize="22">
              Select an image or review item
            </text>
          )}
          {annotations.map((annotation) => (
            <AnnotationShape
              key={annotation.id}
              annotation={annotation}
              visual={annotationVisual(annotation, visualContext)}
              selected={annotation.id === selectedId}
              onSelect={() => onSelect(annotation.id)}
              onVertex={(ring, index, event) => {
                event.stopPropagation();
                onSelect(annotation.id);
                setDrag({ type: "vertex", id: annotation.id, ring, index });
              }}
              onBbox={(event) => {
                if (annotation.value.kind !== "bounding_box") return;
                event.stopPropagation();
                onSelect(annotation.id);
                setDrag({
                  type: "bbox",
                  id: annotation.id,
                  start: localPoint(event),
                  original: annotation.value.rect,
                });
              }}
            />
          ))}
        </g>
      </svg>
      <p className="canvas-hint">Drag shapes and vertices · double-click to add a line/polygon vertex · wheel to zoom</p>
    </div>
  );
}

function AnnotationShape({
  annotation,
  visual,
  selected,
  onSelect,
  onVertex,
  onBbox,
}: {
  annotation: Annotation;
  visual: ReturnType<typeof annotationVisual>;
  selected: boolean;
  onSelect: () => void;
  onVertex: (ring: number, index: number, event: React.PointerEvent) => void;
  onBbox: (event: React.PointerEvent) => void;
}) {
  const color = annotationColor(visual.slot);
  const strokeWidth = selected ? 4 : 2.5;
  const strokeDasharray = !selected && visual.pattern === "dashed-box" ? "12 8" : undefined;
  const fill = visual.pattern === "diagonal-fill" ? `url(#aa-diagonal-${visual.slot})` : color;
  const label = `${annotation.label ?? annotation.task_id} ${annotation.confidence ? `${Math.round(annotation.confidence * 100)}%` : ""}`;
  const vertices = (points: Point[], ring = 0) =>
    points.map(([x, y], index) => (
      <circle
        key={`${ring}-${index}`}
        cx={x * WIDTH}
        cy={y * HEIGHT}
        r={selected ? 8 : 6}
        fill="var(--aa-bg)"
        stroke={color}
        strokeWidth={3}
        className="annotation-control"
        onPointerDown={(event) => onVertex(ring, index, event)}
      />
    ));

  if (annotation.value.kind === "bounding_box") {
    const [x, y, width, height] = annotation.value.rect;
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        <rect
          x={x * WIDTH}
          y={y * HEIGHT}
          width={width * WIDTH}
          height={height * HEIGHT}
          fill={fill}
          fillOpacity={selected ? 0.18 : 0.09}
          stroke={color}
          strokeWidth={strokeWidth}
          strokeDasharray={strokeDasharray}
          className="aa-annotation-shape"
          onPointerDown={onBbox}
        />
        <ShapeLabel x={x * WIDTH} y={y * HEIGHT} text={label} color={color} />
      </g>
    );
  }
  if (annotation.value.kind === "polyline") {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        <polyline points={pointText(annotation.value.points)} fill="none" stroke={color} strokeWidth={strokeWidth} strokeDasharray={strokeDasharray} className="aa-annotation-shape" />
        {vertices(annotation.value.points)}
        <ShapeLabel x={annotation.value.points[0][0] * WIDTH} y={annotation.value.points[0][1] * HEIGHT} text={label} color={color} />
      </g>
    );
  }
  const rings = annotation.value.kind === "polygon"
    ? annotation.value.rings
    : annotation.value.kind === "instance_mask" && annotation.value.mask.kind === "polygon"
      ? annotation.value.mask.rings
      : undefined;
  if (rings) {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        {rings.map((ring, ringIndex) => (
          <g key={ringIndex}>
            <polygon points={pointText(ring)} fill={fill} fillOpacity={selected ? 0.2 : 0.1} stroke={color} strokeWidth={strokeWidth} strokeDasharray={strokeDasharray} className="aa-annotation-shape" />
            {vertices(ring, ringIndex)}
          </g>
        ))}
        {rings[0]?.[0] && <ShapeLabel x={rings[0][0][0] * WIDTH} y={rings[0][0][1] * HEIGHT} text={label} color={color} />}
      </g>
    );
  }
  if (annotation.value.kind === "keypoints") {
    return (
      <g className={selected ? "annotation-shape selected" : "annotation-shape"} onClick={onSelect}>
        {annotation.value.points.map((keypoint, index) => (
          <g key={keypoint.name}>
            <circle cx={keypoint.point[0] * WIDTH} cy={keypoint.point[1] * HEIGHT} r={selected ? 10 : 7} fill={color} className="aa-annotation-shape" onPointerDown={(event) => onVertex(0, index, event)} />
            <ShapeLabel x={keypoint.point[0] * WIDTH} y={keypoint.point[1] * HEIGHT} text={keypoint.name} color={color} />
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
