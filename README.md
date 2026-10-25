# Nook

> Fast because it refuses to do the work.

Nook is a Rust CLI that gives every agent run an isolated, ready-to-use project
tree on macOS by cloning a prebuilt "golden" tree with `clonefile(2)` instead of
checking out, copying, or installing. It also keeps clones from filling your disk.

## Why

File creation and deletion are the slowest things macOS does. Agent workloads
create tens of thousands of small files per run (checkout, `node_modules`, build
output). A clone still writes a metadata record per file but copies no file data
blocks—APFS copy-on-write shares blocks until a file is modified.

## How this compares to git worktree

| Aspect | git worktree | Nook |
|---|---|---|
| Setup speed | Must run `npm ci` / `cargo build` per worktree | Dependencies pre-installed in base |
| Disk usage | Full copy of worktree + deps | CoW shares base blocks; only deltas use space |
| Isolation | Full isolation | Full isolation (CoW) |
| Disk bounds | None (can fill disk) | TTL + LRU + optional byte cap |
| Temp cleanup | Manual | Automatic per-workspace TMPDIR |
| Base updates | Not applicable | Atomic rebuild, old clones unaffected |

Nook's advantage: dependencies are already installed and disk use stays bounded.

## What it does NOT do

- It does not speed up reads/writes inside a running agent.
- It does not fix Spotlight, XProtect, or Docker bind-mount overhead.
- It does not work across volumes (clone source and target must be on one APFS volume).

## Known limitations

- **Path-dependent build output**: Some tools hardcode absolute paths (Python venvs, node_modules/.bin scripts, Cargo fingerprints, TypeScript .tsbuildinfo). These break when cloned. Run `nook base check` to detect them, and use `--post-clone` hooks or recreate after cloning. See [base check docs](docs/CLI.md#nook-base-check).

## Quick start

```sh
# Option 1: dedicated quota'd APFS volume (recommended; see docs/DISK_SAFETY.md)
diskutil apfs list                          # find your APFS container
diskutil apfs addVolume <container> APFS Work -quota 100g
export NOOK_ROOT=/Volumes/Work/nook

# Option 2: any APFS volume with size cap (e.g., main disk)
export NOOK_ROOT=$HOME/nook
export NOOK_MAX_BYTES=50000000000  # 50 GB cap

nook base build --repo https://github.com/you/proj.git --setup "npm ci"
nook new --ttl 2h        # prints workspace path
nook list
nook rm <id>
nook gc                  # expire + evict
nook stats               # real volume usage
```

## Install / develop

```sh
cargo build --release          # needs Rust (rustup) on macOS
make check                     # fmt + clippy + tests
PREFIX=$HOME/.local make install
INSTALL_LAUNCHD=1 NOOK_ROOT=/Volumes/Work/nook make install   # + gc every 10 min
./scripts/setup-volume.sh <container> 100g Work                # quota'd volume (find container with `diskutil apfs list`)
./scripts/bench.sh /path/to/your/project                       # measure FIRST
```

## Docs

| File | Contents |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | Problem, goals, non-goals, requirements |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Components, layout, flows |
| [docs/CLI.md](docs/CLI.md) | Command reference |
| [docs/DATA_MODEL.md](docs/DATA_MODEL.md) | SQLite schema, state machine |
| [docs/DISK_SAFETY.md](docs/DISK_SAFETY.md) | Quota, GC, caches, temp dirs |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | How to measure (no results yet) |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases and open questions |
| [AGENTS.md](AGENTS.md) | Instructions for coding agents building this |

## Benchmarks (run on arm64 macOS 26.6.2)

| Method | Median | P95 | Speedup vs git clone |
|---|---|---|---|
| git clone + setup | 3.10s | 3.19s | 1x (baseline) |
| git worktree + setup | 2.51s | 2.61s | 1.2x |
| cp -c (clonefile) | 1.33s | 1.37s | 2.3x |
| **nook new** | **66ms** | **75ms** | **47x** |
| **nook rm** | **22ms** | **721ms** | N/A |

Test project: 1,842 files, 32MB (npm project with 70 dependencies)
- `nook new` is **47x faster** than `git clone + npm ci`
- `nook new` is **20x faster** than `cp -c` (raw clonefile)
- Clone creates zero-copy workspace (only metadata records)
- Disk usage per clone: ~0 MB additional (CoW blocks shared)

## Status

Code compiles and `make check` passes (fmt, clippy, tests) on macOS. GitHub Actions CI runs on macOS runners. Benchmarks run on arm64 macOS 26.6.2 (M-series).