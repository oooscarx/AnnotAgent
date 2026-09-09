import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isAgentEntry } from "./agent-ui/routes";

// Separate entry bundles keep legacy management CSS out of the approved UI.
// Neither branch imports the preview or Fixture Adapter.
async function start() {
  const root = createRoot(document.getElementById("root")!);
  if (isAgentEntry(new URL(location.href))) {
    const [{ AgentPreviewApp }, { HttpAdapter }] = await Promise.all([
      import("./agent-ui/App"), import("./agent-ui/http"),
      import("./annotagent-tokens.css"), import("./agent-ui/ui.css"),
    ]);
    const adapter = new HttpAdapter(undefined, localStorage);
    root.render(<StrictMode><AgentPreviewApp adapter={adapter} /></StrictMode>);
    void adapter.refresh().catch(() => { /* Real errors are rendered by the adapter. */ });
  } else {
    const [{ App }] = await Promise.all([import("./App"), import("./styles.css")]);
    root.render(<StrictMode><App /></StrictMode>);
  }
}
void start();
