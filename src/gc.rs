use anyhow::{bail, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Lease file marker: a workspace is "in use" while its lease file is fresh.
/// `replica gc` skips fresh-leased workspaces so it never deletes a workspace
/// an agent is actively using. The lease is created by `replica new`,
/// refreshed by `replica touch`, and removed by `replica rm`.
pub const LEASE_TTL_SECS: i64 = 600; // 10 minutes, matches the gc LRU window

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Path of the lease file for a workspace: $ws_root/<id>/.replica/lease.
pub fn lease_path(ws_root: &Path, id: &str) -> PathBuf {
    ws_root.join(id).join(".replica").join("lease")
}

/// Create (or refresh) the lease file for a workspace, marking it in use.
pub fn create_lease(ws_root: &Path, id: &str) -> Result<()> {
    let p = lease_path(ws_root, id);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&p, now_secs().to_string())?;
    Ok(())
}

/// Refresh the lease file for a workspace (heartbeat from `replica touch`).
pub fn refresh_lease(ws_root: &Path, id: &str) -> Result<()> {
    create_lease(ws_root, id)
}

/// Remove the lease file for a workspace (called by `replica rm`).
pub fn remove_lease(ws_root: &Path, id: &str) -> Result<()> {
    let _ = fs::remove_file(lease_path(ws_root, id));
    Ok(())
}

/// True if the workspace has a fresh lease, i.e. it is in use and `gc` must
/// not remove it. A missing or stale lease means the workspace is safe to
/// remove.
pub fn lease_is_fresh(ws_root: &Path, id: &str) -> Result<bool> {
    let p = lease_path(ws_root, id);
    let md = match fs::metadata(&p) {
        Ok(md) => md,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let now = SystemTime::now();
    let mtime = md.modified()?;
    let age = now.duration_since(mtime).unwrap_or(Duration::MAX).as_secs() as i64;
    Ok(age < LEASE_TTL_SECS)
}

/// Parallel, symlink-safe recursive delete. Refuses paths outside `ws_root`.
pub fn safe_remove(ws_root: &Path, target: &Path) -> Result<()> {
    let root = fs::canonicalize(ws_root)?;
    let t = match fs::canonicalize(target) {
        Ok(t) => t,
        Err(_) => return Ok(()), // already gone
    };
    if !t.starts_with(&root) || t == root {
        bail!("refusing to delete outside workspace root: {}", t.display());
    }
    par_remove(&t)
}

fn par_remove(p: &Path) -> Result<()> {
    let md = fs::symlink_metadata(p)?;
    if md.is_dir() {
        let entries: Vec<_> = fs::read_dir(p)?.filter_map(|e| e.ok()).collect();
        entries.par_iter().try_for_each(|e| par_remove(&e.path()))?;
        fs::remove_dir(p)?;
    } else {
        fs::remove_file(p)?; // files and symlinks (never followed)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volume;
    use std::os::unix::fs::symlink;

    #[test]
    fn removes_nested_tree() {
        let root = tempfile::tempdir().unwrap();
        let t = root.path().join("a/b/c");
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("f"), b"x").unwrap();
        safe_remove(root.path(), &root.path().join("a")).unwrap();
        assert!(!root.path().join("a").exists());
    }

    #[test]
    fn refuses_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        assert!(safe_remove(root.path(), other.path()).is_err());
        assert!(other.path().exists());
    }

    #[test]
    fn refuses_root_itself() {
        let root = tempfile::tempdir().unwrap();
        assert!(safe_remove(root.path(), root.path()).is_err());
    }

    #[test]
    fn does_not_follow_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"x").unwrap();
        let ws = root.path().join("ws1");
        fs::create_dir_all(&ws).unwrap();
        symlink(outside.path(), ws.join("link")).unwrap();
        safe_remove(root.path(), &ws).unwrap();
        assert!(!ws.exists());
        assert!(outside.path().join("keep").exists());
    }

    #[test]
    fn missing_target_is_ok() {
        let root = tempfile::tempdir().unwrap();
        safe_remove(root.path(), &root.path().join("nope")).unwrap();
    }

    #[test]
    fn lease_missing_is_not_in_use() {
        let root = tempfile::tempdir().unwrap();
        assert!(!lease_is_fresh(root.path(), "ws1").unwrap());
    }

    #[test]
    fn lease_fresh_is_in_use() {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws1");
        fs::create_dir_all(&ws).unwrap();
        create_lease(root.path(), "ws1").unwrap();
        assert!(lease_is_fresh(root.path(), "ws1").unwrap());
    }

    #[test]
    fn lease_stale_is_not_in_use() {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws1");
        fs::create_dir_all(&ws).unwrap();
        create_lease(root.path(), "ws1").unwrap();
        let p = lease_path(root.path(), "ws1");
        // Backdate the lease by more than the TTL so it counts as stale.
        let old = SystemTime::now() - Duration::from_secs(LEASE_TTL_SECS as u64 + 60);
        let f = std::fs::File::open(&p).unwrap();
        f.set_modified(old).unwrap();
        assert!(!lease_is_fresh(root.path(), "ws1").unwrap());
    }

    #[test]
    fn lease_remove_then_missing() {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws1");
        fs::create_dir_all(&ws).unwrap();
        create_lease(root.path(), "ws1").unwrap();
        remove_lease(root.path(), "ws1").unwrap();
        assert!(!lease_is_fresh(root.path(), "ws1").unwrap());
    }

    #[test]
    fn used_pct_in_range() {
        let p = volume::volume_used_pct(Path::new("/")).unwrap();
        assert!((0.0..=100.0).contains(&p));
    }
}
