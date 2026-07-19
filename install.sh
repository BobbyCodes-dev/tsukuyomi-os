#!/usr/bin/env bash
set -euo pipefail

# Tsukuyomi OS — Linux install script
# Installs the binary to ~/.local/bin and sets up data directories

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/TsukuyomiOS"
CONFIG_DIR="${HOME}/.config/TsukuyomiOS"

BINARY_NAME="tsukuyomi"

echo "=== Tsukuyomi OS — Linux Installer ==="
echo ""

# Create directories
mkdir -p "${INSTALL_DIR}" "${DATA_DIR}/tools" "${DATA_DIR}/vm" "${CONFIG_DIR}"
echo "[✓] Created data directories:"
echo "    ${DATA_DIR}"
echo "    ${CONFIG_DIR}"

# Build if binary doesn't exist
BINARY_PATH="${SCRIPT_DIR}/target/release/${BINARY_NAME}"
if [[ ! -f "${BINARY_PATH}" ]]; then
    echo ""
    echo "[*] Binary not found, building from source..."
    cd "${SCRIPT_DIR}"
    cargo build --release
fi

if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "[✗] Build failed — binary not found at ${BINARY_PATH}"
    exit 1
fi

# Install binary
cp "${BINARY_PATH}" "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
echo "[✓] Installed binary to ${INSTALL_DIR}/${BINARY_NAME}"

# Check PATH
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        echo "[✓] ${INSTALL_DIR} is already in PATH"
        ;;
    *)
        echo ""
        echo "[!] ${INSTALL_DIR} is NOT in your PATH"
        echo "    Add this line to your ~/.bashrc or ~/.zshrc:"
        echo '    export PATH="$HOME/.local/bin:$PATH"'
        echo ""
        # Try adding it automatically
        SHELL_RC="${HOME}/.bashrc"
        if [[ -f "${HOME}/.zshrc" ]] && [[ "${SHELL}" == */zsh ]]; then
            SHELL_RC="${HOME}/.zshrc"
        fi
        if ! grep -q '.local/bin' "${SHELL_RC}" 2>/dev/null; then
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "${SHELL_RC}"
            echo "[✓] Added ${INSTALL_DIR} to ${SHELL_RC}"
            echo "    Run 'source ${SHELL_RC}' or restart your terminal."
        fi
        ;;
esac

# Check for optional dependencies
echo ""
echo "=== Optional Dependencies ==="

check_dep() {
    local dep="$1"
    local install_hint="$2"
    if command -v "${dep}" &>/dev/null; then
        echo "[✓] ${dep} — installed"
    else
        echo "[ ] ${dep} — not found (${install_hint})"
    fi
}

check_dep "curl" "required for tool downloads"
check_dep "nmap" "for network scanning: apt install nmap"
check_dep "ufw" "for firewall management: apt install ufw"
check_dep "rsync" "for backups: apt install rsync"
check_dep "qemu-system-x86_64" "for VM sandbox: apt install qemu-system-x86"
check_dep "virsh" "for libvirt VM management: apt install libvirt-clients"

echo ""
echo "=== Installation Complete ==="
echo "Run 'tsukuyomi' to start Tsukuyomi OS"
echo ""