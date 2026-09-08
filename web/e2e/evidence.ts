import { isAbsolute, relative, resolve, sep } from "node:path";

/** Redirect legacy evidence only; preserve explicit external output paths and defaults. */
export function isolatedEvidencePath(path: string, destination = process.env.ANNOTAGENT_E2E_EVIDENCE_DIR): string {
  if (!destination) return path;
  const suffix = relative(resolve("../docs/execution"), resolve(path));
  if (!suffix || suffix === ".." || suffix.startsWith(`..${sep}`) || isAbsolute(suffix)) return path;
  return resolve(destination, suffix);
}
