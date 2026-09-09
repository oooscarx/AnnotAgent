import {useEffect,useState} from "react";
import {parseReviewAttributes} from "./reviewEdits";
export function ReviewAttributes({value,disabled,onApply,onPendingChange}:{value:Record<string,unknown>;disabled:boolean;onApply:(value:Record<string,unknown>)=>void;onPendingChange:(pending:boolean)=>void}) {
  const saved=JSON.stringify(value,null,2);
  const [text,setText]=useState(saved);
  const [error,setError]=useState("");
  const [base,setBase]=useState(saved);
  const dirty=text!==base;
  useEffect(()=>{onPendingChange(dirty);return()=>onPendingChange(false);},[dirty,onPendingChange]);
  useEffect(()=>{if(!dirty){setText(saved);setBase(saved);}},[saved,dirty]);
  useEffect(()=>{const guard=(e:Event)=>{if(dirty&&!confirm("属性文本还未应用到编辑，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty]);
  return <section aria-label="标注属性编辑"><label>属性 JSON<textarea aria-label="属性 JSON" value={text} disabled={disabled} onChange={e=>setText(e.target.value)}/></label>{error&&<p role="alert">{error}</p>}{dirty&&saved!==base&&<p role="alert">其他编辑已改变属性，请先取消此文本编辑，再读取新属性。</p>}<div className="actions"><button disabled={disabled||!dirty||saved!==base} onClick={()=>{try{const next=parseReviewAttributes(text);onApply(next);setText(JSON.stringify(next,null,2));setBase(JSON.stringify(next,null,2));setError("");}catch(e){setError((e as Error).message);}}}>应用属性到编辑</button><button disabled={disabled||!dirty} onClick={()=>{setText(saved);setBase(saved);setError("");}}>取消属性编辑</button></div><p>应用只修改当前编辑；仍需点击“保存编辑”写入服务器。未应用文本离开前会提醒。</p></section>;
}
