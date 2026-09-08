import { resolve } from "node:path";
import { expect, test } from "./fixtures";

test("TEST metadata transport: local setup restores progress without another install or image call", async ({ page }) => {
  // Only the delivery transport is simulated. Project uploads/goal persistence
  // use the isolated server. No plugin, weights or native inference are installed.
  const entry = { catalog_id: "test", bundle_id: "test-model", bundle_version: "1.0.0", display_name: "Local classifier TEST metadata", description: "Simulated delivery state; not an inference or model-quality test", capabilities: ["image_classification"], bundle_url: "https://example.invalid/TEST-model", bundle_size_bytes: 1024, bundle_sha256: "test-bundle", publisher: { display_name: "TEST publisher", verified: true }, fixture: false, publishable: true, license_summary: { name: "TEST license", requires_acceptance: true, license_digest: "test-license", license_url: "https://example.invalid/TEST-license" } };
  const plugin = { enabled: true, manifest: { id: "test-plugin", version: "1.0.0" }, package_sha256: "test-plugin-digest" };
  let installed = false;
  let started = false;
  let installCalls = 0;
  let licenseCalls = 0;
  const charged: string[] = [];
  await page.route(/\/api\/model-profiles(?:\?.*)?$/, (route) => route.fulfill({ json: { models: [] } }));
  await page.route("**/api/agent-model-bindings", (route) => route.fulfill({ json: {} }));
  const operation = () => ({ id: "test-operation", catalog_id: "test", bundle_id: entry.bundle_id, bundle_version: entry.bundle_version, plugin_id: plugin.manifest.id, plugin_version: plugin.manifest.version, status: installed ? "succeeded" : "running", stage: installed ? "ready" : "downloading_bundle", detail: installed ? "TEST local model prepared" : "TEST download in progress", bytes_completed: installed ? 1024 : 128, bytes_total: 1024, model_instance_ids: installed ? ["test-instance"] : [], created_at: "2026-09-08T00:00:00Z", updated_at: "2026-09-08T00:00:00Z" });
  await page.route("**/api/plugins", (route) => route.fulfill({ json: { installations: [plugin], models: [] } }));
  await page.route("**/compatible-model-bundles", (route) => route.fulfill({ json: { plugin_runtime_status: "ready", available: [entry], installed: [], setup_blockers: [] } }));
  await page.route("**/api/model-bundles", (route) => route.fulfill({ json: { bundles: installed ? [{ enabled: true, manifest: { ...entry, id: entry.bundle_id, version: entry.bundle_version }, bundle_sha256: entry.bundle_sha256 }] : [] } }));
  await page.route("**/api/model-instances", (route) => route.fulfill({ json: { instances: installed ? [{ id: "test-instance", status: "ready", model_profile_revision: 1, plugin_id: plugin.manifest.id, plugin_version: plugin.manifest.version, plugin_package_sha256: plugin.package_sha256, model_bundle_id: entry.bundle_id, model_bundle_version: entry.bundle_version, model_bundle_sha256: entry.bundle_sha256, contract_inspection: { valid: true }, smoke_test_result: { status: "passed" } }] : [], model_profiles: installed ? [{ model_instance_id: "test-instance", model_profile_revision: 1, selectable: true, availability: "available", capabilities: entry.capabilities, display_name: entry.display_name }] : [] } }));
  await page.route("**/api/model-installations", async (route) => {
    if (route.request().method() === "POST") { installCalls++; started = true; return route.fulfill({ status: 202, json: operation() }); }
    return route.fulfill({ json: { operations: started ? [operation()] : [] } });
  });
  await page.route("**/api/model-installations/test-operation", (route) => route.fulfill({ json: operation() }));
  await page.route("**/license-acceptance", (route) => { licenseCalls++; return route.fulfill({ status: 204 }); });
  page.on("request", (request) => { if (request.method() === "POST" && /suggest|sample-operations|dry-run|active-probe/.test(request.url())) charged.push(request.url()); });
  await page.goto("/projects?new=1");
  await page.getByLabel("Choose images", { exact: true }).setInputFiles(resolve("../examples/robocup/images/synthetic-robocup.png"));
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page).toHaveURL(/\/work$/);
  const projectId = new URL(page.url()).pathname.split("/")[2];
  // Retained legacy deep-link contract; new project entry is the conversation workspace.
  await page.goto(`/projects/${projectId}/task/goal`);
  await page.getByRole("radio", { name: /Image categories/ }).check();
  await page.getByLabel("Categories to keep", { exact: true }).fill("day, night");
  await page.getByLabel("Describe your goal", { exact: true }).fill("TEST local setup only");
  await page.getByRole("button", { name: /Save goal and connect model|Connect an image model|Prepare sample results/ }).click();
  await expect(page).toHaveURL(/\/task\/model/);
  await page.goto(`/projects/${projectId}/task/model?purpose=vision`);
  await page.getByRole("button", { name: "Prepare a local image model", exact: true }).click();
  await page.getByRole("radio", { name: /^Local classifier TEST metadata/ }).check();
  await expect(page.getByRole("button", { name: "Install this local model", exact: true })).toBeDisabled();
  await page.reload();
  await expect(page.getByRole("radio", { name: /^Local classifier TEST metadata/ })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "I accept this model license and authorize this download and local test", exact: true })).not.toBeChecked();
  expect(installCalls).toBe(0);
  await page.getByRole("checkbox", { name: "I accept this model license and authorize this download and local test", exact: true }).check();
  await page.getByRole("button", { name: "Install this local model", exact: true }).click();
  await expect(page.getByText("TEST download in progress", { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByText("TEST download in progress", { exact: true })).toBeVisible();
  expect(installCalls).toBe(1); expect(licenseCalls).toBe(1);
  installed = true;
  await expect(page.getByRole("button", { name: "Return to saved task", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Return to saved task", exact: true }).click();
  await expect(page).toHaveURL(`/projects/${projectId}/task/goal`);
  await expect(page.getByLabel("Categories to keep", { exact: true })).toHaveValue("day, night");
  await expect(page.getByLabel("Describe your goal", { exact: true })).toHaveValue("TEST local setup only");
  expect(charged).toEqual([]); expect(installCalls).toBe(1);
});
