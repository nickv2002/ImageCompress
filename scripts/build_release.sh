#!/bin/bash
# Builds the notices bundle, the universal CLI + gifsicle helper, then archives
# and exports a Developer-ID-signed Release build of the app.
# Produces build/export/ImageCompress.app.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

archive_path="build/ImageCompress.xcarchive"
export_path="build/export"

echo "==> license notices (build/licenses)"
scripts/collect_notices.sh

# Universal imgcompress (+ gifsicle) -> build/cli/imgcompress, build/helpers/gifsicle.
# The Xcode project embeds and signs them (macos/project.yml script phases).
if [[ -x scripts/build_cli_mac.sh ]]; then
  echo "==> building universal CLI + gifsicle"
  scripts/build_cli_mac.sh
else
  echo "==> scripts/build_cli_mac.sh not found; relying on the Xcode CLI build phase"
fi

echo "==> xcodegen generate"
(cd macos && xcodegen generate)

echo "==> archiving (Release)"
rm -rf "$archive_path"
xcodebuild -project macos/ImageCompress.xcodeproj -scheme ImageCompress \
  -configuration Release clean archive -archivePath "$archive_path"

echo "==> exporting signed .app"
rm -rf "$export_path"
xcodebuild -exportArchive -archivePath "$archive_path" \
  -exportPath "$export_path" -exportOptionsPlist ExportOptions.plist

echo "==> exported: $export_path/ImageCompress.app"
