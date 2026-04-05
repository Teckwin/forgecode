use thiserror::Error;

/// Errors that can occur when working with sandboxes.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// The sandbox mechanism is not available on this platform.
    #[error("sandbox is not available: {0}")]
    NotAvailable(String),

    /// Failed to create or write the sandbox profile.
    #[error("failed to create sandbox profile: {0}")]
    ProfileCreation(String),

    /// The sandboxed command failed to execute.
    #[error("sandboxed execution failed: {0}")]
    ExecutionFailed(String),
}
