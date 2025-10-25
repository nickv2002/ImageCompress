#!/bin/bash
set -e

# Set repository details
REPO="shssoichiro/oxipng"

# Check current version if oxipng is installed
echo "Checking current version..."
CURRENT_VERSION=""
if command -v oxipng >/dev/null 2>&1; then
    # Extract version from oxipng --version output
    CURRENT_VERSION_OUTPUT=$(oxipng --version 2>&1 | head -n1)
    if [[ -n "$CURRENT_VERSION_OUTPUT" ]]; then
        # Extract version number (e.g., "9.1.5" from "oxipng 9.1.5")
        CURRENT_VERSION=$(echo "$CURRENT_VERSION_OUTPUT" | grep -oP '\d+\.\d+\.\d+' | head -n1)
        CURRENT_VERSION="v$CURRENT_VERSION"
    fi
fi

if [[ -n "$CURRENT_VERSION" ]]; then
    echo "Current installed version: $CURRENT_VERSION"
else
    echo "oxipng not found or version could not be determined"
fi

# Fetch the latest release information from GitHub API
echo "Fetching the latest release information..."
RELEASE_INFO=$(curl -s -L "https://api.github.com/repos/$REPO/releases/latest")

# Extract the version tag (e.g., "v9.1.5")
LATEST_VERSION=$(echo "$RELEASE_INFO" | grep -Po '"tag_name": "\K.*?(?=")')

# Check if a version was found
if [[ -z "$LATEST_VERSION" ]]; then
    echo "Error: Could not determine the latest version."
    exit 1
fi

echo "Latest available version: $LATEST_VERSION"

# Compare versions if current version is available
if [[ -n "$CURRENT_VERSION" ]]; then
    if [[ "$CURRENT_VERSION" == "$LATEST_VERSION" ]]; then
        echo "You already have the latest version ($CURRENT_VERSION). No update needed."
        exit 0
    else
        echo "Update available: $CURRENT_VERSION -> $LATEST_VERSION"
    fi
else
    echo "Installing oxipng for the first time."
fi

# Get the download URL for the .deb package
ASSET_URL=$(echo "$RELEASE_INFO" | grep -Po '"browser_download_url": "\K[^"]*amd64\.deb')

if [[ -z "$ASSET_URL" ]]; then
    echo "Error: Could not find amd64.deb download URL"
    exit 1
fi

# Extract filename from URL
ASSET_NAME=$(basename "$ASSET_URL")

# Download the .deb file
echo "Downloading $ASSET_NAME..."
wget -O "$ASSET_NAME" "$ASSET_URL"

# Verify the download
if [[ $? -ne 0 ]]; then
    echo "Error: Failed to download $ASSET_NAME"
    exit 1
fi

# Install the .deb package
echo "Installing $ASSET_NAME..."
sudo dpkg -i "$ASSET_NAME"

# Clean up
echo "Cleaning up..."
rm "$ASSET_NAME"

echo "Done! oxipng $LATEST_VERSION installed successfully."
