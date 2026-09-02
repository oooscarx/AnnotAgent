# Model Bundle Smoke Tests

Every runnable Bundle declares a fixed, bounded smoke suite: one decodable image, one data-only
typed request, expected Artifact summary, output tolerances, and timeout. AnnotAgent replaces any
packaged Image Artifact with a fresh host-scoped image and rebinds Detection/Prompt subject
lineage; package data cannot choose live Run identities.

The test starts the exact installed Plugin process with the verified role map, checks authenticated
health/capability/model/Contract discovery, performs real inference, validates typed finite
Artifacts and lineage, then applies required kind/count/item/mask-coverage/duration tolerances.
Crash, typed error, Contract failure, empty required output, or tolerance failure becomes
`FailedSmokeTest` and never Ready.

Smoke proves compatibility and minimum executable behavior, not task accuracy. The built-in
Fixture deliberately proves only this protocol. Dataset quality still requires reviewed Dry Runs,
geometry evaluation/calibration, and human acceptance.
