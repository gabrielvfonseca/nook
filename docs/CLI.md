# CLI Reference

Env: `NOOK_ROOT` (default `/Volumes/Work/nook`).

## nook base build --repo <url|path> --setup <cmd>
Build the golden tree. Atomic swap. Fails if setup exits non-zero. Runs `base check` automatically after building. Records git HEAD and lockfile hash.

## nook base check
Scan the golden tree for absolute paths that will break when cloned. Reports findings with suggestions.

## nook base refresh
Rebuild the base side-by-side in `base.new`, then atomically swap. Existing clones keep working (they own their blocks). Old base moved to `base.old` and cleaned up by `gc` once no clones reference it.

## nook base status
Prints base age, HEAD, lockfile hash, stale yes/no (compares current git HEAD and lockfiles to recorded values).

## nook new [--ttl 2h] [--name label] [--post-clone <cmd>] [--require-fresh]
Clone base. TTL accepts `30m`, `2h`, `1d`. Prints workspace path on stdout (only); logs go to stderr so `cd $(nook new)` works. Optional `--post-clone` runs a shell command in the new workspace after cloning (for project-specific fixups like recreating a venv). `--require-fresh` fails if base is stale (behind remote or lockfiles changed).

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