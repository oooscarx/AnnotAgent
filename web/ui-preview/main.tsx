import React from "react";
import { createRoot } from "react-dom/client";
import { AgentPreviewApp } from "../src/agent-ui/App";
import { FixtureAdapter } from "./fixture";
import "../src/annotagent-tokens.css";
import "../src/agent-ui/ui.css";
const adapter = new FixtureAdapter(localStorage);
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AgentPreviewApp
      adapter={adapter}
      preview={{
        scenario: (id, phase) => adapter.scenario(id, phase),
        fail: () => {
          adapter.failNext = true;
        },
      }}
    />
  </React.StrictMode>,
);
