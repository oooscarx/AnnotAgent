import { describe, expect, it } from "vitest";
import { keyboardBox } from "./bboxKeyboard";

describe("keyboard box editing", () => {
  it("moves in original pixels and preserves size", () => {
    expect(keyboardBox([.2, .3, .1, .2], "ArrowRight", false, 1, 1000, 500)).toEqual({ kind: "bounding_box", rect: [.201, .3, .1, .2] });
  });
  it("resizes from the bottom right with a one-pixel minimum", () => {
    expect(keyboardBox([.2, .3, .01, .02], "ArrowLeft", true, 10, 1000, 500)).toEqual({ kind: "bounding_box", rect: [.2, .3, .001, .02] });
  });
  it("clamps movement and resizing to the image", () => {
    expect(keyboardBox([0, 0, 1, 1], "ArrowDown", false, 10, 1000, 500)).toEqual({ kind: "bounding_box", rect: [0, 0, 1, 1] });
    expect(keyboardBox([0, 0, 1, 1], "ArrowRight", true, 10, 1000, 500)).toEqual({ kind: "bounding_box", rect: [0, 0, 1, 1] });
  });
  it("ignores unrelated keys and unavailable image dimensions", () => {
    expect(keyboardBox([0, 0, .1, .1], "Enter", false, 1, 1000, 500)).toBeUndefined();
    expect(keyboardBox([0, 0, .1, .1], "ArrowRight", false, 1, 0, 500)).toBeUndefined();
  });
});
