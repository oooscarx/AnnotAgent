import { useId } from "react";
import type { SchemaFields } from "../conversation-future-schema";

/** Shared bounded semantic fields; no model bindings, code or permissions. */
export function ConversationSchemaFields({ value, onChange, disabled, includeGoalAndKind = false }: { value: SchemaFields; onChange: (value: SchemaFields) => void; disabled: boolean; includeGoalAndKind?: boolean }) {
  const id = useId();
  return <div className="conversation-schema-fields">
    {includeGoalAndKind && <><label htmlFor={`${id}-goal`}>Future task goal</label><textarea id={`${id}-goal`} rows={3} disabled={disabled} value={value.goal} onChange={event => onChange({ ...value, goal: event.target.value })} /><label htmlFor={`${id}-kind`}>Output type</label><select id={`${id}-kind`} disabled={disabled} value={value.kind} onChange={event => onChange({ ...value, kind: event.target.value as SchemaFields["kind"] })}><option value="bounding_box">Object boxes</option><option value="classification">Whole-image categories</option></select></>}
    <label htmlFor={`${id}-labels`}>Labels · one per line</label><textarea id={`${id}-labels`} rows={3} disabled={disabled} value={value.labels} onChange={event => onChange({ ...value, labels: event.target.value })} />
    <label htmlFor={`${id}-rules`}>Boundary rules · one per line</label><textarea id={`${id}-rules`} rows={3} disabled={disabled} value={value.rules} onChange={event => onChange({ ...value, rules: event.target.value })} />
  </div>;
}
