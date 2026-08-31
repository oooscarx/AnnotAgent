# Provider Registry + Pipeline Builder Alpha — five-minute demo

This demo uses the offline Mock Provider, so it sends no network request and needs no API key.
Start from the repository root:

```bash
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

## 0:00–0:40 — reusable Provider and Model setup

Open **Settings → Providers**. Explain that a Provider owns one reusable connection and an opaque
credential reference; it does not choose a model for every Project. Add **Mock (offline)** and run
**Check connection**. The passive check does not send a generation request or create usage.

Open **Settings → Models**. Add a Mock Model Profile with text and image input, Text generation,
Vision language, Image classification, Tool calls and Structured output. Run the separately labelled
billable-model test; Mock records deterministic usage but the same action requires explicit charge
confirmation for a live Provider. Set it as the Pipeline Builder and Vision Language default.

## 0:40–1:15 — explicit compatibility migration

If the workspace still uses the old singleton Settings, return to **Providers**. The migration card
previews the Provider, Model Profile, credential-reference source and Project binding count. Choose
**Review and import** and confirm.

Point out the guarantees:

- the Provider, revision-1 Model Profile and `default-vision` Project bindings are one SQLite
  transaction;
- rerunning the import is a no-op;
- an existing user binding wins;
- a legacy credential remains a reference and its value is not copied;
- Published Versions and historical Runs are not rewritten.

## 1:15–2:00 — Project binding and constrained Agent

Open a generic Project and go to **Automation**. Expand **Project model choices** and select the
configured Profile for Classification or Detection. Reload once to show that the locked Project
binding is durable.

In **Build with Agent**, choose the available Pipeline Builder Profile. The selector shows the Model,
Provider, health, capabilities and binding provenance. If no compatible model exists, the page shows
the inline Provider setup; it accepts only an environment-variable reference or a session-only key.

Start the Builder in **Scripted Mock** for a deterministic course run. Explain that live mode uses
the same bounded tools but drives them with the selected OpenAI-compatible Profile.

## 2:00–3:15 — repair, Dry Run and revision

Follow the Agent trace:

1. inspect Project, target Label, Skills, Node Catalog, Providers and compatible Models;
2. create a real persisted Draft;
3. attempt an incompatible binding and receive a typed Rust validation error;
4. query compatible Profiles and repair the binding;
5. connect only registered typed nodes and pass static validation;
6. run a 1–10 image sandbox Dry Run;
7. inspect bounded artifacts and measured Review workload;
8. add the controlled Crop Classification revision and Dry Run again.

The trace shows tool actions, Profile revision, tokens, cost, duration and safe errors. It does not
show API keys, Authorization headers, arbitrary reasoning text, Shell access or arbitrary URLs.

## 3:15–4:10 — human approval and immutable publication

The Agent stops at **Waiting for human**. Show the structured Draft Diff, apply or reject selected
changes, and use undo if needed. The Agent cannot publish or start a Dataset Batch.

In **Test & Activate**, inspect the sample overlay and node artifacts, then publish manually. Open the
new version and show that it freezes Model Profile ID/revision, Provider adapter/base URL, remote
model ID, capabilities, limits and generation defaults. Credential references and price are not in
the semantic snapshot.

## 4:10–5:00 — Run, Review, Replay and Export

Start a Run with the exact Published Version. A disabled or unavailable current Provider/Profile
blocks a new Run with an actionable error; AnnotAgent itself still starts and historical Runs remain
viewable. Open **Debug** to inspect node input/output/configuration/timing and Replay from a downstream
node. Review any queued result, then export through the existing schema-aware exporter.

Finish in **Settings → Usage**: passive checks create no charge record; confirmed probes and model
calls record revisioned identity and token/cost provenance without storing credentials.

## Live-conditional variant

For a real Provider, configure an environment-variable reference, run the passive connection check,
declare the exact remote Model ID and capabilities, then explicitly confirm the active probe. The
normal CI and this course demo never use a live credential or make a billable request.
