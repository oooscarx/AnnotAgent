/** Explicit categories remain deterministic; parsing never requires an LLM. */
export function splitGoalLabels(value:string):string[] {
  return [...new Set(value.split(/[,，\n]/).map(label=>label.trim()).filter(Boolean))];
}
