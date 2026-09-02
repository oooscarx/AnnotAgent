# `.annotmodel` Format

The format is a deterministic ZIP archive with `model-bundle.toml` at its root. Manifest schema
version `1` declares:

- Bundle ID, semantic version, family, architecture, format, variant, capabilities, and whether it
  is a non-publishable Fixture;
- each model file's generic role, descendant path, exact size, SHA-256, and external-data files;
- compatible Plugin ID/version/model, required roles, and Plugin model Contract hash;
- versioned tensor Contract and optional transform documents;
- upstream source/revision/digests, exporter/version/opset, runtime providers/resources;
- exact license documents and permissions;
- fixed test input, expected summary, tolerances, and timeout.

The archive accepts only Manifest-referenced files plus its checksum/signature metadata. Packing is
stable in path order with normalized metadata. Verification rejects unknown/missing entries,
absolute or traversing paths, case collisions, symlinks, duplicate names, expansion limits, and any
hash/size mismatch. Extraction repeats the checks before atomic activation.

Developer commands:

```bash
annotagent models bundle pack ./bundle-source --output model.annotmodel
annotagent models bundle inspect model.annotmodel
annotagent models bundle verify model.annotmodel
```

Packing performs no export or conversion. See [Publishing](MODEL_BUNDLE_PUBLISHING.md).
