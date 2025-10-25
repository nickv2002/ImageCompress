#!/bin/bash
set -e

echo "Installing image compression tools..."

# Install tools available via apt
echo "Installing pngquant and gifsicle via apt..."
sudo apt update
sudo apt install -y pngquant gifsicle

# Install oxipng from GitHub releases
echo "Installing oxipng from GitHub releases..."
./oxipng-static-downloader.sh

# Install cavif from GitHub releases
echo "Installing cavif from GitHub releases..."
./cavif-static-downloader.sh

# Install JXL tools using existing script
echo "Installing JXL tools..."
./jxl-static-downloader.sh

echo "All image compression tools installed successfully!"
echo "Installed tools:"
echo "- pngquant: $(pngquant --version 2>/dev/null || echo 'not found')"
echo "- gifsicle: $(gifsicle --version 2>/dev/null | head -n1 || echo 'not found')"
echo "- oxipng: $(oxipng --version 2>/dev/null || echo 'not found')"
echo "- cavif: $(cavif --version 2>/dev/null || echo 'not found')"
echo "- cjxl: $(cjxl --version 2>/dev/null | head -n1 || echo 'not found')"
echo "- cjpegli: $(cjpegli --version 2>/dev/null | head -n1 || echo 'not found')"
