import { describe, expect, it, vi } from "vitest";
import { api } from "./api";

describe("API client", () => {
  it("reports server errors instead of silently accepting them", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      statusText: "Bad Request",
      json: () => Promise.resolve({ error: "invalid project" }),
    }));
    await expect(api.health()).rejects.toThrow("invalid project");
    vi.unstubAllGlobals();
  });
});
