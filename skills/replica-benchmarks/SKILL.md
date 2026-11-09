---
name: replica-benchmarks
description: Run Replica's benchmark suite and publish the results accurately. Use when a change could affect speed or disk use (the clone path, delete path, base handling), when someone asks for benchmark numbers, when updating the performance claims in README.md or docs/BENCHMARKS.md, or when adding a new project to benchmark. Enforces the rule that performance claims need a number from a recorded run.
---

# Replica benchmarks

Replica's central claim is speed and near-zero disk for each workspace. The
rule in `AGENTS.md`: no performance claim without a number from a recorded
run in `benchmarks/results/`. This skill covers how to produce that number and
how to publish it without drifting.

## Run

A clean run needs an idle machine. The harness refuses to start above a quarter of the cores in 1-minute load; `--allow-busy` overrides that and must be stated with any numbers published.

```sh
make release
make bench                                        # fixture project, 20 runs (10 for installs)
make bench PROJECT=/path/to/app SETUP="npm ci" RUNS=30
```

Expect about 12 minutes for the fixture. Each run writes
`benchmarks/results/<date>-<arch>-macos<version>.md` and `.json`.

Before you trust a run, check:

1. **Idle machine.** Close builds, sync jobs, and indexing. Spotlight runs in the background, and `replica new` writes many files. Look at `uptime` and `top -l 1 | head -12` first. If the 1-minute load average is above the core count, rerun later.
2. **Free space.** The script refuses to run with under 2 GB free. Results are only comparable when the volume has room, so note the free space if it is low.
3. **Replica is the build you mean.** `make bench` uses `target/release/replica` only if it is first on `PATH`. The script prints `replica --version` and the path in the report. Check them.
4. **Same tree every time.** Do not edit the project during a run.

## Read the numbers correctly

- **Shell baseline row**: the cost of hyperfine's shell. It should be near 0 ms. A high value means the machine is loaded.
- **Median vs P95**: use the median for the headline and report P95 for spread. P95 needs at least 20 runs to mean anything.
- **Extra disk per clone**: `cp -c` and `replica new` add APFS metadata, not data. The fixture's clone costs about 1.4 MB, and the install's data is about 11 MB. The docs must say "about 1.4 MB of metadata for ~1,700 files", not "0 MB".
- **git clone disk** is `du` of one clone (no sharing). The others are `df` deltas over 20 clones. They are not the same measurement, so do not compare them as if they were.
- **Base build time** is one-time and excluded from the table.

## Publish a run

Only publish numbers from a single run in `benchmarks/results/`. Do not mix
numbers from two runs in one table, and do not hand-edit values.

1. Copy the table from the `.md` file into `docs/BENCHMARKS.md` under a new dated `## Results` section. Keep the older sections, because they show how the numbers changed.
2. Update the README's benchmark table and its speedup sentence. Compute speedups from the medians: `git clone + setup median / replica new median`. Round the same way every time.
3. Update every claim that cites a number: README, `docs/BENCHMARKS.md`, `docs/SPEC.md` if it has targets, and `CHANGELOG.md`.
4. Search for stale numbers after you publish: `rg "47x|66ms|3\.10s|about 0 MB|~0 MB" README.md docs/ CHANGELOG.md`.

## Interpreting a regression

If `replica new` or `replica rm` got slower after a change:

1. Rerun the suite twice to rule out noise. Two runs within 10% of each other is a real signal.
2. Compare the `cp -c -R` row. If it moved too, the cause is the system, not Replica.
3. Profile before you optimize: `sample <pid> 5` while a loop of `replica new` runs, or `sudo fs_usage -w -f filesys replica`.
4. Record the new run and explain the change in `CHANGELOG.md`.

## Adding a project

`--project` accepts any directory. The script copies it without `node_modules` or `.git`, makes a fresh git repo, and runs `--setup` in each clone. Prefer a setup command that installs from a lockfile. Commit a lockfile-pinned fixture under `benchmarks/fixtures/` if you want the result to be reproducible, and say which project a published number came from.

## Do not

- Do not claim a speedup from a single run, from a cold cache, or from a different project than the one you cite.
- Do not run the suite against a `REPLICA_ROOT` you care about. The script uses its own scratch directory, but do not point it at shared state.
- Do not change the fixture's dependencies without publishing a new baseline. Changing them changes every number.
