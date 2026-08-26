# Architecture

## Composition

The binary is the composition root. It registers `RoboCupSkill`, providers, SQLite, the shared application service, TUI, and HTTP server. Dependencies point inward: Core has no database, HTTP, provider implementation, or RoboCup dependency.

| Layer | Crates | Responsibility |
|---|---|---|
| Contracts | `annotagent-core` | Checked types, schemas, events, extension traits, usage and budget types |
| Execution | `annotagent-runtime`, `annotagent-provider`, `annotagent-image-tools` | Agent loop, registries, control, model adapters, bounded image operations |
| Domain | `annotagent-skill-robocup` | RoboCup DAG, prompts, tools, validators, refiner, review policy |
| Persistence/output | `annotagent-storage`, `annotagent-export` | SQLite audit history, revisions, correction memory, dataset formats |
| Delivery | `annotagent-application`, `annotagent-server`, `apps/annotagent`, `web` | Use cases, HTTP/SSE, CLI/TUI, React review UI |

## Runtime topology

```text
DatasetCoordinator (bounded concurrent images)
  └─ LocalApplication::start_run_image_path
       └─ AgentRuntime::run_image
            ├─ Skill TaskGraph topological order
            ├─ ContextManager task-focused messages
            ├─ VisionModelProvider
            ├─ ModelRegistry / NodeRegistry
            ├─ typed VisionArtifact + hybrid backends
            ├─ ToolRegistry (task scope + JSON Schema)
            ├─ AnnotationValidator / AnnotationRefiner
            ├─ ReviewPolicy
            ├─ RuntimeStore (SQLite)
            └─ EventBus → Application broadcast → TUI / SSE
```

Each image currently has its own durable run ID. This makes failure and audit history independently addressable. Dataset coordination is in the application crate because enumeration and workspace policy are application concerns; the image loop remains reusable and filesystem-agnostic in Runtime.

The implemented hybrid model boundary is described in [Hybrid vision execution](HYBRID_VISION.md). Auxiliary detectors and segmenters supply typed Artifacts; they do not bypass Runtime validation, provenance, review, or commit.

## State and persistence

`RunControl` is the process-local transition gate, while SQLite is the durable source of truth. Project DTOs expose `active_run` separately from `last_run`; task outcomes include `SucceededEmpty`, and stale worker leases reconcile to `Interrupted`. Pause waits at safe boundaries; cancellation uses `CancellationToken` and does not become failure. Versioned typed events are persisted before broadcast. SQLite uses 24 explicit tables from `migrations/0001_initial.sql`; images remain files and only relative metadata/hash references belong in history.

## Trust boundaries

- Project roots and HTTP import paths are canonicalized under the configured workspace.
- Symlinks are not followed during enumeration.
- Images have a decode-pixel bound and model copies are resized.
- Model responses are untrusted: tools must be registered, applicable to the task, schema-valid, and within the active run.
- Model-visible images are data, never instructions.
- Secrets are read from an environment variable or held in process memory only.

## Intentional exclusions

Compile-time Rust registration and YAML resources are sufficient for this release. Dynamic libraries, WASM/plugin marketplaces, MCP, distributed scheduling, authentication, and a second production Skill would add operational surface without improving the course scenario.
