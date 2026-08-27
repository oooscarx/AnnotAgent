#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pattern='football|white_shoe|white_sock|penalty_mark|field_line|team_color'
paths=(
  crates/annotagent-core/src
  crates/annotagent-runtime/src
  crates/annotagent-server/src
  web/src/components
)

if rg -n -i "$pattern" "${paths[@]}"; then
  printf '\nAgent + Skill domain boundary check failed.\n' >&2
  exit 1
fi

printf 'Agent + Skill domain boundary check passed.\n'
