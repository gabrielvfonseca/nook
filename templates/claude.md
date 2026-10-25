# Nook Integration for Claude Code

## Overview
Nook is a fast APFS clonefile workspace manager designed for agent workloads. It creates isolated workspaces in milliseconds by using macOS's `clonefile(2)` syscall for copy-on-write cloning.

## When to Use Nook
- **Starting a new task** that needs a clean, isolated project tree
- **Running multiple parallel sessions** without interference
- **When you need fast workspace creation** (< 100ms vs seconds for git clone)
- **When disk space is constrained** - clones share blocks copy-on-write

## Quick Start
```bash
# Create a new workspace (prints path to stdout)
cd $(nook new --ttl 2h)

# Or with a name for easier identification
cd $(nook new --ttl 2h --name "feature-x")

# Run your work, then clean up automatically via TTL
# Or manually: nook rm <id>
```

## Environment Variables
Nook sets up the following in each workspace:
- `NOOK_WS` - Workspace root path
- `TMPDIR` - Per-workspace temp directory (`$NOOK_WS/.nook/tmp`)
- `CARGO_HOME` - Shared cargo cache (`$NOOK_ROOT/cache/cargo`)
- `npm_config_cache` - Shared npm cache (`$NOOK_ROOT/cache/npm`)
- `SCCACHE_DIR` - Shared sccache cache (`$NOOK_ROOT/cache/sccache`)

## Workflow Integration
```bash
# In your shell or script
WS_PATH=$(nook new --ttl 4h --name "task-$(date +%s)")
cd "$WS_PATH"
# Your repo is ready at $WS_PATH with all dependencies pre-installed
claude .
```

## Key Commands
| Command | Description |
|---------|-------------|
| `nook new [--ttl 2h] [--name label] [--post-clone <cmd>]` | Create workspace |
| `nook env <id>` | Print export lines for workspace |
| `nook list` | List active workspaces |
| `nook touch <id>` | Update last-used timestamp (prevents LRU eviction) |
| `nook rm <id>...` | Delete workspace(s) |
| `nook gc [--dry-run]` | Expire by TTL, evict LRU if volume > 80% |
| `nook stats` | Show volume usage and workspace count |
| `nook base build --repo <url> --setup <cmd>` | Build the golden base tree |
| `nook base status` | Check base freshness |

## Configuration
Set `NOOK_ROOT` to your APFS volume (default: `/Volumes/Work/nook`):
```bash
export NOOK_ROOT=/Volumes/Work/nook
```

## Automatic Cleanup
- Workspaces expire after TTL (default 2h)
- GC runs automatically via launchd every 10 minutes (if enabled)
- LRU eviction kicks in when volume usage > 80%
- Run `nook gc` manually anytime

## Tips
- Use `--require-fresh` with `nook new` to fail if base is stale
- Use `--post-clone` for project-specific setup (e.g., `direnv allow`)
- Call `nook touch <id>` from long-running agent sessions to prevent eviction
- Share caches via `$NOOK_ROOT/cache` to avoid re-downloading dependencies

## Troubleshooting
- **"different volumes" error**: Base and workspaces must be on same APFS volume
- **"no base" error**: Run `nook base build` first
- **"disk cap reached"**: Increase `NOOK_MAX_BYTES` or run `nook gc`
- **Stale base warning**: Run `nook base refresh` to rebuild

## More Info
- `nook --help` for all commands
- `man nook` (if installed)
- Source: https://github.com/gabrielfonseca/nook