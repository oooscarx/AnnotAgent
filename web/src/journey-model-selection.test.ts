import { describe, expect, it } from "vitest";
import { journeyModelSelection } from "./journey-model-selection";

describe("journey model selection", () => {
  const models = Array.from({length: 40}, (_, i) => `model-profile:${i}`);
  it("does not choose an arbitrary subset when a registry exceeds the consent bound", () => {
    expect(journeyModelSelection(models)).toEqual([]);
    expect(journeyModelSelection(models.slice(0,32))).toEqual(models.slice(0,32));
  });
  it("preserves an explicit selection without adding later registry entries", () => {
    expect(journeyModelSelection(models, [models[39],models[3],models[39]])).toEqual([models[39],models[3]]);
    expect(journeyModelSelection(models, [])).toEqual([]);
    expect(journeyModelSelection(models, ["removed",models[1]])).toEqual([models[1]]);
  });
  it("deduplicates available bindings and leaves an excessive explicit selection visible for validation", () => {
    expect(journeyModelSelection(["one","one"])).toEqual(["one"]);
    expect(journeyModelSelection(models, models)).toEqual(models);
  });
});
