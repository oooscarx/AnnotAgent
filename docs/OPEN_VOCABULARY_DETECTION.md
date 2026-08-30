# Open-vocabulary Detection

`annotagent.open_vocabulary_grounding` is a domain-neutral Capability Skill. It exposes two node
operations:

- `open_vocabulary_grounding.detect` requires `OpenVocabularyDetection`;
- `open_vocabulary_grounding.ground_phrase` requires `PhraseGrounding`.

Both accept an Image plus one to 100 text queries and produce one `DetectionSetArtifact`. Each
query has a stable ID, bounded text, and an optional Project Label. Every returned Detection keeps
that query ID and label mapping, including when several queries share one model call. A valid
no-object response is an empty DetectionSet, not a failed task.

```json
{
  "queries": [
    {
      "id": "target-ball",
      "text": "a football used in a robot soccer match",
      "target_label": "football"
    }
  ],
  "max_objects": 20,
  "generation_mode": "hybrid"
}
```

The Skill owns query validation only. Model identity and endpoint come from the Model Registry;
the Provider adapter owns wire validation and coordinate conversion; Core owns typed Artifacts,
Review, and Commit. Crop remains a separate Core node.

The detection Cache Key includes the exact query array, Project Label mapping, image content
identity, model/version/protocol and node configuration. Changing a query invalidates this model
call without invalidating an unchanged specialist detector.

## Offline Mock

Bind `mock-open-vocabulary` to exercise category detection, phrase grounding, multi-query mapping,
and empty results without a key, GPU, network, or weights. Mock candidates deliberately use
`not_provided` score semantics. They are contract fixtures, not accuracy evidence.

## Visual exemplars

Visual exemplar prompts are outside the Alpha Skill schema. The UI keeps that control disabled
when live capability discovery reports `supports_visual_prompt=false`, and both Workflow static
validation and the Skill runner reject visual-prompt parameters before inference.

See [LocateAnything Backend](LOCATE_ANYTHING_BACKEND.md) for the optional local implementation and
[HTTP Vision Protocol](HTTP_VISION_PROTOCOL.md) for the untrusted Worker boundary.
