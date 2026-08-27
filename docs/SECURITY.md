# Security

## Secrets

- Do not commit `.env` or local provider configuration containing a key.
- CLI providers read the configured environment variable only when making a request.
- The Web API key is write-only and stored per workspace in the operating system keychain (Keychain Services on macOS, Credential Manager on Windows, Secret Service on Linux). It is omitted from responses, the local TOML settings file, SQLite, model trace, and logs.
- Headless CI and browser automation may set `ANNOTAGENT_DISABLE_KEYCHAIN=1`; in that mode the server never reads or writes the system keychain and live providers must use their configured API-key environment variable. This opt-out is never enabled by default.
- Non-secret Web settings are atomically written to `<workspace>/.annotagent/settings.toml` with owner-only permissions on Unix.
- Provider configuration rejects credential-bearing custom headers such as `Authorization`, API-key/access-token/secret/password fields, including nested extra request metadata. Endpoint URLs cannot contain embedded user info. Authorization and image base64 are redacted from Provider errors, model messages, history, and logs.
- A key pasted into a chat or terminal transcript should be rotated after testing.

## Filesystem

The server canonicalizes workspace/project paths, rejects project IDs containing separators or traversal components, checks containment after canonicalization, and does not follow symlinks during dataset enumeration. HTTP import can only copy from a source already under the workspace. The CLI is the explicit trusted path for copying external folders.

Images are not database blobs. Header dimensions and maximum pixels are checked before the full decode, thumbnails constrain model input, and crop rectangles use checked normalized geometry. ZIP archives are rejected before extraction; Workflow Alpha deliberately has no archive unpacker, so ZIP traversal has no extraction surface. Arbitrary URL fetch, Shell tools, and arbitrary model-controlled file reads are absent.

HTTP Backend endpoints must use `http` or `https`, include a host, and contain no embedded credentials. Local model/resource paths are resolved by trusted registry configuration rather than model output. Image and annotation import paths are canonicalized and checked against the workspace, including symlink targets.

## Model/tool boundary

Model output is untrusted. The Tool Registry rejects unregistered tools, non-applicable task tools, unknown object fields, missing fields, invalid types, enum violations, numeric ranges, and array bounds. Published DAG backends additionally validate response protocol/model identity and Artifact image/task/type/label scope. Tools receive a checked project root, current image object, task ID, and cancellation token—not a free path. Runtime validates again before Commit.

System and backend prompts state that text visible inside an image is untrusted visual data, never an instruction. No OCR/image text is interpolated into the system message. The trace contains visible model content and structured tool/validation decisions, not hidden chain-of-thought.

## Network and UI

The server binds to loopback by default. CORS is permissive for local development; do not expose it to an untrusted network without adding origin restrictions and authentication. There is no login or multi-user isolation in this release.

## Reporting checklist

Before sharing a history export, verify that it has schema version 1, no secret fields, no Authorization text, and no `data:` image payload. Storage/provider tests exercise redaction, source scans, safe settings rejection-before-write, and history ID remapping. Real credentials are never needed by the offline acceptance suite.
