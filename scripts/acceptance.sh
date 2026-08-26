#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

cd "$repo_root"

run cargo fmt --all -- --check
run cargo clippy --workspace --all-targets --all-features -- -D warnings
run cargo test --workspace --all-features
run cargo build --workspace --all-features
run npm --prefix "$repo_root/web" run typecheck
run npm --prefix "$repo_root/web" run test
run npm --prefix "$repo_root/web" run build
run cargo run -p annotagent -- doctor

printf '\nBaseline acceptance completed successfully.\n'
