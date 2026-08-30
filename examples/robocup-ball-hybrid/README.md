# RoboCup Ball Hybrid

This is the live configuration for the capability-bound RoboCup Ball workflow. Add the three
Registry model ids named in `project.yaml` under Settings → Models, import images into `images/`,
then create a Workflow from `robocup.ball.specialist_with_open_vocab_fallback`.

The template itself contains no model brand or concrete model id. During Draft creation,
AnnotAgent resolves `capability.object_detection`, `capability.open_vocabulary_detection`, and
`capability.classification` from the Project configuration. Validate, Dry Run, inspect every node
Artifact, and publish only after the bindings are healthy.

The specialist fast path commits clean evidence without calling the fallback. Empty, low-score,
domain-risk, or correction-risk evidence can invoke one bounded open-vocabulary request. A
single-source fallback or geometry conflict goes through candidate projection, Crop, crop
classification, and then Commit, Reject, or Human Review.
