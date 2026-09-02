# Real Prompted-Segmentation Delivery Alpha Plan

Last updated: 2026-09-03 CST

## Goal

Ship at least one non-Fixture `PromptedSegmentation` Model Bundle that a normal user can install
from the curated Catalog and execute through the Rust Plugin/ONNX Runtime path on the current
macOS ARM64 host without Python, conversion, or raw model-file upload.

## Milestones

| Milestone | Deliverable | Required evidence |
| --- | --- | --- |
| M0 | Reproduce the Fixture-only gap and freeze the baseline | platform, Registry/Catalog state, failing-path regression, full baseline |
| M1 | Audit at least three official candidates and select one | source, licenses, immutable assets, tensor interface, platform decision |
| M2 | Add controlled Rust Recipe and trusted local Catalog | audit/fetch/build/verify commands, fixed hashes, real `.annotmodel` outside Git |
| M3 | Add the matching Rust Plugin and pass real inference | graph inspection, real box prompt, non-empty mask, refined bbox, report |
| M4 | Deliver normal-user one-click installation | real Catalog card, source/license review, truthful progress/errors, browser E2E |
| M5 | Close Pipeline Builder and Runtime lineage | retry saved Draft, Geometry Safety, Run Debug, Review, Replay, restart |
| M6 | Produce release artifacts and run the complete matrix | local Catalog, release metadata, cross-platform truth, full Rust/Web checks |

## Candidate order

1. EfficientSAM-Ti split ONNX from the official EfficientSAM project-linked author Space.
2. MobileSAM ViT-T from its official repository.
3. Meta SAM 1 ViT-B.
4. Meta SAM 2.1 Tiny remains a Labs candidate unless an official directly executable asset exists.

The first candidate is accepted for delivery only after its exact graph loads on this host and its
official contract is compatible with a Rust implementation. It becomes Supported only after M3
real inference passes. Failure moves work to the next audited candidate rather than weakening the
acceptance gate.

## Non-negotiable boundaries

- No Python or subprocess conversion in install, smoke, run, or replay.
- No model weights in Git.
- No automatic license acceptance.
- No Fixture selection in publishable Workflows.
- No raw encoder/decoder upload in the normal user path.
- No push, remote mutation, credential use, history rewrite, or destructive checkout.
