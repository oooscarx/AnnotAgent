import { useEffect,useState } from "react";
import type { ProviderPresetProfile } from "../types";
import type { ProviderControlService } from "./ProviderControls";
/** Registry suggestions only: selecting a preset never probes or writes a Provider. */
export function ProviderPresetPicker({service,onSelect}:{service:ProviderControlService;onSelect:(preset:ProviderPresetProfile)=>void}) {
  const [presets,setPresets]=useState<ProviderPresetProfile[]>();
  const [error,setError]=useState("");
  const [selected,setSelected]=useState("");
  useEffect(()=>{let current=true;void service.providerPresets().then(r=>{if(current)setPresets(r.presets);}).catch(e=>{if(current)setError(e.message);});return()=>{current=false;};},[service]);
  return <><label>连接预设<select value={selected} disabled={!presets} onChange={e=>{setSelected(e.target.value);const preset=presets?.find(p=>p.id===e.target.value);if(preset)onSelect(preset);}}><option value="">自定义 OpenAI-compatible Endpoint</option>{presets?.filter(p=>p.adapter==="open_ai_compatible").map(p=><option key={p.id} value={p.id}>{p.display_name}</option>)}</select></label>{error&&<p role="alert">预设读取失败：{error}。仍可填写自定义连接。</p>}<p>预设只填写名称和地址；保存后仍需配置凭证及模型，不会自动探测。</p></>;
}
