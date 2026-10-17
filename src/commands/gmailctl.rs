use crate::GmailTriageError;
use crate::config::{LabelAllowlistConfig, resolve_config_dir};
use crate::tools::GmailctlRunner;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the `test` subcommand (wraps `gmailctl test --config <dir>`).
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct TestCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,
}

impl TestCommand {
    /// Creates a new `TestCommand`.
    pub fn new(config: Option<PathBuf>) -> Self {
        Self { config }
    }

    /// Runs `gmailctl test` and `gmailctl debug` against the resolved configuration directory.
    pub fn run(self) -> Result<(), GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let _allowlist = LabelAllowlistConfig::load_from_path(&target_dir)?;
        GmailctlRunner::default().verify_offline(&target_dir)
    }
}

/// Arguments for the `diff` subcommand (wraps `gmailctl diff --config <dir>`).
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,
}

impl DiffCommand {
    /// Creates a new `DiffCommand`.
    pub fn new(config: Option<PathBuf>) -> Self {
        Self { config }
    }

    /// Runs `gmailctl diff --config <dir>`.
    pub fn run(self) -> Result<String, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        GmailctlRunner::default().run_diff(&target_dir)
    }
}

/// Arguments for the `apply` subcommand (wraps `gmailctl test` + `gmailctl apply --config <dir>`).
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,
}

impl ApplyCommand {
    /// Creates a new `ApplyCommand`.
    pub fn new(config: Option<PathBuf>) -> Self {
        Self { config }
    }

    /// Verifies offline tests pass and runs `gmailctl apply --config <dir>`.
    pub fn run(self) -> Result<String, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        GmailctlRunner::default().run_apply(&target_dir)
    }
}

/// Arguments for the `gmailctl` passthrough subcommand (automatically injects `--config <dir>`).
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct GmailctlPassthroughCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,

    /// `gmailctl` subcommand to execute (e.g. `debug`, `export`, `download`, `test`, `diff`, `apply`)
    gmailctl_subcommand: String,

    /// Additional arguments forwarded to `gmailctl`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    extra_args: Vec<String>,
}

impl GmailctlPassthroughCommand {
    /// Creates a new `GmailctlPassthroughCommand`.
    pub fn new(
        config: Option<PathBuf>,
        gmailctl_subcommand: String,
        extra_args: Vec<String>,
    ) -> Self {
        Self {
            config,
            gmailctl_subcommand,
            extra_args,
        }
    }

    /// Executes `gmailctl <subcommand> --config <dir> <extra_args...>`.
    pub fn run(self) -> Result<String, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        GmailctlRunner::default().run_passthrough(
            &self.gmailctl_subcommand,
            &target_dir,
            &self.extra_args,
        )
    }
}
