# Dataset Batch Coordinator

A Dataset Batch has its own `BatchId`; every image keeps a separate child `RunId` and audit history. Creation freezes the Project, settings, budget limits, and either the exact Published Workflow Version or the explicit legacy compatibility Workflow.

Workers claim images transactionally under a renewable lease. Claiming also reserves the configured worst-case token, request, image, and cost usage against one exact-decimal global ledger. Completion atomically releases the reservation and records actual usage. Concurrent workers therefore cannot oversell the same budget.

Persistent state includes image queue order, child Run IDs, per-image and per-node status, Artifact references, retry counters, review suspensions, runtime checkpoint, consumed/reserved budget, errors, and a monotonic Batch event sequence. Pause stops new claims; resume continues unfinished work; retry explicitly requeues failed images; cancel prevents any new image node from starting. Startup recovers orphaned leases and preserves completed work.

Selected Published Workflow identity is carried from the Project API into the Batch snapshot and then cloned into every child Run. The two-image integration test asserts each child uses `published_dag_runtime` and stores the same content hash and checkpoint. The durability gate uses 100 images with concurrency four, pauses during progress, destroys the original application owner, reopens SQLite, resumes, and finishes with exactly 100 child Runs and matching persisted usage totals.

