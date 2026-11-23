#!/bin/bash
# Builds the GPLv3 §6 Corresponding Source for a release:
#
#   dist/imgcompress-<version>-sources.tar.gz
#       git archive of the release commit
#       + vendor-crates/   (cargo vendor --locked: every crates.io dependency,
#                           including the C/C++ sources inside the -sys crates)
#       + crates/jpegli-sys/vendor/jpegli  (pinned google/jpegli + submodules;
#                           gitignored in the repo because it is a checkout)
#       + .cargo/config.toml  (source replacement -> vendor-crates)
#       + BUILD_FROM_SOURCE.txt
#   dist/gifsicle-<gifsicle-version>-src.tar.gz
#       the verbatim upstream gifsicle release tarball (sha256-checked)
#
# Usage: scripts/make_source_tarball.sh [--verify] [--worktree] <version>
#   --worktree  use tracked + untracked-not-ignored working-tree files instead
#               of `git archive HEAD` (development dry runs only)
#   --verify    after building, extract the tarball and build it OFFLINE
#               (--network none, cargo build --offline --locked) inside the
#               Alpine toolchain image from docker/linux-build.Dockerfile
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

verify=0
worktree=0
version=""
for arg in "$@"; do
  case "$arg" in
    --verify) verify=1 ;;
    --worktree) worktree=1 ;;
    -*) echo "error: unknown option $arg" >&2; exit 2 ;;
    *) version="$arg" ;;
  esac
done
[[ -n "$version" ]] || { echo "usage: make_source_tarball.sh [--verify] [--worktree] <version>" >&2; exit 2; }

set -a
# shellcheck source=scripts/versions.env
. scripts/versions.env
set +a

name="imgcompress-${version}-sources"
stage_root="build/sources"
stage="${stage_root}/${name}"
dist_dir="dist"
tarball="${dist_dir}/${name}.tar.gz"
cache_dir="build/cache"

rm -rf "$stage"
mkdir -p "$stage" "$dist_dir" "$cache_dir"

echo "==> exporting repository sources"
if [[ $worktree -eq 1 ]]; then
  echo "    (--worktree: including uncommitted files; NOT for releases)"
  git ls-files -z --cached --others --exclude-standard \
    | while IFS= read -r -d '' f; do [[ -e "$f" ]] && printf '%s\0' "$f"; done \
    | tar --null -T - -cf - | tar -xf - -C "$stage"
else
  if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    echo "warning: working tree has uncommitted changes; they are NOT in the archive" >&2
  fi
  git archive --format=tar HEAD | tar -xf - -C "$stage"
fi

echo "==> adding jpegli source (${JPEGLI_COMMIT})"
jpegli_dest="$stage/crates/jpegli-sys/vendor/jpegli"
mkdir -p "$(dirname "$jpegli_dest")"
local_jpegli="crates/jpegli-sys/vendor/jpegli"
if [[ -n "${JPEGLI_SRC:-}" ]]; then
  jpegli_src="$JPEGLI_SRC"
elif [[ -d "$local_jpegli" ]]; then
  jpegli_src="$local_jpegli"
else
  jpegli_src=""
fi
if [[ -n "$jpegli_src" ]]; then
  have=$(git -C "$jpegli_src" rev-parse HEAD 2>/dev/null || echo unknown)
  if [[ "$have" != "$JPEGLI_COMMIT" ]]; then
    echo "error: $jpegli_src is at $have, expected pinned $JPEGLI_COMMIT" >&2
    exit 1
  fi
  rsync -a --exclude '/.git' --exclude '/testdata' "$jpegli_src"/ "$jpegli_dest"/
else
  clone="$cache_dir/jpegli-${JPEGLI_COMMIT}"
  if [[ ! -d "$clone" ]]; then
    git clone --recurse-submodules "$JPEGLI_GIT" "$clone"
    git -C "$clone" checkout --recurse-submodules "$JPEGLI_COMMIT"
  fi
  rsync -a --exclude '/.git' --exclude '/testdata' "$clone"/ "$jpegli_dest"/
fi
# Submodule checkouts carry a `.git` pointer file/dir; jpegli's CMake only tests
# that lcms/.git EXISTS. Keep a placeholder file so the plain export still builds.
find "$jpegli_dest" -name .git -type d -prune -exec rm -rf {} +
find "$jpegli_dest" -name .git -type f -exec sh -c 'echo "placeholder: source export, not a git checkout" > "$1"' _ {} \;
[[ -f "$jpegli_dest/CMakeLists.txt" ]] || { echo "error: jpegli source incomplete" >&2; exit 1; }

echo "==> cargo vendor --locked"
mkdir -p "$stage/.cargo"
(
  cd "$stage"
  cargo vendor --locked vendor-crates > .cargo/config.toml
)

cat > "$stage/BUILD_FROM_SOURCE.txt" <<TXT
ImageCompress ${version} - Corresponding Source (GPL-3.0-or-later)

Contents
  *                      the git tree of release tag v${version}
  vendor-crates/         all crates.io dependencies (cargo vendor --locked)
  crates/jpegli-sys/vendor/jpegli/
                         google/jpegli @ ${JPEGLI_COMMIT}, with submodules
  .cargo/config.toml     redirects crates.io to vendor-crates/ (offline builds)
  licenses/, COPYING, NOTICE.md   license texts and notices

gifsicle (GPL-2.0-only, a separate executable) is published as
gifsicle-${GIFSICLE_VERSION}-src.tar.gz next to this file, verbatim from
${GIFSICLE_URL}
(sha256 ${GIFSICLE_SHA256}). It is built with:
  ./configure --disable-gifview --disable-gifdiff && make

Build the command-line tool (offline, no network required)
  Requirements: Rust (stable), cmake, ninja, nasm, meson, a C/C++ compiler.
  cargo build --release --offline --locked -p imgcompress
  -> target/release/imgcompress

Static Linux (musl) binaries: scripts/build_linux.sh and
docker/linux-build.Dockerfile reproduce the release build in Alpine.
The macOS app: scripts/build_cli_mac.sh, then Xcode (macos/project.yml,
xcodegen). Signing and notarization are not required to run rebuilt binaries.
TXT

echo "==> creating $tarball"
rm -f "$tarball"
COPYFILE_DISABLE=1 tar -C "$stage_root" -czf "$tarball" "$name"

echo "==> gifsicle ${GIFSICLE_VERSION} source"
gif_cache="$cache_dir/gifsicle-${GIFSICLE_VERSION}.tar.gz"
if [[ ! -f "$gif_cache" ]]; then
  curl -fsSL -o "$gif_cache" "$GIFSICLE_URL"
fi
echo "${GIFSICLE_SHA256}  ${gif_cache}" | shasum -a 256 -c - >/dev/null \
  || { echo "error: gifsicle tarball sha256 mismatch" >&2; rm -f "$gif_cache"; exit 1; }
cp "$gif_cache" "${dist_dir}/gifsicle-${GIFSICLE_VERSION}-src.tar.gz"

ls -lh "$tarball" "${dist_dir}/gifsicle-${GIFSICLE_VERSION}-src.tar.gz"

if [[ $verify -eq 1 ]]; then
  command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 \
    || { echo "error: --verify needs a running docker" >&2; exit 1; }
  echo "==> verifying: offline build of the tarball in Alpine (network disabled)"
  verify_dir="$repo_root/build/verify"
  rm -rf "$verify_dir"; mkdir -p "$verify_dir"
  tar -xzf "$tarball" -C "$verify_dir"
  docker build -q -f docker/linux-build.Dockerfile --target toolchain -t imgcompress-linux-toolchain . >/dev/null
  # Build on the container's own filesystem: rustc mmap on a macOS bind mount
  # dies with SIGBUS. The source is mounted read-only and copied in first.
  # About one rustc job per 2 GB of the Docker VM (fat LTO is memory hungry), as in build_linux.sh.
  vm_bytes=$(docker info --format '{{.MemTotal}}' 2>/dev/null || echo 0)
  cargo_jobs=${CARGO_BUILD_JOBS:-$(( vm_bytes / 2147483648 ))}
  (( cargo_jobs >= 1 )) || cargo_jobs=1
  docker run --rm --network none -e CARGO_HOME=/tmp/cargo-home -e CARGO_BUILD_JOBS="$cargo_jobs" \
    -v "$verify_dir/$name":/src-ro:ro imgcompress-linux-toolchain \
    sh -c 'cp -a /src-ro /work && cd /work && triple=$(rustc -vV | sed -n "s/^host: //p") && cargo build --release --offline --locked -p imgcompress --target "$triple" && "./target/$triple/release/imgcompress" --version'
  rm -rf "$verify_dir"
  echo "==> offline source build verified"
fi

echo "==> sources: $tarball"
