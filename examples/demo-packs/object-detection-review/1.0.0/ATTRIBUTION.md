# Tabletop cup and bottle demo pack

This pack contains six original, procedurally generated illustrations made for the
AnnotAgent guided demo. They are **synthetic images**, not photographs, captured
camera data, model outputs, or a benchmark dataset.

## Source and authorship

- Pack ID: `object-detection-review`
- Version: `1.0.0`
- Created: 2026-09-12
- Source: generated locally by `tools/build_pack.py`
- External visual sources: none
- People, trademarks, private data, and third-party logos: none
- Alterations: the PNG files are direct output from the generator; thumbnails are
  Lanczos downscales of the matching originals. No rotation or hidden orientation
  correction is applied.

The original images use simple geometric shapes, procedural color, shadows, and
noise. PNG metadata also identifies each asset as `synthetic_procedural` and records
the pack version and license.

## Data license

The six images, six thumbnails, and the reference-candidate JSON authored for this
pack are made available under **CC0 1.0 Universal (`CC0-1.0`)**. The person committing
these generated assets applies CC0 to the extent they hold copyright and related
rights in the generated output.

- Human-readable deed: <https://creativecommons.org/publicdomain/zero/1.0/>
- Legal code: <https://creativecommons.org/publicdomain/zero/1.0/legalcode>
- SPDX identifier: `CC0-1.0`

The generator source remains governed by the repository's software license. The
application repository currently declares MIT in Cargo metadata; this data notice
does not repair or replace any missing repository-level license text.

## Reference candidates

`references/*.json` contains supplier-authored reference boxes for only two uses:

1. the explicitly named **preset-candidate experience**, where the product states
   that no model inference occurred; and
2. offline evaluation of a model result against the pack reference.

These files must not be placed in prompts, model requests, visual-model inputs, or
real-mode caches. They do not represent a previous model run. They also do not mean
that the current user has accepted the annotations. The user still reviews each
image and confirms positive, negative, or excluded status before a dataset can be
packaged.

`desk_06` intentionally includes a loose preset candidate around `cup-04` that also
covers its cast shadow and neighboring bottle area. Its checked physical-object box is
stored separately, so the user can make a visible, explainable boundary correction.
This is a designed teaching case, not evidence of model error rates.

## Quality scope

The pack is intended to exercise an annotation and delivery workflow. Six synthetic
images are not enough to train or evaluate a useful production detector. File/hash,
coordinate, split, review, and package checks do not establish annotation accuracy or
real-world model quality.
