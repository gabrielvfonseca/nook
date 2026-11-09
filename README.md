![Replica Banner](banner.jpg)

# Isolated replicas of your workspace for AI agents

**Isolated, fully installed workspace replicas for every AI agent, in milliseconds. Copy-on-write shares the data, so each workspace adds only metadata, and automatic cleanup keeps the total bounded.**

| | Median | vs `git clone` + `npm ci` |
|---|---|---|
| `git clone` + setup | 3.10s | 1x |
| `git worktree` + setup | 2.51s | 1.2x |
| **`replica new`** | **66ms** | **47x faster** |

*Previous recorded run (2026-10-25, arm64 macOS 26.6.2). A re-run of the current suite is pending. See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for methodology and corrections.*

## How it works

Creating and deleting files is one of the slowest things macOS does, and agent runs create tens of thousands of them: checkouts, `node_modules`, build output.

Replica skips that work. You build a "golden" base tree once, with dependencies installed. Each `replica new` then clones it with APFS `clonefile(2)`. A clone copies no data blocks: blocks stay shared until an agent modifies a file. It does write APFS metadata for every file, about 1.4 MB for a 1,700-file project, so a workspace is cheap rather than free.

To keep clones from piling up, Replica expires them by TTL, evicts the least recently used ones, and can enforce a byte cap. It also gives each workspace its own `TMPDIR` and cleans it up.

## Requirements

- macOS on an APFS volume
- Rust toolchain ([rustup](https://rustup.rs)) to build from source
- The base tree and its clones must live on the **same APFS volume**

## Install

Homebrew (available once the first release is published; see [skills/replica-release](skills/replica-release/SKILL.md)):

```sh
brew tap gabrielvfonseca/tap
brew install replica
```

From source:

```sh
git clone https://github.com/gabrielvfonseca/replica.git
cd replica
make release
PREFIX=$HOME/.local make install
```

Run `replica help` for a guided overview, or `replica help quickstart` to get started.

To also run `replica gc` automatically every 10 minutes through launchd:

```sh
INSTALL_LAUNCHD=1 REPLICA_ROOT=/Volumes/Work/replica make install
```

## Quick start

### 1. Choose where workspaces live

**Recommended: a dedicated APFS volume with a quota.** Clones can never fill your main disk.

```sh
diskutil apfs list                                   # find your APFS container
diskutil apfs addVolume <container> APFS Work -quota 100g
export REPLICA_ROOT=/Volumes/Work/replica
```

Or use the helper script: `./scripts/setup-volume.sh <container> 100g Work`

**Alternative: any APFS volume with a byte cap.**

```sh
export REPLICA_ROOT=$HOME/replica
export REPLICA_MAX_BYTES=50000000000                 # 50 GB
```

See [docs/DISK_SAFETY.md](docs/DISK_SAFETY.md) for the trade-offs.

### 2. Build a base

```sh
replica base build --repo https://github.com/you/proj.git --setup "npm ci"
replica base check                                   # flag path-dependent build output
```

### 3. Create and manage workspaces

```sh
cd "$(replica new --ttl 2h --name fix-login)"   # path on stdout; status on stderr
replica list                                    # table; add --json for scripts
eval "$(replica env <id>)"                      # TMPDIR and shared caches
replica touch <id>                              # keep a long session out of LRU eviction
replica rm <id>                                 # remove one (idempotent)
replica gc                                      # expire and evict old workspaces
replica stats                                   # real volume usage
replica base refresh                            # rebuild the base with its original repo and setup
```

Run `replica help` for the guide, or `replica <command> --help` for one command. Exit codes are listed in [docs/CLI.md](docs/CLI.md#exit-codes).

### 4. Give your coding agents the skill

The skills in [`skills/`](skills/) follow the SKILL.md format. Install them with the skills.sh CLI:

```sh
npx skills add gabrielvfonseca/replica            # all skills
```

`replica setup` also detects Claude Code, Cursor, Copilot, Zed, and Aider on your machine and writes each one its configuration.

## Replica vs. git worktree

| | git worktree | Replica |
|---|---|---|
| Setup | Run `npm ci` / `cargo build` in every worktree | Dependencies already installed in the base |
| Disk usage | Full copy of files and dependencies | Shared data blocks; each clone adds APFS metadata, and only changed files use new space |
| Isolation | Full | Full (copy-on-write) |
| Disk limits | None, can fill your disk | TTL, LRU eviction, optional byte cap |
| Temp files | Manual cleanup | Per-workspace `TMPDIR`, cleaned up automatically |
| Base updates | n/a | Rebuild, then swap; existing clones unaffected |

## Limitations

**Path-dependent build output.** Some tools hardcode absolute paths: Python venvs, `node_modules/.bin` scripts, Cargo fingerprints, TypeScript `.tsbuildinfo`. These break when cloned. Run `replica base check` to find them, then fix them with `--post-clone` hooks or regenerate them after cloning. See [`replica base check`](docs/CLI.md#replica-base-check) and the [replica-base skill](skills/replica-base/SKILL.md).

**What Replica doesn't do:**

- Speed up reads or writes inside a running agent
- Fix Spotlight, XProtect, or Docker bind-mount overhead
- Clone across volumes (source and target must share one APFS volume)

## Benchmarks

The suite in [`benchmarks/`](benchmarks/README.md) compares `replica new` with `git clone` + install, `git worktree` + install, and `cp -c`, and it measures disk per clone.

```sh
make bench                                         # bundled fixture project (~12 min)
make bench PROJECT=~/code/app SETUP="npm ci" RUNS=30
```

The last recorded run (2026-10-25, arm64 macOS 26.6.2, 1,842 files) is in [docs/BENCHMARKS.md](docs/BENCHMARKS.md), together with the corrections to it. A re-run of the current suite is pending. Replica adds about 1.4 MB of APFS metadata per clone for a 1,700-file project. Data blocks are shared, so that is the whole cost.

## Development

```sh
make help                 # every target
make check                # fmt + clippy + tests (what CI runs)
make bench                # benchmark suite
make skills-check         # validate skills/*/SKILL.md
```

CI runs on GitHub Actions macOS runners. See [CONTRIBUTING.md](CONTRIBUTING.md) to get involved, and [AGENTS.md](AGENTS.md) for instructions aimed at coding agents working on this repo.

## Docs

| | |
|---|---|
| [SPEC](docs/SPEC.md) | Problem, goals, non-goals, requirements |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Components, layout, flows |
| [CLI](docs/CLI.md) | Command reference |
| [DATA_MODEL](docs/DATA_MODEL.md) | SQLite schema, state machine |
| [DISK_SAFETY](docs/DISK_SAFETY.md) | Quotas, GC, caches, temp dirs |
| [BENCHMARKS](docs/BENCHMARKS.md) | Methodology, results, corrections |
| [benchmarks/](benchmarks/README.md) | Benchmark suite and fixture project |
| [skills/](skills/) | Agent skills: usage, base, maintenance, development, benchmarks, release |
| [ROADMAP](docs/ROADMAP.md) | Phases and open questions |
| [CHANGELOG](CHANGELOG.md) | Notable changes |

## License

[Apache-2.0](LICENSE.md)
