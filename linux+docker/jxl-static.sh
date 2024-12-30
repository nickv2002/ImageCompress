#!/bin/bash
set -e

# Set repository details
REPO="libjxl/libjxl"

# Fetch the latest release information from GitHub API
echo "Fetching the latest release information..."
RELEASE_INFO=$(curl -s "https://api.github.com/repos/$REPO/releases/latest")

# Extract the version tag (e.g., "v0.11.1")
LATEST_VERSION=$(echo "$RELEASE_INFO" | grep -Po '"tag_name": "\K.*?(?=")')

# Check if a version was found
if [[ -z "$LATEST_VERSION" ]]; then
    echo "Error: Could not determine the latest version."
    exit 1
fi

# Construct the download URL for the static binary
ASSET_NAME="jxl-linux-x86_64-static-${LATEST_VERSION}.tar.gz"
ASSET_URL="https://github.com/$REPO/releases/download/$LATEST_VERSION/$ASSET_NAME"

# Download the asset
echo "Downloading $ASSET_NAME..."
curl -Ss -L -o "$ASSET_NAME" "$ASSET_URL"

# Verify the download
if [[ $? -ne 0 ]]; then
    echo "Error: Failed to download $ASSET_NAME"
    exit 1
fi

# Extract the downloaded tar.gz file
echo "Extracting $ASSET_NAME..."

EXTRACTED_DIR="$PWD/jxl-static"
mkdir -p $EXTRACTED_DIR
tar -xzf "$ASSET_NAME" --directory $EXTRACTED_DIR

if [[ ! -d "$EXTRACTED_DIR/tools" ]]; then
    echo "Error: 'tools' directory not found in the archive."
    exit 1
fi

cd "$EXTRACTED_DIR/tools"
sudo mv -v cjpegli cjxl /usr/local/bin/
cd ../..

echo "Cleaning up..."
rm -rf "$EXTRACTED_DIR" "$ASSET_NAME"

echo "Done!"
