#!/usr/bin/env bash
# Install wdroid for the current user: runtime dependencies, the binary,
# the waydroid-up launcher (only if you don't already have one), a desktop
# entry, and — under WSL — a Windows shortcut.
#
# Run from the repository root after `cargo build --release`.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ ! -x target/release/wdroid ]; then
    echo "target/release/wdroid not found — build it first:" >&2
    echo "  scripts/install-build-deps.sh && cargo build --release" >&2
    exit 1
fi

echo "Installing runtime dependencies…"
# wl-clipboard powers the clipboard bridge and waydroid's own pyclip sync
sudo apt-get install -y libegl1 libxkbcommon0 libgles2 wl-clipboard

echo "Installing binary to ~/.local/bin/wdroid…"
install -D target/release/wdroid "$HOME/.local/bin/wdroid"

# Session launcher: never clobber an existing one — users may have their own.
if [ ! -e "$HOME/.local/bin/waydroid-up" ]; then
    echo "Installing waydroid-up launcher…"
    install -D scripts/waydroid-up "$HOME/.local/bin/waydroid-up"
else
    echo "Keeping existing ~/.local/bin/waydroid-up"
fi

# Clipboard bridge (Windows <-> WSLg <-> wdroid); same never-clobber policy.
if [ ! -e "$HOME/.local/bin/weston-clip-bridge" ]; then
    echo "Installing weston-clip-bridge…"
    install -D scripts/weston-clip-bridge "$HOME/.local/bin/weston-clip-bridge"
else
    echo "Keeping existing ~/.local/bin/weston-clip-bridge"
fi

echo "Installing desktop entry…"
mkdir -p "$HOME/.local/share/applications"
# Absolute Exec path: desktop launchers don't run login shells, so
# ~/.local/bin may not be in their PATH.
sed "s|Exec=/usr/bin/|Exec=$HOME/.local/bin/|" packaging/wdroid.desktop \
    > "$HOME/.local/share/applications/wdroid.desktop"

if grep -qi microsoft /proc/version 2>/dev/null; then
    echo "WSL detected — creating Windows shortcut…"
    scripts/install-windows-shortcut.sh || \
        echo "Windows shortcut failed (non-fatal); run scripts/install-windows-shortcut.sh manually."
fi

echo "Done. Launch with: wdroid   (or from the desktop entry / Start Menu)"
