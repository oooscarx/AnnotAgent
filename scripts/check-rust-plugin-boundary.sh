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
