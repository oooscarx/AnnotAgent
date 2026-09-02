CREATE TABLE IF NOT EXISTS plugins (
    plugin_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    publisher TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS plugin_versions (
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    plugin_api_version TEXT NOT NULL,
    protocol_version TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (plugin_id, version),
    FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id)
);

CREATE TABLE IF NOT EXISTS plugin_installations (
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    installation_root TEXT NOT NULL,
    installed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (plugin_id, version),
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_permissions (
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    permissions_json TEXT NOT NULL,
    reviewed_at TEXT NOT NULL,
    PRIMARY KEY (plugin_id, version),
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_models (
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    model_id TEXT NOT NULL,
    model_revision INTEGER NOT NULL,
    capability_contract_sha256 TEXT NOT NULL,
    descriptor_json TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    PRIMARY KEY (plugin_id, version, model_id, model_revision),
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_weight_sets (
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    model_id TEXT NOT NULL,
    checkpoint_sha256 TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    provisioned_at TEXT NOT NULL,
    PRIMARY KEY (plugin_id, version, model_id, checkpoint_sha256),
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_health_checks (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    detail TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_test_runs (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    passed INTEGER NOT NULL,
    report_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_references (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    reference_kind TEXT NOT NULL,
    reference_location TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_license_acceptances (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    code_license TEXT NOT NULL,
    weight_license TEXT NOT NULL,
    accepted_at TEXT NOT NULL,
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE TABLE IF NOT EXISTS plugin_events (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    detail TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (plugin_id, version) REFERENCES plugin_versions(plugin_id, version)
);

CREATE INDEX IF NOT EXISTS idx_plugin_references_identity
    ON plugin_references(plugin_id, version);
CREATE INDEX IF NOT EXISTS idx_plugin_events_identity
    ON plugin_events(plugin_id, version, created_at);
