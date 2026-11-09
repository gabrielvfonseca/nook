use crate::error::{CliError, EXIT_CROSS_VOLUME, EXIT_DISK_FULL, EXIT_GENERIC};
use anyhow::Result;
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
        return Err(match e.raw_os_error() {
            Some(libc::EXDEV) => CliError::new(
                EXIT_CROSS_VOLUME,
                "base and workspace are on different volumes",
            )
            .with_hint("keep REPLICA_ROOT on the same APFS volume as the base tree")
            .into(),
            Some(libc::ENOSPC) => CliError::new(EXIT_DISK_FULL, "the volume is out of space")
                .with_hint("run `replica gc`, or free space on the volume")
                .into(),
            Some(libc::ENOTSUP) => CliError::new(
                EXIT_GENERIC,
                "filesystem does not support clonefile (not APFS?)",
            )
            .with_hint("set REPLICA_ROOT to a directory on an APFS volume")
            .into(),
            _ => CliError::new(EXIT_GENERIC, format!("clonefile failed: {e}")).into(),
        });
    }
    Ok(())
}
