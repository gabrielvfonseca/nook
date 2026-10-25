use anyhow::{bail, Result};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Check if the given path is on an APFS filesystem
pub fn is_apfs(path: &Path) -> Result<bool> {
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        bail!("statfs failed: {}", std::io::Error::last_os_error());
    }
    // APFS filesystem type on macOS
    // f_fstypename is a fixed-size array, convert to string
    let fs_name = unsafe {
        std::ffi::CStr::from_ptr(s.f_fstypename.as_ptr())
            .to_string_lossy()
            .to_string()
    };
    Ok(fs_name == "apfs")
}

/// Get the device ID (st_dev) for a path
pub fn get_device_id(path: &Path) -> Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::stat(c.as_ptr(), &mut s) } != 0 {
        bail!("stat failed: {}", std::io::Error::last_os_error());
    }
    Ok(s.st_dev as u64)
}

/// Check that base and workspace root are on the same APFS volume
pub fn verify_same_volume(base: &Path, ws_root: &Path) -> Result<()> {
    let base_dev = get_device_id(base)?;
    let ws_dev = get_device_id(ws_root)?;
    if base_dev != ws_dev {
        bail!("base and workspace are on different volumes (exit 3)");
    }
    Ok(())
}

/// Check that a path is on APFS
pub fn verify_apfs(path: &Path) -> Result<()> {
    if !is_apfs(path)? {
        bail!("filesystem does not support clonefile (not APFS?)");
    }
    Ok(())
}

/// Get volume usage percentage
pub fn volume_used_pct(path: &Path) -> Result<f64> {
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        bail!("statvfs failed: {}", std::io::Error::last_os_error());
    }
    let total = s.f_blocks as f64;
    let avail = s.f_bavail as f64;
    Ok(100.0 * (1.0 - avail / total))
}

/// Get volume total bytes
pub fn volume_total_bytes(path: &Path) -> Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        bail!("statvfs failed: {}", std::io::Error::last_os_error());
    }
    Ok(s.f_blocks as u64 * s.f_frsize as u64)
}

/// Get volume available bytes
pub fn volume_available_bytes(path: &Path) -> Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        bail!("statvfs failed: {}", std::io::Error::last_os_error());
    }
    Ok(s.f_bavail as u64 * s.f_frsize as u64)
}

/// Get volume used bytes
pub fn volume_used_bytes(path: &Path) -> Result<u64> {
    let total = volume_total_bytes(path)?;
    let avail = volume_available_bytes(path)?;
    Ok(total.saturating_sub(avail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn volume_checks_work() {
        let d = tempdir().unwrap();
        // Should be APFS on macOS
        assert!(is_apfs(d.path()).unwrap_or(false));
        assert!(verify_apfs(d.path()).is_ok());

        // Same volume check
        assert!(verify_same_volume(d.path(), d.path()).is_ok());
    }

    #[test]
    fn volume_bytes() {
        let d = tempdir().unwrap();
        let total = volume_total_bytes(d.path()).unwrap();
        let avail = volume_available_bytes(d.path()).unwrap();
        let used = volume_used_bytes(d.path()).unwrap();
        assert!(total > 0);
        assert!(avail > 0);
        // Allow small difference due to concurrent FS activity between calls
        let expected_used = total.saturating_sub(avail);
        let diff = (used as i128 - expected_used as i128).unsigned_abs() as u64;
        assert!(
            diff < 1_000_000,
            "used={} expected={} diff={}",
            used,
            expected_used,
            diff
        );
    }
}
