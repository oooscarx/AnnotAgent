import type { ConversationSchemaDraft } from "./types";
import type { FutureSchemaInput, FutureSchemaState, SavedFutureSchemaInput } from "./conversation-future-schema-api";

export type SchemaFields = { goal: string; kind: "classification" | "bounding_box"; labels: string; rules: string };
export type FutureSchemaProvenance = { proposal_call_id: string; proposal_digest: string; definition: SchemaDefinition };
export type FutureSchemaLocal = { call_id: string; base_schema_id: string; base_schema_revision: number; scope_answer_command_id: string; context_digest: string; fields: SchemaFields; frozen?: FutureSchemaInput; proposal?: FutureSchemaProvenance };
export type SchemaDefinition = ConversationSchemaDraft["definition"];
export type SchemaChange = { field: string; before: string[]; after: string[]; added?: string[]; removed?: string[] };
const lines = (value: string) => value.split("\n").map(item => item.trim()).filter(Boolean);
const object = (value: unknown): value is Record<string, unknown> => Boolean(value && typeof value === "object" && !Array.isArray(value));
const text = (value: unknown): value is string => typeof value === "string";
const nonempty = (value: unknown): value is string => text(value) && value.length > 0;
const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(text);
const canonical = (value: unknown): string => JSON.stringify(value, (_key, item) => object(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);

export function schemaFields(definition: SchemaDefinition): SchemaFields {
  return { goal: definition.goal, kind: definition.task.kind, labels: definition.task.labels.join("\n"), rules: definition.boundary_rules.join("\n") };
}
export function futureSchemaDefinition(base: ConversationSchemaDraft, fields: SchemaFields, proposal?: FutureSchemaProvenance): SchemaDefinition {
  const definition = proposal?.definition ?? base.definition;
  return { ...definition, goal: fields.goal.trim(), boundary_rules: lines(fields.rules), task: { ...definition.task, id: base.definition.task.id, kind: fields.kind, labels: lines(fields.labels) } };
}
export function futureSchemaDiff(before: SchemaDefinition, after: SchemaDefinition): SchemaChange[] {
  const result: SchemaChange[] = [];
  if (before.goal !== after.goal) result.push({ field: "Goal", before: [before.goal], after: [after.goal] });
  if (before.task.kind !== after.task.kind) result.push({ field: "Output type", before: [before.task.kind], after: [after.task.kind] });
  for (const [field, previous, next] of [["Labels", before.task.labels, after.task.labels], ["Boundary rules", before.boundary_rules, after.boundary_rules]] as const) {
    if (canonical(previous) !== canonical(next)) result.push({ field, before: previous, after: next, added: next.filter(value => !previous.includes(value)), removed: previous.filter(value => !next.includes(value)) });
  }
  if (before.task.multi_label !== after.task.multi_label) result.push({ field: "Multiple labels", before: [String(before.task.multi_label)], after: [String(after.task.multi_label)] });
  if (canonical(before.task.attributes) !== canonical(after.task.attributes)) result.push({ field: "Attributes", before: [canonical(before.task.attributes)], after: [canonical(after.task.attributes)] });
  return result;
}
export function makeFutureSchemaLocal(call: string, state: FutureSchemaState): FutureSchemaLocal {
  return { call_id: call, base_schema_id: state.base_schema.id, base_schema_revision: state.base_schema.revision, scope_answer_command_id: state.source.scope_answer_command_id, context_digest: state.source.context_digest, fields: schemaFields(state.base_schema.definition) };
}
export function makeFutureSchemaInput(command: string, state: FutureSchemaState, fields: SchemaFields, proposal?: FutureSchemaProvenance): FutureSchemaInput {
  const definition = futureSchemaDefinition(state.base_schema, fields, proposal);
  const task = definition.task;
  return { command_id: command, expected_scope_answer_command_id: state.source.scope_answer_command_id, expected_context_digest: state.source.context_digest, base_schema_id: state.base_schema.id, base_schema_revision: state.base_schema.revision, goal: definition.goal, decision: { decision: "draft", kind: task.kind, labels: task.labels, multi_label: task.multi_label, attributes: task.attributes, boundary_rules: definition.boundary_rules, rationale: proposal ? "Human-reviewed model-assisted future rule proposal; existing results remain unchanged" : "Human-authored future rule proposal; existing results remain unchanged" }, ...(proposal ? { proposal_call_id: proposal.proposal_call_id, proposal_digest: proposal.proposal_digest } : {}) };
}
export function futureSchemaSourceMatches(local: FutureSchemaLocal, state: FutureSchemaState) {
  return local.base_schema_id === state.base_schema.id && local.base_schema_revision === state.base_schema.revision && local.scope_answer_command_id === state.source.scope_answer_command_id && local.context_digest === state.source.context_digest;
}
export function sameFutureSchemaInput(a: FutureSchemaInput, b: FutureSchemaInput) { return canonical(a) === canonical(b); }
export function sameSavedFutureSchemaInput(request: FutureSchemaInput, saved: SavedFutureSchemaInput, call: string) {
  const task = saved.definition.task, decision = request.decision;
  return saved.command_id === request.command_id && saved.feedback_call_id === call && saved.scope_answer_command_id === request.expected_scope_answer_command_id && saved.context_digest === request.expected_context_digest && saved.base_schema_id === request.base_schema_id && saved.base_schema_revision === request.base_schema_revision && (saved.proposal_call_id ?? undefined) === request.proposal_call_id && (saved.proposal_digest ?? undefined) === request.proposal_digest && saved.definition.goal === request.goal && task.kind === decision.kind && task.multi_label === decision.multi_label && canonical(task.labels) === canonical(decision.labels) && canonical(task.attributes) === canonical(decision.attributes) && canonical(saved.definition.boundary_rules) === canonical(decision.boundary_rules);
}
export function futureSchemaLocalConflicts(local: FutureSchemaLocal | undefined, state: FutureSchemaState | undefined) {
  if (!local || !state?.record) return false;
  // Matching field text is not a receipt for an unsubmitted browser proposal.
  return !local.frozen || !sameSavedFutureSchemaInput(local.frozen, state.record.input, local.call_id);
}
export function mergeFutureSchemaState(previous: FutureSchemaState | undefined, incoming: FutureSchemaState): FutureSchemaState {
  if (!previous?.record) return incoming;
  if (!incoming.record || previous.record.schema_id !== incoming.record.schema_id || canonical(previous.record.input) !== canonical(incoming.record.input)) return previous;
  if (previous.schema && (!incoming.schema || incoming.schema.revision < previous.schema.revision)) return { ...incoming, schema: previous.schema };
  return incoming;
}
export function parseFutureSchemaLocal(raw: string | null, call: string): FutureSchemaLocal | undefined {
  if (!raw) return undefined;
  try {
    const value: unknown = JSON.parse(raw);
    if (!object(value) || value.call_id !== call || !nonempty(value.base_schema_id) || !Number.isInteger(value.base_schema_revision) || Number(value.base_schema_revision) < 1 || !nonempty(value.scope_answer_command_id) || !nonempty(value.context_digest)) return undefined;
    const fields = value.fields;
    if (!object(fields) || !text(fields.goal) || !text(fields.labels) || !text(fields.rules) || !["classification", "bounding_box"].includes(String(fields.kind))) return undefined;
    if (value.proposal !== undefined) {
      const proposal = value.proposal;
      if (!object(proposal) || !nonempty(proposal.proposal_call_id) || !nonempty(proposal.proposal_digest) || !object(proposal.definition) || !nonempty(proposal.definition.goal) || !strings(proposal.definition.boundary_rules) || !object(proposal.definition.task)) return undefined;
      const task = proposal.definition.task;
      if (!nonempty(task.id) || !["classification", "bounding_box"].includes(String(task.kind)) || !strings(task.labels) || typeof task.multi_label !== "boolean" || !object(task.attributes)) return undefined;
    }
    if (value.frozen !== undefined) {
      const input = value.frozen;
      if (!object(input) || !nonempty(input.command_id) || input.base_schema_id !== value.base_schema_id || input.base_schema_revision !== value.base_schema_revision || input.expected_scope_answer_command_id !== value.scope_answer_command_id || input.expected_context_digest !== value.context_digest || input.goal !== fields.goal.trim()) return undefined;
      const decision = input.decision;
      if (!object(decision) || decision.decision !== "draft" || decision.kind !== fields.kind || !strings(decision.labels) || canonical(decision.labels) !== canonical(lines(fields.labels)) || !strings(decision.boundary_rules) || canonical(decision.boundary_rules) !== canonical(lines(fields.rules)) || typeof decision.multi_label !== "boolean" || !object(decision.attributes) || !text(decision.rationale)) return undefined;
      const proposal = value.proposal as FutureSchemaProvenance | undefined;
      if (input.proposal_call_id !== proposal?.proposal_call_id || input.proposal_digest !== proposal?.proposal_digest || (proposal && (decision.multi_label !== proposal.definition.task.multi_label || canonical(decision.attributes) !== canonical(proposal.definition.task.attributes)))) return undefined;
      if (Object.keys(input).some(key => !["command_id", "expected_scope_answer_command_id", "expected_context_digest", "base_schema_id", "base_schema_revision", "goal", "decision", "proposal_call_id", "proposal_digest"].includes(key))) return undefined;
    }
    return value as FutureSchemaLocal;
  } catch { return undefined; }
}
