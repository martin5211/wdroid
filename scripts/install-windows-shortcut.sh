#!/usr/bin/env bash
# Create a Windows desktop shortcut that launches wdroid without a terminal.
# WSL-only: uses wslg.exe (the windowless wsl.exe) and PowerShell.
set -euo pipefail

if ! grep -qi microsoft /proc/version 2>/dev/null; then
    echo "Not running under WSL — nothing to do." >&2
    exit 1
fi

DISTRO="${WSL_DISTRO_NAME:-Ubuntu}"

WSLG_EXE='C:\Program Files\WSL\wslg.exe'
if [ ! -e "/mnt/c/Program Files/WSL/wslg.exe" ]; then
    if [ -e "/mnt/c/Windows/System32/wslg.exe" ]; then
        WSLG_EXE='C:\Windows\System32\wslg.exe'
    else
        echo "wslg.exe not found — is WSLg installed?" >&2
        exit 1
    fi
fi

# Icon: wrap the Waydroid PNG into an .ico (Windows accepts PNG payloads).
# Stored in %LOCALAPPDATA% — outside OneDrive so sync can't break it.
LOCALAPPDATA_WIN=$(powershell.exe -NoProfile -Command 'Write-Output $env:LOCALAPPDATA' | tr -d '\r')
LOCALAPPDATA_WSL=$(wslpath "$LOCALAPPDATA_WIN")
ICON_ARG=""
PNG=/usr/share/icons/hicolor/512x512/apps/waydroid.png
if [ -f "$PNG" ]; then
    mkdir -p "$LOCALAPPDATA_WSL/wdroid"
    python3 - "$PNG" "$LOCALAPPDATA_WSL/wdroid/wdroid.ico" <<'EOF'
import struct, sys
png = open(sys.argv[1], 'rb').read()
ico = struct.pack('<HHH', 0, 1, 1) + struct.pack('<BBBBHHII', 0, 0, 0, 0, 1, 32, len(png), 22) + png
open(sys.argv[2], 'wb').write(ico)
EOF
    ICON_ARG="\$l.IconLocation=\"$LOCALAPPDATA_WIN\\wdroid\\wdroid.ico,0\""
fi

powershell.exe -NoProfile -Command - <<EOF
\$d=[Environment]::GetFolderPath("Desktop")
\$ws=New-Object -ComObject WScript.Shell
\$l=\$ws.CreateShortcut("\$d\\wdroid.lnk")
\$l.TargetPath="$WSLG_EXE"
\$l.Arguments='-d $DISTRO -- sh -lc "~/.local/bin/wdroid >/tmp/wdroid.log 2>&1"'
\$l.Description="Waydroid in a window"
$ICON_ARG
\$l.Save()
Write-Output "Shortcut created: \$d\\wdroid.lnk"
EOF
