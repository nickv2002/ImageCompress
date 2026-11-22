#!/bin/bash
# Uploads the release's .deb and .rpm packages to the Cloudsmith repository that apt and dnf users install from,
# so `apt upgrade` / `dnf upgrade` pick up new releases, plus the GPL source tarballs beside them.
#
# Usage: scripts/publish_linux_packages.sh <version>
#
# Needs the cloudsmith CLI (`pipx install cloudsmith-cli`) and an API key in CLOUDSMITH_API_KEY, or a 1Password
# reference in CLOUDSMITH_OP_REF (e.g. "op://<vault>/<item>/credential", read with OP_ACCOUNT if set). Both can live in
# scripts/release.local.env (gitignored). Fails open: without the CLI or a key it prints a warning and exits 0, so a
# release is never blocked on it (the packages are still attached to the GitHub release).
#
# Env: CLOUDSMITH_REPO (default nickv2002/imagecompress, i.e. <owner>/<repo>).
set -euo pipefail

version="${1:?usage: publish_linux_packages.sh <version>}"
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
# shellcheck disable=SC1091
[ -f scripts/release.local.env ] && . scripts/release.local.env
repo="${CLOUDSMITH_REPO:-nickv2002/imagecompress}"

if ! command -v cloudsmith >/dev/null 2>&1; then
  echo "    warning: cloudsmith CLI not found; Linux packages not published to ${repo}"
  exit 0
fi
if [[ -z "${CLOUDSMITH_API_KEY:-}" && -n "${CLOUDSMITH_OP_REF:-}" ]] && command -v op >/dev/null 2>&1; then
  CLOUDSMITH_API_KEY=$(op read ${OP_ACCOUNT:+--account "$OP_ACCOUNT"} "$CLOUDSMITH_OP_REF" 2>/dev/null || true)
fi
if [[ -z "${CLOUDSMITH_API_KEY:-}" ]]; then
  echo "    warning: no CLOUDSMITH_API_KEY; Linux packages not published to ${repo}"
  exit 0
fi
export CLOUDSMITH_API_KEY

shopt -s nullglob
debs=(dist/imgcompress_"${version}"_*.deb)
rpms=(dist/imgcompress-"${version}"-*.rpm)
shopt -u nullglob
[[ ${#debs[@]} -gt 0 && ${#rpms[@]} -gt 0 ]] || { echo "error: no packages for ${version} in dist/" >&2; exit 1; }

# GPL: the Corresponding Source (and gifsicle's, GPL-2.0-only) goes in the same place as the binaries,
# uploaded first so a failure never leaves packages there without it.
shopt -s nullglob
sources=(dist/imgcompress-"${version}"-sources.tar.gz dist/gifsicle-*-src.tar.gz)
shopt -u nullglob
[[ ${#sources[@]} -ge 2 ]] || { echo "error: source tarballs for ${version} missing from dist/" >&2; exit 1; }
for f in "${sources[@]}"; do
  echo "==> cloudsmith (source): $(basename "$f")"
  cloudsmith push raw "$repo" "$f"
done
# The packages are static and dependency-free, so one "any distro" upload serves every release of each family.
for f in "${debs[@]}"; do
  echo "==> cloudsmith: $(basename "$f")"
  cloudsmith push deb "${repo}/any-distro/any-version" "$f"
done
for f in "${rpms[@]}"; do
  echo "==> cloudsmith: $(basename "$f")"
  cloudsmith push rpm "${repo}/any-distro/any-version" "$f"
done
