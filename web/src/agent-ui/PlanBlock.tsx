import type { Task } from "./adapter";
import { Icon } from "./Icon";
import { Disclosure } from "./Disclosure";
export function PlanBlock({
  plan,
  expanded,
  fixture = true,
}: {
  plan: NonNullable<Task["plan"]>;
  expanded: boolean;
  fixture?: boolean;
}) {
  const content = (
    <section className="plan-block">
      <strong><Icon name="plan" />标注计划</strong>
      <ol>
        {plan.steps.map((step,index) => (
          <li key={`${index}:${step}`}>{step}</li>
        ))}
      </ol>
      <small>{plan.destination} · {plan.budget ?? "费用未知"}{fixture ? "（演示不收费）" : ""}</small>
      <Disclosure title="计划详情">
        <p>精确版本：{plan.revision}</p>
        <p>模型：{plan.models.join(" → ")}</p>
      </Disclosure>
    </section>
  );
  return expanded ? (
    content
  ) : (
    <Disclosure className="plan-history" title={<>查看{fixture ? "演示" : ""}计划 · {plan.steps.length} 个步骤</>}>
      {content}
    </Disclosure>
  );
}
