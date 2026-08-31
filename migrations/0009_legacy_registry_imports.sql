CREATE TABLE IF NOT EXISTS legacy_registry_imports (
    fingerprint TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    model_profile_id TEXT NOT NULL,
    report_json TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_legacy_registry_imports_provider
ON legacy_registry_imports(provider_id, imported_at DESC);
