# Model Provisioning

The normal product journey is:

1. Install the compatible Rust Plugin and review its code permissions/licenses.
2. Choose **Install compatible model**. Review publisher, exact Bundle digest, size, source,
   platform requirements, and model-license digest.
3. Explicitly accept the exact license if required.
4. AnnotAgent obtains the Catalog-pinned package, verifies it, and atomically activates it in the
   content-addressed store.
5. Compatibility binds a Model Instance to an exact Plugin model and execution provider.
6. Rust ONNX inspection checks the packaged tensor Contract.
7. A fixed Bundle smoke test runs through the real isolated Plugin process.
8. Only a publishable, enabled, Ready instance is offered to Workflow nodes.

CLI uses the same workspace Registry:

```bash
annotagent models --workspace ./workspace catalog
annotagent models --workspace ./workspace search prompted-segmentation
annotagent models --workspace ./workspace install <bundle-id>@<version> --accept
annotagent models --workspace ./workspace import ./model.annotmodel --accept
annotagent models --workspace ./workspace list
annotagent models --workspace ./workspace test <model-instance-id>
annotagent models --workspace ./workspace doctor <model-instance-id>
annotagent models --workspace ./workspace disable <model-instance-id>
annotagent models --workspace ./workspace enable <model-instance-id>
annotagent models --workspace ./workspace references <bundle-id>@<version>
annotagent models --workspace ./workspace remove <bundle-id>@<version>
annotagent models --workspace ./workspace gc
```

The TUI provides read-only `/models catalogs`, `/models bundles`, `/models instances`, `/models
references …`, and `/models doctor …` views. Pipeline Builder may inspect readiness and create a
blocked Draft, but cannot install assets or accept terms. After setup, Retry reconciles only the
Draft's unresolved bindings and preserves its identity and human edits.
