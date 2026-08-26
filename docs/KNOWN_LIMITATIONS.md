# Known Limitations

These are real gaps, not roadmap claims:

- Project schema v1 configures exactly one Skill. The product DTO permits `enabled_skills: []` or multiple entries for forward compatibility, but persistence, project creation, and Runtime do not yet execute zero-Skill or multi-Skill Projects.
- Runtime derives one compatibility Workflow from Project tasks and the active Skill graph. Multiple Workflow drafts, immutable published snapshots, default-version selection, arbitrary typed mixed graphs, Run-time Workflow selection, LLM suggestion, human graph editing, static draft validation endpoints, dry run, and publish are not implemented.
- Existing Run history does not persist a first-class Workflow snapshot/version or per-node model binding. API summaries report the compatibility Workflow version and the saved provider/model truthfully.
- Dataset Coordinator has bounded concurrency but no persisted dataset checkpoint/auto-resume record, shared global budget ledger, or batch-wide pause/cancel handle. Import deduplicates by hash; enumeration does not re-hash duplicates.
- HTTP starts one image Run. Multi-image coordination is CLI-only when `--limit 1` is omitted.
- Folder image import exists; AnnotAgent Native, COCO, and LabelMe annotation import do not.
- The GUI edits existing bbox, keypoint, polyline, polygon, and polygon-mask geometry and can append vertices. It cannot yet delete individual vertices, resize bbox corners, draw from an empty canvas, compare overlays side by side, or apply advanced queue filters.
- TUI starts and controls a Run and shows trace/usage/history. `/init` provides the real CLI invocation rather than creating files inside the TUI; `/export`, panel focus/navigation, and two-step cancel confirmation remain CLI/manual operations.
- Failed provider HTTP attempts cannot always record full usage when the remote returns none. JSON-only fallback for compatible providers without tool calling is absent.
- Thumbnail/crop operations have bounds but no durable cross-process cache.
- Server CORS is permissive and there is no login; deployment is loopback single-user only.
- COCO RLE is represented/exported but not generally drawable/editable in the GUI.
- A real Qwen-compatible smoke test reached authenticated vision/tool/retry/persistence/review behavior but a complete six-task provider run has not been verified.
- Dynamic plugins, WASM, MCP, distributed workers, cloud storage, video, training loops, vector search, and a second production Skill are outside the current implementation.
