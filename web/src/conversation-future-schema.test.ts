import { describe, expect, it } from "vitest";
import type { ConversationSchemaDraft } from "./types";
import { futureSchemaDefinition, futureSchemaDiff, makeFutureSchemaLocal, makeFutureSchemaInput, parseFutureSchemaLocal, sameFutureSchemaInput, futureSchemaSourceMatches, mergeFutureSchemaState, sameSavedFutureSchemaInput, futureSchemaLocalConflicts } from "./conversation-future-schema";
import type { FutureSchemaState } from "./conversation-future-schema-api";

const base: ConversationSchemaDraft = { id: "TEST-base", revision: 4, task_id: "TEST-task", source_call_id: null, source_request_id: "TEST-original", base_schema_revision: "TEST-project", definition: { goal: "Find cups", boundary_rules: ["Exclude bottles"], task: { id: "original_label_id", kind: "bounding_box", labels: ["cup"], multi_label: true, attributes: { material: { type: "enum", required: false, values: ["glass", "metal"] } } } } };
const state: FutureSchemaState = { record: null, base_schema: base, schema: null, source: { scope_answer_command_id: "TEST-answer", context_digest: "TEST-digest" }, scope: "future_tasks_only" };

describe("future Schema proposals remain separate and recoverable", () => {
  it("projects only allowed decision fields from an actual wider TaskConfig response", () => {
    const wire = { ...state, base_schema: { ...base, definition: { ...base.definition, task: { ...base.definition.task, required: true, validators: ["TEST-validator"], depends_on: ["TEST-stage"] } } } };
    const input = makeFutureSchemaInput("TEST-command", wire, makeFutureSchemaLocal("TEST-call", wire).fields);
    expect(Object.keys(input.decision).sort()).toEqual(["decision", "kind", "labels", "multi_label", "attributes", "boundary_rules", "rationale"].sort());
  });
  it("retains the full model proposal and provenance when a human edits fields and retries after reload", () => {
    const definition = structuredClone(base.definition);
    definition.task.multi_label = false;
    definition.task.attributes = { color: { type: "enum", required: true, values: ["yellow", "red"] } };
    const proposal = { proposal_call_id: "TEST-proposal-call", proposal_digest: "TEST-proposal-digest", definition };
    const local = { ...makeFutureSchemaLocal("TEST-call", state), proposal };
    local.fields.labels = "yellow cup";
    const frozen = makeFutureSchemaInput("TEST-command", state, local.fields, proposal);
    expect(frozen.decision.attributes).toEqual(definition.task.attributes);
    expect(frozen.decision.multi_label).toBe(false);
    expect(frozen.proposal_call_id).toBe(proposal.proposal_call_id);
    expect(parseFutureSchemaLocal(JSON.stringify({ ...local, frozen }), "TEST-call")).toEqual({ ...local, frozen });
    const stored = { command_id: frozen.command_id, feedback_call_id: "TEST-call", scope_answer_command_id: frozen.expected_scope_answer_command_id, context_digest: frozen.expected_context_digest, base_schema_id: frozen.base_schema_id, base_schema_revision: frozen.base_schema_revision, proposal_call_id: proposal.proposal_call_id, proposal_digest: proposal.proposal_digest, definition: futureSchemaDefinition(base, local.fields, proposal) };
    expect(sameSavedFutureSchemaInput(frozen, stored, "TEST-call")).toBe(true);
    expect(sameSavedFutureSchemaInput(frozen, { ...stored, proposal_digest: "OTHER-model-result" }, "TEST-call")).toBe(false);
    expect(parseFutureSchemaLocal(JSON.stringify({ ...local, frozen: { ...frozen, proposal_call_id: "OTHER-call" } }), "TEST-call")).toBeUndefined();
    expect(base.definition.task.multi_label).toBe(true);
    expect(Object.keys(base.definition.task.attributes)).toEqual(["material"]);
  });
  it("matches the real saved Store input rather than pretending the POST body is echoed", () => {
    const local = makeFutureSchemaLocal("TEST-call", state);
    const request = makeFutureSchemaInput("TEST-command", state, local.fields);
    const stored = { command_id: request.command_id, feedback_call_id: "TEST-call", scope_answer_command_id: request.expected_scope_answer_command_id, context_digest: request.expected_context_digest, base_schema_id: request.base_schema_id, base_schema_revision: request.base_schema_revision, definition: futureSchemaDefinition(base, local.fields) };
    expect(sameSavedFutureSchemaInput(request, stored, "TEST-call")).toBe(true);
    expect(sameSavedFutureSchemaInput(request, stored, "OTHER-call")).toBe(false);
    expect(sameSavedFutureSchemaInput(request, { ...stored, definition: { ...stored.definition, goal: "Another goal" } }, "TEST-call")).toBe(false);
    expect(sameSavedFutureSchemaInput({ ...request, decision: { ...request.decision, rationale: "Different explanatory wording is not saved semantics" } }, stored, "TEST-call")).toBe(true);
    // Identical unsubmitted fields are not an acknowledgement for this browser.
    expect(futureSchemaLocalConflicts(local, { ...state, record: { input: stored, schema_id: "TEST-new", created_at: "TEST-date" }, schema: { ...base, id: "TEST-new" } })).toBe(true);
  });
  it("does not erase a saved proposal or roll its edited revision back with a late GET", () => {
    const input = makeFutureSchemaInput("TEST-command", state, { ...makeFutureSchemaLocal("TEST-call", state).fields, goal: "A new goal" });
    const stored = { command_id: input.command_id, feedback_call_id: "TEST-call", scope_answer_command_id: input.expected_scope_answer_command_id, context_digest: input.expected_context_digest, base_schema_id: input.base_schema_id, base_schema_revision: input.base_schema_revision, definition: { ...base.definition, goal: input.goal } };
    const saved: FutureSchemaState = { ...state, record: { input: stored, schema_id: "TEST-new", created_at: "TEST-date" }, schema: { ...base, id: "TEST-new", revision: 2 } };
    expect(mergeFutureSchemaState(saved, state)).toEqual(saved);
    expect(mergeFutureSchemaState(saved, { ...saved, schema: { ...saved.schema!, revision: 1 } })).toEqual(saved);
    expect(mergeFutureSchemaState(saved, { ...saved, record: { ...saved.record!, schema_id: "TEST-other" }, schema: { ...saved.schema!, id: "TEST-other" } })).toEqual(saved);
    expect(mergeFutureSchemaState(saved, { ...saved, schema: { ...saved.schema!, revision: 3 } }).schema?.revision).toBe(3);
  });
  it("preserves original task identity, multi-label and attributes while editing bounded fields", () => {
    const local = makeFutureSchemaLocal("TEST-call", state);
    local.fields = { ...local.fields, goal: "Classify scenes", kind: "classification", labels: "indoor\noutdoor", rules: "Use entire image" };
    const definition = futureSchemaDefinition(base, local.fields);
    expect(definition.task).toMatchObject({ id: "original_label_id", kind: "classification", multi_label: true, attributes: base.definition.task.attributes });
    expect(definition.task.labels).toEqual(["indoor", "outdoor"]);
    expect(base.definition.goal).toBe("Find cups");
    expect(base.revision).toBe(4);
    const diff = futureSchemaDiff(base.definition, definition);
    expect(diff.map(item => item.field)).toEqual(["Goal", "Output type", "Labels", "Boundary rules"]);
    expect(diff.find(item => item.field === "Labels")?.added).toEqual(["indoor", "outdoor"]);
    expect(diff.find(item => item.field === "Labels")?.removed).toEqual(["cup"]);
  });
  it("shows no invented diff and reports real attribute or label-order changes", () => {
    expect(futureSchemaDiff(base.definition, structuredClone(base.definition))).toEqual([]);
    const changed = structuredClone(base.definition); changed.task.attributes.material.required = true;
    expect(futureSchemaDiff(base.definition, changed).map(item => item.field)).toEqual(["Attributes"]);
    const two = { ...base.definition, task: { ...base.definition.task, labels: ["cup", "bottle"] } };
    const reordered = { ...two, task: { ...two.task, labels: ["bottle", "cup"] } };
    expect(futureSchemaDiff(two, reordered)[0]).toMatchObject({ field: "Labels", added: [], removed: [] });
  });
  it("retains the exact frozen save identity and rejects another source or malformed local data", () => {
    const local = makeFutureSchemaLocal("TEST-call", state);
    local.fields.goal = "Find only yellow cups";
    local.frozen = makeFutureSchemaInput("TEST-command", state, local.fields);
    expect(parseFutureSchemaLocal(JSON.stringify(local), "TEST-call")).toEqual(local);
    expect(parseFutureSchemaLocal(JSON.stringify(local), "OTHER-call")).toBeUndefined();
    expect(parseFutureSchemaLocal(JSON.stringify({ ...local, frozen: { ...local.frozen, base_schema_id: "OTHER-schema" } }), "TEST-call")).toBeUndefined();
    expect(parseFutureSchemaLocal(JSON.stringify({ ...local, fields: { ...local.fields, kind: "execute_code" } }), "TEST-call")).toBeUndefined();
    expect(futureSchemaSourceMatches(local, state)).toBe(true);
    expect(futureSchemaSourceMatches(local, { ...state, base_schema: { ...base, revision: 5 } })).toBe(false);
    expect(sameFutureSchemaInput(local.frozen, structuredClone(local.frozen))).toBe(true);
    expect(sameFutureSchemaInput(local.frozen, { ...local.frozen, command_id: "OTHER-command" })).toBe(false);
  });
});
