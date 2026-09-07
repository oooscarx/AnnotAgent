# Product screenshots

Captured on 2026-09-07 from the running AnnotAgent GUI using the existing local workspace.
These are actual UI captures, not generated mockups. Captures target the page content or
model card so the README does not repeat browser chrome and navigation around every image.

| Image | Source | What it demonstrates |
| --- | --- | --- |
| `bhuman-data.png` | `/projects/robocup-ball/build/data` | Four imported real B-Human football images |
| `ball-labels.png` | `/projects/robocup-ball/build/labels` | Existing `ball` label and bounding-box output |
| `efficientsam-ready.png` | `/settings/plugins` | Installed EfficientSAM-Ti ONNX in Ready state, with zero published references |

Only presentation state was changed (the Runtime disclosure was collapsed). No labels,
model bindings, project names, results or success states were changed for these captures.
No paid model request was issued. The source photos remain subject to their dataset rights;
this directory contains UI screenshots, not a redistributed image dataset.

There were no retained formal Runs in this workspace at capture time, and the current Drafts
were empty. Accordingly, the README does not present a synthetic result or an empty Draft
as a successful real-world annotation pipeline. These screenshots demonstrate product setup,
not segmentation accuracy or a completed end-to-end B-Human benchmark.

Keep these editorial assets separate from `docs/execution/screenshots`, which is written by
automated acceptance tests. When updating, capture genuine state and review for sensitive
paths, credentials, readable layout and complete component boundaries before committing.
