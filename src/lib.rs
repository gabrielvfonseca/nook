//! Replica: fast because it refuses to do the work.
#[cfg(not(target_os = "macos"))]
compile_error!("replica targets macOS (APFS clonefile) only");

pub mod base_check;
pub mod clone;
pub mod error;
pub mod gc;
pub mod help;
pub mod lockfile;
pub mod setup;
pub mod store;
pub mod ui;
pub mod util;
pub mod volume;
