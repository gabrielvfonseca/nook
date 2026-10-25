#!/usr/bin/env bash
# Install the release binary and (optionally) the launchd GC agent.
set -euo pipefail
cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-/usr/local}"
BIN="target/release/nook"
[[ -x "$BIN" ]] || { echo "build first: cargo build --release" >&2; exit 1; }
install -d "$PREFIX/bin"
install -m 0755 "$BIN" "$PREFIX/bin/nook"
echo "installed $PREFIX/bin/nook"

if [[ "${INSTALL_LAUNCHD:-0}" == "1" ]]; then
  ROOT="${NOOK_ROOT:-/Volumes/Work/nook}"
  PLIST="$HOME/Library/LaunchAgents/dev.nook.gc.plist"
  sed -e "s#__NOOK_BIN__#$PREFIX/bin/nook#g" -e "s#__NOOK_ROOT__#$ROOT#g" \
      packaging/dev.nook.gc.plist > "$PLIST"
  launchctl unload "$PLIST" 2>/dev/null || true
  launchctl load "$PLIST"
  echo "loaded launchd agent: $PLIST (gc every 10 minutes)"
fi
