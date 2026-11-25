#!/bin/bash
# Signs the release's .rpm packages in dist/ with the repository key, so `gpgcheck=1` (dnf, zypper) can verify
# them and the copies attached to the GitHub release are signed too. Run after build_linux_packages.sh.
# (.deb packages are not signed individually; apt verifies them through the signed repository index.)
#
# Usage: scripts/sign_linux_packages.sh <version>
# Env: REPO_KEY_FILE=<armored private key> instead of 1Password (tests, or an unattended release).
set -euo pipefail

version="${1:?usage: sign_linux_packages.sh <version>}"
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
. scripts/repo_key.sh

shopt -s nullglob
rpms=(dist/imgcompress-"${version}"-*.rpm)
shopt -u nullglob
[[ ${#rpms[@]} -gt 0 ]] || { echo "error: no .rpm packages for ${version} in dist/" >&2; exit 1; }

repo_tools
key=$(repo_private_key)
[[ "$key" == *"PRIVATE KEY BLOCK"* ]] || { echo "error: could not read the repository signing key" >&2; exit 1; }

echo "==> signing ${#rpms[@]} rpm package(s)"
# The key goes in on stdin and lives only in the container's tmpfs keyring.
printf '%s\n' "$key" | docker run --rm -i --tmpfs /root/.gnupg:mode=0700 -v "$repo_root/dist:/dist" \
  -e FPR="$REPO_KEY_FPR" "$REPO_TOOLS_IMAGE" bash -euo pipefail -c '
    gpg --batch --quiet --import 2>/dev/null
    cd /dist
    rpmsign --addsign --define "__gpg /usr/bin/gpg" --define "_gpg_name $FPR" "$@" 2>&1 >/dev/null | grep -v GPG_TTY || true
    gpg --batch --quiet --armor --export "$FPR" > /tmp/pub.asc
    rpm --import /tmp/pub.asc
    for f in "$@"; do
      rpm -K "$f" | grep -q "signatures OK" || { echo "error: $f is not signed" >&2; exit 1; }
      echo "    signed $f"
    done
  ' sign "${rpms[@]#dist/}"
