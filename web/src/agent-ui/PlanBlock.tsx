import type { Task } from "./adapter";
export function PlanBlock({
  plan,
  expanded,
}: {
  plan: NonNullable<Task["plan"]>;
  expanded: boolean;
}) {
  const content = (
    <section className="plan-block">
      <strong>☷ 标注计划</strong>
      <ol>
        {plan.steps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <small>{plan.destination} · 费用未知（演示不收费）</small>
      <details>
        <summary>计划详情</summary>
        <p>精确版本：{plan.revision}</p>
        <p>模型：{plan.models.join(" → ")}</p>
      </details>
    </section>
  );
  return expanded ? (
    content
  ) : (
    <details className="plan-history">
      <summary>☷ 查看演示计划 · {plan.steps.length} 个步骤</summary>
      {content}
    </details>
  );
}
