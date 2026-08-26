#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo run -p annotagent -- demo robocup
