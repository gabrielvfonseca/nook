use anyhow::{bail, Result};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

extern "C" {
    fn clonefile(src: *const libc::c_char, dst: *const libc::c_char, flags: u32) -> libc::c_int;
}
const CLONE_NOFOLLOW: u32 = 0x0001;

/// Clone `src` to `dst` via APFS copy-on-write. Fails (never falls back to copy)
/// if dst exists or src/dst are on different volumes.
pub fn clone_tree(src: &Path, dst: &Path) -> Result<()> {
    let s = CString::new(src.as_os_str().as_bytes())?;
    let d = CString::new(dst.as_os_str().as_bytes())?;
    let rc = unsafe { clonefile(s.as_ptr(), d.as_ptr(), CLONE_NOFOLLOW) };
    if rc != 0 {
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::EXDEV) => bail!("base and workspace are on different volumes (exit 3)"),
            Some(libc::ENOTSUP) => bail!("filesystem does not support clonefile (not APFS?)"),
            _ => bail!("clonefile failed: {e}"),
        }
    }
    Ok(())
}
