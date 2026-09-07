import { t } from "../../i18n";
export function NotFoundPage({
  invalidPath,
  onNavigate,
}: {
  invalidPath: string;
  onNavigate: (path: string) => void;
}) {
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">{t("404 · Workspace route")}</span>
          <h2>{t("This page does not exist")}</h2>
          <p>{t("AnnotAgent kept the requested address visible instead of silently sending you somewhere unrelated.")}</p>
          <code>{invalidPath}</code>
        </div>
        <div className="button-row">
          <button onClick={() => window.history.back()}>{t("Go back")}</button>
          <button className="primary" onClick={() => onNavigate("/projects")}>{t("Open Projects")}</button>
        </div>
      </div>
    </section>
  );
}
