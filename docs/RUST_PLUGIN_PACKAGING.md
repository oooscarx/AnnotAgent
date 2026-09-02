# `.annotplugin` packaging

AnnotAgent packages use a deterministic ZIP profile with sorted entries, a fixed timestamp,
normalized permissions and a stable SHA-256 digest. `annotagent plugin pack` creates
`checksums.json`; `verify` requires an exact file-list match and validates every digest before an
executable path is considered.

Verification rejects absolute paths, parent traversal, mixed path separators, duplicate entries,
links, excessive expansion, missing manifests and empty or unsupported target executables. Install
extracts only already-verified bytes into a random sibling staging directory, then atomically
renames it to `<data>/plugins/<id>/<version>` without overwriting an existing version.

Publisher signatures are optional in Alpha. Packages truthfully report `unsigned` or
`present_unverified`; the latter is not treated as publisher authentication.
