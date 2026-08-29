import type { Point } from "./types";

export const MIN_CANVAS_ZOOM = 0.5;
export const MAX_CANVAS_ZOOM = 4;

export function clampCanvasZoom(value: number): number {
  return Math.max(MIN_CANVAS_ZOOM, Math.min(MAX_CANVAS_ZOOM, value));
}

export function zoomAroundPoint(
  zoom: number,
  pan: Point,
  anchor: Point,
  requestedZoom: number,
): { zoom: number; pan: Point } {
  const nextZoom = clampCanvasZoom(requestedZoom);
  const ratio = nextZoom / zoom;
  return {
    zoom: nextZoom,
    pan: [
      anchor[0] - (anchor[0] - pan[0]) * ratio,
      anchor[1] - (anchor[1] - pan[1]) * ratio,
    ],
  };
}
