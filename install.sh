#!/usr/bin/env bash
set -e

echo "=== Installing Arch Explorer ==="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Copy executable
sudo install -Dm755 "${SCRIPT_DIR}/arch-explorer" /usr/bin/arch-explorer

# Copy desktop file
if [ -f "${SCRIPT_DIR}/arch-explorer.desktop" ]; then
    sudo install -Dm644 "${SCRIPT_DIR}/arch-explorer.desktop" /usr/share/applications/arch-explorer.desktop
fi

# Copy icon
if [ -f "${SCRIPT_DIR}/arch-explorer.png" ]; then
    sudo install -Dm644 "${SCRIPT_DIR}/arch-explorer.png" /usr/share/pixmaps/arch-explorer.png
    sudo install -Dm644 "${SCRIPT_DIR}/arch-explorer.png" /usr/share/icons/hicolor/256x256/apps/arch-explorer.png
fi

# Update desktop database if available
if command -v update-desktop-database >/dev/null 2>&1; then
    sudo update-desktop-database -q /usr/share/applications || true
fi

echo "✓ Arch Explorer installed successfully!"
echo "Run with: arch-explorer"
