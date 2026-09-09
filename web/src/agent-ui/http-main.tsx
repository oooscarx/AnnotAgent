import { createRoot } from "react-dom/client";
import { AgentPreviewApp } from "./App";
import { HttpAdapter } from "./http";
import "../annotagent-tokens.css";
import "./ui.css";
const adapter = new HttpAdapter(undefined, localStorage);
createRoot(document.getElementById("root")!).render(<AgentPreviewApp adapter={adapter} />);
void adapter.refresh().catch(() => { /* The adapter exposes the real failure to the page. */ });
