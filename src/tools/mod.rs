use crate::GmailTriageError;
use std::path::Path;
use std::process::Command;

/// Centralized tool runner that wraps `gmailctl` and automatically injects `--config <config_dir>`.
#[derive(Debug, Clone)]
pub struct GmailctlRunner {
    binary: String,
}

impl Default for GmailctlRunner {
    fn default() -> Self {
        Self {
            binary: "gmailctl".to_string(),
        }
    }
}

impl GmailctlRunner {
    /// Creates a new `GmailctlRunner` with the specified executable name or path.
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// Runs `gmailctl test --config <config_dir>` to execute declarative filter unit tests.
    pub fn run_test(&self, config_dir: &Path) -> Result<String, GmailTriageError> {
        self.execute_with_config("test", config_dir, &[])
    }

    /// Runs `gmailctl debug --config <config_dir>` to compile and validate filter rules and labels.
    pub fn run_debug(&self, config_dir: &Path) -> Result<String, GmailTriageError> {
        self.execute_with_config("debug", config_dir, &[])
    }

    /// Runs `gmailctl diff --config <config_dir>` to compare local filters against Gmail.
    pub fn run_diff(&self, config_dir: &Path) -> Result<String, GmailTriageError> {
        self.execute_with_config("diff", config_dir, &[])
    }

    /// Runs `gmailctl apply --config <config_dir>` after verifying offline tests pass.
    pub fn run_apply(&self, config_dir: &Path) -> Result<String, GmailTriageError> {
        self.verify_offline(config_dir)?;
        self.execute_with_config("apply", config_dir, &[])
    }

    /// Runs both `gmailctl test` and `gmailctl debug` against `<config_dir>`.
    pub fn verify_offline(&self, config_dir: &Path) -> Result<(), GmailTriageError> {
        self.run_test(config_dir)?;
        self.run_debug(config_dir)?;
        Ok(())
    }

    /// Executes an arbitrary `gmailctl` subcommand with `--config <config_dir>` automatically injected
    /// unless `--config` was already explicitly supplied in `extra_args`.
    pub fn run_passthrough(
        &self,
        subcommand: &str,
        config_dir: &Path,
        extra_args: &[String],
    ) -> Result<String, GmailTriageError> {
        self.execute_with_config(subcommand, config_dir, extra_args)
    }

    fn execute_with_config(
        &self,
        subcommand: &str,
        config_dir: &Path,
        extra_args: &[String],
    ) -> Result<String, GmailTriageError> {
        let mut cmd = Command::new(&self.binary);
        cmd.arg(subcommand);
        let has_explicit_config = extra_args
            .iter()
            .any(|arg| arg == "--config" || arg.starts_with("--config="));
        if !has_explicit_config {
            cmd.arg("--config").arg(config_dir);
        }
        cmd.args(extra_args);
        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(GmailTriageError::GmailctlCommandFailed {
                subcommand: subcommand.to_string(),
                details: stderr,
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn gmailctl_runner_nonexistent_binary_returns_error() {
        let runner = GmailctlRunner::new("nonexistent-gmailctl-binary-xyz");
        let result = runner.run_test(Path::new("."));
        assert_that!(result, err(anything()));
    }
}
