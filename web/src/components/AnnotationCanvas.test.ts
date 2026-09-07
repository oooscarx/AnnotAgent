import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AnnotationCanvas } from "./AnnotationCanvas";
import type { Annotation } from "../types";

function canvas(value: Annotation["value"], readOnly: boolean) {
  const annotation = { id: "test", task_id: "shape", label: "TEST", value, provenance: {} } as Annotation;
  return renderToStaticMarkup(createElement(AnnotationCanvas, { annotations: [annotation], selectedId: annotation.id, readOnly, onSelect() {}, onChange() {} }));
}
describe("shared canvas keyboard surfaces", () => {
  it("exposes labeled, focusable polygon vertices only while editable", () => {
    const value: Annotation["value"] = { kind: "polygon", rings: [[[.1, .1], [.5, .1], [.5, .5]]] };
    expect(canvas(value, false)).toContain("arrows move, Shift moves faster, Delete removes");
    expect(canvas(value, false)).toContain('tabindex="0"');
    expect(canvas(value, true)).not.toContain('class="annotation-control"');
  });
  it("exposes named keypoints without making read-only points interactive", () => {
    const value: Annotation["value"] = { kind: "keypoints", points: [{ name: "TEST corner", point: [.5, .5], visible: true }] };
    expect(canvas(value, false)).toContain('role="button" tabindex="0"');
    expect(canvas(value, true)).not.toContain('role="button" tabindex="0"');
  });
  it("does not offer pixel bbox controls before an actual image is measured", () => {
    expect(canvas({ kind: "bounding_box", rect: [.1, .1, .2, .2] }, false)).not.toContain("Move box with arrow keys");
  });
});
