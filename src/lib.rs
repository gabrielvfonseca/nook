//! Nook: fast because it refuses to do the work.
#[cfg(not(target_os = "macos"))]
compile_error!("nook targets macOS (APFS clonefile) only");

pub mod clone;
pub mod gc;
pub mod store;
pub mod util;
