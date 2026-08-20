#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/deb-release-installer.sh"

REPO="shssoichiro/oxipng"
TOOL_NAME="oxipng"

install_latest_deb_release
