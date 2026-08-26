# CLI and HTTP API

## CLI

```text
annotagent init <directory> --skill robocup
annotagent project validate <project.yaml>
annotagent import --project <project.yaml> --images <directory>
annotagent run --project <project.yaml> --provider <mock|openai_compatible> [--config file] [--limit N]
annotagent tui --project <project.yaml>
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
| POST | `/api/projects/{id}/import` | Controlled workspace-folder image import |
| GET | `/api/projects/{id}/images` | Ordered image list |
| GET | `/api/projects/{id}/images/{index}/content` | Bounded workspace image content |
| POST | `/api/projects/{id}/runs` | Start one image run with process settings |
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
| GET/PUT | `/api/settings` | Process settings; temporary key is write-only |
| GET | `/api/events?run_id=...` | Live SSE stream |

Errors are JSON objects with an HTTP status and a concrete message. User paths are canonicalized against the workspace before reads.

## SSE

Each SSE event name is the serialized `RunEventKind`; data is the complete versioned `RunEvent`. Reconnecting clients can replay durable history from `/api/runs/{id}/events`. The stream has keep-alive frames and accepts an optional run filter.

## Settings

`GET /api/settings` exposes safe provider/pricing/budget metadata plus a boolean indicating whether a temporary key exists. `PUT` accepts a full settings object and optional `temporary_api_key`. The key is removed before validation/storage and held only by the current `ServerState` process.
