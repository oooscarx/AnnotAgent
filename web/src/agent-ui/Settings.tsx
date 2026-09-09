import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import type {
  WorkspaceAdapter,
  Snapshot,
  Section,
  Settings,
  Provider,
} from "./adapter";
import { sections } from "./adapter";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";
import { Disclosure } from "./Disclosure";
import { PluginSettings } from "./PluginSettings";
function Row({
  title,
  help,
  children,
}: {
  title: string;
  help: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-row">
      <div>
        <strong>{title}</strong>
        <p>{help}</p>
      </div>
      <div className="row-control">{children}</div>
    </div>
  );
}
export function SettingsView({
  adapter,
  state,
  section,
  navigate,
  onTheme,
  back,
  fail,
  managementLinks,
}: {
  adapter: WorkspaceAdapter;
  state: Snapshot;
  section: Section;
  navigate: (s: Section) => void;
  onTheme: (s: string | undefined) => void;
  back: () => void;
  fail: () => void;
  managementLinks?: Record<string,string>;
}) {
  const fixture = adapter.kind === "fixture";
  const [draft, setDraft] = useState<Settings>(() =>
    structuredClone(state.settings),
  );
  const [base, setBase] = useState(state.settings);
  const [view, setView] = useState("normal");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState("");
  const [editor, setEditor] = useState<Provider | null>(null);
  const [credential, setCredential] = useState("");
  useEffect(()=>{setCredential("");},[editor?.id]);
  const [confirm, setConfirm] = useState<{
    title: string;
    body: string;
    action: () => Promise<void>;
  } | null>(null);
  const [testResult, setTestResult] = useState<
    "success" | "failed" | "unknown"
  >("success");
  const [installFail, setInstallFail] = useState(false);
  const dirty = JSON.stringify(draft) !== JSON.stringify(base) || !!editor;
  useEffect(() => {
    const guard = (e: Event) => {
      if (dirty && !window.confirm("有未保存的设置，放弃编辑并离开？"))
        e.preventDefault();
    };
    window.addEventListener("ui-preview:before-navigate", guard);
    return () =>
      window.removeEventListener("ui-preview:before-navigate", guard);
  }, [dirty]);
  const config = sections.find((s) => s.id === section);
  const en = draft.language === "en";
  useEffect(() => {
    const prevent = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", prevent);
    return () => window.removeEventListener("beforeunload", prevent);
  }, [dirty]);
  useEffect(() => {
    if (JSON.stringify(draft) === JSON.stringify(base)) {
      setDraft(structuredClone(state.settings));
      setBase(state.settings);
    }
  }, [state.settings]);
  const change = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDraft((d) => ({ ...d, [key]: value }));
    setSaved("");
    if (key === "theme") onTheme(String(value));
  };
  const leave = (fn: () => void) => {
    fn();
  };
  useEffect(() => () => onTheme(undefined), []);
  const save = async (value = draft) => {
    setSaving(true);
    setError("");
    try {
      await adapter.updateSettings(base.revision, value);
      const current = adapter.snapshot().settings;
      setBase(current);
      setDraft(structuredClone(current));
      setEditor(null);
      setSaved(fixture ? "已保存到 UI Preview · 非生产配置" : "已保存；展示偏好保存在此浏览器，业务配置保存在服务器");
      onTheme(undefined);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const cancel = () => {
    setDraft(structuredClone(base));
    setEditor(null);
    setError("");
    setSaved("已取消编辑");
    onTheme(undefined);
  };
  const run = async (fn: () => Promise<void>) => {
    setError("");
    try {
      await fn();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  if (!config) return <p>设置分类不存在。</p>;
  return (
    <div className="settings-layout">
      <nav className="settings-nav" aria-label="设置分类">
        <button onClick={() => leave(back)}>
          ← {en ? "Back to task" : "返回任务"}
        </button>
        {sections.map((s) => (
          <button
            key={s.id}
            className={s.id === section ? "selected" : ""}
            aria-current={s.id === section ? "page" : undefined}
            onClick={() => leave(() => navigate(s.id))}
          >
            {en ? s.en : s.zh}
          </button>
        ))}
      </nav>
      <section className="settings-scroll">
        <div className="settings-content">
          <header>
            <h1>{en ? config.en : config.zh}</h1>
            <p>{!fixture && section === "providers" ? "账户连接与凭证状态；不会在读取时自动探测模型。" : config.description}</p>
          </header>
          {fixture && <Disclosure className="fixture-controls" title="预览状态控制">
            <div className="actions">
              <label>
                页面状态{" "}
                <select
                  aria-label="设置页面状态"
                  value={view}
                  onChange={(e) => setView(e.target.value)}
                >
                  <option value="normal">正常</option>
                  <option value="loading">加载</option>
                  <option value="empty">空数据</option>
                  <option value="error">加载错误</option>
                </select>
              </label>
              <button onClick={fail}>模拟下一次保存失败</button>
            </div>
          </Disclosure>}
          {view === "loading" ? (
            <p role="status">
              正在加载演示设置…{" "}
              <button onClick={() => setView("normal")}>完成模拟加载</button>
            </p>
          ) : view === "error" ? (
            <div className="error" role="alert">
              演示读取失败。没有回退成成功数据。
              <button onClick={() => setView("normal")}>重试模拟读取</button>
            </div>
          ) : view === "empty" ? (
            <div className="empty-state">
              <h2>还没有{config.zh}数据</h2>
              <p>此空状态仅供预览；不会删除已保存的配置。</p>
              <button onClick={() => setView("normal")}>开始配置</button>
            </div>
          ) : (
            <>
              {section === "general" && (
                <>
                  <Row title="外观" help="立即预览；取消后恢复已保存主题。">
                    <select
                      aria-label="外观"
                      value={draft.theme}
                      onChange={(e) =>
                        change("theme", e.target.value as Settings["theme"])
                      }
                    >
                      <option value="light">浅色</option>
                      <option value="dark">深色</option>
                      <option value="system">跟随系统</option>
                    </select>
                  </Row>
                  <Row
                    title="语言"
                    help="中文 / English；预览导航支持两种语言。"
                  >
                    <select
                      aria-label="语言"
                      value={draft.language}
                      onChange={(e) =>
                        change(
                          "language",
                          e.target.value as Settings["language"],
                        )
                      }
                    >
                      <option value="zh">中文</option>
                      <option value="en">English</option>
                    </select>
                  </Row>
                  <Row title="文字大小" help="更大文字不会裁掉必要操作。">
                    <select
                      value={draft.font}
                      aria-label="文字大小"
                      onChange={(e) => change("font", e.target.value)}
                    >
                      <option>标准</option>
                      <option>大</option>
                    </select>
                  </Row>
                  <Row title="界面密度" help="调整列表和控件间距。">
                    <select
                      value={draft.density}
                      aria-label="界面密度"
                      onChange={(e) => change("density", e.target.value)}
                    >
                      <option>舒适</option>
                      <option>紧凑</option>
                    </select>
                  </Row>
                  <Row
                    title="项目分组偏好"
                    help="保存下次打开时收起项目分组的偏好。"
                  >
                    <input
                      type="checkbox"
                      aria-label="默认收起项目"
                      checked={draft.collapsed}
                      onChange={(e) => change("collapsed", e.target.checked)}
                    />
                  </Row>
                  <Row title="键盘快捷键" help="输入法组合期间不会触发发送。">
                    <span>
                      Enter 发送
                      <br />
                      Shift+Enter 换行
                      <br />
                      Escape 关闭选择器
                    </span>
                  </Row>
                </>
              )}
              {section === "providers" && (
                <>
                  {editor ? (
                    <div className="provider-editor">
                      <h2>
                        {editor.id.startsWith("new-") ? "添加账户" : "编辑账户"}
                      </h2>
                      <label>
                        显示名称
                        <input
                          autoFocus
                          value={editor.name}
                          onChange={(e) =>
                            setEditor({ ...editor, name: e.target.value })
                          }
                        />
                      </label>
                      <label>
                        Endpoint
                        <input
                          value={editor.endpoint}
                          onChange={(e) =>
                            setEditor({ ...editor, endpoint: e.target.value })
                          }
                        />
                      </label>
                      {fixture && <label>
                        演示凭证槽位
                        <select
                          value={editor.credential ? "configured" : "empty"}
                          onChange={(e) =>
                            setEditor({
                              ...editor,
                              credential: e.target.value === "configured",
                            })
                          }
                        >
                          <option value="configured">
                            已配置（模拟状态，不含密钥）
                          </option>
                          <option value="empty">未配置</option>
                        </select>
                      </label>}
                      {!fixture && !editor.id.startsWith("new-") && <div>
                        <label>替换 API Key（只写）<input type="password" autoComplete="new-password" value={credential} onChange={e=>setCredential(e.target.value)} /></label>
                        <p>保存到服务器本地工作区文件，重启后保留；不使用系统钥匙串，不写入浏览器存储。</p>
                        <button disabled={!credential.trim() || saving || !adapter.saveCredential} onClick={()=>void run(async()=>{setSaving(true);try{await adapter.saveCredential!(editor.id,credential);setCredential("");setEditor({...editor,credential:true});setSaved("凭证已由服务器保存；不会返回密钥内容");}finally{setSaving(false);}})}>保存新凭证</button>
                        {saved && <p role="status">{saved}</p>}
                      </div>}
                      <p>
                        {fixture ? "不提供 API Key 输入框。" : "只显示凭证是否已配置，不会读取原密钥。"}不要在名称或 Endpoint 中填写密钥。
                      </p>
                      <div className="actions">
                        <button onClick={() => setEditor(null)}>
                          取消账户编辑
                        </button>
                        <button
                          className="primary"
                          disabled={saving}
                          onClick={() => {
                            if (!editor.name.trim()) {
                              setError("请填写显示名称");
                              return;
                            }
                            try {
                              const u = new URL(editor.endpoint);
                              if (
                                !["http:", "https:"].includes(u.protocol) ||
                                u.username ||
                                u.password ||
                                u.search
                              )
                                throw Error();
                            } catch {
                              setError(
                                "请输入不含凭证和查询参数的 HTTP(S) Endpoint",
                              );
                              return;
                            }
                            const providers = [
                              ...draft.providers.filter(
                                (p) => p.id !== editor.id,
                              ),
                              editor,
                            ];
                            void save({ ...draft, providers });
                          }}
                        >
                          保存账户
                        </button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <div className="section-toolbar">
                        <small>{fixture ? "仅示例账户 · 不发起真实连接" : "真实 Registry 账户 · 测试连接为显式操作"}</small>
                        <button
                          onClick={() =>
                            setEditor({
                              id: `new-${crypto.randomUUID()}`,
                              name: "",
                              endpoint: fixture ? "https://example.invalid/v1" : "",
                              credential: false,
                              status: fixture ? "演示 · 未测试" : "未测试",
                            })
                          }
                        >
                          <Icon name="plus" size={16} />添加 Provider
                        </button>
                      </div>
                      {draft.providers.length === 0 ? (
                        <p>尚未添加 Provider。使用上方按钮配置{fixture ? "演示" : "服务器"}账户。</p>
                      ) : (
                        draft.providers.map((p) => (
                          <Row
                            key={p.id}
                            title={p.name}
                            help={`${p.status} · ${p.credential ? "凭证槽位已配置" : "缺少凭证"}`}
                          >
                            <button onClick={() => setEditor({ ...p })}>
                              编辑
                            </button>
                            <button
                              onClick={() =>
                                void run(() =>
                                  adapter.testProvider(p.id, testResult),
                                )
                              }
                            >
                              测试连接
                            </button>
                            <button
                              onClick={() =>
                                setConfirm({
                                  title: fixture ? "删除演示账户？" : "删除这个 Provider？",
                                  body: fixture ? "可能影响引用此账户的规划与视觉模型。只删除预览账户，不触碰真实凭证。" : "此操作删除服务器上的账户配置；引用保护以服务器校验为准。不会删除项目图片。",
                                  action: async () => {
                                    await save({
                                      ...draft,
                                      providers: draft.providers.filter(
                                        (x) => x.id !== p.id,
                                      ),
                                    });
                                  },
                                })
                              }
                            >
                              删除
                            </button>
                          </Row>
                        ))
                      )}
                      {fixture && <label>
                        模拟连接结果{" "}
                        <select
                          aria-label="模拟连接结果"
                          value={testResult}
                          onChange={(e) =>
                            setTestResult(e.target.value as typeof testResult)
                          }
                        >
                          <option value="success">成功</option>
                          <option value="failed">失败</option>
                          <option value="unknown">未知</option>
                        </select>
                      </label>}
                    </>
                  )}
                </>
              )}
              {section === "agent" && (
                <>
                  <Row
                    title="默认规划模型"
                    help="只影响新任务；现有任务在 Composer 里单独选择。"
                  >
                    <select
                      aria-label="默认规划模型"
                      value={draft.defaultModel}
                      onChange={(e) => change("defaultModel", e.target.value)}
                    >
                      {state.models.map((m) => (
                        <option key={m.id} value={m.id} disabled={!!m.reason}>
                          {m.name}
                          {m.reason ? ` · ${m.reason}` : ""}
                        </option>
                      ))}
                    </select>
                  </Row>
                  {[...new Set(state.models.map((m) => m.provider))].map(
                    (provider) => (
                      <section key={provider}>
                        <h2>{provider}</h2>
                        {state.models
                          .filter((m) => m.provider === provider)
                          .map((m) => (
                            <Row
                              key={m.id}
                              title={m.name}
                              help={
                                m.reason || `文本生成 · 工具调用${fixture ? " · 示例能力声明" : " · Registry 能力声明"}`
                              }
                            >
                              <button
                                disabled={!!m.reason}
                                aria-pressed={draft.defaultModel === m.id}
                                onClick={() => change("defaultModel", m.id)}
                              >
                                {draft.defaultModel === m.id
                                  ? <><Icon name="check" size={16} />当前默认</>
                                  : "设为默认"}
                              </button>
                            </Row>
                          ))}
                      </section>
                    ),
                  )}
                  <p>没有执行真实探测。视觉工作流绑定在下一页单独管理。</p>
                  {managementLinks?.models && <a href={managementLinks.models} onClick={e=>{if(!window.dispatchEvent(new Event("ui-preview:before-navigate",{cancelable:true})))e.preventDefault();}}>管理模型配置 →</a>}
                </>
              )}
              {section === "vision" && !fixture && adapter.pluginManagement && <PluginSettings service={adapter.pluginManagement} />}
              {section === "vision" && (fixture || !adapter.pluginManagement) && (
                <>
                  {managementLinks?.plugins && <p><a href={managementLinks.plugins} onClick={e=>{if(!window.dispatchEvent(new Event("ui-preview:before-navigate",{cancelable:true})))e.preventDefault();}}>打开真实模型与插件管理 →</a> · 安装需单独确认权限和许可证。</p>}
                  {draft.plugins.map((p) => (
                    <Row
                      key={p.id}
                      title={p.name}
                      help={`${p.version} · ${p.status}`}
                    >
                      <Disclosure title="兼容信息">
                        <p>
                          {fixture ? "演示协议：HTTP Vision / ONNX；该信息不证明真实文件已安装。" : "状态来自已安装 Plugin/Model Instance；模型 Ready 与插件启用是不同状态。"}
                        </p>
                      </Disclosure>
                      {p.status !== "Ready" && (
                        <button
                          disabled={
                            !fixture || p.status === "禁用" || p.status === "模拟安装中"
                          }
                          onClick={() =>
                            setConfirm({
                              title: "模拟安装兼容模型？",
                              body: "仅展示安装与校验状态。不下载权重、不授予插件权限，不修改真实文件。",
                              action: () =>
                                adapter.installPlugin(p.id, installFail),
                            })
                          }
                        >
                          安装兼容模型
                        </button>
                      )}
                    </Row>
                  ))}
                  {fixture && <label>
                    <input
                      type="checkbox"
                      checked={installFail}
                      onChange={(e) => setInstallFail(e.target.checked)}
                    />{" "}
                    模拟安装后校验失败
                  </label>}
                </>
              )}
              {section === "privacy" && (
                <>
                  {managementLinks?.storage && <p><a href={managementLinks.storage} onClick={e=>{if(!window.dispatchEvent(new Event("ui-preview:before-navigate",{cancelable:true})))e.preventDefault();}}>查看服务器数据与存储 →</a></p>}
                  <Row
                    title="工作区"
                    help={fixture ? "隔离的浏览器演示命名空间。没有读取本机真实 workspace。" : "本地服务器工作区；没有将服务器目录误称为浏览器本机目录。"}
                  >
                    <span>{fixture ? "UI Preview · 浏览器本地存储" : "Local workspace"}</span>
                  </Row>
                  <Row
                    title="外传授权偏好"
                    help={fixture ? "这里只保存演示偏好，真实发送仍须批准具体数据和接收方。" : "每个操作需批准具体数据和接收方；不提供无限期全局授权开关。"}
                  >
                    <input
                      type="checkbox"
                      aria-label="外传授权偏好"
                      disabled={!fixture}
                      checked={draft.allowExternal}
                      onChange={(e) =>
                        change("allowExternal", e.target.checked)
                      }
                    />
                  </Row>
                  <Row
                    title={fixture ? "演示缓存" : "缓存占用"}
                    help={fixture ? "这个数字来自 Fixture，不是实际磁盘占用。" : "服务端暂无全工作区汇总，不能显示为零；按项目管理引用保护与清理。"}
                  >
                    <span>{fixture ? `${draft.cache} MB · 模拟` : "未汇总"}</span>
                    <button
                      disabled={!fixture}
                      onClick={() =>
                        setConfirm({
                          title: "预览缓存清理范围",
                          body: `${draft.cache} MB 演示缓存中，${state.protectedCache} MB 被引用保护。原图和已确认标注始终保留；此动作不删除真实文件。`,
                          action: async () => {
                            await save({
                              ...draft,
                              cache: state.protectedCache,
                            });
                          },
                        })
                      }
                    >
                      查看清理范围
                    </button>
                  </Row>
                  <Row
                    title="受保护的数据"
                    help="删除历史不等于删除原图或正式标注。"
                  >
                    <span>原图 · 已确认标注 · 被引用模型资产</span>
                  </Row>
                </>
              )}
              {section === "usage" && (
                <>
                  <Row title="统计范围" help={fixture ? "示例数据不是你的真实 API 用量。" : "仅未来 Run 默认预算；Task 账本、Provider probe 与正式执行不是同一个统计范围。"}>
                    <select
                      disabled={!fixture}
                      aria-label="统计范围"
                      value={draft.range}
                      onChange={(e) => change("range", e.target.value)}
                    >
                      {fixture ? <><option>当前任务</option><option>本月</option><option>全部预览</option></> : <option>未来 Run 默认预算</option>}
                    </select>
                  </Row>
                  <Row
                    title="预算上限（USD）"
                    help={fixture ? "演示预算，不会更新真实 Provider 或项目授权。" : "保存未来 Run 的默认预算，不修改已冻结运行或授予任务调用权限。"}
                  >
                    <input
                      aria-label="预算上限"
                      inputMode="decimal"
                      value={draft.budget}
                      onChange={(e) => change("budget", e.target.value)}
                    />
                  </Row>
                  <div className="usage-table">
                    <table>
                      <caption>{fixture ? "Fixture 用量示例" : "暂无统一费用汇总"} · {draft.range}</caption>
                      <thead>
                        <tr>
                          <th>模型</th>
                          <th>Tokens</th>
                          <th>费用 USD</th>
                        </tr>
                      </thead>
                      <tbody>
                        {state.usage.map((row) => (
                          <tr key={row.id}>
                            <td>{row.model}</td>
                            <td>{row.tokens}</td>
                            <td>
                              {row.cost === null
                                ? "未知"
                                : `${row.cost}（模拟）`}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <p
                    className={
                      fixture && Number(draft.budget) <= Number(state.knownCost)
                        ? "error"
                        : "notice"
                    }
                  >
                    {!fixture ? "未知费用不计为零；此处不展示全系统费用。" : Number(draft.budget) < Number(state.knownCost)
                      ? "演示用量超过预算"
                      : Number(draft.budget) * 0.8 <= Number(state.knownCost)
                        ? "演示用量接近预算"
                        : "部分模型费用未知，无法确认总费用。"}
                  </p>
                </>
              )}
              {!editor && !(section === "vision" && !fixture && adapter.pluginManagement) && (
                <div className="settings-actions">
                  <span role="status">
                    {saving
                      ? "保存中…"
                      : saved || (dirty ? "有未保存的更改" : fixture ? "预览设置已载入" : "服务器设置已载入")}
                  </span>
                  <button onClick={cancel} disabled={saving}>
                    取消
                  </button>
                  <button
                    className="primary"
                    disabled={!dirty || saving || !!editor}
                    onClick={() => void save()}
                  >
                    保存设置
                  </button>
                </div>
              )}
            </>
          )}
          {error && (
            <div className="error" role="alert">
              {error}
            </div>
          )}
        </div>
      </section>
      {confirm && (
        <Dialog title={confirm.title} onClose={() => setConfirm(null)}>
          <p>{confirm.body}</p>
          <div className="actions">
            <button autoFocus onClick={() => setConfirm(null)}>
              取消
            </button>
            <button
              className="primary"
              onClick={() => {
                void run(confirm.action);
                setConfirm(null);
              }}
            >
              确认模拟操作
            </button>
          </div>
        </Dialog>
      )}
    </div>
  );
}
