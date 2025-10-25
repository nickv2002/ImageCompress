#!/bin/bash
set -e

echo "Installing image compression tools using individual downloaders..."
echo "This replaces the previous Docker-based approach with direct package installation."

# Run the master installation script
./install-all-tools.sh

echo "Installation complete!"
echo "Note: mozjpeg (cjpeg-static, jpegtran-static) has been removed since we now use jpegli from JXL instead."
