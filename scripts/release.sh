#!/usr/bin/env bash
# Cut a release: build the binaries, tag, publish a GitHub release, and update the Homebrew tap.
#
#   scripts/release.sh 0.1.0           dry run: checks everything, builds bin/, writes nothing else
#   scripts/release.sh 0.1.0 --apply   tag vX.Y.Z, push it, create the GitHub release, update the tap
#
# The formula's sha256 values come from the local bin/ artifacts, which are the exact files
# uploaded to the release. Nothing is taken from GitHub's generated source tarball.
#
# Environment:
#   TAP_DIR   path to the Homebrew tap checkout (default: ../homebrew-tap next to the main repo)
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
MAIN_REPO="$(dirname "$(git -C "$REPO_ROOT" rev-parse --path-format=absolute --git-common-dir)")"
TAP_DIR="${TAP_DIR:-$MAIN_REPO/../homebrew-tap}"
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
echo "  tap:        $TAP_DIR/Formula/replica.rb"

if [[ "$APPLY" -ne 1 ]]; then
  echo
  echo "dry run: nothing published. bin/ was rebuilt. To release, run:"
  echo "  scripts/release.sh $VERSION --apply"
  exit 0
fi

# ---- apply ------------------------------------------------------------------

[[ -d "$TAP_DIR/Formula" ]] || fail "tap not found at $TAP_DIR (set TAP_DIR)"
gh auth status >/dev/null 2>&1 || fail "gh is not logged in"

echo "== tagging and pushing $TAG"
git tag -a "$TAG" -m "replica $VERSION"
git push origin "$TAG"

echo "== creating GitHub release $TAG"
gh release create "$TAG" \
  --title "replica $VERSION" \
  --notes-file "$NOTES" \
  bin/replica-darwin-arm64 bin/replica-darwin-x86_64 bin/replica-darwin-universal bin/sha256sums.txt

echo "== updating tap formula"
SHA_ARM="$(awk '$2 == "replica-darwin-arm64" {print $1}' bin/sha256sums.txt)"
SHA_INTEL="$(awk '$2 == "replica-darwin-x86_64" {print $1}' bin/sha256sums.txt)"
[[ -n "$SHA_ARM" && -n "$SHA_INTEL" ]] || fail "could not read checksums from bin/sha256sums.txt"
python3 - "$TAP_DIR/Formula/replica.rb" "$VERSION" "$SHA_ARM" "$SHA_INTEL" <<'PY'
import re, sys
path, version, sha_arm, sha_intel = sys.argv[1:]
text = open(path).read()
text, n = re.subn(r'version "[^"]*"', f'version "{version}"', text, count=1)
assert n == 1, "formula has no version line"
# The on_macos block lists the arm64 checksum first, then x86_64.
shas = list(re.finditer(r'sha256 "[^"]*"', text))
assert len(shas) == 2, f"expected two sha256 lines, found {len(shas)}"
text = (text[:shas[0].start()] + f'sha256 "{sha_arm}"'
        + text[shas[0].end():shas[1].start()] + f'sha256 "{sha_intel}"'
        + text[shas[1].end():])
open(path, "w").write(text)
PY
ruby -c "$TAP_DIR/Formula/replica.rb" >/dev/null
echo "wrote $TAP_DIR/Formula/replica.rb"
cat <<EOF

Next, in the tap (not done by this script):
  cd "$TAP_DIR"
  brew style Formula/replica.rb
  git add Formula/replica.rb && git commit -m "replica $VERSION"
  git push origin main
EOF
