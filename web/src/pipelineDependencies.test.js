import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
it("keeps geometry and artifact helpers independent of legacy pages and APIs", () => {
  const source=readFileSync(new URL("./pipelinePresentation.ts",import.meta.url),"utf8");
  const imports=[...source.matchAll(/from\s+["']([^"']+)["']/g)].map(match=>match[1]);
  expect(imports.every(path=>["./types","./annotationVisuals","./skills/visualProfiles"].includes(path))).toBe(true);
  const tests=readFileSync(new URL("./labelPipelineUi.test.ts",import.meta.url),"utf8");
  expect(tests).not.toMatch(/from\s+["']\.\/App["']/);
});
