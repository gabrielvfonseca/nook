# Architecture

```
 user / agent
      |
      v
 +-----------+        +-----------------+
 | replica   |------> | help, ui, error |   output, hints, exit codes
 | (main.rs) |        +-----------------+
 +-----+-----+
       |
  +----+-----------+-----------------+----------------+
  |                |                 |                |
 base              workspace         gc               setup
 (lockfile.rs,     (clone.rs,        (gc.rs,          (setup.rs,
  base_check.rs)    store.rs)         volume.rs)       templates/)
  |                |                 |
  git + setup      clonefile(2)      rayon parallel rm (safe_remove)
  |                |                 |
  +------ APFS volume ($REPLICA_ROOT) ------+
                   |
           SQLite (WAL): replica.db
```

There is no daemon. `replica gc` is meant to run on a timer, through launchd or
`brew services`. A daemon with a batched file API is a Phase 4 idea in the
[roadmap](ROADMAP.md).

## Layout (all on one APFS volume)

```
$REPLICA_ROOT/
  base/              golden tree: repo, dependencies, setup output
  base.new/          staging area while a base is built or refreshed
  base.old/          the previous base, briefly, during a swap
  ws/<id>/           one clone per workspace
    .replica/tmp/    TMPDIR for that workspace
  cache/             shared caches: cargo, npm, sccache (not cloned)
  replica.db         workspace and base metadata
```

## Modules

| Module | Role |
|---|---|
| `main.rs` | Clap definitions, root checks, and one function per command |
| `error.rs` | `CliError`: exit code plus hint. Other errors map to exit 1 |
| `ui.rs` | Color, status lines, tables, human units, JSON strings |
| `help.rs` | Help overview and topics. Needs no root |
| `clone.rs` | `clonefile(2)`. Maps EXDEV, ENOSPC, and ENOTSUP to typed errors. Never copies |
| `store.rs` | SQLite schema, migrations, and queries |
| `gc.rs` | `safe_remove`: canonicalize, check the prefix, delete in parallel without following symlinks |
| `volume.rs` | APFS check, same-volume check, `statvfs` usage |
| `lockfile.rs` | Git HEAD, lockfile hashing, staleness |
| `base_check.rs` | Scan for path-dependent files |
| `setup.rs` | Agent detection, config and skill installation, Homebrew formula |
| `util.rs` | TTL parsing, id generation |

## Flows

**new:** check root and APFS → open database → confirm `base/` exists (else exit 2) →
warn or fail on staleness → enforce `REPLICA_MAX_BYTES` (else exit 4) → `clonefile`
→ create `.replica/tmp` → insert row (`active`) → run `--post-clone` → print the path.

**rm:** look up row (unknown → "already removed") → mark `deleting` → `safe_remove`
→ delete row. A failure leaves the row in `deleting` for gc.

**gc:** retry `deleting` rows → remove expired workspaces → if usage is above the
high-water mark, remove least-recently-used workspaces not used in the last 10 minutes,
stopping once usage is below the low-water mark. Each removal is marked `deleting`
first.

**base build:** clone repo into `base.new` → run setup there → record HEAD, lockfile
hash, repo, and setup command → rename `base` to `base.old` → rename `base.new` to
`base` → delete `base.old` → write metadata → run `base check`. Existing clones
are unaffected, because each one owns its blocks.

**base refresh:** read the recorded repo and setup command → run the build flow.

## Why clonefile

A clone shares data blocks copy-on-write, and only modified blocks take new
space. It works only within one APFS volume. That is why the base, workspaces,
and caches all live under `$REPLICA_ROOT`. The clone still writes metadata for
every file, so cost scales with file count. The [benchmarks](BENCHMARKS.md)
measure that cost.

## Why not FUSE or FSKit

A user-space filesystem adds a context switch to every operation, and APFS
already does this work in the kernel.
