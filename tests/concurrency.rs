//! Concurrency test: N simultaneous `replica new` and `replica rm` calls must
//! not produce SQLite busy errors, corruption, or lost workspaces.
//!
//! Runs the real `replica` binary against its own REPLICA_ROOT under a temp
//! dir (APFS on macOS runners/dev machines).
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

const N: usize = 10;

fn replica_bin() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir).join("target/debug/replica")
}

fn run_replica(args: &[&str], replica_root: &Path) -> std::process::Output {
    Command::new(replica_bin())
        .args(args)
        .env("REPLICA_ROOT", replica_root)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run replica")
}

/// Build a minimal base: git repo + setup that does nothing (so the test is
/// fast and hermetic). Returns the replica root.
fn setup_base() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let base = root.join("base");

    // A tiny git repo as the base source.
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("README.md"), b"# bench\n").unwrap();
    Command::new("git")
        .args(["-C", src.to_str().unwrap(), "init", "-q"])
        .status()
        .expect("git init");
    Command::new("git")
        .args([
            "-C",
            src.to_str().unwrap(),
            "-c",
            "user.name=replica-test",
            "-c",
            "user.email=test@example.com",
            "add",
            "-A",
        ])
        .status()
        .expect("git add");
    Command::new("git")
        .args([
            "-C",
            src.to_str().unwrap(),
            "-c",
            "user.name=replica-test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "test base",
        ])
        .status()
        .expect("git commit");

    // Build the base with a no-op setup so no package manager is needed.
    let out = run_replica(
        &[
            "base",
            "build",
            "--repo",
            src.to_str().unwrap(),
            "--setup",
            "true",
        ],
        &root,
    );
    assert!(
        out.status.success(),
        "base build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(base.exists(), "base directory was not created");

    (tmp, root)
}

/// Parse a workspace path printed on stdout by `replica new`.
fn ws_path_from_output(o: &std::process::Output) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())
}

#[test]
fn ten_concurrent_new_and_rm_without_errors() {
    let (_tmp, root) = setup_base();

    // --- Phase A: N concurrent `replica new` ---
    let new_ok = Arc::new(AtomicUsize::new(0));
    let new_err = Arc::new(AtomicUsize::new(0));
    let paths: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let new_ok = new_ok.clone();
        let new_err = new_err.clone();
        let paths = paths.clone();
        let root = root.clone();
        handles.push(std::thread::spawn(move || {
            let o = run_replica(&["new", "--ttl", "1h"], &root);
            if o.status.success() {
                new_ok.fetch_add(1, Ordering::SeqCst);
                paths.lock().unwrap().push(ws_path_from_output(&o));
            } else {
                new_err.fetch_add(1, Ordering::SeqCst);
                eprintln!("replica new failed: {}", String::from_utf8_lossy(&o.stderr));
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let new_ok = new_ok.load(Ordering::SeqCst);
    let new_err = new_err.load(Ordering::SeqCst);
    eprintln!("new: ok={new_ok} err={new_err}");
    assert_eq!(new_err, 0, "no `replica new` should fail under concurrency");
    assert_eq!(new_ok, N, "all N workspaces should be created");

    let paths = Arc::try_unwrap(paths).unwrap().into_inner().unwrap();
    assert_eq!(paths.len(), N);

    // Sanity: each created workspace exists on disk.
    for p in &paths {
        assert!(p.exists(), "workspace {p:?} should exist after creation");
    }

    // --- Phase B: touch each workspace (heartbeat / lease refresh) ---
    for p in &paths {
        let id = p.file_name().unwrap().to_str().unwrap();
        let o = run_replica(&["touch", id], &root);
        assert!(
            o.status.success(),
            "replica touch {id} failed: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }

    // --- Phase C: N concurrent `replica rm` ---
    let rm_ok = Arc::new(AtomicUsize::new(0));
    let rm_err = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(N);
    for p in &paths {
        let rm_ok = rm_ok.clone();
        let rm_err = rm_err.clone();
        let id = p.file_name().unwrap().to_str().unwrap().to_string();
        let root = root.clone();
        handles.push(std::thread::spawn(move || {
            let o = run_replica(&["rm", &id], &root);
            if o.status.success() {
                rm_ok.fetch_add(1, Ordering::SeqCst);
            } else {
                rm_err.fetch_add(1, Ordering::SeqCst);
                eprintln!(
                    "replica rm {id} failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let rm_ok = rm_ok.load(Ordering::SeqCst);
    let rm_err = rm_err.load(Ordering::SeqCst);
    eprintln!("rm: ok={rm_ok} err={rm_err}");
    assert_eq!(rm_err, 0, "no `replica rm` should fail under concurrency");
    assert_eq!(rm_ok, N, "all N workspaces should be removed");

    // --- Phase D: verify nothing remains on disk or in the database ---
    let ws_dir = root.join("ws");
    let remaining: Vec<_> = fs::read_dir(&ws_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        remaining.is_empty(),
        "ws/ should be empty after all removals, found {} entries",
        remaining.len()
    );

    // The database should have no active rows.
    let o = run_replica(&["list", "--json"], &root);
    assert!(o.status.success());
    let list_out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(list_out.trim(), "[]", "no active workspaces should remain");
}
