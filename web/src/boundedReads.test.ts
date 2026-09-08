import { describe, expect, it } from "vitest";
import { boundedReads } from "./boundedReads";

describe("bounded context reads", () => {
  it("limits active reads and preserves source order", async () => {
    let active = 0, peak = 0;
    const result = await boundedReads([0, 1, 2, 3, 4, 5, 6], 3, async value => {
      active++; peak = Math.max(peak, active);
      await new Promise(resolve => setTimeout(resolve, value % 2 ? 1 : 4));
      active--;
      return value * 2;
    }, new AbortController().signal);
    expect(peak).toBe(3);
    expect(result).toEqual([0, 2, 4, 6, 8, 10, 12]);
  });
  it("never admits queued reads after cancellation", async () => {
    const controller = new AbortController(), started: number[] = [];
    await expect(boundedReads([0, 1, 2, 3], 2, async value => {
      started.push(value);
      controller.abort();
      return value;
    }, controller.signal)).rejects.toMatchObject({ name: "AbortError" });
    expect(started).toEqual([0]);
  });
  it("stops admission on failure instead of inventing partial records", async () => {
    const started: number[] = [];
    await expect(boundedReads([0, 1, 2], 1, async value => {
      started.push(value); throw new Error("read failed");
    }, new AbortController().signal)).rejects.toThrow("read failed");
    expect(started).toEqual([0]);
  });
});
