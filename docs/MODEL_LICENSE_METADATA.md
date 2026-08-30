# Model License Metadata

Model license facts are versioned Registry metadata, not a global claim about a model family.
AnnotAgent records the terms for the concrete code checkout and weight/checkpoint selected by the
operator.

## Stored fields

Each Model Descriptor can retain:

- code license and weight license;
- official source URL;
- commercial-use and redistribution status (`allowed`, `restricted`, or `unknown`);
- operator notes and whether the source was verified as official;
- architecture, model version, checkpoint SHA-256, training-dataset version and label space.

Models and Detection Worker Settings display these facts. They are informational metadata, not
legal advice and not an automatic permission decision. `unknown` is rendered as unknown; the UI
does not infer commercial permission from an open-source package license.

## Enable and publication boundary

A Worker profile marked `requires_checkpoint_metadata` cannot be enabled until architecture,
model version, checkpoint SHA-256, training-dataset version, label space and concrete weight-license
text are present. Registry validation also rejects malformed hashes, inconsistent contracts and an
unverified license claim without an official source.

Published Workflow Versions freeze the selected Model Descriptor. A later Settings edit does not
rewrite historical Runs. Operators must publish a new Workflow Version to use a new checkpoint or
new terms.

The default LocateAnything profile records the official restricted non-commercial model terms.
The RF-DETR profile records package provenance but leaves checkpoint permission unknown until the
operator identifies the exact weights, because RF-DETR variants do not all share one weight
license.
