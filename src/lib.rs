mod classify;
mod cluster;
mod commands;
mod config;
mod synthesize;
mod tier1;
mod tools;

pub use classify::{
    ClassifierCategory, ClassifierJsonSchema, ClassifierJsonSchemaProperties, ClassifierRawOutput,
    JsonSchemaEnumProperty, JsonSchemaNullableStringProperty, JsonSchemaNumberProperty,
    MAX_BODY_SNIPPET_CHARS, MAX_SUBJECT_CHARS, SanitizedEmailEnvelope, UntrustedEmailInput,
    ValidatedClassifierDecision, classifier_output_json_schema, parse_and_validate_classifier_json,
    sanitize_untrusted_email, validate_classifier_output, validate_new_label_slug,
};
pub use cluster::{
    ClusterKey, EmailCluster, FetchedGmailMessage, GwsHeader, GwsMessageListResponse,
    GwsMessagePayload, GwsMessageResource, GwsMessageStub, cluster_messages, convert_gws_resource,
    extract_structural_subject_prefix, parse_gws_message_json,
};
pub use commands::{
    ApplyCommand, AuditCommand, DiffCommand, GmailTriageCommand, GmailTriageError,
    GmailTriageSubcommand, GmailctlPassthroughCommand, InitCommand, TestCommand, TriageCommand,
    TriageOutcome,
};
pub use config::{
    ClassifierBackendConfig, FenceCommandBackendConfig, LabelAllowlistConfig, OpenAiBackendConfig,
    TriageWorkspaceConfig, resolve_config_dir, resolve_config_file_path,
    resolve_or_default_home_config_dir, validate_synthetic_domains,
};
pub use synthesize::{
    FilterRuleActions, GmailctlAuthorConfig, GmailctlConfig, GmailctlLabelConfig,
    GmailctlTestConfig, GmailctlTestMessageConfig, RuleDisposition, StructuralFilterCriteria,
    SynthesizedFilterRule, build_cluster_filter_criteria, synthesize_classifier_rule,
    synthesize_criteria_classifier_rule, synthesize_tier1_noise_rule,
};
pub use tier1::{
    SenderEngagementMetrics, Tier1Decision, Tier1ThresholdsConfig, UnsubscribeCapability,
    evaluate_tier1_sender,
};
pub use tools::{
    FenceCommandClassifierRunner, GmailctlRunner, GwsGetMessageParams, GwsGmailRunner,
    GwsListMessagesParams, LocalClassifierRunner, OpenAiChatChoice, OpenAiChatCompletionRequest,
    OpenAiChatCompletionResponse, OpenAiChatMessage, OpenAiHttpClassifierRunner,
    OpenAiJsonSchemaSpec, OpenAiResponseFormat, OpenAiToolDefinition,
    build_openai_chat_request_json, extract_openai_message_content,
};

#[cfg(test)]
mod tests {
    use googletest::prelude::*;
    use std::fs;
    use std::path::Path;

    #[googletest::test]
    fn verify_msrv_badge_consistency_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml_content = fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
        let rust_version_line = cargo_toml_content
            .lines()
            .find(|line| line.trim().starts_with("rust-version"))
            .ok_or("rust-version not found in Cargo.toml")?;
        let rust_version = rust_version_line
            .split('=')
            .nth(1)
            .ok_or("Invalid rust-version entry")?
            .trim()
            .trim_matches('"');

        let mut parts = rust_version.split('.');
        let major = parts.next().ok_or("Missing major version component")?;
        let minor = parts.next().ok_or("Missing minor version component")?;
        let badge_target = format!("https://img.shields.io/badge/MSRV-{major}.{minor}%2B-");

        let readme_content = fs::read_to_string(manifest_dir.join("README.md"))?;
        assert_that!(readme_content.contains(&badge_target), is_true());

        Ok(())
    }
}
