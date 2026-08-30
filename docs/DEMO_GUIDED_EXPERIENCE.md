# Guided Experience Offline Demo

This demo exercises the product journey without an API key, external model, or GPU.

## Launch

```bash
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

## Demo script

1. Choose **Projects → New project**.
2. Select **Find objects**, name the Project `Offline ball demo`, and name the object `Ball`.
3. Add one or more PNG/JPEG images. For a repository sample, use `examples/robocup/images/synthetic-robocup.png`.
4. Select **Balanced** and choose the **Mock** Provider.
5. Use the recommendation. Confirm that Project Overview shows one next action and a Journey.
6. Open **Build → Automation**. Inspect the readable Shared detector and Ball Label Pipeline, open **Edit automation**, and choose **Apply Detect & Crop template**. The Draft autosaves `shared detector → filter → Core Crop → Artifact Cache` while bounding-box Commit remains on the filtered DetectionSet. Keep Expert Graph closed.
7. Open **Test & Activate**, test one image, inspect Results and Full Run Estimate, then activate the immutable Version.
8. Return to Project Overview and run the Dataset.
9. Open the new Run. Confirm Results shows one editable ball box, one linked Crop, and one structured annotation-list entry. Open Debug to inspect the real node checkpoint, then return to Results.
10. Resolve any Review item with Accept & next or Reject & next.
11. Open Export, choose a compatible configured format, export, and reload the completion page.

The deterministic command-line examples remain available:

```bash
cargo run -p annotagent -- demo generic-classification
cargo run -p annotagent -- demo generic-detection-crop
cargo run -p annotagent -- demo robocup-ball
```

## Automated version

```bash
npm --prefix web run test:e2e
```

The isolated browser suite creates its own generic Project, imports a real image, publishes and runs a Mock Classification Workflow, creates and executes a real Mock YOLO → Filter → Crop → Artifact Cache Workflow, verifies bbox/Crop parent linkage, completes Review and Export, interrupts and restores SSE, and audits the full journey at 1024 px and 720×450.
