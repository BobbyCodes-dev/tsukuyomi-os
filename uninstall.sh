#!/usr/bin/env bash
set -euo pipefail

# Tsukuyomi OS — Linux uninstall script
# Removes the binary and optionally all data

BINARY_NAME="tsukuyomi"
INSTALL_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/TsukuyomiOS"
CONFIG_DIR="${HOME}/.config/TsukuyomiOS"
CACHE_DIR="${HOME}/.cache/TsukuyomiOS"

echo "=== Tsukuyomi OS — Linux Uninstaller ==="
echo ""

# Remove binary
if [[ -f "${INSTALL_DIR}/${BINARY_NAME}" ]]; then
    rm -f "${INSTALL_DIR}/${BINARY_NAME}"
    echo "[✓] Removed binary: ${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "[—] Binary not found at ${INSTALL_DIR}/${BINARY_NAME}"
fi

# Ask about data removal
KEEP_DATA=false
if [[ "${1:-}" == "--nuke" ]]; then
    KEEP_DATA=false
elif [[ "${1:-}" == "--keep-data" ]]; then
    KEEP_DATA=true
else
    echo "Data directories:"
    [[ -d "${DATA_DIR}" ]]   && echo "  ${DATA_DIR}"   || echo "  ${DATA_DIR} (not present)"
    [[ -d "${CONFIG_DIR}" ]] && echo "  ${CONFIG_DIR}" || echo "  ${CONFIG_DIR} (not present)"
    [[ -d "${CACHE_DIR}" ]]  && echo "  ${CACHE_DIR}"  || echo "  ${CACHE_DIR} (not present)"
    echo ""
    read -rp "Remove all Tsukuyomi OS data? (type NUKE to confirm): " answer
    if [[ "${answer}" == "NUKE" ]]; then
        KEEP_DATA=false
    else
        KEEP_DATA=true
        echo "[—] Keeping data directories."
    fi
fi

if [[ "${KEEP_DATA}" == false ]]; then
    for dir in "${DATA_DIR}" "${CONFIG_DIR}" "${CACHE_DIR}"; do
        if [[ -d "${dir}" ]]; then
            rm -rf "${dir}"
            echo "[✓] Removed: ${dir}"
        fi
    done
fi

echo ""
echo "=== Uninstallation Complete ==="
if [[ "${KEEP_DATA}" == true ]]; then
    echo "Data was preserved. Remove manually if needed:"
    echo "  rm -rf ${DATA_DIR} ${CONFIG_DIR} ${CACHE_DIR}"
fi
echo ""