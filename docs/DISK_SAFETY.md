# Disk Safety

Clones start free but grow as builds write output. Layers of defense:

1. **Quota'd volume (recommended).** Create a dedicated APFS volume with a quota:
   ```sh
   # Find your APFS container (usually disk3, but check with `diskutil apfs list`)
   diskutil apfs list
   diskutil apfs addVolume <container> APFS Work -quota 100g
   export NOOK_ROOT=/Volumes/Work/nook
   ```
   Everything (base, ws, cache) lives here. Runaway output fills only this volume.
   Note: APFS volumes share the container's free space; the quota caps this volume.

2. **Size cap without quota (NOOK_MAX_BYTES).** If not using a quota'd volume, set a byte limit:
   ```sh
   export NOOK_ROOT=$HOME/nook
   export NOOK_MAX_BYTES=50000000000  # 50 GB
   ```
   - `nook new` refuses to create a clone when usage ≥ cap, showing current usage.
   - `gc` uses the cap as its effective high-water mark (evicts until below cap).
   - Works on any APFS volume including the main disk.

3. **Startup checks.** On every command, Nook verifies:
   - `$NOOK_ROOT` is on APFS (fails with clear error if not).
   - Base and workspaces share the same volume (fails if cross-volume).

4. **TTL.** Default 2h idle-agnostic expiry; override per workspace.

5. **LRU eviction.** Above 80% volume use (or NOOK_MAX_BYTES cap), evict least-recently-used
   (not touched in last 10 min) until below 60% (or cap).

6. **Shared caches outside workspaces.** Set via `nook env`: `CARGO_HOME`,
   `npm_config_cache`, pnpm store, `SCCACHE_DIR` -> `$NOOK_ROOT/cache`. Caches must
   NOT live in the cloned tree.

7. **Per-workspace TMPDIR.** `ws/<id>/.nook/tmp`, so deleting the workspace deletes temp files.

8. **Honest stats.** `du` and Finder over-count shared blocks; use `nook stats`.

9. **Parallel, symlink-safe delete.** Never traverses symlinks; refuses paths outside `ws/`.

## Caveat
Deleting one clone frees only blocks nobody else shares. Evicting a workspace
may free less than its `du` size suggests.