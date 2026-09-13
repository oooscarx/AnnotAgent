import type { TaskModelBindingSummary } from "./modelBindingSummary";
import { Disclosure } from "./Disclosure";

export function TaskModelBindings({ summary }: { summary: TaskModelBindingSummary }) {
  const hasEvidence = summary.planningModel || summary.operationModel || summary.workflowModels.length || summary.authorizationBindings.length;
  if (!hasEvidence) return null;
  return <section className="task-model-bindings" aria-label="规划与视觉模型绑定">
    <div className="task-model-role">
      <span>规划模型</span>
      <strong>{summary.planningModel || "服务端未记录"}</strong>
      <small>用于整理 Schema 和方案，不作为图片样例的视觉推理模型。</small>
    </div>
    <div className="task-model-role">
      <span>Workflow 视觉与工具绑定</span>
      <strong>{summary.workflowModels.length ? summary.workflowModels.join(" → ") : "Draft 尚未提供模型节点"}</strong>
      <small>样例按已保存 Draft 的节点绑定执行，不会因为选择了规划模型而改用它处理图片。</small>
    </div>
    <div className={`task-refiner-state task-refiner-${summary.refiner.state}`}>
      <span>分割精修</span>
      <strong>{summary.refiner.state === "in_draft" ? "已进入 Draft" : summary.refiner.state === "authorized_only" ? "仅在授权范围" : "未进入 Draft"}</strong>
      <small>{summary.refiner.detail}</small>
    </div>
    {(summary.operationModel || summary.authorizationBindings.length > 0) && <Disclosure title="查看精确模型与授权绑定">
      {summary.operationModel && <p>当前操作记录的模型：<code>{summary.operationModel}</code></p>}
      {summary.authorizationBindings.length > 0 && <ul>{summary.authorizationBindings.map(binding => <li key={binding}><code>{binding}</code></li>)}</ul>}
    </Disclosure>}
  </section>;
}
