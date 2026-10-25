use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use nook::{base_check, clone, gc, lockfile, store, util, volume};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "nook",
    version,
    about = "Fast because it refuses to do the work."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Manage the golden base tree
    Base {
        #[command(subcommand)]
        cmd: BaseCmd,
    },
    /// Clone the base into a new workspace (prints its path)
    New {
        #[arg(long, default_value = "2h")]
        ttl: String,
        #[arg(long)]
        name: Option<String>,
        /// Run a command in the new workspace after cloning (for project-specific fixups)
        #[arg(long)]
        post_clone: Option<String>,
        /// Fail if base is stale (behind remote or lockfiles changed)
        #[arg(long)]
        require_fresh: bool,
    },
    /// Print export lines for a workspace
    Env { id: String },
    /// List active workspaces
    List,
    /// Mark a workspace as recently used
    Touch { id: String },
    /// Delete workspaces
    Rm { ids: Vec<String> },
    /// Expire by TTL, then evict LRU if the volume is too full
    Gc {
        #[arg(long)]
        dry_run: bool,
        /// Start evicting LRU workspaces above this volume usage (%)
        #[arg(long, default_value_t = 80.0)]
        high: f64,
        /// Stop evicting once usage falls below this (%)
        #[arg(long, default_value_t = 60.0)]
        low: f64,
    },
    /// Show real volume usage
    Stats,
}

#[derive(Subcommand)]
enum BaseCmd {
    /// git clone a repo, run a setup command, atomically swap in as base
    Build {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        setup: String,
    },
    /// Scan the golden tree for absolute paths that will break when cloned
    Check,
    /// Rebuild base side-by-side and atomically swap (keeps old base for existing clones)
    Refresh,
    /// Show base info: age, HEAD, lockfile hash, stale status
    Status,
}

fn root() -> PathBuf {
    PathBuf::from(std::env::var("NOOK_ROOT").unwrap_or_else(|_| "/Volumes/Work/nook".into()))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("nook: {e:#}");
        let msg = format!("{e:#}");
        let code = if msg.contains("different volumes") {
            3
        } else if msg.contains("no base") {
            2
        } else {
            1
        };
        std::process::exit(code);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let r = root();

    // Verify NOOK_ROOT is on APFS
    volume::verify_apfs(&r)?;

    let ws_root = r.join("ws");
    std::fs::create_dir_all(&ws_root)?;
    std::fs::create_dir_all(r.join("cache"))?;
    let db = store::open(&r.join("nook.db"))?;

    // For commands that need base, verify base and ws_root are on same volume
    let needs_base = matches!(
        cli.cmd,
        Cmd::New { .. }
            | Cmd::Base {
                cmd: BaseCmd::Build { .. } | BaseCmd::Check | BaseCmd::Refresh | BaseCmd::Status
            }
    );
    if needs_base {
        let base = r.join("base");
        if base.exists() {
            volume::verify_same_volume(&base, &ws_root)?;
        }
    }

    match cli.cmd {
        Cmd::New {
            ttl,
            name,
            post_clone,
            require_fresh,
        } => {
            let ttl = util::parse_ttl(&ttl)?;
            let base = r.join("base");
            if !base.exists() {
                bail!("no base; run `nook base build` first");
            }
            // Check if base is stale
            let base_info = store::get_base_info(&db)?;
            let base_hash = base_info.as_ref().and_then(|i| i.lockfile_hash.clone());
            if let Some(ref info) = base_info {
                if let (Some(head), lockfile_hash) =
                    (info.head.as_deref(), info.lockfile_hash.as_deref())
                {
                    let (head_changed, lockfile_changed, current_head, current_lockfile_hash) =
                        lockfile::is_base_stale(&base, head, lockfile_hash)?;
                    if head_changed || lockfile_changed {
                        let msg = if head_changed && lockfile_changed {
                            "base is stale: git HEAD and lockfiles have changed"
                        } else if head_changed {
                            "base is stale: git HEAD has changed (run `nook base refresh`)"
                        } else {
                            "base is stale: lockfiles have changed (run `nook base refresh`)"
                        };
                        if require_fresh {
                            bail!("{}", msg);
                        } else {
                            eprintln!("warning: {}", msg);
                            if let Some(h) = current_head {
                                eprintln!("  current HEAD: {}", h);
                            }
                            if let Some(lh) = current_lockfile_hash {
                                eprintln!("  current lockfile hash: {}", lh);
                            }
                        }
                    }
                }
            }
            // Check NOOK_MAX_BYTES cap if set
            if let Ok(max_bytes_str) = std::env::var("NOOK_MAX_BYTES") {
                if let Ok(max_bytes) = max_bytes_str.parse::<u64>() {
                    let used = volume::volume_used_bytes(&r)?;
                    if used >= max_bytes {
                        bail!(
                            "disk cap reached: {:.1} GB used of {:.1} GB limit (set NOOK_MAX_BYTES or run gc)",
                            used as f64 / 1e9,
                            max_bytes as f64 / 1e9
                        );
                    }
                }
            }
            let id = util::new_id();
            let dst = ws_root.join(&id);
            clone::clone_tree(&base, &dst)?;
            std::fs::create_dir_all(dst.join(".nook/tmp"))?;
            store::insert(
                &db,
                &id,
                name.as_deref(),
                &dst.to_string_lossy(),
                ttl,
                base_hash,
            )?;
            if let Some(cmd) = post_clone {
                eprintln!("Running post-clone hook: {}", cmd);
                if !Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .current_dir(&dst)
                    .status()?
                    .success()
                {
                    bail!("post-clone hook failed");
                }
            }
            println!("{}", dst.display()); // stdout is the path only
        }
        Cmd::Env { id } => {
            let p = ws_root.join(&id);
            println!("export NOOK_WS={}", p.display());
            println!("export TMPDIR={}/.nook/tmp", p.display());
            println!("export CARGO_HOME={}/cache/cargo", r.display());
            println!("export npm_config_cache={}/cache/npm", r.display());
            println!("export SCCACHE_DIR={}/cache/sccache", r.display());
        }
        Cmd::List => {
            for w in store::list(&db, "active")? {
                let left = w.created_at + w.ttl_secs - store::now();
                println!("{}\t{}\tttl_left={}s", w.id, w.path, left);
            }
        }
        Cmd::Touch { id } => store::touch(&db, &id)?,
        Cmd::Rm { ids } => {
            for id in ids {
                store::set_state(&db, &id, "deleting")?;
                gc::safe_remove(&ws_root, &ws_root.join(&id))?;
                store::delete(&db, &id)?;
            }
        }
        Cmd::Gc { dry_run, high, low } => run_gc(&db, &r, &ws_root, dry_run, high, low)?,
        Cmd::Stats => {
            println!("volume used: {:.1}%", volume::volume_used_pct(&r)?);
            println!("workspaces: {}", store::list(&db, "active")?.len());
        }
        Cmd::Base { cmd } => match cmd {
            BaseCmd::Build { repo, setup } => build_base(&r, &repo, &setup)?,
            BaseCmd::Check => check_base(&r)?,
            BaseCmd::Refresh => refresh_base(&r, &db)?,
            BaseCmd::Status => status_base(&r, &db)?,
        },
    }
    Ok(())
}

fn run_gc(
    db: &rusqlite::Connection,
    r: &Path,
    ws_root: &Path,
    dry_run: bool,
    high: f64,
    low: f64,
) -> Result<()> {
    // Check NOOK_MAX_BYTES cap if set - use as effective high-water mark
    let max_bytes = std::env::var("NOOK_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let effective_high = if let Some(max) = max_bytes {
        let total = volume::volume_total_bytes(r)?;
        (max as f64 / total as f64 * 100.0).min(high)
    } else {
        high
    };

    // Retry anything left in `deleting` by a crash.
    for w in store::list(db, "deleting")? {
        if !dry_run {
            gc::safe_remove(ws_root, Path::new(&w.path))?;
            store::delete(db, &w.id)?;
        }
    }
    let now = store::now();
    let active = store::list(db, "active")?;
    let mut victims: Vec<&store::Ws> = active
        .iter()
        .filter(|w| w.created_at + w.ttl_secs < now)
        .collect();
    let over_high = volume::volume_used_pct(r)? > effective_high;
    if over_high {
        // list() is ordered least-recently-used first; skip anything touched in the last 10 min.
        for w in &active {
            if now - w.last_used > 600 && !victims.iter().any(|v| v.id == w.id) {
                victims.push(w);
            }
        }
    }
    let mut ttl_count = active
        .iter()
        .filter(|w| w.created_at + w.ttl_secs < now)
        .count();
    for w in victims {
        eprintln!("evict {}", w.id);
        if dry_run {
            continue;
        }
        store::set_state(db, &w.id, "deleting")?;
        gc::safe_remove(ws_root, Path::new(&w.path))?;
        store::delete(db, &w.id)?;
        // Always finish TTL victims; stop LRU victims once below the low-water mark.
        if ttl_count > 0 {
            ttl_count -= 1;
        } else if volume::volume_used_pct(r)? < low {
            break;
        }
    }
    Ok(())
}

fn build_base(r: &Path, repo: &str, setup: &str) -> Result<()> {
    let new = r.join("base.new");
    let base = r.join("base");
    let old = r.join("base.old");
    for p in [&new, &old] {
        if p.exists() {
            std::fs::remove_dir_all(p)?;
        }
    }
    if !Command::new("git")
        .args(["clone", "--", repo])
        .arg(&new)
        .status()?
        .success()
    {
        bail!("git clone failed");
    }
    if !Command::new("sh")
        .arg("-c")
        .arg(setup)
        .current_dir(&new)
        .status()?
        .success()
    {
        bail!("setup command failed");
    }
    // Record git HEAD and lockfile hash before swapping
    let head = lockfile::get_git_head(&new)?;
    let lockfile_hash = lockfile::compute_lockfile_hash(&new)?;
    let built_at = store::now();

    if base.exists() {
        std::fs::rename(&base, &old)?;
    }
    std::fs::rename(&new, &base)?;
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    // Store base info
    let db = store::open(&r.join("nook.db"))?;
    store::set_base_info(&db, built_at, head.clone(), lockfile_hash.clone())?;

    eprintln!("base built");
    if let Some(h) = &head {
        eprintln!("  HEAD: {}", h);
    }
    if let Some(lh) = &lockfile_hash {
        eprintln!("  lockfile hash: {}", lh);
    }
    // Run check on the new base
    base_check::check_base(&base)?.into_iter().for_each(|f| {
        eprintln!(
            "  [{}] {}: {}",
            format!("{:?}", f.kind)
                .to_lowercase()
                .replace("findingkind::", ""),
            f.path.strip_prefix(&base).unwrap_or(&f.path).display(),
            f.detail
        );
        eprintln!("    -> suggestion: {}", f.kind.suggestion());
    });
    Ok(())
}

fn refresh_base(r: &Path, db: &rusqlite::Connection) -> Result<()> {
    let base = r.join("base");
    if !base.exists() {
        bail!("no base; run `nook base build` first");
    }
    // Get current base info to find the repo URL
    let base_info = store::get_base_info(db)?;
    let _head = base_info.as_ref().and_then(|i| i.head.clone());

    // We need to know the repo URL. For now, we'll require the user to specify it.
    // In a real implementation, we'd store the repo URL in the base table.
    bail!("base refresh not yet fully implemented: need to store repo URL in base table");
}

fn status_base(r: &Path, db: &rusqlite::Connection) -> Result<()> {
    let base = r.join("base");
    if !base.exists() {
        println!("base: not built");
        return Ok(());
    }
    let base_info = store::get_base_info(db)?;
    let info = match base_info {
        Some(i) => i,
        None => {
            println!("base: exists but no metadata (built before metadata tracking)");
            return Ok(());
        }
    };

    let age_secs = store::now() - info.built_at;
    let age_str = if age_secs < 60 {
        format!("{}s ago", age_secs)
    } else if age_secs < 3600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86400 {
        format!("{}h ago", age_secs / 3600)
    } else {
        format!("{}d ago", age_secs / 86400)
    };

    println!("base: built {}", age_str);
    if let Some(h) = &info.head {
        println!("  HEAD: {}", h);
    }
    if let Some(lh) = &info.lockfile_hash {
        println!("  lockfile hash: {}", lh);
    }

    // Check staleness
    if let Some(head) = &info.head {
        let (head_changed, lockfile_changed, current_head, current_lockfile_hash) =
            lockfile::is_base_stale(&base, head, info.lockfile_hash.as_deref())?;

        if head_changed || lockfile_changed {
            println!("  status: STALE");
            if head_changed {
                println!("    - git HEAD changed");
                if let Some(ch) = current_head {
                    println!("      current: {}", ch);
                }
            }
            if lockfile_changed {
                println!("    - lockfiles changed");
                if let Some(clh) = current_lockfile_hash {
                    println!("      current hash: {}", clh);
                }
            }
        } else {
            println!("  status: fresh");
        }
    } else {
        println!("  status: unknown (no git HEAD recorded)");
    }
    Ok(())
}

fn check_base(r: &Path) -> Result<()> {
    let base = r.join("base");
    if !base.exists() {
        bail!("no base; run `nook base build` first");
    }
    let findings = base_check::check_base(&base)?;
    if findings.is_empty() {
        eprintln!("base check: no path-dependent content found");
        return Ok(());
    }
    eprintln!("base check: found {} issue(s)", findings.len());
    for f in findings {
        eprintln!(
            "  [{}] {}: {}",
            format!("{:?}", f.kind)
                .to_lowercase()
                .replace("findingkind::", ""),
            f.path.strip_prefix(&base).unwrap_or(&f.path).display(),
            f.detail
        );
        eprintln!("    -> suggestion: {}", f.kind.suggestion());
    }
    Ok(())
}
