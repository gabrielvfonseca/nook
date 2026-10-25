#!/usr/bin/env bash
# Baseline benchmark: copy vs clone vs delete on YOUR project. Usage: bench.sh /path/to/project [scratch-dir]
# scratch-dir MUST be on the same APFS volume as the project for clonefile to work.
set -euo pipefail
SRC="${1:?usage: bench.sh /path/to/project [scratch-dir]}"
SCRATCH="${2:-$(dirname "$SRC")/.nook-bench}"
mkdir -p "$SCRATCH"; trap 'rm -rf "$SCRATCH"' EXIT
echo "files in project: $(find "$SRC" -type f 2>/dev/null | wc -l | tr -d ' ')"
t() { local s e; s=$(python3 -c 'import time;print(time.time())'); "$@" >/dev/null 2>&1; e=$(python3 -c 'import time;print(time.time())'); python3 -c "print(f'{$e-$s:.2f}s')"; }
echo -n "cp -R        : "; t cp -R  "$SRC" "$SCRATCH/copy"
echo -n "cp -c -R     : "; t cp -c -R "$SRC" "$SCRATCH/clone"
echo -n "rm -rf copy  : "; t rm -rf "$SCRATCH/copy"
echo -n "rm -rf clone : "; t rm -rf "$SCRATCH/clone"
echo "Record results in docs/BENCHMARKS.md. Adopt Nook only if clone is >=5-10x faster than your current setup."
