---
name: replica
description: Create, use, and clean up isolated APFS clone workspaces with the replica CLI. Use when a task needs a fresh, fully installed copy of a project (parallel agent runs, risky experiments, throwaway branches), when the user mentions replica, clonefile workspaces, or copy-on-write checkouts, or when a command needs `replica new`, `replica list`, `replica rm`, `replica gc`, or `replica env`.
---

# Replica: isolated workspaces for agents

Replica clones a prebuilt "base" tree (a git checkout with its dependencies
already installed) with APFS `clonefile(2)`. A new workspace takes milliseconds
and shares disk blocks with the base until a file changes. Use it whenever you
need a clean, ready-to-run copy of the project.

Requires macOS, an APFS volume, and `REPLICA_ROOT` set. Check with
`replica stats` before you start.

## Decide if you should use it

Use Replica when:
- the task needs its own checkout and you do not want to touch the user's working tree
- several agent sessions will run on the same project at once
- setup is slow (`npm ci`, `cargo build`, venv creation) and the base already has it
- the work is disposable and should expire on its own

Skip it when:
- a small edit in the current checkout is enough
- there is no base yet and the project has no reusable setup (run `replica base build` first, or work in place)
- the path must live on a different volume from the base (clones cannot cross volumes)

## Core workflow

```sh
# 1. Create a workspace. stdout is ONLY the path, so this is safe to cd into.
cd "$(replica new --ttl 2h --name fix-login)"

# 2. Point tools at the workspace (temp dir, shared caches).
eval "$(replica env <id>)"        # <id> is the basename of the path

# 3. Work. Keep a long session from being evicted.
replica touch <id>

# 4. Clean up when done (or let the TTL expire).
replica rm <id>
```

The workspace id is the last path component: `basename "$(replica new)"`.

## Command reference

| Command | What it does | Output |
|---|---|---|
| `replica new [--ttl 30m\|2h\|1d] [--name L] [--post-clone CMD] [--require-fresh]` | Clone the base into a new workspace | path on stdout; status on stderr |
| `replica list [--json]` | Active workspaces with age, TTL left, path | table, or JSON array |
| `replica env <id>` | Export lines: `REPLICA_WS`, `TMPDIR`, cache dirs | shell `export` lines |
| `replica touch <id>` | Mark used now, so LRU eviction skips it | status line |
| `replica rm <id>...` | Delete workspaces. Idempotent | status per id |
| `replica gc [--dry-run] [--high 80] [--low 60]` | Remove expired workspaces; evict LRU when the volume is full | status lines and summary |
| `replica stats [--json]` | Volume usage, free space, workspace count, base age | report, or JSON |
| `replica help [topic]` | Built-in guide. Topics: quickstart, workspaces, base, safety, agents, env, exit-codes | text |

Use `replica <command> --help` for every flag. Use `--json` when you parse
output. Never parse the human table.

## Rules for agents

1. Always create workspaces with `replica new` and never delete a path by hand. `replica rm` checks that the path is inside `$REPLICA_ROOT/ws/` and never follows symlinks.
2. Capture the id from `replica new` (via `basename`) so you can `rm` it later.
3. Run `replica touch <id>` during any session that lasts longer than 10 minutes. Otherwise LRU eviction can remove it while you are still using it.
4. Pass `--require-fresh` when the result must match the current commit or lockfiles. It fails with exit 2 if the base is stale, instead of warning and continuing.
5. Use `--post-clone` for project fixups that cannot be shared, such as `--post-clone "direnv allow"` or recreating an absolute-path venv. Keep it short, because it runs in the workspace before you get the path.
6. Do not `replica rm` a workspace that someone else is still using. List first, and check the `name`.

## Exit codes

| Code | Meaning | What to do |
|---|---|---|
| 0 | Success | Continue |
| 1 | Generic failure (bad arguments, I/O, failed hook) | Read the message and hint |
| 2 | No base, or stale base with `--require-fresh` | Run `replica base build` or `replica base refresh` |
| 3 | Base and workspaces on different APFS volumes | Set `REPLICA_ROOT` on the base's volume |
| 4 | Disk full, or `REPLICA_MAX_BYTES` reached | Run `replica gc`, or raise the cap |

Errors print as `✗ message`, followed by a `hint:` line when Replica knows the fix.

## Environment

`REPLICA_ROOT` (default `/Volumes/Work/replica`) must be a directory on an APFS
volume. Optional: `REPLICA_MAX_BYTES` caps the total bytes Replica may use.
`NO_COLOR=1` turns off colored output.

## Troubleshooting

- **`REPLICA_ROOT does not exist`**: create the directory on an APFS volume and export it.
- **`no base tree has been built yet`**: `replica base build --repo <url> --setup "<cmd>"`. See the `replica-base` skill.
- **`base is stale`**: `replica base refresh` rebuilds with the original repo and setup command.
- **Tools inside the workspace fail with "No such file" for absolute paths**: the base has path-dependent files. Run `replica base check`, then the `replica-base` skill.
- **`disk cap reached` or exit 4**: `replica gc`, then `replica stats` to confirm the space came back.
