#!/bin/bash
set -e

# Set repository details
REPO="libjxl/libjxl"

# Check current version if tools are installed
echo "Checking current version..."
CURRENT_VERSION=""
if command -v cjxl >/dev/null 2>&1; then
    # Extract version from cjxl --version output (format: "cjxl v0.11.1 794a5dc [AVX2,SSE4,SSE2]")
    CURRENT_VERSION_OUTPUT=$(cjxl --version 2>&1 | head -n1)
    if [[ -n "$CURRENT_VERSION_OUTPUT" ]]; then
        # Extract version number from the first line (e.g., "v0.11.1" from "cjxl v0.11.1 ...")
        CURRENT_VERSION=$(echo "$CURRENT_VERSION_OUTPUT" | grep -oP 'v\d+\.\d+\.\d+' | head -n1)
    fi
fi

if [[ -n "$CURRENT_VERSION" ]]; then
    echo "Current installed version: $CURRENT_VERSION"
else
    echo "JXL tools not found or version could not be determined"
fi

# Fetch the latest release information from GitHub API
echo "Fetching the latest release information..."
RELEASE_INFO=$(curl -s --connect-timeout 10 --max-time 30 --retry 2 --retry-delay 2 "https://api.github.com/repos/$REPO/releases/latest")

if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not installed. Install it with: sudo apt install jq"
    exit 1
fi

# Extract the version tag (e.g., "v0.11.1")
LATEST_VERSION=$(echo "$RELEASE_INFO" | jq -r '.tag_name // empty')

# Check if a version was found
if [[ -z "$LATEST_VERSION" ]]; then
    echo "Error: Could not determine the latest version."
    exit 1
fi

echo "Latest available version: $LATEST_VERSION"

# Compare versions if current version is available
if [[ -n "$CURRENT_VERSION" ]]; then
    # Remove 'v' prefix from both versions for comparison
    CURRENT_CLEAN=$(echo "$CURRENT_VERSION" | sed 's/^v//')
    LATEST_CLEAN=$(echo "$LATEST_VERSION" | sed 's/^v//')

    if [[ "$CURRENT_CLEAN" == "$LATEST_CLEAN" ]]; then
        echo "You already have the latest version ($CURRENT_VERSION). No update needed."
        exit 0
    else
        echo "Update available: $CURRENT_VERSION -> $LATEST_VERSION"
    fi
else
    echo "Installing JXL tools for the first time."
fi

# Construct the download URL for the static binary
ASSET_NAME="jxl-linux-x86_64-static.tar.lz"
ASSET_URL="https://github.com/$REPO/releases/download/$LATEST_VERSION/$ASSET_NAME"

# tar needs the `lzip` binary to extract .tar.lz archives
if ! command -v lzip >/dev/null 2>&1; then
    echo "Installing lzip (required to extract .tar.lz archive)..."
    sudo apt update
    sudo apt install -y lzip
fi

# Download the asset
echo "Downloading $ASSET_NAME..."
curl -Ss -L --connect-timeout 10 --max-time 120 --retry 2 --retry-delay 2 -o "$ASSET_NAME" "$ASSET_URL"

# Verify the download
if [[ $? -ne 0 ]]; then
    echo "Error: Failed to download $ASSET_NAME"
    exit 1
fi

# Extract the downloaded tar.gz file
echo "Extracting $ASSET_NAME..."

EXTRACTED_DIR="$PWD/jxl-static"
mkdir -p "$EXTRACTED_DIR"
tar -xf "$ASSET_NAME" --lzip --directory "$EXTRACTED_DIR"

if [[ ! -d "$EXTRACTED_DIR/tools" ]]; then
    echo "Error: 'tools' directory not found in the archive."
    exit 1
fi

cd "$EXTRACTED_DIR/tools"

if [[ ! -f cjxl ]]; then
    echo "Error: 'cjxl' not found in the archive."
    exit 1
fi

sudo mv -v cjxl /usr/local/bin/
cd ../..

echo "Cleaning up..."
rm -rf "$EXTRACTED_DIR" "$ASSET_NAME"

echo "Done!"
