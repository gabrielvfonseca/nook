# AGENTS.md

Guidance for coding agents implementing Nook.

## Ground rules
- Rust stable, macOS (aarch64 + x86_64) only. Gate with `#[cfg(target_os = "macos")]`.
- Never delete outside `$NOOK_ROOT/ws/`. Every delete path must be canonicalized and
  checked to be a child of that directory first.
- Never follow symlinks during deletion (`symlink_metadata`, `remove_file` on links).
- No `unwrap()` in non-test code. Use `anyhow::Result`.
- No claims of performance without a number from `docs/BENCHMARKS.md`.

## Build order
1. `clone.rs` + `nook new` (Phase 1 in ROADMAP)
2. `store.rs` + `list`/`rm`
3. `gc.rs` (parallel delete, TTL, LRU)
4. `base build`
5. `stats`

## Tests
- Unit: TTL parsing, path-safety check, LRU ordering.
- Integration: create temp APFS sparse image (`hdiutil create -type SPARSE -fs APFS`),
  run new/rm/gc against it.
- Cross-volume clone must fail with a clear error, not fall back to copy silently.

## Definition of done per command
Documented in docs/CLI.md, tested, and exits non-zero with a human-readable message on failure.
