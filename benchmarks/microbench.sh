#!/usr/bin/env bash
# Microbenchmark: whole-tree clonefile(2) vs per-file clonefile walk (`cp -c -R`).
#
# `replica new` clones the base with a single clonefile(2) call on the root
# (clonefile on a directory recursively clones the whole tree atomically in the
# kernel). `cp -c -R` of the same tree does *not* do that: cp recurses in
# userspace and clonefiles every file individually.
#
# This script times both on the same tree so the gap is measured, not asserted.
#
# Usage:
#   benchmarks/microbench.sh                      # fixture project, default runs
#   benchmarks/microbench.sh --project DIR --setup CMD --runs 30
#
# Writes benchmarks/results/<date>-<arch>-macos<version>-microbench.{md,json}.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="$HERE/fixtures/npm-app"
SETUP_CMD="npm ci --no-audit --no-fund"
RUNS=30
OUT_DIR="$HERE/results"
KEEP=0
ALLOW_BUSY=0

usage() {
  sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Options:
  --project DIR      project to benchmark (default: benchmarks/fixtures/npm-app)
  --setup CMD        dependency install command run in each clone (default: npm ci)
  --runs N           runs per method (default 30)
  --out DIR          results directory (default benchmarks/results)
  --keep             keep the scratch directory for inspection
  --allow-busy       run even if the 1-minute load average is high
  -h, --help         show this help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --project) PROJECT="$2"; shift 2 ;;
    --setup) SETUP_CMD="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    --keep) KEEP=1; shift ;;
    --allow-busy) ALLOW_BUSY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1 ($2)" >&2; exit 1; }; }
need hyperfine "brew install hyperfine"
need jq "brew install jq"
need replica "cargo build --release && PREFIX=\$HOME/.local make install"
need git "xcode-select --install"

[ "$(uname -s)" = "Darwin" ] || { echo "microbench needs macOS (clonefile)" >&2; exit 1; }

NCPU=$(sysctl -n hw.ncpu)
LOAD_START=$(sysctl -n vm.loadavg | awk '{print $2}')
if [ "$ALLOW_BUSY" -ne 1 ] && awk -v l="$LOAD_START" -v c="$NCPU" 'BEGIN { exit !(l > c / 4) }'; then
  echo "machine is busy: 1-minute load $LOAD_START on $NCPU cores. Close other work and retry, or pass --allow-busy." >&2
  exit 1
fi
[ -d "$PROJECT" ] || { echo "project not found: $PROJECT" >&2; exit 1; }
PROJECT="$(cd "$PROJECT" && pwd)"

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/replica-microbench.XXXXXX")"
touch "$SCRATCH/.metadata_never_index"
cleanup() {
  if [ "$KEEP" -eq 1 ]; then echo "kept scratch: $SCRATCH"; return; fi
  if [ -d "$SCRATCH/root/ws" ]; then
    for ws in "$SCRATCH"/root/ws/*; do
      [ -e "$ws" ] && replica rm "$(basename "$ws")" >/dev/null 2>&1 || true
    done
  fi
  rm -rf "$SCRATCH"
}
trap cleanup EXIT

export REPLICA_ROOT="$SCRATCH/root"
export npm_config_cache="$SCRATCH/npm-cache"
export NO_COLOR=1
mkdir -p "$REPLICA_ROOT" "$SCRATCH/work"

# The replica root, git clones, and workspaces must share one APFS volume.
[ "$(stat -f %d "$SCRATCH")" = "$(stat -f %d "$REPLICA_ROOT")" ] || { echo "scratch and root differ in volume" >&2; exit 1; }
diskutil info "$(df "$SCRATCH" | tail -1 | awk '{print $NF}')" 2>/dev/null | grep -q "APFS" \
  || { echo "scratch volume is not APFS; clonefile would fail" >&2; exit 1; }

FREE_KB=$(df -k "$SCRATCH" | tail -1 | awk '{print $4}')
if [ "$FREE_KB" -lt 2000000 ]; then
  echo "need at least 2 GB free for the benchmark; have $((FREE_KB / 1024)) MB" >&2; exit 1
fi

HELPERS="$HERE/helpers.sh"
W="$SCRATCH/work"
SRC="$SCRATCH/src"
RAW="$SCRATCH/raw"
mkdir -p "$RAW"

# ---- source repo ------------------------------------------------------------
copy_project() {
  (cd "$PROJECT" && tar --exclude=node_modules --exclude=.git -cf - .) | (mkdir -p "$SRC" && cd "$SRC" && tar -xf -)
}
copy_project
git -C "$SRC" init -q
git -C "$SRC" add -A
git -C "$SRC" -c user.name=replica-bench -c user.email=bench@localhost commit -qm "microbench source" >/dev/null

# ---- warm caches and build the base -----------------------------------------
echo "== warming npm cache"
(cd "$(mktemp -d "$W/warm.XXXXXX")" && cp -R "$SRC/." . && rm -rf .git && sh -c "$SETUP_CMD" >/dev/null 2>&1)

echo "== building replica base (one-time cost, not timed)"
replica base build --repo "$SRC" --setup "$SETUP_CMD" >"$RAW/base-build.log" 2>&1
BASE="$REPLICA_ROOT/base"
[ -d "$BASE" ] || { echo "base build failed; see $RAW/base-build.log" >&2; exit 1; }
FILE_COUNT=$(find "$BASE" -path "$BASE/.git" -prune -o -type f -print | wc -l | tr -d ' ')

# ---- timed microbenchmarks --------------------------------------------------

REPLICA_VERSION="$(replica --version)"

bench() {
  # bench NAME RUNS WARMUP PREPARE CMD
  local name="$1" runs="$2" warmup="$3" prepare="$4" cmd="$5" slug json
  slug=$(echo "$name" | tr -cs 'A-Za-z0-9' '-' | tr 'A-Z' 'a-z')
  json="$RAW/$slug.json"
  echo "== $name ($runs runs)"
  local args=(--runs "$runs" --warmup "$warmup" --style basic --shell=default --export-json "$json" --command-name "$name")
  [ -n "$prepare" ] && args+=(--prepare "$prepare")
  hyperfine "${args[@]}" "$cmd" >"$RAW/$slug.log" 2>&1 || { cat "$RAW/$slug.log" >&2; exit 1; }
  printf "%s\t%s\n" "$slug" "$name" >>"$RAW/order.txt"
}

# Whole-tree clone: a single clonefile(2) call on the root. This is what replica new does.
bench "replica new (whole-tree clonefile)" "$RUNS" 3 \
  "$HELPERS rm-recorded $W/mb-path path" \
  "replica new --ttl 1h >'$W/mb-path' 2>/dev/null"

# Per-file clonefile walk: cp -c -R recurses and clonefiles every file individually.
bench "cp -c -R of prebuilt base (per-file clonefile)" "$RUNS" 3 \
  "$HELPERS rm-tree $W/mb-cp" \
  "cp -c -R '$BASE' '$W/mb-cp'"

# Keep the workspaces replica new left behind from inflating later samples.
for ws in "$REPLICA_ROOT"/ws/*; do [ -e "$ws" ] && replica rm "$(basename "$ws")" >/dev/null 2>&1 || true; done

# ---- report -----------------------------------------------------------------

MACHINE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
LOAD_END=$(sysctl -n vm.loadavg | awk '{print $2}')
MACOS="$(sw_vers -productVersion)"
BUILD="$(sw_vers -buildVersion)"
ARCH="$(uname -m)"
DATE="$(date -u +%Y-%m-%d)"
SLUG="${DATE}-${ARCH}-macos${MACOS}-microbench"
mkdir -p "$OUT_DIR"
MD="$OUT_DIR/$SLUG.md"
JSON="$OUT_DIR/$SLUG.json"

MERGED="$RAW/merged.json"
echo "[]" >"$MERGED"
while IFS=$'\t' read -r slug name; do
  jq -c --slurpfile f "$RAW/$slug.json" --arg name "$name" \
    '($f[0].results[0].summary.time_wall_clock) as $w
     | ($f[0].results[0].measurements | map(.time_wall_clock.value) | sort) as $t
     | . + [{name: $name, mean: $w.mean, median: $w.median, min: $w.min, max: $w.max,
              stddev: $w.stddev, runs: ($t | length), p95: $t[(($t | length) * 0.95 | ceil) - 1]}]' "$MERGED" >"$MERGED.tmp"
  mv "$MERGED.tmp" "$MERGED"
done <"$RAW/order.txt"

jq -n --arg date "$DATE" --arg machine "$MACHINE" --arg macos "$MACOS" --arg build "$BUILD" \
  --arg arch "$ARCH" --arg project "$PROJECT" --arg setup "$SETUP_CMD" --arg replica "$REPLICA_VERSION" \
  --arg files "$FILE_COUNT" --arg base_secs "$(cat "$RAW/base-build.log" 2>/dev/null | grep -oE '[0-9.]+' | head -1 || echo 0)" \
  --slurpfile rows "$MERGED" \
  '{date: $date, machine: $machine, macos: $macos, build: $build, arch: $arch, project: $project,
    setup: $setup, replica: $replica, files: ($files | tonumber),
    results: $rows[0]}' >"$JSON"

row() {
  local name="$1"
  jq -r --arg n "$name" '
    .results[] | select(.name == $n) |
    "| \(.name) | \(.runs) | \(.mean * 1000 | round) | \(.median * 1000 | round) | \(.p95 * 1000 | round) | \(.min * 1000 | round)–\(.max * 1000 | round) |"' "$JSON"
}

{
  echo "## Microbench results: $DATE on $MACHINE, macOS $MACOS ($BUILD)"
  echo
  echo "Whole-tree clonefile(2) on the root (replica new) vs per-file clonefile walk (cp -c -R) on the same tree."
  echo
  echo "| Method | Runs | Mean (ms) | Median (ms) | P95 (ms) | Min–max (ms) |"
  echo "|---|---|---|---|---|---|"
  row "replica new (whole-tree clonefile)"
  row "cp -c -R of prebuilt base (per-file clonefile)"
  echo
  echo "- Project: \`$PROJECT\` ($FILE_COUNT files after setup)"
  echo "- Setup command: \`$SETUP_CMD\`"
  echo "- Replica: $REPLICA_VERSION"
  echo "- Machine: $MACHINE, macOS $MACOS ($BUILD), $ARCH, $NCPU cores"
  echo "- Load average (1 min): $LOAD_START at start, $LOAD_END at end"
  echo "- Scratch volume: APFS, same volume for source, base, and workspaces"
  echo "- Methodology: hyperfine, untimed prepare per run, P95 nearest-rank over the runs"
} >"$MD"

cat "$MD"
echo
echo "wrote $MD"
echo "wrote $JSON"
