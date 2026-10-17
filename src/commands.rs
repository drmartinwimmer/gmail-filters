use clap::Args;
use std::path::PathBuf;

/// Error type for Gmail filter workspace inspection and validation.
#[derive(Debug, thiserror::Error)]
pub enum GmailFiltersError {
    /// Target path was not found on disk.
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    /// I/O error during workspace inspection.
    #[error("I/O error during workspace inspection: {0}")]
    Io(#[from] std::io::Error),
}

/// Command-line arguments for `gmail-filters`.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct GmailFiltersCommand {
    /// Path to target workspace or configuration directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl GmailFiltersCommand {
    /// Creates a new `GmailFiltersCommand`.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self { path, quiet }
    }

    /// Executes the command.
    pub fn run(self) -> Result<(), GmailFiltersError> {
        let target_dir = self.path.unwrap_or_else(|| PathBuf::from("."));
        if !target_dir.exists() {
            return Err(GmailFiltersError::PathNotFound(target_dir));
        }
        Ok(())
    }
}
