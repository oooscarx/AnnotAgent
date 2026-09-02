# Rust plugin versioning and reproducibility

Plugin versions install side by side. Tests and updates create a new model selection; they never
rewrite old Project bindings or published workflows.

A plugin-backed published model snapshot contains:

- plugin ID and semantic version;
- package SHA-256;
- Plugin API and worker protocol versions;
- model ID and Model Profile revision;
- checkpoint SHA-256 when weights are required;
- capability-contract SHA-256 and declared capabilities.

The snapshot is included in workflow content hashing. Registry references cover published
workflows, drafts, runs, replay, calibration and artifacts; any recorded reference blocks normal
uninstall. Historical metadata remains readable if an executable is later unavailable.
