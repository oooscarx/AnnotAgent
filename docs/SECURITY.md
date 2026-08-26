# Security

## Secrets

- Do not commit `.env` or local provider configuration containing a key.
- CLI providers read the configured environment variable only when making a request.
- The Web API key is write-only and stored per workspace in the operating system keychain (Keychain Services on macOS, Credential Manager on Windows, Secret Service on Linux). It is omitted from responses, the local TOML settings file, SQLite, model trace, and logs.
- Headless CI and browser automation may set `ANNOTAGENT_DISABLE_KEYCHAIN=1`; in that mode the server never reads or writes the system keychain and live providers must use their configured API-key environment variable. This opt-out is never enabled by default.
- Non-secret Web settings are atomically written to `<workspace>/.annotagent/settings.toml` with owner-only permissions on Unix.
- Custom headers and endpoint metadata are represented safely; Authorization and image base64 are not persisted.
- A key pasted into a chat or terminal transcript should be rotated after testing.

## Filesystem

The server canonicalizes workspace/project paths, rejects project IDs containing separators or traversal components, checks containment after canonicalization, and does not follow symlinks during dataset enumeration. HTTP import can only copy from a source already under the workspace. The CLI is the explicit trusted path for copying external folders.

Images are not database blobs. Decode dimensions are checked before decoding, total pixels are bounded, thumbnails constrain model input, and crop rectangles are checked normalized geometry. Arbitrary URL fetch, ZIP extraction, shell tools, and arbitrary file-read tools are absent.

## Model/tool boundary

Model output is untrusted. The Tool Registry rejects unregistered tools, non-applicable task tools, unknown object fields, missing fields, invalid types, enum violations, numeric ranges, and array bounds. Tools receive a checked project root, current image object, task ID, and cancellation token—not a free path. Runtime validates again before commit.

The system message says text inside images is visual data and cannot control the Agent. The trace contains visible model content and structured tool/validation decisions, not hidden chain-of-thought.

## Network and UI

The server binds to loopback by default. CORS is permissive for local development; do not expose it to an untrusted network without adding origin restrictions and authentication. There is no login or multi-user isolation in this release.

## Reporting checklist

Before sharing a history export, verify that it has schema version 1, no secret fields, no Authorization text, and no `data:` image payload. The storage round-trip tests exercise redaction and ID remapping.
