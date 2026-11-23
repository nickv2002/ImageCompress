#!/bin/bash
# One-time setup: creates the OpenPGP key that signs the apt/dnf repository and the .rpm packages, and stores the
# private key and its revocation certificate in 1Password. Nothing secret touches the disk: gpg runs in a container
# with its keyring in tmpfs, and the item JSON is piped straight to `op item create`.
#
# RSA 4096 (not Ed25519) so rpm on RHEL 8 / openSUSE Leap can verify it; no expiry, because an expired repo key
# breaks `apt update` / `dnf upgrade` for every user.
#
# Usage: scripts/make_repo_key.sh
# Env (or scripts/release.local.env): OP_ACCOUNT, OP_VAULT, REPO_KEY_ITEM (default "Linux package signing key").
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
# shellcheck disable=SC1091
[ -f scripts/release.local.env ] && . scripts/release.local.env
item="${REPO_KEY_ITEM:-Linux package signing key}"
op_args=(${OP_ACCOUNT:+--account "$OP_ACCOUNT"})

# Unlock first so a timed-out approval stops here instead of reading as "no such item".
op vault get "${op_args[@]}" "${OP_VAULT:-Private}" >/dev/null || { echo "error: 1Password is not unlocked" >&2; exit 1; }
if op item get "${op_args[@]}" ${OP_VAULT:+--vault "$OP_VAULT"} "$item" >/dev/null 2>&1; then
  echo "error: 1Password item '$item' already exists; refusing to replace the repository key" >&2
  exit 1
fi

docker build -q -t imagecompress-repo-tools -f docker/repo-tools.Dockerfile docker >/dev/null
docker run --rm -i --tmpfs /root/.gnupg:mode=0700 -e ITEM="$item" imagecompress-repo-tools bash -euo pipefail -c '
  gpg --batch --quiet --passphrase "" --quick-generate-key \
    "nickv2002 packages <704392+nickv2002@users.noreply.github.com>" rsa4096 sign never 2>/dev/null
  fpr=$(gpg --list-keys --with-colons 2>/dev/null | awk -F: "/^fpr/ {print \$10; exit}")
  jq -n --arg title "$ITEM" --arg fpr "$fpr" \
    --arg priv "$(gpg --armor --export-secret-keys "$fpr")" \
    --arg pub "$(gpg --armor --export "$fpr")" \
    --arg rev "$(cat /root/.gnupg/openpgp-revocs.d/"$fpr".rev)" \
    "{title: \$title, category: \"SECURE_NOTE\",
      fields: [
        {id: \"notesPlain\", type: \"STRING\", purpose: \"NOTES\",
         value: \"Signs the apt/dnf repository at nickv2002.github.io/packages and the imgcompress .rpm packages. Used by ImageCompress scripts/sign_linux_packages.sh and publish_linux_packages.sh.\"},
        {label: \"fingerprint\", type: \"STRING\", value: \$fpr},
        {label: \"private key\", type: \"CONCEALED\", value: \$priv},
        {label: \"public key\", type: \"STRING\", value: \$pub},
        {label: \"revocation certificate\", type: \"CONCEALED\", value: \$rev}
      ]}"
' | op item create "${op_args[@]}" ${OP_VAULT:+--vault "$OP_VAULT"} --format json - >/dev/null

fpr=$(op item get "${op_args[@]}" ${OP_VAULT:+--vault "$OP_VAULT"} "$item" --fields label=fingerprint)
echo "==> created '$item' (fingerprint $fpr)"
