import type { SchemaClarification, Task } from "./adapter";
import { taskIsComplete, type IntakeSlot, type MainlineAction } from "./mainline";

export type CurrentTaskAction =
  | { kind: "prepare_sample" }
  | { kind: "confirm_approval" }
  | { kind: "stop" }
  | { kind: "resume" }
  | { kind: "open_review"; id?: string }
  | { kind: "prepare_processing" }
  | { kind: "prepare_export" }
  | { kind: "download_package"; id: string; url?: string };

export type CurrentTaskPresentation = {
  kind:
    | "needs_information"
    | "ready_to_start"
    | "running"
    | "needs_review"
    | "ready_to_process"
    | "ready_to_deliver"
    | "delivered"
    | "interrupted"
    | "blocked"
    | "idle";
  title: string;
  detail: string;
  missing?: IntakeSlot[];
  primary?: CurrentTaskAction;
  action?: MainlineAction;
  clarification?: SchemaClarification;
};

const missingQuestions: Record<IntakeSlot, string> = {
  dataset_scope: "请上传或选择这次要处理的图片。",
  label_spec: "这些图片里需要标注哪些类别？",
  training_target: "你需要给图片分类、框出目标，还是描出区域？",
};

const action = (task: Task, id: string, state?: MainlineAction["state"]) =>
  task.mainline?.available_actions.find(
    (candidate) => candidate.id === id && (!state || candidate.state === state),
  );

const blockerText = (task: Task) => {
  const blocker = task.mainline?.blockers[0];
  if (typeof blocker === "string") return blocker;
  return blocker?.message;
};

/**
 * The only selector allowed to decide which task-level decision is current.
 * It never advances work: React renders this projection and all execution remains
 * behind explicit adapter commands or the server-owned Journey worker.
 */
export function selectCurrentTaskPresentation(task: Task): CurrentTaskPresentation {
  const view = task.mainline;
  if (!view) {
    return {
      kind: "blocked",
      title: "无法读取当前任务状态",
      detail: "服务器没有提供版本化任务状态；不会猜测或自动执行下一步。",
    };
  }

  if (taskIsComplete(view)) {
    const packageId =
      view.completion.package_id ||
      view.package.jobs.find((job) => job.phase === "ready")?.id;
    return {
      kind: "delivered",
      title: "训练数据包已准备好",
      detail: "标注范围和审核快照已经冻结，可以下载真实交付文件。",
      primary: packageId
        ? {
            kind: "download_package",
            id: packageId,
            url: view.completion.download_url,
          }
        : undefined,
    };
  }

  if (task.clarification) {
    return {
      kind: "needs_information",
      title: "只需要确认输出类型",
      detail:
        "已记录的图片、标签和授权范围保持不变；这里只补充当前缺失的输出类型。",
      clarification: task.clarification,
    };
  }

  // The server only exposes this action after every formal image review is
  // current. Persistent review-work-item lineage must not reopen the already
  // completed Sample stop or hide the next server-owned delivery decision.
  const packageAction = action(task, "authorize_training_package", "requires_confirmation");
  if (packageAction) {
    return {
      kind: "ready_to_deliver",
      title: "正式审核完成，可以生成训练数据包",
      detail: "打开交付卡后，可授权服务器按当前冻结审核快照生成一次真实 ZIP。",
      primary: { kind: "prepare_export" },
      action: packageAction,
    };
  }

  const activeReviewCount =
    task.sampleResult?.images.length ||
    view.review_summary.current_reviews ||
    (task.human || view.review_work_item_id ? 1 : 0);
  const formalProcessingAction = action(
    task,
    "start_delivery_processing",
    "requires_confirmation",
  );
  if (
    task.phase === "waiting_for_human" ||
    !!task.human ||
    !!view.review_work_item_id ||
    (!!task.sampleResult?.images.length && !formalProcessingAction && !task.processing?.length)
  ) {
    return {
      kind: "needs_review",
      title: task.humanQuestion || "这一张的目标是否完整、边界是否合适？",
      detail: `${Math.max(activeReviewCount, 1)} 个结果需要人工判断；修改只保存到当前候选或正式审核范围。`,
      primary: { kind: "open_review", id: view.review_work_item_id },
    };
  }

  const runningStep = view.steps?.find((step) => step.status === "running");
  const automaticContinuation = action(
    task,
    "inspect_automatic_sample_progress",
    "available",
  );
  const dispatchStatus = (
    automaticContinuation?.scope as { dispatch_status?: string } | undefined
  )?.dispatch_status;
  const active =
    task.phase === "planning" ||
    task.phase === "running" ||
    task.phase === "stopping" ||
    !!view.active_operation_ids?.length ||
    !!runningStep ||
    !!automaticContinuation;
  if (active) {
    return {
      kind: "running",
      title:
        task.phase === "stopping"
          ? "正在停止当前操作"
          : dispatchStatus === "queued"
            ? "任务已排队，等待服务器继续"
            : runningStep?.title || "AnnotAgent 正在处理这个任务",
      detail:
        runningStep?.detail ||
        "任务由服务器继续推进；关闭页面不会取消，重新打开可恢复到同一等待点。",
      primary:
        task.phase !== "stopping" && task.actions?.stop?.available
          ? { kind: "stop" }
          : undefined,
    };
  }

  if (task.phase === "interrupted") {
    return {
      kind: "interrupted",
      title: "任务已停止",
      detail:
        task.actions?.resume?.reason ||
        "已有结果保留；只有服务器明确允许时才可从保存点继续。",
      primary: task.actions?.resume?.available ? { kind: "resume" } : undefined,
    };
  }

  if (task.phase === "outcome_unknown") {
    return {
      kind: "blocked",
      title: "远端结果未知",
      detail: "不能直接重试收费请求。请在执行详情中核实服务端回执。",
    };
  }

  if (task.phase === "failed") {
    return {
      kind: "blocked",
      title: "任务没有完成",
      detail:
        task.error ||
        "失败原因已保留；不会自动重试收费请求或用预置结果替代。",
    };
  }

  if (task.approval) {
    const processing = task.approval.title.includes("处理");
    return {
      kind: processing ? "ready_to_process" : "ready_to_start",
      title: processing
        ? "确认剩余图片的处理范围"
        : "确认这次样例范围",
      detail: "这是当前任务唯一待确认的范围；详细模型、图片和预算保持可查看。",
      primary: { kind: "confirm_approval" },
    };
  }

  const buildAndSample = action(
    task,
    "build_and_test_pipeline",
    "requires_confirmation",
  );
  const missing = view.intake?.missing_slots || [];
  const hasFrozenImages = !missing.includes("dataset_scope");
  if (buildAndSample && hasFrozenImages) {
    return {
      kind: "ready_to_start",
      title: "图片和要求已记录，可以准备样例",
      detail:
        "确认当前图片、模型和预算范围后，服务器会连续准备规范、生成方案并运行最多 3 张样例。",
      primary: { kind: "prepare_sample" },
      action: buildAndSample,
    };
  }

  if (missing.length) {
    return {
      kind: "needs_information",
      title: missingQuestions[missing[0]],
      detail:
        missing.length === 1
          ? "其他任务信息已经保留，只需要补充这一项。"
          : `已知信息不会重复询问；当前还缺 ${missing.length} 项。`,
      missing,
    };
  }

  const duplicateSampleApproval = action(
    task,
    "test_pipeline_samples",
    "requires_confirmation",
  );
  if (duplicateSampleApproval) {
    return {
      kind: "blocked",
      title: "样例自动接续没有完成",
      detail:
        "同一 Builder 与样例授权不应再次要求手动启动。请核实授权是否失效或范围是否变化。",
      action: duplicateSampleApproval,
    };
  }

  const processing = formalProcessingAction;
  if (processing) {
    return {
      kind: "ready_to_process",
      title: "样例已经确认，可以处理剩余图片",
      detail:
        "下一次批准只覆盖冻结的剩余图片、模型与预算；不会自动接受正式标注。",
      primary: { kind: "prepare_processing" },
      action: processing,
    };
  }

  const exportAction = action(task, "export", "requires_confirmation");
  if (exportAction) {
    return {
      kind: "ready_to_deliver",
      title: "审核完成，可以生成训练数据包",
      detail: "数据包由服务器根据已确认的正式标注快照生成。",
      primary: { kind: "prepare_export" },
      action: exportAction,
    };
  }

  const failedAction = view.available_actions.find(
    (candidate) => candidate.state === "blocked" && candidate.failure,
  );
  if (failedAction || view.blockers.length) {
    return {
      kind: "blocked",
      title: "当前任务需要处理一个阻塞",
      detail:
        failedAction?.failure?.message ||
        failedAction?.reason ||
        blockerText(task) ||
        "请查看执行详情。",
      action: failedAction,
    };
  }

  return {
    kind: "idle",
    title: "任务状态已保存",
    detail: "当前没有需要你执行的技术步骤；等待服务器状态更新或继续说明需求。",
  };
}
