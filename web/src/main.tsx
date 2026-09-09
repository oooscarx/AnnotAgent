import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
// Every production URL uses the same UI. Unknown routes are errors, not a
// second application, and failed HTTP requests never fall back to fixtures.
async function start() {
  const root = createRoot(document.getElementById("root")!);
    const [{ AgentPreviewApp }, { HttpAdapter }] = await Promise.all([
      import("./agent-ui/App"), import("./agent-ui/http"),
      import("./annotagent-tokens.css"), import("./agent-ui/ui.css"),
    ]);
    const adapter = new HttpAdapter(undefined, localStorage);
    root.render(<StrictMode><AgentPreviewApp adapter={adapter} /></StrictMode>);
    void adapter.refresh().catch(() => { /* Real errors are rendered by the adapter. */ });
}
void start();
