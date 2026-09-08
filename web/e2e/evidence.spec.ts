import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { isolatedEvidencePath } from "./evidence";

test.describe("isolated browser evidence", () => {
  test("keeps subdirectories and supports absolute legacy paths", () => {
    expect(isolatedEvidencePath("../docs/execution/a/result.png", "/tmp/test-evidence")).toBe("/tmp/test-evidence/a/result.png");
    expect(isolatedEvidencePath(resolve("../docs/execution/b/result.png"), "/tmp/test-evidence")).toBe("/tmp/test-evidence/b/result.png");
  });
  test("does not redirect explicit paths outside legacy evidence or change default behavior", () => {
    expect(isolatedEvidencePath("/tmp/explicit/result.png", "/tmp/test-evidence")).toBe("/tmp/explicit/result.png");
    expect(isolatedEvidencePath("../docs/execution-other/result.png", "/tmp/test-evidence")).toBe("../docs/execution-other/result.png");
    expect(isolatedEvidencePath("../docs/execution/a.png", "")).toBe("../docs/execution/a.png");
  });
});
