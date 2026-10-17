use crate::config::{
    LabelAllowlistConfig, TriageWorkspaceConfig, resolve_config_dir, resolve_config_file_path,
};
use crate::tools::GmailctlRunner;
use clap::Args;
use std::path::PathBuf;

/// Error type for `gmail-triage` workspace inspection, triage, and `gmailctl` orchestration.
#[derive(Debug, thiserror::Error)]
pub enum GmailTriageError {
    /// Target path was not found on disk.
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    /// Expected `config.jsonnet` was not found in the target directory.
    #[error("Configuration file '{0}' was not found")]
    MissingConfigFile(PathBuf),

    /// Invalid setting in `gmail-triage.toml`.
    #[error("Invalid gmail-triage.toml configuration: {0}")]
    InvalidTriageConfig(String),

    /// No labels were declared in `config.jsonnet`.
    #[error("No labels found in configuration allowlist")]
    EmptyLabelAllowlist,

    /// Synthetic fixture contains a non-example email domain.
    #[error("Disallowed domain '{0}' in synthetic configuration fixture")]
    DisallowedSyntheticDomain(String),

    /// `gmailctl` subcommand failed.
    #[error("gmailctl '{subcommand}' failed: {details}")]
    GmailctlCommandFailed {
        /// Subcommand that failed.
        subcommand: String,
        /// Standard error output from `gmailctl`.
        details: String,
    },

    /// Sender engagement metrics failed validation.
    #[error("Invalid sender engagement metrics: {0}")]
    InvalidEngagementMetrics(String),

    /// Sandboxed classifier output did not match the required JSON schema.
    #[error("Invalid classifier JSON output: {0}")]
    InvalidClassifierJson(String),

    /// Classifier proposed a label that is not in the declared `config.jsonnet` allowlist.
    #[error("Classifier proposed disallowed label '{0}'")]
    DisallowedClassifierLabel(String),

    /// Classifier confidence was below the minimum required threshold.
    #[error("Classifier confidence {actual_bps} bps is below minimum threshold {min_bps} bps")]
    LowClassifierConfidence {
        /// Actual confidence in basis points.
        actual_bps: u32,
        /// Minimum required confidence in basis points.
        min_bps: u32,
    },

    /// I/O error during workspace inspection.
    #[error("I/O error during workspace inspection: {0}")]
    Io(#[from] std::io::Error),
}

/// Command-line arguments for `gmail-triage`.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct GmailTriageCommand {
    /// Path to target configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl GmailTriageCommand {
    /// Creates a new `GmailTriageCommand`.
    pub fn new(config: Option<PathBuf>, quiet: bool) -> Self {
        Self { config, quiet }
    }

    /// Executes the command.
    pub fn run(self) -> Result<(), GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let _workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
        if target_dir.join("config.jsonnet").exists() {
            let config_file = resolve_config_file_path(&target_dir)?;
            let _allowlist = LabelAllowlistConfig::load_from_path(&config_file)?;
        }
        Ok(())
    }

    /// Audits the target configuration directory using `LabelAllowlistConfig` extraction and `gmailctl`.
    pub fn audit_with_runner(
        self,
        runner: &GmailctlRunner,
    ) -> Result<LabelAllowlistConfig, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let _workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
        let allowlist = LabelAllowlistConfig::load_from_path(&target_dir)?;
        runner.verify_offline(&target_dir)?;
        Ok(allowlist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn run_examples_directory_loads_allowlist_and_succeeds() {
        let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
        let cmd = GmailTriageCommand::new(Some(examples), true);
        assert_that!(cmd.run(), ok(anything()));
    }
}
