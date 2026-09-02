#!/usr/bin/env bash
set -euo pipefail

roots=()
for candidate in crates/annotagent-plugin-api crates/annotagent-plugin-sdk \
  crates/annotagent-plugin-host crates/annotagent-plugin-registry \
  crates/annotagent-model-runtime-common crates/annotagent-model-runtime-onnx plugins; do
  if [[ -e "$candidate" ]]; then
    roots+=("$candidate")
  fi
done

active_release_roots=(apps crates plugins examples scripts web/e2e)

tracked_script_files="$(
  git ls-files "${active_release_roots[@]}" \
    | rg -i '(^|/)([^/]+\.py|requirements[^/]*\.txt|pyproject\.toml|uv\.lock)$' \
    || true
)"
if [[ -n "$tracked_script_files" ]]; then
  printf '%s\n' "$tracked_script_files"
  echo 'Official install, Run, test, and release paths must not contain scripting-runtime files.' >&2
  exit 1
fi

if rg -n 'Command::new\("(python|python3|pip|pip3|uv|conda)"\)' apps crates plugins; then
  echo 'Official Rust processes may not launch scripting runtimes or package managers.' >&2
  exit 1
fi

if rg -n -i '(^|[[:space:]/])(python3?|pip3?|uv|conda|venv)([[:space:]/]|$)' scripts \
  --glob '!check-rust-plugin-boundary.sh'; then
  echo 'Official release scripts may not start or install a scripting runtime.' >&2
  exit 1
fi

if [[ ${#roots[@]} -eq 0 ]]; then
  exit 0
fi

if rg -n -i '(^|[^a-z])(python3?|pip|uv|conda|venv|fastapi|pydantic)([^a-z]|$)|requirements\.txt' "${roots[@]}"; then
  echo 'Active Rust plugin paths must not depend on the legacy scripting runtime.' >&2
  exit 1
fi

if rg -n 'Command::new\("(python|python3|pip|uv|conda)"\)' "${roots[@]}"; then
  echo 'Rust plugins may not launch legacy package or worker processes.' >&2
  exit 1
fi
