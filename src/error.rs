//! Errors that carry a process exit code and an optional hint.
//!
//! Exit codes (see `replica help exit-codes`):
//!   1  generic failure
//!   2  no base, or base is stale and `--require-fresh` was given
//!   3  base and workspaces are on different volumes
//!   4  disk full, or the REPLICA_MAX_BYTES cap is reached

use std::fmt;

pub const EXIT_GENERIC: i32 = 1;
pub const EXIT_NO_BASE: i32 = 2;
pub const EXIT_CROSS_VOLUME: i32 = 3;
pub const EXIT_DISK_FULL: i32 = 4;

#[derive(Debug)]
pub struct CliError {
    pub code: i32,
    pub message: String,
    pub hint: Option<String>,
}

impl CliError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn no_base() -> Self {
        Self::new(EXIT_NO_BASE, "no base tree has been built yet")
            .with_hint("run `replica base build --repo <url> --setup \"<cmd>\"` first")
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// Map any error to an exit code, looking for a typed `CliError` first.
pub fn exit_code(err: &anyhow::Error) -> i32 {
    if let Some(e) = err.downcast_ref::<CliError>() {
        return e.code;
    }
    EXIT_GENERIC
}

/// Hint attached to a typed error, if any.
pub fn hint(err: &anyhow::Error) -> Option<&str> {
    err.downcast_ref::<CliError>()
        .and_then(|e| e.hint.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_error_keeps_code_through_anyhow() {
        let e: anyhow::Error = CliError::no_base().into();
        assert_eq!(exit_code(&e), EXIT_NO_BASE);
        assert!(hint(&e).unwrap().contains("base build"));
    }

    #[test]
    fn untyped_error_is_generic() {
        let e = anyhow::anyhow!("boom");
        assert_eq!(exit_code(&e), EXIT_GENERIC);
        assert!(hint(&e).is_none());
    }
}
