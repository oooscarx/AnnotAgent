# Hybrid Detection Workflows

AnnotAgent can combine open-vocabulary models, specialist detectors, domain validators, and human
review into versioned annotation pipelines. The composition is capability-driven: Core contains no
branch for a detector brand or a domain Label.

## Recommended recipes

Cold start, when no trained detector covers the target Label:

```text
Image → Open-vocabulary Detection → Filter → Crop → Classification → Review → Commit
```

Specialist first, after a compatible trained detector is registered:

```text
Image → Object Detection → Evidence Gate
  accepted ───────────────────────────────────────────→ Commit
  fallback → Open-vocabulary Detection → Match → Verify → Review / Commit
```

Two independent detectors may also run eagerly and feed `core.match_detection_sets`. Candidate
Clusters retain every source box, label/query identity and score semantic. `core.evidence_gate`
persists the reason for accept, fallback, review or explicit reject.

## Cache and Replay

Detection nodes emit a Cache Key derived from the input image content identity, model ID, model
version, checkpoint SHA-256, backend protocol version, model configuration, node-configuration
hash, query text, Project Label mapping and enabled Skill versions.

- A shared detector stage has one graph node and executes once per image even when several Label
  Pipelines consume it.
- Repeating an identical cacheable detector input in one executor yields `Cached` with zero model
  usage.
- Editing only Evidence Gate configuration does not invalidate upstream detector keys.
- Editing an open-vocabulary query invalidates that detector key while the unchanged specialist key
  stays valid.
- Editing the specialist model version, checkpoint, protocol or class mapping invalidates its key.

Replay starts at the selected node and preserves completed ancestors from the persisted checkpoint.
Replaying a gate or classifier does not execute upstream detectors. Replay is a sandbox operation:
it reports preserved/re-executed nodes and cannot create a duplicate committed Annotation.

The current executor cache is process-local. Persisted checkpoints provide cross-process Replay;
durable cross-process reuse of arbitrary model outputs is not claimed.

## Failure behavior

A valid empty DetectionSet is evidence, not an exception. Invalid coordinates, non-finite scores,
model/capability/version spoofing, undeclared labels and oversized responses fail closed at the
Worker boundary. Timeout and cancellation are distinct structured outcomes. An opted-in fallback
may recover from a primary Worker interruption once; otherwise the primary evidence is preserved
and the Run stops at Review rather than entering an unbounded retry loop.
