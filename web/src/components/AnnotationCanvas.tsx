import { useMemo, useRef, useState } from "react";
import type { Annotation, Point } from "../types";

interface Props {
  imageUrl?: string;
  annotations: Annotation[];
  selectedId?: string;
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
    <div className="canvas-shell">
      <div className="canvas-tools">
        <span>SVG overlay</span>
        <button onClick={() => setZoom((value) => Math.max(0.5, value - 0.2))}>−</button>
        <strong>{Math.round(zoom * 100)}%</strong>
        <button onClick={() => setZoom((value) => Math.min(4, value + 0.2))}>+</button>
        <button onClick={() => { setZoom(1); setPan([0, 0]); }}>Reset</button>
      </div>
      <svg
        ref={svgRef}
        className="annotation-canvas"
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
        <g transform={`translate(${pan[0]} ${pan[1]}) scale(${zoom})`}>
          <rect width={WIDTH} height={HEIGHT} fill="#07100d" />
          {imageUrl ? (
            <image href={imageUrl} width={WIDTH} height={HEIGHT} preserveAspectRatio="xMidYMid meet" />
          ) : (
            <text x="500" y="325" textAnchor="middle" fill="#8ca299" fontSize="22">
              Select an image or review item
            </text>
          )}
          {annotations.map((annotation) => (
            <AnnotationShape
              key={annotation.id}
              annotation={annotation}
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
  selected,
  onSelect,
  onVertex,
  onBbox,
}: {
  annotation: Annotation;
  selected: boolean;
  onSelect: () => void;
  onVertex: (ring: number, index: number, event: React.PointerEvent) => void;
  onBbox: (event: React.PointerEvent) => void;
}) {
  const color = selected ? "#f7c65a" : "#60e9ac";
  const strokeWidth = selected ? 4 : 2;
  const label = `${annotation.label ?? annotation.task_id} ${annotation.confidence ? `${Math.round(annotation.confidence * 100)}%` : ""}`;
  const vertices = (points: Point[], ring = 0) =>
    points.map(([x, y], index) => (
      <circle
        key={`${ring}-${index}`}
        cx={x * WIDTH}
        cy={y * HEIGHT}
        r={selected ? 7 : 4}
        fill="#07100d"
        stroke={color}
        strokeWidth={3}
        onPointerDown={(event) => onVertex(ring, index, event)}
      />
    ));

  if (annotation.value.kind === "bounding_box") {
    const [x, y, width, height] = annotation.value.rect;
    return (
      <g onClick={onSelect}>
        <rect
          x={x * WIDTH}
          y={y * HEIGHT}
          width={width * WIDTH}
          height={height * HEIGHT}
          fill={selected ? "rgba(247,198,90,.12)" : "rgba(96,233,172,.08)"}
          stroke={color}
          strokeWidth={strokeWidth}
          onPointerDown={onBbox}
        />
        <ShapeLabel x={x * WIDTH} y={y * HEIGHT} text={label} color={color} />
      </g>
    );
  }
  if (annotation.value.kind === "polyline") {
    return (
      <g onClick={onSelect}>
        <polyline points={pointText(annotation.value.points)} fill="none" stroke={color} strokeWidth={strokeWidth} />
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
      <g onClick={onSelect}>
        {rings.map((ring, ringIndex) => (
          <g key={ringIndex}>
            <polygon points={pointText(ring)} fill="rgba(96,233,172,.12)" stroke={color} strokeWidth={strokeWidth} />
            {vertices(ring, ringIndex)}
          </g>
        ))}
        {rings[0]?.[0] && <ShapeLabel x={rings[0][0][0] * WIDTH} y={rings[0][0][1] * HEIGHT} text={label} color={color} />}
      </g>
    );
  }
  if (annotation.value.kind === "keypoints") {
    return (
      <g onClick={onSelect}>
        {annotation.value.points.map((keypoint, index) => (
          <g key={keypoint.name}>
            <circle cx={keypoint.point[0] * WIDTH} cy={keypoint.point[1] * HEIGHT} r={9} fill={color} onPointerDown={(event) => onVertex(0, index, event)} />
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
      <rect x="0" y="-18" width={Math.max(90, text.length * 8)} height="22" fill="#07100d" />
      <text x="6" y="-3" fill={color} fontSize="14" fontWeight="700">{text}</text>
    </g>
  );
}
