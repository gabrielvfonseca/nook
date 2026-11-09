# Replica Integration for GitHub Copilot

## Overview
Replica creates instant, isolated workspaces using macOS APFS clonefile (copy-on-write) for fast agent workloads.

## When to Use Replica
- New coding tasks needing clean project state
- Parallel Copilot sessions without interference
- Millisecond workspace creation
- Disk efficiency via copy-on-write

## Quick Start
```bash
# Create workspace
cd $(replica new --ttl 2h)

# Your project is ready with all dependencies
```

## Key Commands
- `replica new [--ttl 2h] [--name label]` - Create workspace
- `replica env <id>` - Environment exports
- `replica list` - Active workspaces
- `replica gc` - Cleanup
- `replica stats` - Disk usage

## Configuration
```bash
export REPLICA_ROOT=/Volumes/Work/replica
```

## Workflow
1. `replica base build --repo <url> --setup "make deps"` (once)
2. `cd $(replica new)` - Ready to code
3. Workspace auto-cleans via TTL/GC

## Tips
- Shared caches: `$REPLICA_ROOT/cache` (cargo, npm, sccache)
- `replica touch <id>` prevents LRU eviction
- `--require-fresh` ensures base is current

## More Info
- `replica --help`
- https://github.com/gabrielvfonseca/replica