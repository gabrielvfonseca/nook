# Nook

> Fast because it refuses to do the work.

Nook is a Rust CLI that gives every agent run an isolated, ready-to-use project
tree on macOS by cloning a prebuilt "golden" tree with `clonefile(2)` instead of
checking out, copying, or installing. It also keeps clones from filling your disk.

## Why

File creation and deletion are the slowest things macOS does. Agent workloads
create tens of thousands of small files per run (checkout, `node_modules`, build
output). Nook avoids creating them at all.

## What it does NOT do

- It does not speed up reads/writes inside a running agent.
- It does not fix Spotlight, XProtect, or Docker bind-mount overhead.
- It does not work across volumes (clone source and target must be on one APFS volume).

## Quick start

```sh
# one-time: dedicated quota'd APFS volume (see docs/DISK_SAFETY.md)
diskutil apfs addVolume disk3 APFS Work -quota 100g
export NOOK_ROOT=/Volumes/Work/nook

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
./scripts/setup-volume.sh disk3 100g Work                        # quota'd volume
./scripts/bench.sh /path/to/your/project                         # measure FIRST
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

## Status

Repo scaffold complete (CI, tests, packaging). The code has NOT been compiled or run by its author; run `cargo test` first. No benchmarks exist. Do not trust any speedup claim
until `docs/BENCHMARKS.md` has been run on your machine.
