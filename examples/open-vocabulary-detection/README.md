# Open-vocabulary Detection Demo

This Generic Project enables only `annotagent.open_vocabulary_grounding`; it has no RoboCup or
model-brand dependency. Add PNG/JPEG files under `images/`, open the Project's Automation page,
and choose **Find objects by description**.

Bind `mock-open-vocabulary` for a fully offline contract run. To use a local LocateAnything
installation, enable `locate-anything-local` under Settings, start the tracked Worker, test its
live capabilities on Settings → Models, and bind that registered model to the Grounding node.

The template always includes Human Review because the shipped LocateAnything adapter reports
`score_semantics=not_provided`; no default confidence is inserted.
