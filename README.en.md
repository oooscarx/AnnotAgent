<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/product/readme/brand-dark.svg">
  <img src="docs/product/readme/brand-light.svg" alt="AnnotAgent monochrome logo" width="800">
</picture>

# AnnotAgent

**A visual data agent, from raw images to training data packages.**

Specify the images, what to annotate, and what you intend to train. An LLM organizes vision models and processing tools; you inspect and correct results when needed, then export the supported format. The model generating annotations need not be the model you plan to train.

> Alpha · Local, single-user application. Model setup, sample validation, formal review and delivery checks are required. Complete training packages currently support the Ultralytics YOLO detection preset only. AnnotAgent does not train models.

[Get started](#get-started) · [Example task](#one-task-from-images-to-a-package) · [Documentation](docs/product/readme/GUIDE.md) · [简体中文](README.md)

![Real B-Human football sample with project navigation, conversation, a bounding-box candidate and pending human assistance](docs/product/readme/workspace.png)

*Real saved sample, with review warnings and read-only state preserved. This is not evidence of accepted formal annotations or a completed training package. The capture service is not the final integration build. See the [asset manifest](docs/product/readme/ASSET_MANIFEST.json).*

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

## Get started

The recommended route is **building from source**. The repository pins Rust **1.98.0**. Use Node.js **22.12 or later**, npm and the committed lockfile. Native inference also needs compatible platform dependencies and weights.

```bash
npm --prefix web ci
npm --prefix web run build
cargo run --locked -p annotagent -- serve --workspace ./workspace --open
```

Open the [local workspace](http://127.0.0.1:8787). Create a project and task. Configure your Provider endpoint, API key and models in Settings. Supply the image scope, annotation goal and training target; follow the task prompts to confirm samples and formal processing. Resolve missing capabilities and return to the original task.

- **Source mode** connects the Rust service and local workspace. Loading the page does not verify inference.
- **UI Preview** uses development preview data and is not a real model execution environment. See [development documentation](docs/DEVELOPMENT.md).
- **Real inference** requires your configured Provider or compatible local weights, plus confirmation of request scope and costs. Production weights are not all bundled.

See the [validation record](docs/product/readme/VALIDATION.md) for commands actually run for this documentation change.

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
