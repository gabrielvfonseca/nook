#!/usr/bin/env bash
# Render the Homebrew formula for a release into the local tap.
#
# Two phases, because the sha256 must match the tarball GitHub serves for the tag:
#
#   scripts/release.sh 0.1.0
#       Pre-tag dry run. Checks the tree is clean and the tag is free, then prints
#       the plan and a PREVIEW sha256 from `git archive`. Writes nothing.
#
#   scripts/release.sh 0.1.0 --apply
#       Post-tag. Requires the tag to exist on origin. Downloads the GitHub tag
#       tarball, hashes it, and writes Formula/replica.rb into the tap.
#
# This script never creates or pushes git tags and never creates GitHub releases.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAP_DIR="${TAP_DIR:-$REPO_ROOT/../homebrew-tap}"
GITHUB_REPO="gabrielvfonseca/replica"

VERSION="${1:-}"
APPLY=0
if [[ -z "$VERSION" ]]; then
  echo "usage: scripts/release.sh <version> [--apply]   e.g. 0.1.0" >&2
  exit 2
fi
shift
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version must look like X.Y.Z (got: $VERSION)" >&2
  exit 2
fi
TAG="v$VERSION"
URL="https://github.com/$GITHUB_REPO/archive/refs/tags/$TAG.tar.gz"
FORMULA_OUT="$TAP_DIR/Formula/replica.rb"

cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean; commit or stash first" >&2
  git status --short >&2
  exit 1
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/replica-release.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ "$APPLY" -ne 1 ]]; then
  # Pre-tag: the tag must not exist yet.
  if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null ||
     git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
    echo "error: tag $TAG already exists (locally or on origin); pick a new version" >&2
    exit 1
  fi
  git archive --format=tar.gz --prefix="replica-$VERSION/" -o "$TMP_DIR/preview.tar.gz" HEAD
  PREVIEW_SHA="$(shasum -a 256 "$TMP_DIR/preview.tar.gz" | awk '{print $1}')"
  echo "release plan for $TAG (dry run)"
  echo "  commit:          $(git rev-parse HEAD)"
  echo "  url (after tag): $URL"
  echo "  preview sha256:  $PREVIEW_SHA  (git archive; NOT the value to use)"
  echo "  formula target:  $FORMULA_OUT"
  echo
  echo "Nothing written. Publish first, then run: scripts/release.sh $VERSION --apply"
  echo "  git tag -a $TAG -m \"replica $VERSION\""
  echo "  git push origin $TAG"
  echo "  gh release create $TAG --title \"replica $VERSION\" --notes-file CHANGELOG.md"
  exit 0
fi

# Post-tag: the tag must exist on origin so the URL resolves.
if ! git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
  echo "error: tag $TAG is not on origin yet. Create and push it first (see dry run)." >&2
  exit 1
fi
if [[ ! -d "$TAP_DIR/Formula" ]]; then
  echo "error: tap not found at $TAP_DIR (set TAP_DIR to override)" >&2
  exit 1
fi

curl --fail --silent --show-error --location "$URL" -o "$TMP_DIR/release.tar.gz"
SHA256="$(shasum -a 256 "$TMP_DIR/release.tar.gz" | awk '{print $1}')"

sed -E \
  -e "s|^  url \".*\"|  url \"$URL\"|" \
  -e "s|^  sha256 \".*\"|  sha256 \"$SHA256\"|" \
  "$FORMULA_OUT" > "$TMP_DIR/replica.rb"
mv "$TMP_DIR/replica.rb" "$FORMULA_OUT"

echo "wrote $FORMULA_OUT"
echo "  url:    $URL"
echo "  sha256: $SHA256"
cat <<EOF

Next (run yourself):
  cd "$TAP_DIR"
  brew style Formula/replica.rb
  git add Formula/replica.rb && git commit -m "replica $VERSION"
  git remote add origin git@github.com:gabrielvfonseca/homebrew-tap.git   # first time only
  git push -u origin main
EOF
