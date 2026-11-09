use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use replica::error::{self, CliError, EXIT_DISK_FULL, EXIT_NO_BASE};
use replica::ui::{self, Status};
use replica::{base_check, clone, gc, help, lockfile, setup, store, util, volume};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "replica",
    version,
    about = "Fast because it refuses to do the work.",
    long_about = "Isolated, fully installed workspace replicas for AI agents.\n\n\
                  Replica clones a prebuilt base tree with APFS clonefile(2), so a new\n\
                  workspace takes milliseconds and costs almost no disk.",
    after_help = "Run `replica help` for a guided overview and topic guides.",
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the guide, or the guide for one topic
    #[command(display_order = 1)]
    Help {
        /// quickstart, workspaces, base, safety, agents, env, or exit-codes
        topic: Option<String>,
    },
    /// Clone the base into a new workspace (prints its path on stdout)
    #[command(display_order = 2)]
    New {
        /// How long the workspace lives: 30m, 2h, or 1d
        #[arg(long, default_value = "2h")]
        ttl: String,
        /// Label shown by `replica list`
        #[arg(long)]
        name: Option<String>,
        /// Shell command to run inside the new workspace (e.g. "direnv allow")
        #[arg(long)]
        post_clone: Option<String>,
        /// Fail (exit 2) instead of warning when the base is stale
        #[arg(long)]
        require_fresh: bool,
        /// Branch to create the workspace from; checked against the base lockfile hash
        #[arg(long = "ref")]
        ref_branch: Option<String>,
    },
    /// List active workspaces
    #[command(display_order = 3)]
    List {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Print export lines for a workspace (eval them to set up your shell)
    #[command(display_order = 4)]
    Env {
        /// Workspace id from `replica list`
        id: String,
    },
    /// Mark a workspace as recently used, so LRU eviction skips it
    #[command(display_order = 5)]
    Touch {
        /// Workspace id
        id: String,
    },
    /// Delete one or more workspaces (idempotent)
    #[command(display_order = 6)]
    Rm {
        /// Workspace ids
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
    /// Expire by TTL, then evict least-recently-used workspaces if the volume is full
    #[command(display_order = 7)]
    Gc {
        /// Show what would be removed without deleting anything
        #[arg(long)]
        dry_run: bool,
        /// Start evicting LRU workspaces above this volume usage (%)
        #[arg(long, default_value_t = 80.0)]
        high: f64,
        /// Stop evicting once usage falls below this (%)
        #[arg(long, default_value_t = 60.0)]
        low: f64,
    },
    /// Show volume usage, workspace counts, and base status
    #[command(display_order = 8)]
    Stats {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage the golden base tree
    #[command(display_order = 9)]
    Base {
        #[command(subcommand)]
        cmd: BaseCmd,
    },
    /// Detect coding agents, write their config and skills, optionally a Homebrew formula
    #[command(display_order = 10)]
    Setup {
        /// Also generate a local Homebrew formula at ~/homebrew/replica.rb
        #[arg(long)]
        homebrew: bool,
        /// Overwrite existing configs (originals are backed up first)
        #[arg(long)]
        force: bool,
        /// Only detect agents and show what would be written
        #[arg(long)]
        dry_run: bool,
        /// Path to a custom agents config (TOML)
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BaseCmd {
    /// Clone a repo, run the setup command in it, and swap it in as the base
    Build {
        /// Git repo URL or local path
        #[arg(long)]
        repo: String,
        /// Command run inside the clone to install dependencies, e.g. "npm ci"
        #[arg(long)]
        setup: String,
    },
    /// Scan the base for absolute paths that will break inside clones
    Check,
    /// Update the base from its remote; re-runs setup only if a lockfile changed
    Refresh,
    /// Show base age, git HEAD, lockfile hash, and whether it is stale
    Status {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

fn root() -> PathBuf {
    PathBuf::from(std::env::var("REPLICA_ROOT").unwrap_or_else(|_| "/Volumes/Work/replica".into()))
}

fn main() {
    if let Err(e) = run() {
        ui::status(Status::Err, &format!("{e:#}"));
        if let Some(hint) = error::hint(&e) {
            ui::detail(&format!("hint: {hint}"));
        }
        std::process::exit(error::exit_code(&e));
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Help never needs the workspace root, so it works on a fresh machine.
    if let Cmd::Help { topic } = &cli.cmd {
        return help::print(topic.as_deref());
    }
    // Setup only writes agent configs and never touches the root, so it runs before the root checks.
    if let Cmd::Setup {
        homebrew,
        force,
        dry_run,
        config,
    } = &cli.cmd
    {
        return setup::run_setup(&root(), *homebrew, *force, *dry_run, config.as_deref());
    }

    let r = root();
    ensure_root(&r)?;

    let ws_root = r.join("ws");
    std::fs::create_dir_all(&ws_root)?;
    std::fs::create_dir_all(r.join("cache"))?;
    let db = store::open(&r.join("replica.db"))?;

    // For commands that need base, verify base and ws_root are on same volume
    let needs_base = matches!(
        cli.cmd,
        Cmd::New { .. }
            | Cmd::Base {
                cmd: BaseCmd::Build { .. }
                    | BaseCmd::Check
                    | BaseCmd::Refresh
                    | BaseCmd::Status { .. }
            }
    );
    if needs_base {
        let base = r.join("base");
        if base.exists() {
            volume::verify_same_volume(&base, &ws_root)?;
        }
    }

    match cli.cmd {
        // Handled before the root checks above.
        Cmd::Help { .. } | Cmd::Setup { .. } => {}
        Cmd::New {
            ttl,
            name,
            post_clone,
            require_fresh,
            ref_branch,
        } => new_workspace(
            &r,
            &ws_root,
            &db,
            &ttl,
            name,
            post_clone,
            require_fresh,
            ref_branch.as_deref(),
        )?,
        Cmd::Env { id } => {
            let ws = require_ws(&db, &id)?;
            let p = ws_root.join(&ws.id);
            println!("export REPLICA_WS={}", p.display());
            println!("export TMPDIR={}/.replica/tmp", p.display());
            println!("export CARGO_HOME={}/cache/cargo", r.display());
            println!("export npm_config_cache={}/cache/npm", r.display());
            println!("export SCCACHE_DIR={}/cache/sccache", r.display());
        }
        Cmd::List { json } => list_workspaces(&db, json)?,
        Cmd::Touch { id } => {
            let ws = require_ws(&db, &id)?;
            store::touch(&db, &ws.id)?;
            gc::refresh_lease(ws_root.as_path(), &ws.id)?;
            ui::status(Status::Ok, &format!("touched {}", ws.id));
        }
        Cmd::Rm { ids } => remove_workspaces(&db, &ws_root, &ids)?,
        Cmd::Gc { dry_run, high, low } => run_gc(&db, &r, &ws_root, dry_run, high, low)?,
        Cmd::Stats { json } => stats(&r, &db, json)?,
        Cmd::Base { cmd } => match cmd {
            BaseCmd::Build { repo, setup } => build_base(&r, &db, &repo, &setup)?,
            BaseCmd::Check => check_base(&r)?,
            BaseCmd::Refresh => refresh_base(&r, &db)?,
            BaseCmd::Status { json } => status_base(&r, &db, json)?,
        },
    }
    Ok(())
}

/// The root must exist and be on APFS. Checked once per command, before any state is touched.
fn ensure_root(r: &Path) -> Result<()> {
    if !r.exists() {
        return Err(CliError::new(
            1,
            format!("REPLICA_ROOT does not exist: {}", r.display()),
        )
        .with_hint("create a directory on an APFS volume and export REPLICA_ROOT to it (see `replica help quickstart`)")
        .into());
    }
    volume::verify_apfs(r)
}

fn require_ws(db: &rusqlite::Connection, id: &str) -> Result<store::Ws> {
    store::find(db, id)?.ok_or_else(|| {
        CliError::new(1, format!("no workspace with id `{id}`"))
            .with_hint("list workspaces with `replica list`")
            .into()
    })
}

#[allow(clippy::too_many_arguments)]
fn new_workspace(
    r: &Path,
    ws_root: &Path,
    db: &rusqlite::Connection,
    ttl: &str,
    name: Option<String>,
    post_clone: Option<String>,
    require_fresh: bool,
    ref_branch: Option<&str>,
) -> Result<()> {
    let ttl_secs = util::parse_ttl(ttl)?;
    let base = r.join("base");
    if !base.exists() {
        return Err(CliError::no_base().into());
    }
    let base_info = store::get_base_info(db)?;
    check_staleness(&base, base_info.as_ref(), require_fresh)?;
    enforce_cap(r)?;

    let id = util::new_id();
    let dst = ws_root.join(&id);
    clone::clone_tree(&base, &dst)?;
    std::fs::create_dir_all(dst.join(".replica/tmp"))?;
    // Mark the workspace in use before it becomes visible to gc, so a
    // concurrent `replica gc` never removes a workspace that is still being
    // set up.
    gc::create_lease(ws_root, &id)?;
    // Extract what we need before base_info goes out of scope.
    let setup_cmd = base_info.as_ref().and_then(|i| i.setup_cmd.as_deref());
    let repo = base_info.as_ref().and_then(|i| i.repo.as_deref());
    let base_lockfile_hash = base_info.as_ref().and_then(|i| i.lockfile_hash.as_deref());
    store::insert(
        db,
        &id,
        name.as_deref(),
        &dst.to_string_lossy(),
        ttl_secs,
        base_lockfile_hash.map(|s| s.to_string()),
    )?;

    // --ref: decide whether the branch's lockfile matches the base.
    // Match -> plain clone. Mismatch -> clone then run the setup command
    // (incremental install) so the workspace gets correct dependencies.
    if let Some(branch) = ref_branch {
        let drift = ref_lockfile_drift(base_lockfile_hash, repo, branch)?;
        if drift {
            ui::status(
                Status::Warn,
                &format!("lockfile drift on branch `{branch}`; running incremental install"),
            );
            if let Some(setup) = setup_cmd {
                ui::status(
                    Status::Info,
                    &format!("running incremental install: {setup}"),
                );
                let ok = Command::new("sh")
                    .arg("-c")
                    .arg(setup)
                    .current_dir(&dst)
                    .status()?
                    .success();
                if !ok {
                    return Err(CliError::new(
                        1,
                        format!("incremental install failed for branch `{branch}`"),
                    )
                    .with_hint(format!(
                        "the workspace was cloned; inspect it or remove it with `replica rm {id}`"
                    ))
                    .into());
                }
            } else {
                ui::status(
                    Status::Warn,
                    "no setup command recorded for the base; skipping incremental install",
                );
            }
        } else {
            ui::status(
                Status::Ok,
                &format!("branch `{branch}` lockfile matches base; cloned directly"),
            );
        }
    }

    if let Some(cmd) = post_clone {
        ui::status(Status::Info, &format!("running post-clone hook: {cmd}"));
        let ok = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&dst)
            .status()?
            .success();
        if !ok {
            return Err(CliError::new(1, "post-clone hook failed")
                .with_hint(format!(
                    "the workspace was kept; inspect it or remove it with `replica rm {id}`"
                ))
                .into());
        }
    }

    let label = name.map(|n| format!(" `{n}`")).unwrap_or_default();
    ui::status(
        Status::Ok,
        &format!(
            "workspace {id}{label} ready, expires in {}",
            ui::human_duration(ttl_secs)
        ),
    );
    println!("{}", dst.display()); // stdout is the path only
    Ok(())
}

/// Warn about a stale base, or fail when `require_fresh` is set.
fn check_staleness(base: &Path, info: Option<&store::BaseInfo>, require_fresh: bool) -> Result<()> {
    let Some(head) = info.and_then(|i| i.head.as_deref()) else {
        return Ok(());
    };
    let lockfile_hash = info.and_then(|i| i.lockfile_hash.as_deref());
    let (head_changed, lockfile_changed, current_head, current_lockfile_hash) =
        lockfile::is_base_stale(base, head, lockfile_hash)?;
    if !head_changed && !lockfile_changed {
        return Ok(());
    }
    let reason = match (head_changed, lockfile_changed) {
        (true, true) => "git HEAD and lockfiles have changed",
        (true, false) => "git HEAD has changed",
        _ => "lockfiles have changed",
    };
    if require_fresh {
        return Err(
            CliError::new(EXIT_NO_BASE, format!("base is stale: {reason}"))
                .with_hint("run `replica base refresh`, or drop --require-fresh")
                .into(),
        );
    }
    ui::status(Status::Warn, &format!("base is stale: {reason}"));
    ui::detail("run `replica base refresh` to rebuild it");
    if let Some(h) = current_head {
        ui::detail(&format!("current HEAD: {h}"));
    }
    if let Some(lh) = current_lockfile_hash {
        ui::detail(&format!("current lockfile hash: {lh}"));
    }
    Ok(())
}

/// Refuse new workspaces once REPLICA_MAX_BYTES is reached.
fn enforce_cap(r: &Path) -> Result<()> {
    let Some(max_bytes) = std::env::var("REPLICA_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    else {
        return Ok(());
    };
    let used = volume::volume_used_bytes(r)?;
    if used >= max_bytes {
        return Err(CliError::new(
            EXIT_DISK_FULL,
            format!(
                "disk cap reached: {} used of {} limit",
                ui::human_bytes(used),
                ui::human_bytes(max_bytes)
            ),
        )
        .with_hint("raise REPLICA_MAX_BYTES, or run `replica gc`")
        .into());
    }
    Ok(())
}

/// Clone `--ref <branch>` from the recorded repo into a temp dir and compare
/// its lockfile hash to the base's. Returns true if they differ (lockfile
/// drift, so an incremental install is needed), false if they match.
/// On any error (unreachable repo, no lockfiles) reports drift so the workspace
/// gets an incremental install rather than stale dependencies.
fn ref_lockfile_drift(
    base_lockfile_hash: Option<&str>,
    repo: Option<&str>,
    branch: &str,
) -> Result<bool> {
    let Some(repo) = repo else {
        ui::status(
            Status::Warn,
            "no repo recorded for the base; cannot compare ref lockfile",
        );
        return Ok(true);
    };

    let temp_dir = std::env::temp_dir().join(format!(
        "replica-ref-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let drift = (|| -> Result<bool> {
        fs::create_dir_all(&temp_dir)?;
        let status = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--single-branch",
                "--branch",
                branch,
                "--",
                repo,
                temp_dir
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid temp dir path"))?,
            ])
            .status()?;
        if !status.success() {
            ui::status(
                Status::Warn,
                &format!("could not fetch branch `{branch}` from {repo}; assuming drift"),
            );
            return Ok(true);
        }
        let branch_hash = lockfile::compute_lockfile_hash(&temp_dir)?;
        Ok(branch_hash.as_deref() != base_lockfile_hash)
    })();
    let _ = fs::remove_dir_all(&temp_dir);
    drift
}

fn list_workspaces(db: &rusqlite::Connection, json: bool) -> Result<()> {
    let now = store::now();
    let active = store::list(db, "active")?;
    if json {
        let items: Vec<String> = active
            .iter()
            .map(|w| {
                format!(
                    "{{\"id\":{},\"name\":{},\"path\":{},\"created_at\":{},\"last_used\":{},\"ttl_secs\":{},\"expires_at\":{}}}",
                    ui::json_string(&w.id),
                    w.name.as_deref().map(ui::json_string).unwrap_or_else(|| "null".into()),
                    ui::json_string(&w.path),
                    w.created_at,
                    w.last_used,
                    w.ttl_secs,
                    w.expires_at()
                )
            })
            .collect();
        println!("[{}]", items.join(","));
        return Ok(());
    }
    if active.is_empty() {
        ui::status(Status::Info, "no active workspaces");
        ui::detail("create one with `replica new`");
        return Ok(());
    }
    let on = ui::stdout_color();
    let rows: Vec<Vec<String>> = active
        .iter()
        .map(|w| {
            let left = w.expires_at() - now;
            let expires = if left <= 0 {
                ui::yellow("expired", on)
            } else {
                ui::human_duration(left)
            };
            vec![
                ui::cyan(&w.id, on),
                w.name.clone().unwrap_or_else(|| ui::dim("-", on)),
                ui::human_duration(now - w.created_at),
                ui::human_duration(now - w.last_used),
                expires,
                w.path.clone(),
            ]
        })
        .collect();
    print!(
        "{}",
        ui::table(
            &["ID", "NAME", "AGE", "LAST USED", "EXPIRES IN", "PATH"],
            &rows,
            on
        )
    );
    Ok(())
}

fn remove_workspaces(db: &rusqlite::Connection, ws_root: &Path, ids: &[String]) -> Result<()> {
    let mut failures = 0;
    for id in ids {
        if store::find(db, id)?.is_none() {
            ui::status(
                Status::Info,
                &format!("{id}: no such workspace (already removed)"),
            );
            continue;
        }
        store::set_state(db, id, "deleting")?;
        match gc::safe_remove(ws_root, &ws_root.join(id)) {
            Ok(()) => {
                gc::remove_lease(ws_root, id)?;
                store::delete(db, id)?;
                ui::status(Status::Ok, &format!("removed {id}"));
            }
            Err(e) => {
                failures += 1;
                ui::status(Status::Err, &format!("could not remove {id}: {e:#}"));
                ui::detail("the workspace stays marked for cleanup; `replica gc` retries it");
            }
        }
    }
    if failures > 0 {
        return Err(
            CliError::new(1, format!("{failures} workspace(s) could not be removed")).into(),
        );
    }
    Ok(())
}

/// Why a workspace is being removed by gc.
#[derive(Clone, Copy, PartialEq)]
enum Reason {
    Expired,
    Lru,
}

fn run_gc(
    db: &rusqlite::Connection,
    r: &Path,
    ws_root: &Path,
    dry_run: bool,
    high: f64,
    low: f64,
) -> Result<()> {
    // REPLICA_MAX_BYTES acts as an effective high-water mark, if it is tighter.
    let max_bytes = std::env::var("REPLICA_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let effective_high = match max_bytes {
        Some(max) => {
            let total = volume::volume_total_bytes(r)?;
            (max as f64 / total as f64 * 100.0).min(high)
        }
        None => high,
    };

    // Retry anything left in `deleting` by a crash.
    let leftovers = store::list(db, "deleting")?;
    if !dry_run {
        for w in &leftovers {
            gc::safe_remove(ws_root, Path::new(&w.path))?;
            store::delete(db, &w.id)?;
        }
    }

    let now = store::now();
    let active = store::list(db, "active")?;
    let mut victims: Vec<(&store::Ws, Reason)> = active
        .iter()
        .filter(|w| w.expires_at() < now)
        .map(|w| (w, Reason::Expired))
        .collect();
    if volume::volume_used_pct(r)? > effective_high {
        // `active` is least-recently-used first. Skip anything touched in the last 10 minutes.
        for w in &active {
            if now - w.last_used > 600 && !victims.iter().any(|(v, _)| v.id == w.id) {
                victims.push((w, Reason::Lru));
            }
        }
    }

    let (mut expired, mut evicted) = (0, 0);
    for (w, reason) in victims {
        // Expired workspaces are always removed. LRU victims stop once usage is below `low`.
        if reason == Reason::Lru && volume::volume_used_pct(r)? < low {
            break;
        }
        let label = match reason {
            Reason::Expired => "expired",
            Reason::Lru => "evicted for space",
        };
        if dry_run {
            ui::status(Status::Info, &format!("would remove {} ({label})", w.id));
        } else {
            // Never remove a workspace with a fresh lease: an agent is using it.
            if gc::lease_is_fresh(ws_root, &w.id)? {
                ui::status(Status::Info, &format!("skip {} (in use)", w.id));
                continue;
            }
            store::set_state(db, &w.id, "deleting")?;
            gc::safe_remove(ws_root, Path::new(&w.path))?;
            gc::remove_lease(ws_root, &w.id)?;
            store::delete(db, &w.id)?;
            ui::status(Status::Ok, &format!("removed {} ({label})", w.id));
        }
        match reason {
            Reason::Expired => expired += 1,
            Reason::Lru => evicted += 1,
        }
    }

    let verb = if dry_run { "would remove" } else { "removed" };
    if expired + evicted == 0 && leftovers.is_empty() {
        ui::status(Status::Info, "nothing to clean up");
    } else {
        ui::status(
            Status::Ok,
            &format!("gc: {verb} {expired} expired, {evicted} evicted for space"),
        );
    }
    ui::detail(&format!("volume {:.1}% used", volume::volume_used_pct(r)?));
    Ok(())
}

fn stats(r: &Path, db: &rusqlite::Connection, json: bool) -> Result<()> {
    let total = volume::volume_total_bytes(r)?;
    let free = volume::volume_available_bytes(r)?;
    let used = total.saturating_sub(free);
    let used_pct = volume::volume_used_pct(r)?;
    let now = store::now();
    let active = store::list(db, "active")?;
    let expired = active.iter().filter(|w| w.expires_at() < now).count();
    let base = store::get_base_info(db)?;

    if json {
        println!(
            "{{\"root\":{},\"total_bytes\":{},\"used_bytes\":{},\"free_bytes\":{},\"used_pct\":{:.2},\"workspaces\":{},\"expired\":{},\"base_built_at\":{}}}",
            ui::json_string(&r.display().to_string()),
            total,
            used,
            free,
            used_pct,
            active.len(),
            expired,
            base.map(|b| b.built_at.to_string()).unwrap_or_else(|| "null".into())
        );
        return Ok(());
    }

    let on = ui::stdout_color();
    let base_line = match store::get_base_info(db)? {
        Some(b) => format!("built {} ago", ui::human_duration(now - b.built_at)),
        None => ui::dim("not built", on),
    };
    let expired_note = if expired > 0 {
        format!(" ({})", ui::yellow(&format!("{expired} expired"), on))
    } else {
        String::new()
    };
    println!("{}  {}", ui::bold("Volume", on), r.display());
    println!(
        "{}  {} {:.1}%  {} used of {}",
        ui::bold("Usage ", on),
        ui::cyan(&ui::usage_bar(used_pct, 20), on),
        used_pct,
        ui::human_bytes(used),
        ui::human_bytes(total)
    );
    println!("{}  {}", ui::bold("Free  ", on), ui::human_bytes(free));
    println!(
        "{}  {} active{}",
        ui::bold("Spaces", on),
        active.len(),
        expired_note
    );
    println!("{}  {}", ui::bold("Base  ", on), base_line);
    Ok(())
}

fn build_base(r: &Path, db: &rusqlite::Connection, repo: &str, setup_cmd: &str) -> Result<()> {
    let new = r.join("base.new");
    if new.exists() {
        std::fs::remove_dir_all(&new)?;
    }
    ui::status(Status::Info, &format!("cloning {repo}"));
    if !Command::new("git")
        .args(["clone", "--", repo])
        .arg(&new)
        .status()?
        .success()
    {
        bail!("git clone failed for {repo}");
    }
    run_setup(&new, setup_cmd)?;
    publish_base(r, db, &new, repo, setup_cmd)
}

/// Run the setup command inside a base tree. On failure the staged tree is removed.
fn run_setup(dir: &Path, setup_cmd: &str) -> Result<()> {
    ui::status(Status::Info, &format!("running setup: {setup_cmd}"));
    let ok = Command::new("sh")
        .arg("-c")
        .arg(setup_cmd)
        .current_dir(dir)
        .status()?
        .success();
    if !ok {
        std::fs::remove_dir_all(dir)?;
        bail!("setup command failed: {setup_cmd}");
    }
    Ok(())
}

/// Swap a staged base tree in as `base/` and record its metadata.
fn publish_base(
    r: &Path,
    db: &rusqlite::Connection,
    staged: &Path,
    repo: &str,
    setup_cmd: &str,
) -> Result<()> {
    let base = r.join("base");
    let old = r.join("base.old");
    // Record git HEAD and lockfile hash before swapping
    let head = lockfile::get_git_head(staged)?;
    let lockfile_hash = lockfile::compute_lockfile_hash(staged)?;
    let built_at = store::now();

    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    if base.exists() {
        std::fs::rename(&base, &old)?;
    }
    std::fs::rename(staged, &base)?;
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    store::set_base_info(
        db,
        built_at,
        head.clone(),
        lockfile_hash.clone(),
        Some(repo.to_string()),
        Some(setup_cmd.to_string()),
    )?;

    ui::status(Status::Ok, "base ready");
    if let Some(h) = &head {
        ui::detail(&format!("HEAD: {h}"));
    }
    if let Some(lh) = &lockfile_hash {
        ui::detail(&format!("lockfile hash: {lh}"));
    }
    let findings = base_check::check_base(&base)?;
    print_findings(&base, &findings);
    Ok(())
}

/// Bring the base up to date with its remote.
///
/// Fast path: clonefile the existing base (metadata only, no data copied), fetch and reset
/// the copy to the remote's default branch, and re-run setup only if a lockfile changed.
/// If the fast path fails for any reason, fall back to a full rebuild from the repo.
fn refresh_base(r: &Path, db: &rusqlite::Connection) -> Result<()> {
    let base = r.join("base");
    if !base.exists() {
        return Err(CliError::no_base().into());
    }
    let info = store::get_base_info(db)?;
    let Some((repo, setup_cmd)) = info
        .as_ref()
        .and_then(|i| i.repo.clone().zip(i.setup_cmd.clone()))
    else {
        return Err(CliError::new(
            1,
            "this base was built before repo tracking, so it cannot be refreshed",
        )
        .with_hint("rebuild it once with `replica base build --repo <url> --setup \"<cmd>\"`")
        .into());
    };
    let prev_lock = info.and_then(|i| i.lockfile_hash);

    let new = r.join("base.new");
    if new.exists() {
        std::fs::remove_dir_all(&new)?;
    }
    ui::status(Status::Info, "updating base from its remote (fast path)");
    clone::clone_tree(&base, &new)?;
    if let Err(e) = fetch_and_reset(&new) {
        ui::status(
            Status::Warn,
            &format!("fast path failed ({e:#}); rebuilding from the repo"),
        );
        std::fs::remove_dir_all(&new)?;
        return build_base(r, db, &repo, &setup_cmd);
    }

    let lock = lockfile::compute_lockfile_hash(&new)?;
    if lock != prev_lock {
        ui::status(Status::Info, "lockfiles changed, so setup runs again");
        run_setup(&new, &setup_cmd)?;
    } else {
        ui::status(Status::Info, "lockfiles unchanged, so setup is skipped");
    }
    publish_base(r, db, &new, &repo, &setup_cmd)
}

/// `git fetch` and move the checkout to the remote's default branch.
fn fetch_and_reset(dir: &Path) -> Result<()> {
    let git = |args: &[&str]| -> Result<()> {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
        if !out.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    };
    git(&["fetch", "--quiet", "--prune", "origin"])?;
    git(&["reset", "--quiet", "--hard", "origin/HEAD"])
}

fn status_base(r: &Path, db: &rusqlite::Connection, json: bool) -> Result<()> {
    let base = r.join("base");
    let on = ui::stdout_color();
    if !base.exists() {
        if json {
            println!("{{\"built\":false}}");
        } else {
            ui::status(Status::Info, "no base built yet");
            ui::detail("run `replica base build --repo <url> --setup \"<cmd>\"`");
        }
        return Ok(());
    }
    let Some(info) = store::get_base_info(db)? else {
        if json {
            println!("{{\"built\":true,\"tracked\":false}}");
        } else {
            ui::status(Status::Warn, "base exists but has no metadata");
            ui::detail(
                "it was built before metadata tracking; rebuild it with `replica base build`",
            );
        }
        return Ok(());
    };

    let now = store::now();
    let age = now - info.built_at;
    let (stale, reasons) = match &info.head {
        Some(head) => {
            let (head_changed, lockfile_changed, cur_head, cur_lock) =
                lockfile::is_base_stale(&base, head, info.lockfile_hash.as_deref())?;
            let mut reasons = Vec::new();
            if head_changed {
                reasons.push(format!(
                    "git HEAD changed (now {})",
                    cur_head.as_deref().unwrap_or("unknown")
                ));
            }
            if lockfile_changed {
                reasons.push(format!(
                    "lockfiles changed (now {})",
                    cur_lock.as_deref().unwrap_or("removed")
                ));
            }
            (Some(head_changed || lockfile_changed), reasons)
        }
        None => (None, Vec::new()),
    };

    if json {
        let opt = |o: &Option<String>| {
            o.as_deref()
                .map(ui::json_string)
                .unwrap_or_else(|| "null".into())
        };
        let reasons_json: Vec<String> = reasons.iter().map(|s| ui::json_string(s)).collect();
        println!(
            "{{\"built\":true,\"tracked\":true,\"built_at\":{},\"age_secs\":{},\"head\":{},\"lockfile_hash\":{},\"repo\":{},\"setup_cmd\":{},\"stale\":{},\"stale_reasons\":[{}]}}",
            info.built_at,
            age,
            opt(&info.head),
            opt(&info.lockfile_hash),
            opt(&info.repo),
            opt(&info.setup_cmd),
            match stale {
                Some(true) => "true",
                Some(false) => "false",
                None => "null",
            },
            reasons_json.join(",")
        );
        return Ok(());
    }

    let key = |k: &str| ui::bold(&format!("{k:<10}"), on);
    println!("{}{} ago", key("built"), ui::human_duration(age));
    if let Some(h) = &info.head {
        println!("{}{}", key("HEAD"), &h[..h.len().min(12)]);
    }
    if let Some(lh) = &info.lockfile_hash {
        println!("{}{}", key("lockfiles"), &lh[..lh.len().min(12)]);
    }
    if let Some(repo) = &info.repo {
        println!("{}{}", key("repo"), repo);
    }
    if let Some(cmd) = &info.setup_cmd {
        println!("{}{}", key("setup"), cmd);
    }
    match stale {
        Some(true) => {
            println!("{}{}", key("status"), ui::red("STALE", on));
            for reason in &reasons {
                println!("{}- {}", " ".repeat(10), reason);
            }
            println!("{}run `replica base refresh`", " ".repeat(10));
        }
        Some(false) => println!("{}{}", key("status"), ui::green("fresh", on)),
        None => println!(
            "{}{}",
            key("status"),
            ui::dim("unknown (no git HEAD recorded)", on)
        ),
    }
    Ok(())
}

fn check_base(r: &Path) -> Result<()> {
    let base = r.join("base");
    if !base.exists() {
        return Err(CliError::no_base().into());
    }
    let findings = base_check::check_base(&base)?;
    if findings.is_empty() {
        ui::status(Status::Ok, "base check: no path-dependent content found");
        return Ok(());
    }
    ui::status(
        Status::Warn,
        &format!("base check: found {} issue(s)", findings.len()),
    );
    print_findings(&base, &findings);
    Ok(())
}

fn print_findings(base: &Path, findings: &[base_check::Finding]) {
    for f in findings {
        let kind = format!("{:?}", f.kind)
            .to_lowercase()
            .replace("findingkind::", "");
        let rel = f.path.strip_prefix(base).unwrap_or(&f.path).display();
        ui::status(Status::Warn, &format!("[{kind}] {rel}: {}", f.detail));
        ui::detail(&format!("fix: {}", f.kind.suggestion()));
    }
}
