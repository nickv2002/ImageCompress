#!/usr/bin/env bash
# Fetch google/jpegli at the pinned commit (with submodules, minus testdata) into crates/jpegli-sys/vendor/jpegli.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/scripts/versions.env"
dest="${1:-$root/crates/jpegli-sys/vendor/jpegli}"
if [[ -f "$dest/CMakeLists.txt" && "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" == "$JPEGLI_COMMIT" ]]; then exit 0; fi
rm -rf "$dest"; mkdir -p "$(dirname "$dest")"
git clone --quiet https://github.com/google/jpegli.git "$dest"
git -C "$dest" checkout --quiet "$JPEGLI_COMMIT"
git -C "$dest" -c submodule.testdata.update=none submodule update --init --recursive --quiet --depth 1 2>/dev/null \
  || git -C "$dest" submodule update --init --recursive --quiet
echo "jpegli @ $JPEGLI_COMMIT ready"
