![Replica Banner](banner.jpg)

# Isolated replicas of your workspace for AI agents

**Replica gives every agent run its own fully installed copy of your project, created in milliseconds with APFS copy-on-write. Each copy is a real git checkout, and automatic cleanup keeps total disk use bounded.**

```sh
cd "$(replica new --ttl 2h --name fix-login)"   # a ready workspace; the path goes to stdout
git checkout -b agent/fix-login && git commit -am "fix login" && git push -u origin agent/fix-login
replica rm <id>                                 # done; or let the TTL expire
```

## Why

Agent workloads are dominated by creating and deleting small files (a checkout,
`node_modules`, build output), and that's where macOS is slowest. A fresh
install takes seconds to minutes before the agent does any work.

Replica builds a **base** once: a git clone with its dependencies installed.
Each `replica new` clones that base with one `clonefile(2)` call. The data
blocks stay shared with the base until a file changes, so the new workspace is
ready almost at once. Measured on an Apple M1 Max, `replica new` of a 1,700-file
project takes about 100 ms at N = 1 and about 230 ms at N = 10 concurrent
workspaces, versus seconds for `git clone` plus an install.
[Numbers and methodology](docs/BENCHMARKS.md).

## What you get

- **Fast workspaces.** A clone of a 1,700-file project takes about 100 ms at N = 1 (about 230 ms with 10 concurrent workspaces), against seconds for `git clone` plus an install. Exact numbers vary with machine load; see [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for the full table and methodology.
- **Real git.** Each workspace is a full git checkout. It can branch, commit, and push, and the base stays untouched. [Details below](#git-branch-commit-and-push-from-a-workspace).
- **Bounded disk.** TTL expiry, least-recently-used eviction, a byte cap, and an optional dedicated APFS volume with a quota.
- **Cheap updates.** `replica base refresh` brings the base up to date with its remote without a full rebuild. [Details below](#keeping-the-base-fresh).
- **Agent-ready.** A stable CLI with `--json` output, documented exit codes, and skills for Claude Code, Cursor, and the skills.sh CLI.
- **Concurrency tested.** `tests/concurrency.rs` runs 10 simultaneous `replica new`/use/`rm` cycles with zero SQLite busy errors, no corruption, and all workspaces created and removed. `replica gc` skips workspaces with a fresh lease (`.replica/lease`), so a launchd gc every 10 minutes never deletes a workspace an agent is using.

## Requirements

- macOS on an APFS volume. Replica uses `clonefile(2)`, which Linux does not provide. Tested on macOS 26.6 (Apple M1 Max).
- The base tree and every workspace must be on the **same APFS volume**. Clones cannot cross volumes, and Replica refuses rather than silently copying.

## Install

Binaries are published for macOS on Apple Silicon (`arm64`) and Intel (`x86_64`). A universal binary covers both.

```sh
# Homebrew is not available yet; see HOMEBREW.md.

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

**Branches with different lockfiles.** If you create a workspace from a branch
whose lockfile differs from the base, use `--ref`:

```sh
replica new --ref my-feature-branch
```

Replica compares the branch's lockfile hash with the base's recorded hash. If
they match, the workspace is cloned directly ("cloned directly"). If they differ,
Replica clones the base, checks out `my-feature-branch`, then runs the setup
command (an incremental install) so the workspace gets correct dependencies, and
reports `lockfile drift on branch my-feature-branch; running incremental install`.
The CLI always tells you which path it took.

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

Check freshness with `replica base status`, and use `replica new --require-fresh` to refuse a stale base. The timing of the fast path is measured in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Disk usage: what a workspace really costs

- **At creation**, a workspace adds APFS metadata for every file and directory it contains. For the benchmark fixture (about 1,700 files) that is about 1.4 MB. Data blocks are shared with the base.
- **After the agent works**, every file it changes takes real space, because APFS copies the blocks that change. Rewriting 500 MB of build output costs 500 MB, and so does a `npm rebuild` that rewrites `node_modules`.
- **Deleting** a workspace frees only the blocks no other file shares. A fresh workspace frees little, and a heavily changed one frees much more.

This is why the TTL, LRU eviction, and byte cap exist. [DISK_SAFETY](docs/DISK_SAFETY.md) explains each one.

Spotlight can also add writes on a busy volume. `scripts/setup-volume.sh` turns indexing off for the dedicated volume only (it needs `sudo`). We have not yet measured how much that changes benchmark results.

## Limitations

- **Path-dependent build output.** Some tools write absolute paths into their output: Python venvs, `node_modules/.bin` scripts, TypeScript `.tsbuildinfo`. These break in a clone. Run `replica base check` to find them, then fix them with `--post-clone` or by regenerating them. See the [replica-base skill](skills/replica-base/SKILL.md).
- **Cargo `target/` clones safely but adds cost.** A cloned `target/` gives a correct incremental no-op build (~20 ms, no recompile) when nothing changed, and recompiles only changed crates after a source touch — it does not force a full rebuild. However, `target/` is created by the setup command, so `.gitignore` does not keep it out of the base; you must exclude it another way (or not build during setup). A shared `CARGO_TARGET_DIR` under `$REPLICA_ROOT/cache` keeps incremental builds fast but reintroduces Cargo's file-lock waiting, so parallel agents can block on `Blocking waiting for file lock`. For parallel use, consider `sccache` instead of a shared target dir.
- **Cost scales with file count.** Each clone writes metadata for every file. Replica is much faster than copying or cloning file by file, but it is not free, and very large trees (hundreds of thousands of files) have not been measured.
- **Same volume only.** Source, base, and workspaces must share one APFS volume.
- **Not a speedup inside a running agent.** Replica makes workspace creation fast. It does not speed up reads or writes during a run, does not reduce the RAM or CPU used by running many agents at once, and does not fix Spotlight or XProtect scanning.
- **macOS only.** Linux and Windows are out of scope.

## Benchmarks

The suite in [`benchmarks/`](benchmarks/README.md) compares `replica new` with
`git clone` plus install, `git worktree` plus install, `pnpm` with the global
virtual store, and `cp -c`, and it also measures `replica rm` and
`replica base refresh`, at concurrency N = 1, 5, and 10.

```sh
make release                                         # target/release/replica
make bench                                           # bundled fixture (~15 min)
make bench CONCURRENCY=1,5,10                        # parallel workspace creation
```

Full results, methodology, machine details, and exact commands are in
[docs/BENCHMARKS.md](docs/BENCHMARKS.md).

Headline (Apple M1 Max, macOS 26.6.2, under load, 1,739-file npm project):

| Concurrency | `replica new` median | `cp -c -R` median | `git clone` + setup median |
|---|---:|---:|---:|
| N = 1 | ~100 ms | ~958 ms | ~3.96 s |
| N = 10 (all at once) | ~230 ms | ~3.55 s | ~4 s |

`replica new` is about 15x faster than `cp -c -R` even with 10 concurrent
workspaces, and tens of times faster than `git clone` plus an install. The
one-time `replica base build` (~2–3 s) is less than a single `git clone` +
setup, so Replica comes out ahead on the very first workspace.

- **2026-11-09, under load** (1-minute load 171–184 on 10 cores). These
  absolute times are inflated by the shared machine load; relative ordering
  held, but do not quote the ratios as settled. Exact numbers and P95 values
  are in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).
- **2026-10-25, quiet machine** (previous harness, 1,842 files): `replica new`
  66 ms, `git clone` + setup 3.10 s, `cp -c` 1.33 s. Different harness and
  fixture; not directly comparable to the 2026-11-09 run.

Replica adds APFS metadata per file per clone (~1.4 MB for a 1,700-file project
on a quiet machine); data blocks are shared with the base and changed files cost
real space as the agent works. The [replica-benchmarks skill](skills/replica-benchmarks/SKILL.md) lists what a clean run needs.

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
