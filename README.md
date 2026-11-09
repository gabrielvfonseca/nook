![Replica Banner](banner.jpg)

# Isolated replicas of your workspace for AI agents

**Replica gives every agent run its own fully installed copy of your project, created in milliseconds with APFS copy-on-write. Each copy is a real git checkout, and automatic cleanup keeps total disk use bounded.**

```sh
cd "$(replica new --ttl 2h --name fix-login)"   # a ready workspace; the path goes to stdout
git checkout -b agent/fix-login && git commit -am "fix login" && git push -u origin agent/fix-login
replica rm <id>                                 # done; or let the TTL expire
```

## Why

Creating files is one of the slowest things macOS does. An agent run creates thousands of them (a checkout, `node_modules`, build output), and a fresh install takes seconds to minutes before the agent does any work.

Replica builds a **base** once: a git clone with its dependencies installed. Each `replica new` clones that base with one `clonefile(2)` call. The data blocks stay shared with the base until a file changes, so the new workspace is ready almost at once.

## What you get

- **Fast workspaces.** A clone of a 1,700-file project takes tens of milliseconds, against seconds for `git clone` plus an install. [Numbers and methodology](docs/BENCHMARKS.md).
- **Real git.** Each workspace is a full git checkout. It can branch, commit, and push, and the base stays untouched. [Details below](#git-branch-commit-and-push-from-a-workspace).
- **Bounded disk.** TTL expiry, least-recently-used eviction, a byte cap, and an optional dedicated APFS volume with a quota.
- **Cheap updates.** `replica base refresh` brings the base up to date with its remote without a full rebuild. [Details below](#keeping-the-base-fresh).
- **Agent-ready.** A stable CLI with `--json` output, documented exit codes, and skills for Claude Code, Cursor, and the skills.sh CLI.

## Requirements

- macOS on an APFS volume. Replica uses `clonefile(2)`, which Linux does not provide. Tested on macOS 26.6 (Apple M1 Max).
- The base tree and every workspace must be on the **same APFS volume**. Clones cannot cross volumes, and Replica refuses rather than silently copying.

## Install

Binaries are published for macOS on Apple Silicon (`arm64`) and Intel (`x86_64`). A universal binary covers both.

```sh
# Homebrew: not wired up yet. The formula is in homebrew/ (see homebrew/README.md).

# Installer script: verifies the download against sha256sums.txt
curl -fsSL https://raw.githubusercontent.com/gabrielvfonseca/replica/main/scripts/get-replica.sh | sh

# From source (needs a Rust toolchain, https://rustup.rs)
git clone https://github.com/gabrielvfonseca/replica.git && cd replica
make release && PREFIX=$HOME/.local make install
```

Run `replica help` for a guided overview, or `replica help quickstart` to get started.

## Quick start

**1. Choose a volume.** A dedicated APFS volume with a quota is the safest setup, because clones can never fill your main disk.

```sh
diskutil apfs list                                   # find your APFS container
./scripts/setup-volume.sh disk3 100g Work            # creates /Volumes/Work, turns off Spotlight indexing there
export REPLICA_ROOT=/Volumes/Work/replica
```

Any APFS directory works too. Add `REPLICA_MAX_BYTES` to cap its size. See [DISK_SAFETY](docs/DISK_SAFETY.md) for the trade-offs.

**2. Build a base once.**

```sh
replica base build --repo https://github.com/you/proj.git --setup "npm ci"
replica base check                                   # find build output that breaks when cloned
```

**3. Create, use, and remove workspaces.**

```sh
cd "$(replica new --ttl 4h --name feature-x)"        # path on stdout, status on stderr
eval "$(replica env <id>)"                           # per-workspace TMPDIR and shared caches
replica touch <id>                                   # keep a long session out of LRU eviction
replica list                                         # table; add --json for scripts
replica rm <id>                                      # remove one (idempotent)
```

**4. Give your agents the skill.**

```sh
npx skills add gabrielvfonseca/replica               # skills.sh CLI: all six skills
replica setup --dry-run                              # see what setup would write for Claude Code, Cursor, Copilot, Zed, Aider
replica setup                                        # write it
```

## Git: branch, commit, and push from a workspace

Yes. A workspace is a complete git repository, not a read-only copy:

| Operation | In a workspace | Effect on the base |
|---|---|---|
| `git checkout -b`, `git commit` | Works. Commits live in the workspace's `.git` | None |
| `git push origin <branch>` | Works, to the remote the base was built from | None |
| `git pull`, `git fetch` | Works | None |
| `git status`, `git diff` | Works | None |

Things to know:

- **Remote credentials.** `push` uses whatever credentials git uses on this machine (SSH keys, a credential helper). Replica does not add or store any.
- **The remote is the one you built from.** `origin` in a workspace is the `--repo` you passed to `base build`. If that is a local path, pushes go to that path.
- **Workspaces are independent.** Two workspaces on the same branch will conflict on push, the same as two clones would. Use one branch per workspace.
- **Verified.** `tests/git_workspace.rs` builds a base from a bare repo, commits and pushes from a workspace, and checks that the origin and the base are as expected.

For agents that open pull requests, the flow is: `replica new`, work and commit on a branch, push, open the PR with `gh pr create`, then `replica rm`.

## Keeping the base fresh

When the remote moves on, `replica base refresh` updates the base. It does not rebuild from scratch:

1. Clonefiles the current base. This copies metadata only, so it is quick and uses almost no extra disk.
2. Runs `git fetch` and resets the copy to `origin/HEAD`. Only files that changed upstream are rewritten.
3. Re-runs setup **only if a lockfile changed**. If none did, setup is skipped entirely.
4. Swaps the new base in. Existing workspaces are unaffected, because each one owns its blocks.

So an upstream commit that does not touch dependencies costs a fetch and a checkout. A lockfile change costs about as much as the setup command itself. If the fast path fails for any reason, refresh falls back to a full rebuild.

Check freshness with `replica base status`, and use `replica new --require-fresh` to refuse a stale base. The timing of the fast path is measured in the [benchmark suite](benchmarks/README.md); the results will appear in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Disk usage: what a workspace really costs

- **At creation**, a workspace adds APFS metadata for every file and directory it contains. For the benchmark fixture (about 1,700 files) that is about 1.4 MB. Data blocks are shared with the base.
- **After the agent works**, every file it changes takes real space, because APFS copies the blocks that change. Rewriting 500 MB of build output costs 500 MB, and so does a `npm rebuild` that rewrites `node_modules`.
- **Deleting** a workspace frees only the blocks no other file shares. A fresh workspace frees little, and a heavily changed one frees much more.

This is why the TTL, LRU eviction, and byte cap exist. [DISK_SAFETY](docs/DISK_SAFETY.md) explains each one.

Spotlight can also add writes on a busy volume. `scripts/setup-volume.sh` turns indexing off for the dedicated volume only (it needs `sudo`). We have not yet measured how much that changes benchmark results.

## Limitations

- **Path-dependent build output.** Some tools write absolute paths into their output: Python venvs, `node_modules/.bin` scripts, Cargo fingerprints, TypeScript `.tsbuildinfo`. These break in a clone. Run `replica base check` to find them, then fix them with `--post-clone` or by regenerating them. See the [replica-base skill](skills/replica-base/SKILL.md).
- **Cost scales with file count.** Each clone writes metadata for every file. Replica is much faster than copying or cloning file by file, but it is not free, and very large trees (hundreds of thousands of files) have not been measured.
- **Same volume only.** Source, base, and workspaces must share one APFS volume.
- **Not a speedup inside a running agent.** Replica makes workspace creation fast. It does not speed up reads or writes during a run, nor does it fix Spotlight or XProtect scanning.
- **macOS only.** Linux and Windows are out of scope.

## Benchmarks

The suite in [`benchmarks/`](benchmarks/README.md) compares `replica new` with `git clone` plus install, `git worktree` plus install, and `cp -c`, and it also measures `replica rm` and `replica base refresh`.

```sh
make bench                                         # bundled fixture project (~15 min)
make bench PROJECT=~/code/app SETUP="npm ci" RUNS=30
```

Two runs are recorded in [docs/BENCHMARKS.md](docs/BENCHMARKS.md), with methodology and corrections:

- **2026-11-09, under heavy load** (1-minute load 171–184 on 10 cores, from other sessions). Relative ordering matches the clean run, but absolute times are inflated.

  | Method | Median |
  |---|---|
  | `git clone` + setup | 3.9 s |
  | `git worktree` + setup | 3.1 s |
  | `cp -c -R` of a built tree | 1.3 s |
  | **`replica new`** | **78 ms** |
  | `replica rm` | 235 ms |
  | `replica base refresh` (no upstream changes) | 534 ms |

- **2026-10-25, quiet machine** (previous harness, 1,842 files): `replica new` 66 ms, `git clone` + setup 3.10 s, `cp -c` 1.33 s. Its `replica rm` row and its disk column are superseded.

Replica adds about 1.4 MB of APFS metadata per clone for a 1,700-file project, measured on a quiet machine. Data blocks are shared, and changed files cost real space as the agent works. The [replica-benchmarks skill](skills/replica-benchmarks/SKILL.md) lists what a clean run needs.

## Development

```sh
make help              # every target
make check             # fmt, clippy (warnings are errors), and tests; what CI runs
make bench             # benchmark suite
make dist              # release binaries in bin/
```

CI runs on GitHub Actions macOS runners. [CONTRIBUTING.md](CONTRIBUTING.md) covers the workflow, and [AGENTS.md](AGENTS.md) holds the rules for coding agents. The [replica-dev skill](skills/replica-dev/SKILL.md) describes the code layout.

## Docs

| | |
|---|---|
| [CLI](docs/CLI.md) | Every command, flag, output, and exit code |
| [SPEC](docs/SPEC.md) | Problem, goals, non-goals, requirements |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Components, layout, and flows |
| [DATA_MODEL](docs/DATA_MODEL.md) | SQLite schema, state machine, migrations |
| [DISK_SAFETY](docs/DISK_SAFETY.md) | Quotas, TTL, eviction, caches, and what deleting frees |
| [BENCHMARKS](docs/BENCHMARKS.md) | Methodology, results, and corrections |
| [benchmarks/](benchmarks/README.md) | The benchmark suite and fixture project |
| [skills/](skills/README.md) | Agent skills: usage, base, maintenance, development, benchmarks, release |
| [ROADMAP](docs/ROADMAP.md) | Shipped work, next steps, open questions |
| [CHANGELOG](CHANGELOG.md) | Notable changes by version |

## License

[Apache-2.0](LICENSE.md)
