#!/bin/sh
# Install replica from a GitHub release, after checking its SHA-256.
#
#   curl -fsSL https://raw.githubusercontent.com/gabrielvfonseca/replica/main/scripts/get-replica.sh | sh
#   sh get-replica.sh --version 0.1.0
#   sh get-replica.sh --uninstall
#
# Environment:
#   PREFIX               install prefix (default: $HOME/.local); binary goes to $PREFIX/bin/replica
#   REPLICA_RELEASE_URL  base URL that holds the release assets (for testing or mirrors)
set -eu

REPO="gabrielvfonseca/replica"
DEFAULT_BASE="https://github.com/$REPO/releases/latest/download"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
BIN="$BIN_DIR/replica"
VERSION=""
UNINSTALL=0

die() {
  printf 'get-replica: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '%s\n' "$*"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || die "--version needs a value, e.g. --version 0.1.0"
      VERSION="$2"
      shift 2
      ;;
    --uninstall)
      UNINSTALL=1
      shift
      ;;
    -h|--help)
      sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

if [ "$UNINSTALL" -eq 1 ]; then
  if [ -e "$BIN" ]; then
    rm -f "$BIN"
    info "removed $BIN"
  else
    info "nothing to remove: $BIN does not exist"
  fi
  info "Your workspaces under REPLICA_ROOT were NOT deleted."
  exit 0
fi

[ "$(uname -s)" = "Darwin" ] || die "replica runs on macOS only (APFS clonefile); this is $(uname -s)"

case "$(uname -m)" in
  arm64) ARCH_ASSET="replica-darwin-arm64" ;;
  x86_64) ARCH_ASSET="replica-darwin-x86_64" ;;
  *) die "unsupported architecture: $(uname -m)" ;;
esac

if [ -n "$VERSION" ]; then
  case "$VERSION" in
    *[!0-9.]*|"") die "version must look like 0.1.0 (got: $VERSION)" ;;
  esac
  BASE="${REPLICA_RELEASE_URL:-https://github.com/$REPO/releases/download/v$VERSION}"
else
  BASE="${REPLICA_RELEASE_URL:-$DEFAULT_BASE}"
fi
BASE="${BASE%/}"

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v shasum >/dev/null 2>&1 || die "shasum is required (ships with macOS)"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/get-replica.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT INT TERM

info "downloading $ARCH_ASSET from $BASE"
curl -fsSL "$BASE/$ARCH_ASSET" -o "$TMP/$ARCH_ASSET" || die "download failed: $BASE/$ARCH_ASSET"
curl -fsSL "$BASE/sha256sums.txt" -o "$TMP/sha256sums.txt" || die "download failed: $BASE/sha256sums.txt"

EXPECTED="$(awk -v f="$ARCH_ASSET" '$2 == f || $2 == "*" f { print $1 }' "$TMP/sha256sums.txt")"
[ -n "$EXPECTED" ] || die "no checksum for $ARCH_ASSET in sha256sums.txt"
ACTUAL="$(shasum -a 256 "$TMP/$ARCH_ASSET" | awk '{ print $1 }')"
if [ "$EXPECTED" != "$ACTUAL" ]; then
  die "checksum mismatch for $ARCH_ASSET (expected $EXPECTED, got $ACTUAL); nothing was installed"
fi
info "checksum ok"

mkdir -p "$BIN_DIR"
install -m 0755 "$TMP/$ARCH_ASSET" "$BIN"
info "installed $BIN"
"$BIN" --version

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    info ""
    info "note: $BIN_DIR is not on your PATH. Add this to your shell profile:"
    info "  export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac

info ""
info "Next: export REPLICA_ROOT to a directory on an APFS volume, then run: replica help quickstart"
