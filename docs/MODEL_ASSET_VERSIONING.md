# Model Asset Versioning

Bundle ID plus semantic version is immutable: the same identity cannot later map to different
bytes. Content is stored by whole-Bundle SHA-256, while every model role has its own digest.

A Published Workflow freezes Plugin ID/version/package digest/API/protocol/model, Bundle
ID/version/digest, Model Instance/Profile revision, execution provider, capability Contract hash,
and every role digest. Run admission resolves that exact instance and re-hashes files before child
startup. A later Catalog entry, Plugin update, or same-family checkpoint cannot silently alter an
existing Version or Replay.

Disable prevents new use without erasing history. Remove is blocked while a Workflow reference
exists. Historical snapshots remain readable if local content is later intentionally removed.
