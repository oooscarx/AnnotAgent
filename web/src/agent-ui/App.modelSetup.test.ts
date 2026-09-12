import { describe, expect, it } from "vitest";
import type { Task } from "./adapter";
import { modelSetupContext } from "./App";
import type { CapabilityReadiness } from "./modelPreparation";

const action = {
  id: "build_and_test_pipeline",
  state: "requires_confirmation",
  method: "GET",
  url: "/api/TEST/sample-preview",
  requires_confirmation: true,
  reason: null,
} as const;

function readiness(
  candidates: CapabilityReadiness["candidates"],
): CapabilityReadiness {
  return {
    contract_version: "mainline-capability-v1",
    project_id: "TEST-project",
    project_owner_id: "TEST-owner",
    conversation_id: "TEST-conversation",
    task_id: "TEST-task",
    task_schema_revision: "schema-1",
    draft: null,
    registry_revision: "registry-sha",
    registry_revision_kind: "snapshot_sha256",
    candidates,
    setup_requests: [],
    visual_readiness_boundary: {
      status: "awaiting_frozen_draft",
      reason: "Visual readiness is evaluated only after the exact Draft is frozen.",
      builder_preview_url: "/api/TEST/builder-preview",
      sample_preview_url: "/api/TEST/sample-preview",
    },
    agent_model_preference: {
      revision: 2,
      model_profile_id: "agent-ready",
    },
    authorization: {
      source: null,
      consent_id: null,
      expires_at: null,
      permission_digest: null,
      allowed_models: [],
      active: false,
      can_resume_without_authorization: false,
      continuation_state: "not_authorized",
      continuation_reason: "no_active_journey_consent",
    },
    budget: null,
    task_cost: {
      scope: "conversation_task_model_calls",
      receipt_count: 0,
      known: true,
      amount: "0",
      currency: null,
      reason: null,
    },
    passive: true,
    setup_recheck_only: true,
    auto_expands_allowed_models: false,
    consistency: "server_composed_versioned_snapshot",
  };
}

function candidate(
  id: string,
  roles: string[],
  capabilities: CapabilityReadiness["candidates"][number]["capabilities"],
  overrides: Partial<CapabilityReadiness["candidates"][number]> = {},
): CapabilityReadiness["candidates"][number] {
  return {
    id,
    candidate_type: "model_profile",
    model_profile_id: id,
    revision: 1,
    digest: `${id}-digest`,
    roles,
    capabilities,
    quality_contracts: [],
    readiness: "ready",
    production_eligible: true,
    test_fixture: false,
    blocker: null,
    project_bindings: [],
    allowed_by_current_scope: false,
    selected_for_next_agent_request: false,
    setup: { kind: "model_profile", api_url: `/api/model-profiles/${id}` },
    ...overrides,
  };
}

function task(value: CapabilityReadiness): Task {
  return {
    id: "TEST-task",
    project: "TEST-project",
    title: "TEST detection",
    phase: "idle",
    revision: "schema-1",
    draft: "",
    items: [],
    queue: [],
    boxes: [],
    image: "TEST-image",
    model: "",
    mainline: {
      contract_version: "mainline-task-v1",
      project_id: "TEST-project",
      project_owner_id: "TEST-owner",
      conversation_id: "TEST-conversation",
      task_id: "TEST-task",
      read_model_revision: "read-1",
      delivery: null,
      schema: null,
      review_summary: {
        selected_images: 0,
        saved_review_receipts: 0,
        current_reviews: 0,
        pending_reviews: 0,
      },
      package: { consents: [], jobs: [] },
      available_actions: [action],
      blockers: [],
      completion: {
        model_request_completed: false,
        processing_completed: false,
        package_ready: false,
        task_completed: false,
      },
      capability_readiness: value,
    },
  };
}

describe("mainline task model setup composition", () => {
  it("does not interrupt the sample action when selected Agent and project-bound visual models are ready", () => {
    const value = readiness([
      candidate("agent-ready", ["agent"], ["text_generation"], {
        selected_for_next_agent_request: true,
      }),
      candidate("vision-ready", ["vision_language"], ["vision_language"], {
        project_bindings: [
          {
            id: "binding",
            project_id: "TEST-project",
            capability: "vision_language",
            role: "primary_inference",
            match_kind: "capability",
            model_profile_id: "vision-ready",
            locked: false,
            created_at: "2026-09-12T00:00:00Z",
          },
        ],
      }),
    ]);
    value.setup_requests = [{
      id: "setup-ready",
      project_id: "TEST-project",
      task_id: "TEST-task",
      task_revision: "schema-1",
      registry_revision: "registry-sha",
      role: "task_planning_and_vision",
      required_capabilities: ["text_generation", "vision_language"],
      compatible_model_ids: ["agent-ready", "vision-ready"],
      status: "ready",
      return_path: "/projects/TEST-project/work?task=TEST-task",
    }];

    expect(
      modelSetupContext(
        task(value),
        new URL("http://annotagent.local/projects/TEST-project/work?task=TEST-task"),
      ),
    ).toBeUndefined();
  });

  it("creates a frozen same-task setup request only for missing capabilities", () => {
    const value = readiness([
      candidate("agent-unselected", ["agent"], ["text_generation"]),
      candidate("vision-unbound", ["vision_language"], ["vision_language"], {
        readiness: "unknown",
      }),
    ]);
    value.setup_requests = [{
      id: "setup-required",
      project_id: "TEST-project",
      task_id: "TEST-task",
      task_revision: "schema-1",
      registry_revision: "registry-sha",
      role: "task_planning_and_vision",
      required_capabilities: ["text_generation", "vision_language"],
      compatible_model_ids: ["agent-unselected", "vision-unbound"],
      status: "required",
      return_path: "/projects/TEST-project/work?task=TEST-task&pane=thread",
    }];

    const context = modelSetupContext(
      task(value),
      new URL("http://annotagent.local/projects/TEST-project/work?task=TEST-task&pane=thread"),
    );

    expect(context).toMatchObject({
      project_id: "TEST-project",
      conversation_id: "TEST-conversation",
      task_id: "TEST-task",
      task_revision: "schema-1",
      registry_revision: "registry-sha",
      setup_status: "required",
      return_to: "/projects/TEST-project/work?task=TEST-task&pane=thread",
      compatible_model_ids: ["agent-unselected", "vision-unbound"],
    });
    expect(context?.requirements.map((item) => item.capability)).toEqual([
      "text_generation",
      "vision_language",
    ]);
    expect(context?.requirements.every((item) => item.target===undefined)).toBe(true);
    expect(context?.allowed_models).toEqual([]);
  });

  it("does not invent setup work outside the server sample action", () => {
    const value = readiness([]);
    const current = task(value);
    current.mainline!.available_actions = [];
    expect(
      modelSetupContext(
        current,
        new URL("http://annotagent.local/projects/TEST-project/work?task=TEST-task"),
      ),
    ).toBeUndefined();
  });
});
