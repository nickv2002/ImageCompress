#!/bin/bash
# Builds .deb and .rpm packages from the staged Linux tarball contents that scripts/build_linux.sh leaves in
# build/linux/stage/, using nfpm in a container (packaging/linux/nfpm.yaml). Run after build_linux.sh.
#
#   dist/imgcompress_<version>_amd64.deb   dist/imgcompress-<version>-1.x86_64.rpm
#   dist/imgcompress_<version>_arm64.deb   dist/imgcompress-<version>-1.aarch64.rpm
#
# Usage: scripts/build_linux_packages.sh <version>
# Env: ARCHES="x86_64 aarch64" overrides the architecture list (tarball names).
set -euo pipefail

version="${1:?usage: build_linux_packages.sh <version>}"
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

nfpm_image="goreleaser/nfpm:v2.48.0"
docker info >/dev/null 2>&1 || { echo "error: docker is not running" >&2; exit 1; }
mkdir -p dist

for arch in ${ARCHES:-x86_64 aarch64}; do
  stage="build/linux/stage/imgcompress-${version}-linux-${arch}"
  [[ -x "$stage/imgcompress" ]] || { echo "error: $stage missing (run scripts/build_linux.sh first)" >&2; exit 1; }
  [[ -d "$stage/licenses" ]] || { echo "error: $stage has no licenses/ (not releasable)" >&2; exit 1; }
  case "$arch" in x86_64) pkg_arch=amd64 ;; aarch64) pkg_arch=arm64 ;; *) echo "error: unknown arch $arch" >&2; exit 1 ;; esac
  # nfpm expands ${VERSION} and ${PKG_ARCH} itself, but not paths in `contents`, so fill in the stage here.
  config="build/linux/nfpm-${arch}.yaml"
  sed "s#\${STAGE}#/src/${stage}#g" packaging/linux/nfpm.yaml > "$config"
  for packager in deb rpm; do
    echo "==> ${packager} for linux/${arch}"
    docker run --rm -v "$repo_root:/src" -w /src \
      -e VERSION="$version" -e PKG_ARCH="$pkg_arch" \
      "$nfpm_image" package --config "$config" --packager "$packager" --target dist/
  done
done
ls -lh dist/imgcompress_"${version}"_*.deb dist/imgcompress-"${version}"-*.rpm
