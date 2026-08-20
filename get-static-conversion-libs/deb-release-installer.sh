#!/bin/bash
# Shared logic for installing a tool from a GitHub release's amd64 .deb asset.
# Sourced by oxipng-static-downloader.sh and cavif-static-downloader.sh, which
# set REPO and TOOL_NAME then call install_latest_deb_release.

install_latest_deb_release() {
    if ! command -v jq >/dev/null 2>&1; then
        echo "Error: jq is required but not installed. Install it with: sudo apt install jq"
        exit 1
    fi

    echo "Checking current version..."
    CURRENT_VERSION=""
    if command -v "$TOOL_NAME" >/dev/null 2>&1; then
        CURRENT_VERSION_OUTPUT=$("$TOOL_NAME" --version 2>&1 | head -n1)
        if [[ -n "$CURRENT_VERSION_OUTPUT" ]]; then
            CURRENT_VERSION=$(echo "$CURRENT_VERSION_OUTPUT" | grep -oP '\d+\.\d+\.\d+' | head -n1)
            CURRENT_VERSION="v$CURRENT_VERSION"
        fi
    fi

    if [[ -n "$CURRENT_VERSION" ]]; then
        echo "Current installed version: $CURRENT_VERSION"
    else
        echo "$TOOL_NAME not found or version could not be determined"
    fi

    echo "Fetching the latest release information..."
    RELEASE_INFO=$(curl -s -L --connect-timeout 10 --max-time 30 --retry 2 --retry-delay 2 "https://api.github.com/repos/$REPO/releases/latest")

    LATEST_VERSION=$(echo "$RELEASE_INFO" | jq -r '.tag_name // empty')

    if [[ -z "$LATEST_VERSION" ]]; then
        echo "Error: Could not determine the latest version."
        exit 1
    fi

    echo "Latest available version: $LATEST_VERSION"

    if [[ -n "$CURRENT_VERSION" ]]; then
        if [[ "$CURRENT_VERSION" == "$LATEST_VERSION" ]]; then
            echo "You already have the latest version ($CURRENT_VERSION). No update needed."
            exit 0
        else
            echo "Update available: $CURRENT_VERSION -> $LATEST_VERSION"
        fi
    else
        echo "Installing $TOOL_NAME for the first time."
    fi

    ASSET_URL=$(echo "$RELEASE_INFO" | jq -r '.assets[].browser_download_url | select(endswith("amd64.deb"))' | head -n1)

    if [[ -z "$ASSET_URL" ]]; then
        echo "Error: Could not find amd64.deb download URL"
        exit 1
    fi

    ASSET_NAME=$(basename "$ASSET_URL")

    echo "Downloading $ASSET_NAME..."
    wget --timeout=30 --tries=3 -O "$ASSET_NAME" "$ASSET_URL"

    echo "Installing $ASSET_NAME..."
    sudo dpkg -i "$ASSET_NAME"

    echo "Cleaning up..."
    rm "$ASSET_NAME"

    echo "Done! $TOOL_NAME $LATEST_VERSION installed successfully."
}
