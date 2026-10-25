# Contributing
- `make check` (fmt + clippy + tests) must pass. Tests need macOS/APFS for the clone tests; they skip if clonefile is unsupported.
- Deletion code is safety-critical: any change to `gc::safe_remove` needs a test.
- No performance claims without numbers from `scripts/bench.sh` recorded in `docs/BENCHMARKS.md`.
- Follow `AGENTS.md` if you're a coding agent.
