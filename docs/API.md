# CLI and HTTP API

## CLI

```text
annotagent init <directory> --skill robocup
annotagent project validate <project.yaml>
annotagent import --project <project.yaml> --images <directory>
annotagent run --project <project.yaml> --provider <mock|openai_compatible> [--config file] [--limit N]
annotagent tui [--project <project.yaml>]
annotagent serve --workspace <directory> [--port 8787] [--open]
annotagent skills list
annotagent skills show robocup
annotagent history list
annotagent history show <run-id>
annotagent history export <run-id> --output <file>
annotagent history import <file>
annotagent export --project <project.yaml> --format <native|coco|yolo|yolo_segmentation|labelme> --output <directory>
annotagent doctor
annotagent demo robocup
```

## HTTP

All paths are relative to the local Axum server.

| Method | Path | Purpose |
|---|---|---|
| GET | `/api/health` | Service/workspace/database status |
| GET | `/api/skills`, `/api/skills/{id}` | Skill boundary, tools, validators, resources, template |
| GET/POST | `/api/projects` | Dashboard list or validated project creation |
| GET | `/api/projects/{id}` | Project summary |
| GET | `/api/projects/{id}/workflow-catalog` | Registry-bounded Advisor/editor context and data profile |
| GET | `/api/workflows` | Published Workflow Versions for all Projects |
| GET/POST | `/api/workflow-drafts` | List or create blank/template Drafts |
| POST | `/api/workflow-drafts/suggest` | Mock or constrained workspace-LLM Advisor suggestion |
| PATCH | `/api/workflow-drafts/{id}` | Persist an editable Draft |
| POST | `/api/workflow-drafts/{id}/dry-run` | Validate and execute up to ten selected images in an isolated sandbox |
| POST | `/api/workflow-drafts/{id}/publish` | Publish an immutable Workflow Version |
| POST | `/api/workflow-drafts/{id}/archive` | Archive and lock a Draft |
| POST | `/api/workflows/{id}/versions/{version}/clone` | Clone an immutable version to a new editable Draft |
| POST | `/api/workflows/compare` | Compare two immutable Workflow Versions |
| GET | `/api/models` | Saved workspace Model Binding |
| GET | `/api/runs` | Cross-Project Run summaries with Workflow/Skill/model context and usage |
| POST | `/api/projects/{id}/import` | Controlled workspace-folder image import |
| GET | `/api/projects/{id}/images` | Ordered image list |
| GET | `/api/projects/{id}/images/{index}/content` | Bounded workspace image content |
| POST | `/api/projects/{id}/runs` | Start one image run; optional `workflow_id` + `version` select an immutable version together |
| POST | `/api/projects/{id}/batches` | Create and asynchronously execute a durable dataset batch |
| GET | `/api/batches` | List active and terminal dataset batches |
| GET | `/api/batches/{batch}` | Durable checkpoint, progress summary, budget ledger, and ordered events |
| POST | `/api/batches/{batch}/pause` | Stop new image/node claims and persist resumable state |
| POST | `/api/batches/{batch}/resume` | Recover a paused/failed batch and execute only remaining images |
| POST | `/api/batches/{batch}/cancel` | Cancel active child runs and prevent new node claims |
| GET | `/api/runs/{run}` | Durable run summary |
| POST | `/api/runs/{run}/pause` | Pause at a safe boundary |
| POST | `/api/runs/{run}/resume` | Resume a paused run |
| POST | `/api/runs/{run}/cancel` | Propagate cancellation |
| GET | `/api/runs/{run}/events` | Historical typed events |
| GET | `/api/reviews`, `/api/reviews/{id}` | Review queue/details |
| POST | `/api/reviews/{id}/decision` | Accept/reject/delete and correction record |
| PATCH | `/api/annotations/{id}` | Validate edit and append revision |
| GET | `/api/annotations/{id}/revisions` | Revision chain |
| POST | `/api/projects/{id}/export` | Native/COCO/YOLO/LabelMe export |
| GET/PUT | `/api/settings` | Durable workspace settings; API key is write-only |
| GET | `/api/events?run_id=...` | Live SSE stream |

Errors are JSON objects with an HTTP status and a concrete message. User paths are canonicalized against the workspace before reads.

## Product DTO compatibility

Project responses include Dataset, Annotation Schema, `EnabledSkill`, active/available published `WorkflowVersion`, and `ModelBinding` fields. When no version has been published, schema v1 exposes its configured compatibility graph. Draft creation, registry-bounded Advisor suggestions, editing, sample Dry Run, publication, archive, clone, comparison, and explicit Run version selection are first-class product APIs.

When a dataset batch is active, Project responses also include `active_batch` and
`active_batch_progress`. These values come from SQLite rather than process memory, so a
restarted server can render Pending, Running, Paused, or Awaiting Review state before the
operator resumes work.

Run list summaries derive the selected immutable Workflow name/version, Skill version, provider/model binding, usage, cost, and status from persisted history. The current image Runtime records both the selected immutable version and the compatibility Skill graph it actually executes; it does not claim that the selected generic DAG replaced the compatibility executor.

Batch mutation is lease-guarded and transactional. Budget reservations include already
consumed and concurrently reserved usage before an image is claimed. Batch events use a
strictly increasing per-batch sequence and `/api/batches/{batch}` is the replay endpoint.

## SSE

Each SSE event name is the serialized `RunEventKind`; data is the complete versioned `RunEvent`. Reconnecting clients can replay durable history from `/api/runs/{id}/events`. The stream has keep-alive frames and accepts an optional run filter.

## Settings

`GET /api/settings` exposes safe provider/pricing/budget metadata, the default run provider, persistence status, settings path, and booleans indicating whether an API key is configured and persisted. It never returns the key.

`PUT /api/settings` accepts the full settings object plus optional write-only `api_key` or `clear_saved_api_key: true`. Non-secret settings are atomically stored at `<workspace>/.annotagent/settings.toml`. The key is stored per workspace in the operating system keychain and is never written to TOML or SQLite. A run request may still provide an explicit `provider`; when omitted, the saved `default_provider` is used.
