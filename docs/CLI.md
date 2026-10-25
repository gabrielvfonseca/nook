# CLI Reference

Env: `NOOK_ROOT` (default `/Volumes/Work/nook`).

## nook base build --repo <url|path> --setup <cmd>
Build the golden tree. Atomic swap. Fails if setup exits non-zero. Runs `base check` automatically after building. Records git HEAD and lockfile hash.

## nook base check
Scan the golden tree for absolute paths that will break when cloned. Reports findings with suggestions.

## nook base refresh
Rebuild the base side-by-side in `base.new`, then atomically swap. Existing clones keep working (they own their blocks). Old base moved to `base.old` and cleaned up by `gc` once no clones reference it.

## nook base status
Prints base age, HEAD, lockfile hash, stale yes/no (compares current git HEAD and lockfiles to recorded values).

## nook new [--ttl 2h] [--name label] [--post-clone <cmd>] [--require-fresh]
Clone base. TTL accepts `30m`, `2h`, `1d`. Prints workspace path on stdout (only); logs go to stderr so `cd $(nook new)` works. Optional `--post-clone` runs a shell command in the new workspace after cloning (for project-specific fixups like recreating a venv). `--require-fresh` fails if base is stale (behind remote or lockfiles changed).

## nook env <id>
Prints `export` lines: `NOOK_WS`, `TMPDIR`, cache dirs.

## nook list [--json]
id, name, age, last used, ttl remaining, state.

## nook touch <id>
Update `last_used` (call from agent harness to avoid LRU eviction).

## nook rm <id>...
Parallel delete. Idempotent.

## nook gc [--dry-run]
Expire by TTL, then evict LRU if volume above high-water.

## nook stats
Volume total/free/used %, workspace count, oldest/newest. Uses `statvfs`.

## nook setup [--homebrew] [--force] [--dry-run] [--config <path>]
Detect coding agents on the machine, generate agent-specific configuration with nook usage instructions, set up skills integration, and optionally generate a local Homebrew formula.

### Options
- `--homebrew` - Also generate a local Homebrew formula at `~/homebrew/nook.rb`
- `--force` - Force re-generation even if configs exist (backs up existing files)
- `--dry-run` - Only detect agents and show what would be written, don't write files
- `--config <path>` - Path to custom agents config (TOML), default: `~/.config/nook/agents.toml`

### Supported Agents (built-in)
| Agent | Config Files | Skills |
|-------|-------------|--------|
| Claude Code | `~/.claude/CLAUDE.md` (append) | `~/.claude/skills/nook/` |
| Cursor | `.cursorrules` (overwrite) | `.cursor/skills/nook/` |
| GitHub Copilot | `~/Library/Application Support/Code/User/copilot-instructions.md` (append) | N/A |
| Zed | `~/.config/zed/settings.json` (merge) | N/A |
| Aider | `~/.aider.conf.yml` (append) | N/A |

### Custom Agents
Add custom agents in `~/.config/nook/agents.toml`:
```toml
[[custom_agents]]
name = "my-agent"
detect_paths = ["~/.my-agent", "~/.config/my-agent"]
config_files = [
  { target = "~/.my-agent/AGENTS.md", template = "my-agent.md", append = true }
]
skill_registration = { enabled = true, skill_dir = "~/.my-agent/skills", install_script = "skills.sh" }
```

### Skills Integration
For agents with skill support (Claude Code, Cursor), `nook setup` creates:
- `~/.claude/skills/nook/skill.toml` - Skill metadata
- `~/.claude/skills/nook/instructions.md` - Usage instructions
- `~/.claude/skills/skills.sh` - Installer script

Run `skills.sh` to verify installation or re-install skills.

### Homebrew Formula
With `--homebrew`, generates `~/homebrew/nook.rb` for local installation:
```bash
brew install ~/homebrew/nook.rb
```
The formula includes a launchd plist for automatic GC (every 10 minutes).

### Examples
```bash
# Detect agents, generate configs, install skills
nook setup

# Also generate Homebrew formula
nook setup --homebrew

# Preview what would happen
nook setup --dry-run

# Force regenerate (backs up existing configs)
nook setup --force

# Use custom agents config
nook setup --config ./my-agents.toml
```

## Exit codes
0 ok, 1 generic error, 2 base missing/stale-and-strict, 3 cross-volume, 4 disk full.