import { useEffect, useRef, useState } from "react";
import type { ExpertPluginRegistry, InstalledModelInstance, InstalledModelBundle, VerifiedExpertPluginPackage } from "../types";
import type { PluginManagement } from "./pluginManagement";
import { Disclosure } from "./Disclosure";
import { Dialog } from "./Dialog";
import { BundleInstaller } from "./BundleInstaller";

export function PluginSettings({ service, workspaceId }: { service: PluginManagement; workspaceId?: string }) {
  const [data, setData] = useState<{ registry: ExpertPluginRegistry; instances: InstalledModelInstance[]; bundles: InstalledModelBundle[] }>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [testReport, setTestReport] = useState("");
  const [confirmation, setConfirmation] = useState<{ title: string; detail: string; action: () => Promise<unknown> }>();
  const [file, setFile] = useState<File>();
  const [inspection, setInspection] = useState<VerifiedExpertPluginPackage>();
  const [accepted, setAccepted] = useState(false);
  const generation = useRef(0);
  const pending = useRef(false);
  const reload = async () => {
    const current = ++generation.current;
    const [registry, instances, bundles] = await Promise.all([service.expertPlugins(), service.modelInstances(), service.modelBundles()]);
    if (current === generation.current) setData({registry, instances: instances.instances, bundles: bundles.bundles});
  };
  useEffect(() => { void reload().catch(e => setError(e.message)); return () => { generation.current++; }; }, [service]);
  useEffect(() => {
    const guard = (event: Event) => { if ((file || busy) && !window.confirm("安装包尚未提交或操作尚未核实，仍要离开？")) event.preventDefault(); };
    const unload = (event: BeforeUnloadEvent) => { if (file || busy) { event.preventDefault(); event.returnValue = ""; } };
    window.addEventListener("ui-preview:before-navigate", guard); window.addEventListener("beforeunload", unload);
    return () => { window.removeEventListener("ui-preview:before-navigate", guard); window.removeEventListener("beforeunload", unload); };
  }, [file, busy]);
  const execute = async () => {
    if (!confirmation || pending.current) return;
    pending.current = true; setBusy(true); setError(""); setMessage("");
    try {
      await confirmation.action();
      setConfirmation(undefined);
      setMessage("服务器已返回操作结果。正在重新读取状态…");
      await reload();
      setMessage("状态已从服务器更新。测试是否通过请查看对应测试结果。");
    } catch (e) { setConfirmation(undefined); setError(`${(e as Error).message}。未自动重试；请重新读取状态后核实结果。`); }
    finally { pending.current = false; setBusy(false); }
  };
  const ask = (title: string, detail: string, action: () => Promise<unknown>) => setConfirmation({title, detail, action});
  return <section className="plugin-settings" aria-label="真实插件与模型包管理">
    <p>本机插件与模型实例分别显示。旧权重槽缺失不等于配套 Model Bundle 不可用。</p>
    {error && <p role="alert" className="error">{error}</p>}
    {message && <p role="status">{message}</p>}
    {testReport && <Disclosure title="最近一次插件测试结果"><p>{testReport}</p></Disclosure>}
    <button disabled={busy} onClick={() => { setError(""); void reload().catch(e => setError(e.message)); }}>重新读取状态</button>
    {!data ? <p role="status">{error ? "未能读取插件清单。" : "读取插件、模型包与实例…"}</p> : <>
      <h2>已安装插件</h2>
      {!data.registry.installations.length && <p>尚未安装插件。可在下方检查本地安装包。</p>}
      {data.registry.installations.map(plugin => {
        const { id, version, display_name } = plugin.manifest;
        const instances = data.instances.filter(i => i.plugin_id === id && i.plugin_version === version);
        return <article className="settings-row" key={`${id}@${version}`}>
          <div><strong>{display_name}</strong><p>{version} · {plugin.enabled ? "已启用" : "已禁用"}</p><p>旧权重入口：{plugin.status} · Ready 模型实例：{instances.filter(i => i.status === "ready").length}</p></div>
          <div className="row-control">
            <Disclosure title="权限、来源与引用"><p>{id}</p><p>包摘要：{plugin.package_sha256}</p><pre>{JSON.stringify(plugin.manifest.permissions, null, 2)}</pre><p>代码许可证：{plugin.manifest.license.code}；权重：{plugin.manifest.license.weights}</p>{plugin.references.map((r, i) => <p key={i}>{r.kind} · {r.location}</p>)}</Disclosure>
            <div className="actions">
              <button disabled={busy} onClick={() => ask("测试插件", "将启动已安装插件进行实际测试，可能使用本机计算资源。", async () => { const result = await service.testExpertPlugin(id, version); setTestReport(`${display_name}: ${result.report.passed ? "通过" : "未通过"}；${result.report.checks.map(check => `${check.name}: ${check.passed ? "通过" : "失败"} — ${check.detail}`).join("；")}`); })}>测试插件</button>
              <button disabled={busy} onClick={() => ask(plugin.enabled ? "禁用插件" : "启用插件", "将改变后续模型可用性；服务端仍会检查引用与权限。", () => service.setExpertPluginEnabled(id, version, !plugin.enabled))}>{plugin.enabled ? "禁用" : "启用"}</button>
              <button disabled={busy || plugin.references.length > 0} onClick={() => ask("卸载插件", `仅卸载 ${id}@${version}。已有引用由服务端保护；这不是删除项目或标注。`, () => service.uninstallExpertPlugin(id, version))}>卸载</button>
            </div>
            {plugin.references.length > 0 && <p>存在引用，不能在此直接卸载。</p>}
            <Disclosure title="安装兼容模型与查看进度"><BundleInstaller service={service} pluginId={id} version={version} workspaceId={workspaceId}/></Disclosure>
          </div>
        </article>;
      })}
      <h2>模型实例</h2>
      {!data.instances.length && <p>尚无模型实例。安装插件不等于模型已经可用。</p>}
      {data.instances.map(instance => <article className="settings-row" key={instance.id}>
        <div><strong>{instance.model_id}</strong><p>{instance.status} · {instance.execution_provider}</p><p>最近测试：{instance.smoke_test_result?.status ?? "尚未测试"}</p></div>
        <div className="row-control"><Disclosure title="模型包与测试来源"><p>{instance.model_bundle_id}@{instance.model_bundle_version}</p><p>{instance.id}</p><p>{instance.contract_inspection.errors.join("；") || "契约检查未报告错误"}</p></Disclosure><button disabled={busy} onClick={() => ask("测试模型实例", "执行真实模型冒烟测试；不会标注当前项目图片。", () => service.testModelInstance(instance.id))}>运行模型测试</button></div>
      </article>)}
      <h2>模型包</h2>
      {data.bundles.map(bundle => <article className="settings-row" key={`${bundle.manifest.id}@${bundle.manifest.version}`}>
        <div><strong>{bundle.manifest.display_name}</strong><p>{bundle.manifest.version} · {bundle.status} · {bundle.enabled ? "已启用" : "已禁用"}</p></div>
        <div className="row-control"><button disabled={busy} onClick={() => { void service.modelBundleReferences(bundle.manifest.id, bundle.manifest.version).then(result => ask(bundle.enabled ? "禁用模型包" : "启用模型包", `当前引用 ${result.references.length} 项。${result.references.map(r => `${r.kind}: ${r.location}`).join("；")}。服务端仍负责最终引用保护。`, () => service.setModelBundleEnabled(bundle.manifest.id, bundle.manifest.version, !bundle.enabled))).catch(e => setError(e.message)); }}>{bundle.enabled ? "禁用…" : "启用…"}</button></div>
      </article>)}
    </>}
    <h2>安装本地插件包</h2>
    <p>文件尚未上传时不会保存。先检查包，再明确确认权限与许可证；不自动下载模型。</p>
    <label className="plugin-package-picker">选择插件包<input type="file" disabled={busy} onChange={e => { setFile(e.target.files?.[0]); setInspection(undefined); setAccepted(false); }} /></label>
    {file && <div className="actions"><span>{file.name}</span><button disabled={busy} onClick={() => ask("检查插件包", `上传 ${file.name} 到当前服务器进行检查，不安装。`, async () => setInspection(await service.inspectExpertPluginPackage(file)))}>检查安装包</button><button disabled={busy} onClick={() => { setFile(undefined); setInspection(undefined); }}>取消选择</button></div>}
    {inspection && <div className="plan-block"><strong>{inspection.manifest.display_name} · {inspection.manifest.version}</strong><p>{inspection.install_guidance}</p><p>签名：{inspection.signature} · {inspection.signature_trusted ? "受信任" : "未建立信任"}</p><pre>{JSON.stringify(inspection.manifest.permissions, null, 2)}</pre><p>代码：{inspection.manifest.license.code}；权重：{inspection.manifest.license.weights}</p><label><input type="checkbox" checked={accepted} onChange={e => setAccepted(e.target.checked)} />我已检查权限，并接受上述代码和权重许可证</label><button disabled={busy || !accepted || !inspection.web_installable} onClick={() => file && ask("确认安装插件", `安装 ${inspection.manifest.id}@${inspection.manifest.version}，包摘要 ${inspection.package_sha256}。不会自动安装权重。`, async () => { await service.installExpertPluginPackage(file, {permissions_reviewed:true, code_license_accepted:true, weight_license_accepted:true}); setFile(undefined); setInspection(undefined); })}>安装已检查的包</button></div>}
    {confirmation && <Dialog title={confirmation.title} onClose={() => { if (!busy) setConfirmation(undefined); }}><p>{confirmation.detail}</p><div className="actions"><button disabled={busy} onClick={() => setConfirmation(undefined)}>取消</button><button disabled={busy} onClick={() => void execute()}>{busy ? "等待服务器结果…" : "确认操作"}</button></div></Dialog>}
  </section>;
}
