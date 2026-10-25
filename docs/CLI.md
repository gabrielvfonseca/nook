# CLI Reference

Env: `NOOK_ROOT` (default `/Volumes/Work/nook`).

## nook base build --repo <url|path> --setup <cmd>
Build the golden tree. Atomic swap. Fails if setup exits non-zero.

## nook base status
Prints base age, HEAD, lockfile hash, stale yes/no.

## nook new [--ttl 2h] [--name label]
Clone base. TTL accepts `30m`, `2h`, `1d`. Prints workspace path on stdout (only);
logs go to stderr so `cd $(nook new)` works.

## nook env <id>
Prints `export` lines: `NOOK_WS`, `TMPDIR`, cache dirs.

## nook list [--json]
id, name, age, last used, ttl remaining, state.

## nook touch <id>
Update `last_used` (call from agent harness to avoid LRU eviction).

## nook rm <id>...
Parallel delete. Idempotent.

## nook gc [--dry-run]
Expire by TTL, then evict LRU if volume above high-water.

## nook stats
Volume total/free/used %, workspace count, oldest/newest. Uses `statvfs`.

## Exit codes
0 ok, 1 generic error, 2 base missing/stale-and-strict, 3 cross-volume, 4 disk full.
