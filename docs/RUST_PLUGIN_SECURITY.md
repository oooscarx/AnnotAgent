# Rust plugin process security

Alpha runs model code outside Core in a separate native process. The Host clears inherited
environment variables, sets a private working directory, sends only isolated state/cache/temporary
paths and a read-only-model-root contract, and does not send provider credentials, database handles
or Project paths. Images and Artifacts cross the authenticated loopback protocol as bounded data.

Each start generates a new in-memory session token and nonce. The token is written once through the
child standard input, is required on every endpoint, is redacted from bounded logs and is never
persisted. The plugin chooses an ephemeral loopback port; the Host verifies identity, protocol,
nonce and address before making requests.

Panics become structured inference errors. Process exit becomes `Crashed`; cancellation, timeout,
graceful shutdown and forced termination are bounded. A failed plugin cannot terminate Core.

This is process isolation, not a universal strong sandbox. Target-specific namespaces, resource
control groups and syscall filters are future optional features and are never implied when absent.
