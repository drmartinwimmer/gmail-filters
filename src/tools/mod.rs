use crate::GmailTriageError;
use crate::cluster::{
    EmailCluster, FetchedGmailMessage, GwsMessageListResponse, cluster_messages,
    parse_gws_message_json,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Query parameters for `gws gmail users messages list --params <json>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GwsListMessagesParams {
    /// Gmail user identifier (`"me"`).
    pub user_id: String,
    /// Maximum number of message stubs to return.
    pub max_results: u32,
    /// Optional Gmail search query (`q`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
}

/// Query parameters for `gws gmail users messages get --params <json>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GwsGetMessageParams {
    /// Gmail user identifier (`"me"`).
    pub user_id: String,
    /// Message ID to retrieve.
    pub id: String,
    /// Message payload format (`"metadata"`).
    pub format: String,
    /// Header names to include when `format` is `"metadata"`.
    pub metadata_headers: Vec<String>,
}

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

/// Centralized tool runner that invokes the `gws` (Google Workspace CLI) to fetch Gmail messages and headers.
#[derive(Debug, Clone)]
pub struct GwsGmailRunner {
    binary: String,
}

impl Default for GwsGmailRunner {
    fn default() -> Self {
        Self {
            binary: "gws".to_string(),
        }
    }
}

impl GwsGmailRunner {
    /// Creates a new `GwsGmailRunner` with the specified executable name or path.
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// Lists message IDs matching a Gmail search query using `gws gmail users messages list`.
    pub fn list_message_ids(
        &self,
        query: Option<&str>,
        max_results: u32,
    ) -> Result<Vec<String>, GmailTriageError> {
        let q = query
            .map(str::trim)
            .filter(|trimmed| !trimmed.is_empty())
            .map(ToOwned::to_owned);
        let params = GwsListMessagesParams {
            user_id: "me".to_string(),
            max_results,
            q,
        };
        let params_json = serde_json::to_string(&params)
            .map_err(|err| GmailTriageError::GwsParseError(err.to_string()))?;
        let stdout = self.execute_gws(&[
            "gmail",
            "users",
            "messages",
            "list",
            "--params",
            &params_json,
            "--format",
            "json",
        ])?;
        let parsed: GwsMessageListResponse = serde_json::from_str(&stdout)
            .map_err(|err| GmailTriageError::GwsParseError(err.to_string()))?;
        Ok(parsed.messages.into_iter().map(|m| m.id).collect())
    }

    /// Fetches metadata and snippet for a single Gmail message ID via `gws gmail users messages get`.
    pub fn fetch_message(&self, message_id: &str) -> Result<FetchedGmailMessage, GmailTriageError> {
        let params = GwsGetMessageParams {
            user_id: "me".to_string(),
            id: message_id.to_string(),
            format: "metadata".to_string(),
            metadata_headers: [
                "From",
                "To",
                "Subject",
                "List-Id",
                "List-Unsubscribe",
                "List-Unsubscribe-Post",
                "Precedence",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        };
        let params_json = serde_json::to_string(&params)
            .map_err(|err| GmailTriageError::GwsParseError(err.to_string()))?;
        let stdout = self.execute_gws(&[
            "gmail",
            "users",
            "messages",
            "get",
            "--params",
            &params_json,
            "--format",
            "json",
        ])?;
        parse_gws_message_json(&stdout)
    }

    /// Fetches messages matching `query` via `gws` and groups them into `EmailCluster` streams.
    pub fn fetch_and_cluster(
        &self,
        query: Option<&str>,
        max_messages: u32,
    ) -> Result<Vec<EmailCluster>, GmailTriageError> {
        let ids = self.list_message_ids(query, max_messages)?;
        let mut messages = Vec::with_capacity(ids.len());
        for id in ids {
            messages.push(self.fetch_message(&id)?);
        }
        Ok(cluster_messages(&messages))
    }

    fn execute_gws(&self, args: &[&str]) -> Result<String, GmailTriageError> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(GmailTriageError::GwsCommandFailed(stderr));
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

    #[googletest::test]
    fn gws_runner_nonexistent_binary_returns_error() {
        let runner = GwsGmailRunner::new("nonexistent-gws-binary-xyz");
        let result = runner.list_message_ids(None, 10);
        assert_that!(result, err(anything()));
    }
}
