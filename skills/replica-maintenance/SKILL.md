---
name: replica-maintenance
description: Keep Replica's disk use bounded. Covers `replica gc` (TTL expiry, LRU eviction, dry runs), `replica stats`, the REPLICA_MAX_BYTES byte cap, dedicated APFS volumes with quotas, the launchd GC agent and Homebrew service, and recovering from crashed deletes. Use when the disk is filling up, when gc removes or keeps the wrong workspaces, when setting up automatic cleanup, or when a workspace is stuck in the deleting state.
---

# Replica maintenance: disk limits and garbage collection

Replica bounds its disk use three ways. Know which one applies before you
change anything.

1. **TTL**: every workspace expires after `--ttl` (default 2h).
2. **LRU eviction**: when the volume is above `--high` (default 80%), gc removes the least recently used workspaces until usage is below `--low` (default 60%).
3. **Byte cap**: `REPLICA_MAX_BYTES` makes `replica new` refuse to create a workspace past that many bytes, and it also lowers the effective high-water mark.

## Check the state first

```sh
replica stats              # usage bar, free space, active and expired counts, base age
replica stats --json       # same, for scripts
replica list               # per-workspace age, last use, and time left
replica gc --dry-run       # what gc would remove, without removing it
```

## What gc does, in order

1. Retries any workspace left in the `deleting` state by an earlier crash.
2. Removes every workspace past its TTL. This always happens.
3. If the volume is above `--high`, it considers LRU victims, oldest first. It skips any workspace used in the last 10 minutes.
4. It removes LRU victims only while usage is above `--low`.

So a full disk with no expired workspaces and every workspace used recently
stays full. That is intended. Run `replica touch <id>` on sessions you still
need, or `replica rm <id>` on ones you do not.

```sh
replica gc                       # defaults: high 80%, low 60%
replica gc --high 70 --low 50    # evict earlier and go lower
```

Every removed workspace is printed with its reason (`expired` or `evicted for space`).

## Set up a bounded volume

Recommended: a dedicated APFS volume with a quota, so clones can never fill
the boot disk.

```sh
diskutil apfs list                                   # find the container
diskutil apfs addVolume <container> APFS Work -quota 100g
export REPLICA_ROOT=/Volumes/Work/replica
```

Or use `./scripts/setup-volume.sh <container> 100g Work`.

Alternative, on any APFS volume with a byte cap:

```sh
export REPLICA_ROOT=$HOME/replica
export REPLICA_MAX_BYTES=50000000000                 # 50 GB
```

The trade-offs are in [docs/DISK_SAFETY.md](../../docs/DISK_SAFETY.md).

## Automatic gc

Run gc every 10 minutes with either:

```sh
INSTALL_LAUNCHD=1 REPLICA_ROOT=/Volumes/Work/replica make install   # launchd agent
brew services start replica                                         # Homebrew service
```

The launchd agent is `dev.replica.gc` (`packaging/dev.replica.gc.plist`). Its
log is `/tmp/replica-gc.log`. Check it with `launchctl list | grep replica`.
Remove it with `./scripts/uninstall.sh`, or `launchctl unload` followed by
deleting the plist.

## Recovering from a crash

A workspace goes to `deleting` before its files are removed. If the process
dies in between, the next `replica gc` finishes the job. Nothing else is
needed. To check for stuck entries, look at the database directly:

```sh
sqlite3 "$REPLICA_ROOT/replica.db" "SELECT id, path FROM workspaces WHERE state='deleting';"
replica gc                                           # retries them
```

## Safety guarantees

- Deletion only touches paths under `$REPLICA_ROOT/ws/`. The path is canonicalized and checked before anything is removed. The root itself is refused.
- Symlinks are removed as links and never followed, so a link into your home directory is safe.
- If a removal fails, the workspace stays `deleting` and gc retries it. `rm` reports the failure and exits 1.

## Troubleshooting

- **gc removes nothing while the disk is full**: every workspace is recent. Use `replica list`, then `replica rm` or `replica touch` as needed.
- **`disk cap reached` (exit 4)**: gc did not free enough, or the cap is too low. Compare `replica stats` with `REPLICA_MAX_BYTES`.
- **`REPLICA_ROOT does not exist`**: the volume is not mounted. Mount it, or create the directory, then retry.
- **The disk is full even though `replica stats` looks fine**: other data is on the volume. `replica stats` counts the whole volume, so check `du -sh "$REPLICA_ROOT"` and `tmutil listlocalsnapshots /` for snapshots.
