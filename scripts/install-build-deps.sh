#!/usr/bin/env bash
# Install everything needed to compile wdroid on Debian/Ubuntu.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libxkbcommon-dev \
    libegl-dev

# Optional but handy for testing the compositor with simple clients.
sudo apt-get install -y wayland-utils weston || true

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust toolchain not found — installing via rustup…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    echo "Run 'source \$HOME/.cargo/env' (or open a new shell) before building."
else
    echo "Rust toolchain already present: $(cargo --version)"
fi

echo "Build dependencies installed. Compile with: cargo build --release"
