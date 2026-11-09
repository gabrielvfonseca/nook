# Disk safety

A clone starts out free, but it grows as builds and tests write output. Replica
bounds disk use in layers.

1. **Dedicated, quota'd volume (recommended).** Put everything (base, workspaces, caches) on its own APFS volume with a quota. Runaway output then fills only that volume.
   ```sh
   diskutil apfs list                                   # find the container
   diskutil apfs addVolume <container> APFS Work -quota 100g
   export REPLICA_ROOT=/Volumes/Work/replica
   ```
   APFS volumes in one container share its free space. The quota caps this volume only.

2. **Byte cap without a quota (`REPLICA_MAX_BYTES`).** For any APFS volume, including the boot disk:
   ```sh
   export REPLICA_ROOT=$HOME/replica
   export REPLICA_MAX_BYTES=50000000000               # 50 GB
   ```
   - `replica new` refuses to clone once the volume's used bytes reach the cap. It exits 4 and reports usage.
   - `gc` uses the cap as its high-water mark when it is lower than `--high`.

3. **Startup checks.** Every command except `help` checks that:
   - `$REPLICA_ROOT` exists and is on APFS.
   - The base and `ws/` are on the same volume (checked when the base exists).

4. **TTL.** Each workspace expires after its `--ttl` (default 2h), whether or not it is in use. Use `replica touch` to keep one alive for LRU purposes. Touching does not extend the TTL.

5. **LRU eviction.** When usage is above `--high` (default 80%, or the cap's equivalent), gc removes the least recently used workspaces, skipping any used in the last 10 minutes. It stops once usage is below `--low` (default 60%).

6. **Shared caches outside workspaces.** `replica env` points `CARGO_HOME`, `npm_config_cache`, and `SCCACHE_DIR` at `$REPLICA_ROOT/cache`, so caches are shared and never copied into a workspace. Caches are not cloned with the base.

7. **Per-workspace `TMPDIR`.** `ws/<id>/.replica/tmp`. Deleting the workspace deletes its temp files too.

8. **Honest accounting.** `du` and Finder count shared blocks once per file, so they overstate clone sizes. Use `replica stats`, which reads `statvfs`.

9. **Symlink-safe, parallel delete.** Symlinks are removed as links and never followed. The path must be a child of `ws/`, and the root itself is refused.

## What deleting frees

Deleting a workspace frees only the blocks no other file shares. Right after a
clone, that is almost nothing, because the clone's data blocks are the base's.
Freed space grows as the workspace diverges from the base. Evicting a workspace
may free much less than its `du` size suggests.

The per-clone metadata cost is real, though. Each clone adds an inode record
for every file and directory it contains. For the benchmark fixture that is
about 1.4 MB for 1,700 files. See [BENCHMARKS](BENCHMARKS.md) for measurements.
