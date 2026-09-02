# Expert Vision Models

New expert vision models are native Rust plugins installed through **Settings → Expert Model
Plugins**. Their manifests declare capability, typed contracts, score/geometry semantics, exact
weights, resources and license. Only enabled models with complete package, checkpoint and
installed-process evidence become Ready and enter runnable Drafts.

Providers remain the place for remote LLM/VLM APIs and credentials. **Legacy HTTP** reads existing
HTTP Vision v1 endpoint bindings but is not the new-model installation path.

See [Rust Model Plugins](RUST_MODEL_PLUGINS.md), [Product Integration](RUST_PLUGIN_PRODUCT_INTEGRATION.md)
and [Migration](RUST_PLUGIN_MIGRATION.md). The superseded external-worker guide is retained at
[`docs/legacy/python-workers/docs/EXPERT_VISION_MODELS.md`](legacy/python-workers/docs/EXPERT_VISION_MODELS.md).
