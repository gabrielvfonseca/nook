# Roadmap

## Done (v0.1)
- **Phase 0, measure.** Benchmark suite in `benchmarks/`, with recorded runs in `benchmarks/results/`.
- **Phase 1, clone core.** `new`, `rm`, `list`, `env`, `touch`, SQLite store, same-volume and APFS checks.
- **Phase 2, safety.** `gc` (TTL, LRU, dry run, byte cap), parallel symlink-safe delete, `stats`, crash recovery for `deleting`.
- **Phase 3, base.** `base build`, `check`, `status`, and `refresh` (refresh fast-path: clonefile + fetch + reset; setup runs only when a lockfile changed).
- **Setup.** `replica setup` for Claude Code, Cursor, Copilot, Zed, Aider, and custom agents. Installs the SKILL.md skill.
- **CLI.** `replica help` with topics, color and `NO_COLOR`, `--json` on list, stats, and base status, typed exit codes 1 to 4.
- **Distribution.** GitHub releases with universal binaries and an installer script. Homebrew is not available yet (see HOMEBREW.md).

## Next
- **Orphan sweep.** Detect directories under `ws/` with no database row and remove them in `gc`.
- **Concurrency.** Make `new` and `gc` safe to run at once. Today each SQL statement is atomic, but a `gc` pass and a `new` can interleave. Wrap multi-step changes in transactions.
- **Workspace cleanup on hook failure.** Currently a failed `--post-clone` keeps the workspace on purpose. Offer a flag to remove it instead.
- **Linux reflinks.** Reflink support on btrfs and XFS, if there is demand.

## Phase 4, optional speedups
- Batched read, glob, grep, and patch API over a Unix socket.
- FSEvents-backed cache, and an in-memory write overlay.
- Scheduled gc daemon. `brew services` and launchd cover this for now.

## Open questions
- One base per lockfile for monorepos?
- Are warm build artifacts (Cargo `target/`) safe to clone? Path-dependent fingerprints say no, so the base only fetches dependencies.
- How much metadata per clone is acceptable on a volume with millions of files?
