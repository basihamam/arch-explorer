#!/usr/bin/env bash
# ============================================================
#  Arch Explorer — Install Script
#  Arch Linux only. Kurmak için: chmod +x install.sh && ./install.sh
# ============================================================
set -e
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

echo -e "${CYAN}◈ Arch Explorer Setup${NC}"
echo "=============================="

# -- Dependencies check --
echo -e "\n${YELLOW}[1/5] Checking dependencies...${NC}"

check_cmd() {
  if ! command -v "$1" &>/dev/null; then
    echo -e "  ${RED}✕ $1 not found — installing...${NC}"
    return 1
  else
    echo -e "  ${GREEN}✓ $1${NC}"
    return 0
  fi
}

# Rust
check_cmd rustc || {
  echo "Installing Rust via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
}

# Node
check_cmd node || sudo pacman -S --noconfirm nodejs npm

# Tauri deps
echo -e "\n${YELLOW}[2/5] Installing Tauri system dependencies...${NC}"
sudo pacman -S --needed --noconfirm \
  webkit2gtk-4.1 \
  base-devel \
  curl \
  wget \
  openssl \
  appmenu-gtk-module \
  gtk3 \
  libayatana-appindicator \
  librsvg \
  xdotool \
  ntfs-3g \
  udisks2

# Make sure udisks2 is running
sudo systemctl enable --now udisks2 2>/dev/null || true

echo -e "\n${YELLOW}[3/5] Installing npm packages...${NC}"
npm install

echo -e "\n${YELLOW}[4/5] Adding Tauri CLI...${NC}"
npm install -g @tauri-apps/cli@latest 2>/dev/null || true

echo -e "\n${YELLOW}[5/5] Build check...${NC}"
echo -e "  Run ${CYAN}npm run tauri dev${NC} to start development mode"
echo -e "  Run ${CYAN}npm run tauri build${NC} to build release binary"

# Add user to disk group (for udisksctl without sudo)
if ! groups | grep -q disk; then
  echo -e "\n${YELLOW}Adding you to 'disk' group (for mount operations)...${NC}"
  sudo usermod -aG disk "$USER"
  echo -e "  ${YELLOW}⚠ Log out and back in for group changes to take effect${NC}"
fi

echo -e "\n${GREEN}=============================="
echo -e "◈ Setup complete!"
echo -e ""
echo -e "Dev:     npm run tauri dev"
echo -e "Build:   npm run tauri build"
echo -e "==============================${NC}"
