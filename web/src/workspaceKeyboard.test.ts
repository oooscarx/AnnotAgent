import { describe, expect, it } from "vitest";
import { workspaceShortcutAllowed } from "./workspaceKeyboard";

describe("workspace shortcuts", () => {
  it("allows focused canvas navigation", () => {
    expect(workspaceShortcutAllowed({}, false, false)).toBe(true);
  });
  it.each([
    [{ isComposing: true }, false, false],
    [{ keyCode: 229 }, false, false],
    [{ defaultPrevented: true }, false, false],
    [{}, true, false],
    [{}, false, true],
  ] as const)("does not consume IME, input, dialogs or handled keys (%j)", (event, input, dialog) => {
    expect(workspaceShortcutAllowed(event, input, dialog)).toBe(false);
  });
});
