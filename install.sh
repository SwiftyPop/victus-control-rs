#!/bin/bash

set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
    echo "This script requires root privileges. Re-running with sudo..."
    exec sudo "$0" "$@"
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${script_dir}"

echo "--> Bootstrapping Victus Control Rust Installer..."

if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    sudo -u "${SUDO_USER}" cargo build --package victus-installer
else
    cargo build --package victus-installer
fi

exec "./target/debug/victus-installer" "$@"
