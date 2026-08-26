# Known Limitations

The following are real gaps, not hidden roadmap claims:

- Dataset Coordinator enumerates images with bounded concurrency and isolates each image as its own run, but it has no persisted dataset-level checkpoint/auto-resume record, no shared global budget ledger across concurrent image runtimes, and no batch-wide pause/cancel handle. Import deduplicates by hash; coordinator enumeration itself does not re-hash duplicates.
- The HTTP `POST /api/projects/{id}/runs` starts one image run. Multi-image bounded coordination is exposed by the CLI when `--limit 1` is omitted, not yet as a first-class server batch resource.
- Folder image import is implemented. AnnotAgent Native, COCO, and LabelMe annotation import are not implemented.
- The GUI edits existing bbox, keypoint, polyline, polygon and polygon-mask geometry and can append vertices. It does not yet delete individual vertices, resize bbox corners, draw a brand-new annotation from an empty canvas, compare model/refined overlays side-by-side, or provide advanced queue filters.
- TUI starts and controls a run and shows trace/usage/history summaries. It does not implement every requested command (`/init` and `/export` are CLI-only), panel focus/navigation, or two-step cancel confirmation.
- Provider failure attempts are bounded and surfaced, but failed HTTP attempts do not yet create the same full `UsageRecord` shape as successful calls when the remote supplies no usage.
- OpenAI-compatible operation uses Chat Completions with tool calls. The configured capability fields are reported, but JSON-only fallback for providers without tool calling is not implemented.
- Thumbnail/crop operations are bounded, but there is no durable thumbnail/crop cache across processes.
- Server CORS is permissive and there is no login; the server is designed for loopback single-user use only.
- COCO RLE is represented and exported, but there is no general RLE drawing/editing UI; the GUI handles polygon-based masks.
- The real Qwen-compatible smoke test reached authentication, vision upload, task-scoped tool calling, structured retry, SQLite and review on a user-provided frame. It was cancelled during a slow field-line request, so a complete real-provider six-task DAG was not verified.
- Dynamic plugins, WASM, MCP, distributed workers, cloud storage, video, training loops, vector search, and a second production Skill are intentionally outside scope.
