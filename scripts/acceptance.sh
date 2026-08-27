#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

cd "$repo_root"

run "$repo_root/scripts/check-agent-skill-boundaries.sh"

if rg -n -i 'robocup|yolo|field_line|penalty_mark|team_color|\bball\b|\brobot\b' \
  crates/annotagent-core/src; then
  printf '\nCore domain-boundary scan failed.\n' >&2
  exit 1
fi

if git grep -n -E 'sk-ws-|sk-[A-Za-z0-9_-]{20,}' -- . \
  ':!docs/execution/ACCEPTANCE_EVIDENCE.md' ':!scripts/acceptance.sh'; then
  printf '\nRepository secret-prefix scan failed.\n' >&2
  exit 1
fi

run cargo fmt --all -- --check
run cargo clippy --workspace --all-targets --all-features -- -D warnings
run cargo test --workspace --all-features
run cargo build --workspace --all-features
run npm --prefix "$repo_root/web" run typecheck
run npm --prefix "$repo_root/web" run test
run npm --prefix "$repo_root/web" run build
run cargo run -p annotagent -- doctor
run cargo run -p annotagent -- demo generic-classification
run cargo run -p annotagent -- demo generic-detection-crop
run cargo run -p annotagent -- demo robocup-ball

printf '\nAnnotAgent Agent + Skill Alpha acceptance checks completed successfully.\n'
