use crate::{GmailTriageError, GmailctlConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.jsonnet";
const TRIAGE_SETTINGS_FILENAME: &str = "gmail-triage.toml";
const ENV_GMAIL_TRIAGE_CONFIG_DIR: &str = "GMAIL_TRIAGE_CONFIG_DIR";
const ENV_XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const ENV_HOME: &str = "HOME";
const DEFAULT_SUBDIR: &str = "gmail-triage";

/// Configuration for an OpenAI-compatible HTTP `/v1/chat/completions` classifier backend
/// (defaults to local Ollama at `http://127.0.0.1:11434/v1/chat/completions`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenAiBackendConfig {
    /// OpenAI-compatible `/v1/chat/completions` endpoint URL (default `http://127.0.0.1:11434/v1/chat/completions` for local Ollama).
    pub url: String,
    /// Model identifier (default `qwen2.5:7b-instruct`).
    pub model: String,
    /// Optional environment variable name containing a Bearer API key (not needed for local Ollama).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

impl Default for OpenAiBackendConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
            model: "qwen2.5:7b-instruct".to_string(),
            api_key_env: None,
        }
    }
}

/// Configuration for the fence-sandboxed CLI classifier backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FenceCommandBackendConfig {
    /// Command line for the sandboxed CLI classifier receiving the prompt via `stdin`.
    pub command: String,
}

impl Default for FenceCommandBackendConfig {
    fn default() -> Self {
        Self {
            command: "fence -n -- opencode run --format json".to_string(),
        }
    }
}

/// Mutually exclusive local/sandboxed LLM backend configuration for Tier 2 / Tier 3 classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClassifierBackendConfig {
    /// OpenAI-compatible HTTP `/v1/chat/completions` endpoint (e.g., local Ollama at `http://127.0.0.1:11434/v1/chat/completions`, `llama-server`, `vLLM`, or `LM Studio`) with `"tools": []` and structured JSON schema output.
    #[serde(alias = "openai", alias = "ollama", alias = "ollama_http")]
    OpenAiHttp(OpenAiBackendConfig),
    /// Sandboxed CLI runner (`fence` with network disabled or a custom command receiving the prompt via `stdin`).
    #[serde(alias = "fence", alias = "command")]
    FenceCommand(FenceCommandBackendConfig),
}

impl Default for ClassifierBackendConfig {
    fn default() -> Self {
        Self::OpenAiHttp(OpenAiBackendConfig::default())
    }
}

/// User settings stored in `<config-dir>/gmail-triage.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TriageWorkspaceConfig {
    /// Minimum classifier confidence in basis points (`0..=10_000`, default `7500` = `0.75`).
    pub min_confidence_bps: u32,
    /// Minimum received messages before low-open-rate heuristics trigger in Tier 1.
    pub min_messages_for_engagement: u32,
    /// Maximum open rate in basis points (`0..=10_000`, default `1000` = `10.00%`) still considered ignored.
    pub max_open_rate_bps: u32,
    /// Minimum unread-archive rate in basis points (`0..=10_000`, default `8000` = `80.00%`) considered ignored.
    pub min_unread_archive_rate_bps: u32,
    /// Mutually exclusive local/sandboxed classifier backend configuration.
    pub backend: ClassifierBackendConfig,
}

impl Default for TriageWorkspaceConfig {
    fn default() -> Self {
        Self {
            min_confidence_bps: 7500,
            min_messages_for_engagement: 5,
            max_open_rate_bps: 1000,
            min_unread_archive_rate_bps: 8000,
            backend: ClassifierBackendConfig::default(),
        }
    }
}

impl TriageWorkspaceConfig {
    /// Loads optional settings from `<config-dir>/gmail-triage.toml`, returning `Self::default()` if the file does not exist.
    pub fn load_from_dir(config_dir: &Path) -> Result<Self, GmailTriageError> {
        let settings_path = config_dir.join(TRIAGE_SETTINGS_FILENAME);
        if !settings_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&settings_path)?;
        Self::from_toml_str(&content)
    }

    /// Deserializes `TriageWorkspaceConfig` from a `gmail-triage.toml` document via `serde` and `toml`.
    pub fn from_toml_str(content: &str) -> Result<Self, GmailTriageError> {
        toml::from_str(content)
            .map_err(|err| GmailTriageError::InvalidTriageConfig(err.to_string()))
    }

    /// Serializes `TriageWorkspaceConfig` to pretty-printed TOML for `gmail-triage init`.
    pub fn to_toml_string(&self) -> Result<String, GmailTriageError> {
        toml::to_string_pretty(self)
            .map_err(|err| GmailTriageError::InvalidTriageConfig(err.to_string()))
    }
}

/// Validated allowlist of Gmail labels declared in a `gmailctl` configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelAllowlistConfig {
    labels: BTreeSet<String>,
}

impl LabelAllowlistConfig {
    /// Creates a new `LabelAllowlistConfig` from an iterator of label names.
    pub fn from_labels<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let labels = iter.into_iter().map(Into::into).collect();
        Self { labels }
    }

    /// Parses declared label names (`labels: [{ "name": "..." }]`) from a `config.jsonnet` document via `serde_json`.
    pub fn parse_jsonnet(content: &str) -> Result<Self, GmailTriageError> {
        let cfg = GmailctlConfig::parse_jsonnet(content)?;
        let labels: BTreeSet<String> = cfg
            .labels
            .into_iter()
            .map(|l| l.name)
            .filter(|name| !name.trim().is_empty())
            .collect();
        if labels.is_empty() {
            return Err(GmailTriageError::EmptyLabelAllowlist);
        }
        Ok(Self { labels })
    }

    /// Loads the `LabelAllowlistConfig` from `<config_dir>/config.jsonnet` or directly from a `.jsonnet` file path.
    pub fn load_from_path(path: &Path) -> Result<Self, GmailTriageError> {
        let config_file = resolve_config_file_path(path)?;
        let content = fs::read_to_string(&config_file)?;
        Self::parse_jsonnet(&content)
    }

    /// Checks whether a proposed label is present in the declared allowlist.
    pub fn contains(&self, label: &str) -> bool {
        self.labels.contains(label)
    }

    /// Registers an approved new label into the allowlist.
    pub fn insert_label(&mut self, label: impl Into<String>) -> bool {
        self.labels.insert(label.into())
    }

    /// Returns an ordered iterator over the allowed labels.
    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, String> {
        self.labels.iter()
    }

    /// Returns the number of declared labels in the allowlist.
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Returns `true` if no labels are in the allowlist.
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

/// Resolves the target configuration directory from an explicit `--config` path,
/// `$GMAIL_TRIAGE_CONFIG_DIR`, `$XDG_CONFIG_HOME/gmail-triage`, `~/.config/gmail-triage`,
/// or the current directory.
pub fn resolve_config_dir(explicit: Option<PathBuf>) -> Result<PathBuf, GmailTriageError> {
    let candidate = match explicit {
        Some(path) => path,
        None => resolve_env_config_dir().unwrap_or_else(|| PathBuf::from(".")),
    };
    if !candidate.exists() {
        return Err(GmailTriageError::PathNotFound(candidate));
    }
    Ok(candidate)
}

/// Resolves the default home configuration path (`$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
/// even if the directory does not yet exist on disk (used by `gmail-triage init`).
pub fn resolve_or_default_home_config_dir(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Ok(dir) = env::var(ENV_GMAIL_TRIAGE_CONFIG_DIR)
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }
    if let Ok(xdg) = env::var(ENV_XDG_CONFIG_HOME)
        && !xdg.trim().is_empty()
    {
        return PathBuf::from(xdg).join(DEFAULT_SUBDIR);
    }
    if let Ok(home) = env::var(ENV_HOME)
        && !home.trim().is_empty()
    {
        return PathBuf::from(home).join(".config").join(DEFAULT_SUBDIR);
    }
    PathBuf::from(".")
}

/// Resolves the path to `config.jsonnet` inside a configuration directory (or returns the file if directly passed).
pub fn resolve_config_file_path(path: &Path) -> Result<PathBuf, GmailTriageError> {
    if !path.exists() {
        return Err(GmailTriageError::PathNotFound(path.to_path_buf()));
    }
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let config_file = path.join(CONFIG_FILENAME);
    if !config_file.exists() {
        return Err(GmailTriageError::MissingConfigFile(config_file));
    }
    Ok(config_file)
}

/// Verifies that a synthetic fixture file only uses approved non-personal domains (`example.com`, `example.org`, `example.net`, `github.com`).
pub fn validate_synthetic_domains(content: &str) -> Result<(), GmailTriageError> {
    for token in content.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '\'' | '"' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
            )
    }) {
        if let Some((_, domain)) = token.rsplit_once('@')
            && !is_allowed_synthetic_domain(domain)
        {
            return Err(GmailTriageError::DisallowedSyntheticDomain(
                domain.to_string(),
            ));
        }
    }
    Ok(())
}

fn is_allowed_synthetic_domain(domain: &str) -> bool {
    let lower = domain.to_ascii_lowercase();
    lower == "example.com"
        || lower.ends_with(".example.com")
        || lower == "example.org"
        || lower.ends_with(".example.org")
        || lower == "example.net"
        || lower.ends_with(".example.net")
        || lower == "github.com"
        || lower.ends_with(".github.com")
}

fn resolve_env_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var(ENV_GMAIL_TRIAGE_CONFIG_DIR)
        && !dir.trim().is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    if let Ok(xdg) = env::var(ENV_XDG_CONFIG_HOME)
        && !xdg.trim().is_empty()
    {
        let candidate = PathBuf::from(xdg).join(DEFAULT_SUBDIR);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(home) = env::var(ENV_HOME)
        && !home.trim().is_empty()
    {
        let candidate = PathBuf::from(home).join(".config").join(DEFAULT_SUBDIR);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_jsonnet_extracts_examples_labels() -> Result<(), GmailTriageError> {
        let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let allowlist = LabelAllowlistConfig::load_from_path(&examples_dir)?;
        assert_that!(allowlist.len(), eq(3));
        assert_that!(allowlist.contains("noise"), is_true());
        assert_that!(allowlist.contains("lists/github"), is_true());
        assert_that!(allowlist.contains("receipts"), is_true());
        assert_that!(allowlist.contains("unregistered"), is_false());
        Ok(())
    }

    #[googletest::test]
    fn parse_jsonnet_empty_labels_returns_error() {
        let content = r#"{"version": "v1alpha3", "labels": [], "rules": []}"#;
        let result = LabelAllowlistConfig::parse_jsonnet(content);
        assert_that!(result, err(anything()));
    }

    #[googletest::test]
    fn parse_triage_workspace_config_succeeds() -> Result<(), GmailTriageError> {
        let toml = r#"
min_confidence_bps = 8500
min_messages_for_engagement = 10

[backend]
type = "open_ai_http"
model = "llama3.2:3b"
"#;
        let cfg = TriageWorkspaceConfig::from_toml_str(toml)?;
        assert_that!(cfg.min_confidence_bps, eq(8500));
        assert_that!(cfg.min_messages_for_engagement, eq(10));
        assert_that!(cfg.max_open_rate_bps, eq(1000));
        assert_that!(
            &cfg.backend,
            eq(&ClassifierBackendConfig::OpenAiHttp(OpenAiBackendConfig {
                url: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
                model: "llama3.2:3b".to_string(),
                api_key_env: None,
            }))
        );
        Ok(())
    }

    #[googletest::test]
    fn parse_triage_workspace_config_rejects_mixed_backend_fields() {
        let toml = r#"
[backend]
type = "fence_command"
command = "fence -n -- opencode run --format json"
model = "llama3.2:3b"
"#;
        let result = TriageWorkspaceConfig::from_toml_str(toml);
        assert_that!(result, err(anything()));
    }

    #[googletest::test]
    fn validate_synthetic_domains_accepts_examples_config() -> Result<(), GmailTriageError> {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("config.jsonnet");
        let content = fs::read_to_string(config_path)?;
        let result = validate_synthetic_domains(&content);
        assert_that!(result, ok(anything()));
        Ok(())
    }

    #[googletest::test]
    fn validate_synthetic_domains_rejects_personal_domain() {
        let content = r#"{"from": "alice@personal-domain.io"}"#;
        let result = validate_synthetic_domains(content);
        assert_that!(result, err(anything()));
    }
}
