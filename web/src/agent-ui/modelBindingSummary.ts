import type { Task } from "./adapter";

export type TaskModelBindingSummary = {
  planningModel: string | null;
  operationModel: string | null;
  workflowModels: string[];
  authorizationBindings: string[];
  refiner: {
    state: "in_draft" | "authorized_only" | "absent";
    models: string[];
    detail: string;
  };
};

const refinerPattern = /(efficient\s*sam|\bsam\b|prompted[_ -]?segmentation|segment(?:ation)?[_ -]?refin|mask[_ -]?to[_ -]?bbox|geometry[_ -]?refin)/i;
const bindingPattern = /(model-profile:|model-instance:|binding|模型|qwen|glm|sam|provider)/i;

/**
 * Summarizes only persisted task evidence. A model in an authorization scope is
 * not described as part of the Draft until the saved Builder proposal lists it.
 */
export function taskModelBindingSummary(task: Task): TaskModelBindingSummary {
  const workflowModels = [...new Set(task.plan?.models.filter(Boolean) || [])];
  const authorizationBindings = [...new Set(
    (task.approval?.scope || []).filter(item => bindingPattern.test(item)),
  )];
  const draftEvidence = [
    ...workflowModels,
    ...(task.plan?.steps || []),
  ].filter(item => refinerPattern.test(item));
  const authorizedEvidence = authorizationBindings.filter(item => refinerPattern.test(item));
  const state = draftEvidence.length
    ? "in_draft"
    : authorizedEvidence.length
      ? "authorized_only"
      : "absent";
  return {
    planningModel: task.model || null,
    operationModel: task.operationModel || null,
    workflowModels,
    authorizationBindings,
    refiner: {
      state,
      models: [...new Set(state === "in_draft" ? draftEvidence : authorizedEvidence)],
      detail: state === "in_draft"
        ? "已保存 Draft 含分割精修节点或模型绑定；这不等于本次样例已经实际调用精修模型。"
        : state === "authorized_only"
          ? "精修模型只出现在当前授权范围，尚无证据表明它已进入保存的 Draft。"
          : "当前保存的 Draft 和授权摘要都没有发现分割精修绑定。",
    },
  };
}
