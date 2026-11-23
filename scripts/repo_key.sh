#!/bin/bash
# Sourced by sign_linux_packages.sh and publish_linux_packages.sh: settings for the OpenPGP key that signs the
# apt/dnf repository and the .rpm packages (created once by make_repo_key.sh, kept in 1Password).
#
# repo_private_key  prints the armored private key (REPO_KEY_FILE if set, for tests; otherwise 1Password).
# repo_tools        builds the container image the signing and indexing run in.

# shellcheck disable=SC1091
[ -f scripts/release.local.env ] && . scripts/release.local.env
REPO_KEY_ITEM="${REPO_KEY_ITEM:-Linux package signing key}"
REPO_KEY_FPR="${REPO_KEY_FPR:-01F93355749B96C11D12B14D63E5419C7998DC58}"
REPO_TOOLS_IMAGE=imagecompress-repo-tools

repo_private_key() {
  if [[ -n "${REPO_KEY_FILE:-}" ]]; then
    cat "$REPO_KEY_FILE"
  else
    # If 1Password is still waiting for approval after a minute, get the owner's attention.
    local nag
    ( sleep 60; command -v afplay >/dev/null 2>&1 && afplay /System/Library/Sounds/Glass.aiff ) >/dev/null 2>&1 &
    nag=$!
    op read ${OP_ACCOUNT:+--account "$OP_ACCOUNT"} "op://${OP_VAULT:-Private}/${REPO_KEY_ITEM}/private key"
    local rc=$?
    kill "$nag" 2>/dev/null || true
    return $rc
  fi
}

repo_tools() {
  docker build -q -t "$REPO_TOOLS_IMAGE" -f docker/repo-tools.Dockerfile docker >/dev/null
}
