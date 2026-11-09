# Benchmarks

Replica's claims are speed (creating a fully installed workspace) and disk use
(what each workspace adds). This file records what was measured, how, and what
is still pending. The suite lives in [`benchmarks/`](../benchmarks/README.md).
The rule from `AGENTS.md` applies: no performance claim without a number from a
recorded run.

## Status

| | |
|---|---|
| Suite | `benchmarks/run.sh`: every method, plus `replica base refresh` |
| Latest run | 2026-11-09, **under heavy background load** (1-minute load 171 at start, 184 at end, on 10 cores). Run with `--allow-busy`. Treat the absolute times as inflated and the disk figures as noisy. |
| Clean reference | 2026-10-25, previous harness, measured on a quiet machine. Kept below, with corrections. |

The harness refuses to start while the 1-minute load exceeds a quarter of the
cores. The 2026-11-09 run overrode that guard on purpose, and its report records
the load so the caveat travels with the numbers. A clean run needs an idle
machine: close other heavy work, then `make bench`.

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

### 2026-11-09 on Apple M1 Max, macOS 26.6.2 (25G83), **under load**

Load: 1-minute average 171 at start and 184 at end on 10 cores. Other user
sessions were running. Source: `benchmarks/results/2026-11-09-arm64-macos26.6.2.{md,json}`.
Fixture: 39 locked packages, 1,739 files after setup, 11 MB installed.

| Method | Runs | Median (ms) | P95 (ms) | Min–max (ms) | Extra disk per clone |
|---|---|---|---|---|---|
| shell baseline (`true`) | 20 | 0 | 8 | 0–14 | – |
| git clone + setup | 10 | 3898 | 4904 | 3090–4904 | 11.5 MB (du, not shared) |
| git worktree + setup | 10 | 3074 | 4073 | 2553–4073 | n/a (shares the object store) |
| cp -c -R of prebuilt base | 20 | 1262 | 2080 | 1085–2108 | 7.9 MB (df, noisy) |
| **replica new** | 20 | **78** | 135 | 62–167 | 2.2 MB (df, noisy) |
| replica rm | 20 | 235 | 272 | 185–276 | – |
| rm -rf of a cp -c clone | 20 | 398 | 486 | 361–514 | – |
| replica base refresh (no changes) | 10 | 534 | 628 | 451–628 | – |

What this run supports, with the load caveat:

- Relative ordering held: `replica new` (78 ms) was the fastest creation method, ahead of `cp -c` (1,262 ms) and both git methods (about 3–4 s).
- `replica rm` (235 ms) was faster than `rm -rf` of an equivalent clonefile copy (398 ms). The two were measured in the same conditions.
- `replica base refresh` with no upstream changes took about half a second. It fetches and resets without re-running setup. This is the common case.
- Under load every absolute number is larger than in the clean October run. Do not compare these times to the October table, and do not quote the ratios as settled.
- Disk figures varied from the clean control. The 2026-11-09 control measured about 1.4 MB of metadata per clone with no load. Under this load the `df` deltas read 2.2 MB and 7.9 MB, which is noise from other writes on the volume. Use the clean control for the metadata cost.

### 2026-10-25 on arm64 macOS 26.6.2 (25G83), previous harness, quiet machine, superseded in part

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
