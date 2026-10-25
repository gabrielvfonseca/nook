#!/usr/bin/env bash
# Comprehensive benchmark: compare git clone+install, git worktree+install, cp -c, and nook new
# Usage: bench.sh /path/to/project [scratch-dir] [runs=5]
# scratch-dir MUST be on the same APFS volume as the project for clonefile to work.
# Requires: hyperfine, python3, nook binary in PATH, NOOK_ROOT set to an APFS volume.
set -euo pipefail

SRC="${1:?usage: bench.sh /path/to/project [scratch-dir] [runs=5]}"
SCRATCH="${2:-$(dirname "$SRC")/.nook-bench}"
RUNS="${3:-5}"
mkdir -p "$SCRATCH"

# Check for hyperfine
if ! command -v hyperfine >/dev/null 2>&1; then
    echo "hyperfine not found. Install with: brew install hyperfine"
    exit 1
fi

# Check for nook
if ! command -v nook >/dev/null 2>&1; then
    echo "nook not found in PATH. Build with: cargo build --release"
    exit 1
fi

# Check NOOK_ROOT
if [ -z "${NOOK_ROOT:-}" ]; then
    echo "NOOK_ROOT not set. Export it to an APFS volume."
    exit 1
fi

# Verify NOOK_ROOT is on APFS
if ! stat -f "$NOOK_ROOT" | grep -q "apfs"; then
    echo "NOOK_ROOT ($NOOK_ROOT) is not on APFS. Benchmarks require APFS for clonefile."
    exit 1
fi

# Verify SCRATCH is on same volume as NOOK_ROOT for clonefile to work
if [ "$(stat -f "%d" "$NOOK_ROOT")" != "$(stat -f "%d" "$SCRATCH")" ]; then
    echo "SCRATCH ($SCRATCH) and NOOK_ROOT ($NOOK_ROOT) are on different volumes. clonefile will fail."
    exit 1
fi

echo "=== Nook Benchmark ==="
echo "Project: $SRC"
echo "Scratch: $SCRATCH"
echo "Runs per benchmark: $RUNS"
echo "NOOK_ROOT: $NOOK_ROOT"
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

# Build nook base if needed
echo "=== Building nook base ==="
if [ ! -d "$NOOK_ROOT/base" ]; then
    echo "Building base tree..."
    nook base build --repo "$SRC" --setup "$SETUP_CMD"
else
    echo "Base already exists, using existing base."
fi
echo ""

# Helper: run hyperfine and extract median/p95
run_bench() {
    local name="$1"
    local cmd="$2"
    echo "--- $name ---"
    local output
    output=$(hyperfine --runs "$RUNS" --warmup 1 --style basic "$cmd" 2>&1 || true)
    echo "$output"
    # Extract median and p95 from hyperfine output
    local median
    median=$(echo "$output" | grep -E "Median:" | sed -E 's/.*Median: *([0-9.]+)s.*/\1/')
    local p95
    p95=$(echo "$output" | grep -E "P95:" | sed -E 's/.*P95: *([0-9.]+)s.*/\1/')
    echo "MEDIAN: ${median:-N/A}s  P95: ${p95:-N/A}s"
    echo ""
}

# Benchmark 1: git clone + setup (baseline)
if [ -n "$REPO_URL" ]; then
    run_bench "git clone + setup" \
        "cd '$SCRATCH' && rm -rf git-clone && git clone '$REPO_URL' git-clone && cd git-clone && $SETUP_CMD"
fi

# Benchmark 2: git worktree + setup
if [ -n "$REPO_URL" ] && [ -d "$SRC/.git" ]; then
    run_bench "git worktree + setup" \
        "cd '$SCRATCH' && rm -rf worktree && git -C '$SRC' worktree add '$SCRATCH/worktree' && cd '$SCRATCH/worktree' && $SETUP_CMD"
fi

# Benchmark 3: cp -c (clonefile copy)
run_bench "cp -c (clonefile)" \
    "rm -rf '$SCRATCH/cp-clone' && cp -c -R '$SRC' '$SCRATCH/cp-clone'"

# Benchmark 4: nook new
run_bench "nook new" \
    "nook new --ttl 1m"

# Benchmark 5: nook rm (cleanup)
run_bench "nook rm" \
    "nook gc --dry-run >/dev/null && nook list | head -5 | cut -f1 | xargs -r nook rm"

echo "=== Disk Usage Summary ==="
echo "Volume total: $(df -h "$NOOK_ROOT" | tail -1 | awk '{print $2}')"
echo "Volume used:  $(df -h "$NOOK_ROOT" | tail -1 | awk '{print $3}')"
echo "Volume free:  $(df -h "$NOOK_ROOT" | tail -1 | awk '{print $4}')"
echo "nook stats:"
nook stats

echo ""
echo "=== Results Template for docs/BENCHMARKS.md ==="
cat <<EOF

## Results - $(date -u +"%Y-%m-%d") on $(uname -m) macOS $(sw_vers -productVersion) ($(sw_vers -buildVersion))

| Method | Median (s) | P95 (s) | Files | Disk/clone |
|---|---|---|---|---|
| git clone + setup | | | $(find "$SRC" -type f 2>/dev/null | wc -l | tr -d ' ') | |
| git worktree + setup | | | | |
| cp -c -R | | | | |
| nook new | | | | |
| nook rm | | | | |

- Project: \`$SRC\`
- Setup command: \`$SETUP_CMD\`
- NOOK_ROOT: \`$NOOK_ROOT\`
- Runs per benchmark: $RUNS

EOF