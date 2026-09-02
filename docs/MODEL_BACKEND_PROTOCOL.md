# Model Backend Protocol

The official local model boundary is the versioned Rust model-plugin process protocol. A plugin
declares one or more capability-oriented model contracts and returns validated typed Artifacts.
Remote LLM/VLM Providers remain separate; historical HTTP Vision v1 endpoint bindings remain an
explicit compatibility type.

See [Rust Model Plugins](RUST_MODEL_PLUGINS.md) and [Migration](RUST_PLUGIN_MIGRATION.md).
