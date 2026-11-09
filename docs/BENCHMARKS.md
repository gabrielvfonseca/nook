# Benchmarks

Replica's claims are speed (creating a fully installed workspace) and disk use
(what each workspace adds). This file records what was measured, how, and what
is still pending. The suite lives in [`benchmarks/`](../benchmarks/README.md).
The rule from `AGENTS.md` applies: no performance claim without a number from a
recorded run.

## Status

| | |
|---|---|
| Suite | `benchmarks/run.sh`, with rows for every method plus `replica base refresh` |
| Latest valid run | 2026-10-25, arm64 macOS 26.6.2, **made with the previous harness**. Kept below, with corrections |
| Pending | A run of the current suite. It refuses to start while the 1-minute load average is above a quarter of the cores. On 2026-11-09 the load was 60 to 150, from long-running agent sessions (`kilo`, `opencode`) on the same machine. Those timings were discarded. |

To publish the current suite: close other heavy work, check `uptime`, then run `make bench`. The harness records the load average at start and end in its report, so a run can be checked afterwards.

## Methodology

Every method creates the same thing: a fresh copy of a project with its
dependencies installed.

| Row | What is timed |
|---|---|
| shell baseline | `true`, to show hyperfine's shell overhead |
| git clone + setup | `git clone` from a local path, then `npm ci` |
| git worktree + setup | `git worktree add --detach`, then `npm ci` |
| cp -c -R of prebuilt base | `cp -c -R` (clonefile) of the built base |
| replica new | `replica new --ttl 1h` |
| replica rm | `replica rm <id>` of a workspace created in an untimed prepare step |
| rm -rf of a cp -c clone | `rm -rf` of a clonefile copy, to compare with `replica rm` |

- **Untimed prepare and cleanup.** Each run's setup and teardown happen outside the timer.
- **Warm npm cache**, shared by all methods. The base build is timed separately (one time).
- **One APFS volume** for source, base, and workspaces.
- **Run counts.** 20 runs for cheap operations, 10 for installs, with warmup runs first. P95 is nearest-rank over the runs.
- **Disk per clone.** For clonefile methods, the change in volume usage over 20 clones (`df`). For git, `du` of one clone, because git copies data and nothing is shared.

### Fixture

[`benchmarks/fixtures/npm-app`](../benchmarks/fixtures/npm-app): 11 direct
dependencies, 39 locked packages, about 1,700 installed files and 11 MB on disk.
Its lockfile is committed.

### Validation of the disk method

A control run on 2026-11-09 (same machine, same fixture):

| Method | `df` delta per clone | Expected |
|---|---|---|
| real copy (`cp -R`) | 12.9 MB | 11.7 MB, which is `du` of one copy |
| `cp -c -R` (clonefile) | 1.4 MB | metadata only, data blocks shared |
| `replica new` (clonefile) | 1.4 MB | same as `cp -c`, because it uses the same syscall |

So a clone is not free. It adds APFS metadata for each file and directory, about
1.4 MB for this fixture. Its data blocks are shared with the base until a file
changes.

## Corrections to the previous run

The 2026-10-25 run used an earlier harness. Four things need correcting:

1. **`replica rm` row.** It did not time one removal. The command ran `replica gc --dry-run`, then `replica list`, then up to five `replica rm`s on whatever happened to be left from the `replica new` row. Its 22 ms median is not a per-workspace removal time, so do not cite it.
2. **"About 0 MB" per clone.** This claim was not supported by the old run. Its disk column was not a measured `df` delta, and the 2026-11-09 control measured about 1.4 MB of APFS metadata per clone for this fixture.
3. **`replica new` timing.** The 66 ms median is the same command the current suite times. It stands as recorded.
4. **P95.** The old script used a floor-based index, not nearest-rank. With 20 or more runs, the difference is small but real, so do not compare old and new P95 values directly. The two can differ by one run.

## Results

### 2026-10-25 on arm64 macOS 26.6.2 (25G83), previous harness, superseded in part

Fixture: a 1,842-file npm project with 70 dependencies, on a 1 TB APFS volume.
Runs: 3 per method. Corrections above apply to the `replica rm` row and the disk column.

| Method | Median (s) | P95 (s) | Files | Disk/clone (as recorded) |
|---|---|---|---|---|
| git clone + setup | 3.10 | 3.19 | 1842 | ~33 MB (du, superseded) |
| git worktree + setup | 2.51 | 2.61 | 1842 | ~33 MB (du, superseded) |
| cp -c -R | 1.33 | 1.37 | 1842 | ~33 MB (du, superseded) |
| replica new | 0.066 | 0.075 | 1842 | ~0 MB (superseded: about 1.4 MB, see above) |
| replica rm | 0.022 | 0.721 | N/A | N/A (superseded: not a single removal, see above) |

- `replica new` at 66 ms was 47x faster than `git clone` plus `npm ci` (3.10 s), and 20x faster than `cp -c` (1.33 s).
- The `cp -c` result was 2.3x faster than the git clone, so most of the git clone's time is the install, not the copy.
- The base build (one time) is not part of these numbers.

Add new runs above this section, newest first.
