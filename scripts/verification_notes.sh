#!/bin/bash
# Appends a Verification section (SHA256 of every release asset + code
# signature/notarization info) and a Source Code section (GPLv3 §6 pointer) to
# build/release-notes.md, so every GitHub release documents how to confirm a
# downloaded build and where its Corresponding Source lives.
# Usage: scripts/verification_notes.sh <version>
set -euo pipefail

version="${1:?usage: verification_notes.sh <version>}"

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

repo_slug="nickv2002/ImageCompress"
app_path="build/export/ImageCompress.app"
zip_name="ImageCompress-${version}.zip"
dist_zip="dist/${zip_name}"
notes_file="build/release-notes.md"

codesign_info=$(codesign -dv --verbose=4 "$app_path" 2>&1)
cdhash=$(echo "$codesign_info" | grep '^CDHash=' | cut -d= -f2)
team_id=$(echo "$codesign_info" | grep '^TeamIdentifier=' | cut -d= -f2)
notarization=$(echo "$codesign_info" | grep '^Notarization Ticket=' | cut -d= -f2)

# Every asset that release.sh uploads: the app zip, CLI tarballs, sources.
shopt -s nullglob
assets=("$dist_zip" dist/imgcompress-"${version}"-*.tar.gz dist/gifsicle-*-src.tar.gz)
shopt -u nullglob

{
  echo
  echo "## Verification"
  echo
  echo "SHA256 of every asset:"
  echo
  for f in "${assets[@]}"; do
    [[ -f "$f" ]] || continue
    echo "- \`$(basename "$f")\`: \`$(shasum -a 256 "$f" | awk '{print $1}')\`"
  done
  echo
  echo "- Code signature (macOS app and CLI): Developer ID Application, Team ID \`${team_id}\`"
  echo "- Notarization: ${notarization}"
  echo "- App CDHash: \`${cdhash}\`"
  echo
  echo "Verify after unzipping with:"
  echo '```sh'
  echo "shasum -a 256 ${zip_name}"
  echo 'codesign -dv --verbose=4 "ImageCompress.app"'
  echo '```'
  echo
  echo "## Source code"
  echo
  echo "ImageCompress binaries are licensed GPL-3.0-or-later (see \`COPYING\` and the \`Licenses/\` folder shipped in every download). The complete Corresponding Source for this release, including vendored Rust crates and build scripts, is attached as \`imgcompress-${version}-sources.tar.gz\`; the bundled gifsicle (GPL-2.0-only) source is attached as \`gifsicle-*-src.tar.gz\`. The git tag is [v${version}](https://github.com/${repo_slug}/tree/v${version})."
} >> "$notes_file"

echo "==> verification info appended to $notes_file"
