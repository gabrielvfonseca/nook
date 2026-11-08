# Replica Integration for Claude Code

## Overview
Replica is a fast APFS clonefile workspace manager designed for agent workloads. It creates isolated workspaces in milliseconds by using macOS's `clonefile(2)` syscall for copy-on-write cloning.

## When to Use Replica
- **Starting a new task** that needs a clean, isolated project tree
- **Running multiple parallel sessions** without interference
- **When you need fast workspace creation** (< 100ms vs seconds for git clone)
- **When disk space is constrained** - clones share blocks copy-on-write

## Quick Start
```bash
# Create a new workspace (prints path to stdout)
cd $(replica new --ttl 2h)

# Or with a name for easier identification
cd $(replica new --ttl 2h --name "feature-x")

# Run your work, then clean up automatically via TTL
# Or manually: replica rm <id>
```

## Environment Variables
Replica sets up the following in each workspace:
- `REPLICA_WS` - Workspace root path
- `TMPDIR` - Per-workspace temp directory (`$REPLICA_WS/.replica/tmp`)
- `CARGO_HOME` - Shared cargo cache (`$REPLICA_ROOT/cache/cargo`)
- `npm_config_cache` - Shared npm cache (`$REPLICA_ROOT/cache/npm`)
- `SCCACHE_DIR` - Shared sccache cache (`$REPLICA_ROOT/cache/sccache`)

## Workflow Integration
```bash
# In your shell or script
WS_PATH=$(replica new --ttl 4h --name "task-$(date +%s)")
cd "$WS_PATH"
# Your repo is ready at $WS_PATH with all dependencies pre-installed
claude .
```

## Key Commands
| Command | Description |
|---------|-------------|
| `replica new [--ttl 2h] [--name label] [--post-clone <cmd>]` | Create workspace |
| `replica env <id>` | Print export lines for workspace |
| `replica list` | List active workspaces |
| `replica touch <id>` | Update last-used timestamp (prevents LRU eviction) |
| `replica rm <id>...` | Delete workspace(s) |
| `replica gc [--dry-run]` | Expire by TTL, evict LRU if volume > 80% |
| `replica stats` | Show volume usage and workspace count |
| `replica base build --repo <url> --setup <cmd>` | Build the golden base tree |
| `replica base status` | Check base freshness |

## Configuration
Set `REPLICA_ROOT` to your APFS volume (default: `/Volumes/Work/replica`):
```bash
export REPLICA_ROOT=/Volumes/Work/replica
```

## Automatic Cleanup
- Workspaces expire after TTL (default 2h)
- GC runs automatically via launchd every 10 minutes (if enabled)
- LRU eviction kicks in when volume usage > 80%
- Run `replica gc` manually anytime

## Tips
- Use `--require-fresh` with `replica new` to fail if base is stale
- Use `--post-clone` for project-specific setup (e.g., `direnv allow`)
- Call `replica touch <id>` from long-running agent sessions to prevent eviction
- Share caches via `$REPLICA_ROOT/cache` to avoid re-downloading dependencies

## Troubleshooting
- **"different volumes" error**: Base and workspaces must be on same APFS volume
- **"no base" error**: Run `replica base build` first
- **"disk cap reached"**: Increase `REPLICA_MAX_BYTES` or run `replica gc`
- **Stale base warning**: Run `replica base refresh` to rebuild

## More Info
- `replica --help` for all commands
- `man replica` (if installed)
- Source: https://github.com/gabrielfonseca/replica