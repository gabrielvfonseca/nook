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
| `store.rs` | SQLite schema, migrations, and queries. Opens in WAL mode with a
| 10 s `busy_timeout` so concurrent `new`/`gc` writers serialize instead of
| failing with `SQLITE_BUSY`. |
| `gc.rs` | `safe_remove`: canonicalize, check the prefix, delete in parallel without following symlinks.
| Plus per-workspace lease files (`.replica/lease`) created by `new`, refreshed
| by `touch`, and removed by `rm`; `gc` skips any workspace with a fresh lease
| so it never deletes a workspace an agent is actively using. |
| `volume.rs` | APFS check, same-volume check, `statvfs` usage |
| `lockfile.rs` | Git HEAD, lockfile hashing, staleness |
| `base_check.rs` | Scan for path-dependent files |
| `setup.rs` | Agent detection, config and skill installation, Homebrew formula |
| `util.rs` | TTL parsing, id generation |

## Flows

**new:** check root and APFS → open database → confirm `base/` exists (else exit 2) →
warn or fail on staleness → enforce `REPLICA_MAX_BYTES` (else exit 4) → `clonefile`
→ create `.replica/tmp` → create a lease file marking the workspace in use →
insert row (`active`) → run `--post-clone` → print the path.

**rm:** look up row (unknown → "already removed") → mark `deleting` → `safe_remove`
→ remove lease → delete row. A failure leaves the row in `deleting` for gc.

**gc:** retry `deleting` rows → remove expired workspaces → if usage is above the
high-water mark, remove least-recently-used workspaces not used in the last 10 minutes,
stopping once usage is below the low-water mark. Each removal is marked `deleting`
first. A workspace with a fresh lease file (created by `new`, refreshed by `touch`)
is skipped, so `gc` (including launchd every 10 min) never deletes a workspace
an agent is actively using.

**base build:** clone repo into `base.new` → run setup there → record HEAD, lockfile
hash, repo, and setup command → rename `base` to `base.old` → rename `base.new` to
`base` → delete `base.old` → write metadata → run `base check`. Existing clones
are unaffected, because each one owns its blocks.

**base refresh:** read the recorded repo and setup command → run the build flow.

## Why clonefile

A clone shares data blocks copy-on-write, and only modified blocks take new
space. It works only within one APFS volume. That is why the base, workspaces,
and caches all live under `$REPLICA_ROOT`.

`replica new` clones the base with **one** `clonefile(2)` call on the root of
the tree. Because `src` is a directory, `clonefile` recursively clones the whole
tree atomically in the kernel — it is not a per-file userspace walk like
`cp -c -R`, which clonefiles every file individually. That single call is why
`replica new` takes tens of milliseconds while `cp -c -R` of the same tree takes
over a second (see the [microbenchmark](../benchmarks/microbench.sh)).

The clone still writes one metadata record per file and directory, so cost
scales with file count even though the data blocks are shared. The
[benchmarks](BENCHMARKS.md) measure that cost.

## Why not FUSE or FSKit

A user-space filesystem adds a context switch to every operation, and APFS
already does this work in the kernel.

## Path-dependent artifacts in cloned trees

Some tools bake the absolute path of the build tree into their output. Those
artifacts break when the tree is clonefiled to a new path. `replica base check`
reports each one it finds (exit 0 even when issues exist), and the fix is one
of: a post-clone hook, a rewrite, or excluding the artifact from the base.

| Artifact | Kind detected by `base check` | Fix |
|---|---|---|
| Python venv (`pyvenv.cfg`, `bin/activate`) | `pyvenvcfg`, `venvscript` | Exclude venvs from the base and recreate them in a `--post-clone` hook, or use `virtualenv --copies` so shebangs are relative. |
| `node_modules/.bin` shims | `nodemodulesbin` | Reinstall deps after cloning (`npm ci` / `pnpm install` in `--post-clone`); pnpm handles relocatable shims best. |
| TypeScript `.tsbuildinfo` | `tsbuildinfo` | Exclude `.tsbuildinfo` from the base (add to `.gitignore`) and regenerate after cloning. |
| Cargo `target/` fingerprints | `cargotargetfile` | See below. |

### Rust `target/`

Cargo bakes absolute paths into `target/` (`.d` dep files, `.rlib`, `.rmeta`,
`.fingerprint`). I measured what happens when a tree containing `target/` is
clonefiled to a new path:

- **No-op build** (`cargo build` with no source change): `Finished` in ~20 ms,
  no recompilation — cloned `target/` does **not** force a full rebuild.
- **Incremental rebuild** (touch one source file): Cargo recompiles only the
  changed crate and its dependents, correctly, in the clone.

So cloning `target/` is safe for correctness, but it adds APFS metadata per
clone (one record per file) for no benefit, since a workspace rarely needs the
base's build output. **Recommendation:** exclude `target/` from the base (add to
`.gitignore`), point `CARGO_TARGET_DIR` at a shared directory under
`$REPLICA_ROOT/cache`, and optionally use `sccache`. This keeps clones small
and fast while preserving incremental builds across workspaces.

### Verified by ecosystem

| Ecosystem | `base check` finds | Recommended handling | Verified to clone cleanly |
|---|---|---|---|
| npm / `node_modules` | `.bin` shims with absolute paths | `npm ci` in `--post-clone`, or exclude `node_modules` | Yes, after reinstall |
| pnpm | `.bin` shims (relocatable by design) | `pnpm install --frozen-lockfile` in `--post-clone` | Yes, best of the three |
| Cargo | `target/` fingerprints | Exclude `target/`; shared `CARGO_TARGET_DIR` + sccache | Yes (target/ optional) |
| Python | `pyvenv.cfg`, `bin/activate` | Recreate venv after cloning | Yes, after recreating venv |

