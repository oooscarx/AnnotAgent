import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type {
  WorkspaceAdapter,
  Phase,
  Task,
  Command,
  Section,
} from "./adapter";
import { Dialog } from "./Dialog";
import { PlanBlock } from "./PlanBlock";
import { SettingsView } from "./Settings";
import { ArtifactPane } from "./ArtifactPane";
export const phaseNames: Record<Phase, string> = {
  idle: "准备任务",
  planning: "正在模拟规划",
  awaiting_approval: "等待批准",
  running: "模拟执行中",
  stopping: "正在请求停止",
  interrupted: "已打断",
  waiting_for_human: "需要人工协助",
  completed: "演示已完成",
  failed: "模拟执行失败",
  cancelled: "已取消",
  outcome_unknown: "执行结果未知",
};
export function command(task: Task): Command {
  return {
    id: crypto.randomUUID(),
    project: task.project,
    task: task.id,
    revision: task.revision,
  };
}
export function AgentPreviewApp({
  adapter,
  preview,
}: {
  adapter: WorkspaceAdapter;
  preview: { scenario: (id: string, phase: Phase) => void; fail: () => void };
}) {
  const state = useSyncExternalStore(adapter.subscribe, adapter.snapshot);
  const [url, setUrl] = useState(() => new URL(location.href));
  const [expanded, setExpanded] = useState(
    state.settings.collapsed ? [] : state.projects.slice(0, 1).map((p) => p.id),
  );
  const [search, setSearch] = useState("");
  const [mobileNav, setMobileNav] = useState(false);
  const [error, setError] = useState("");
  const [mode, setMode] = useState<"plan" | "execute">("plan");
  const [picker, setPicker] = useState(false);
  const [modelSearch, setModelSearch] = useState("");
  const [attachments, setAttachments] = useState<
    { name: string; url: string; task: string }[]
  >([]);
  const [split, setSplit] = useState(54);
  const [reference, setReference] = useState<{
    task: string;
    image: string;
    candidate: string;
    revision: string;
  } | null>(null);
  const [approval, setApproval] = useState<Command | null>(null);
  const [previewTheme, setPreviewTheme] = useState<string>();
  const compose = useRef<HTMLTextAreaElement>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  const pickerButton = useRef<HTMLButtonElement>(null);
  const composing = useRef(false);
  const sendPending = useRef(false);
  const retryCommand = useRef<{ signature: string; command: Command } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const task = state.tasks.find(
    (t) => t.id === (url.searchParams.get("task") || state.tasks[0]?.id),
  );
  const section = url.searchParams.get("settings") as Section | null;
  const pane = url.searchParams.get("pane") === "image";
  const theme = previewTheme || state.settings.theme;
  const en = state.settings.language === "en";
  const text = (zh: string, eng: string) => (en ? eng : zh);
  const urlRef = useRef(url);
  urlRef.current = url;
  const canNavigate = () =>
    window.dispatchEvent(
      new Event("ui-preview:before-navigate", { cancelable: true }),
    );
  useEffect(() => {
    const fn = () => {
      if (canNavigate()) setUrl(new URL(location.href));
      else history.pushState(null, "", urlRef.current);
    };
    window.addEventListener("popstate", fn);
    return () => window.removeEventListener("popstate", fn);
  }, []);
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () =>
      (document.documentElement.dataset.aaTheme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme);
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => {
    if (!picker) return;
    const close = (e: PointerEvent) => {
      if (
        !pickerRef.current?.contains(e.target as Node) &&
        !pickerButton.current?.contains(e.target as Node)
      )
        setPicker(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPicker(false);
        pickerButton.current?.focus();
      }
      if (
        (e.key === "ArrowDown" || e.key === "ArrowUp") &&
        pickerRef.current?.contains(document.activeElement)
      ) {
        const controls = Array.from(
          pickerRef.current.querySelectorAll<HTMLElement>(
            "input,button:not(:disabled)",
          ),
        );
        const index = controls.indexOf(document.activeElement as HTMLElement);
        e.preventDefault();
        controls[
          (index + (e.key === "ArrowDown" ? 1 : -1) + controls.length) %
            controls.length
        ]?.focus();
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", key);
    pickerRef.current?.querySelector("input")?.focus();
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", key);
    };
  }, [picker]);
  const navigate = (
    values: Record<string, string | null>,
    alreadyConfirmed = false,
  ) => {
    if (!alreadyConfirmed && !canNavigate()) return;
    const next = new URL(location.href);
    for (const [k, v] of Object.entries(values))
      v === null ? next.searchParams.delete(k) : next.searchParams.set(k, v);
    history.pushState(null, "", next);
    setUrl(next);
    setMobileNav(false);
    setPicker(false);
  };
  const act = async (fn: () => Promise<unknown>) => {
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String((e as Error).message));
    }
  };
  const send = async () => {
    if (!task || sendPending.current || !task.draft.trim()) return;
    sendPending.current = true;
    setBusy(true);
    const signature = JSON.stringify([
      task.project,
      task.id,
      task.draft,
      mode,
      task.model,
      reference?.task === task.id ? reference : null,
    ]);
    if (retryCommand.current?.signature !== signature)
      retryCommand.current = {
        signature,
        command: {
          ...command(task),
          selection: reference?.task === task.id ? reference : undefined,
        },
      };
    await act(async () => {
      await adapter.sendMessage(
        retryCommand.current!.command,
        task.draft,
        mode,
        task.model,
      );
      retryCommand.current = null;
    });
    sendPending.current = false;
    setBusy(false);
  };
  const currentProject = state.projects.find((p) => p.id === task?.project);
  const active =
    task && ["running", "planning", "stopping"].includes(task.phase);
  return (
    <div
      className="ui-app"
      style={{ fontSize: state.settings.font === "大" ? "17px" : "14px" }}
      data-density={state.settings.density}
    >
      <div className="preview-banner">
        <span>UI 预览 · 演示数据 · 无真实模型调用</span>
        <details>
          <summary>演示场景</summary>
          <div className="scenario-menu">
            {task &&
              Object.entries(phaseNames).map(([phase, name]) => (
                <button
                  key={phase}
                  onClick={() => preview.scenario(task.id, phase as Phase)}
                >
                  {name}
                </button>
              ))}
            <button onClick={preview.fail}>下一次保存失败</button>
          </div>
        </details>
      </div>
      <div className="app-body">
        <aside
          className={`project-sidebar ${mobileNav ? "mobile-open" : ""}`}
          aria-label="项目导航"
        >
          <a
            className="brand"
            href="?task=new"
            onClick={(e) => {
              e.preventDefault();
              navigate({ settings: null, task: state.tasks[0]?.id || null });
            }}
          >
            <img src="/assets/mark-ink.svg" alt="" />
            AnnotAgent
          </a>
          <button
            className="new-task"
            onClick={() =>
              void act(async () => {
                if (!canNavigate()) return;
                const id = await adapter.createTask(
                  task?.project || state.projects[0]?.id,
                );
                navigate({ settings: null, task: id, pane: null }, true);
              })
            }
          >
            ＋ {text("新任务", "New task")}
          </button>
          <input
            aria-label="搜索任务"
            placeholder={text("搜索会话", "Search tasks")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <small>{text("项目", "Projects")}</small>
          <nav>
            {state.projects.map((p) => (
              <div key={p.id}>
                <button
                  className="project-folder"
                  aria-expanded={expanded.includes(p.id)}
                  onClick={() =>
                    setExpanded((x) =>
                      x.includes(p.id)
                        ? x.filter((id) => id !== p.id)
                        : [...x, p.id],
                    )
                  }
                >
                  {expanded.includes(p.id) ? "⌄" : "›"}　▱ {p.title}
                </button>
                {(expanded.includes(p.id) || search) &&
                  state.tasks
                    .filter(
                      (t) => t.project === p.id && t.title.includes(search),
                    )
                    .map((t) => (
                      <button
                        className={`task-link ${!section && t.id === task?.id ? "selected" : ""}`}
                        aria-current={
                          !section && t.id === task?.id ? "page" : undefined
                        }
                        key={t.id}
                        onClick={() =>
                          navigate({ settings: null, task: t.id, pane: null })
                        }
                      >
                        · {t.title}
                      </button>
                    ))}
              </div>
            ))}
          </nav>
          <footer>
            <button onClick={() => navigate({ settings: "general" })}>
              ⚙ {text("设置", "Settings")}
            </button>
            <small>本地 UI Preview 工作区</small>
          </footer>
        </aside>
        <main className="workspace-main">
          <header className="workspace-header">
            <button
              className="mobile-menu"
              aria-label="打开项目导航"
              onClick={() => setMobileNav(!mobileNav)}
            >
              ☰
            </button>
            <div>
              {section ? (
                text("设置", "Settings")
              ) : (
                <>
                  <span>{currentProject?.title || "项目不存在"}</span>
                  <b> / {task?.title || "任务不存在"}</b>
                </>
              )}
            </div>
            <div className="header-actions">
              {!section && task && (
                <>
                  <button
                    onClick={() => navigate({ pane: pane ? null : "image" })}
                  >
                    {pane
                      ? text("收起数据", "Close data")
                      : text("打开数据", "Open data")}
                  </button>
                  <details>
                    <summary aria-label="项目管理菜单">···</summary>
                    <div className="project-menu">
                      <strong>项目管理 · 预览</strong>
                      <p>
                        原应用中的数据、方案、处理记录、审核、导出与回收站保持不变。
                      </p>
                      <p>此隔离界面尚未连接这些真实管理操作。</p>
                    </div>
                  </details>
                </>
              )}
              <span className="preview-chip">UI Preview</span>
            </div>
          </header>
          {error && (
            <div className="error" role="alert">
              {error}
              <button onClick={() => setError("")}>关闭</button>
            </div>
          )}
          {section ? (
            <SettingsView
              key={section}
              adapter={adapter}
              state={state}
              section={section}
              navigate={(s) => navigate({ settings: s })}
              onTheme={setPreviewTheme}
              back={() => navigate({ settings: null })}
              fail={preview.fail}
            />
          ) : !task ? (
            <div className="empty">
              <h1>找不到这个任务</h1>
              <p>没有自动打开其他项目。</p>
              <button onClick={() => navigate({ task: "new" })}>
                返回示例项目
              </button>
            </div>
          ) : (
            <div
              className={`work-columns ${pane ? "with-data" : ""}`}
              style={{
                gridTemplateColumns: pane
                  ? `${split}% 6px minmax(0,1fr)`
                  : undefined,
              }}
            >
              <section className="conversation">
                <div
                  className={`thread-scroll ${!task.items.length ? "is-empty" : ""}`}
                >
                  <div className="thread">
                    {!task.items.length ? (
                      <div className="welcome">
                        <h1>
                          {text(
                            "你想标注什么？",
                            "What would you like to annotate?",
                          )}
                        </h1>
                        <p>
                          {text(
                            "添加图片，描述目标。先一起确定方法，再开始处理。",
                            "Add images and describe your goal. Agree on a plan before processing.",
                          )}
                        </p>
                      </div>
                    ) : (
                      <>
                        {task.items.map((item) => (
                          <article
                            key={item.id}
                            className={
                              item.role === "user"
                                ? "user-message"
                                : "assistant-message"
                            }
                          >
                            {item.role === "assistant" && (
                              <strong>⌁ AnnotAgent</strong>
                            )}
                            <p>{item.text}</p>
                            {item.reference && (
                              <small>
                                引用：示意图片 {item.reference.image} ·{" "}
                                {item.reference.candidate}
                              </small>
                            )}
                            {item.role === "user" && (
                              <small>
                                演示输入 ·{" "}
                                {state.models.find((m) => m.id === item.model)
                                  ?.name || "示意图片"}
                              </small>
                            )}
                          </article>
                        ))}
                        <div className="operation" aria-live="polite">
                          {phaseNames[task.phase]}
                          {task.operationModel && (
                            <small>
                              本次模型：
                              {
                                state.models.find(
                                  (m) => m.id === task.operationModel,
                                )?.name
                              }
                            </small>
                          )}
                        </div>
                        {task.plan && (
                          <PlanBlock
                            plan={task.plan}
                            expanded={task.phase === "awaiting_approval"}
                          />
                        )}
                        {task.phase === "awaiting_approval" && (
                          <button
                            className="primary"
                            onClick={() => setApproval(command(task))}
                          >
                            批准并试跑 3 张
                          </button>
                        )}
                        {task.phase === "interrupted" && (
                          <div className="notice">
                            <strong>已在模拟安全边界停止</strong>
                            <p>保留已有结果；继续不会重新提交已保存的修正。</p>
                            <button
                              className="primary"
                              onClick={() =>
                                void act(() =>
                                  adapter.resumeOperation(command(task)),
                                )
                              }
                            >
                              继续任务
                            </button>
                          </div>
                        )}
                        {task.phase === "waiting_for_human" && (
                          <div className="notice">
                            <strong>请确认杯柄是否包含在框内</strong>
                            <p>
                              打开图片，拖动边界或在标注列表里精确修改。只保存当前演示候选。
                            </p>
                            <button
                              onClick={() =>
                                navigate({ pane: "image", image: "1" })
                              }
                            >
                              定位需要修正的目标 →
                            </button>
                          </div>
                        )}
                        {task.phase === "outcome_unknown" && (
                          <div className="error">
                            远端结果未知。不能直接重试收费请求；需要 Adapter
                            核实状态。此处不模拟成功。
                          </div>
                        )}
                        {task.phase === "failed" && (
                          <div className="error">
                            演示失败：输入和计划保留。可以修改需求后重新发送。
                          </div>
                        )}
                        {task.phase === "completed" && (
                          <p>
                            修正已保存在 UI
                            Preview。没有创建正式标注或真实导出文件。
                          </p>
                        )}
                        {task.phase === "running" && (
                          <details>
                            <summary>查看模拟执行记录</summary>
                            <p>
                              已读取示意图片；候选来自 Fixture，不代表推理输出。
                            </p>
                            <p>
                              队列不会自动派发；本阶段只演示排队与控制状态。
                            </p>
                          </details>
                        )}
                      </>
                    )}
                  </div>
                </div>
                <div className="composer-region">
                  {task.queue.length > 0 && (
                    <details className="queue">
                      <summary>
                        {task.queue.length} 条排队输入 · 模拟，未自动执行
                      </summary>
                      {task.queue.map((q, i) => (
                        <p key={i}>{q}</p>
                      ))}
                    </details>
                  )}
                  <form
                    className="composer"
                    onSubmit={(e) => {
                      e.preventDefault();
                      void send();
                    }}
                  >
                    {reference?.task === task.id && (
                      <div className="reference-chip">
                        引用：示意图片 {reference.image} · {reference.candidate}
                        <button
                          type="button"
                          aria-label="移除对象引用"
                          onClick={() => setReference(null)}
                        >
                          ×
                        </button>
                      </div>
                    )}
                    {attachments.some((a) => a.task === task.id) && (
                      <div className="attachments">
                        {attachments
                          .filter((a) => a.task === task.id)
                          .map((a) => (
                            <span key={a.url}>
                              <img src={a.url} alt="本地预览" />
                              {a.name}
                              <button
                                type="button"
                                aria-label={`移除 ${a.name}`}
                                onClick={() => {
                                  URL.revokeObjectURL(a.url);
                                  setAttachments((xs) =>
                                    xs.filter((x) => x.url !== a.url),
                                  );
                                }}
                              >
                                ×
                              </button>
                            </span>
                          ))}
                        <small>内存预览，未上传；刷新后需重新选择</small>
                      </div>
                    )}
                    <textarea
                      ref={compose}
                      aria-label="给 AnnotAgent 的需求"
                      placeholder={text(
                        "例如：找出杯子和瓶子，用来训练检测模型…",
                        "Find cups and bottles for a detection dataset…",
                      )}
                      value={task.draft}
                      onChange={(e) =>
                        adapter.saveDraft(task.id, e.target.value)
                      }
                      onCompositionStart={() => {
                        composing.current = true;
                      }}
                      onCompositionEnd={() => {
                        composing.current = false;
                      }}
                      onKeyDown={(e) => {
                        if (
                          e.key === "Enter" &&
                          !e.shiftKey &&
                          !e.nativeEvent.isComposing &&
                          !composing.current &&
                          e.keyCode !== 229
                        ) {
                          e.preventDefault();
                          void send();
                        }
                      }}
                    />
                    <div className="composer-tools">
                      <label className="attach-button">
                        ＋ 图片
                        <input
                          type="file"
                          accept="image/*"
                          multiple
                          onChange={(e) =>
                            setAttachments((old) => [
                              ...old,
                              ...Array.from(e.target.files || []).map((f) => ({
                                name: f.name,
                                task: task.id,
                                url: URL.createObjectURL(f),
                              })),
                            ])
                          }
                        />
                      </label>
                      <button
                        type="button"
                        onClick={() =>
                          setMode(mode === "plan" ? "execute" : "plan")
                        }
                      >
                        {mode === "plan" ? "☷ Plan" : "▷ 执行"}
                      </button>
                      <div className="model-anchor">
                        <button
                          type="button"
                          ref={pickerButton}
                          aria-expanded={picker}
                          onClick={() => setPicker(!picker)}
                        >
                          {state.models.find((m) => m.id === task.model)
                            ?.name || "模型已移除"}{" "}
                          · 模型⌄
                        </button>
                        {picker && (
                          <div
                            ref={pickerRef}
                            className="model-picker"
                            role="dialog"
                            aria-label="选择 Agent 模型"
                          >
                            <small>Agent 模型 · 下次请求生效</small>
                            <input
                              aria-label="搜索模型"
                              placeholder="搜索模型或 Provider"
                              value={modelSearch}
                              onChange={(e) => setModelSearch(e.target.value)}
                            />
                            {[
                              ...new Set(state.models.map((m) => m.provider)),
                            ].map((group) => (
                              <section key={group}>
                                <small>{group}</small>
                                {state.models
                                  .filter(
                                    (m) =>
                                      m.provider === group &&
                                      (m.name + m.provider)
                                        .toLowerCase()
                                        .includes(modelSearch.toLowerCase()),
                                  )
                                  .map((m) => (
                                    <button
                                      key={m.id}
                                      disabled={!!m.reason}
                                      onClick={() =>
                                        void act(async () => {
                                          await adapter.selectAgentModel(
                                            command(task),
                                            m.id,
                                          );
                                          setPicker(false);
                                          pickerButton.current?.focus();
                                        })
                                      }
                                      type="button"
                                    >
                                      <span>
                                        {m.name}
                                        {m.id === task.model ? " ✓" : ""}
                                        <small>
                                          {m.reason || "文本 / 工具调用 · 演示"}
                                        </small>
                                      </span>
                                    </button>
                                  ))}
                              </section>
                            ))}
                            <p>
                              不改变在途请求或 Workflow
                              视觉模型，不进行连接探测。
                            </p>
                            <button
                              type="button"
                              onClick={() => navigate({ settings: "agent" })}
                            >
                              管理 Provider 与模型 →
                            </button>
                          </div>
                        )}
                      </div>
                      <span className="grow" />
                      {active && (
                        <button
                          type="button"
                          disabled={task.phase === "stopping"}
                          onClick={() =>
                            void act(() =>
                              adapter.interruptOperation(command(task)),
                            )
                          }
                        >
                          {task.phase === "stopping" ? "正在停止…" : "■ 停止"}
                        </button>
                      )}
                      <button
                        className="primary"
                        disabled={!task.draft.trim() || busy}
                        type="submit"
                      >
                        {active ? "排队" : "↑ 发送"}
                      </button>
                    </div>
                  </form>
                  <small className="composer-hint">
                    {mode === "plan"
                      ? "仅规划；本预览不调用模型、不上传图片。"
                      : "执行前需要批准具体范围；本预览只模拟。"}
                    　↵ 发送 / Shift+Enter 换行
                  </small>
                  {!task.items.length && (
                    <div className="examples">
                      <div>
                        {state.artifacts.map((asset) => (
                          <button
                            key={asset.id}
                            onClick={() =>
                              navigate({
                                pane: "image",
                                image: String(asset.id),
                              })
                            }
                          >
                            <img src={asset.src} alt={asset.name} />
                          </button>
                        ))}
                        <small>
                          3 张原创示意图
                          <br />
                          不含真实模型输出
                        </small>
                      </div>
                      <button
                        onClick={() =>
                          adapter.saveDraft(
                            task.id,
                            "框出杯子和瓶子，不确定的边界交给我确认。",
                          )
                        }
                      >
                        框出杯子和瓶子
                      </button>
                      <button
                        onClick={() =>
                          adapter.saveDraft(task.id, "给图片分类：室内或室外。")
                        }
                      >
                        给图片分类
                      </button>
                    </div>
                  )}
                </div>
              </section>
              {pane && (
                <>
                  <div
                    className="splitter"
                    role="separator"
                    tabIndex={0}
                    aria-label="调整对话与图片宽度"
                    aria-orientation="vertical"
                    aria-valuenow={split}
                    aria-valuemin={35}
                    aria-valuemax={70}
                    onKeyDown={(e) => {
                      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
                        e.preventDefault();
                        setSplit((v) =>
                          Math.max(
                            35,
                            Math.min(70, v + (e.key === "ArrowLeft" ? -2 : 2)),
                          ),
                        );
                      }
                    }}
                    onPointerDown={(e) => {
                      e.currentTarget.setPointerCapture(e.pointerId);
                    }}
                    onPointerMove={(e) => {
                      if (e.buttons === 1) {
                        const r =
                          e.currentTarget.parentElement!.getBoundingClientRect();
                        setSplit(
                          Math.max(
                            35,
                            Math.min(
                              70,
                              ((e.clientX - r.left) / r.width) * 100,
                            ),
                          ),
                        );
                      }
                    }}
                  />
                  <ArtifactPane
                    key={`${task.id}:${url.searchParams.get("image") || "1"}`}
                    task={task}
                    adapter={adapter}
                    assets={state.artifacts}
                    image={Math.max(
                      1,
                      Math.min(3, Number(url.searchParams.get("image")) || 1),
                    )}
                    onImage={(n) => navigate({ image: String(n) })}
                    onReference={(candidate, image) =>
                      setReference({
                        task: task.id,
                        image: String(image),
                        candidate,
                        revision: task.revision,
                      })
                    }
                    close={() => navigate({ pane: null })}
                    onError={setError}
                  />
                </>
              )}
            </div>
          )}
        </main>
      </div>
      {approval && task && (
        <Dialog title="批准当前计划？" onClose={() => setApproval(null)}>
          <p>范围：3 张示意图 · 演示 VLM 与 SAM</p>
          <p>目的地：当前浏览器 Fixture，不外传。</p>
          <p>预算：未知。演示不收费，不写正式标注。</p>
          <small>绑定任务 revision {approval.revision}</small>
          <div className="actions">
            <button autoFocus onClick={() => setApproval(null)}>
              取消
            </button>
            <button
              className="primary"
              onClick={() =>
                void act(async () => {
                  await adapter.approveAction(approval);
                  setApproval(null);
                })
              }
            >
              确认模拟试跑
            </button>
          </div>
        </Dialog>
      )}
    </div>
  );
}
