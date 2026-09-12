import { useEffect, useMemo, useRef, useState } from "react";
import { BundleInstaller, type BundleInstallerService } from "./BundleInstaller";
import { Disclosure } from "./Disclosure";
import {
  clearSetupContext,
  completeSetupReturn,
  preparationCardState,
  preserveSetupContext,
  setupSettingsPath,
  type ModelPreparationService,
  type PreparationRecheck,
  type PreparationSnapshot,
  type SetupContext,
  type SetupTarget,
} from "./modelPreparation";
import "./setup-request.css";

const targetLabels: Record<SetupTarget, string> = {
  agent_model: "Agent 规划模型",
  provider_model: "视觉 Model Profile",
  plugin: "Plugin 运行时",
  model_instance: "Model Instance",
};

const stateLabels = {
  ready: "已准备",
  uncertain: "尚未验证",
  setup_required: "需要准备",
  blocked: "不可使用",
};

const capabilityLabels: Record<string, string> = {
  text_generation: "规划并生成标注方案",
  vision_language: "理解图片并定位目标",
  image_classification: "图片分类",
  object_detection: "目标框定位",
  open_vocabulary_detection: "开放词汇目标定位",
  phrase_grounding: "文字指代定位",
  semantic_segmentation: "语义区域分割",
  prompted_segmentation: "按提示精修区域",
  instance_segmentation: "实例区域分割",
  keypoint_detection: "关键点定位",
};

export type SetupRequestProps = {
  context: SetupContext;
  service: ModelPreparationService;
  workspaceId?: string;
  bundleInstallerService?: BundleInstallerService;
  onOpenSettings: (path: string, context: SetupContext) => void;
  onReturn: (result: PreparationRecheck) => void;
  onCancel: (context: SetupContext, result?: PreparationRecheck) => void;
};

export function SetupRequest({
  context,
  service,
  workspaceId,
  bundleInstallerService,
  onOpenSettings,
  onReturn,
  onCancel,
}: SetupRequestProps) {
  const [snapshot, setSnapshot] = useState<PreparationSnapshot>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [generation, setGeneration] = useState(0);
  const [installer, setInstaller] = useState<{ id: string; version: string }>();
  const live = useRef(0);
  const pending = useRef(false);
  const actionController = useRef<AbortController | undefined>(undefined);

  useEffect(() => {
    const current = ++live.current;
    const controller = new AbortController();
    setSnapshot(undefined);
    setError("");
    void service
      .inspect(context, controller.signal)
      .then((value) => {
        if (current === live.current) setSnapshot(value);
      })
      .catch((reason) => {
        if (current === live.current && !controller.signal.aborted)
          setError((reason as Error).message);
      });
    return () => {
      controller.abort();
      actionController.current?.abort();
      live.current++;
    };
  }, [context.id, service, generation]);

  const requirementById = useMemo(
    () => new Map(context.requirements.map((item) => [item.id, item])),
    [context.requirements],
  );
  const card = snapshot ? preparationCardState(snapshot) : undefined;

  const open = (target: SetupTarget, candidateId?: string) => {
    try {
      preserveSetupContext(sessionStorage, context);
      onOpenSettings(setupSettingsPath(context, target, candidateId), context);
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const checkAndReturn = async () => {
    if (!snapshot || pending.current) return;
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      const controller = new AbortController();
      actionController.current = controller;
      const current = live.current;
      const result = await service.recheck(snapshot, controller.signal);
      if (controller.signal.aborted || current !== live.current) return;
      // Return immediately after the passive recheck. The task workspace owns
      // the one current approval or real server progress; Setup never dispatches
      // Builder/Sample work and never adds an extra "continue" relay click.
      onReturn(completeSetupReturn(sessionStorage, context, result));
    } catch (reason) {
      setError((reason as Error).message);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };

  const cancelAndReturn = async () => {
    if (pending.current) return;
    if (!snapshot) {
      clearSetupContext(sessionStorage, context.id);
      onCancel(context);
      return;
    }
    pending.current = true;
    setBusy(true);
    const controller = new AbortController();
    actionController.current = controller;
    const current = live.current;
    try {
      const result = await service.recheck(snapshot, controller.signal);
      if (controller.signal.aborted || current !== live.current) return;
      clearSetupContext(sessionStorage, context.id);
      onCancel(context, result);
    } catch (reason) {
      if (!controller.signal.aborted && current === live.current) {
        setError(`${(reason as Error).message}。仍可返回原任务，由任务页重新读取。`);
        clearSetupContext(sessionStorage, context.id);
        onCancel(context);
      }
    } finally {
      pending.current = false;
      if (current === live.current) setBusy(false);
    }
  };

  return (
    <section className="setup-request" aria-label="任务模型准备">
      <header>
        <div>
          <h2>
            {!snapshot
              ? "检查此任务需要的模型"
              : card?.state === "ready"
                ? "所需模型已经可用"
                : card?.state === "stale"
                  ? "任务在配置期间发生了变化"
                  : "完成一次模型准备后继续"}
          </h2>
          <p>配置或取消后都会回到原任务；不会重建任务、扩大模型范围或自动调用模型。</p>
        </div>
        <button disabled={busy} onClick={() => void cancelAndReturn()}>
          取消并返回任务
        </button>
      </header>

      {error && <div role="alert"><p>{error}</p><button type="button" disabled={busy} onClick={() => setGeneration((value) => value + 1)}>重新读取</button></div>}
      {!snapshot && !error && <p role="status">读取任务版本与模型 Registry…</p>}

      {snapshot && card && (
        <>
          {snapshot.context_changes.length > 0 && <div className="setup-context-change" role="alert"><strong>原任务范围已经变化</strong><p>返回后重新检查图片、模型集合、Registry revision 与授权；旧许可不会自动复活。</p></div>}
          <article className="setup-current-blocker" data-state={card.state}>
            <div className="setup-current-heading">
              <div>
                <strong>{card.state === "ready" ? "可以返回原任务继续" : card.state === "uncertain" ? "有兼容候选，但可用性尚未验证" : card.state === "stale" ? "返回原任务后重新核验" : "当前缺少任务所需能力"}</strong>
                <p>{card.state === "ready" ? "已有模型满足当前任务，不需要重新配置或逐节点绑定。" : card.state === "uncertain" ? "Unknown 不等于失败。选择候选只打开既有设置，不会自动探测、安装或收费。" : card.state === "stale" ? "服务端会保留原任务，但不会沿用已经变化的授权。" : "只需完成当前缺失能力；不会把所有模型、Plugin 和权重都列为必选。"}</p>
              </div>
              <span data-state={card.state}>{card.state === "ready" ? "已准备" : card.state === "uncertain" ? "待核实" : card.state === "stale" ? "需复核" : "被阻塞"}</span>
            </div>
            <div className="setup-capability-list">
              {snapshot.requirements
                .filter((item) => card.state === "ready" || item.ready_candidate_ids.length === 0)
                .map(({ requirement, ready_candidate_ids, uncertain_candidate_ids, alternatives }) => {
                  const ready = ready_candidate_ids.length > 0;
                  const visibleAlternatives = alternatives.filter((candidate) => candidate.state !== "blocked").slice(0, 3);
                  return (
                    <section key={requirement.id}>
                      <div className="setup-capability-heading">
                        <div><strong>{capabilityLabels[requirement.capability] ?? requirement.capability}</strong><p>{requirement.purpose}</p></div>
                        <span data-state={ready ? "ready" : uncertain_candidate_ids.length ? "uncertain" : "setup_required"}>{ready ? "已有可用模型" : uncertain_candidate_ids.length ? "可核实候选" : "需要配置"}</span>
                      </div>
                      {!ready && visibleAlternatives.length > 0 && (
                        <div className="setup-alternatives" aria-label={`${requirement.capability} 兼容候选`}>
                          {visibleAlternatives.map((candidate) => (
                            <div className="setup-alternative" key={candidate.id}>
                              <div><strong>{candidate.display_name}</strong><p>{targetLabels[candidate.target]}</p>{candidate.reasons.slice(0, 1).map((reason) => <p key={reason}>{reason}</p>)}</div>
                              <div className="actions"><span data-state={candidate.state}>{stateLabels[candidate.state]}</span><button type="button" onClick={() => open(candidate.target, candidate.id)}>配置此方案</button></div>
                            </div>
                          ))}
                          {alternatives.length > visibleAlternatives.length && <p>另有 {alternatives.length - visibleAlternatives.length} 个兼容候选，可在详情中查看。</p>}
                        </div>
                      )}
                      {!ready && visibleAlternatives.length === 0 && <div className="actions">{requirement.capability === "text_generation" ? <button type="button" onClick={() => open("agent_model")}>连接规划模型</button> : <><button type="button" onClick={() => open("provider_model")}>连接视觉模型</button><button type="button" onClick={() => open("plugin")}>准备本地模型</button></>}</div>}
                    </section>
                  );
                })}
            </div>
            <div className="setup-scope-summary">
              <p><strong>本任务费用：</strong>{snapshot.readiness.task_cost.known ? snapshot.readiness.task_cost.receipt_count === 0 ? "无本次模型请求" : `${snapshot.readiness.task_cost.receipt_count} 次调用 · ${snapshot.readiness.task_cost.amount ?? "金额未知"}${snapshot.readiness.task_cost.currency ? ` ${snapshot.readiness.task_cost.currency}` : ""}` : `未知 · ${snapshot.readiness.task_cost.reason ?? "现有回执不足"}`}</p>
              <p>读取候选不会收费；新增接收方、安装权重或扩大模型集合仍需单独确认。</p>
            </div>
          </article>

          <Disclosure className="setup-technical-details" title="查看模型、费用与版本详情">
          <div className="setup-visual-boundary">
            <strong>视觉模型在方案确定后校验</strong>
            <p>
              {snapshot.readiness.visual_readiness_boundary.status === "awaiting_frozen_draft"
                ? "当前还没有冻结的 Draft，不能从 bbox 目标直接推断必须安装专用检测器或 SAM；合规 VLM 可进入强制人工审核路径。"
                : "已有 Draft；视觉节点、模型绑定和权限由实际 Builder 与 Sample Preview 校验。"}
            </p>
            <p>{snapshot.readiness.visual_readiness_boundary.reason}</p>
          </div>

          <Disclosure title={`Provider Model Profiles · ${snapshot.provider_models.length}`}>
            <p>Agent 规划模型与视觉模型按具体输入、capability 和协议要求分开匹配。</p>
            {snapshot.provider_models.length === 0 && <p>没有兼容 Model Profile。</p>}
            {snapshot.provider_models.map((model) => (
              <article className="setup-candidate" key={`${model.kind}:${model.id}:${model.revision}`}>
                <div>
                  <strong>{model.display_name}</strong>
                  <p>{targetLabels[model.kind]} · {model.provider_name} · {model.remote_model_id}</p>
                  <p>Revision {model.revision} · 能力来源 {model.capability_source}</p>
                  <p>{model.cost.summary}</p>
                  {model.reasons.map((reason) => <p key={reason}>{reason}</p>)}
                </div>
                <span data-state={model.state}>{stateLabels[model.state]}</span>
              </article>
            ))}
          </Disclosure>

          <Disclosure title={`Plugins · ${snapshot.plugins.length}`}>
            <p>Plugin 是执行代码，不等于模型权重或 Ready Model Instance。</p>
            {snapshot.plugins.length === 0 && <p>没有声明所需 capability 的已安装 Plugin。</p>}
            {snapshot.plugins.map((plugin) => (
              <article className="setup-candidate" key={`${plugin.id}:${plugin.version}`}>
                <div>
                  <strong>{plugin.display_name}</strong>
                  <p>{plugin.id}@{plugin.version}</p>
                  {plugin.reasons.map((reason) => <p key={reason}>{reason}</p>)}
                </div>
                <div className="actions">
                  <span data-state={plugin.state}>{stateLabels[plugin.state]}</span>
                  {plugin.state !== "ready" && bundleInstallerService && (
                    <button onClick={() => setInstaller({ id: plugin.id, version: plugin.version })}>
                      查看兼容 Model Bundle…
                    </button>
                  )}
                </div>
              </article>
            ))}
          </Disclosure>

          <Disclosure title={`Model Instances · ${snapshot.model_instances.length}`}>
            <p>只有 Plugin、Bundle、能力契约和实例状态共同就绪，实例才标为已准备。</p>
            {snapshot.model_instances.length === 0 && <p>没有匹配的 Model Instance。</p>}
            {snapshot.model_instances.map((instance) => (
              <article className="setup-candidate" key={instance.id}>
                <div>
                  <strong>{instance.display_name}</strong>
                  <p>{instance.plugin_id}@{instance.plugin_version} · {instance.bundle_id}@{instance.bundle_version}</p>
                  <p>Model Profile {instance.model_profile_id}@{instance.model_profile_revision}</p>
                  {instance.reasons.map((reason) => <p key={reason}>{reason}</p>)}
                </div>
                <span data-state={instance.state}>{stateLabels[instance.state]}</span>
              </article>
            ))}
          </Disclosure>

          <Disclosure title={`可用 Model Bundles · ${snapshot.model_bundles.length}`}>
            {snapshot.model_bundles.length === 0 && <p>目录中没有匹配 capability 的包。</p>}
            {snapshot.model_bundles.map((bundle) => (
              <article className="setup-candidate" key={`${bundle.id}:${bundle.version}`}>
                <div>
                  <strong>{bundle.display_name}</strong>
                  <p>{bundle.id}@{bundle.version} · {bundle.state}</p>
                  <p>兼容 Plugin：{bundle.compatible_plugins.map((item) => `${item.plugin_id}@${item.plugin_version}`).join(" · ") || "未声明"}</p>
                  {bundle.reasons.map((reason) => <p key={reason}>{reason}</p>)}
                </div>
              </article>
            ))}
          </Disclosure>

          {installer && bundleInstallerService && (
            <section className="setup-installer" aria-label="任务所需模型安装">
              <div className="actions">
                <strong>为 {installer.id}@{installer.version} 准备模型</strong>
                <button onClick={() => setInstaller(undefined)}>关闭安装区</button>
              </div>
              <BundleInstaller
                service={bundleInstallerService}
                pluginId={installer.id}
                version={installer.version}
                workspaceId={workspaceId}
              />
            </section>
          )}

          <div className="setup-cost-note">
            <strong>当前 Task 费用</strong>
            <p>
              {snapshot.readiness.task_cost.known
                ? `${snapshot.readiness.task_cost.receipt_count} 条模型调用回执 · ${snapshot.readiness.task_cost.amount ?? "金额未知"}${snapshot.readiness.task_cost.currency ? ` ${snapshot.readiness.task_cost.currency}` : ""}`
                : `费用未知 · ${snapshot.readiness.task_cost.reason ?? "现有回执无法形成可比较金额"}`}
            </p>
            <p>范围仅为当前 conversation task，不是全系统统计。兼容候选的 Registry 单价见各候选；未自动运行收费探测、模型调用或安装。</p>
          </div>

          <Disclosure title="冻结的回流范围">
            <pre>{JSON.stringify({
              project_id: context.project_id,
              conversation_id: context.conversation_id,
              task_id: context.task_id,
              task_revision: snapshot.guard.task_revision,
              draft_id: context.draft_id,
              draft_revision: snapshot.guard.draft_revision,
              allowed_models: context.allowed_models,
              registry_revision: context.registry_revision,
              compatible_model_ids: context.compatible_model_ids,
              authorization: snapshot.authorization,
              context_changes: snapshot.context_changes,
              requirements: context.requirements.map((item) => ({
                id: item.id,
                target: item.target,
                capability: item.capability,
                purpose: requirementById.get(item.id)?.purpose,
              })),
            }, null, 2)}</pre>
          </Disclosure>
          </Disclosure>
          <div className="actions setup-return-actions">
            <button className="primary" disabled={busy} onClick={() => void checkAndReturn()}>
              {busy ? "检查能力与原任务范围…" : card.state === "ready" ? "返回原任务继续" : card.state === "stale" ? "返回原任务重新核验" : "配置完成，重新检查并返回"}
            </button>
          </div>
        </>
      )}
    </section>
  );
}
