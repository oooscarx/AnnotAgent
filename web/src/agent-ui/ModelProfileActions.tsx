import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { ModelCapabilityQualityContract, ProviderProfile, ProviderProbeUsage, RegistryModelProfile } from "../types";
import { geometrySemanticsLabel, scoreSemanticsLabel } from "../pipelinePresentation";
import { Disclosure } from "./Disclosure";
import { Dialog } from "./Dialog";
export type ModelActionService = Pick<typeof api,"modelProfiles"|"providers"|"updateModelProfile"|"deleteModelProfile"|"activeProbe"|"modelProfileUsage"|"modelQualityContracts">;
type Operation = "lock" | "unlock" | "delete" | "probe";
export function ModelProfileActions({model,provider,service,reload}:{model:RegistryModelProfile;provider?:ProviderProfile;service:ModelActionService;reload:()=>Promise<void>}) {
  const [operation,setOperation]=useState<Operation>();
  const [confirmed,setConfirmed]=useState(false);
  const [typed,setTyped]=useState("");
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [contracts,setContracts]=useState<ModelCapabilityQualityContract[]>();
  const [usage,setUsage]=useState<ProviderProbeUsage[]>();
  const pending=useRef(false);
  const live=useRef(true);
  useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const run=async(action:()=>Promise<void>)=>{
    if(pending.current)return;pending.current=true;setBusy(true);setError("");
    try{await action();}catch(reason){if(live.current)setError((reason as Error).message);}
    finally{pending.current=false;if(live.current)setBusy(false);}
  };
  const show=(action:Operation)=>{setConfirmed(false);setTyped("");setError("");setOperation(action);};
  const execute=()=>void run(async()=>{
    if(!operation||!confirmed||(operation==="delete"&&typed!==model.display_name))return;
    const current=(await service.modelProfiles()).models.find(m=>m.id===model.id);
    if(!current||current.revision!==model.revision||current.locked!==model.locked)throw new Error("模型配置已变化，未执行操作。关闭后重新读取。");
    if(operation==="probe"){
      const connection=(await service.providers()).providers.find(p=>p.id===model.provider_id);
      if(!provider||!connection?.enabled||JSON.stringify(connection)!==JSON.stringify(provider))throw new Error("Provider 配置已变化，未开始收费测试。请重新读取并确认。");
      const result=await service.activeProbe(model.provider_id,model.id);
      if(live.current)setUsage(old=>[result.usage,...old??[]]);
    } else if(operation==="delete")await service.deleteModelProfile(model.id);
    else await service.updateModelProfile(model.id,{locked:operation==="lock",expected_revision:model.revision});
    if(live.current)setOperation(undefined);
    await reload();
  });
  return <Disclosure title="模型状态、质量与管理">
    {error&&!operation&&<p role="alert">{error}</p>}
    <p>Revision {model.revision} · {model.capability_source} · {model.locked?"已锁定":"可编辑"}</p>
    <div className="actions">
      <button disabled={busy} onClick={()=>show(model.locked?"unlock":"lock")}>{model.locked?"解锁配置…":"锁定配置…"}</button>
      <button disabled={busy||!model.enabled||!provider?.enabled} onClick={()=>show("probe")}>收费连接测试…</button>
      <button disabled={busy||model.locked} onClick={()=>show("delete")}>删除模型配置…</button>
      <button disabled={busy} onClick={()=>void run(async()=>{const result=await service.modelQualityContracts(model.id);if(result.model_profile_id!==model.id||result.model_profile_revision!==model.revision)throw new Error("质量契约版本已变化，请重新读取模型。");if(live.current)setContracts(result.contracts);})}>读取质量契约</button>
      <button disabled={busy} onClick={()=>void run(async()=>{const result=await service.modelProfileUsage(model.id);if(result.model_profile_id!==model.id)throw new Error("用量归属不匹配");if(live.current)setUsage(result.active_probes);})}>读取测试记录</button>
    </div>
    {contracts?.map(contract=><article className="model-contract" key={`${contract.operation}:${contract.capability}`}>
      <strong>{contract.operation} · {contract.capability}</strong>
      <p>{geometrySemanticsLabel(contract.output_geometry)} · {scoreSemanticsLabel(contract.score_semantics)}</p>
      <p>自动接受策略：{contract.auto_accept_eligibility}；证据来源：{contract.evidence_source}</p>
      {contract.requires_geometry_verification&&<p>框的自动接受仍需要几何验证或人工审核，不能只依赖模型分数。</p>}
    </article>)}
    {contracts?.length===0&&<p>没有几何操作契约。</p>}
    {usage?.map(item=><article key={item.id}><strong>{item.succeeded?"测试成功":"测试失败"} · {item.created_at}</strong><p>{item.safe_message}</p><p>{item.duration_ms} ms · tokens {item.total_tokens??"未知"} · {Number(item.cost)>0?`${item.currency} ${item.cost}（服务器估算，非账单）`:"费用未核实（服务器零记录不等于免费）"}</p></article>)}
    {usage?.length===0&&<p>没有保存的测试记录。</p>}
    <Disclosure title="限制与生成默认值"><pre>{JSON.stringify({limits:model.limits,generation_defaults:model.generation_defaults},null,2)}</pre></Disclosure>
    {operation&&<Dialog title={operation==="probe"?"确认收费连接测试":operation==="delete"?"删除模型配置":operation==="lock"?"锁定模型配置":"解锁模型配置"} onClose={()=>{if(!busy)setOperation(undefined);}}>
      <p>{model.display_name} · {model.remote_model_id} · revision {model.revision}</p>
      {operation==="probe"?<><p>目标：{provider?.endpoint_summary||provider?.base_url}</p><p>此操作会发送服务端测试请求，可能产生费用，金额未知。连接成功不代表所有能力或几何准确性通过验证。不会运行项目数据集。</p></>:operation==="delete"?<><p>永久删除这个 Registry 配置；不会删除图片或发布版本。被引用时由服务器阻止删除，不自动解除引用。</p><label>输入模型显示名称<input value={typed} disabled={busy} onChange={e=>setTyped(e.target.value)}/></label></>:<p>只更改此配置的编辑锁，不改变已发布工作流。</p>}
      <label className="model-check"><input type="checkbox" checked={confirmed} disabled={busy} onChange={e=>setConfirmed(e.target.checked)}/>我确认上述范围{operation==="probe"?"，并同意可能产生的费用":""}</label>
      {error&&<p role="alert">{error}；未确认成功时请先核对服务器记录，不自动重试。</p>}
      <div className="actions"><button disabled={busy} onClick={()=>setOperation(undefined)}>取消</button><button disabled={busy||!!error||!confirmed||(operation==="delete"&&typed!==model.display_name)} onClick={execute}>{busy?"等待服务器回执…":"确认操作"}</button></div>
    </Dialog>}
  </Disclosure>;
}
