#!/bin/bash
# Renders the Homebrew cask (app) and formula (CLI) for a release and commits them to the shared tap,
# nickv2002/homebrew-tap. Reads the SHA-256 of each asset in dist/.
#
# Usage: scripts/update_tap.sh <version> [--push]
#
# The tap checkout is $TAP_DIR (default: ../homebrew-tap next to this repo); it is cloned there if missing.
# Without --push the commit stays local, so the result can be checked with `brew style` first.
set -euo pipefail

version="${1:?usage: update_tap.sh <version> [--push]}"
push=0
[[ "${2:-}" == "--push" ]] && push=1

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
tap_dir="${TAP_DIR:-$repo_root/../homebrew-tap}"
tap_slug="nickv2002/homebrew-tap"
dist="${DIST_DIR:-dist}"

sha() {
  [[ -f "$1" ]] || { echo "error: release asset missing: $1" >&2; exit 1; }
  shasum -a 256 "$1" | cut -d' ' -f1
}
app_sha=$(sha "$dist/ImageCompress-${version}.zip")
mac_sha=$(sha "$dist/imgcompress-${version}-macos-universal.tar.gz")
x86_sha=$(sha "$dist/imgcompress-${version}-linux-x86_64.tar.gz")
arm_sha=$(sha "$dist/imgcompress-${version}-linux-aarch64.tar.gz")

if [[ ! -d "$tap_dir/.git" ]]; then
  echo "==> cloning ${tap_slug} into ${tap_dir}"
  gh repo clone "$tap_slug" "$tap_dir"
fi
if [[ -n "$(git -C "$tap_dir" status --porcelain)" ]]; then
  echo "error: ${tap_dir} has uncommitted changes" >&2
  exit 1
fi
# A brand-new tap has no main on GitHub yet; there is nothing to pull then.
if git -C "$tap_dir" ls-remote --exit-code origin main >/dev/null 2>&1; then
  git -C "$tap_dir" pull --ff-only --quiet
fi

render() {
  sed -e "s/@VERSION@/${version}/g" -e "s/@SHA256_APP@/${app_sha}/g" -e "s/@SHA256_MACOS@/${mac_sha}/g" \
      -e "s/@SHA256_LINUX_X86_64@/${x86_sha}/g" -e "s/@SHA256_LINUX_AARCH64@/${arm_sha}/g" "$1" > "$2"
  if grep -q '@[A-Z0-9_]*@' "$2"; then echo "error: unfilled placeholder in $2" >&2; exit 1; fi
}
mkdir -p "$tap_dir/Casks" "$tap_dir/Formula"
render packaging/homebrew/imagecompress.rb "$tap_dir/Casks/imagecompress.rb"
render packaging/homebrew/imgcompress.rb "$tap_dir/Formula/imgcompress.rb"

if command -v brew >/dev/null 2>&1; then
  brew style "$tap_dir/Casks/imagecompress.rb" "$tap_dir/Formula/imgcompress.rb"
fi

git -C "$tap_dir" add Casks/imagecompress.rb Formula/imgcompress.rb
if git -C "$tap_dir" diff --cached --quiet; then
  echo "==> tap already at ${version}"
else
  git -C "$tap_dir" commit --quiet -m "imagecompress, imgcompress ${version}"
  echo "==> tap commit: $(git -C "$tap_dir" log --oneline -1)"
fi
if [[ $push -eq 1 ]]; then
  git -C "$tap_dir" push --quiet origin HEAD:main
  echo "==> pushed ${tap_slug}"
fi
