---
name: replica-base
description: Build, check, refresh, and diagnose the golden base tree that replica clones. Use when setting up Replica for a project for the first time, choosing the setup command for an ecosystem (npm, pnpm, Cargo, Python, Go), when `replica base check` reports path-dependent files, when `replica base status` says STALE, or when clones fail because they reference the base's absolute path.
---

# Replica base: building and maintaining the golden tree

The base is a git clone of your project with its setup command already run.
Every `replica new` clones it. Get the base right and every workspace is ready
in milliseconds. Get it wrong and every workspace inherits the mistake.

## Build it

```sh
replica base build --repo <url-or-local-path> --setup "<command>"
```

- `--repo` is cloned with `git clone`. A remote URL or a local path both work. Use a remote if the base must follow `main`.
- `--setup` runs inside the clone with `sh -c`. It must leave the tree ready to run, and it must exit non-zero on failure. A failure aborts the build and leaves the old base in place.
- The build records git HEAD, a hash of every lockfile present, the repo, and the setup command. `replica base refresh` needs the last two.

Then check it:

```sh
replica base check      # find absolute paths that will break in clones
replica base status     # age, HEAD, lockfile hash, stale or fresh
```

## Setup commands by ecosystem

| Project | `--setup` | Notes |
|---|---|---|
| npm | `npm ci` | Deterministic from `package-lock.json`. Produces `node_modules/.bin` scripts with absolute paths, so check them. |
| pnpm | `pnpm install --frozen-lockfile` | Symlinked `node_modules`. Run `replica base check` to confirm. |
| yarn | `yarn install --frozen-lockfile` | Same as pnpm. |
| Cargo | `cargo fetch` | Downloads crates into the builder's `~/.cargo`. Workspaces use `CARGO_HOME` from `replica env`, so they share a cache outside the base. Prefer `fetch` to `build`, because `target/` fingerprints hold absolute paths. |
| Python (uv) | `uv sync --frozen` | Creates `.venv` with absolute paths. Usually needs a `--post-clone` fix. |
| Python (pip) | `python3 -m venv .venv && .venv/bin/pip install -r requirements.txt` | `pyvenv.cfg` and `bin/` scripts hold the base path. Recreate the venv per workspace. |
| Go | `go mod download` | Module cache is shared. Nothing path-dependent. |

If the project has several ecosystems, chain them with `&&`, for example
`--setup "npm ci && cargo fetch"`.

## Path-dependent output

Some tools write the absolute base path into files. Clones keep those paths,
so the clone ends up using the base's files. `replica base check` looks for:

| Finding | Typical cause | Fix |
|---|---|---|
| `absolute path in file` | A generated config or build file names the base path | Regenerate it per workspace, or add a `--post-clone` that rewrites it |
| `absolute symlink` | A symlink points at the base | Recreate the link in `--post-clone` |
| `venv activation script` / `pyvenv.cfg` | Python venv created in the base | Recreate the venv per workspace |
| `node_modules/.bin script` | npm bin shims with absolute paths | `npm rebuild` or reinstall in `--post-clone` |
| `cargo fingerprint` | `target/` built in the base | Do not ship `target/`. Use `cargo fetch` as the setup |
| `tsbuildinfo` | TypeScript incremental cache | Delete the file in `--post-clone` |

Fix a finding at the source where you can. Use `--post-clone` for the rest:

```sh
replica new --post-clone "rm -rf .venv && uv sync --frozen"
replica new --post-clone "npm rebuild"
```

`--post-clone` runs after the clone, in every workspace that uses it, and it
adds to the time `replica new` takes. Keep it cheap. The benchmark suite does
not use it, so its numbers are for a plain `replica new`.

## Freshness and refresh

A base goes stale when git HEAD or any lockfile changes after the build.

```sh
replica base status              # shows STALE and the reasons
replica new --require-fresh      # exit 2 instead of cloning a stale base
replica base refresh             # rebuild with the original repo and setup
```

`refresh` rebuilds the base in `base.new`, then swaps it in. Existing
workspaces keep working, because clonefile gave them their own blocks. A base
built before v0.1 has no stored repo or setup command. `refresh` says so and
tells you to rebuild once with `replica base build`.

## Common problems

- **`base build` fails at clone**: check the repo URL. For a private repo, the machine needs git credentials.
- **`setup command failed`**: the command runs in the clone. Reproduce it in a temp clone, then fix it in the project.
- **`base is stale` on every `new`**: the lockfile has changed since the last build. Run `replica base refresh`.
- **`different volumes` (exit 3)**: `REPLICA_ROOT` moved to another volume after the base was built. Rebuild on the new volume.
- **Base is large and `replica stats` shows low free space**: each base takes its full size on disk. Rebuild only when needed, and keep one base per project.
