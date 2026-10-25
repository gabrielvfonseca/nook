# Nook Skill

## When to use nook
- Starting a new coding task that needs an isolated workspace
- Running multiple agent sessions in parallel
- When you need fast workspace creation (< 100ms)

## Commands
- `nook new [--ttl 2h]` - Create workspace, prints path
- `nook env <id>` - Get export lines for workspace
- `nook list` - List active workspaces
- `nook gc` - Clean up expired workspaces
- `nook stats` - Show disk usage

## Integration
Add to your workflow: `cd $(nook new) && <your-agent> .`

## Configuration
Set NOOK_ROOT to your APFS volume:
```bash
export NOOK_ROOT=/Volumes/Work/nook
```