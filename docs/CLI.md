# CLI reference

Every command, flag, output stream, and exit code. `replica help` shows a
guided overview, and `replica help <topic>` covers one topic. `replica <command>
--help` lists the flags for a command.

## Conventions

- **Root.** `REPLICA_ROOT` (default `/Volumes/Work/replica`) must be a directory on an APFS volume. It holds `base/`, `ws/`, `cache/`, and `replica.db`. `help` and `--version` do not need it.
- **stdout is data.** `replica new` prints only the workspace path. `list`, `stats`, `base status`, and `env` print their data to stdout.
- **stderr is for people.** Status lines look like `✓ message`, `! warning`, `✗ error`, or `· info`. Follow-up lines are indented, and fixes are printed as `hint:`.
- **`--json`** on `list`, `stats`, and `base status` prints stable JSON for scripts. Do not parse the human tables.
- **Color** is used only on a TTY. It is off when `NO_COLOR` is set.
- **Errors** print one status line, then a `hint:` line when Replica knows the fix. The process exits with the code in [Exit codes](#exit-codes).

## Workspaces

### `replica new`

```
replica new [--ttl 2h] [--name LABEL] [--post-clone CMD] [--require-fresh] [--ref BRANCH]
```

Clones the base into a new workspace with `clonefile(2)`. It prints the path on stdout and nothing else, so `cd "$(replica new)"` works.

| Flag | Default | Meaning |
|---|---|---|
| `--ttl` | `2h` | Lifetime: `30m`, `2h`, or `1d`. After this, `replica gc` removes it. |
| `--name` | none | Label shown by `replica list`. |
| `--post-clone` | none | Shell command run inside the new workspace after cloning. A failure keeps the workspace and exits 1. |
| `--require-fresh` | off | Exit 2 instead of warning when the base is stale. |
| `--ref` | none | Branch to create the workspace from. If the branch's lockfile hash matches the base, clone as normal and report "cloned directly". If it differs, clone the base, check out the branch, then run the setup command (an incremental install) and report `lockfile drift on branch my-feature-branch; running incremental install` (with the actual branch name). |

Fails with exit 2 if no base exists, with exit 4 if `REPLICA_MAX_BYTES` is reached, and with exit 3 if the base and `ws/` are on different volumes.

### `replica list`

```
replica list [--json]
```

Active workspaces, least recently used first.

```
ID        NAME     AGE  LAST USED  EXPIRES IN  PATH
340b64a8  fix      4m   4m         1h56m       /Volumes/Work/replica/ws/340b64a8
```

`--json` prints an array of `{id, name, path, created_at, last_used, ttl_secs, expires_at}`. Times are Unix seconds. `name` is `null` when unset.

### `replica env`

```
replica env <id>
```

Prints `export` lines for a workspace. Evaluate them to set up a shell:

```sh
eval "$(replica env 340b64a8)"
```

Sets `REPLICA_WS`, `TMPDIR` (`<workspace>/.replica/tmp`, removed with the workspace), `CARGO_HOME`, `npm_config_cache`, and `SCCACHE_DIR`. The last three point into `$REPLICA_ROOT/cache`, so workspaces share caches. Exit 1 if the id is unknown.

### `replica touch`

```
replica touch <id>
```

Sets the workspace's last-used time to now. LRU eviction skips workspaces used in the last 10 minutes, so call this from long sessions. Exit 1 if the id is unknown.

### `replica rm`

```
replica rm <id>...
```

Deletes one or more workspaces. It is idempotent: an unknown id prints `already removed` and does not fail. Deletion checks the path is under `ws/` and never follows symlinks. If a removal fails, the workspace stays marked for cleanup, `replica gc` retries it, and the command exits 1.

## Base

The base is a git clone of your project with its setup command already run. Workspaces are clones of it.

### `replica base build`

```
replica base build --repo <url-or-path> --setup <cmd>
```

Clones `--repo`, runs `--setup` inside the clone with `sh -c`, and swaps the result in as `base/`. A failed clone or setup leaves the existing base in place. On success it records the git HEAD, a hash of the lockfiles present, the repo, and the setup command. It then runs the same checks as `base check`.

### `replica base check`

```
replica base check
```

Scans the base for absolute paths to the golden tree that will break in clones. Each finding has a kind, a path, and a suggested fix. It exits 0 even when it finds issues, and exits 2 when no base exists.

| Kind | Meaning |
|---|---|
| `absolute path in file` | A text file contains the base path |
| `absolute symlink` | A symlink points into the base |
| `venv activation script` | A venv `bin/activate` contains the base path |
| `pyvenv.cfg` | A venv's `home` points into the base |
| `node_modules/.bin script` | An npm bin shim contains the base path |
| `cargo fingerprint` | `target/` contains fingerprints with the base path |
| `tsbuildinfo` | A TypeScript incremental cache contains the base path |

### `replica base status`

```
replica base status [--json]
```

Shows when the base was built, its git HEAD, lockfile hash, repo, and setup command, and whether it is stale.

```
built     2h
HEAD      c4d0f31e0420
lockfiles 1d68381cb0ab
repo      /src/app
setup     npm ci
status    STALE
          - git HEAD changed (now 9a1b...)
          run `replica base refresh`
```

Staleness means git HEAD moved or a lockfile changed since the build. `--json` reports `stale` as `true`, `false`, or `null` (unknown, when no HEAD was recorded).

### `replica base refresh`

```
replica base refresh
```

Brings the base up to date with its remote. It uses the repo and setup command recorded at build time.

1. **Fast path.** Clonefiles the current base (a metadata-only copy; no data is copied), then runs `git fetch origin` and `git reset --hard origin/HEAD` in the copy.
2. **Setup only if needed.** Re-runs the setup command only when a lockfile hash changed. If nothing lockfile-related changed, setup is skipped.
3. **Swap.** Replaces `base/` with the copy. Existing workspaces keep working, because each one owns its blocks.

If the fast path fails (for example, the remote is unreachable or `origin/HEAD` is missing), refresh falls back to a full rebuild from the repo. A base built before v0.1 has no recorded repo, so refresh asks you to rebuild once with `base build` (exit 1).

## Maintenance

### `replica gc`

```
replica gc [--dry-run] [--high 80] [--low 60]
```

1. Finishes any workspace stuck in the `deleting` state.
2. Removes every workspace past its TTL.
3. If the volume is above `--high` percent (or the `REPLICA_MAX_BYTES` equivalent, if lower), evicts least-recently-used workspaces that were not used in the last 10 minutes, until usage is below `--low`.

`--dry-run` prints what would be removed and changes nothing. Each removal is printed with its reason: `expired` or `evicted for space`.

### `replica stats`

```
replica stats [--json]
```

```
Volume  /Volumes/Work/replica
Usage   ██████░░░░░░░░░░░░░░ 31.2%  6.2 GB used of 20.0 GB
Free    13.8 GB
Spaces  3 active (1 expired)
Base    built 2h ago
```

`--json` reports `root`, `total_bytes`, `used_bytes`, `free_bytes`, `used_pct`, `workspaces`, `expired`, and `base_built_at` (or `null`).

## Setup

### `replica setup`

```
replica setup [--dry-run] [--force] [--homebrew] [--config <path>]
```

Detects the coding agents installed on this machine and writes each one a usage guide and, where supported, the replica skill.

Two kinds of config are written:

- **Appended** (Claude Code, Copilot, Aider): the Replica section is added to the end of the file. It is added once, so running setup again does not duplicate it.
- **Replaced** (Cursor, Zed): the file is written from a template. An existing file is kept unless `--force` is given.

`setup` does not need `REPLICA_ROOT`, so `replica setup --dry-run` works on a fresh machine.

| Agent | Config written | Skill installed |
|---|---|---|
| Claude Code | `~/.claude/CLAUDE.md` (appended) | `~/.claude/skills/replica/SKILL.md` |
| Cursor | `.cursorrules` (replaced) | `.cursor/skills/replica/SKILL.md` |
| GitHub Copilot | `copilot-instructions.md` (appended) | not supported |
| Zed | `~/.config/zed/settings.json` (replaced) | not supported |
| Aider | `~/.aider.conf.yml` (appended) | not supported |

| Flag | Meaning |
|---|---|
| `--dry-run` | Show what would be written. Write nothing. |
| `--force` | Replace existing replaced-type configs (Cursor, Zed). The original is always backed up first, as `<name>.bak`. |
| `--homebrew` | Also write a local formula to `~/homebrew/replica.rb`. The formula has placeholder checksums and is not a supported install path; see HOMEBREW.md. |
| `--config` | Read custom agents from a TOML file (default `~/.config/replica/agents.toml`). |

Custom agents use the same schema as the built-ins:

```toml
[[custom_agents]]
name = "my-agent"
detect_paths = ["~/.my-agent"]
config_files = [
  { target = "~/.my-agent/AGENTS.md", template = "my-agent.md", append = true }
]
skill_registration = { enabled = true, skill_dir = "~/.my-agent/skills", install_script = "skills.sh" }
```

`install_script` is kept for compatibility and is not written by current versions.

### Skills for other agents

The skills in `skills/` follow the SKILL.md format and install with the skills.sh CLI:

```sh
npx skills add gabrielvfonseca/replica            # all skills
npx skills add gabrielvfonseca/replica --list     # see what is available
```

`make skills-check` validates the frontmatter of every skill in the repo.

## Help

```
replica help [topic]
```

With no topic, prints the command overview and the topic list. Topics: `quickstart`, `workspaces`, `base`, `safety`, `agents`, `env`, `exit-codes`. Help never touches `REPLICA_ROOT`, so it works on a machine that has not been set up. An unknown topic exits 1 with the list of topics.

## Exit codes

| Code | Meaning | Typical fix |
|---|---|---|
| 0 | Success | |
| 1 | Generic failure: bad argument, I/O error, failed hook, unknown id | Read the message and hint |
| 2 | No base built, or the base is stale with `--require-fresh` | `replica base build`, or `replica base refresh` |
| 3 | Base and workspaces are on different APFS volumes | Put `REPLICA_ROOT` on the base's volume |
| 4 | Disk full, or the `REPLICA_MAX_BYTES` cap is reached | `replica gc`, or raise the cap |

## Environment variables

| Variable | Default | Meaning |
|---|---|---|
| `REPLICA_ROOT` | `/Volumes/Work/replica` | Root directory on an APFS volume |
| `REPLICA_MAX_BYTES` | unset | Refuse new workspaces past this many bytes, and lower the gc high-water mark |
| `NO_COLOR` | unset | Disable colored output |
