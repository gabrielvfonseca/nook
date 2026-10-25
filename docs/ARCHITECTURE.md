# Architecture

```
            +-----------+
 agent ---> |  nook CLI | ---- optional ----> nook daemon (Phase 3)
            +-----+-----+                      (scheduled gc, batched fs ops)
                  |
   +--------------+----------------+
   |              |                |
 Base Manager   Workspace Mgr    Reaper (GC)
   |              |                |
   |          clonefile(2)      rayon parallel rm
   |              |                |
   +------- APFS volume (quota) ---+
                  |
             SQLite (WAL) metadata
```

## Layout (all on ONE APFS volume)
```
$NOOK_ROOT/
  base/            golden tree (repo + deps + warm build)
  base.new/        staging during rebuild
  ws/<id>/         one clone per run
    .nook/tmp/     TMPDIR for that run
  cache/           shared: cargo, pnpm/npm, sccache (NOT cloned)
  nook.db          metadata
```

## Components
1. **Base Manager** - `git clone` into `base.new`, runs setup command, atomically
   renames to `base`. Records lockfile hash; stale if lockfile or HEAD changes.
2. **Workspace Manager** - `clonefile(base, ws/<id>)`, inserts row, writes env file.
3. **Reaper** - TTL expiry, then LRU eviction past high-water mark (default 80%)
   down to low-water (default 60%). Marks row `deleting`, removes in parallel, drops row.
4. **Store** - SQLite via `rusqlite`; schema in DATA_MODEL.md.
5. **Stats** - `statvfs` for real free space; per-workspace logical size is advisory only.
6. **Daemon (optional)** - runs gc on a timer; later, batched read/grep/patch API.

## Flows
**new:** load config -> verify base exists + same volume -> `clonefile` -> insert row
(`active`) -> print path.

**gc:** begin txn -> select expired -> mark `deleting` -> commit -> parallel remove ->
delete rows -> check volume % -> if above high-water, repeat with LRU candidates
(skip workspaces touched in last N minutes).

**base build:** clone repo into `base.new` -> run setup -> record hash -> rename
`base` -> `base.old`, `base.new` -> `base`, delete `base.old`. Existing clones are
unaffected (they own their blocks).

## Why clonefile
Clone shares data blocks copy-on-write; only modified blocks consume new space.
It must stay on one volume, which is why the base, workspaces, and quota volume are co-located.

## Why not FUSE/FSKit
User-space filesystem adds context-switch overhead per op and usually loses to APFS.
