# Benchmarks

Measure before building further. Record machine, macOS version, file count.

## How to Run

```sh
# Requires: hyperfine (brew install hyperfine), nook in PATH, NOOK_ROOT set
./scripts/bench.sh /path/to/your/project [scratch-dir] [runs=5]
```

The script benchmarks:
1. **git clone + setup** - Fresh clone from remote + dependency install
2. **git worktree + setup** - Worktree from existing repo + dependency install
3. **cp -c -R** - APFS clonefile copy (baseline for clonefile speed)
4. **nook new** - Nook's clonefile-based workspace creation
5. **nook rm** - Workspace cleanup via gc

Reports median and P95 wall time over N runs (default 5), file count, and real disk space used.

## Results Format

| Metric | Description |
|---|---|
| Method | The benchmark method |
| Median (s) | Median wall time from hyperfine |
| P95 (s) | 95th percentile wall time |
| Files | File count in project |
| Disk/clone | Extra disk space per clone (from `nook stats`) |

## Results Template

Fill in after running `./scripts/bench.sh`:

```markdown
## Results - YYYY-MM-DD on ARCH macOS VERSION (BUILD)

| Method | Median (s) | P95 (s) | Files | Disk/clone |
|---|---|---|---|---|
| git clone + setup | X.XX | X.XX | NNNNN | Y.Y GB |
| git worktree + setup | X.XX | X.XX | NNNNN | Y.Y GB |
| cp -c -R | X.XX | X.XX | NNNNN | Y.Y GB |
| nook new | X.XX | X.XX | NNNNN | Y.Y MB |
| nook rm | X.XX | X.XX | N/A | N/A |

- Project: `/path/to/project`
- Setup command: `npm ci` (or cargo build, pip install, etc.)
- NOOK_ROOT: `/Volumes/Work/nook` (or $HOME/nook with NOOK_MAX_BYTES)
- Runs per benchmark: 5
```

## Baselines (on your real project)

```sh
time cp -R proj /tmp/a          # copy
time cp -c -R proj /tmp/b       # clonefile (same volume only)
time rm -rf /tmp/a /tmp/b       # delete
time (git clone <url> c && cd c && npm ci)   # current setup path
```

## Where is time really going?

```sh
sudo fs_usage -w -f filesys <agent-pid> | head -n 500
sample <pid> 5
```
If time is dominated by process spawn or security scanning, Nook will not help much.

## Nook Metrics

| Metric | Method |
|---|---|
| `nook new` latency | `hyperfine 'nook new --ttl 1m'` (cold + warm) |
| `nook rm` latency | compare vs `rm -rf` |
| Extra disk per clone | `nook stats` before/after |
| Disk after 100 runs + gc | `nook stats` |

## Decision Rule

Adopt if clone+delete beats current setup+cleanup by >= 5-10x. Otherwise investigate
Spotlight/XProtect first.

## Results

### 2026-10-25 on arm64 macOS 26.6.2 (25G83)

| Method | Median (s) | P95 (s) | Files | Disk/clone |
|---|---|---|---|---|
| git clone + setup | 3.10 | 3.19 | 1842 | ~33 MB |
| git worktree + setup | 2.51 | 2.61 | 1842 | ~33 MB |
| cp -c -R | 1.33 | 1.37 | 1842 | ~33 MB |
| nook new | 0.066 | 0.075 | 1842 | ~0 MB (CoW) |
| nook rm | 0.022 | 0.721 | N/A | N/A |

- Project: `/tmp/nook-bench-project` (npm project, 70 deps)
- Setup command: `npm ci`
- NOOK_ROOT: `/tmp/nook-bench` (APFS, 1TB volume)
- Runs per benchmark: 3

### Key Findings

- **nook new (66ms median)** is **47x faster** than `git clone + npm ci` (3.10s)
- **nook new (66ms median)** is **20x faster** than raw `cp -c` clonefile (1.33s)
- Clonefile copy (`cp -c`) is only 2.3x faster than git clone because npm ci dominates
- Nook's advantage: dependencies pre-installed in base, only metadata records created
- Disk per clone: ~0 MB additional (APFS copy-on-write shares all blocks)
- nook rm median 22ms, but first run can be slower (721ms P95) due to cache effects