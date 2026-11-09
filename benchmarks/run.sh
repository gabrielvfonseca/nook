#!/usr/bin/env bash
# Replica benchmark suite.
#
# Compares creating a fresh, fully installed project copy four ways:
#   git clone + setup, git worktree + setup, cp -c (clonefile) of a prebuilt tree, replica new.
# and removing one: replica rm vs rm -rf of a clonefile copy.
#
# Usage:
#   benchmarks/run.sh                          # fixture project, default runs
#   benchmarks/run.sh --project DIR            # your project (must have a setup command)
#   benchmarks/run.sh --setup "pnpm install --frozen-lockfile" --runs 30
#
# Writes benchmarks/results/<date>-<arch>-macos<version>.{md,json}. See benchmarks/README.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="$HERE/fixtures/npm-app"
SETUP_CMD="npm ci --no-audit --no-fund"
RUNS=20           # cheap operations (cp, replica new, replica rm, rm -rf)
SLOW_RUNS=10      # operations that install dependencies (git clone + setup, worktree + setup)
DISK_RUNS=20      # disk-per-clone sampling (cheap operations)
OUT_DIR="$HERE/results"
KEEP=0
ALLOW_BUSY=0

usage() {
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Options:
  --project DIR      project to benchmark (default: benchmarks/fixtures/npm-app)
  --setup CMD        dependency install command run in each clone (default: npm ci)
  --runs N           runs for cheap operations (default 20; p95 needs >= 20)
  --slow-runs N      runs for install operations (default 10)
  --out DIR          results directory (default benchmarks/results)
  --keep             keep the scratch directory for inspection
  --allow-busy       run even if the 1-minute load average is high (results will be noisy)
  -h, --help         show this help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --project) PROJECT="$2"; shift 2 ;;
    --setup) SETUP_CMD="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --slow-runs) SLOW_RUNS="$2"; shift 2 ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    --keep) KEEP=1; shift ;;
    --allow-busy) ALLOW_BUSY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

# ---- preflight -------------------------------------------------------------

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1 ($2)" >&2; exit 1; }; }
need hyperfine "brew install hyperfine"
need jq "brew install jq"
need replica "cargo build --release && PREFIX=\$HOME/.local make install"
need git "xcode-select --install"

[ "$(uname -s)" = "Darwin" ] || { echo "benchmarks need macOS (clonefile)" >&2; exit 1; }

# Timings are only meaningful on an idle machine. Refuse when the 1-minute load is above a quarter of the cores.
NCPU=$(sysctl -n hw.ncpu)
LOAD_START=$(sysctl -n vm.loadavg | awk '{print $2}')
if [ "$ALLOW_BUSY" -ne 1 ] && awk -v l="$LOAD_START" -v c="$NCPU" 'BEGIN { exit !(l > c / 4) }'; then
  echo "machine is busy: 1-minute load $LOAD_START on $NCPU cores. Close other work and retry, or pass --allow-busy." >&2
  echo "top CPU consumers:" >&2
  ps -Ao pcpu,comm -r | sed -n '2,6p' >&2
  exit 1
fi
[ -d "$PROJECT" ] || { echo "project not found: $PROJECT" >&2; exit 1; }
PROJECT="$(cd "$PROJECT" && pwd)"

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/replica-bench.XXXXXX")"
# Keep Spotlight from indexing the thousands of files we create; its writes would skew disk sampling.
touch "$SCRATCH/.metadata_never_index"
cleanup() {
  if [ "$KEEP" -eq 1 ]; then
    echo "kept scratch: $SCRATCH"
    return
  fi
  # Remove workspaces the run created, then the scratch tree. Both live under $SCRATCH.
  if [ -d "$SCRATCH/root/ws" ]; then
    for ws in "$SCRATCH"/root/ws/*; do
      [ -e "$ws" ] && replica rm "$(basename "$ws")" >/dev/null 2>&1 || true
    done
  fi
  rm -rf "$SCRATCH"
}
trap cleanup EXIT

export REPLICA_ROOT="$SCRATCH/root"
export npm_config_cache="$SCRATCH/npm-cache"   # one npm cache for every method, warmed below
export NO_COLOR=1
mkdir -p "$REPLICA_ROOT" "$SCRATCH/work"

# The replica root, git clones, and workspaces must share one APFS volume.
[ "$(stat -f %d "$SCRATCH")" = "$(stat -f %d "$REPLICA_ROOT")" ] || { echo "scratch and root differ in volume" >&2; exit 1; }
diskutil info "$(df "$SCRATCH" | tail -1 | awk '{print $NF}')" 2>/dev/null | grep -q "APFS" \
  || { echo "scratch volume is not APFS; clonefile would fail" >&2; exit 1; }

FREE_KB=$(df -k "$SCRATCH" | tail -1 | awk '{print $4}')
if [ "$FREE_KB" -lt 2000000 ]; then
  echo "need at least 2 GB free for the benchmark; have $((FREE_KB / 1024)) MB" >&2
  exit 1
fi

HELPERS="$HERE/helpers.sh"
W="$SCRATCH/work"
SRC="$SCRATCH/src"
RAW="$SCRATCH/raw"
mkdir -p "$RAW"

# ---- source repo ------------------------------------------------------------

copy_project() {
  # Copy the project without any previous install output.
  (cd "$PROJECT" && tar --exclude=node_modules --exclude=.git -cf - .) | (mkdir -p "$SRC" && cd "$SRC" && tar -xf -)
}
copy_project
git -C "$SRC" init -q
git -C "$SRC" add -A
git -C "$SRC" -c user.name=replica-bench -c user.email=bench@localhost commit -qm "benchmark source" >/dev/null

PKG_COUNT=$(grep -c '"resolved"' "$SRC/package-lock.json" 2>/dev/null || true)
PKG_COUNT=${PKG_COUNT:-0}

# ---- warm caches and build the base -----------------------------------------

echo "== warming npm cache"
(cd "$(mktemp -d "$W/warm.XXXXXX")" && cp -R "$SRC/." . && rm -rf .git && sh -c "$SETUP_CMD" >/dev/null 2>&1)

echo "== building replica base (informational: a one-time cost)"
BASE_START=$(python3 -c 'import time; print(time.time())')
replica base build --repo "$SRC" --setup "$SETUP_CMD" >"$RAW/base-build.log" 2>&1
BASE_END=$(python3 -c 'import time; print(time.time())')
BASE_SECS=$(python3 -c "print(f'{$BASE_END - $BASE_START:.2f}')")
BASE="$REPLICA_ROOT/base"
[ -d "$BASE" ] || { echo "base build failed; see $RAW/base-build.log" >&2; exit 1; }
# Installed files (after setup), not the source fixture.
FILE_COUNT=$(find "$BASE" -path "$BASE/.git" -prune -o -type f -print | wc -l | tr -d ' ')

# ---- timed benchmarks -------------------------------------------------------

REPLICA_VERSION="$(replica --version)"
REPLICA_PATH="$(command -v replica)"
BASE_LOGICAL_KB=$(du -sk "$BASE" | awk '{print $1}')

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

bench "shell baseline (true)" "$RUNS" 3 "" "true"

bench "git clone + setup" "$SLOW_RUNS" 1 \
  "$HELPERS rm-tree $W/git-clone" \
  "git clone -q '$SRC' '$W/git-clone' && cd '$W/git-clone' && $SETUP_CMD"

bench "git worktree + setup" "$SLOW_RUNS" 1 \
  "$HELPERS git-clean '$SRC' '$W/wt'" \
  "git -C '$SRC' worktree add -q --detach '$W/wt' HEAD && cd '$W/wt' && $SETUP_CMD"

bench "cp -c -R of prebuilt base" "$RUNS" 3 \
  "$HELPERS rm-tree $W/cp-clone" \
  "cp -c -R '$BASE' '$W/cp-clone'"

bench "replica new" "$RUNS" 3 \
  "$HELPERS rm-recorded $W/last-path path" \
  "replica new --ttl 1h >'$W/last-path' 2>/dev/null"

bench "replica rm" "$RUNS" 3 \
  "$HELPERS new-id $W/rm-id" \
  "replica rm \"\$(cat '$W/rm-id')\""

bench "rm -rf of a cp -c clone" "$RUNS" 3 \
  "cp -c -R '$BASE' '$W/rm-target'" \
  "rm -rf '$W/rm-target'"

# Keeping the base fresh. Nothing changed upstream: this is the common case.
bench "replica base refresh (no changes)" "$SLOW_RUNS" 1 "" \
  "replica base refresh >/dev/null 2>&1"

# ---- disk per clone (df delta over many clones) -----------------------------

used_kb() { df -k "$REPLICA_ROOT" | tail -1 | awk '{print $3}'; }

disk_per() {
  # disk_per N CMD_PER_ITERATION CLEANUP_PER_ITERATION -> prints MB per clone
  local n="$1" make="$2" clean="$3" before after i
  sync; sleep 1
  before=$(used_kb)
  for i in $(seq 1 "$n"); do eval "$make" >/dev/null 2>&1; done
  sync; sleep 1
  after=$(used_kb)
  for i in $(seq 1 "$n"); do eval "$clean" >/dev/null 2>&1 || true; done
  python3 -c "print(f'{max(0, $after - $before) / 1024 / $n:.2f}')"
}

echo "== disk per clone"
DISK_CP=$(disk_per "$DISK_RUNS" "cp -c -R '$BASE' '$W/disk-cp-\$i'" "rm -rf '$W/disk-cp-\$i'")
DISK_REPLICA=$(disk_per "$DISK_RUNS" "replica new --ttl 1h >/dev/null 2>&1" "true")
# Clean up the workspaces the replica sample left behind.
for ws in "$REPLICA_ROOT"/ws/*; do [ -e "$ws" ] && replica rm "$(basename "$ws")" >/dev/null 2>&1 || true; done
# A git clone has no block sharing, so du (allocated size) is exact for it. One sample is enough.
GIT_CHECK="$W/disk-git"
rm -rf "$GIT_CHECK"
git clone -q "$SRC" "$GIT_CHECK" && (cd "$GIT_CHECK" && eval "$SETUP_CMD" >/dev/null 2>&1)
DISK_GIT=$(python3 -c "print(f'{$(du -sk "$GIT_CHECK" | awk '{print $1}') / 1024:.2f}')")
rm -rf "$GIT_CHECK"

# ---- report -----------------------------------------------------------------

MACHINE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
LOAD_END=$(sysctl -n vm.loadavg | awk '{print $2}')
MACOS="$(sw_vers -productVersion)"
BUILD="$(sw_vers -buildVersion)"
ARCH="$(uname -m)"
DATE="$(date -u +%Y-%m-%d)"
SLUG="${DATE}-${ARCH}-macos${MACOS}"
mkdir -p "$OUT_DIR"
MD="$OUT_DIR/$SLUG.md"
JSON="$OUT_DIR/$SLUG.json"

# Merge hyperfine results with computed p95 (nearest rank) into one JSON document.
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
  --arg files "$FILE_COUNT" --arg pkgs "$PKG_COUNT" --arg base_secs "$BASE_SECS" \
  --arg base_kb "$BASE_LOGICAL_KB" --arg dcp "$DISK_CP" --arg drep "$DISK_REPLICA" --arg dgit "$DISK_GIT" \
  --slurpfile rows "$MERGED" \
  '{date: $date, machine: $machine, macos: $macos, build: $build, arch: $arch, project: $project,
    setup: $setup, replica: $replica, files: ($files | tonumber), packages: $pkgs,
    base_build_secs: ($base_secs | tonumber), base_logical_kb: ($base_kb | tonumber),
    disk_mb_per_clone: {git_clone_setup: ($dgit | tonumber), cp_clonefile: ($dcp | tonumber), replica_new: ($drep | tonumber)},
    results: $rows[0]}' >"$JSON"

fmt() { python3 -c "print(f'{float(\"$1\"):.3f}')"; }
row() {
  # row NAME DISK_MB  (looks up the result by command name)
  local name="$1" disk="$2"
  jq -r --arg n "$name" --arg d "$disk" '
    .results[] | select(.name == $n) |
    "| \(.name) | \(.runs) | \(.mean * 1000 | round) | \(.median * 1000 | round) | \(.p95 * 1000 | round) | \(.min * 1000 | round)–\(.max * 1000 | round) | \($d) |"' "$JSON"
}

{
  echo "## Results: $DATE on $MACHINE, macOS $MACOS ($BUILD)"
  echo
  echo "| Method | Runs | Mean (ms) | Median (ms) | P95 (ms) | Min–max (ms) | Extra disk per clone |"
  echo "|---|---|---|---|---|---|---|"
  row "shell baseline (true)" "–"
  row "git clone + setup" "$(fmt "$DISK_GIT") MB (du)"
  row "git worktree + setup" "n/a (shares the object store)"
  row "cp -c -R of prebuilt base" "$(fmt "$DISK_CP") MB"
  row "replica new" "$(fmt "$DISK_REPLICA") MB"
  row "replica rm" "–"
  row "rm -rf of a cp -c clone" "–"
  row "replica base refresh (no changes)" "–"
  echo
  echo "- Project: \`$PROJECT\` ($PKG_COUNT locked packages, $FILE_COUNT files after setup)"
  echo "- Setup command: \`$SETUP_CMD\`"
  echo "- Base build (one time, not in the table): ${BASE_SECS}s"
  echo "- Replica: $REPLICA_VERSION at \`$REPLICA_PATH\`"
  echo "- Machine: $MACHINE, macOS $MACOS ($BUILD), $ARCH, $NCPU cores"
  echo "- Load average (1 min): $LOAD_START at start, $LOAD_END at end"
  echo "- Scratch volume: APFS, same volume for source, base, and workspaces"
  echo "- Methodology: hyperfine, untimed prepare and cleanup per run, npm cache warmed,"
  echo "  git clones from a local path (no network), P95 nearest-rank over the runs"
  echo "- Disk per clone: for clonefile methods, the change in volume used over $DISK_RUNS clones (df),"
  echo "  so it counts the APFS metadata each clone adds. Data blocks are shared and not counted."
  echo "  The git clone figure is du of one clone, because git copies data and nothing is shared."
  echo "  Other writes on the volume during sampling can add noise, so expect roughly +/- 0.5 MB."
} >"$MD"

cat "$MD"
echo
echo "wrote $MD"
echo "wrote $JSON"
