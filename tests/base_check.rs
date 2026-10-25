//! Integration tests for base check functionality.
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn nook_bin() -> PathBuf {
    // Use the binary from the current build
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir).join("target/debug/nook")
}

fn run_nook(args: &[&str], nook_root: &std::path::Path) -> std::process::Output {
    Command::new(nook_bin())
        .args(args)
        .env("NOOK_ROOT", nook_root)
        .output()
        .expect("failed to run nook")
}

#[test]
fn base_check_detects_absolute_symlink_and_venv() {
    let d = tempdir().unwrap();
    let nook_root = d.path().join("nook_root");
    fs::create_dir_all(&nook_root).unwrap();
    let base = nook_root.join("base");
    fs::create_dir_all(base.join("sub")).unwrap();
    fs::write(base.join("sub/file.txt"), b"hello").unwrap();

    // Create absolute symlink
    let base_canon = fs::canonicalize(&base).unwrap();
    let link = base.join("link_to_sub");
    symlink(base_canon.join("sub"), &link).unwrap();

    // Create venv with absolute path
    fs::create_dir_all(base.join("venv/bin")).unwrap();
    fs::write(
        base.join("venv/bin/activate"),
        format!(
            "VIRTUAL_ENV=\"{}\"\nexport VIRTUAL_ENV\n",
            base_canon.display()
        ),
    )
    .unwrap();

    // Create pyvenv.cfg
    fs::write(
        base.join("pyvenv.cfg"),
        format!(
            "home = {}\ninclude-system-site-packages = false\n",
            base_canon.display()
        ),
    )
    .unwrap();

    // Run base check
    let output = run_nook(&["base", "check"], &nook_root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stderr: {}", stderr);

    assert!(output.status.success(), "base check should exit 0");
    assert!(stderr.contains("absolutesymlink") || stderr.contains("absolute-symlink"));
    assert!(stderr.contains("venvscript") || stderr.contains("venv-script"));
    assert!(stderr.contains("pyvenvcfg") || stderr.contains("pyvenv-cfg"));
}

#[test]
fn base_check_clean_tree_no_warnings() {
    let d = tempdir().unwrap();
    let nook_root = d.path().join("nook_root");
    fs::create_dir_all(&nook_root).unwrap();
    let base = nook_root.join("base");
    fs::create_dir_all(base.join("src")).unwrap();
    fs::write(base.join("src/main.rs"), b"fn main() {}").unwrap();
    fs::write(
        base.join("Cargo.toml"),
        b"[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // Create relative symlink (should not trigger warning)
    fs::create_dir_all(base.join("sub")).unwrap();
    symlink("sub", base.join("link_to_sub")).unwrap();

    // Run base check
    let output = run_nook(&["base", "check"], &nook_root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stderr: {}", stderr);

    assert!(output.status.success(), "base check should exit 0");
    assert!(stderr.contains("no path-dependent content found"));
}

#[test]
fn new_with_post_clone_hook() {
    let d = tempdir().unwrap();
    let nook_root = d.path().join("nook_root");
    fs::create_dir_all(&nook_root).unwrap();
    let base = nook_root.join("base");
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("file.txt"), b"hello").unwrap();

    // Create a post-clone script that creates a marker file
    let marker = d.path().join("post_clone_ran");

    let output = run_nook(
        &[
            "new",
            "--ttl",
            "1h",
            "--post-clone",
            &format!("touch {}", marker.display()),
        ],
        &nook_root,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {}", stdout);
    eprintln!("stderr: {}", stderr);

    assert!(output.status.success(), "nook new should succeed");
    let ws_path = stdout.trim();
    assert!(
        std::path::Path::new(ws_path).exists(),
        "workspace should exist"
    );
    assert!(marker.exists(), "post-clone hook should have run");
}
