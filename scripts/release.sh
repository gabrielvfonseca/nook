#!/usr/bin/env bash
# Cut a release: build the binaries, tag, and publish a GitHub release. (Homebrew is not included; see HOMEBREW.md.)
#
#   scripts/release.sh 0.1.0           dry run: checks everything, builds bin/, writes nothing else
#   scripts/release.sh 0.1.0 --apply   tag vX.Y.Z, push it, create the GitHub release
#
# The checksums in bin/sha256sums.txt are computed from the exact files uploaded to the release.
set -euo pipefail

usage() {
  echo "usage: scripts/release.sh X.Y.Z [--apply]" >&2
  exit 2
}

VERSION="${1:-}"
[[ -n "$VERSION" ]] || usage
shift
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    *) usage ;;
  esac
done
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like X.Y.Z (got: $VERSION)" >&2; exit 2; }

TAG="v$VERSION"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NOTES="$(mktemp "${TMPDIR:-/tmp}/replica-notes.XXXXXX")"
trap 'rm -f "$NOTES"' EXIT

cd "$REPO_ROOT"

fail() {
  echo "release: $*" >&2
  exit 1
}

# ---- checks -----------------------------------------------------------------

[[ -z "$(git status --porcelain)" ]] || { git status --short >&2; fail "working tree is not clean; commit or stash first"; }

CARGO_VERSION="$(awk -F'"' '/^version = /{print $2; exit}' Cargo.toml)"
[[ "$CARGO_VERSION" == "$VERSION" ]] || fail "Cargo.toml says $CARGO_VERSION, not $VERSION"

awk -v v="$VERSION" '$0 ~ "^## "v"( |$)" {found=1} END {exit !found}' CHANGELOG.md \
  || fail "CHANGELOG.md has no '## $VERSION' section"

git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && fail "tag $TAG already exists locally"
if git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
  fail "tag $TAG already exists on origin"
fi

# Release notes: the CHANGELOG section for this version.
awk -v v="$VERSION" '
  $0 ~ "^## "v"( |$)" {on=1; next}
  on && /^## / {exit}
  on {print}
' CHANGELOG.md > "$NOTES"

# ---- build ------------------------------------------------------------------

echo "== make check"
make check
echo "== make dist"
make dist

# ---- plan -------------------------------------------------------------------

echo
echo "release plan for $TAG"
echo "  commit:     $(git rev-parse --short HEAD)"
echo "  artifacts:"
sed 's/^/    /' bin/sha256sums.txt
echo "  notes:      $(wc -l < "$NOTES" | tr -d ' ') lines from CHANGELOG.md"

if [[ "$APPLY" -ne 1 ]]; then
  echo
  echo "dry run: nothing published. bin/ was rebuilt. To release, run:"
  echo "  scripts/release.sh $VERSION --apply"
  exit 0
fi

# ---- apply ------------------------------------------------------------------

gh auth status >/dev/null 2>&1 || fail "gh is not logged in"

echo "== tagging and pushing $TAG"
git tag -a "$TAG" -m "replica $VERSION"
git push origin "$TAG"

echo "== creating GitHub release $TAG"
gh release create "$TAG" \
  --title "replica $VERSION" \
  --notes-file "$NOTES" \
  bin/replica-darwin-arm64 bin/replica-darwin-x86_64 bin/replica-darwin-universal bin/sha256sums.txt
