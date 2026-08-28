#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
venv_dir="$repo_dir/workspace/.annotagent/sam2-venv"
model_dir="$repo_dir/workspace/.annotagent/models"
checkpoint="$model_dir/sam2.1_hiera_tiny.pt"

command -v uv >/dev/null 2>&1 || {
  echo "uv is required: https://docs.astral.sh/uv/"
  exit 1
}

mkdir -p "$model_dir"
if [[ ! -x "$venv_dir/bin/python" ]]; then
  uv venv "$venv_dir" --python 3.12
fi
SAM2_BUILD_CUDA=0 uv pip install --python "$venv_dir/bin/python" \
  'git+https://github.com/facebookresearch/sam2.git@2b90b9f5ceec907a1c18123530e92e794ad901a4' pillow

if [[ ! -s "$checkpoint" ]]; then
  curl -fL --retry 3 \
    -o "$checkpoint" \
    https://dl.fbaipublicfiles.com/segment_anything_2/092824/sam2.1_hiera_tiny.pt
fi

echo "SAM2 environment ready: $venv_dir"
echo "SAM2 checkpoint ready: $checkpoint"
