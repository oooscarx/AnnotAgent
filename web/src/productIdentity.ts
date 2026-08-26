import type { EnabledSkill } from "./types";

export type ProductPage = "dashboard" | "projects" | "project" | "workflows" | "models" | "skills" | "runs" | "review" | "settings";

export const PRODUCT_NAME = "AnnotAgent";
export const PRODUCT_TAGLINE = "Composable annotation workflows for vision data.";
export const NO_PROJECT_MESSAGE = "No project opened";

export const PRIMARY_NAVIGATION = [
  { page: "dashboard", label: "Dashboard", icon: "history" },
  { page: "projects", label: "Projects", icon: "bbox" },
  { page: "workflows", label: "Workflows", icon: "tool-call" },
  { page: "models", label: "Models", icon: "model-call" },
  { page: "skills", label: "Skills", icon: "validate" },
  { page: "runs", label: "Runs", icon: "agent-trace" },
  { page: "review", label: "Review", icon: "review" },
  { page: "settings", label: "Settings", icon: "settings" },
] as const satisfies ReadonlyArray<{ page: ProductPage; label: string; icon: string }>;

export function activeSkills(project?: { enabled_skills: EnabledSkill[] }): EnabledSkill[] {
  return project?.enabled_skills ?? [];
}
