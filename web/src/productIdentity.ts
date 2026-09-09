import type { EnabledSkill } from "./types";

export type ProductPage = "home" | "projects" | "runs" | "review" | "settings";

export const PRODUCT_NAME = "AnnotAgent";
export const PRODUCT_TAGLINE = "Composable annotation workflows for vision data.";
export const NO_PROJECT_MESSAGE = "No project opened";

export const PRIMARY_NAVIGATION = [
  { page: "projects", label: "Projects", icon: "bbox", href: "/projects" },
  { page: "settings", label: "Settings", icon: "settings", href: "/settings" },
] as const satisfies ReadonlyArray<{ page: ProductPage; label: string; icon: string; href: string }>;

export function activeSkills(project?: { enabled_skills: EnabledSkill[] }): EnabledSkill[] {
  return project?.enabled_skills ?? [];
}
