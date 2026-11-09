#!/usr/bin/env bash
# Install the release binary and (optionally) the launchd GC agent.
set -euo pipefail
cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-/usr/local}"
BIN="target/release/replica"
[[ -x "$BIN" ]] || { echo "build first: cargo build --release" >&2; exit 1; }
install -d "$PREFIX/bin"
install -m 0755 "$BIN" "$PREFIX/bin/replica"
echo "installed $PREFIX/bin/replica"

if [[ "${INSTALL_LAUNCHD:-0}" == "1" ]]; then
  ROOT="${REPLICA_ROOT:-/Volumes/Work/replica}"
  PLIST="$HOME/Library/LaunchAgents/dev.replica.gc.plist"
  sed -e "s#__REPLICA_BIN__#$PREFIX/bin/replica#g" -e "s#__REPLICA_ROOT__#$ROOT#g" \
      packaging/dev.replica.gc.plist > "$PLIST"
  launchctl unload "$PLIST" 2>/dev/null || true
  launchctl load "$PLIST"
  echo "loaded launchd agent: $PLIST (gc every 10 minutes)"
fi
