import { describe, it, expect, vi, afterEach } from "vitest";
import { FixtureAdapter, initialBoxes } from "../../ui-preview/fixture";
import type { Task, Command, WorkspaceAdapter } from "./adapter";
const cmd = (t: Task): Command => ({
  id: crypto.randomUUID(),
  project: t.project,
  task: t.id,
  revision: t.revision,
});
afterEach(() => vi.useRealTimers());
describe("UI Preview contract and isolation", () => {
  it("has stable owners, explicit simulated models and unknown budget", async () => {
    const a: WorkspaceAdapter = new FixtureAdapter();
    expect(a.kind).toBe("fixture");
    const task = a.snapshot().tasks[0];
    await a.sendMessage(cmd(task), "框出杯子", "plan", "planner");
    expect(a.snapshot().tasks[0].plan?.budget).toBeNull();
    expect(a.snapshot().tasks[0].phase).toBe("awaiting_approval");
  });
  it("rejects wrong owner and stale revision", async () => {
    const a = new FixtureAdapter();
    const c = cmd(a.snapshot().tasks[0]);
    await expect(
      a.sendMessage({ ...c, project: "scenes" }, "x", "plan", "planner"),
    ).rejects.toThrow("归属");
    await expect(
      a.selectAgentModel({ ...c, revision: "old" }, "qwen"),
    ).rejects.toThrow("版本");
  });
  it("queues once and refuses reuse with different payload", async () => {
    const a = new FixtureAdapter();
    a.scenario("new", "running");
    const c = cmd(a.snapshot().tasks[0]);
    await a.sendMessage(c, "下一步", "execute", "planner");
    await a.sendMessage(c, "下一步", "execute", "planner");
    expect(a.snapshot().tasks[0].queue).toEqual(["下一步"]);
    await expect(
      a.sendMessage(c, "别的", "execute", "planner"),
    ).rejects.toThrow("命令");
  });
  it("stop is acknowledged before interrupted; unknown cannot resume", async () => {
    vi.useFakeTimers();
    const a = new FixtureAdapter();
    a.scenario("new", "running");
    const promise = a.interruptOperation(cmd(a.snapshot().tasks[0]));
    expect(a.snapshot().tasks[0].phase).toBe("stopping");
    await vi.runAllTimersAsync();
    await promise;
    expect(a.snapshot().tasks[0].phase).toBe("interrupted");
    a.scenario("new", "outcome_unknown");
    await expect(a.resumeOperation(cmd(a.snapshot().tasks[0]))).rejects.toThrow(
      "不能继续",
    );
  });
  it("freezes active model and persists separate artifact drafts", async () => {
    const a = new FixtureAdapter();
    await a.sendMessage(cmd(a.snapshot().tasks[0]), "x", "plan", "planner");
    await a.selectAgentModel(cmd(a.snapshot().tasks[0]), "qwen");
    expect(a.snapshot().tasks[0].operationModel).toBe("planner");
    a.saveArtifactDraft("new", 1, []);
    expect(a.snapshot().tasks[0].boxes).toEqual(initialBoxes);
    expect(a.snapshot().tasks[0].editBoxes?.[1]).toEqual([]);
  });
  it("failed save preserves human request and does not accept invalid geometry", async () => {
    const a = new FixtureAdapter();
    a.scenario("new", "waiting_for_human");
    a.failNext = true;
    await expect(
      a.answerHumanRequest(cmd(a.snapshot().tasks[0]), initialBoxes),
    ).rejects.toThrow("失败");
    expect(a.snapshot().tasks[0].phase).toBe("waiting_for_human");
    await expect(
      a.answerHumanRequest(cmd(a.snapshot().tasks[0]), [
        { ...initialBoxes[0], w: 9999 },
      ]),
    ).rejects.toThrow("范围");
  });
  it("uses only isolated storage namespace; fixture and transport projection share ViewModel", () => {
    const store = new Map<string, string>();
    const a = new FixtureAdapter({
      getItem: (k) => store.get(k) || null,
      setItem: (k, v) => {
        store.set(k, v);
      },
    });
    a.saveDraft("new", "草稿");
    expect([...store.keys()]).toEqual(["annotagent.ui-preview.v1"]);
    const httpProjection: ReturnType<WorkspaceAdapter["snapshot"]> = JSON.parse(
      JSON.stringify(a.snapshot()),
    );
    expect(httpProjection).toEqual(a.snapshot());
    expect(
      new FixtureAdapter({
        getItem: (k) => store.get(k) || null,
        setItem: () => {},
      }).snapshot().tasks[0].draft,
    ).toBe("草稿");
  });
});
