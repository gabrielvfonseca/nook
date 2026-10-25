# Roadmap

## Phase 0 - Measure
Run BENCHMARKS.md baselines. Go/no-go.

## Phase 1 - Clone core
`nook new`, `rm`, `list`, SQLite store, same-volume check.

## Phase 2 - Safety
`gc` (TTL + LRU), parallel delete, `stats`, `env`, orphan recovery.

## Phase 3 - Base + daemon
`base build/status`, atomic swap, scheduled gc daemon (launchd plist).

## Phase 4 - Optional speedups
Batched read/glob/grep/patch API over a Unix socket; FSEvents-backed cache; in-memory write overlay.

## Open questions
- Detect per-lockfile multiple bases (monorepo)?
- Warm build artifacts (Cargo `target/`) safe to clone? (path-dependent fingerprints)
- Support Linux reflinks (btrfs/xfs) later?
