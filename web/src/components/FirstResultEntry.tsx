import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import type { Annotation } from "../types";
import { AnnotationCanvas } from "./AnnotationCanvas";

const exampleImage = "/brand/core/offline-example.png";
// Explicitly authored illustration, never a prediction or a persisted annotation.
const exampleAnnotation: Annotation = {
  id: "offline-example", image_id: "offline-example", task_id: "example",
  label: "Ball", value: { kind: "bounding_box", rect: [0.547, 0.75, 0.041, 0.063] },
  attributes: {}, source: "offline illustration", review_status: "needs_review",
  provenance: {}, created_at: "2026-09-07T00:00:00Z",
};

export function FirstResultEntry({ returning, onStart }: { returning: boolean; onStart: () => void }) {
  const [example, setExample] = useState(false);
  return <>
    <section className={`first-result-entry ${returning ? "returning" : ""}`}>
      <div className="first-result-intro">
        <img className="first-result-logo" src="/brand/core/annotagent-mark.svg" alt="AnnotAgent" />
        <h2>{t(returning ? "Continue your work" : "Turn images into annotations you can inspect.")}</h2>
        <p>{t(returning ? "Open a Project below to continue processing, review results, or add images." : "Tell AnnotAgent what to find. Check a few samples before processing the rest.")}</p>
        <div className="button-row">
          <button className={returning ? "" : "primary"} onClick={onStart}>{t("Start with images")}</button>
          <button onClick={() => setExample(true)}>{t("Explore an example")}</button>
        </div>
        <small>{t("Define your goal without a key. Real inference requires a compatible model and your authorization.")}</small>
      </div>
      {!returning && <figure className="first-result-cover"><img src={exampleImage} alt={t("Illustrated example scene")} /><figcaption>{t("Offline demo · no model inference")}</figcaption></figure>}
    </section>
    {example && <OfflineExample onClose={() => setExample(false)} onStart={() => { setExample(false); onStart(); }} />}
  </>;
}

function OfflineExample({ onClose, onStart }: { onClose: () => void; onStart: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [step, setStep] = useState(0);
  const [annotation, setAnnotation] = useState(exampleAnnotation);
  const [selected, setSelected] = useState<string>();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.showModal();
    return () => { previous?.focus(); };
  }, []);
  return <dialog ref={dialog} className="first-result-example" aria-label={t("Offline annotation example")} onCancel={onClose}>
    <header className="first-result-example-header">
      <div><strong>{t("Offline demo · no model inference")}</strong><p>{t("Illustrated data only. Edits stay in this example; nothing is saved or sent.")}</p></div>
      <button onClick={onClose}>{t("Close")}</button>
    </header>
    <nav className="first-result-stages" aria-label={t("Example stages")}>
      {["Define your goal", "See a sample", "Start processing"].map((title, index) => <button key={title} aria-current={index === step ? "step" : undefined} onClick={() => setStep(index)}>{index + 1}. {t(title)}</button>)}
    </nav>
    <div className="first-result-example-grid">
      <div className="first-result-image-stage">{step === 0 ? <img src={exampleImage} alt={t("Illustrated example scene")} /> : <AnnotationCanvas imageUrl={exampleImage} annotations={[annotation]} selectedId={selected} onSelect={setSelected} onChange={setAnnotation} />}</div>
      <section className="first-result-decision">
        <h2>{t(["Define your goal", "Inspect the result", "Approve a plan, not every annotation"][step])}</h2>
        {step === 0 && <><p>{t("Example goal: find the ball and draw a box around it.")}</p><p>{t("Choose images and a label yourself. A language model is optional for defining the goal.")}</p><button className="primary" onClick={() => setStep(1)}>{t("See a sample")}</button></>}
        {step === 1 && <><p>{t("This box is hand-authored for the demo. Select it to try the existing annotation editor.")}</p><button onClick={() => setSelected(annotation.id)}>{t("Select example box")}</button><button onClick={() => setAnnotation(exampleAnnotation)}>{t("Reset example edits")}</button><button className="primary" onClick={() => setStep(2)}>{t("See processing options")}</button></>}
        {step === 2 && <><p>{t("In a real task, inspect the model, data destination, sample results and budget before authorizing a published plan and a batch.")}</p><p>{t("Review uncertain results and spot-check accepted and empty images before export. A successful run is not proof of accuracy.")}</p></>}
        {step > 0 && <button onClick={onStart}>{t("Use my images")}</button>}
      </section>
    </div>
  </dialog>;
}
