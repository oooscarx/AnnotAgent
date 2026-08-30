# Guided Project Setup

## Start AnnotAgent

```bash
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

Open **Projects → New project**.

## Four setup steps

1. **What do you want to annotate?** Choose Classification, Bounding boxes, Segmentation, or Custom; enter a Project name and a human-facing Label name. Stable workspace/task/Label IDs are generated automatically and remain under Advanced.
2. **Add data.** Enter a workspace-local PNG/JPEG file or folder. Import performs bounded image decode validation, skips content duplicates, reports corrupt/unsupported files, and copies valid images into the Project.
3. **Choose a priority.** Select Faster, Balanced, or Higher accuracy. Optional Advanced settings capture expected cost, target Review rate, local models, and offline-only operation.
4. **Recommended Automation.** Choose a registered Provider/model connection. Mock is offline and needs no key. External providers require a concrete model and a write-only key. Using the recommendation creates a real Project, imports the data, and persists a registry-bounded Advisor Draft.

## Continue in Build

- **Data** lists real Project images and import diagnostics.
- **Labels** defines annotation meaning and output shape; model choice does not belong here.
- **Automation** shows Shared Stages and per-Label Recipes. Preview, compare, then explicitly apply Advisor changes to the Draft. Node configuration autosaves. Expert Graph edits the same Draft.
- **Test & Activate** executes 1–10 real images without writing formal Annotations. A successful test shows outcomes and Full Run estimates before explicit immutable activation.

If a prerequisite is missing, the server returns a blocker and repair route. Refresh, back/forward, and Project switching preserve the exact Build step.

## Credentials

GUI keys are never returned by the API, stored in SQLite, written to logs, or placed in a keychain. The local server writes the current key to `<workspace>/.annotagent/credentials/provider-api-key` with owner-only permissions. Use Mock for the deterministic offline path.
