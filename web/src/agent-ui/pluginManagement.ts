import type { api } from "../api";

/** Real management operations, supplied only by HttpAdapter. No Fixture fallback. */
export type PluginManagement = Pick<typeof api,
  "expertPlugins" | "modelInstances" | "modelBundles" | "skills" |
  "testExpertPlugin" | "setExpertPluginEnabled" | "uninstallExpertPlugin" |
  "testModelInstance" | "modelBundleReferences" | "setModelBundleEnabled" |
  "inspectExpertPluginPackage" | "installExpertPluginPackage" |
  "compatibleModelBundles" | "modelInstallOperations" | "modelInstallCommand" | "acceptModelBundleLicense" | "startModelInstallOperation" |
  "inspectModelBundlePackage" | "importModelBundlePackage"
>;
