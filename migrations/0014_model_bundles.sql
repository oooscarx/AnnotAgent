CREATE TABLE IF NOT EXISTS model_catalogs (
    catalog_id TEXT PRIMARY KEY,
    schema_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    catalog_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_catalog_entries (
    catalog_id TEXT NOT NULL,
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    bundle_sha256 TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    PRIMARY KEY (catalog_id, bundle_id, bundle_version),
    FOREIGN KEY (catalog_id) REFERENCES model_catalogs(catalog_id)
);

CREATE TABLE IF NOT EXISTS model_bundles (
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    bundle_sha256 TEXT NOT NULL,
    status TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    installed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (bundle_id, bundle_version),
    UNIQUE (bundle_sha256)
);

CREATE TABLE IF NOT EXISTS model_bundle_files (
    bundle_sha256 TEXT NOT NULL,
    file_role TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    file_sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    PRIMARY KEY (bundle_sha256, file_role)
);

CREATE TABLE IF NOT EXISTS model_bundle_contracts (
    bundle_sha256 TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    contract_sha256 TEXT NOT NULL,
    contract_json TEXT,
    PRIMARY KEY (bundle_sha256, contract_id)
);

CREATE TABLE IF NOT EXISTS model_bundle_installations (
    installation_id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    bundle_sha256 TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error_json TEXT
);

CREATE TABLE IF NOT EXISTS model_bundle_verifications (
    verification_id TEXT PRIMARY KEY,
    bundle_sha256 TEXT NOT NULL,
    status TEXT NOT NULL,
    report_json TEXT NOT NULL,
    verified_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_bundle_smoke_tests (
    smoke_test_id TEXT PRIMARY KEY,
    bundle_sha256 TEXT NOT NULL,
    model_instance_id TEXT,
    status TEXT NOT NULL,
    report_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_bundle_license_acceptances (
    acceptance_id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    license_digest TEXT NOT NULL,
    actor_kind TEXT NOT NULL,
    accepted_at TEXT NOT NULL,
    UNIQUE (bundle_id, bundle_version, license_digest)
);

CREATE TABLE IF NOT EXISTS model_instances (
    model_instance_id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    plugin_version TEXT NOT NULL,
    plugin_package_sha256 TEXT NOT NULL,
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    bundle_sha256 TEXT NOT NULL,
    status TEXT NOT NULL,
    descriptor_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_instance_health (
    health_id TEXT PRIMARY KEY,
    model_instance_id TEXT NOT NULL,
    status TEXT NOT NULL,
    detail TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    FOREIGN KEY (model_instance_id) REFERENCES model_instances(model_instance_id)
);

CREATE TABLE IF NOT EXISTS model_bundle_references (
    reference_id TEXT PRIMARY KEY,
    bundle_sha256 TEXT NOT NULL,
    reference_kind TEXT NOT NULL,
    reference_location TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_bundle_events (
    event_id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL,
    bundle_version TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    detail_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_bundle_events_identity
    ON model_bundle_events(bundle_id, bundle_version, created_at);
CREATE INDEX IF NOT EXISTS idx_model_bundle_references_digest
    ON model_bundle_references(bundle_sha256);
CREATE INDEX IF NOT EXISTS idx_model_instances_bundle
    ON model_instances(bundle_id, bundle_version);
