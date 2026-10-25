use anyhow::Result;
use std::fs;
use std::path::Path;

/// Known lockfile names to check
const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.lock",
    "poetry.lock",
    "uv.lock",
    "Pipfile.lock",
    "composer.lock",
    "Gemfile.lock",
    "mix.lock",
    "go.sum",
];

/// Compute a hash of all lockfiles in the given directory
pub fn compute_lockfile_hash(dir: &Path) -> Result<Option<String>> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut found_any = false;

    for lockfile in LOCKFILES {
        let path = dir.join(lockfile);
        if path.exists() {
            found_any = true;
            let content = fs::read(&path)?;
            hasher.update(&content);
            // Also include the filename to distinguish between different lockfiles with same content
            hasher.update(lockfile.as_bytes());
        }
    }

    if !found_any {
        return Ok(None);
    }

    Ok(Some(format!("{:x}", hasher.finalize())))
}

/// Get the current git commit hash (HEAD) for the repo at dir
pub fn get_git_head(dir: &Path) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        return Ok(None);
    }
    Ok(Some(hash))
}

/// Check if the base is stale compared to the remote
pub fn is_base_stale(
    base_dir: &Path,
    base_head: &str,
    base_lockfile_hash: Option<&str>,
) -> Result<(bool, bool, Option<String>, Option<String>)> {
    // Check git head
    let current_head = get_git_head(base_dir)?;
    let head_changed = current_head.as_ref() != Some(&base_head.to_string());

    // Check lockfile hash
    let current_lockfile_hash = compute_lockfile_hash(base_dir)?;
    let lockfile_changed = current_lockfile_hash.as_deref() != base_lockfile_hash;

    Ok((
        head_changed,
        lockfile_changed,
        current_head,
        current_lockfile_hash,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn lockfile_hash_empty_when_no_lockfiles() {
        let d = tempdir().unwrap();
        let hash = compute_lockfile_hash(d.path()).unwrap();
        assert!(hash.is_none());
    }

    #[test]
    fn lockfile_hash_detects_cargo_lock() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("Cargo.lock"), b"[package]\nname = \"test\"\n").unwrap();
        let hash = compute_lockfile_hash(d.path()).unwrap();
        assert!(hash.is_some());
        assert_eq!(hash.unwrap().len(), 64); // SHA256 hex
    }

    #[test]
    fn lockfile_hash_changes_with_content() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("Cargo.lock"), b"version 1").unwrap();
        let hash1 = compute_lockfile_hash(d.path()).unwrap();
        fs::write(d.path().join("Cargo.lock"), b"version 2").unwrap();
        let hash2 = compute_lockfile_hash(d.path()).unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn lockfile_hash_includes_filename() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("package-lock.json"), b"same content").unwrap();
        let hash1 = compute_lockfile_hash(d.path()).unwrap();
        fs::remove_file(d.path().join("package-lock.json")).unwrap();
        fs::write(d.path().join("yarn.lock"), b"same content").unwrap();
        let hash2 = compute_lockfile_hash(d.path()).unwrap();
        assert_ne!(hash1, hash2);
    }
}
