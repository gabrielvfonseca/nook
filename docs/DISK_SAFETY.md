# Disk Safety

Clones start free but grow as builds write output. Layers of defense:

1. **Quota'd volume.** `diskutil apfs addVolume disk3 APFS Work -quota 100g`.
   Everything (base, ws, cache) lives here. Runaway output fills only this volume.
   Note: APFS volumes share the container's free space; the quota caps this volume.
2. **TTL.** Default 2h idle-agnostic expiry; override per workspace.
3. **LRU eviction.** Above 80% volume use, evict least-recently-used (not touched in
   last 10 min) until below 60%.
4. **Shared caches outside workspaces.** Set via `nook env`: `CARGO_HOME`,
   `npm_config_cache`, pnpm store, `SCCACHE_DIR` -> `$NOOK_ROOT/cache`. Caches must
   NOT live in the cloned tree.
5. **Per-workspace TMPDIR.** `ws/<id>/.nook/tmp`, so deleting the workspace deletes temp files.
6. **Honest stats.** `du` and Finder over-count shared blocks; use `nook stats`.
7. **Parallel, symlink-safe delete.** Never traverses symlinks; refuses paths outside `ws/`.

## Caveat
Deleting one clone frees only blocks nobody else shares. Evicting a workspace
may free less than its `du` size suggests.
