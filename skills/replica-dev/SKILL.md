---
name: replica-dev
description: Contributor guide for working on the replica Rust codebase. Covers module layout, the ground rules in AGENTS.md (path safety, no unwrap in non-test code, anyhow errors, macOS-only), CLI output conventions (stdout is for data, stderr for status, typed exit codes), the test layers and how to run them, and the docs that must change with any CLI change. Use when editing src/, adding or changing a command, writing tests, or reviewing a change to replica.
---

# Replica development

Replica is a macOS-only Rust CLI. It clones an APFS base tree with
`clonefile(2)` and manages the resulting workspaces. Read `AGENTS.md` at the
repo root first. Its rules are binding, and this skill expands on them.

## Map of the code

| File | Responsibility |
|---|---|
| `src/main.rs` | Clap definitions and command dispatch. `run()` checks the root, opens SQLite, and routes each command |
| `src/error.rs` | `CliError` with an exit code and optional hint. `exit_code()` maps any `anyhow::Error` |
| `src/ui.rs` | Colors (TTY and `NO_COLOR` aware), status lines, aligned tables, human units, JSON string escaping |
| `src/help.rs` | `replica help [topic]` text. Works without a root volume |
| `src/clone.rs` | `clonefile(2)` wrapper. Never falls back to copying |
| `src/store.rs` | SQLite schema (`workspaces`, `base`), migrations, queries |
| `src/gc.rs` | `safe_remove`: canonicalized, symlink-safe, parallel delete |
| `src/volume.rs` | APFS check, same-volume check, `statvfs` usage |
| `src/lockfile.rs` | Git HEAD and lockfile hashing, staleness |
| `src/base_check.rs` | Scans the base for path-dependent files |
| `src/setup.rs` | Agent detection, config and skill generation, Homebrew formula |
| `src/util.rs` | TTL parsing, workspace ids |
| `templates/` | Agent config templates embedded with `include_str!` |
| `skills/` | Agent skills (`SKILL.md`), installable with the skills.sh CLI |
| `scripts/` | `install.sh`, `uninstall.sh`, `release.sh`, `setup-volume.sh` |
| `benchmarks/` | `run.sh` benchmark suite, `helpers.sh`, pinned fixture, recorded results. See the `replica-benchmarks` skill |

## Ground rules (from AGENTS.md)

- **Target**: Rust stable, macOS on aarch64 and x86_64 only. Keep the `#[cfg(target_os = "macos")]` gate in `lib.rs`.
- **Path safety**: never delete outside `$REPLICA_ROOT/ws/`. Every delete goes through `gc::safe_remove`, which canonicalizes the path and checks it is a child of the workspace root. Never follow symlinks while deleting (`symlink_metadata`, `remove_file` on links).
- **No `unwrap()` or `expect()` in non-test code.** Use `?` with `anyhow::Result`, or a `CliError`. Test code may unwrap.
- **No performance claims without a number from `docs/BENCHMARKS.md`.** If you change a hot path, measure it with the benchmark harness, then update that file.
- **Cross-volume clones must fail loudly.** `clone_tree` never falls back to a copy.

## Output conventions

These keep Replica scriptable, and they are part of the public contract.

- **stdout is data only.** `replica new` prints only the workspace path, so `cd "$(replica new)"` works. `list`, `stats`, `base status`, and `env` print data. Use `--json` when there is structure.
- **stderr is for people.** Status lines go through `ui::status(Status::Ok|Warn|Err|Info, msg)`. Use `ui::detail` for indented follow-up lines. Keep the `hint:` convention for fixes.
- **Errors are typed.** Return `CliError::new(code, msg).with_hint(...)` when the exit code matters. Exit codes are 1 generic, 2 no base or stale with `--require-fresh`, 3 cross-volume, 4 disk full or cap. Document any new code in `help.rs` (`exit-codes` topic) and `docs/CLI.md`.
- **Color** only when stderr or stdout is a TTY and `NO_COLOR` is unset. Tests run with `NO_COLOR=1`.
- **Human units** come from `ui::human_bytes`, `ui::human_duration`, and `ui::usage_bar`. Don't write new formatters.

## Adding or changing a command

1. Add the variant to `Cmd` (or `BaseCmd`) in `src/main.rs`, with a doc comment (it becomes `--help`) and a `display_order`.
2. Put logic in a module if it is more than a few lines. Keep `main.rs` as dispatch and output.
3. Route state changes through `store::` and deletes through `gc::safe_remove`.
4. Add an integration test in `tests/cli.rs`. Use `REPLICA_ROOT` set to a tempdir, and set `NO_COLOR=1`.
5. Update every doc that names the command. The list is at the end of this skill.

## Tests and checks

| Layer | Where | Run |
|---|---|---|
| Unit | `#[cfg(test)]` in each `src/` module | `cargo test --lib` |
| Integration (CLI) | `tests/cli.rs` (binary via `CARGO_BIN_EXE_replica`) | `cargo test --test cli` |
| Integration (clonefile) | `tests/clone.rs` (skips on non-APFS) | `cargo test --test clone` |
| Base check | `tests/base_check.rs` | `cargo test --test base_check` |
| Setup | `tests/setup.rs` | `cargo test --test setup` |

Before you finish, run `make check`. It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`. CI runs the same on macOS.

Manual smoke test on a real APFS volume:

```sh
export REPLICA_ROOT=$(mktemp -d)
replica base build --repo <local repo> --setup "true"
replica new --name smoke && replica list && replica gc --dry-run
```

Use `$(mktemp -d)` for `REPLICA_ROOT`, not `/tmp`. Both are on the boot APFS
volume, but `mktemp` keeps the run isolated.

## Docs that must change with the CLI

A CLI change is incomplete until these match:

- `src/help.rs`: overview and topic text
- `docs/CLI.md`: command reference and exit codes
- `skills/replica/SKILL.md`: the agent-facing command table
- `README.md`: the quick start and command summary
- `templates/claude.md` and the other `templates/` files, if they name the command
- `CHANGELOG.md`: an entry under the unreleased version

Do not claim a feature is implemented in a doc until a test covers it.

## Gotchas

- `REPLICA_ROOT` defaults to `/Volumes/Work/replica`. Most commands fail early if it does not exist. `help` and `--version` do not need it.
- `replica new` and `base` commands check that `base` and `ws/` are on one volume. The check runs only when `base/` exists.
- The SQLite database is in WAL mode. Migrations go in `store::open` and must be idempotent.
- Benchmarks need real free space. A small boot volume can make results and runs fail, so check `df -h` first.
