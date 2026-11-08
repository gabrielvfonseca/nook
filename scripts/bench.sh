#!/usr/bin/env bash
# Comprehensive benchmark: compare git clone+install, git worktree+install, cp -c, and replica new
# Usage: bench.sh /path/to/project [scratch-dir] [runs=5]
# scratch-dir MUST be on the same APFS volume as the project for clonefile to work.
# Requires: hyperfine, python3, replica binary in PATH, REPLICA_ROOT set to an APFS volume.
set -euo pipefail

SRC="${1:?usage: bench.sh /path/to/project [scratch-dir] [runs=5]}"
SCRATCH="${2:-$(dirname "$SRC")/.replica-bench}"
RUNS="${3:-5}"
mkdir -p "$SCRATCH"

# Check for hyperfine
if ! command -v hyperfine >/dev/null 2>&1; then
    echo "hyperfine not found. Install with: brew install hyperfine"
    exit 1
fi

# Check for replica
if ! command -v replica >/dev/null 2>&1; then
    echo "replica not found in PATH. Build with: cargo build --release"
    exit 1
fi

# Check REPLICA_ROOT
if [ -z "${REPLICA_ROOT:-}" ]; then
    echo "REPLICA_ROOT not set. Export it to an APFS volume."
    exit 1
fi

# Verify REPLICA_ROOT is on APFS (macOS)
# Find the mount point for REPLICA_ROOT
REPLICA_MOUNT=$(df "$REPLICA_ROOT" | tail -1 | awk '{print $NF}')
if ! diskutil info "$REPLICA_MOUNT" 2>/dev/null | grep -q "APFS"; then
    echo "REPLICA_ROOT ($REPLICA_ROOT) is not on APFS (mount: $REPLICA_MOUNT). Benchmarks require APFS for clonefile."
    exit 1
fi

# Verify SCRATCH is on same volume as REPLICA_ROOT for clonefile to work
# Use stat to get device ID (st_dev)
if [ "$(stat -f "%d" "$REPLICA_ROOT")" != "$(stat -f "%d" "$SCRATCH")" ]; then
    echo "SCRATCH ($SCRATCH) and REPLICA_ROOT ($REPLICA_ROOT) are on different volumes. clonefile will fail."
    exit 1
fi

echo "=== Replica Benchmark ==="
echo "Project: $SRC"
echo "Scratch: $SCRATCH"
echo "Runs per benchmark: $RUNS"
echo "REPLICA_ROOT: $REPLICA_ROOT"
echo "Machine: $(uname -m) $(sw_vers -productVersion) ($(sw_vers -buildVersion))"
echo "Files in project: $(find "$SRC" -type f 2>/dev/null | wc -l | tr -d ' ')"
echo "Project size (du): $(du -sh "$SRC" | cut -f1)"
echo ""

# Get repo URL if in a git repo
REPO_URL=""
if [ -d "$SRC/.git" ]; then
    REPO_URL=$(cd "$SRC" && git config --get remote.origin.url || echo "")
    if [ -n "$REPO_URL" ]; then
        echo "Repo URL: $REPO_URL"
    fi
fi
echo ""

# Detect setup command from project
SETUP_CMD=""
if [ -f "$SRC/package.json" ]; then
    if [ -f "$SRC/pnpm-lock.yaml" ]; then
        SETUP_CMD="pnpm install --frozen-lockfile"
    elif [ -f "$SRC/yarn.lock" ]; then
        SETUP_CMD="yarn install --frozen-lockfile"
    else
        SETUP_CMD="npm ci"
    fi
elif [ -f "$SRC/Cargo.toml" ]; then
    SETUP_CMD="cargo build"
elif [ -f "$SRC/requirements.txt" ] || [ -f "$SRC/pyproject.toml" ]; then
    SETUP_CMD="pip install -r requirements.txt 2>/dev/null || pip install -e . 2>/dev/null || true"
fi

if [ -n "$SETUP_CMD" ]; then
    echo "Detected setup command: $SETUP_CMD"
else
    echo "No recognized setup command detected (package.json, Cargo.toml, requirements.txt)"
    SETUP_CMD="echo 'no setup'"
fi
echo ""

# Build replica base if needed
echo "=== Building replica base ==="
if [ ! -d "$REPLICA_ROOT/base" ]; then
    echo "Building base tree..."
    replica base build --repo "$SRC" --setup "$SETUP_CMD"
else
    echo "Base already exists, using existing base."
fi
echo ""

# Helper: run hyperfine and extract median/p95
run_bench() {
    local name="$1"
    local cmd="$2"
    echo "--- $name ---"
    local json_file
    json_file=$(mktemp)
    hyperfine --runs "$RUNS" --warmup 1 --style basic --shell=default --export-json "$json_file" "$cmd" 2>&1
    cat "$json_file"
    # Extract median and p95 from hyperfine JSON output
    local median
    median=$(jq -r '.results[0].summary.time_wall_clock.median // "N/A"' "$json_file")
    local p95
    # Calculate P95 from measurements
    p95=$(jq -r '[.results[0].measurements[].time_wall_clock.value] | sort | .[length * 0.95 | floor]' "$json_file") || p95="N/A"
    echo "MEDIAN: ${median}s  P95: ${p95}s"
    echo ""
    rm -f "$json_file"
}

# Benchmark 1: git clone + setup (baseline)
if [ -n "$REPO_URL" ]; then
    run_bench "git clone + setup" \
        "cd '$SCRATCH' && rm -rf git-clone && git clone '$REPO_URL' git-clone && cd git-clone && $SETUP_CMD"
fi

# Benchmark 2: git worktree + setup
if [ -n "$REPO_URL" ] && [ -d "$SRC/.git" ]; then
    run_bench "git worktree + setup" \
        "git -C '$SRC' worktree remove -f '$SCRATCH/worktree' 2>/dev/null || true; git -C '$SRC' worktree prune 2>/dev/null || true; git -C '$SRC' worktree add '$SCRATCH/worktree' && cd '$SCRATCH/worktree' && $SETUP_CMD"
fi

# Benchmark 3: cp -c (clonefile copy)
run_bench "cp -c (clonefile)" \
    "rm -rf '$SCRATCH/cp-clone' && cp -c -R '$SRC' '$SCRATCH/cp-clone'"

# Benchmark 4: replica new
run_bench "replica new" \
    "replica new --ttl 1m"

# Benchmark 5: replica rm (cleanup)
run_bench "replica rm" \
    "replica gc --dry-run >/dev/null && replica list | head -5 | cut -f1 | xargs -r replica rm"

echo "=== Disk Usage Summary ==="
echo "Volume total: $(df -h "$REPLICA_ROOT" | tail -1 | awk '{print $2}')"
echo "Volume used:  $(df -h "$REPLICA_ROOT" | tail -1 | awk '{print $3}')"
echo "Volume free:  $(df -h "$REPLICA_ROOT" | tail -1 | awk '{print $4}')"
echo "replica stats:"
replica stats

echo ""
echo "=== Results Template for docs/BENCHMARKS.md ==="
cat <<EOF

## Results - $(date -u +"%Y-%m-%d") on $(uname -m) macOS $(sw_vers -productVersion) ($(sw_vers -buildVersion))

| Method | Median (s) | P95 (s) | Files | Disk/clone |
|---|---|---|---|---|
| git clone + setup | | | $(find "$SRC" -type f 2>/dev/null | wc -l | tr -d ' ') | |
| git worktree + setup | | | | |
| cp -c -R | | | | |
| replica new | | | | |
| replica rm | | | | |

- Project: \`$SRC\`
- Setup command: \`$SETUP_CMD\`
- REPLICA_ROOT: \`$REPLICA_ROOT\`
- Runs per benchmark: $RUNS

EOF