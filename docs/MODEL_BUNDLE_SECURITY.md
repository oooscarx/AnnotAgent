# Model Bundle Security

The Bundle boundary treats Catalog and archive data as hostile until verified.

- Downloads use public credential-free HTTPS, DNS destination checks, no redirects, bounded
  streaming, cancellation, exact length, and SHA-256.
- ZIP verification rejects traversal, absolute/mixed paths, links, duplicates/case collisions,
  undeclared files, excessive expansion, and checksum changes.
- Installation extracts to a unique staging directory, re-verifies bytes, then atomically renames
  to `models/sha256/<prefix>/<digest>`. Registry state is persisted after activation.
- A Bundle cannot run code, select local paths, choose Run/Image identities, supply credentials, or
  bypass the Plugin Host.
- License acceptance is tied to Bundle ID/version and the exact license digest.
- Plugin/Bundle/Contract/platform compatibility and smoke evidence fail closed.
- Published references protect content from removal; GC removes only disabled, unreferenced content
  and bounded staging entries.

Optional Ed25519 Bundle and Catalog signature metadata strengthens publisher authentication, but a
valid signature does not replace license, Contract, compatibility, or smoke validation. Native
Plugin process isolation has separate limits described in [Plugin Security](RUST_PLUGIN_SECURITY.md).
