#!/bin/bash
# Build the Rust CLI and embed it, gifsicle and licence notices into the app bundle.
#  Debug  : host-arch cargo build; gifsicle from build/helpers or Homebrew.
#  Release: universal CLI + gifsicle via scripts/build_*_mac.sh, Developer ID signed.
set -euo pipefail
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
ROOT="$(cd "${SRCROOT}/.." && pwd)"
cd "$ROOT"
DEST="${TARGET_BUILD_DIR}/${EXECUTABLE_FOLDER_PATH}"
HELPERS="${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Helpers"
RES="${TARGET_BUILD_DIR}/${UNLOCALIZED_RESOURCES_FOLDER_PATH}"
mkdir -p "$DEST" "$HELPERS" "$RES"

if [ "${CONFIGURATION}" = "Release" ]; then
  SIGN=(codesign -f -o runtime --timestamp -s "Developer ID Application: Nicholas Vance (3D9SMH4RWM)")
  scripts/build_cli_mac.sh
  CLI="$ROOT/build/cli/imgcompress"
  GIF="$ROOT/build/helpers/gifsicle"
else
  SIGN=(codesign -f -s -)
  cargo build --release -p imgcompress
  CLI="$ROOT/target/release/imgcompress"
  GIF="$ROOT/build/helpers/gifsicle"
  [ -x "$GIF" ] || GIF="$(command -v gifsicle || true)"
fi

cp -f "$CLI" "$DEST/imgcompress"
"${SIGN[@]}" "$DEST/imgcompress"
if [ -n "${GIF:-}" ] && [ -x "$GIF" ]; then
  cp -f "$GIF" "$HELPERS/gifsicle"
  "${SIGN[@]}" "$HELPERS/gifsicle"
elif [ "${CONFIGURATION}" = "Release" ]; then
  echo "error: gifsicle helper missing" >&2; exit 1
else
  echo "warning: no gifsicle found; GIF optimisation unavailable in this build"
fi
if [ -d "$ROOT/build/licenses" ]; then
  rm -rf "$RES/Licenses"; cp -R "$ROOT/build/licenses" "$RES/Licenses"
elif [ "${CONFIGURATION}" = "Release" ]; then
  echo "error: run scripts/collect_notices.sh first" >&2; exit 1
fi
