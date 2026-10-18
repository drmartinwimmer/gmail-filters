mod audit;
mod gmailctl;
mod init;
mod triage;

pub use audit::AuditCommand;
pub use gmailctl::{ApplyCommand, DiffCommand, GmailctlPassthroughCommand, TestCommand};
pub use init::InitCommand;
pub use triage::{TriageCommand, TriageOutcome};

use crate::classify::{UntrustedEmailInput, sanitize_untrusted_email};
use crate::config::{
    LabelAllowlistConfig, TriageWorkspaceConfig, resolve_config_dir, resolve_config_file_path,
};
use crate::tools::GmailctlRunner;
use clap::{Args, Subcommand};
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

    /// Classifier proposed a `proposed_new_label` that violated slug naming rules.
    #[error("Invalid proposed new label slug: {0}")]
    InvalidNewLabelSlug(String),

    /// Classifier confidence was below the minimum required threshold.
    #[error("Classifier confidence {actual_bps} bps is below minimum threshold {min_bps} bps")]
    LowClassifierConfidence {
        /// Actual confidence in basis points.
        actual_bps: u32,
        /// Minimum required confidence in basis points.
        min_bps: u32,
    },

    /// Proposed filter rule violated structural criteria or action safety invariants.
    #[error("Forbidden filter rule: {0}")]
    ForbiddenFilterRule(String),

    /// `gws` CLI invocation failed.
    #[error("gws command failed: {0}")]
    GwsCommandFailed(String),

    /// Failed to parse JSON returned by `gws`.
    #[error("Failed to parse gws JSON output: {0}")]
    GwsParseError(String),

    /// Sandboxed local classifier backend execution failed.
    #[error("Classifier backend execution failed: {0}")]
    ClassifierBackendFailed(String),

    /// I/O error during workspace inspection.
    #[error("I/O error during workspace inspection: {0}")]
    Io(#[from] std::io::Error),
}

/// User-journey subcommands supported by `gmail-triage`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum GmailTriageSubcommand {
    /// Run the unified 3-tier email triage pipeline (fetches messages via `gws`, groups into sub-clusters, runs Tier 1 + sandboxed LLM, and synthesizes/patches `config.jsonnet`)
    Triage(TriageCommand),

    /// Audit `<config-dir>/config.jsonnet` label allowlists, settings, and offline `gmailctl` checks
    Audit(AuditCommand),

    /// Run `gmailctl test` against the resolved configuration directory
    Test(TestCommand),

    /// Run `gmailctl diff` against the resolved configuration directory
    Diff(DiffCommand),

    /// Verify offline tests and run `gmailctl apply` against the resolved configuration directory
    Apply(ApplyCommand),

    /// Initialize `~/.config/gmail-triage` (or `--config <dir>`) with starter `config.jsonnet` and `gmail-triage.toml`
    Init(InitCommand),

    /// Pass arbitrary subcommands and arguments to `gmailctl` with `--config <dir>` automatically set
    Gmailctl(GmailctlPassthroughCommand),
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

    /// User-journey subcommand (`triage`, `audit`, `test`, `diff`, `apply`, `init`, `gmailctl`)
    #[command(subcommand)]
    subcommand: Option<GmailTriageSubcommand>,
}

impl GmailTriageCommand {
    /// Creates a new `GmailTriageCommand`.
    pub fn new(config: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            config,
            quiet,
            subcommand: None,
        }
    }

    /// Creates a new `GmailTriageCommand` with an explicit subcommand.
    pub fn with_subcommand(subcommand: GmailTriageSubcommand, quiet: bool) -> Self {
        Self {
            config: None,
            quiet,
            subcommand: Some(subcommand),
        }
    }

    /// Executes the command.
    pub fn run(self) -> Result<(), GmailTriageError> {
        match self.subcommand {
            Some(GmailTriageSubcommand::Triage(cmd)) => cmd.run().map(|_| ()),
            Some(GmailTriageSubcommand::Audit(cmd)) => cmd.run().map(|_| ()),
            Some(GmailTriageSubcommand::Test(cmd)) => cmd.run(),
            Some(GmailTriageSubcommand::Diff(cmd)) => cmd.run().map(|_| ()),
            Some(GmailTriageSubcommand::Apply(cmd)) => cmd.run().map(|_| ()),
            Some(GmailTriageSubcommand::Init(cmd)) => cmd.run().map(|_| ()),
            Some(GmailTriageSubcommand::Gmailctl(cmd)) => cmd.run().map(|_| ()),
            None => {
                let target_dir = resolve_config_dir(self.config)?;
                let _workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
                if target_dir.join("config.jsonnet").exists() {
                    let config_file = resolve_config_file_path(&target_dir)?;
                    let _allowlist = LabelAllowlistConfig::load_from_path(&config_file)?;
                }
                Ok(())
            }
        }
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

    /// Sanitizes an untrusted email input into a bounded `SanitizedEmailEnvelope` prompt.
    pub fn sanitize_prompt(input: &UntrustedEmailInput) -> String {
        sanitize_untrusted_email(input).to_delimited_prompt()
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
