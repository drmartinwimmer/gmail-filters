use crate::GmailTriageError;
use crate::config::{LabelAllowlistConfig, TriageWorkspaceConfig, resolve_config_dir};
use crate::tools::GmailctlRunner;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the `audit` subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Also run `gmailctl test` and `gmailctl debug` offline verification
    #[arg(long)]
    run_gmailctl: bool,
}

impl AuditCommand {
    /// Creates a new `AuditCommand`.
    pub fn new(config: Option<PathBuf>, run_gmailctl: bool) -> Self {
        Self {
            config,
            run_gmailctl,
        }
    }

    /// Executes the audit subcommand.
    pub fn run(self) -> Result<LabelAllowlistConfig, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let _workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
        let allowlist = LabelAllowlistConfig::load_from_path(&target_dir)?;
        if self.run_gmailctl {
            GmailctlRunner::default().verify_offline(&target_dir)?;
        }
        Ok(allowlist)
    }
}
