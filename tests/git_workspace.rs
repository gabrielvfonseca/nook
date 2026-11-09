//! Integration tests for the git behaviour that agents rely on:
//! a workspace is a full git checkout that can commit and push, the base stays untouched,
//! and `base refresh` picks up new remote commits without re-running setup unless a lockfile changed.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn replica(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_replica"))
        .args(args)
        .env("REPLICA_ROOT", root)
        .env("NO_COLOR", "1")
        .output()
        .expect("run replica")
}

fn ok(o: &std::process::Output) -> String {
    assert!(
        o.status.success(),
        "replica failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// Bare origin with one commit on `main`, plus a working clone we use to push more commits.
struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    origin: PathBuf,
    author: PathBuf,
}

fn fixture(setup_marker: bool) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let origin = base_dir.join("origin.git");
    let author = base_dir.join("author");
    let root = base_dir.join("root");
    fs::create_dir_all(&root).unwrap();

    Command::new("git")
        .args(["init", "-q", "--bare", "-b", "main"])
        .arg(&origin)
        .status()
        .unwrap();
    Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .arg(&author)
        .status()
        .unwrap();
    fs::write(author.join("src.txt"), "v1\n").unwrap();
    fs::write(author.join("package-lock.json"), "{\"lock\":1}\n").unwrap();
    git(&author, &["add", "-A"]);
    git(&author, &["commit", "-qm", "initial"]);
    git(
        &author,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&author, &["push", "-q", "origin", "main"]);
    // Make origin/HEAD resolve on clones of this bare repo.
    git(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);

    let setup = if setup_marker {
        "echo run >> setup-runs.txt && touch installed"
    } else {
        "touch installed"
    };
    let out = replica(
        &root,
        &[
            "base",
            "build",
            "--repo",
            origin.to_str().unwrap(),
            "--setup",
            setup,
        ],
    );
    ok(&out);
    Fixture {
        _tmp: tmp,
        root,
        origin,
        author,
    }
}

fn new_workspace(root: &Path) -> PathBuf {
    let out = replica(root, &["new", "--name", "agent-task"]);
    PathBuf::from(ok(&out))
}

#[test]
fn workspace_commits_and_pushes_without_touching_base() {
    let f = fixture(false);
    let base = f.root.join("base");
    let base_head_before = git(&base, &["rev-parse", "HEAD"]);

    let ws = new_workspace(&f.root);
    assert!(ws.join(".git").exists(), "workspace must be a git checkout");
    assert!(
        ws.join("installed").exists(),
        "setup output must be cloned too"
    );

    fs::write(ws.join("src.txt"), "agent change\n").unwrap();
    git(&ws, &["checkout", "-q", "-b", "agent/fix-login"]);
    git(&ws, &["commit", "-qam", "agent: fix login"]);
    git(&ws, &["push", "-q", "origin", "agent/fix-login"]);

    // The commit is on the origin, under the branch the agent created.
    let remote_log = git(&f.origin, &["log", "-1", "--format=%s", "agent/fix-login"]);
    assert_eq!(remote_log, "agent: fix login");

    // The base is unchanged: same HEAD, clean tree, no agent branch.
    assert_eq!(git(&base, &["rev-parse", "HEAD"]), base_head_before);
    assert_eq!(
        git(&base, &["diff", "HEAD", "--stat"]),
        "",
        "no tracked changes in base"
    );
    assert!(git(&base, &["branch", "--list", "agent/fix-login"]).is_empty());
}

#[test]
fn refresh_fast_path_picks_up_new_commits_without_setup() {
    let f = fixture(true);
    let base = f.root.join("base");

    fs::write(f.author.join("src.txt"), "v2\n").unwrap();
    git(&f.author, &["commit", "-qam", "second"]);
    git(&f.author, &["push", "-q", "origin", "main"]);
    let new_head = git(&f.author, &["rev-parse", "HEAD"]);

    ok(&replica(&f.root, &["base", "refresh"]));

    assert_eq!(git(&base, &["rev-parse", "HEAD"]), new_head);
    assert_eq!(
        fs::read_to_string(base.join("setup-runs.txt"))
            .unwrap()
            .lines()
            .count(),
        1,
        "setup must not re-run when no lockfile changed"
    );
}

#[test]
fn refresh_reruns_setup_when_a_lockfile_changes() {
    let f = fixture(true);
    let base = f.root.join("base");

    fs::write(f.author.join("package-lock.json"), "{\"lock\":2}\n").unwrap();
    git(&f.author, &["commit", "-qam", "bump lock"]);
    git(&f.author, &["push", "-q", "origin", "main"]);

    ok(&replica(&f.root, &["base", "refresh"]));

    assert_eq!(
        fs::read_to_string(base.join("setup-runs.txt"))
            .unwrap()
            .lines()
            .count(),
        2,
        "setup must re-run when a lockfile changed"
    );
    let status = ok(&replica(&f.root, &["base", "status", "--json"]));
    assert!(status.contains("\"stale\":false"), "{status}");
}

#[test]
fn ref_branch_matching_lockfile_clones_directly() {
    let f = fixture(false); // setup = "touch installed"
    let root = &f.root;

    // Create a dep-bump branch on origin with a different lockfile.
    git(&f.author, &["checkout", "-q", "-b", "dep-bump"]);
    fs::write(f.author.join("package-lock.json"), "{\"lock\":2}\n").unwrap();
    git(&f.author, &["add", "-A"]);
    git(&f.author, &["commit", "-qm", "bump lock on dep-bump"]);
    git(&f.author, &["push", "-q", "origin", "dep-bump"]);

    // --ref main matches the base lockfile -> cloned directly.
    let out = replica(root, &["new", "--ttl", "1h", "--ref", "main"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "replica new --ref main failed: {stderr}"
    );
    assert!(
        stderr.contains("cloned directly"),
        "expected 'cloned directly', got: {stderr}"
    );

    // --ref dep-bump has a different lockfile -> drift -> incremental install.
    let out = replica(root, &["new", "--ttl", "1h", "--ref", "dep-bump"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "replica new --ref dep-bump failed: {stderr}"
    );
    assert!(
        stderr.contains("lockfile drift"),
        "expected 'lockfile drift', got: {stderr}"
    );
    assert!(
        stderr.contains("running incremental install"),
        "expected 'running incremental install', got: {stderr}"
    );
}
