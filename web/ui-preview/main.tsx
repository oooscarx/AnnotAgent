import React from "react";
import { createRoot } from "react-dom/client";
import { AgentPreviewApp } from "../src/agent-ui/App";
import { FixtureAdapter } from "./fixture";
import { IconGallery } from "./IconGallery";
import "../src/annotagent-tokens.css";
import "../src/agent-ui/ui.css";
const adapter = new FixtureAdapter(localStorage);
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {new URL(location.href).searchParams.has("icons") ? <IconGallery /> : <AgentPreviewApp
      adapter={adapter}
      preview={{
        scenario: (id, phase) => adapter.scenario(id, phase),
        fail: () => {
          adapter.failNext = true;
        },
      }}
    />}
  </React.StrictMode>,
);
