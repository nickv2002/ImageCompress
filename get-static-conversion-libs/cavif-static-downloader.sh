#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/deb-release-installer.sh"

REPO="kornelski/cavif-rs"
TOOL_NAME="cavif"

install_latest_deb_release
