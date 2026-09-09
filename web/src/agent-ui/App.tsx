import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type {
  WorkspaceAdapter,
  Phase,
  Task,
  Command,
  Section,
} from "./adapter";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";
import { SidebarTitle } from "./SidebarTitle";
import { ProjectManagement } from "./ProjectManagement";
import { TrashManagement } from "./TrashManagement";
import { HistoryManagement } from "./HistoryManagement";
import { ReviewManagement } from "./ReviewManagement";
import { RunDetail } from "./RunDetail";
import { BatchDetail } from "./BatchDetail";
import { WorkflowEditor } from "./WorkflowEditor";
import {WorkflowVersionDetail} from "./WorkflowVersionDetail";
import { ExportManagement } from "./ExportManagement";
import { Icon, BrandMark } from "./Icon";
import { ProjectMenu } from "./ProjectMenu";
import { PlanBlock } from "./PlanBlock";
import { SettingsView } from "./Settings";
import { ArtifactPane } from "./ArtifactPane";
import { ExecutionProgress } from "./ExecutionProgress";
import { routeProject, taskLocation } from "./routes";
import { parseAgentRoute } from "./navigationContract";
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
  preview?: { scenario: (id: string, phase: Phase) => void; fail: () => void };
}) {
  const state = useSyncExternalStore(adapter.subscribe, adapter.snapshot);
  const fixture = adapter.kind === "fixture";
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
  const [approvalBusy, setApprovalBusy] = useState(false);
  const approvalPending = useRef(false);
  const owner = fixture ? null : routeProject(url);
  const selectedId = url.searchParams.get("task");
  const task = (!fixture && !owner && !selectedId) ? undefined : state.tasks.find(t => (!owner || t.project === owner) &&
    (!url.searchParams.get("conversation") || t.conversationId === url.searchParams.get("conversation")) &&
    (selectedId ? t.id === selectedId : owner ? t.id === `new:${owner}` : true));
  useEffect(()=>{
    if(task && !state.settings.collapsed)setExpanded(ids=>ids.includes(task.project)?ids:[...ids,task.project]);
  },[task?.project]);
  const allowed = (action: "send" | "stop" | "resume" | "approve" | "answer") => fixture || task?.actions?.[action]?.available === true;
  useEffect(() => {
    if (task && adapter.loadTask) void adapter.loadTask(task.project, task.id).catch(e => setError(e.message));
  }, [adapter, task?.id, task?.project]);
  useEffect(() => {
    if (!task || !adapter.loadTask || (!approvalBusy && !["planning","running","stopping"].includes(task.phase))) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { await adapter.loadTask!(task.project, task.id); }
      catch (error) { if (!disposed) setError((error as Error).message); }
      if (!disposed) timer = setTimeout(poll, 2000);
    };
    timer = setTimeout(poll, 2000);
    return () => { disposed = true; clearTimeout(timer); };
  }, [adapter,task?.id,task?.phase,approvalBusy]);
  useEffect(() => {
    if (!state.settings.collapsed && state.projects.length) setExpanded(x => x.length ? x : [state.projects[0].id]);
  }, [state.projects.length]);
  const settingsRoute = parseAgentRoute(url);
  const settingsSections: Record<string, Section> = {general:"general",providers:"providers","agent-models":"agent","vision-models":"vision",plugins:"vision",storage:"privacy",privacy:"privacy",usage:"usage"};
  const unknownSettings = !fixture && url.pathname.startsWith("/settings/") && settingsRoute.kind !== "settings";
  const section = (settingsRoute.kind === "settings" ? settingsSections[settingsRoute.page] : url.searchParams.get("settings") || (url.pathname === "/settings" ? "general" : null)) as Section | null;
  const pane = ["image", "artifacts"].includes(url.searchParams.get("pane") || "");
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
    const place=()=>{
      const panel=pickerRef.current,button=pickerButton.current;if(!panel||!button)return;
      const r=button.getBoundingClientRect();
      panel.style.position="fixed";panel.style.right="auto";panel.style.bottom="auto";
      panel.style.width=`${Math.min(320,innerWidth-24)}px`;
      panel.style.left=`${Math.max(12,Math.min(r.left,innerWidth-panel.offsetWidth-12))}px`;
      panel.style.top=`${Math.max(12,Math.min(r.top-panel.offsetHeight-8,innerHeight-panel.offsetHeight-12))}px`;
    };
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
    place();window.addEventListener("resize",place);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", key);
      window.removeEventListener("resize",place);
    };
  }, [picker]);
  const navigate = (
    values: Record<string, string | null>,
    alreadyConfirmed = false,
  ) => {
    if (!alreadyConfirmed && !canNavigate()) return;
    let next = new URL(location.href);
    if (!fixture && values.task) {
      const target = adapter.snapshot().tasks.find(t => t.id === values.task);
      if (target) next = taskLocation(next, target.project);
      next.searchParams.delete("conversation");
      next.searchParams.delete("image");
    } else if (!fixture && values.settings === null && next.pathname.startsWith("/settings")) {
      if(task) next = taskLocation(next, task.project);
      else next = new URL("/projects", location.origin);
    }
    for (const [k, v] of Object.entries(values))
      v === null ? next.searchParams.delete(k) : next.searchParams.set(k, v);
    if (!fixture && values.settings) {
      const paths: Record<string,string> = {general:"general",providers:"providers",agent:"agent-models",vision:"vision-models",privacy:"privacy",usage:"usage"};
      next.pathname = `/settings/${paths[values.settings] || "general"}`;
      next.searchParams.delete("settings");
      for(const key of [...next.searchParams.keys()]) if(!["task","image","pane","conversation"].includes(key)) next.searchParams.delete(key);
    }
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
    if (!task || sendPending.current || !task.draft.trim() || !allowed("send")) return;
    sendPending.current = true;
    const sentFrom = urlRef.current.href;
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
      const resultTask = await adapter.sendMessage(
        retryCommand.current!.command,
        task.draft,
        mode,
        task.model,
      );
      retryCommand.current = null;
      if (typeof resultTask === "string" && urlRef.current.href === sentFrom) navigate({task:resultTask});
    });
    sendPending.current = false;
    setBusy(false);
  };
  const managementPage = settingsRoute.kind === "management" || settingsRoute.kind === "detail" ? settingsRoute : undefined;
  const currentProject = state.projects.find((p) => p.id === (managementPage?.projectId || task?.project));
  const managementTitle = managementPage ? ({data:"图片数据",labels:"标签定义",pipelines:"自动化方案",runs:"处理记录",batches:"批量处理",review:"审核标注",export:"导出",trash:"回收站"})[managementPage.page] : undefined;
  const active =
    task && ["running", "planning", "stopping"].includes(task.phase);
  return (
    <div
      className="ui-app"
      data-adapter={adapter.kind}
      style={{ fontSize: state.settings.font === "大" ? "17px" : "14px" }}
      data-density={state.settings.density}
    >
      {preview && <div className="preview-banner">
        <span>UI 预览 · 演示数据 · 无真实模型调用</span>
        <Disclosure title="演示场景">
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
        </Disclosure>
      </div>}
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
            <BrandMark />
            AnnotAgent
          </a>
          <button
            className="new-task"
            onClick={() =>
              void act(async () => {
                if (!canNavigate()) return;
                if (!fixture && !state.projects.length) { location.assign("/projects/new"); return; }
                const id = await adapter.createTask(
                  task?.project || state.projects[0]?.id,
                );
                navigate({ settings: null, task: id, pane: null }, true);
              })
            }
          >
            <Icon name="plus" />{text("新任务", "New task")}
          </button>
          {!fixture && <a className="new-project" href="/projects/new"><Icon name="plus" />新建项目</a>}
          <label className="task-search"><Icon name="search" />
          <input
            aria-label="搜索任务"
            placeholder={text("搜索会话", "Search tasks")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          </label>
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
                  <span className="project-tree-chevron"><Icon name="chevron-right" size={14} /></span><Icon name="folder" /><SidebarTitle>{p.title}</SidebarTitle>
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
                        <span className="task-dot" aria-hidden="true" /><SidebarTitle>{t.title}</SidebarTitle>
                      </button>
                    ))}
              </div>
            ))}
          </nav>
          <footer>
            <button onClick={() => navigate({ settings: "general" })}>
              <Icon name="settings" />{text("设置", "Settings")}
            </button>
            <small title={fixture ? "Fixture Adapter" : "HttpAdapter · 服务器持久化"}>{fixture ? "本地 UI Preview 工作区" : "本地工作区"}</small>
          </footer>
        </aside>
        <main className="workspace-main">
          <header className="workspace-header">
            <button
              className="mobile-menu"
              aria-label="打开项目导航"
              onClick={() => setMobileNav(!mobileNav)}
            >
              <Icon name="panel" />
            </button>
            <div>
              {section ? (
                text("设置", "Settings")
              ) : (
                <>
                  <span>{settingsRoute.kind === "projects" ? "我的项目" : settingsRoute.kind === "create-project" ? "新建项目" : currentProject?.title || "项目不存在"}</span>
                  {(managementTitle || task) && <b> / {managementTitle || task?.title}</b>}
                </>
              )}
            </div>
            <div className="header-actions">
              {!fixture && active && pane && <button className="mobile-stop" aria-label="停止当前执行" disabled={task.phase==="stopping" || !allowed("stop")} onClick={()=>void act(()=>adapter.interruptOperation(command(task)))}>{task.phase==="stopping"?"停止中…":"停止"}</button>}
              {!section && task && (
                <>
                  <button
                    onClick={() => navigate({ pane: pane ? null : "image" })}
                  >
                    <Icon name="panel" size={16} />
                    {pane
                      ? text("收起数据", "Close data")
                      : text("打开数据", "Open data")}
                  </button>
                  <ProjectMenu>
                      <strong>项目管理{fixture ? " · 预览" : ""}</strong>
                      <p>
                        原应用中的数据、方案、处理记录、审核、导出与回收站保持不变。
                      </p>
                      {fixture ? <p>此隔离界面尚未连接这些真实管理操作。</p> : <><a href={`/projects/${encodeURIComponent(task.project)}/manage/data`}>图片数据</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/labels`}>标签定义</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/review`}>审核标注</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/export`}>导出标注</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/trash`}>回收站</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/pipelines`}>自动化方案</a><a href={`/projects/${encodeURIComponent(task.project)}/manage/runs`}>处理记录</a></>}
                  </ProjectMenu>
                </>
              )}
              <span className="preview-chip" title={state.testOnly ? "隔离 TEST 数据库；真实 HTTP；外部模型是测试后端，不是真实模型准确率验证" : undefined}>{fixture ? "UI Preview" : state.testOnly ? "TEST · HTTP" : "服务器工作区"}</span>
            </div>
          </header>
          {(error || state.error) && (
            <div className="error" role="alert">
              {error || state.error}
              <button onClick={() => setError("")}>关闭</button>
            </div>
          )}
          {!fixture && settingsRoute.kind === "management" && (settingsRoute.page === "pipelines" || settingsRoute.page === "runs") && adapter.historyManagement && state.workspaceId ? <HistoryManagement key={`${settingsRoute.projectId}:${settingsRoute.page}`} projectId={settingsRoute.projectId} workspaceId={state.workspaceId} kind={settingsRoute.page} service={adapter.historyManagement}/> : !fixture && settingsRoute.kind === "detail" && settingsRoute.page === "pipelines" && url.searchParams.has("version") && adapter.workflowVersion ? <WorkflowVersionDetail workspaceId={state.workspaceId} service={adapter.workflowVersion} projectId={settingsRoute.projectId} workflowId={settingsRoute.objectId} version={url.searchParams.get("version")||""}/> : !fixture && settingsRoute.kind === "detail" && settingsRoute.page === "pipelines" && adapter.workflowEditor ? <WorkflowEditor key={`${settingsRoute.projectId}:${settingsRoute.objectId}`} projectId={settingsRoute.projectId} draftId={settingsRoute.objectId} workspaceId={state.workspaceId} service={adapter.workflowEditor}/> : !fixture && settingsRoute.kind === "detail" && settingsRoute.page === "batches" && adapter.batchDetail ? <BatchDetail key={`${settingsRoute.projectId}:${settingsRoute.objectId}`} projectId={settingsRoute.projectId} batchId={settingsRoute.objectId} service={adapter.batchDetail}/> : !fixture && settingsRoute.kind === "detail" && settingsRoute.page === "runs" && adapter.runDetail ? <RunDetail key={`${settingsRoute.projectId}:${settingsRoute.objectId}`} projectId={settingsRoute.projectId} runId={settingsRoute.objectId} service={adapter.runDetail}/> : !fixture && settingsRoute.kind === "management" && settingsRoute.page === "export" && adapter.exportManagement ? <ExportManagement key={settingsRoute.projectId} projectId={settingsRoute.projectId} service={adapter.exportManagement}/> : !fixture && (settingsRoute.kind === "management" || settingsRoute.kind === "detail") && settingsRoute.page === "review" && adapter.reviewManagement && state.workspaceId ? <ReviewManagement key={url.pathname} projectId={settingsRoute.projectId} reviewId={settingsRoute.kind==="detail"?settingsRoute.objectId:undefined} workspaceId={state.workspaceId} service={adapter.reviewManagement}/> : !fixture && settingsRoute.kind === "management" && settingsRoute.page === "trash" && adapter.trashManagement && state.workspaceId ? <TrashManagement key={settingsRoute.projectId} projectId={settingsRoute.projectId} workspaceId={state.workspaceId} service={adapter.trashManagement}/> : !fixture && adapter.projectManagement && (settingsRoute.kind === "create-project" || (settingsRoute.kind === "management" && ["data","labels"].includes(settingsRoute.page))) ? <ProjectManagement key={url.pathname} service={adapter.projectManagement} projectId={settingsRoute.kind==="management"?settingsRoute.projectId:undefined} page={settingsRoute.kind==="create-project"?"create":settingsRoute.page as "data"|"labels"} created={async id=>{await adapter.refresh?.();history.pushState(null,"",`/projects/${encodeURIComponent(id)}/manage/data`);setUrl(new URL(location.href));}}/> : !fixture && settingsRoute.kind === "projects" ? <section className="native-project-manager"><h1>我的项目</h1><p>选择项目继续标注，或创建新的标注项目。</p><a className="primary" href="/projects/new">新建标注项目</a>{state.projects.map(p=><div className="settings-row" key={p.id}><strong>{p.title}</strong><a href={`/projects/${encodeURIComponent(p.id)}/work`}>继续工作</a><a href={`/projects/${encodeURIComponent(p.id)}/manage/data`}>管理数据</a></div>)}</section> : unknownSettings ? <section className="empty"><h1>页面不存在</h1><p>旧设置地址已停用，不会加载旧页面或猜测返回项目。</p><button onClick={()=>navigate({settings:"general"})}>打开设置</button></section> : section ? (
            <SettingsView
              key={section}
              adapter={adapter}
              state={state}
              section={section}
              navigate={(s) => navigate({ settings: s })}
              onTheme={setPreviewTheme}
              back={() => navigate({ settings: null })}
              fail={preview?.fail || (() => {})}
            />
          ) : !task ? (
            <div className="empty">
              <h1>{state.loading ? "正在读取工作区…" : "找不到这个任务"}</h1>
              <p>没有自动打开其他项目，也没有回退到演示数据。</p>
              <button onClick={() => fixture ? navigate({ task: "new" }) : void act(() => adapter.refresh!())}>
                {fixture ? "返回示例项目" : "重新读取"}
              </button>
            </div>
          ) : (
            <div
              className={`work-columns ${pane ? "with-data" : ""}`}
              style={{
                gridTemplateColumns: pane
                  ? `${split}% 1px minmax(0,1fr)`
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
                              <strong className="assistant-author"><BrandMark />AnnotAgent</strong>
                            )}
                            <p>{item.text}</p>
                            {item.reference && (
                              <small>
                                引用：{fixture ? "示意图片" : "图片"} {item.reference.image} ·{" "}
                                {item.reference.candidate}
                              </small>
                            )}
                            {item.role === "user" && (
                              <small>
                                {fixture ? "演示输入" : "已保存输入"} ·{" "}
                                {state.models.find((m) => m.id === item.model)
                                  ?.name || (fixture ? "示意图片" : "模型未记录")}
                              </small>
                            )}
                          </article>
                        ))}
                        <div className="operation" aria-live="polite">
                          {fixture ? phaseNames[task.phase] : ({ planning: "正在规划", running: "执行中", completed: "已完成", failed: "执行失败" } as Partial<Record<Phase, string>>)[task.phase] || phaseNames[task.phase]}
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
                        {!fixture && !!task.receipts?.length && <ExecutionProgress receipts={task.receipts} />}
                        {!fixture && !active && !task.approval && adapter.prepareAction && task.items.length > 0 && <div className="task-next-actions">
                          {(() => {
                            const choices = [{kind:"plan" as const,label:"查看规划授权",icon:"plan" as const},{kind:"sample" as const,label:"构建方案并测试样例…",icon:"image" as const},...(task.sample?[{kind:"process" as const,label:"确认方案并开始处理…",icon:"play" as const}]:[]),{kind:"export" as const,label:"导出…",icon:"download" as const}];
                            const primary = task.sample ? "process" : task.plan ? "sample" : "plan";
                            const action = choices.find(c=>c.kind===primary)!;
                            const render = (c:typeof action,main=false) => <button key={c.kind} className={main ? "primary" : undefined} disabled={busy} onClick={()=>void act(()=>adapter.prepareAction!(command(task),c.kind))}><Icon name={c.icon} size={16} />{c.label}</button>;
                            return <>{render(action,true)}<Disclosure className="secondary-task-actions" title="其他操作"><div>{choices.filter(c=>c!==action).map(c=>render(c))}</div></Disclosure></>;
                          })()}
                        </div>}
                        {!fixture && task.resumeTargets?.map(r=><p key={r.id}>{r.reason}<button onClick={()=>void act(()=>adapter.resumeOperation(command(task),r.id))}>继续 {r.label}</button></p>)}
                        {!fixture && !!task.stopTargets?.length && <div className="notice"><strong>请选择停止哪一项</strong>{task.stopTargets.map(t=><button key={t.id} onClick={()=>void act(()=>adapter.selectStop!(command(task),t.id))}>{t.label}</button>)}</div>}
                        {!fixture && task.processing?.map(p=><p key={p.id}>处理批次 · {p.status} <a href={p.url}>查看本次结果 →</a></p>)}
                        {!fixture && task.exports?.map(e=><p key={e.id}>导出 · {e.status} · {e.detail} {e.url && <a href={e.url} download>下载真实导出文件</a>}</p>)}
                        {!fixture && task.approval && <section className="plan-block"><strong>{task.approval.title}</strong><ul>{task.approval.scope.map((s,i)=><li key={i}>{s}</li>)}</ul><p>费用：{task.approval.budget ?? "未知；可能产生费用"}</p><button className="primary" disabled={approvalBusy} onClick={()=>setApproval(command(task))}>查看并确认授权</button>{approvalBusy && <p role="status">请求已提交，正在读取服务器执行状态；离开不会取消。</p>}</section>}
                        {task.plan && (
                          <PlanBlock
                            plan={task.plan}
                            expanded={task.phase === "awaiting_approval"}
                            fixture={fixture}
                          />
                        )}
                        {!fixture && task.sample?.draft && <a href={`/projects/${encodeURIComponent(task.project)}/manage/pipelines/${encodeURIComponent(task.sample.draft)}`}>编辑此样例的 Workflow 草稿</a>}
                        {fixture && task.phase === "awaiting_approval" && (
                          <button
                            className="primary"
                            disabled={!allowed("approve")}
                            onClick={() => setApproval(command(task))}
                          >
                            {fixture ? "批准并试跑 3 张" : "查看并批准当前操作"}
                          </button>
                        )}
                        {task.phase === "interrupted" && (
                          <div className="notice">
                            <strong>{fixture ? "已在模拟安全边界停止" : "停止回执已确认"}</strong>
                            <p>{fixture ? "保留已有结果；继续不会重新提交已保存的修正。" : task.actions?.resume?.reason}</p>
                            <button
                              className="primary"
                              disabled={!allowed("resume")}
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
                            <strong>{fixture ? "请确认杯柄是否包含在框内" : task.humanQuestion || "需要人工确认；正在读取具体问题"}</strong>
                            <p>
                              打开图片，拖动边界或在标注列表里精确修改。{fixture ? "只保存当前演示候选。" : "修改的作用域以当前人工请求为准。"}
                            </p>
                            <button
                              onClick={() =>
                                navigate({ pane: "image", image: String(task.image) })
                              }
                            >
                              定位需要修正的目标 →
                            </button>
                          </div>
                        )}
                        {task.phase === "outcome_unknown" && (
                          <div className="error">
                            远端结果未知。不能直接重试收费请求；请查看执行记录并核实服务端状态。
                          </div>
                        )}
                        {task.phase === "failed" && (
                          <div className="error">
                            {fixture ? "演示失败：输入和计划保留。可以修改需求后重新发送。" : task.error || "执行失败，请查看已保存的执行记录；不会自动重试收费请求。"}
                          </div>
                        )}
                        {fixture && task.phase === "completed" && (
                          <p>
                            修正已保存在 UI
                            Preview。没有创建正式标注或真实导出文件。
                          </p>
                        )}
                        {fixture && task.phase === "running" && (
                          <Disclosure title="查看模拟执行记录">
                            <p>
                              已读取示意图片；候选来自 Fixture，不代表推理输出。
                            </p>
                            <p>
                              队列不会自动派发；本阶段只演示排队与控制状态。
                            </p>
                          </Disclosure>
                        )}
                      </>
                    )}
                  </div>
                </div>
                <div className="composer-region">
                  {(task.queue.length > 0 || !!task.queueEntries?.length) && (
                    <Disclosure className="queue" title={task.queue.length ? `${task.queue.length} 条排队输入 · ${fixture ? "模拟，" : ""}未自动执行` : `已保存输入历史 · ${task.queueEntries?.length || 0} 条`}>
                      {task.queueEntries ? task.queueEntries.map(q=><div key={q.id}><p>{q.text} · {q.status}</p>{q.canPlan && adapter.prepareQueue && <button type="button" onClick={()=>void act(()=>adapter.prepareQueue!(command(task),q.id))}>查看此输入的规划范围</button>}{q.canCancel && <button type="button" onClick={()=>void act(()=>adapter.cancelQueue!(command(task),q.id))}>取消此输入</button>}</div>) : task.queue.map((q, i) => (
                        <p key={i}>{q}</p>
                      ))}
                    </Disclosure>
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
                        引用：{fixture ? "示意图片" : "图片"} {reference.image} · {reference.candidate}
                        <button
                          type="button"
                          aria-label="移除对象引用"
                          onClick={() => setReference(null)}
                        >
                          <Icon name="close" size={16} />
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
                                <Icon name="close" size={16} />
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
                        <Icon name="plus" size={16} />图片
                        <input
                          type="file"
                          accept="image/*"
                          multiple
                          disabled={busy || (!fixture && !adapter.uploadImages)}
                          onChange={(e) => {
                            const files=Array.from(e.target.files || []);
                            e.target.value="";
                            if(!fixture) {
                              setBusy(true);
                              const uploadedFrom=urlRef.current.href;
                              void act(async()=>{try{await adapter.uploadImages!(command(task),files);if(urlRef.current.href===uploadedFrom)navigate({pane:"image"});}finally{setBusy(false);}});
                              return;
                            }
                            setAttachments((old) => [
                              ...old,
                              ...files.map((f) => ({
                                name: f.name,
                                task: task.id,
                                url: URL.createObjectURL(f),
                              })),
                            ]);
                          }}
                        />
                      </label>
                      <button
                        type="button"
                        onClick={() =>
                          setMode(mode === "plan" ? "execute" : "plan")
                        }
                      >
                        <Icon name={mode === "plan" ? "plan" : "play"} size={16} />{mode === "plan" ? "Plan" : "执行"}
                      </button>
                      <div className="model-anchor">
                        <button
                          type="button"
                          ref={pickerButton}
                          aria-label={`选择模型：${state.models.find(m=>m.id===task.model)?.name || "模型已移除"}`}
                          title="Agent 模型 · 仅下次请求生效"
                          aria-expanded={picker}
                          onClick={() => setPicker(!picker)}
                        >
                          <span className="model-button-name">{state.models.find((m) => m.id === task.model)
                            ?.name || "模型已移除"}</span>
                          <Icon name="chevron-down" size={14} />
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
                                        {m.id === task.model && <Icon name="check" size={16} />}
                                        <small>
                                          {m.reason || `文本 / 工具调用${fixture ? " · 演示" : ""}`}
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
                          disabled={task.phase === "stopping" || !allowed("stop")}
                          onClick={() =>
                            void act(() =>
                              adapter.interruptOperation(command(task)),
                            )
                          }
                        >
                          <Icon name="stop" size={16} />{task.phase === "stopping" ? "正在停止…" : "停止"}
                        </button>
                      )}
                      <button
                        className="primary send-button"
                        aria-label={active ? "排队" : "发送"}
                        title={active ? "排队输入，不改写在途请求" : "发送"}
                        disabled={!task.draft.trim() || busy || !allowed("send")}
                        type="submit"
                      >
                        {active ? "排队" : <Icon name="arrow-up" size={16} />}
                      </button>
                    </div>
                  </form>
                  <small className="composer-hint">
                    {!fixture ? "目标先保存；模型调用、数据外传和执行需单独批准。" : mode === "plan"
                      ? "仅规划；本预览不调用模型、不上传图片。"
                      : "执行前需要批准具体范围；本预览只模拟。"}
                    　↵ 发送 / Shift+Enter 换行
                  </small>
                  {!task.items.length && (
                    <div className="examples">
                      <div>
                        {state.artifacts.slice(0, 3).map((asset) => (
                          <button
                            key={asset.id}
                            onClick={() =>
                              navigate({
                                pane: "image",
                                image: String(asset.id),
                              })
                            }
                          >
                            <img src={asset.src} alt={asset.name} loading="lazy" decoding="async" />
                          </button>
                        ))}
                        <small>
                          {fixture ? "3 张原创示意图" : `${state.artifacts.length} 张项目图片`}
                          <br />
                          {fixture ? "不含真实模型输出" : state.artifacts.length > 3 ? "预览前 3 张 · 打开数据查看全部" : "原始图片"}
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
              {pane && (fixture || task.loaded) && (
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
                    key={`${task.id}:${url.searchParams.get("image") || task.image}:${task.resultRevision || ""}`}
                    task={task}
                    adapter={adapter}
                    assets={state.artifacts.filter(a => !a.project || a.project === task.project)}
                    image={fixture ? Math.max(1, Math.min(3, Number(url.searchParams.get("image")) || 1)) : url.searchParams.get("image") || task.image}
                    onImage={(n) => navigate({ image: String(n) })}
                    onReference={(candidate, image) =>
                      fixture && setReference({
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
        <Dialog title={fixture ? "批准当前计划？" : task.approval?.title || "授权已变化"} onClose={() => {if(!approvalBusy)setApproval(null);}}>
          {fixture ? <><p>范围：3 张示意图 · 演示 VLM 与 SAM</p><p>目的地：当前浏览器 Fixture，不外传。</p><p>预算：未知。演示不收费，不写正式标注。</p></> : <><ul>{task.approval?.scope.map((s,i)=><li key={i}>{s}</li>)}</ul><p>{task.approval?.budget || "费用未知。点击确认表示接受上述明确范围的潜在费用，不授予额外操作权限。"}</p></>}
          <small>绑定任务 revision {approval.revision}</small>
          <div className="actions">
            <button autoFocus disabled={approvalBusy} onClick={() => setApproval(null)}>
              取消
            </button>
            <button
              className="primary"
              disabled={approvalBusy || (!fixture && !task.approval)}
              onClick={() =>
                void act(async () => {
                  if(approvalPending.current)return;
                  approvalPending.current=true;
                  setApprovalBusy(true);
                  const confirmed=approval;
                  setApproval(null);
                  try { await adapter.approveAction(confirmed); } finally { approvalPending.current=false;setApprovalBusy(false); }
                })
              }
            >
              {fixture ? "确认模拟试跑" : approvalBusy ? "提交授权中…" : "接受未知费用并执行此范围"}
            </button>
          </div>
        </Dialog>
      )}
    </div>
  );
}
