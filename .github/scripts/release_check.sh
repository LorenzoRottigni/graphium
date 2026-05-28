#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

if ! command -v jq >/dev/null; then
  echo "jq is required"
  exit 1
fi

tag="${1:-}"
version_from_tag=""
if [[ -n "$tag" ]]; then
  version_from_tag="${tag#v}"
  if [[ "$version_from_tag" == "$tag" ]]; then
    echo "tag must be like vX.Y.Z (got: $tag)"
    exit 2
  fi
fi

core_ver="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="graphium") | .version')"
macro_ver="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="graphium-macro") | .version')"
ui_ver="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="graphium-ui") | .version')"

echo "tag=${version_from_tag:-<none>} core=$core_ver macro=$macro_ver ui=$ui_ver"

if [[ "$core_ver" != "$macro_ver" || "$core_ver" != "$ui_ver" ]]; then
  echo "All crate versions must match."
  exit 1
fi

if [[ -n "$version_from_tag" && "$core_ver" != "$version_from_tag" ]]; then
  echo "Tag version must match all crate versions."
  exit 1
fi
