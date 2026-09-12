import { describe, expect, it } from "vitest";
import type { Task } from "./adapter";
import type { MainlineAction, MainlineResultDiagnostic, MainlineTaskView } from "./mainline";
import { currentResultDiagnostic, selectCurrentTaskPresentation } from "./currentTaskPresentation";

const action = (
  id: string,
  state: MainlineAction["state"],
): MainlineAction => ({
  id,
  state,
  method: state === "requires_confirmation" ? "GET" : "POST",
  url: `/api/TEST/${id}`,
  execution_method: state === "requires_confirmation" ? "POST" : undefined,
  execution_url:
    state === "requires_confirmation" ? `/api/TEST/${id}/execution` : undefined,
  requires_confirmation: state === "requires_confirmation",
  reason: null,
});

const view = (overrides: Partial<MainlineTaskView> = {}): MainlineTaskView => ({
  contract_version: "mainline-task-v1",
  project_id: "TEST-project",
  project_owner_id: "TEST-owner",
  conversation_id: "TEST-conversation",
  task_id: "TEST-task",
  read_model_revision: "read-1",
  intake: {
    missing_slots: [],
    dataset_scope: [{ image_id: "image-1" }],
    label_rules: [{ display_name: "cup" }],
    training_target: { annotation_kind: "bounding_box" },
  },
  delivery: null,
  schema: null,
  review_summary: {
    selected_images: 3,
    saved_review_receipts: 0,
    current_reviews: 0,
    pending_reviews: 0,
  },
  package: { consents: [], jobs: [] },
  available_actions: [],
  blockers: [],
  completion: {
    model_request_completed: false,
    processing_completed: false,
    package_ready: false,
    task_completed: false,
  },
  ...overrides,
});

const task = (mainline: MainlineTaskView, overrides: Partial<Task> = {}): Task => ({
  id: "TEST-task",
  project: "TEST-project",
  conversationId: "TEST-conversation",
  title: "Cup detection",
  phase: "idle",
  revision: "schema-1",
  items: [],
  queue: [],
  draft: "",
  model: "",
  boxes: [],
  image: "image-1",
  mainline,
  ...overrides,
});

const diagnostic=(code:MainlineResultDiagnostic["code"],kind:MainlineResultDiagnostic["source"]["kind"]):MainlineResultDiagnostic=>({
  code,category:"TEST",state:code==="legal_empty_detection"?"completed":"blocked",
  source:{kind,id:kind==="sample_test"?"sample-1":kind==="model_call"?"call-1":"setup-1",...(kind==="sample_test"?{image_index:0}:{})},
  automatic_retry:false,preserves_existing_results:true,
  safe_action:{id:"inspect",method:"GET",url:"/api/TEST/inspect"},
});

describe("single current-task presentation", () => {
  it("asks only for the missing task information", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          intake: {
            missing_slots: ["training_target"],
            dataset_scope: [{ image_id: "image-1" }],
            label_rules: [{ display_name: "cup" }],
            training_target: null,
          },
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_information",
      missing: ["training_target"],
    });
    expect(result.primary).toBeUndefined();
  });

  it("shows one server-owned output choice without reopening the full task form", () => {
    const clarification = {
      callId: "call-1",
      question: "需要框出目标、描出轮廓，还是做整图分类？",
      expectedSchemaRevision: "schema-empty",
      journeyConsentId: "journey-1",
      answerUrl: "/api/TEST/clarification/answer",
      choices: [
        { value: "bounding_box" as const, label: "框住目标", supported: true },
        { value: "segmentation" as const, label: "描出轮廓", supported: false, unsupportedReason: "当前交付路径尚未实现。" },
      ],
    };
    const result = selectCurrentTaskPresentation(task(view(), { clarification }));

    expect(result).toMatchObject({
      kind: "needs_information",
      title: "只需要确认输出类型",
      clarification,
    });
    expect(result.primary).toBeUndefined();
    expect(result.missing).toBeUndefined();
  });

  it("presents one current scope approval for builder plus sample", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          available_actions: [
            action("build_and_test_pipeline", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "ready_to_start",
      primary: { kind: "prepare_sample" },
    });
  });

  it("does not mistake future whole-image review debt for a current review item", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          review_summary: {
            selected_images: 6,
            saved_review_receipts: 0,
            current_reviews: 0,
            pending_reviews: 6,
          },
          available_actions: [
            action("build_and_test_pipeline", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "ready_to_start",
      primary: { kind: "prepare_sample" },
    });
  });

  it("reports the current sample review scope instead of future whole-image debt", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          review_summary: {
            selected_images: 6,
            saved_review_receipts: 0,
            current_reviews: 0,
            pending_reviews: 6,
          },
        }),
        {
          sampleResult: {
            project_id: "TEST-project",
            conversation_id: "TEST-conversation",
            task_id: "TEST-task",
            project_schema_revision: "schema-1",
            draft_id: "draft-1",
            draft_revision: 1,
            sample_test_id: "sample-1",
            images: [
              { image_id: "image-1", image_sha256: "one", result_revision: "one", candidates: [], annotations: [] },
              { image_id: "image-2", image_sha256: "two", result_revision: "two", candidates: [], annotations: [] },
              { image_id: "image-3", image_sha256: "three", result_revision: "three", candidates: [], annotations: [] },
            ],
          },
        },
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_review",
      detail: "3 个结果需要人工判断；修改只保存到当前候选或正式审核范围。",
    });
  });

  it("does not reopen completed Sample evidence after the server offers formal processing", () => {
    const sampleResult = {
      project_id: "TEST-project",
      conversation_id: "TEST-conversation",
      task_id: "TEST-task",
      project_schema_revision: "schema-1",
      draft_id: "draft-1",
      draft_revision: 1,
      sample_test_id: "sample-1",
      images: [{ image_id: "image-1", image_sha256: "one", result_revision: "one", candidates: [], annotations: [] }],
    };
    const result = selectCurrentTaskPresentation(task(view({
      available_actions: [action("start_delivery_processing", "requires_confirmation")],
    }), { sampleResult }));
    expect(result).toMatchObject({kind:"ready_to_process",primary:{kind:"prepare_processing"}});
  });

  it("opens the server-owned training package step after formal review", () => {
    const result=selectCurrentTaskPresentation(task(view({
      available_actions:[action("authorize_training_package","requires_confirmation")],
      review_work_item_id:"persistent-review-lineage",
      review_summary:{selected_images:6,saved_review_receipts:6,current_reviews:0,pending_reviews:0},
    }),{
      phase:"waiting_for_human",
      human:{id:"applied-sample-request",image:"image-1",kind:"bounding_box",labels:["cup"],label:"cup",candidate:"candidate-1"},
      processing:[{id:"processing",batch:"batch",status:"awaiting_review",url:"/batch"}],
    }));
    expect(result).toMatchObject({kind:"ready_to_deliver",primary:{kind:"prepare_export"},action:{id:"authorize_training_package"}});
  });

  it.each([
    ["model_weights_missing","capability_setup_request"],
    ["model_capability_unavailable","capability_setup_request"],
    ["provider_request_not_sent","model_call"],
    ["provider_outcome_unknown","model_call"],
    ["model_response_invalid_structure","model_call"],
    ["legal_empty_detection","sample_test"],
    ["candidate_projection_failed","sample_test"],
    ["authorization_expired","call_grant"],
    ["authorization_revoked","call_grant"],
    ["task_call_budget_exhausted","call_grant"],
  ] as const)("presents the current typed diagnostic %s without inventing a retry",(code,sourceKind)=>{
    const resultDiagnostic=diagnostic(code,sourceKind);
    const mainline=view({
      result_diagnostics:[resultDiagnostic],
      ...(sourceKind==="capability_setup_request"?{capability_readiness:{setup_requests:[{id:"setup-1",status:"required"}]}}:{}),
    });
    const overrides:Partial<Task>=sourceKind==="sample_test"
      ? {sample:{id:"sample-1",draft:"draft-1",revision:1},sampleResult:{project_id:"TEST-project",conversation_id:"TEST-conversation",task_id:"TEST-task",project_schema_revision:"schema-1",draft_id:"draft-1",draft_revision:1,sample_test_id:"sample-1",images:[{image_id:"image-1",image_sha256:"hash",result_revision:"result-1",candidates:[],annotations:[]}]}}
      : sourceKind==="model_call"
        ? {phase:code==="provider_outcome_unknown"?"outcome_unknown":"failed",receipts:[{id:"call-1",title:"call",status:code==="provider_outcome_unknown"?"in_doubt":"failed"}]}
        : {};
    const result=selectCurrentTaskPresentation(task(mainline,overrides));
    expect(result).toMatchObject({kind:"blocked",diagnostic:{code,automatic_retry:false,preserves_existing_results:true}});
    expect(result.primary).toBeUndefined();
  });

  it("keeps a task-scoped authorization blocker ahead of broader model setup",()=>{
    const expired=diagnostic("authorization_expired","call_grant");
    const missingWeights=diagnostic("model_weights_missing","capability_setup_request");
    const result=selectCurrentTaskPresentation(task(view({
      result_diagnostics:[expired,missingWeights],
      capability_readiness:{setup_requests:[{id:"setup-1",status:"required"}]},
    })));
    expect(result).toMatchObject({kind:"blocked",diagnostic:{code:"authorization_expired"}});
    expect(result.primary).toBeUndefined();
  });

  it("keeps a current failed model receipt ahead of a broader setup request",()=>{
    const invalid=diagnostic("model_response_invalid_structure","model_call");
    const missingWeights=diagnostic("model_weights_missing","capability_setup_request");
    const result=selectCurrentTaskPresentation(task(view({
      result_diagnostics:[invalid,missingWeights],
      capability_readiness:{setup_requests:[{id:"setup-1",status:"required"}]},
    }),{phase:"idle",receipts:[{id:"call-1",title:"invalid response",status:"failed"}]}));
    expect(result).toMatchObject({kind:"blocked",diagnostic:{code:"model_response_invalid_structure"}});
    expect(result.primary).toBeUndefined();
  });

  it("keeps an older failed call in details after a later usable Sample result",()=>{
    const old=diagnostic("provider_request_not_sent","model_call");
    const current=task(view({result_diagnostics:[old]}),{
      phase:"failed",receipts:[{id:"call-1",title:"old call",status:"failed"}],
      sample:{id:"sample-1",draft:"draft-1",revision:1},
      sampleResult:{project_id:"TEST-project",conversation_id:"TEST-conversation",task_id:"TEST-task",project_schema_revision:"schema-1",draft_id:"draft-1",draft_revision:1,sample_test_id:"sample-1",images:[{image_id:"image-1",image_sha256:"hash",result_revision:"result-1",candidates:[],annotations:[]}]},
    });
    expect(currentResultDiagnostic(current)).toBeUndefined();
  });

  it("lets the server proposal resolve label and target without another form", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          intake: {
            missing_slots: ["label_spec", "training_target"],
            dataset_scope: [{ image_id: "image-1" }],
            label_rules: null,
            training_target: null,
          },
          available_actions: [
            action("build_and_test_pipeline", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "ready_to_start",
      primary: { kind: "prepare_sample" },
    });
  });

  it("still asks for images before accepting a builder and sample proposal", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          intake: {
            missing_slots: ["dataset_scope", "label_spec", "training_target"],
            dataset_scope: null,
            label_rules: null,
            training_target: null,
          },
          available_actions: [
            action("build_and_test_pipeline", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_information",
      missing: ["dataset_scope", "label_spec", "training_target"],
    });
    expect(result.primary).toBeUndefined();
  });

  it("never exposes a second same-scope Sample execution approval", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          available_actions: [
            action("test_pipeline_samples", "requires_confirmation"),
          ],
        }),
      ),
    );

    expect(result.kind).toBe("blocked");
    expect(result.primary).toBeUndefined();
    expect(result.detail).toContain("授权");
  });

  it("uses actual server activity as the running state", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          steps: [
            {
              id: "builder",
              kind: "builder",
              title: "生成标注方法",
              status: "running",
            },
          ],
          active_operation_ids: ["operation-1"],
        }),
        {
          phase: "running",
          actions: { stop: { available: true, reason: "server operation active" } },
        },
      ),
    );

    expect(result).toMatchObject({
      kind: "running",
      title: "生成标注方法",
      primary: { kind: "stop" },
    });
  });

  it("prefers one exact server-issued resume checkpoint over a stale running projection", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          steps: [{id:"processing",kind:"processing",title:"Dataset processing",status:"running"}],
          active_operation_ids:["batch-1"],
        }),
        {
          phase:"running",
          actions:{resume:{available:true,reason:"exact checkpoint"}},
          resumeTargets:[{id:"batch:batch-1",label:"batch",reason:"Resume the frozen checkpoint"}],
        },
      ),
    );

    expect(result).toMatchObject({
      kind:"interrupted",
      primary:{kind:"resume",target:"batch:batch-1"},
    });
  });

  it("shows durable automatic Journey continuation as progress without another action", () => {
    const continuing = action("inspect_automatic_sample_progress", "available");
    continuing.scope = { dispatch_status: "queued" };
    const result = selectCurrentTaskPresentation(
      task(view({ available_actions: [continuing] })),
    );

    expect(result).toMatchObject({
      kind: "running",
      title: "任务已排队，等待服务器继续",
    });
    expect(result.primary).toBeUndefined();
  });

  it("moves directly to the real review decision when results need a human", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          review_summary: {
            selected_images: 3,
            saved_review_receipts: 0,
            current_reviews: 1,
            pending_reviews: 2,
          },
          review_work_item_id: "review-1",
        }),
        {
          phase: "waiting_for_human",
          humanQuestion: "这一张是否标完整？",
        },
      ),
    );

    expect(result).toMatchObject({
      kind: "needs_review",
      title: "这一张是否标完整？",
      primary: { kind: "open_review" },
    });
  });

  it("defines delivery by the reconciled Ready package", () => {
    const result = selectCurrentTaskPresentation(
      task(
        view({
          package: {
            consents: [],
            jobs: [
              {
                id: "package-1",
                phase: "ready",
                intent_revision: 1,
                snapshot_sha256: "snapshot",
              },
            ],
          },
          completion: {
            model_request_completed: true,
            processing_completed: true,
            package_ready: true,
            task_completed: true,
            status: "package_ready",
            package_id: "package-1",
            download_url: "/api/packages/package-1/download",
          },
        }),
      ),
    );

    expect(result).toMatchObject({
      kind: "delivered",
      primary: { kind: "download_package", id: "package-1" },
    });
  });
});
