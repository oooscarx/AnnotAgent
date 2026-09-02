import { expect, test } from "@playwright/test";

test("Rust Expert Model Plugins are reachable, honest, responsive, and keyboard labeled", async ({ page }) => {
  await page.goto("/settings/plugins");
  await expect(page.getByRole("heading", { name: "Expert Model Plugins" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Expert Model Plugins" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByLabel("Agent plugin permissions")).toContainText("read only");
  await expect(page.getByLabel("Installation steps")).toContainText("Verify package");
  await expect(page.getByLabel("Plugin package")).toHaveAttribute("accept", ".annotplugin");
  await expect(page.getByRole("button", { name: "Choose plugin package" })).toBeVisible();
  await expect(page.getByText("No .annotplugin selected")).toBeVisible();
  await expect(page.getByRole("button", { name: "Verify package" })).toBeDisabled();
  await page.getByLabel("Plugin package").setInputFiles({
    name: "annotagent-sam-onnx-1.1.0.annotplugin",
    mimeType: "application/zip",
    buffer: Buffer.from("package fixture"),
  });
  await expect(page.getByText("annotagent-sam-onnx-1.1.0.annotplugin")).toBeVisible();
  await expect(page.getByRole("button", { name: "Verify package" })).toBeEnabled();
  await expect(page.getByText("No Expert Model Plugins installed")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await expect(page.getByRole("heading", { name: "Expert Model Plugins" })).toBeVisible();
});

test("verified Model Bundle setup replaces raw ONNX provisioning and stays responsive", async ({ page }) => {
  const manifest = {
    schema_version: "1",
    id: "org.annotagent.sam-onnx",
    version: "1.0.0",
    display_name: "SAM Prompted Segmentation",
    description: "Isolated Rust prompted-segmentation runtime",
    publisher: "AnnotAgent",
    plugin_api: "1",
    implementation_status: "runnable",
    runtime: { kind: "native_rust_process", entrypoint: "bin/plugin", protocol: "http-vision-v1", startup_timeout_seconds: 60, shutdown_timeout_seconds: 10 },
    compatibility: { annotagent: ">=0.1.0,<0.2.0", targets: ["macos-aarch64"], accelerators: ["cpu"] },
    permissions: { network: "loopback_only", provider_secrets: false, project_files: false, temporary_images: true, plugin_cache: true, subprocesses: false },
    resources: { minimum_memory_mb: 1024, recommended_memory_mb: 2048, minimum_vram_mb: 0, recommended_vram_mb: 0, maximum_response_mb: 64, maximum_concurrency: 1 },
    models: [{ id: "sam-vit-b-onnx", display_name: "SAM ViT-B", capabilities: ["prompted_segmentation"], required_file_roles: ["image_encoder", "mask_decoder"], input_contracts: [], output_contracts: [], score_semantics: "relative_confidence", geometry_semantics: "refined_geometry" }],
    weights: { bundled: false, required: true, provisioning: "local_path", checkpoint_sha256_required: true, components: [{ id: "image_encoder", model_id: "sam-vit-b-onnx", filename: "encoder.onnx" }, { id: "mask_decoder", model_id: "sam-vit-b-onnx", filename: "decoder.onnx" }] },
    license: { code: "MIT", weights: "Apache-2.0", commercial_use: "allowed" },
  };
  const entry = {
    catalog_id: "annotagent-curated",
    bundle_id: "org.annotagent.models.efficient-prompted-segmentation",
    bundle_version: "1.0.0",
    display_name: "Efficient prompted segmentation model",
    description: "Pinned two-file ONNX model for box-prompt refinement.",
    capabilities: ["prompted_segmentation"],
    compatible_plugins: [{ plugin_id: manifest.id, plugin_version: "=1.0.0", model_id: "sam-vit-b-onnx", contract_hash: "a".repeat(64), required_file_roles: ["image_encoder", "mask_decoder"] }],
    platform_requirements: [{ target: "macos-aarch64", execution_providers: ["cpu"], minimum_memory_mb: 1024, minimum_disk_bytes: 4096 }],
    bundle_url: "https://models.annotagent.example/model.annotmodel",
    bundle_sha256: "b".repeat(64),
    bundle_size_bytes: 4096,
    license_summary: { name: "Apache-2.0", license_url: "https://www.apache.org/licenses/LICENSE-2.0", license_digest: "c".repeat(64), redistribution: "allowed", commercial_use: "allowed", requires_acceptance: true },
    publisher: { id: "annotagent", display_name: "AnnotAgent", verified: true },
    fixture: false,
    publishable: true,
  };
  await page.route("**/api/plugins", (route) => route.fulfill({ json: {
    installations: [{ manifest, package_sha256: "d".repeat(64), signature: "unsigned", status: "needs_weights", enabled: true, installed_at: "2026-09-03T00:00:00Z", updated_at: "2026-09-03T00:00:00Z", weights: [], references: [] }],
    models: [],
    agent_permissions: { discover: true, install: false, accept_licenses: false, provision_weights: false },
  } }));
  await page.route("**/api/model-instances", (route) => route.fulfill({ json: { instances: [], model_profiles: [] } }));
  const compatibilityRoute = "**/api/plugins/org.annotagent.sam-onnx/1.0.0/compatible-model-bundles";
  await page.route(compatibilityRoute, (route) => route.fulfill({ json: { plugin_runtime_status: "installed", available: [entry], installed: [], setup_blockers: [] } }));
  let operationVisible = false;
  const operation = {
    id: "f641755a-a4ad-46a3-a440-dcc2948f1507",
    catalog_id: entry.catalog_id,
    bundle_id: entry.bundle_id,
    bundle_version: entry.bundle_version,
    plugin_id: manifest.id,
    plugin_version: manifest.version,
    status: "running",
    stage: "running_sample_inference",
    bytes_completed: entry.bundle_size_bytes,
    bytes_total: entry.bundle_size_bytes,
    detail: "Running the real image and bbox prompt through the Rust Plugin",
    error: null,
    suggested_action: null,
    model_instance_ids: ["4cecf8f0-e12d-45ce-a34a-a100a252d017"],
    created_at: "2026-09-03T00:00:00Z",
    updated_at: "2026-09-03T00:00:01Z",
  };
  await page.route("**/api/model-installations", (route) => {
    if (route.request().method() === "POST") {
      operationVisible = true;
      return route.fulfill({ status: 202, json: operation });
    }
    return route.fulfill({ json: { operations: operationVisible ? [operation] : [] } });
  });
  await page.route("**/api/model-bundles/*/*/license-acceptance", (route) => route.fulfill({ status: 204 }));

  await page.setViewportSize({ width: 1024, height: 900 });
  await page.goto("/settings/plugins");
  await expect(page.getByText("No compatible model installed").first()).toBeVisible();
  await expect(page.getByText("Compatible Models")).toBeVisible();
  await expect(page.getByText("Installed Models")).toBeVisible();
  await expect(page.getByText("Model Setup", { exact: true })).toBeVisible();
  await expect(page.getByText("References", { exact: true })).toBeVisible();
  await expect(page.locator('input[accept*="onnx"]')).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();

  await page.getByRole("button", { name: "Install compatible model" }).first().click();
  const setupDialog = page.getByRole("dialog", { name: "SAM Prompted Segmentation" });
  await expect(setupDialog).toBeVisible();
  await expect(page.locator(".model-setup-backdrop")).toHaveCSS("position", "fixed");
  await expect(setupDialog.getByRole("button", { name: "Close model setup" })).toBeFocused();
  const desktopDialogBox = await setupDialog.boundingBox();
  expect(desktopDialogBox).not.toBeNull();
  expect(desktopDialogBox!.y).toBeGreaterThanOrEqual(0);
  expect(desktopDialogBox!.y + desktopDialogBox!.height).toBeLessThanOrEqual(900);
  await page.keyboard.press("Escape");
  await expect(setupDialog).toBeHidden();
  await expect(page.getByRole("button", { name: "Install compatible model" }).first()).toBeFocused();
  await page.getByRole("button", { name: "Install compatible model" }).first().click();
  await expect(setupDialog).toBeVisible();
  await expect(page.getByLabel("Model installation progress")).toContainText("Review license");
  await page.getByRole("radio").check();
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Review source", { exact: true }).last()).toBeVisible();
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByText("I accept this exact model license and digest.").click();
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Check compatibility", { exact: true }).last()).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await expect(setupDialog).toBeVisible();
  expect(await setupDialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBeTruthy();
  const mobileDialogBox = await setupDialog.boundingBox();
  expect(mobileDialogBox).not.toBeNull();
  expect(mobileDialogBox!.y).toBeGreaterThanOrEqual(0);
  expect(mobileDialogBox!.y + mobileDialogBox!.height).toBeLessThanOrEqual(844);
  await setupDialog.getByRole("button", { name: "Install model" }).click();
  await expect(setupDialog.getByLabel("Real model installation stages")).toContainText("Run real sample inference");
  await expect(setupDialog).toContainText("Running the real image and bbox prompt through the Rust Plugin");
  await page.reload();
  await expect(page.getByRole("button", { name: "View installation" })).toBeVisible();
  await page.getByRole("button", { name: "View installation" }).click();
  await expect(page.getByRole("dialog", { name: "SAM Prompted Segmentation" })).toContainText("Run real sample inference");
  await page.getByRole("button", { name: "Close model setup" }).click();
  operationVisible = false;
  await page.reload();
  await expect(page.getByText("No compatible model installed").first()).toBeVisible();
  await expect(page.locator('input[accept*="onnx"]')).toHaveCount(0);

  await page.unroute(compatibilityRoute);
  await page.route(compatibilityRoute, (route) => route.fulfill({ json: {
    plugin_runtime_status: "installed",
    available: [],
    installed: [],
    setup_blockers: [{
      bundle_id: entry.bundle_id,
      bundle_version: entry.bundle_version,
      code: "plugin_version_incompatible",
      message: "Installed Plugin 1.0.0 does not satisfy the model requirement =1.1.0. Install a compatible immutable Plugin runtime version before installing this Bundle.",
    }],
  } }));
  await page.reload();
  await expect(page.getByText("Runtime update required", { exact: true })).toBeVisible();
  await expect(page.getByText("No verified bundle is available for this platform")).toHaveCount(0);
  await page.getByRole("button", { name: "Review required update" }).first().click();
  await expect(page.getByRole("dialog", { name: "SAM Prompted Segmentation" })).toContainText("Plugin runtime update required");
  await expect(page.getByRole("dialog", { name: "SAM Prompted Segmentation" })).toContainText("Update required");
  await expect(page.getByRole("dialog", { name: "SAM Prompted Segmentation" })).toContainText("1.1.0");
  await expect(page.getByRole("radio")).toHaveCount(0);
});
