//! Shared utilities for the UI crate.

/// Extension trait that logs an error and converts a `Result` into an `Option`.
///
/// Eliminates the repetitive `match result { Err(e) => tracing::error!(...), Ok(v) => Some(v) }`
/// pattern at call sites.
///
/// # Example
/// ```rust,no_run
/// # use zt_ui::utils::LogErr;
/// # let result: anyhow::Result<String> = Ok("ok".into());
/// result.log_err("Operation failed");
/// ```
pub trait LogErr<T> {
    /// Log the error with the given context label, then return `None`.
    /// Returns `Some(value)` on success.
    fn log_err(self, context: &str) -> Option<T>;
}

impl<T> LogErr<T> for anyhow::Result<T> {
    fn log_err(self, context: &str) -> Option<T> {
        self.map_err(|e| tracing::error!("{context}: {e}")).ok()
    }
}
