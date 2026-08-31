# Grounding DINO Backend

Grounding DINO is represented by `OpenVocabularyDetection` and `PhraseGrounding` Capabilities.
Queries, input/output contracts, score semantics and geometry semantics are declared in the Expert
Model Manifest; no model-branded Core node or Runtime branch is required.

The generic setup flow verifies `/health`, `/v1/capabilities`, `/v1/models`, `/v1/contracts` and a
selected-image sample. The Pipeline Builder may use the model only after it becomes `Available`.
Cold-start or fallback use is still bounded by cost, latency, domain validation and Human Review.

The repository provides a Worker scaffold preset and protocol boundary but no downloaded Grounding
DINO implementation or checkpoint. Real inference is therefore `LIVE-CONDITIONAL`.
