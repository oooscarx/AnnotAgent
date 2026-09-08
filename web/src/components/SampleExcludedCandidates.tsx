import { useState } from "react";
import type { ExcludedSampleCandidate } from "../sampleFeedbackOverlay";
import { AnnotationCanvas } from "./AnnotationCanvas";
import { t } from "../i18n";

/** An explicit exclusion remains inspectable; large groups mount one canvas only. */
export function SampleExcludedCandidates({ excluded, imageUrl }: { excluded: ExcludedSampleCandidate[]; imageUrl: string }) {
  const [opened, setOpened] = useState(false), [selected, setSelected] = useState("");
  const current = excluded.find(item => item.annotation.id === selected) ?? excluded[0];
  if (!current) return null;
  return <details className="sample-excluded-candidates" onToggle={event => setOpened(event.currentTarget.open)}><summary>{t("Excluded sample candidates")} · {excluded.length}</summary>
    {opened && <><p>{t("Explicitly excluded from this sample view. Original predictions and formal annotations are unchanged.")}</p><label>{t("Excluded candidate to inspect")}<select aria-label={t("Excluded candidate to inspect")} value={current.annotation.id} onChange={event => setSelected(event.target.value)}>{excluded.map(item => <option key={item.annotation.id} value={item.annotation.id}>{item.annotation.label} · {item.annotation.id}</option>)}</select></label><AnnotationCanvas compactList readOnly imageUrl={imageUrl} annotations={[current.annotation]} selectedId={current.annotation.id} onSelect={() => {}} onChange={() => {}} /></>}
  </details>;
}
