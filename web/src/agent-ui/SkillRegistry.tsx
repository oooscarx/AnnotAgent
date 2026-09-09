import {useEffect,useState} from "react";
import type {api} from "../api";
import type {SkillDetail} from "../types";
import {Disclosure} from "./Disclosure";
const sections=[['nodes','节点'],['tools','工具'],['capabilities','能力'],['capability_requirements','能力要求'],['validators','验证器'],['refiners','精修器'],['policies','策略'],['correction_taxonomy','修正类型'],['resources','提示资源'],['projects','使用此 Skill 的项目']] as const;
export function SkillRegistry({service}:{service:Pick<typeof api,"skills">}) {
  const [items,setItems]=useState<SkillDetail[]>();const [error,setError]=useState("");const [search,setSearch]=useState("");const [reload,setReload]=useState(0);
  useEffect(()=>{const c=new AbortController();setError("");setItems(undefined);void service.skills(c.signal).then(value=>{if(!c.signal.aborted)setItems(value);}).catch(e=>{if(!c.signal.aborted)setError(e.message);});return()=>c.abort();},[service,reload]);
  const visible=items?.filter(item=>`${item.id} ${item.display_name} ${item.description}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()));
  return <section aria-label="Skill 注册表" className="native-skill-registry"><p>服务器实际注册的能力、领域规则和 Skill Pack。注册不等于模型已安装或推理可用；此处只读取，不调用模型。</p>
    <label>搜索 Skill<input value={search} onChange={e=>setSearch(e.target.value)}/></label><button onClick={()=>setReload(v=>v+1)}>重新读取 Skills</button>
    {error&&<p role="alert">{error}。未使用演示清单替代。</p>}{!items&&!error&&<p role="status">读取 Skill 注册表…</p>}
    {items?.length===0&&<p>服务器未注册 Skill。</p>}{!!items?.length&&visible?.length===0&&<p>没有匹配的 Skill。</p>}
    {visible?.map(item=><Disclosure key={`${item.id}@${item.version}`} title={`${item.display_name} · ${item.version}`}><p>{item.kind} · {item.id}</p><p>{item.description}</p>
      <dl>{sections.map(([key,label])=><div key={key}><dt>{label}</dt><dd>{item[key].length?item[key].join(" · "):"未声明"}</dd></div>)}</dl>
      <h3>Workflow 模板</h3>{item.workflow_templates.length?item.workflow_templates.map(template=><p key={template.id}>{template.name} · {template.id} · {template.node_count} 节点<br/>{template.description}</p>):<p>未声明模板。</p>}
      {item.project_template&&<Disclosure title="项目模板定义"><pre>{item.project_template}</pre></Disclosure>}
    </Disclosure>)}
  </section>;
}
