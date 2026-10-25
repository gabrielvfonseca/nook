#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-/usr/local}"
PLIST="$HOME/Library/LaunchAgents/dev.nook.gc.plist"
[[ -f "$PLIST" ]] && { launchctl unload "$PLIST" 2>/dev/null || true; rm -f "$PLIST"; echo "removed launchd agent"; }
rm -f "$PREFIX/bin/nook" && echo "removed $PREFIX/bin/nook"
echo "Your workspaces under NOOK_ROOT were NOT deleted."
