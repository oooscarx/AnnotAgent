import { Disclosure } from "./Disclosure";
import type { ImageRefinementEvidence } from "./refinementExecutionEvidence";

const sourceLabel = (executed:boolean) => executed ? "prompted segmentation refined" : "VLM-only";

export function RefinementEvidencePanel({evidence,selected}:{evidence:ImageRefinementEvidence;selected?:string}) {
  if (!evidence.candidates.length) return null;
  const selectedEvidence = selected ? evidence.candidates.find((item) => item.candidate_id === selected) : undefined;
  const refined = evidence.candidates.filter((item) => item.executed).length;
  const summary = selectedEvidence
    ? `当前终端候选来源：${sourceLabel(selectedEvidence.executed)}`
    : evidence.candidates.length === 1
      ? `本图终端候选来源：${sourceLabel(evidence.candidates[0].executed)}`
      : `本图终端候选：prompted segmentation refined ${refined} 个，VLM-only ${evidence.candidates.length-refined} 个`;
  const current = selectedEvidence ?? (evidence.candidates.length === 1 ? evidence.candidates[0] : undefined);
  return <section className="refinement-execution-evidence" aria-label="终端候选实际执行来源">
    <strong>{summary}</strong>
    {current && <p>{current.executed ? "提示分割已执行" : "方案包含但本图未执行提示分割"} · {current.reason}</p>}
    <Disclosure title="查看实际节点证据">
      {evidence.candidates.map((candidate) => <div className="refinement-evidence-candidate" key={candidate.candidate_id}>
        <p><strong>{candidate.candidate_id}</strong> · {sourceLabel(candidate.executed)}</p>
        <p>{candidate.reason}</p>
        {candidate.items.length ? <ul>{candidate.items.map((item,index) => <li key={`${item.kind}:${item.node_id}:${item.artifact_id||index}`}>
          <strong>{item.label}</strong> · 节点 <code>{item.node_id}</code>
          {item.status ? ` · ${item.status}` : ""}
          {item.artifact_ref ? <> · Artifact <code>{item.artifact_ref}</code></> : null}
          {item.detail ? ` · ${item.detail}` : ""}
        </li>)}</ul> : <p>没有与此终端 lineage 匹配的精修节点或 Artifact。</p>}
      </div>)}
    </Disclosure>
  </section>;
}
