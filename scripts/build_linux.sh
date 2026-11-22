#!/bin/bash
# Builds static musl Linux tarballs (x86_64 + aarch64) of imgcompress with a
# static gifsicle, inside Alpine containers (docker/linux-build.Dockerfile).
#
#   dist/imgcompress-<version>-linux-x86_64.tar.gz
#   dist/imgcompress-<version>-linux-aarch64.tar.gz
#
# Each tarball: imgcompress, gifsicle, completions/, man/ (if the CLI emits
# them), licenses/ (COPYING, GPL-2.0, LGPL-3.0, THIRD_PARTY_LICENSES.html,
# NOTICE.md, manual/, vendored/).
#
# Usage: scripts/build_linux.sh [--native] [--no-notices] <version>
#   --native       only the host architecture (no QEMU emulation; much faster)
#   --no-notices   skip the licenses bundle (needs cargo-deny/cargo-about, which
#                  Linux build hosts usually lack); the tarball then has no licenses/
# Env: ARCHES="amd64 arm64" overrides the architecture list.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

native=0
notices=1
version=""
for arg in "$@"; do
  case "$arg" in
    --native) native=1 ;;
    --no-notices) notices=0 ;;
    -*) echo "error: unknown option $arg" >&2; exit 2 ;;
    *) version="$arg" ;;
  esac
done
[[ -n "$version" ]] || { echo "usage: build_linux.sh [--native] [--no-notices] <version>" >&2; exit 2; }

docker info >/dev/null 2>&1 || { echo "error: docker is not running" >&2; exit 1; }

set -a
# shellcheck source=scripts/versions.env
. scripts/versions.env
set +a

host_arch=$(uname -m)
case "$host_arch" in
  arm64|aarch64) host_docker_arch=arm64 ;;
  x86_64|amd64) host_docker_arch=amd64 ;;
  *) echo "error: unsupported host arch $host_arch" >&2; exit 1 ;;
esac
if [[ $native -eq 1 ]]; then
  arches="$host_docker_arch"
else
  arches="${ARCHES:-amd64 arm64}"
fi

# Notices bundle (also gates on cargo-deny); reuse if already produced this run.
if [[ $notices -eq 1 && ! -d build/licenses ]]; then
  scripts/collect_notices.sh
fi

dist_dir="dist"
mkdir -p "$dist_dir" build/linux

for arch in $arches; do
  case "$arch" in
    amd64) out_arch=x86_64 ;;
    arm64) out_arch=aarch64 ;;
    *) echo "error: unknown arch $arch" >&2; exit 1 ;;
  esac
  out="build/linux/${out_arch}"
  rm -rf "$out"
  echo "==> building linux/${arch} (static musl) -> ${out}"
  docker buildx build \
    --platform "linux/${arch}" \
    -f docker/linux-build.Dockerfile \
    --target artifacts \
    --build-arg GIFSICLE_VERSION="$GIFSICLE_VERSION" \
    --build-arg GIFSICLE_URL="$GIFSICLE_URL" \
    --build-arg GIFSICLE_SHA256="$GIFSICLE_SHA256" \
    --output "type=local,dest=${out}" \
    .

  [[ -x "$out/imgcompress" && -x "$out/gifsicle" ]] || { echo "error: build output missing in $out" >&2; exit 1; }

  # Independent static check on the extracted ELF (host `file` may not exist on all hosts).
  if command -v file >/dev/null 2>&1; then
    desc=$(file "$out/imgcompress")
    echo "    $desc"
    grep -qE 'statically linked|static-pie linked' <<<"$desc" \
      || { echo "error: $out/imgcompress is not statically linked" >&2; exit 1; }
  fi

  name="imgcompress-${version}-linux-${out_arch}"
  stage="build/linux/stage/${name}"
  rm -rf "$stage"
  mkdir -p "$stage"
  cp "$out/imgcompress" "$out/gifsicle" "$stage/"
  [[ -d "$out/completions" ]] && cp -R "$out/completions" "$stage/"
  [[ -d "$out/man" ]] && cp -R "$out/man" "$stage/"
  if [[ $notices -eq 1 ]]; then cp -R build/licenses "$stage/licenses"; else echo "    warning: --no-notices: tarball has no licenses/ (not releasable)"; fi
  tarball="${dist_dir}/${name}.tar.gz"
  COPYFILE_DISABLE=1 tar -C build/linux/stage -czf "$tarball" "$name"
  ls -lh "$tarball"
done

echo "==> linux tarballs written to ${dist_dir}/"
