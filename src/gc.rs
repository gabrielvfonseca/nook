use anyhow::{bail, Result};
use rayon::prelude::*;
use std::fs;
use std::path::Path;

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

pub fn volume_used_pct(path: &Path) -> Result<f64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        bail!("statvfs failed: {}", std::io::Error::last_os_error());
    }
    let total = s.f_blocks as f64;
    let avail = s.f_bavail as f64;
    Ok(100.0 * (1.0 - avail / total))
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn used_pct_in_range() {
        let p = volume_used_pct(Path::new("/")).unwrap();
        assert!((0.0..=100.0).contains(&p));
    }
}
