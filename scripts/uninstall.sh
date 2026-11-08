#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-/usr/local}"
PLIST="$HOME/Library/LaunchAgents/dev.replica.gc.plist"
[[ -f "$PLIST" ]] && { launchctl unload "$PLIST" 2>/dev/null || true; rm -f "$PLIST"; echo "removed launchd agent"; }
rm -f "$PREFIX/bin/replica" && echo "removed $PREFIX/bin/replica"
echo "Your workspaces under REPLICA_ROOT were NOT deleted."
