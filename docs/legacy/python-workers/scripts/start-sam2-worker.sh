#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
python_bin="$repo_dir/workspace/.annotagent/sam2-venv/bin/python"
checkpoint="$repo_dir/workspace/.annotagent/models/sam2.1_hiera_tiny.pt"

if [[ ! -x "$python_bin" || ! -s "$checkpoint" ]]; then
  echo "SAM2 is not installed. Run ./scripts/setup-sam2.sh first."
  exit 1
fi

export ANNOTAGENT_SAM_MODEL_PATH="${ANNOTAGENT_SAM_MODEL_PATH:-$checkpoint}"
export ANNOTAGENT_SAM_HOST="${ANNOTAGENT_SAM_HOST:-127.0.0.1}"
export ANNOTAGENT_SAM_PORT="${ANNOTAGENT_SAM_PORT:-8790}"
exec "$python_bin" "$repo_dir/examples/sam2_vision_worker.py"
