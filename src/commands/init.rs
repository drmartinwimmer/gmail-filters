use crate::GmailTriageError;
use crate::config::{TriageWorkspaceConfig, resolve_or_default_home_config_dir};
use crate::synthesize::GmailctlConfig;
use clap::Args;
use std::fs;
use std::path::PathBuf;

/// Arguments for the `init` subcommand (scaffolds `~/.config/gmail-triage/`).
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct InitCommand {
    /// Path to configuration directory to initialize (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,
}

impl InitCommand {
    /// Creates a new `InitCommand`.
    pub fn new(config: Option<PathBuf>) -> Self {
        Self { config }
    }

    /// Scaffolds `config.jsonnet` and `gmail-triage.toml` inside the target configuration directory.
    pub fn run(self) -> Result<PathBuf, GmailTriageError> {
        let target_dir = resolve_or_default_home_config_dir(self.config);
        fs::create_dir_all(&target_dir)?;

        let config_jsonnet_path = target_dir.join("config.jsonnet");
        if !config_jsonnet_path.exists() {
            let starter = GmailctlConfig::starter().to_jsonnet_pretty()?;
            fs::write(&config_jsonnet_path, starter)?;
        }

        let settings_toml_path = target_dir.join("gmail-triage.toml");
        if !settings_toml_path.exists() {
            let default_cfg = TriageWorkspaceConfig::default();
            fs::write(&settings_toml_path, default_cfg.to_toml_string())?;
        }

        Ok(target_dir)
    }
}
