#!/bin/bash
# Notarizes and staples the exported .app, then produces the macOS release
# artifacts in dist/:
#   ImageCompress-<version>.zip                       the stapled app
#   imgcompress-<version>-macos-universal.tar.gz      the notarized CLI + gifsicle
#                                                     + licenses, for scripts/brew
#
# The App Store Connect API key is pulled live from 1Password on every run
# (op CLI — prompts for unlock/approval each time). Nothing is ever stored
# in the macOS keychain; the key only ever touches disk as a 0600 file in a
# 0700 temp dir that's deleted on exit (success or failure).
#
# Usage: scripts/notarize.sh <version>
set -euo pipefail

version="${1:?usage: notarize.sh <version>}"
# 1Password location of the App Store Connect API key. Set these in
# scripts/release.local.env (gitignored, sourced below) or the environment:
#   OP_ACCOUNT    1Password account, e.g. example.1password.com
#   OP_VAULT      vault holding the key item
#   OP_ITEM_NAME  item name; the item holds the AuthKey_<id>.p8 file and an
#                 "issuer id" field
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck disable=SC1091
[ -f "$repo_root/scripts/release.local.env" ] && . "$repo_root/scripts/release.local.env"
op_account="${OP_ACCOUNT:?set OP_ACCOUNT (see scripts/notarize.sh header)}"
op_vault="${OP_VAULT:?set OP_VAULT}"
op_item_name="${OP_ITEM_NAME:?set OP_ITEM_NAME}"
op_item="op://${op_vault}/${op_item_name}"

cd "$repo_root"

app_path="build/export/ImageCompress.app"
submit_zip="build/notarize-submission.zip"
dist_dir="dist"
dist_zip="$dist_dir/ImageCompress-${version}.zip"
cli_stage="build/cli-tarball"
cli_name="imgcompress-${version}-macos-universal"

if [[ ! -d "$app_path" ]]; then
  echo "error: $app_path not found — run scripts/build_release.sh first" >&2
  exit 1
fi

# Every nested executable must already be signed with a valid, strict
# signature (hardened runtime + timestamp) BEFORE the app is submitted,
# otherwise notarytool rejects the whole submission minutes later.
echo "==> verifying code signatures of nested code"
nested=(
  "$app_path/Contents/MacOS/imgcompress"
  "$app_path/Contents/Helpers/gifsicle"
)
while IFS= read -r -d '' appex; do nested+=("$appex"); done \
  < <(find "$app_path/Contents/PlugIns" -maxdepth 1 -name '*.appex' -print0 2>/dev/null)
while IFS= read -r -d '' fw; do nested+=("$fw"); done \
  < <(find "$app_path/Contents/Frameworks" -maxdepth 1 \( -name '*.framework' -o -name '*.dylib' \) -print0 2>/dev/null)
for item in "${nested[@]}"; do
  if [[ ! -e "$item" ]]; then
    echo "error: expected bundle component missing: $item" >&2
    exit 1
  fi
  codesign --verify --strict --verbose=2 "$item"
  # Captured first: with pipefail, `grep -q` exiting early would SIGPIPE codesign and fail the check.
  sig_info=$(codesign -dv --verbose=4 "$item" 2>&1)
  if ! grep -q 'flags=.*runtime' <<<"$sig_info"; then
    echo "error: hardened runtime flag missing on $item" >&2
    exit 1
  fi
done
codesign --verify --deep --strict --verbose=2 "$app_path"

if [[ -n "${ASC_KEY_PATH:-}" && -n "${ASC_KEY_ID:-}" && -n "${ASC_ISSUER_ID:-}" ]]; then
  # A local copy (e.g. .context/secrets, gitignored, set in release.local.env) lets a release run unattended.
  echo "==> using the App Store Connect API key at $ASC_KEY_PATH"
  key_path="$ASC_KEY_PATH"; key_id="$ASC_KEY_ID"; issuer_id="$ASC_ISSUER_ID"
else
  echo "==> fetching App Store Connect API key from 1Password (${op_item})"
  # op_retry beeps and asks again if an approval prompt times out.
  . "$repo_root/scripts/op_retry.sh"
  key_filename=$(op_retry item get "$op_item_name" --vault "$op_vault" --account "$op_account" --format json \
    | /usr/bin/python3 -c 'import json, sys; print(json.load(sys.stdin)["files"][0]["name"])')
  key_id=$(echo "$key_filename" | sed -E 's/^AuthKey_([A-Za-z0-9]+)\.p8$/\1/')
  issuer_id=$(op_retry read "${op_item}/issuer id" --account "$op_account")

  secret_dir=$(mktemp -d)
  trap 'rm -rf "$secret_dir"' EXIT
  chmod 700 "$secret_dir"
  key_path="${secret_dir}/${key_filename}"
  op_retry read "${op_item}/${key_filename}" --account "$op_account" --out-file "$key_path" >/dev/null
  chmod 600 "$key_path"
fi

echo "==> zipping for submission"
rm -f "$submit_zip"
ditto -c -k --keepParent "$app_path" "$submit_zip"

echo "==> submitting to notarytool (key id: $key_id)"
xcrun notarytool submit "$submit_zip" \
  --key "$key_path" --key-id "$key_id" --issuer "$issuer_id" --wait

echo "==> stapling ticket"
xcrun stapler staple "$app_path"

echo "==> verifying"
xcrun stapler validate "$app_path"
spctl --assess --type execute --verbose "$app_path"

echo "==> zipping stapled app for release"
mkdir -p "$dist_dir"
rm -f "$dist_zip"
# --norsrc/--noextattr/--noqtn/--noacl: keep AppleDouble "._*" entries out of the zip.
# Plain unzip / Archive Utility extract them as real files inside the bundle, which
# breaks the code-signature seal ("app is damaged"); only ditto hid them.
ditto -c -k --keepParent --norsrc --noextattr --noqtn --noacl "$app_path" "$dist_zip"
if grep -q '/\._' <<<"$(unzip -l "$dist_zip")"; then
  echo "error: $dist_zip contains AppleDouble (._*) entries" >&2
  exit 1
fi

# The standalone CLI tarball reuses the exact signed binaries that were inside
# the notarized app submission (same cdhashes), so Gatekeeper's online check
# passes. A bare Mach-O cannot carry a stapled ticket.
echo "==> packaging CLI tarball"
rm -rf "$cli_stage"
mkdir -p "$cli_stage/$cli_name"
ditto --norsrc --noextattr --noqtn --noacl "$app_path/Contents/MacOS/imgcompress" "$cli_stage/$cli_name/imgcompress"
ditto --norsrc --noextattr --noqtn --noacl "$app_path/Contents/Helpers/gifsicle" "$cli_stage/$cli_name/gifsicle"
licenses_src="$app_path/Contents/Resources/Licenses"
if [[ -d "$licenses_src" ]]; then
  ditto --norsrc --noextattr --noqtn --noacl "$licenses_src" "$cli_stage/$cli_name/licenses"
elif [[ -d build/licenses ]]; then
  ditto --norsrc --noextattr --noqtn --noacl build/licenses "$cli_stage/$cli_name/licenses"
else
  echo "error: no license notices found (run scripts/collect_notices.sh)" >&2
  exit 1
fi
cli_tar="$dist_dir/${cli_name}.tar.gz"
rm -f "$cli_tar"
COPYFILE_DISABLE=1 tar --no-xattrs -C "$cli_stage" -czf "$cli_tar" "$cli_name"
if grep -q '/\._' <<<"$(tar -tzf "$cli_tar")"; then
  echo "error: $cli_tar contains AppleDouble (._*) entries" >&2
  exit 1
fi

echo "==> notarized artifact: $dist_zip"
echo "==> notarized CLI: $cli_tar"
