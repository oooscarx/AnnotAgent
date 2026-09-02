# Model Catalog

A Model Catalog is signed-capable JSON metadata; it contains no executable code or model bytes. An
entry pins Bundle ID/version, HTTPS URL, SHA-256, byte length, capabilities, Plugin requirements,
platform/provider resources, publisher identity, license digest, and Fixture/publishable policy.

AnnotAgent persists configured Catalogs locally. The built-in Catalog contains one Rust-generated,
non-publishable prompted-segmentation Fixture so offline CI can exercise the complete protocol. It
is not SAM and is not accuracy evidence.

Network Catalogs and downloads accept only credential-free public HTTPS. DNS results are checked;
redirects, localhost/private/link-local destinations, unbounded bodies, and digest/size changes are
rejected. Catalog identity and the verified package Manifest must agree before installation.

```bash
annotagent models catalog
annotagent models search prompted-segmentation
annotagent models show org.example.model@1.0.0
annotagent models catalog build ./entry-json --output catalog.json --catalog-id org.example.catalog
annotagent models catalog verify catalog.json
```

`catalog build` reads `ModelCatalogEntry` JSON files from the directory. Signing/distribution is an
operator step; an unsigned local Catalog is not silently presented as publisher-verified.
