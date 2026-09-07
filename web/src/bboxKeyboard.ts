import type { Annotation } from "./types";

/** Keyboard deltas are original-image pixels, independent of canvas zoom. */
export function keyboardBox(rect: [number, number, number, number], key: string, resize: boolean, step: number, width: number, height: number): Annotation["value"] | undefined {
  if (![width, height, step, ...rect].every(Number.isFinite) || width <= 0 || height <= 0 || step <= 0) return;
  const direction: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const delta = direction[key];
  if (!delta) return;
  const [x, y, w, h] = rect;
  if (resize) return { kind: "bounding_box", rect: [x, y, Math.min(1 - x, Math.max(1 / width, w + delta[0] * step / width)), Math.min(1 - y, Math.max(1 / height, h + delta[1] * step / height))] };
  return { kind: "bounding_box", rect: [Math.min(1 - w, Math.max(0, x + delta[0] * step / width)), Math.min(1 - h, Math.max(0, y + delta[1] * step / height)), w, h] };
}
