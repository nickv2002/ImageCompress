#!/bin/bash
set -e

# jpegli (cjpegli) was split out of libjxl into its own repo and is no longer
# shipped as a prebuilt binary in libjxl releases, so we build it ourselves
# from source in Docker. See: https://github.com/google/jpegli
REPO_URL="https://github.com/google/jpegli.git"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCKERFILE="$SCRIPT_DIR/cjpegli-builder.Dockerfile"
VERSION_FILE="$SCRIPT_DIR/.cjpegli-built-commit"

if ! command -v docker >/dev/null 2>&1; then
    echo "Error: docker is required to build cjpegli but was not found."
    exit 1
fi

echo "Checking latest jpegli commit..."
LATEST_COMMIT=$(git ls-remote "$REPO_URL" HEAD | cut -f1)

if [[ -z "$LATEST_COMMIT" ]]; then
    echo "Error: Could not determine the latest jpegli commit."
    exit 1
fi

echo "Latest jpegli commit: $LATEST_COMMIT"

if [[ -f "$VERSION_FILE" ]] && command -v cjpegli >/dev/null 2>&1; then
    BUILT_COMMIT=$(cat "$VERSION_FILE")
    if [[ "$BUILT_COMMIT" == "$LATEST_COMMIT" ]]; then
        echo "cjpegli is already built from the latest commit ($LATEST_COMMIT). No update needed."
        exit 0
    fi
fi

PREV_IMAGE_ID=$(docker images -q cjpegli-builder)

echo "Building cjpegli from source via Docker (this can take a few minutes)..."
docker build -f "$DOCKERFILE" -t cjpegli-builder --target build \
    --build-arg "JPEGLI_COMMIT=$LATEST_COMMIT" "$SCRIPT_DIR"

CONTAINER_ID=$(docker create cjpegli-builder)
trap 'docker rm "$CONTAINER_ID" >/dev/null 2>&1 || true' EXIT

TMP_BIN="$(mktemp)"
docker cp "$CONTAINER_ID:/src/jpegli/build/tools/cjpegli" "$TMP_BIN"
chmod +x "$TMP_BIN"

echo "Installing cjpegli..."
sudo mv "$TMP_BIN" /usr/local/bin/cjpegli

echo "$LATEST_COMMIT" > "$VERSION_FILE"

# Drop the now-orphaned previous image so repeated updates don't pile up
# multi-GB dangling images.
NEW_IMAGE_ID=$(docker images -q cjpegli-builder)
if [[ -n "$PREV_IMAGE_ID" && "$PREV_IMAGE_ID" != "$NEW_IMAGE_ID" ]]; then
    docker rmi "$PREV_IMAGE_ID" >/dev/null 2>&1 || true
fi

echo "Done! cjpegli built from jpegli@$LATEST_COMMIT installed successfully."
