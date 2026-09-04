import { describe, expect, it, vi } from "vitest";
import { RouteQueryCache, queryKeys } from "./queryCache";

describe("route-aware query cache", () => {
  it("deduplicates the same in-flight resource", async () => {
    const cache = new RouteQueryCache();
    let resolve!: (value: string) => void;
    const loader = vi.fn(() => new Promise<string>((done) => { resolve = done; }));
    const first = cache.load("project/a", loader);
    const second = cache.load("project/a", loader);
    expect(loader).toHaveBeenCalledTimes(1);
    resolve("A");
    await expect(first).resolves.toBe("A");
    await expect(second).resolves.toBe("A");
  });

  it("aborts an older generation and never lets it replace the current value", async () => {
    const cache = new RouteQueryCache();
    let firstSignal: AbortSignal | undefined;
    let resolveFirst!: (value: string) => void;
    let resolveSecond!: (value: string) => void;
    const first = cache.load("project/a", (signal) => {
      firstSignal = signal;
      return new Promise<string>((done) => { resolveFirst = done; });
    });
    const second = cache.load("project/a", () =>
      new Promise<string>((done) => { resolveSecond = done; }), { force: true });
    expect(firstSignal?.aborted).toBe(true);
    resolveSecond("new");
    await expect(second).resolves.toBe("new");
    resolveFirst("old");
    await expect(first).resolves.toBe("old");
    expect(cache.snapshot<string>("project/a").data).toBe("new");
  });

  it("invalidates only the requested resource family", async () => {
    const cache = new RouteQueryCache();
    await cache.load(queryKeys.runResults("run-a"), async () => "result");
    await cache.load(queryKeys.project("project-b"), async () => "project");
    cache.invalidate(queryKeys.run("run-a"));
    expect(cache.snapshot(queryKeys.runResults("run-a")).stale).toBe(true);
    expect(cache.snapshot(queryKeys.project("project-b")).stale).toBe(false);
  });
});
