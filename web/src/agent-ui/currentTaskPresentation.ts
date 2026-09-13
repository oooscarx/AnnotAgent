import type { SchemaClarification, Task } from "./adapter";
import { taskIsComplete, type IntakeSlot, type MainlineAction, type MainlineResultDiagnostic } from "./mainline";
import { taskModelBindingSummary, type TaskModelBindingSummary } from "./modelBindingSummary";

export type CurrentTaskAction =
  | { kind: "prepare_sample" }
  | { kind: "confirm_approval" }
  | { kind: "stop" }
  | { kind: "resume"; target?: string }
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
  diagnostic?: MainlineResultDiagnostic;
  modelBindings?: TaskModelBindingSummary;
};

const missingQuestions: Record<IntakeSlot, string> = {
  dataset_scope: "当前任务还没有图片范围。请选择项目图片或上传新图片。",
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

const diagnosticCopy:Record<MainlineResultDiagnostic["code"],{title:string;detail:string}>={
  model_weights_missing:{title:"本地模型缺少权重",detail:"当前模型实例没有可验证的权重。请在模型设置中完成安装后返回此任务；现有结果会保留。"},
  model_capability_unavailable:{title:"缺少当前任务需要的模型能力",detail:"没有 Ready 且兼容的模型绑定。查看模型准备要求后再决定是否配置；不会自动安装或改用演示结果。"},
  provider_request_not_sent:{title:"模型请求没有发出",detail:"请求在发送前失败，没有产生远端结果。修复配置后仍需重新确认新的调用范围。"},
  provider_outcome_unknown:{title:"远端结果未知",detail:"服务端无法确认远端是否完成或计费。请先查看原请求回执；当前页面不会直接重试可能收费的调用。"},
  model_response_invalid_structure:{title:"模型响应结构无法使用",detail:"Provider 已返回响应，但结构校验失败，因此没有执行半截 JSON 或工具参数。请先查看校验记录并修正响应模式；再次调用需要新的授权。"},
  legal_empty_detection:{title:"这张样例没有检测到候选",detail:"这是一次合法的空检测结果，不等于人工确认的负样本。请查看原图后再决定如何处理。"},
  candidate_projection_failed:{title:"候选无法投影到原图",detail:"模型产生了候选，但其 Artifact 无法安全映射到原图。其他有效结果会保留，不会伪造替代框。"},
  authorization_expired:{title:"当前模型授权已过期",detail:"原授权范围已过有效期，服务端已停止继续调用。只读检查不会续期；再次执行必须重新确认当前范围。"},
  authorization_revoked:{title:"当前模型授权已撤销",detail:"原授权已被明确撤销，服务端不会恢复或继续消费。已有调用回执和结果仍保留。"},
  task_call_budget_exhausted:{title:"当前任务的模型调用额度已用完",detail:"此任务已达到原授权的调用上限。先查看授权与用量账本；只有服务器提供新的精确范围时才能再次确认，重复提交原提示不会重置额度。"},
};

const activeProviderReceipt=(task:Task)=>(task.receipts||[]).find(item=>
  ["reserved","running","queued","pending","cancelling"].includes(item.status)
  && (item.title.includes("模型")||item.stage==="provider_request"),
);

/** Select only a diagnostic tied to the task's current failing source. */
export function currentResultDiagnostic(task:Task):MainlineResultDiagnostic|undefined{
  const diagnostics=task.mainline?.result_diagnostics||[];
  if(!diagnostics.length||taskIsComplete(task.mainline!))return undefined;
  const sampleId=task.sampleResult?.sample_test_id||task.sample?.id;
  if(sampleId){
    const sample=[...diagnostics].reverse().find(item=>item.source.kind==="sample_test"&&item.source.id===sampleId);
    const usableTerminal=task.sampleResult?.images.some(image=>image.annotations.some(annotation=>image.candidates.some(candidate=>candidate.candidate_id===annotation.id)));
    if(sample&&!usableTerminal)return sample;
    // A later usable Sample result makes older call failures historical detail.
    if(task.sampleResult)return undefined;
  }
  const setupRequests=(task.mainline?.capability_readiness as {setup_requests?:{id:string;status:string}[]}|undefined)?.setup_requests||[];
  const requiredIds=new Set(setupRequests.filter(item=>item.status==="required").map(item=>item.id));
  const currentReceipts=new Set((task.receipts||[]).filter(item=>["failed","in_doubt","invalid_result"].includes(item.status)).map(item=>item.id));
  const providerRequestActive=!!activeProviderReceipt(task);
  // A provider outcome that is still unknown is the most important safety
  // boundary: an exhausted grant is a consequence of that physical attempt,
  // not permission to hide it behind a generic allowance message.
  const unknownCall=[...diagnostics].reverse().find(item=>item.code==="provider_outcome_unknown"&&item.source.kind==="model_call"&&currentReceipts.has(item.source.id));
  if(unknownCall)return unknownCall;
  // A task-scoped grant is otherwise the immediate execution boundary. Model
  // setup cannot revive an expired/revoked grant or increase an exhausted call
  // budget.
  const authorization=providerRequestActive?undefined:[...diagnostics].reverse().find(item=>item.source.kind==="call_grant");
  if(authorization)return authorization;
  const modelCall=[...diagnostics].reverse().find(item=>item.source.kind==="model_call"&&currentReceipts.has(item.source.id));
  if(modelCall)return modelCall;
  const capability=[...diagnostics].reverse().find(item=>item.source.kind==="capability_setup_request"&&requiredIds.has(item.source.id));
  if(capability)return capability;
  return undefined;
}

/**
 * The only selector allowed to decide which task-level decision is current.
 * It never advances work: React renders this projection and all execution remains
 * behind explicit adapter commands or the server-owned Journey worker.
 */
export function selectCurrentTaskPresentation(task: Task): CurrentTaskPresentation {
  const selected=(():CurrentTaskPresentation=>{
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

  const deliveryReview = action(task, "review_delivery_images", "available");
  if (deliveryReview) {
    return {
      kind: "needs_review",
      title: "这一张的目标是否完整、边界是否合适？",
      detail: `${Math.max(view.review_summary.pending_reviews || view.review_summary.current_reviews || 1, 1)} 个结果需要人工判断；预置候选不是模型推理结果，也尚未被人工接受。`,
      primary: { kind: "open_review", id: view.review_work_item_id },
      action: deliveryReview,
    };
  }

  const providerRequest=activeProviderReceipt(task);
  if(providerRequest){
    return {
      kind:"running",
      title:providerRequest.status==="cancelling"?"正在请求停止模型调用":"模型请求正在执行",
      detail:"这是服务端记录的活动 Provider 请求。原授权可能已预留调用额度，但在请求结算前不会显示成“额度已用完”。关闭页面不会重新发送请求。",
      primary:providerRequest.status!=="cancelling"&&task.actions?.stop?.available?{kind:"stop"}:undefined,
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
    const previousFailure = currentResultDiagnostic(task);
    return {
      kind: "ready_to_start",
      title: previousFailure
        ? "上一轮没有完成，可以重新准备样例"
        : "图片和要求已记录，可以准备样例",
      detail: previousFailure
        ? "上一轮的额度、费用和失败记录会保留；继续前将展示新的图片、模型和调用范围，不会沿用旧授权自动重试。"
        : "确认当前图片、模型和预算范围后，服务器会连续准备规范、生成方案并运行最多 3 张样例。",
      primary: { kind: "prepare_sample" },
      action: buildAndSample,
    };
  }

  const diagnostic=currentResultDiagnostic(task);
  if(diagnostic){
    const copy=diagnosticCopy[diagnostic.code];
    return {kind:"blocked",title:copy.title,detail:copy.detail,diagnostic};
  }

  const activeReviewCount =
    task.sampleResult?.images.length ||
    view.review_summary.current_reviews ||
    view.review_summary.pending_reviews ||
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
  const resumableTarget =
    task.actions?.resume?.available && task.resumeTargets?.length === 1
      ? task.resumeTargets[0]
      : undefined;
  // A concrete server-issued checkpoint is stronger evidence than a stale
  // running projection. This does not invent recovery: the target is sent
  // back verbatim and the adapter revalidates it before POSTing.
  if (resumableTarget) {
    return {
      kind: "interrupted",
      title: "任务已暂停，可以从保存点继续",
      detail: resumableTarget.reason,
      primary: { kind: "resume", target: resumableTarget.id },
    };
  }
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
      kind: "ready_to_start",
      title: "方案已保存，可以继续测试样例",
      detail:
        "继续只执行服务器冻结 Draft 的样例阶段，不会重新提交 Schema、重建方案、发布版本或启动全量处理。确认前会再次显示精确图片、视觉模型和剩余调用范围。",
      primary:{kind:"prepare_sample"},
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
  })();
  return {...selected,modelBindings:taskModelBindingSummary(task)};
}
