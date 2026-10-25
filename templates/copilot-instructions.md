# Nook Integration for GitHub Copilot

## Overview
Nook creates instant, isolated workspaces using macOS APFS clonefile (copy-on-write) for fast agent workloads.

## When to Use Nook
- New coding tasks needing clean project state
- Parallel Copilot sessions without interference
- Sub-100ms workspace creation
- Disk efficiency via copy-on-write

## Quick Start
```bash
# Create workspace
cd $(nook new --ttl 2h)

# Your project is ready with all dependencies
```

## Key Commands
- `nook new [--ttl 2h] [--name label]` - Create workspace
- `nook env <id>` - Environment exports
- `nook list` - Active workspaces
- `nook gc` - Cleanup
- `nook stats` - Disk usage

## Configuration
```bash
export NOOK_ROOT=/Volumes/Work/nook
```

## Workflow
1. `nook base build --repo <url> --setup "make deps"` (once)
2. `cd $(nook new)` - Ready to code
3. Workspace auto-cleans via TTL/GC

## Tips
- Shared caches: `$NOOK_ROOT/cache` (cargo, npm, sccache)
- `nook touch <id>` prevents LRU eviction
- `--require-fresh` ensures base is current

## More Info
- `nook --help`
- https://github.com/gabrielfonseca/nook