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
          <span className="eyebrow">404 · Workspace route</span>
          <h2>This page does not exist</h2>
          <p>
            AnnotAgent kept the requested address visible instead of silently
            sending you somewhere unrelated.
          </p>
          <code>{invalidPath}</code>
        </div>
        <div className="button-row">
          <button onClick={() => window.history.back()}>Go back</button>
          <button className="primary" onClick={() => onNavigate("/projects")}>
            Open Projects
          </button>
        </div>
      </div>
    </section>
  );
}
