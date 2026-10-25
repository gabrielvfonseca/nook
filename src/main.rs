use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use nook::{clone, gc, store, util};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "nook", version, about = "Fast because it refuses to do the work.")]
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
    let ws_root = r.join("ws");
    std::fs::create_dir_all(&ws_root)?;
    std::fs::create_dir_all(r.join("cache"))?;
    let db = store::open(&r.join("nook.db"))?;

    match cli.cmd {
        Cmd::New { ttl, name } => {
            let ttl = util::parse_ttl(&ttl)?;
            let base = r.join("base");
            if !base.exists() {
                bail!("no base; run `nook base build` first");
            }
            let id = util::new_id();
            let dst = ws_root.join(&id);
            clone::clone_tree(&base, &dst)?;
            std::fs::create_dir_all(dst.join(".nook/tmp"))?;
            store::insert(&db, &id, name.as_deref(), &dst.to_string_lossy(), ttl)?;
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
            println!("volume used: {:.1}%", gc::volume_used_pct(&r)?);
            println!("workspaces: {}", store::list(&db, "active")?.len());
        }
        Cmd::Base { cmd } => match cmd {
            BaseCmd::Build { repo, setup } => build_base(&r, &repo, &setup)?,
            BaseCmd::Status => println!("base exists: {}", r.join("base").exists()),
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
    // Retry anything left in `deleting` by a crash.
    for w in store::list(db, "deleting")? {
        if !dry_run {
            gc::safe_remove(ws_root, Path::new(&w.path))?;
            store::delete(db, &w.id)?;
        }
    }
    let now = store::now();
    let active = store::list(db, "active")?;
    let mut victims: Vec<&store::Ws> = active.iter().filter(|w| w.created_at + w.ttl_secs < now).collect();
    let over_high = gc::volume_used_pct(r)? > high;
    if over_high {
        // list() is ordered least-recently-used first; skip anything touched in the last 10 min.
        for w in &active {
            if now - w.last_used > 600 && !victims.iter().any(|v| v.id == w.id) {
                victims.push(w);
            }
        }
    }
    let mut ttl_count = active.iter().filter(|w| w.created_at + w.ttl_secs < now).count();
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
        } else if gc::volume_used_pct(r)? < low {
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
    if !Command::new("git").args(["clone", "--", repo]).arg(&new).status()?.success() {
        bail!("git clone failed");
    }
    if !Command::new("sh").arg("-c").arg(setup).current_dir(&new).status()?.success() {
        bail!("setup command failed");
    }
    if base.exists() {
        std::fs::rename(&base, &old)?;
    }
    std::fs::rename(&new, &base)?;
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    eprintln!("base built");
    Ok(())
}
