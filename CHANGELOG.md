# Changelog

## 0.1.0 - Unreleased

### Added
- `replica help [topic]`: a guided overview, with topics quickstart, workspaces, base, safety, agents, env, and exit-codes. It works before `REPLICA_ROOT` exists.
- Clear command output: status lines (`✓ ! ✗ ·`) on stderr, aligned tables for `list`, a usage bar for `stats`, and `hint:` lines on errors. Color is used only on a TTY, and `NO_COLOR` turns it off.
- `--json` for `list`, `stats`, and `base status`.
- Typed exit codes: 2 (no base or stale with `--require-fresh`), 3 (cross-volume), 4 (disk full or byte cap). They are used consistently and documented.
- `replica base refresh` works. It rebuilds from the repo and setup command recorded at build time.
- `replica env` and `replica touch` reject unknown ids with exit 1.
- Benchmark suite in `benchmarks/`: hyperfine-based, with untimed setup and cleanup, a pinned fixture project, and a machine-readable report. `make bench` runs it.
- Agent skills in `skills/`, in SKILL.md format for the skills.sh CLI: `replica`, `replica-base`, `replica-maintenance`, `replica-dev`, `replica-benchmarks`, `replica-release`.
- `make help`, `make bench`, and `make skills-check`.
- Homebrew tap formula and `scripts/release.sh` for rendering it after a release.

### Changed
- `replica setup` installs the `SKILL.md` skill, replacing `skill.toml` and `instructions.md`. It no longer writes the placeholder `skills.sh`.
- `replica setup` no longer duplicates appended configs when run again.
- `replica setup` keeps existing replaced configs unless `--force` is given. `--force` always backs up the original first.
- Base metadata records the repo and setup command, and the database migrates to add them.
- Benchmark docs now state what was measured. Clone disk cost is about 1.4 MB of APFS metadata per clone, not zero.
- Default GitHub owner is `gabrielvfonseca`, matching the git remote.

### Fixed
- A missing `REPLICA_ROOT` produces a clear error with a hint, not a raw `statfs` failure.
- Exit codes match the documentation. Code 4 was documented but never returned.
- `replica rm` says when a workspace is already gone. It keeps going past a failed removal, reports it, and exits 1 at the end.
- Removed an `unwrap` from `replica base check`'s `pyvenv.cfg` parsing, so non-test code follows the no-unwrap rule in AGENTS.md.

### Notes
- Not yet benchmarked on an idle machine with the current suite. See docs/BENCHMARKS.md.
