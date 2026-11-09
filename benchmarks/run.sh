#!/usr/bin/env bash
# Replica benchmark suite (extended for Phase 2).
#
# Compares creating fresh, fully installed project copies across:
#   - project sizes: small (~2k), medium (~30k), large (~100k+ files)
#   - concurrency: N = 1, 5, 10 workspaces created at the same time
#   - methods: git clone + setup, git worktree + setup,
#              pnpm with the global virtual store, cp -c -R, replica new
#
# Records per run: median and P95 wall time, total wall time for all N,
# disk used before and after (df on the volume + replica stats), and peak
# memory pressure (context only; Replica makes no RAM claim).
#
# Reports the one-time cost of `replica base build` separately and the
# break-even point (after how many workspaces Replica comes out ahead vs
# git clone + setup).
#
# Usage:
#   benchmarks/run.sh                                  # default fixture, N=1
#   benchmarks/run.sh --concurrency 1,5,10             # parallel creation
#   benchmarks/run.sh --project "$PWD/repo|npm ci" --concurrency 1,5,10
#
# Writes benchmarks/results/<date>-<arch>-macos<version>.{md,json}.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_PROJECT="$HERE/fixtures/npm-app"
DEFAULT_SETUP="npm ci --no-audit --no-fund"
RUNS=20           # measurement batches for cheap operations
SLOW_RUNS=10      # measurement batches for install operations
WARMUP=3          # untimed warmup batches
CONCURRENCY="1"   # comma-separated concurrency levels
OUT_DIR="$HERE/results"
KEEP=0
ALLOW_BUSY=0
METHODS=""        # empty = all methods
PROJECT_SPECS=""  # empty = default fixture

usage() {
  sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Options:
  --project DIR:SETUP   project to benchmark (repeatable). "DIR" is copied
                        (without node_modules/.git/target) into a fresh git
                        repo and "SETUP" is the install command run inside
                        each clone. If SETUP is omitted, --setup is used.
                        Default: benchmarks/fixtures/npm-app.
  --setup CMD           default install command (default: npm ci)
  --concurrency N       comma-separated concurrency levels (default: 1;
                        e.g. 1,5,10 creates N workspaces at the same time)
  --runs N              measurement batches per method (default 20 cheap /
                        10 for operations that install dependencies)
  --methods LIST        comma-separated method subset:
                        git-clone,git-worktree,pnpm-store,cp-clonefile,replica
  --out DIR             results directory (default benchmarks/results)
  --keep                keep the scratch directory for inspection
  --allow-busy          run even if the 1-minute load average is high
  -h, --help            show this help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --project) PROJECT_SPECS="$PROJECT_SPECS $2"; shift 2 ;;
    --setup) DEFAULT_SETUP="$2"; shift 2 ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --runs) RUNS="$2"; SLOW_RUNS="$2"; shift 2 ;;
    --methods) METHODS="$2"; shift 2 ;;
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
need python3 "python3 is required for statistics and sampling"
need bc "bc is required for arithmetic"

[ "$(uname -s)" = "Darwin" ] || { echo "benchmarks need macOS (clonefile)" >&2; exit 1; }

# Refuse to run on a busy machine (timings only meaningful when idle).
NCPU=$(sysctl -n hw.ncpu)
LOAD_START=$(sysctl -n vm.loadavg | awk '{print $2}')
if [ "$ALLOW_BUSY" -ne 1 ] && awk -v l="$LOAD_START" -v c="$NCPU" 'BEGIN { exit !(l > c / 4) }'; then
  echo "machine is busy: 1-minute load $LOAD_START on $NCPU cores. Close other work and retry, or pass --allow-busy." >&2
  echo "top CPU consumers:" >&2
  ps -Ao pcpu,comm -r | sed -n '2,6p' >&2
  exit 1
fi

# Resolve project specs "DIR:SETUP" or "DIR" (-> --setup default).
PROJECTS=(); SETUPS=()
add_project() {
  local spec="$1"
  local dir="${spec%%:*}"
  local setup="${spec#*:}"
  [ "$setup" = "$spec" ] && setup="$DEFAULT_SETUP"
  dir="$(cd "$dir" 2>/dev/null && pwd)" || { echo "project not found: $dir" >&2; exit 1; }
  PROJECTS+=("$dir"); SETUPS+=("$setup")
}
if [ -n "$PROJECT_SPECS" ]; then
  # shellcheck disable=SC2086
  for spec in $PROJECT_SPECS; do add_project "$spec"; done
else
  add_project "$DEFAULT_PROJECT"
fi
for p in "${PROJECTS[@]}"; do [ -d "$p" ] || { echo "project not found: $p" >&2; exit 1; }; done

# Validate concurrency levels.
IFS=',' read -ra CONCS <<< "$CONCURRENCY"
for c in "${CONCS[@]}"; do
  case "$c" in ''|*[!0-9]*) echo "invalid concurrency: $c" >&2; exit 2 ;; esac
  [ "$c" -ge 1 ] || { echo "concurrency must be >= 1: $c" >&2; exit 2; }
done

# Validate methods.
ALL_METHODS="git-clone git-worktree pnpm-store cp-clonefile replica"
if [ -n "$METHODS" ]; then
  IFS=',' read -ra METHODS_SEL <<< "$METHODS"
  for m in "${METHODS_SEL[@]}"; do
    case "$m" in
      git-clone|git-worktree|pnpm-store|cp-clonefile|replica) ;;
      *) echo "unknown method: $m (allowed: $ALL_METHODS)" >&2; exit 2 ;;
    esac
  done
else
  METHODS_SEL=($ALL_METHODS)
fi

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/replica-bench.XXXXXX")"
touch "$SCRATCH/.metadata_never_index"
cleanup() {
  if [ "$KEEP" -eq 1 ]; then echo "kept scratch: $SCRATCH"; return; fi
  if [ -d "$SCRATCH/root" ] && command -v replica >/dev/null 2>&1; then
    for ws in "$SCRATCH"/root/ws/*; do
      [ -e "$ws" ] && REPLICA_ROOT="$SCRATCH/root" replica rm "$(basename "$ws")" >/dev/null 2>&1 || true
    done
    rm -rf "$SCRATCH/root"
  fi
  rm -rf "$SCRATCH"
}
trap cleanup EXIT

export NO_COLOR=1
REPLICA_ROOT="$SCRATCH/root"
export REPLICA_ROOT
mkdir -p "$REPLICA_ROOT"
export npm_config_cache="$SCRATCH/npm-cache"
mkdir -p "$npm_config_cache"

[ "$(stat -f %d "$SCRATCH")" = "$(stat -f %d "$REPLICA_ROOT")" ] || { echo "scratch and root differ in volume" >&2; exit 1; }
diskutil info "$(df "$SCRATCH" | tail -1 | awk '{print $NF}')" 2>/dev/null | grep -q "APFS" \
  || { echo "scratch volume is not APFS; clonefile would fail" >&2; exit 1; }

FREE_KB=$(df -k "$SCRATCH" | tail -1 | awk '{print $4}')
if [ "$FREE_KB" -lt 2000000 ]; then
  echo "need at least 2 GB free for the benchmark; have $((FREE_KB / 1024)) MB" >&2; exit 1
fi

HELPERS="$HERE/helpers.sh"
W="$SCRATCH/work"
RAW="$SCRATCH/raw"
mkdir -p "$W" "$RAW"

log() { echo "== $*" >&2; }

# -------- small utilities ----------------------------------------------------

# Copy project into a fresh dir (dropping prior install output) and git-init it.
prepare_source() {
  local dest="$1" src="$2"
  rm -rf "$dest"; mkdir -p "$dest"
  (cd "$src" && tar --exclude=node_modules --exclude=.git --exclude=target --exclude=.next -cf - .) \
    | (cd "$dest" && tar -xf -)
  git -C "$dest" init -q
  git -C "$dest" add -A
  git -C "$dest" -c user.name=replica-bench -c user.email=bench@localhost commit -qm "bench source" >/dev/null
}

file_count() { find "$1" -path "$1/.git" -prune -o -type f -print 2>/dev/null | wc -l | tr -d ' '; }
disk_used_kb() { df -k "$REPLICA_ROOT" | tail -1 | awk '{print $3}'; }
memory_pressure_sample() {
  memory_pressure 2>/dev/null | awk '/System-wide memory free percentage/{print $NF}' || echo unknown
}

# Run command $N times concurrently; each writes "start end" (epoch seconds)
# to COLLECT/$tag.<i>.timing. Returns overall batch wall seconds on stdout.
run_concurrent() {
  local tag="$1" n="$2" cmd="$3"
  local collect="$RAW/collect"
  mkdir -p "$collect"
  rm -f "$collect/$tag."*
  local start end pid i pids=()
  start=$(python3 -c 'import time; print(time.time())')
  for i in $(seq 1 "$n"); do
    {
      local istart iend
      istart=$(python3 -c 'import time; print(time.time())')
      eval "$cmd" >/dev/null 2>&1 || true
      iend=$(python3 -c 'import time; print(time.time())')
      printf '%.6f %.6f\n' "$istart" "$iend" >"$collect/$tag.$i.timing"
    } &
    pids+=($!)
  done
  for pid in "${pids[@]}"; do wait "$pid"; done
  end=$(python3 -c 'import time; print(time.time())')
  python3 -c "print('%.6f' % (float('$end') - float('$start')))"
}

# median + P95 (nearest-rank) + min/max/count from a file of durations (ms).
stats_file() {
  python3 - "$1" <<'PY'
import sys, statistics
vals = sorted(float(x) for x in open(sys.argv[1]) if x.strip())
if not vals:
    print("0 0 0 0 0 0"); sys.exit()
n = len(vals)
print(f"{statistics.mean(vals):.3f} {statistics.median(vals):.3f} "
      f"{vals[max(0,(n*95+99)//100-1)]:.3f} {vals[0]:.3f} {vals[-1]:.3f} {n}")
PY
}

# Clean up any workspaces a method may have left under $W.
clean_method_ws() {
  local method="$1"
  case "$method" in
    git-clone|git-worktree|pnpm-store|cp-clonefile)
      rm -rf "$W"/${method}-* 2>/dev/null || true ;;
    replica)
      for f in "$W"/replica-path-*; do
        [ -e "$f" ] || continue
        local wp; wp="$(cat "$f" 2>/dev/null)"
        [ -n "$wp" ] && REPLICA_ROOT="$REPLICA_ROOT" replica rm "$(basename "$wp")" >/dev/null 2>&1 || true
        rm -f "$f"
      done ;;
  esac
}

# -------- per-method workloads (create ONE workspace) ------------------------

wl_git_clone() {   # tag src setup
  local tag="$1" src="$2" setup="$3"
  local dest="$W/git-clone-$tag"
  rm -rf "$dest"
  git clone -q "$src" "$dest" && (cd "$dest" && sh -c "$setup") >/dev/null 2>&1
}
wl_git_worktree() { # tag src setup
  local tag="$1" src="$2" setup="$3"
  local dest="$W/wt-$tag"
  rm -rf "$dest"
  git -C "$src" worktree add -q --detach "$dest" HEAD >/dev/null 2>&1 && (cd "$dest" && sh -c "$setup") >/dev/null 2>&1
}
wl_pnpm_store() {  # tag src setup
  local tag="$1" src="$2" setup="$3"
  local dest="$W/pnpm-$tag"
  rm -rf "$dest"
  cp -a "$src/." "$dest/"; rm -rf "$dest/.git"
  (cd "$dest" && PNPM_STORE_DIR="$W/shared-cache/pnpm-store" sh -c "$setup") >/dev/null 2>&1
}
wl_cp_clonefile() { # tag base
  local tag="$1" base="$2"
  local dest="$W/cp-$tag"
  rm -rf "$dest"; cp -c -R "$base" "$dest" >/dev/null 2>&1
}
wl_replica_new() { # tag
  local tag="$1" f="$W/replica-path-$tag"
  REPLICA_ROOT="$REPLICA_ROOT" replica new --ttl 1h >"$f" 2>/dev/null
}

# -------- benchmark one (project, concurrency, method) -----------------------

bench_batch() {
  # $1 pname $2 src $3 setup $4 base $5 base_files $6 conc $7 method
  local pname="$1" src="$2" setup="$3" base="$4" base_files="$5"
  local conc="$6" method="$7"
  local tag="${pname}-c${conc}-${method}"
  local collect="$RAW/collect"
  mkdir -p "$collect"

  log "project=$pname concurrency=$conc method=$method"

  # Choose run count.
  local n_runs="$RUNS"
  case "$method" in git-clone|git-worktree|pnpm-store) n_runs="$SLOW_RUNS" ;; esac

  # Warmup batches (untimed).
  for i in $(seq 1 "$WARMUP"); do
    local wt="warm$i"
    case "$method" in
      git-clone)    wl_git_clone   "$wt" "$src" "$setup" ;;
      git-worktree) wl_git_worktree "$wt" "$src" "$setup" ;;
      pnpm-store)   wl_pnpm_store  "$wt" "$src" "$setup" ;;
      cp-clonefile) wl_cp_clonefile "$wt" "$base" ;;
      replica)      wl_replica_new "$wt" ;;
    esac
  done
  clean_method_ws "$method"

  # Disk + memory pressure before.
  local disk_before mem_before rs_before
  disk_before=$(disk_used_kb)
  mem_before=$(memory_pressure_sample)
  rs_before=$(REPLICA_ROOT="$REPLICA_ROOT" replica stats --json 2>/dev/null || echo '{}')

  # Measurement batches: each launches `conc` concurrent creations.
  local batch wall total_wall_s=0
  local all_durations="$RAW/collect/${tag}.durations"
  : >"$all_durations"
  for batch in $(seq 1 "$n_runs"); do
    local btag="${tag}-b${batch}"
    case "$method" in
      git-clone)
        wall=$(run_concurrent "$btag" "$conc" "wl_git_clone job\$RANDOM '$src' '$setup'") ;;
      git-worktree)
        wall=$(run_concurrent "$btag" "$conc" "wl_git_worktree job\$RANDOM '$src' '$setup'") ;;
      pnpm-store)
        wall=$(run_concurrent "$btag" "$conc" "wl_pnpm_store job\$RANDOM '$src' '$setup'") ;;
      cp-clonefile)
        wall=$(run_concurrent "$btag" "$conc" "wl_cp_clonefile job\$RANDOM '$base'") ;;
      replica)
        wall=$(run_concurrent "$btag" "$conc" "wl_replica_new job\$RANDOM") ;;
    esac
    total_wall_s=$(awk -v a="$total_wall_s" -v b="$wall" 'BEGIN{printf "%.6f", a+b}')
    # collect this batch's individual durations
    for i in $(seq 1 "$conc"); do
      cat "$collect/$btag.$i.timing" 2>/dev/null >>"$all_durations" || true
    done
  done

  # Disk + memory after.
  local disk_after mem_after rs_after
  disk_after=$(disk_used_kb)
  mem_after=$(memory_pressure_sample)
  rs_after=$(REPLICA_ROOT="$REPLICA_ROOT" replica stats --json 2>/dev/null || echo '{}')

  # Convert start/end pairs into per-iteration durations (ms).
  local durations="$RAW/collect/${tag}.dur-ms"
  : >"$durations"
  while read -r s e; do
    [ -n "$s" ] && [ -n "$e" ] \
      && awk -v s="$s" -v e="$e" 'BEGIN{printf "%.3f\n", (e - s) * 1000}' >>"$durations"
  done <"$all_durations"

  local mean median p95 mn mx n
  read -r mean median p95 mn mx n < <(stats_file "$durations")

  local disk_delta
  disk_delta=$(awk -v a="$disk_after" -v b="$disk_before" 'BEGIN{printf "%.1f", a-b}')

  local total_wall_ms
  total_wall_ms=$(awk -v s="$total_wall_s" 'BEGIN{printf "%.1f", s*1000}')

  jq -c -n \
    --arg pname "$pname" --arg conc "$conc" --arg method "$method" \
    --argjson n_runs "$n_runs" --argjson conc "$conc" \
    --arg mean "$mean" --arg median "$median" --arg p95 "$p95" \
    --arg min "$mn" --arg max "$mx" --argjson n "$n" \
    --arg total_wall_ms "$total_wall_ms" \
    --arg disk_before_kb "$disk_before" --arg disk_after_kb "$disk_after" \
    --arg disk_delta_kb "$disk_delta" \
    --arg mem_before "$mem_before" --arg mem_after "$mem_after" \
    --arg rs_before "$rs_before" --arg rs_after "$rs_after" \
    '{project:$pname, concurrency:($conc|tonumber), method:$method,
      batches:$n_runs, concurrency_level:$conc,
      mean_ms:($mean|tonumber), median_ms:($median|tonumber),
      p95_ms:($p95|tonumber), min_ms:($min|tonumber), max_ms:($max|tonumber),
      iterations:$n, total_wall_ms:($total_wall_ms|tonumber),
      disk_before_kb:($disk_before_kb|tonumber),
      disk_after_kb:($disk_after_kb|tonumber), disk_delta_kb:($disk_delta_kb|tonumber),
      mem_pressure_before:$mem_before, mem_pressure_after:$mem_after,
      replica_stats_before:$rs_before, replica_stats_after:$rs_after}' \
    >>"$RAW/batches.jsonl"

  clean_method_ws "$method"

  log "  ${method} c=$conc median=${median}ms P95=${p95}ms "
  log "  total_wall=${total_wall_ms}ms disk_delta=${disk_delta}KB iters=$n"
}

# -------- main ---------------------------------------------------------------

echo "== Replica benchmark suite (Phase 2)"
echo "  machine: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
echo "  macOS: $(sw_vers -productVersion) ($(sw_vers -buildVersion))"
echo "  concurrency: ${CONCS[*]}  methods: ${METHODS_SEL[*]}"
echo "  projects: ${#PROJECTS[@]}"

REPLICA_VERSION="$(replica --version 2>/dev/null || echo unknown)"
: >"$RAW/batches.jsonl"

for pidx in "${!PROJECTS[@]}"; do
  pname="project-$((pidx + 1))"
  src_project="${PROJECTS[$pidx]}"
  setup="${SETUPS[$pidx]}"

  log "preparing source for $pname ($src_project)"
  SRC="$W/src-$pidx"
  prepare_source "$SRC" "$src_project"
  FILE_COUNT=$(file_count "$SRC")
  log "  $FILE_COUNT files in source"

  # Build the replica base once (one-time cost, timed separately).
  # replica always publishes to $REPLICA_ROOT/base.
  BASE="$REPLICA_ROOT/base"
  rm -rf "$BASE"
  log "building replica base for $pname (one-time cost)"
  base_start=$(python3 -c 'import time; print(time.time())')
  REPLICA_ROOT="$REPLICA_ROOT" replica base build --repo "$SRC" --setup "$setup" >"$RAW/base-build-$pidx.log" 2>&1
  base_end=$(python3 -c 'import time; print(time.time())')
  base_secs=$(python3 -c "print(f'{$base_end - $base_start:.3f}')")
  base_logical_kb=$(du -sk "$BASE" 2>/dev/null | awk '{print $1}')
  base_files=$(file_count "$BASE")
  [ -d "$BASE" ] || { echo "base build failed for $pname; see $RAW/base-build-$pidx.log" >&2; exit 1; }
  log "  base built in ${base_secs}s ($base_files files, ${base_logical_kb} KB)"

  # Stash base metadata for the report.
  echo "$pidx $base_secs $base_logical_kb $base_files $setup" >>"$RAW/base-meta.txt"

  for conc in "${CONCS[@]}"; do
    for method in "${METHODS_SEL[@]}"; do
      bench_batch "$pname" "$SRC" "$setup" "$BASE" "$base_files" "$conc" "$method"
    done
  done
done

# -------- aggregate + report -------------------------------------------------

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

export DATE MACHINE MACOS BUILD ARCH NCPU LOAD_START LOAD_END REPLICA_VERSION
python3 - "$RAW/batches.jsonl" "$RAW/base-meta.txt" "$JSON" <<'PY'
import json, os, sys
batches_path, meta_path, json_path = sys.argv[1], sys.argv[2], sys.argv[3]
batches = []
for line in open(batches_path):
    line = line.strip()
    if line:
        batches.append(json.loads(line))

# Base metadata: pidx base_secs base_logical_kb base_files setup
base_meta = {}
for line in open(meta_path):
    parts = line.split()
    if len(parts) >= 5:
        pidx = parts[0]
        base_meta[pidx] = {
            "base_build_secs": float(parts[1]),
            "base_logical_kb": int(parts[2]),
            "base_files": int(parts[3]),
            "setup": " ".join(parts[4:]),
        }

# Start from any existing report (so repeated runs accumulate instead of overwrite).
projects = {}
concs = set()
methods_seen = set()
if os.path.exists(json_path):
    try:
        existing = json.load(open(json_path))
        for pname, pdata in existing.get("projects", {}).items():
            projects[pname] = {
                "concurrency": {},
                "base_build_secs": pdata.get("base_build_secs"),
                "base_logical_kb": pdata.get("base_logical_kb"),
                "base_files": pdata.get("base_files"),
                "setup": pdata.get("setup"),
            }
            for cname, cdata in pdata.get("concurrency", {}).items():
                projects[pname]["concurrency"][cname] = {
                    "methods": dict(cdata.get("methods", {})),
                }
                concs.add(int(cname))
        methods_seen.update(existing.get("methods", []))
    except (json.JSONDecodeError, OSError):
        pass

for b in batches:
    p = projects.setdefault(b["project"], {
        "concurrency": {}, "base_build_secs": None,
        "base_logical_kb": None, "base_files": None,
        "setup": None,
    })
    concs.add(b["concurrency_level"])
    methods_seen.add(b["method"])
    c = p["concurrency"].setdefault(str(b["concurrency_level"]), {"methods": {}})
    c["methods"][b["method"]] = b
    # attach base meta (project-N maps to pidx = N-1)
    try:
        pidx = str(int(b["project"].split("-")[1]) - 1)
    except (IndexError, ValueError):
        pidx = "0"
    if pidx in base_meta:
        p.update(base_meta[pidx])

report = {
    "date": os.environ.get("DATE", ""),
    "machine": os.environ.get("MACHINE", ""),
    "macos": os.environ.get("MACOS", ""),
    "build": os.environ.get("BUILD", ""),
    "arch": os.environ.get("ARCH", ""),
    "ncpu": int(os.environ.get("NCPU", "0")),
    "load_start": float(os.environ.get("LOAD_START", "0")),
    "load_end": float(os.environ.get("LOAD_END", "0")),
    "replica": os.environ.get("REPLICA_VERSION", ""),
    "methods": sorted(methods_seen),
    "concurrency_levels": sorted(set(int(x) for x in concs)),
    "projects": projects,
}
with open(json_path, "w") as f:
    json.dump(report, f, indent=2)
PY

{
  echo "## Results: $DATE on $MACHINE, macOS $MACOS ($BUILD)"
  echo
  echo "| Project | Concurrency | Method | Batches | Median (ms) | P95 (ms) | Total wall (ms) | Disk delta (KB) |"
  echo "|---|---|---|---|---|---|---|---|"
  python3 - "$RAW/batches.jsonl" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    line = line.strip()
    if not line: continue
    b = json.loads(line)
    print(f"| {b['project']} | {b['concurrency']} | {b['method']} | {b['batches']} | "
          f"{b['median_ms']:.1f} | {b['p95_ms']:.1f} | {b['total_wall_ms']:.1f} | {b['disk_delta_kb']} |")
PY
  echo
  echo "- Concurrency levels: ${CONCS[*]}"
  echo "- Methods: ${METHODS_SEL[*]}"
  echo "- Machine: $MACHINE, macOS $MACOS ($BUILD), $ARCH, $NCPU cores"
  echo "- Load average (1 min): $LOAD_START at start, $LOAD_END at end"
  echo "- Replica: $REPLICA_VERSION"
  echo "- Scratch volume: APFS; source, base, and workspaces share one volume"
  echo "- Methodology: untimed warmup batches per method, P95 nearest-rank over"
  echo "  all individual iterations, disk delta via df on the replica volume,"
  echo "  memory pressure sampled before/after (context only)."
  echo "  Git clones are from a local copy (no network time)."
  echo
  echo "Base build (one-time, per project):"
  while read -r pidx base_secs base_logical_kb base_files setup; do
    echo "- project-$((pidx + 1)): ${base_secs}s ($base_files files, ${base_logical_kb} KB), setup: $setup"
  done <"$RAW/base-meta.txt"
  echo
  echo "Full machine-readable report: $JSON"
} >"$MD"

cat "$MD"
echo
echo "wrote $MD"
echo "wrote $JSON"
