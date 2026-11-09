//! `replica help [topic]`: a guided overview that works without a REPLICA_ROOT.

use crate::error::CliError;
use crate::ui::{bold, cyan, dim, stdout_color};
use anyhow::Result;

/// Topics shown by `replica help <topic>`, in display order.
pub const TOPICS: &[(&str, &str)] = &[
    ("quickstart", "build a base and make your first workspace"),
    ("workspaces", "create, list, use, and remove workspaces"),
    ("base", "build, check, refresh, and inspect the golden tree"),
    (
        "safety",
        "TTL, LRU eviction, byte caps, and what gets deleted",
    ),
    (
        "agents",
        "wire Replica into Claude Code, Cursor, and other agents",
    ),
    ("env", "environment variables set in each workspace"),
    ("exit-codes", "what each non-zero exit code means"),
];

/// Print the overview (no topic) or one topic to stdout.
pub fn print(topic: Option<&str>) -> Result<()> {
    let on = stdout_color();
    let text = match topic {
        None => overview(on),
        Some(t) => match topic_text(t, on) {
            Some(text) => text,
            None => {
                let names: Vec<&str> = TOPICS.iter().map(|(n, _)| *n).collect();
                return Err(CliError::new(1, format!("unknown help topic `{t}`"))
                    .with_hint(format!("topics: {}", names.join(", ")))
                    .into());
            }
        },
    };
    print!("{text}");
    Ok(())
}

fn heading(s: &str, on: bool) -> String {
    bold(s, on)
}

fn overview(on: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{}  {}\n\n",
        bold("replica", on),
        dim("Fast because it refuses to do the work.", on)
    ));
    s.push_str(&format!("{}\n", heading("USAGE", on)));
    s.push_str("  replica <command> [options]\n\n");
    s.push_str(&format!("{}\n", heading("WORKSPACES", on)));
    s.push_str(&format!(
        "  {}  Clone the base into a new workspace (prints its path)\n",
        cyan("new    ", on)
    ));
    s.push_str(&format!(
        "  {}  Show active workspaces\n",
        cyan("list   ", on)
    ));
    s.push_str(&format!(
        "  {}  Print export lines for a workspace\n",
        cyan("env    ", on)
    ));
    s.push_str(&format!(
        "  {}  Mark a workspace as recently used\n",
        cyan("touch  ", on)
    ));
    s.push_str(&format!(
        "  {}  Delete one or more workspaces\n\n",
        cyan("rm     ", on)
    ));
    s.push_str(&format!("{}\n", heading("BASE", on)));
    s.push_str(&format!(
        "  {}  Build, check, refresh, or inspect the golden tree\n\n",
        cyan("base   ", on)
    ));
    s.push_str(&format!("{}\n", heading("MAINTENANCE", on)));
    s.push_str(&format!(
        "  {}  Expire by TTL, then evict LRU when the volume is full\n",
        cyan("gc     ", on)
    ));
    s.push_str(&format!(
        "  {}  Show volume usage and workspace counts\n\n",
        cyan("stats  ", on)
    ));
    s.push_str(&format!("{}\n", heading("SETUP", on)));
    s.push_str(&format!(
        "  {}  Detect coding agents and write their config and skills\n",
        cyan("setup  ", on)
    ));
    s.push_str(&format!(
        "  {}  Show this guide, or a topic below\n\n",
        cyan("help   ", on)
    ));
    s.push_str(&format!("{}\n", heading("TOPICS", on)));
    for (name, desc) in TOPICS {
        s.push_str(&format!(
            "  {}  {}\n",
            cyan(&format!("{name:<12}"), on),
            desc
        ));
    }
    s.push_str(&format!(
        "\nRun `{}` for options on one command, or `{}` for the guide on one topic.\n",
        "replica <command> --help", "replica help <topic>"
    ));
    s
}

fn topic_text(topic: &str, on: bool) -> Option<String> {
    let body = match topic {
        "quickstart" => QUICKSTART,
        "workspaces" => WORKSPACES,
        "base" => BASE,
        "safety" => SAFETY,
        "agents" => AGENTS,
        "env" => ENV,
        "exit-codes" => EXIT_CODES,
        _ => return None,
    };
    let mut out = String::new();
    for line in body.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            out.push_str(&heading(h, on));
        } else if let Some(c) = line.strip_prefix("$ ") {
            out.push_str(&format!("  {}", cyan(c, on)));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    Some(out)
}

const QUICKSTART: &str = "# Quickstart

1. Point Replica at an APFS volume (use a dedicated volume with a quota):
$ export REPLICA_ROOT=/Volumes/Work/replica

2. Build the golden tree once, with dependencies installed:
$ replica base build --repo https://github.com/you/proj.git --setup \"npm ci\"

3. Make a workspace. The path goes to stdout, so you can cd into it:
$ cd $(replica new --ttl 2h --name fix-login)

4. Clean up when you are done (or let the TTL expire):
$ replica rm <id>

Run `replica help workspaces` and `replica help base` for details.
";

const WORKSPACES: &str = "# Workspaces

Create one with `replica new`. Its path is printed on stdout and everything
else goes to stderr, so `cd $(replica new)` always works.
$ replica new --ttl 4h --name feature-x --post-clone \"direnv allow\"

`--ttl` takes 30m, 2h, or 1d. Workspaces expire after their TTL, and `replica gc`
removes them. `--post-clone` runs a shell command inside the new workspace,
for fixups such as recreating a venv.

Inspect them with `replica list` (table) or `replica list --json` (for scripts).
Keep a long-running session alive with `replica touch <id>`, which stops LRU
eviction from picking it. `replica rm <id>...` deletes one or more and is
idempotent.
";

const BASE: &str = "# The base tree

The base is a git clone with its setup command already run, such as `npm ci`
or `cargo fetch`. Workspaces are clones of it.
$ replica base build --repo <url-or-path> --setup \"<command>\"

`replica base status` shows age, git HEAD, lockfile hash, and whether the base
is stale. `replica new --require-fresh` refuses to clone a stale base.

`replica base refresh` rebuilds the base using the repo and setup command it
was built with. The new tree is built beside the old one and swapped in
atomically, so existing workspaces keep working.

`replica base check` looks for absolute paths to the golden tree that will
break in a clone (venvs, node_modules/.bin, tsbuildinfo, and so on). Fix them
with `--post-clone` or by regenerating them in each workspace.
";

const SAFETY: &str = "# Disk safety

Replica bounds disk use in three ways:
- TTL: `replica gc` removes workspaces older than their TTL.
- LRU: when the volume is above `--high` (default 80%), gc evicts the least
  recently used workspaces until usage falls below `--low` (default 60%).
  Workspaces used in the last 10 minutes are never evicted.
- Byte cap: set REPLICA_MAX_BYTES to refuse new workspaces past a size limit.

Deletion only ever touches paths under $REPLICA_ROOT/ws/. Symlinks are removed
as links and never followed. `replica gc --dry-run` shows what would go first.
";

const AGENTS: &str = "# Coding agents

`replica setup` finds the agents installed on this machine (Claude Code,
Cursor, Copilot, Zed, Aider, plus custom ones in ~/.config/replica/agents.toml)
and writes each one a short usage guide and, where supported, the replica skill.
$ replica setup --dry-run     # preview
$ replica setup               # write

Pass `--homebrew` to also generate a local Homebrew formula, and `--force` to
overwrite existing configs (the old files are backed up first).

Agents should run `cd $(replica new)` for every task, and `replica rm` when
they finish.
";

const ENV: &str = "# Workspace environment

`replica env <id>` prints export lines. Eval them to point tools at the workspace:
$ eval \"$(replica env <id>)\"

It sets REPLICA_WS (the workspace root), TMPDIR (a per-workspace temp dir that
is removed with the workspace), and shared caches under $REPLICA_ROOT/cache:
CARGO_HOME, npm_config_cache, and SCCACHE_DIR.
";

const EXIT_CODES: &str = "# Exit codes

  0  success
  1  generic failure (bad arguments, I/O error, failed hook)
  2  no base built, or the base is stale and --require-fresh was given
  3  base and workspaces are on different APFS volumes
  4  disk full, or the REPLICA_MAX_BYTES cap is reached

Errors are printed as `✗ message`, with a `hint:` line when Replica knows a fix.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_text() {
        for (name, _) in TOPICS {
            assert!(topic_text(name, false).is_some(), "missing topic {name}");
        }
    }

    #[test]
    fn unknown_topic_is_none() {
        assert!(topic_text("nope", false).is_none());
    }

    #[test]
    fn overview_lists_every_topic() {
        let text = overview(false);
        for (name, _) in TOPICS {
            assert!(text.contains(name));
        }
    }

    #[test]
    fn no_escape_codes_when_color_off() {
        assert!(!overview(false).contains('\x1b'));
        assert!(!topic_text("base", false).unwrap().contains('\x1b'));
    }
}
