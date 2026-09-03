#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
release_root="$repo_root/dist/releases/models-v1"
bundle_source="$repo_root/dist/model-catalog/bundles/efficientsam-ti-onnx-1.0.0.annotmodel"
catalog_source="$repo_root/dist/model-catalog/catalog.json"
verification_source="$repo_root/dist/model-catalog/verification/efficientsam-ti-onnx-1.0.0.json"
plugin_source="$repo_root/dist/plugins/efficientsam-onnx-1.0.0-macos-aarch64-final.annotplugin"
plugin_name="efficientsam-onnx-1.0.0-macos-aarch64.annotplugin"
bundle_name="efficientsam-ti-onnx-1.0.0.annotmodel"
verification_name="efficientsam-ti-onnx-1.0.0-verification.json"
expected_plugin_sha256="283a9486edaa7b25ae3cf111cd859ca90fa38de488cd3a8c9196d297d10099cd"
expected_bundle_sha256="3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "This release recipe produces only the evidenced macos-aarch64 asset." >&2
  exit 1
fi

for required in "$plugin_source" "$bundle_source" "$catalog_source" "$verification_source"; do
  if [[ ! -f "$required" ]]; then
    echo "Missing generated release input: $required" >&2
    echo "Build the audited Model Recipe first; no model bytes are stored in Git." >&2
    exit 1
  fi
done

cd "$repo_root"
mkdir -p "$release_root"
cargo build --locked -p annotagent

actual_plugin_sha256=$(shasum -a 256 "$plugin_source" | awk '{print $1}')
if [[ "$actual_plugin_sha256" != "$expected_plugin_sha256" ]]; then
  echo "Plugin release candidate digest changed: $actual_plugin_sha256" >&2
  echo "Do not overwrite version 1.0.0; rebuild under a new Plugin version." >&2
  exit 1
fi

"$repo_root/target/debug/annotagent" plugin verify "$plugin_source" >/dev/null
"$repo_root/target/debug/annotagent" models bundle verify "$bundle_source" >/dev/null

install -m 0644 "$plugin_source" "$release_root/$plugin_name"
install -m 0644 "$bundle_source" "$release_root/$bundle_name"
install -m 0644 "$catalog_source" "$release_root/catalog.json"
install -m 0644 "$verification_source" "$release_root/$verification_name"

actual_plugin_sha256=$(shasum -a 256 "$release_root/$plugin_name" | awk '{print $1}')
actual_bundle_sha256=$(shasum -a 256 "$release_root/$bundle_name" | awk '{print $1}')
if [[ "$actual_plugin_sha256" != "$expected_plugin_sha256" ]]; then
  echo "Plugin package digest changed: $actual_plugin_sha256" >&2
  echo "Audit the release build and intentionally update the pinned release digest." >&2
  exit 1
fi
if [[ "$actual_bundle_sha256" != "$expected_bundle_sha256" ]]; then
  echo "Model Bundle digest changed: $actual_bundle_sha256" >&2
  echo "Do not publish until the audited model provenance is updated." >&2
  exit 1
fi

(
  cd "$release_root"
  shasum -a 256 \
    "$plugin_name" \
    "$bundle_name" \
    catalog.json \
    "$verification_name" > SHA256SUMS
  shasum -a 256 -c SHA256SUMS
)

printf 'Prepared macOS ARM64 release assets in %s\n' "$release_root"
find "$release_root" -maxdepth 1 -type f -print | sort
