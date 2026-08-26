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
DatasetCoordinator (persistent queue + lease + global budget)
  ├─ SQLite batch/image/checkpoint/event state
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

Each image has its own durable child Run ID, while the Dataset batch has an independent
`BatchId`. The coordinator claims images under a renewable worker lease and atomically reserves
global token/request/image/cost budget before starting work. Completed images are never reclaimed;
failed images require an explicit retry transition. Per-image checkpoints preserve node status,
Artifact references, retry counters, review suspensions, and the child Runtime summary. On server
startup, orphaned leases are recovered and unfinished images return to Pending without touching
completed commits.

The implemented hybrid model boundary is described in [Hybrid vision execution](HYBRID_VISION.md). Auxiliary detectors and segmenters supply typed Artifacts; they do not bypass Runtime validation, provenance, review, or commit.

## State and persistence

`RunControl` is the process-local child-Run transition gate, while SQLite is the durable source of truth. Project DTOs expose `active_batch`, `active_batch_progress`, `active_run`, and `last_run`; task outcomes include `SucceededEmpty`, and stale child Runs reconcile to `Interrupted`. Batch pause stops new claims and allows already-running child work to checkpoint; cancellation propagates only to that batch's child Runs. Versioned typed Run events and monotonic Batch events are persisted before display. SQLite uses 28 explicit tables across migrations v1–v3; images remain files and only workspace-relative paths and typed references belong in batch state/history.

## Trust boundaries

- Project roots and HTTP import paths are canonicalized under the configured workspace.
- Symlinks are not followed during enumeration.
- Images have a decode-pixel bound and model copies are resized.
- Model responses are untrusted: tools must be registered, applicable to the task, schema-valid, and within the active run.
- Model-visible images are data, never instructions.
- Secrets are read from an environment variable or held in process memory only.

## Intentional exclusions

Compile-time Rust registration and YAML resources are sufficient for this release. Dynamic libraries, WASM/plugin marketplaces, MCP, distributed scheduling, authentication, and a second production Skill would add operational surface without improving the course scenario.
