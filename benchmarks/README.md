# Benchmarks

This directory measures what Replica claims: creating a fresh, fully installed
project copy is much faster than cloning and installing, and it uses almost no
extra disk. Every number in the README and in [docs/BENCHMARKS.md](../docs/BENCHMARKS.md)
comes from a run of `run.sh`, and the raw data is kept with it.

## Run it

```sh
make release                 # build target/release/replica
make bench                   # fixture project, default run counts (~12 min)
make bench PROJECT=~/code/app SETUP="pnpm install --frozen-lockfile" RUNS=30
```

Or call the script directly:

```sh
PATH="$PWD/target/release:$PATH" benchmarks/run.sh --project DIR --setup CMD --runs 20 --slow-runs 10 --out DIR
```

The script needs `hyperfine`, `jq`, `git`, and a `replica` on `PATH`. Install
the first two with `brew install hyperfine jq`. It needs macOS and at least
2 GB free on the volume that holds `$TMPDIR`. The benchmark will not run on
a machine with less space, because the disk numbers would be meaningless.

## What is measured

| Row | Operation | Notes |
|---|---|---|
| shell baseline | `true` | Cost of hyperfine's shell. Subtract it mentally from the rows below. It is near 0 ms. |
| git clone + setup | `git clone <local repo>` then the setup command | Clones from a local path, so network time is excluded. This favors git. |
| git worktree + setup | `git worktree add --detach` then the setup command | Reuses the existing repo's object store. |
| cp -c -R of prebuilt base | `cp -c -R <base>` | Clonefile copy of an already installed tree. Replica's core operation, without Replica. |
| replica new | `replica new --ttl 1h` | The real command, with the base already built. |
| replica rm | `replica rm <id>` | Removes a workspace created in the untimed prepare step. |
| rm -rf of a cp -c clone | `rm -rf <clone>` | Like-for-like comparison for `replica rm`. |

The base build is timed separately and reported in the header. It runs once,
not per workspace.

## How it stays fair

- **Untimed setup and cleanup.** Each run's `--prepare` step (clearing the old
  clone, creating a workspace to remove) runs before the timer starts. The
  helper is `helpers.sh`.
- **Warm caches.** The npm cache is warmed before the first run, and every
  method uses the same cache. Git and npm do not get a cold-start penalty that
  Replica avoids.
- **Same volume.** Source repo, base, and workspaces all live under one
  `mktemp` directory on one APFS volume. Clonefile cannot cross volumes, so
  the script checks this first.
- **Warmup runs** before measurement: 1 for the install rows, 3 for the cheap rows.
- **Run counts.** Cheap operations use 20 runs. Install operations use 10,
  because each one takes about 20 seconds. P95 is nearest-rank over the runs.
  With fewer than 20 runs, P95 is effectively the maximum, so keep at least 20
  runs when you publish P95.
- **Own scratch area.** The scratch directory is removed on exit, and the
  script removes any workspaces it created first. It never touches your real
  `REPLICA_ROOT`. Use `--keep` to inspect the scratch directory.

## Disk per clone

Disk is sampled differently from time:

- **clonefile methods** (`cp -c`, `replica new`): the change in APFS volume
  usage over 20 clones. Clones share data blocks, so this measures what they
  add: metadata for each file, plus directory records.
- **git clone**: `du` of one clone. Git copies data, nothing is shared, and
  `du` is exact.

The `df` method was validated once with a real-copy control: `cp -R` measured
12.9 MB per clone by `df`, against 11.7 MB by `du`. Other writes to the volume during sampling add noise, so treat differences of
less than about 0.5 MB as zero.

## Output

Each run writes two files to `benchmarks/results/` (or `--out`):

- `<date>-<arch>-macos<version>.md`: the table in the docs, plus the machine and method details.
- `<date>-<arch>-macos<version>.json`: every number in the table, plus the
  hyperfine fields the table is computed from.

To publish a run, copy its table into `docs/BENCHMARKS.md` and the README. The
numbers must match the file exactly. See the `replica-benchmarks` skill.

## Fixture project

`fixtures/npm-app` is a small, pinned npm project: 11 direct dependencies,
39 locked packages, and about 1,700 installed files. Its lockfile is committed,
so the install is reproducible. Changing its dependencies changes every
published number, so record a new run after any change.

## Adding a project

`--project` takes any directory. The script copies it (without `node_modules`
or `.git`), makes a fresh git repo, and runs `--setup` there. Use a setup
command that installs from a lockfile, so every method gets the same tree.
