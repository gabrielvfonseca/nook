# Contributing

- `make check` (fmt, clippy with warnings as errors, and tests) must pass. Run `make help` to see every target.
- The clone tests need macOS on APFS. They skip themselves if clonefile is unsupported.
- Deletion is safety-critical. Any change to `gc::safe_remove` needs a test, and the rules in AGENTS.md apply.
- CLI changes update `src/help.rs`, `docs/CLI.md`, `skills/replica/SKILL.md`, and the README together. See the `replica-dev` skill for the full list.
- No performance claims without a number from a recorded run in `benchmarks/results/`, cited in `docs/BENCHMARKS.md`. Run `make bench`.
- Follow `AGENTS.md` if you are a coding agent.
