<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/product/readme/brand-dark.svg">
  <img src="docs/product/readme/brand-light.svg" alt="AnnotAgent monochrome logo" width="800">
</picture>

# AnnotAgent

**A visual data agent, from raw images to training data packages.**

Specify the images, what to annotate, and what you intend to train. An LLM organizes vision models and processing tools; you inspect and correct results when needed, then export the supported format. The model generating annotations need not be the model you plan to train.

> Alpha · Local, single-user application. Model setup, sample validation, formal review and delivery checks are required. Complete training packages currently support the Ultralytics YOLO detection preset only. AnnotAgent does not train models.

[Build and start](#build-and-start) · [Configure models](#configure-a-provider-endpoint-and-api-key) · [Runnable demos](#two-runnable-demos) · [Documentation](docs/product/readme/GUIDE.md) · [简体中文](README.md)

![Real B-Human football sample with project navigation, conversation, a bounding-box candidate and pending human assistance](docs/product/readme/workspace.png)

*Real saved sample, with review warnings and read-only state preserved. This is not evidence of accepted formal annotations or a completed training package. The capture service is not the final integration build. See the [asset manifest](docs/product/readme/ASSET_MANIFEST.json).*

## First experience

![Six original synthetic tabletop images covering cups, bottles, multiple objects, a negative image and a boundary-edit case](examples/demo-packs/object-detection-review/1.0.0/thumbnail.png)

The repository includes a six-image offline tabletop example with `cup` and `bottle`
labels and an Ultralytics YOLO detection package target. Every image is an explicitly
identified original procedural illustration. It demonstrates the workflow; it is not
photographic data or a training benchmark. [See the assets, fixed splits and license](examples/demo-packs/object-detection-review/1.0.0/README.md).

- **Try with my model** uses an already configured and approved model. Results, tokens and costs come from that live request.
- **No-setup experience: preset candidates** makes no model request. Inspect and edit the supplied candidates, then review every image and package the result.

Preset candidates must not enter live-mode prompts, inputs or caches. Structural
validation is not annotation-accuracy validation. A product screenshot will be added
only after the final integrated build exists; the asset preview is not presented as UI.

## One task: from images to a package

Consider detecting `ball` in match images for Ultralytics YOLO detection. These four steps describe the intended progression of one task. The real screenshot covers the sample stage; later review and delivery mechanisms are supported by code and separately identified TEST evidence.

1. **Describe the goal.** Select the images, define annotation rules and specify the training target. Fill missing fields in the task. Saving an intent does not authorize external requests.
2. **Compose and try.** The planning model proposes a workflow using available capabilities. A real football sample executed visual localization and EfficientSAM prompted segmentation. Check execution evidence, not the installed-model list. Inspect samples before approving formal processing.
3. **Provide human assistance.** Sample feedback helps refine the method. Formal results require object review and separate whole-image decisions for completeness, negatives or exclusions. Accepting one box does not establish image completeness.
4. **Validate and package.** For the supported preset, Rust produces the ZIP from frozen image scope, class mapping, review records and split policy. Meet readiness conditions and confirm packaging. One successful API call is not a completed dataset.

[Task and review guide](docs/product/readme/GUIDE.md) · [Separate TEST review and delivery evidence](docs/product/readme/EVIDENCE.md)

## More than one vision API call

- **Compose models and tools for the task.** An LLM proposes steps; Rust checks nodes, data types and bindings before publication. Detection, cropping, local recognition and segmentation can be combined. [Workflow model](docs/WORKFLOW_MODEL.md).
- **Inspect origins and revisions.** Candidates reference images, runs, nodes and source artifacts. Sample feedback and formal annotation edits are distinct. Historical tasks and execution records remain inspectable. [Task and review guide](docs/product/readme/GUIDE.md).
- **Export deterministically.** Exporters handle coordinates, directories, mappings and validation. Structural validity is not annotation accuracy. [Export scope](docs/product/readme/GUIDE.md#导出范围).

These are mechanisms specialized for visual annotation, not a claim that general-purpose agents cannot implement them.

## Build and start

The recommended route is **building from source**. The repository pins Rust **1.98.0**. Node.js must be **20.19+ or 22.12+**; CI uses Node.js 22. Use npm and the committed `package-lock.json`. Native vision inference also needs the matching Plugin, compatible weights and platform runtime.

```bash
git clone git@github.com:oooscarx/AnnotAgent.git
cd AnnotAgent

# Install and build the production web application
npm --prefix web ci
npm --prefix web run build

# Compile the complete Rust workspace
cargo build --locked --workspace --all-features
```

Start the production application with a **writable** workspace directory:

```bash
cargo run --locked -p annotagent -- \
  serve --workspace ./workspace --port 8787 --open
```

If a browser does not open, visit [http://127.0.0.1:8787/projects](http://127.0.0.1:8787/projects). Use another port, such as `--port 8788`, if the address is already in use. Do not point concurrent servers at the same workspace. SQLite data, tasks, Provider configuration, credential references, annotations and exports live under the selected workspace, so it must be writable and must not be committed to Git.

Run `npm --prefix web run dev:ui-preview` only to inspect Fixture UI states. **UI Preview does not connect to a real workspace, call a model or prove an end-to-end run.**

### Configure a Provider endpoint and API key

1. Open **Settings → Providers** (`/settings/providers`) and select “Add Provider”. Choose a preset or a custom OpenAI-compatible endpoint.
2. Enter a display name and the Provider Base URL, for example `https://api.example.com/v1`. Do not append `/chat/completions`, credentials, a username or query parameters.
3. Save the account, then edit it and select one credential storage method:
   - **Local workspace file (recommended):** paste the API key and save it. The server writes it with owner-only permissions under `.annotagent/credentials/` in the workspace. It is Git-ignored, survives restarts and is not stored in the OS Keychain or browser storage.
   - **Server environment-variable reference:** export the variable before starting AnnotAgent, then enter only its name, such as `ANNOTAGENT_API_KEY`, in the UI. Do not paste the secret into the variable-name field.
   - **Current server process only:** useful for temporary testing and cleared on restart.
4. Explicitly check the connection or discover models. A returned remote model ID proves only that the Provider listed it, not that inference works.

Environment-reference example:

```bash
export ANNOTAGENT_API_KEY='replace-with-your-own-key'
cargo run --locked -p annotagent -- \
  serve --workspace ./workspace --port 8787 --open
```

Never put a real secret in this README, TOML, a project name or an endpoint. The application reports whether a credential exists and never reads the saved value back into the page.

### Register Agent and vision models

A Provider is an account connection; a Model Profile is the concrete model that can be selected. Configure both:

1. Under **Settings → Agent models** (`/settings/agent-models`), add a planning model, select its Provider, enter the exact remote model ID and declare `text` input plus `text_generation`. Enable tool calls, Structured Output or JSON Schema only when the Provider actually supports them.
2. Under **Settings → Vision & plugins** (`/settings/vision-models`), add a VLM with `image` input and only the capabilities it really has, such as Vision Language, Object Detection, Open-vocabulary Detection or Phrase Grounding.
3. Save and enable the profile, then run an explicit test. Manually declared capabilities remain user claims until a successful probe supplies evidence.
4. The model selected in the task Composer interprets and plans the request. Image inference uses the vision model bound to a Workflow node. Changing the Agent model does not silently replace an in-flight Workflow's vision model.

Local expert models such as EfficientSAM are managed on the same Vision & plugins page. `Plugin installed`, `Model Instance Ready`, and **actually executed in this Pipeline** are different states. Check the task trace and Artifact provenance.

## Two runnable demos

### Demo 1: inspect preset candidates and export a YOLO ZIP offline

This demo requires no API key and makes no model request. Use a fresh workspace so existing projects are untouched:

```bash
cargo run --locked -p annotagent -- \
  serve --workspace ./workspace-demo --port 8789 --open
```

Open `/projects` and choose the preset-candidate option in “First experience”. AnnotAgent creates a separate Demo Project and Task with the six bundled synthetic `cup` / `bottle` images. Review every object and whole image, repair a boundary or missing box, and generate an Ultralytics YOLO Detection ZIP after readiness checks pass. The UI identifies the run as having no model request; preset candidates are not model-accuracy evidence.

### Demo 2: annotate footballs with real models

Prepare 1–5 match images you are authorized to process, create a project and import them. Configure a planning model and an image-capable VLM. For boundary refinement, also install a compatible EfficientSAM Plugin and Ready Model Instance. Start a new task with this goal:

> Annotate only the football (`ball`) in the selected project images with a tight bounding box. Exclude robots, shoes, white lines and penalty spots. Build a real plan and test no more than three samples before any full processing. Generate candidates with the VLM; when coordinates are unreliable, inspect the local target and use EfficientSAM only when compatible and Ready. Keep geometry validation and human review. Do not use mock or fixture results. After sample approval, process these five images, review each image and export an Ultralytics YOLO Detection ZIP.

The real sequence is: save images and goal → approve the model, image, destination and cost scope → build and test samples → inspect terminal candidates and trace → approve formal processing → review every image → generate and download the ZIP. A prompt or installed-model list does not prove SAM ran. Its node and Model Instance must appear in the execution trace and the final result must retain matching Artifact provenance.

Remote Providers receive the authorized images and text and may charge for requests. Unknown cost must remain unknown, not zero. If a request has an unknown remote outcome, inspect that request's trace before attempting another billable call.

## Development checks

Run the core checks used by CI before submitting changes:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --locked --workspace --all-features

npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
```

Browser E2E uses `npm --prefix web run test:e2e` and requires its test server and Playwright browser prerequisites. See [development documentation](docs/DEVELOPMENT.md) for architecture, TUI and offline CLI demos. Commands actually run for the product documentation are recorded in the [validation record](docs/product/readme/VALIDATION.md).

## Models and exports

| Role | Current scope |
| --- | --- |
| Agent planning model | Configurable model capabilities and OpenAI-compatible Providers. Planning is distinct from visual execution. |
| Vision model | Registered VLM, detection, classification and segmentation implementations, subject to their protocols and bindings. |
| Local Plugin / Model Instance | A Plugin implements execution; an Instance binds weights and contracts. Registered, installed, Ready and actually executed are distinct states. |
| Annotation export | Native, COCO, YOLO and LabelMe, subject to schema compatibility and reported information loss. |
| Complete training package | `ultralytics_yolo_detection`: original images, YOLO TXT, `data.yaml`, README and provenance, split, exclusion and validation records. Other annotation formats do not imply complete package support. |

EfficientSAM-Ti has real macOS ARM64 CPU evidence. Other models and platforms require separate verification. Ready is not an accuracy guarantee. [Model integration](docs/PROVIDER_MODEL_REGISTRY.md) · [Local model requirements](docs/REAL_MODEL_RELEASE.md).

## Technical responsibilities

<picture>
  <source media="(max-width: 600px) and (prefers-color-scheme: dark)" srcset="docs/product/readme/flow-mobile-dark.svg">
  <source media="(max-width: 600px)" srcset="docs/product/readme/flow-mobile-light.svg">
  <source media="(prefers-color-scheme: dark)" srcset="docs/product/readme/flow-dark.svg">
  <img src="docs/product/readme/flow-light.svg" alt="Conceptual responsibilities: task inputs, LLM planning, Rust execution, vision tools, human revision and deterministic export" width="100%">
</picture>

*Conceptual responsibilities, not the actual pipeline for every image. Rust controls execution, validation and export; React/TypeScript provides interaction; SQLite and local files retain records.*

## Limits, privacy and contributing

- Local, single-user image workflows; no automatic training, video annotation or cloud collaboration. Do not expose the local server directly to the internet.
- **Remote Providers receive the images or text required by authorized requests**, even though the workspace is local. Original metadata may contain private information; universal metadata removal is not promised.
- Stopping does not guarantee immediate remote termination or refunds. Unknown outcomes are not successes or grounds for blind retries. Continuation depends on authorization, current state and remaining budget.
- Automatic packaging needs an integrated server-side authorization and trigger path. Delivery immediately after the last review is not currently promised. Mask representation and editing have format limits.
- Validate quality on representative images. Geometry and file checks do not prove the absence of missed or incorrect labels.

[Known limitations](docs/KNOWN_LIMITATIONS.md) · [Security](docs/SECURITY.md) · [Development and contributions](docs/DEVELOPMENT.md) · [Issues](https://github.com/oooscarx/AnnotAgent/issues)

License: Cargo metadata declares MIT, but no root LICENSE text was found. Maintainers need to resolve this before redistribution. Model weights and datasets have their own terms.
