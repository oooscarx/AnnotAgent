import { useEffect, useMemo, useRef, useState } from "react";
import { BundleInstaller, type BundleInstallerService } from "./BundleInstaller";
import { Disclosure } from "./Disclosure";
import {
  clearSetupContext,
  completeSetupReturn,
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
          <h2>连接此任务需要的模型</h2>
          <p>
            配置或取消后都会回到原任务；不会重建任务、扩大模型范围或自动调用模型。
          </p>
        </div>
        <button disabled={busy} onClick={() => void cancelAndReturn()}>
          取消并返回任务
        </button>
      </header>

      {error && <p role="alert">{error}</p>}
      {!snapshot && !error && <p role="status">读取任务版本与模型 Registry…</p>}

      {snapshot && (
        <>
          {snapshot.context_changes.length > 0 && <div role="alert"><strong>任务上下文已变化</strong>{snapshot.context_changes.map((item) => <p key={item}>{item}</p>)}<p>可以继续配置，但返回后必须重新读取并确认授权。</p></div>}
          <div className="setup-requirements">
            {snapshot.requirements.map(({ requirement, ready_candidate_ids, uncertain_candidate_ids, alternatives }) => {
              const visibleAlternatives = alternatives.slice(0, 3);
              return (
              <article key={requirement.id}>
                <strong>当前缺少模型能力</strong>
                <p>{requirement.purpose}</p>
                <p>
                  {ready_candidate_ids.length
                    ? `${ready_candidate_ids.length} 个已准备候选`
                    : uncertain_candidate_ids.length
                      ? `${uncertain_candidate_ids.length} 个候选尚未验证；这不等于必然失败`
                      : "没有可继续的兼容候选"}
                </p>
                {alternatives.length > 0 ? (
                  <div className="setup-alternatives" aria-label={`${requirement.capability} 可选准备方式`}>
                    <p>选择其中一种即可满足此能力，不需要全部配置。</p>
                    {visibleAlternatives.map((candidate) => (
                      <div className="setup-alternative" key={candidate.id}>
                        <div>
                          <strong>{candidate.display_name}</strong>
                          <p>{targetLabels[candidate.target]}</p>
                          {candidate.reasons.map((reason) => <p key={reason}>{reason}</p>)}
                        </div>
                        <div className="actions">
                          <span data-state={candidate.state}>{stateLabels[candidate.state]}</span>
                          <button
                            title={`服务器设置接口：${candidate.setup_api_url}`}
                            onClick={() => open(candidate.target, candidate.id)}
                          >
                            {candidate.state === "ready" ? "查看配置" : "配置此方案"}
                          </button>
                        </div>
                      </div>
                    ))}
                    {alternatives.length > visibleAlternatives.length && (
                      <p>另有 {alternatives.length - visibleAlternatives.length} 个兼容候选，可在下方详情中查看。</p>
                    )}
                  </div>
                ) : (
                  <div className="actions">
                    {requirement.capability === "text_generation" ? (
                      <button onClick={() => open("agent_model")}>配置 Agent 模型</button>
                    ) : (
                      <>
                        <button onClick={() => open("provider_model")}>配置远程视觉模型</button>
                        <button onClick={() => open("plugin")}>查看 Plugin 与本地模型</button>
                      </>
                    )}
                  </div>
                )}
              </article>
              );
            })}
          </div>

          <Disclosure className="setup-technical-details" title="查看模型、费用与版本详情">
          <div className="setup-visual-boundary">
            <strong>视觉模型将在方案确定后校验</strong>
            <p>
              {snapshot.readiness.visual_readiness_boundary.status === "awaiting_frozen_draft"
                ? "当前还没有冻结的 Draft，不能从“框出目标”直接推断必须使用某一种检测模型。"
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

          <div className="actions">
            <button disabled={busy} onClick={() => setGeneration((value) => value + 1)}>
              重新读取候选
            </button>
            <button className="primary" disabled={busy} onClick={() => void checkAndReturn()}>
              {busy ? "检查能力与原任务范围…" : "完成设置并返回任务"}
            </button>
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
        </>
      )}
    </section>
  );
}
