# Replica Skill

## When to use replica
- Starting a new coding task that needs an isolated workspace
- Running multiple agent sessions in parallel
- When you need fast workspace creation (< 100ms)

## Commands
- `replica new [--ttl 2h]` - Create workspace, prints path
- `replica env <id>` - Get export lines for workspace
- `replica list` - List active workspaces
- `replica gc` - Clean up expired workspaces
- `replica stats` - Show disk usage

## Integration
Add to your workflow: `cd $(replica new) && <your-agent> .`

## Configuration
Set REPLICA_ROOT to your APFS volume:
```bash
export REPLICA_ROOT=/Volumes/Work/replica
```