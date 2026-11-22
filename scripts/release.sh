#!/bin/bash
# Cuts a release: license gate, version bump (macos/project.yml + Cargo.toml),
# builds + signs + notarizes the Mac app and CLI, builds the Linux tarballs and
# the .deb/.rpm packages and the GPL Corresponding Source tarball, generates a
# changelog, tags, pushes, publishes a GitHub Release, updates the Homebrew tap,
# uploads the Linux packages to Cloudsmith (if configured), and installs the
# build into /Applications.
# The only command the owner needs to run.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

repo_slug="nickv2002/ImageCompress"

branch=$(git rev-parse --abbrev-ref HEAD)
if [[ "$branch" != "main" ]]; then
  echo "error: must be on main (currently on $branch)" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean" >&2
  exit 1
fi

for tool in cargo cargo-deny cargo-about xcodegen sign_update gh op docker; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: required tool not found: $tool" >&2; exit 1; }
done

version=$(scripts/next_version.sh)
cargo_version=$(scripts/next_version.sh --cargo "$version")
tag="v${version}"
prev_tag=$(git describe --tags --abbrev=0 2>/dev/null || true)

echo "==> releasing ${tag} (cargo version ${cargo_version}; previous tag: ${prev_tag:-none})"

# License gate (GPL compliance): fail before any long build.
echo "==> license check (cargo deny)"
cargo deny check licenses bans sources

# Drafted now, while the owner is still at the terminal, so any manual prompt
# happens before the long build. Stored apart from the changelog body, which
# changelog.sh rewrites. Fails open: no file means no summary on the release.
mkdir -p build dist
release_summary_file="build/release-summary.md"
rm -f "$release_summary_file"
scripts/release_summary.sh "$version" "$prev_tag" "$release_summary_file"

echo "==> updating versions (macos/project.yml, Cargo.toml)"
sed -i '' -E "s/MARKETING_VERSION: \"[^\"]*\"/MARKETING_VERSION: \"${version}\"/" macos/project.yml
sed -i '' -E "s/CURRENT_PROJECT_VERSION: \"[^\"]*\"/CURRENT_PROJECT_VERSION: \"${version}\"/" macos/project.yml
# Only the [workspace.package] version line.
python3 -I - "$cargo_version" <<'PYEOF'
import re, sys
v = sys.argv[1]
text = open("Cargo.toml", encoding="utf-8").read()
new, n = re.subn(r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")[^"]*(")', r"\g<1>%s\g<2>" % v, text, count=1, flags=re.S)
if n != 1:
    sys.exit("error: could not find [workspace.package] version in Cargo.toml")
open("Cargo.toml", "w", encoding="utf-8").write(new)
PYEOF
# Refresh the workspace members' own entries in Cargo.lock (no network, no dep changes).
cargo update --workspace --offline

scripts/build_release.sh
scripts/notarize.sh "$version"

echo "==> building Linux tarballs (docker)"
scripts/build_linux.sh "$version"
echo "==> building Linux .deb/.rpm packages (docker)"
scripts/build_linux_packages.sh "$version"

scripts/changelog.sh "$version" "$prev_tag"

echo "==> signing update for Sparkle"
zip_name="ImageCompress-${version}.zip"
zip_path="dist/${zip_name}"
sig_output=$(sign_update "$zip_path")
signature=$(printf '%s' "$sig_output" | grep -oE 'sparkle:edSignature="[^"]*"' \
  | sed 's/sparkle:edSignature="//;s/"//')

if [[ -z "$signature" ]]; then
  echo "error: could not extract edSignature from sign_update output" >&2
  echo "sign_update output: $sig_output" >&2
  exit 1
fi

file_size=$(/usr/bin/stat -f%z "$zip_path")
download_url="https://github.com/${repo_slug}/releases/download/v${version}/${zip_name}"

scripts/update_appcast.sh "$version" "$download_url" "$signature" "$file_size" "$release_summary_file"

echo "==> committing version bump + changelog + appcast"
git add macos/project.yml macos/ImageCompress.xcodeproj Cargo.toml Cargo.lock CHANGELOG.md docs/appcast.xml
git commit -m "Release ${version}"

echo "==> tagging ${tag}"
git tag "$tag"

# Built from the release commit (git archive) so the tarball matches the tag.
echo "==> building Corresponding Source tarball"
scripts/make_source_tarball.sh --verify "$version"

scripts/verification_notes.sh "$version"

echo "==> pushing"
git push
git push --tags

echo "==> creating GitHub release"
release_body_file="build/release-body.md"
if [[ -s "$release_summary_file" ]]; then
  { cat "$release_summary_file"; echo; cat build/release-notes.md; } > "$release_body_file"
else
  cp build/release-notes.md "$release_body_file"
fi
# A copy without the version in its name, so the website can link to
# releases/latest/download/ImageCompress.zip. Sparkle and Homebrew use the versioned one.
cp "$zip_path" dist/ImageCompress.zip
assets=("$zip_path" dist/ImageCompress.zip "dist/imgcompress-${version}-macos-universal.tar.gz" "dist/imgcompress-${version}-sources.tar.gz")
shopt -s nullglob
assets+=(dist/imgcompress-"${version}"-linux-*.tar.gz dist/imgcompress_"${version}"_*.deb dist/imgcompress-"${version}"-*.rpm dist/gifsicle-*-src.tar.gz)
shopt -u nullglob
for f in "${assets[@]}"; do
  [[ -f "$f" ]] || { echo "error: release asset missing: $f" >&2; exit 1; }
done
gh release create "$tag" "${assets[@]}" \
  --repo "$repo_slug" \
  --title "${version}" --notes-file "$release_body_file"

echo "==> updating the Homebrew tap"
scripts/update_tap.sh "$version" --push

echo "==> publishing Linux packages"
scripts/publish_linux_packages.sh "$version"

scripts/install_local.sh

echo "==> done: ${tag}"
