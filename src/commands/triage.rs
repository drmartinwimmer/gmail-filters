use crate::GmailTriageError;
pub use crate::cluster::TriageOutcome;
use crate::cluster::{EmailCluster, triage_email_clusters};
use crate::config::{TriageWorkspaceConfig, resolve_config_dir};
use crate::tools::{GwsGmailRunner, LocalClassifierRunner};
use clap::Args;
use std::path::{Path, PathBuf};

/// Arguments for the unified `triage` subcommand (`gmail-triage triage`), which fetches messages
/// from Gmail via `gws`, groups them into sender/stream sub-clusters, evaluates Tier 1 deterministic
/// rules and the sandboxed Tier 2/3 LLM classifier, and synthesizes `gmailctl` filter rules.
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct TriageCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Gmail search query used when fetching messages via `gws` (default: `in:inbox`)
    #[arg(long, default_value = "in:inbox")]
    query: String,

    /// Maximum number of messages to fetch via `gws`
    #[arg(long, default_value_t = 50)]
    max_messages: u32,

    /// Patch `<config-dir>/config.jsonnet` in place with newly proposed labels and synthesized rules
    #[arg(long)]
    write_config: bool,
}

impl Default for TriageCommand {
    fn default() -> Self {
        Self {
            config: None,
            query: "in:inbox".to_string(),
            max_messages: 50,
            write_config: false,
        }
    }
}

impl TriageCommand {
    /// Creates a new `TriageCommand` that fetches messages matching `query` via `gws`.
    pub fn new(
        config: Option<PathBuf>,
        query: impl Into<String>,
        max_messages: u32,
        write_config: bool,
    ) -> Self {
        Self {
            config,
            query: query.into(),
            max_messages,
            write_config,
        }
    }

    /// Evaluates a slice of `EmailCluster` streams through Tier 1 and the configured `LocalClassifierRunner`,
    /// optionally patching `<target_dir>/config.jsonnet` with approved new labels and synthesized rules.
    pub fn triage_clusters(
        target_dir: &Path,
        clusters: &[EmailCluster],
        classifier: &LocalClassifierRunner,
        write_config: bool,
    ) -> Result<Vec<TriageOutcome>, GmailTriageError> {
        triage_email_clusters(target_dir, clusters, classifier, write_config)
    }

    /// Executes the unified 3-tier triage pipeline by fetching messages via `gws`, clustering by
    /// sender and stream, and evaluating Tier 1 + the sandboxed local LLM classifier.
    pub fn run(self) -> Result<Vec<TriageOutcome>, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
        let classifier = LocalClassifierRunner::from_workspace_config(&workspace_cfg);
        let gws = GwsGmailRunner::default();
        let clusters = gws.fetch_and_cluster(Some(&self.query), self.max_messages)?;
        Self::triage_clusters(&target_dir, &clusters, &classifier, self.write_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{UntrustedEmailInput, sanitize_untrusted_email};
    use crate::cluster::ClusterKey;
    use crate::tier1::{SenderEngagementMetrics, UnsubscribeCapability};
    use crate::{
        AuditCommand, FetchedGmailMessage, GmailTriageCommand, GmailTriageSubcommand,
        GmailctlConfig, InitCommand, cluster_messages,
    };
    use googletest::prelude::*;
    use std::fs;

    struct ScopedTempDir {
        path: PathBuf,
    }

    impl ScopedTempDir {
        fn new(suffix: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("gmail-triage-{suffix}-{}", std::process::id()));
            Self { path }
        }
    }

    impl Drop for ScopedTempDir {
        fn drop(&mut self) {
            if self.path.exists() {
                drop(fs::remove_dir_all(&self.path));
            }
        }
    }

    #[googletest::test]
    fn user_journey_triage_and_audit_subcommands_succeed() -> Result<(), GmailTriageError> {
        let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");

        let audit_cmd = GmailTriageCommand::with_subcommand(
            GmailTriageSubcommand::Audit(AuditCommand::new(Some(examples.clone()), false)),
            true,
        );
        assert_that!(audit_cmd.run(), ok(anything()));

        let clusters = vec![
            // Tier 1 unopened sender with unsubscribe -> Unsubscribe outcome
            EmailCluster {
                key: ClusterKey {
                    from_address: "newsletter@marketing.example.com".to_string(),
                    list_id: Some("digest.marketing.example.com".to_string()),
                    to_address: None,
                    subject_prefix: None,
                },
                engagement: SenderEngagementMetrics {
                    from: "newsletter@marketing.example.com".to_string(),
                    list_id: Some("digest.marketing.example.com".to_string()),
                    unsubscribe: UnsubscribeCapability::HeaderPresent,
                    precedence_bulk: true,
                    total_received: 10,
                    opened_count: 0,
                    archived_unread_count: 10,
                },
                representative_sample: sanitize_untrusted_email(&UntrustedEmailInput {
                    from: "newsletter@marketing.example.com".to_string(),
                    list_id: Some("digest.marketing.example.com".to_string()),
                    raw_subject: "Weekly Digest".to_string(),
                    raw_body: "Unsubscribe anytime.".to_string(),
                }),
            },
            // Tier 1 unopened sender without unsubscribe -> Synthesized noise rule
            EmailCluster {
                key: ClusterKey {
                    from_address: "promotions@marketing.example.com".to_string(),
                    list_id: None,
                    to_address: None,
                    subject_prefix: None,
                },
                engagement: SenderEngagementMetrics {
                    from: "promotions@marketing.example.com".to_string(),
                    list_id: None,
                    unsubscribe: UnsubscribeCapability::None,
                    precedence_bulk: true,
                    total_received: 10,
                    opened_count: 0,
                    archived_unread_count: 10,
                },
                representative_sample: sanitize_untrusted_email(&UntrustedEmailInput {
                    from: "promotions@marketing.example.com".to_string(),
                    list_id: None,
                    raw_subject: "Flash Sale".to_string(),
                    raw_body: "50% off today.".to_string(),
                }),
            },
            // Tier 2 engaged sender classified by sandboxed runner -> Synthesized skip-inbox rule
            EmailCluster {
                key: ClusterKey {
                    from_address: "billing@service.example.com".to_string(),
                    list_id: None,
                    to_address: None,
                    subject_prefix: None,
                },
                engagement: SenderEngagementMetrics {
                    from: "billing@service.example.com".to_string(),
                    list_id: None,
                    unsubscribe: UnsubscribeCapability::None,
                    precedence_bulk: false,
                    total_received: 5,
                    opened_count: 4,
                    archived_unread_count: 1,
                },
                representative_sample: sanitize_untrusted_email(&UntrustedEmailInput {
                    from: "billing@service.example.com".to_string(),
                    list_id: None,
                    raw_subject: "Invoice #42".to_string(),
                    raw_body: "Amount due: $10.".to_string(),
                }),
            },
        ];

        let classifier = LocalClassifierRunner::new_fence_command(
            r#"cat >/dev/null && printf '{"category":"tier2_label_and_archive","proposed_label":"receipts","confidence":0.94}'"#,
        );
        let outcomes = TriageCommand::triage_clusters(&examples, &clusters, &classifier, false)?;
        assert_that!(outcomes.len(), eq(3));
        assert_that!(
            outcomes.first(),
            some(eq(&TriageOutcome::Unsubscribe {
                from: "newsletter@marketing.example.com".to_string(),
                list_id: Some("digest.marketing.example.com".to_string()),
                capability: UnsubscribeCapability::HeaderPresent,
            }))
        );
        let Some(TriageOutcome::SynthesizedRule {
            jsonnet: noise_jsonnet,
            ..
        }) = outcomes.get(1)
        else {
            return Err(GmailTriageError::ForbiddenFilterRule(
                "Expected noise SynthesizedRule".to_string(),
            ));
        };
        assert_that!(noise_jsonnet.contains("\"noise\""), is_true());

        let Some(TriageOutcome::SynthesizedRule {
            jsonnet: tier2_jsonnet,
            ..
        }) = outcomes.get(2)
        else {
            return Err(GmailTriageError::ForbiddenFilterRule(
                "Expected Tier 2 SynthesizedRule".to_string(),
            ));
        };
        assert_that!(tier2_jsonnet.contains("\"receipts\""), is_true());
        Ok(())
    }

    #[googletest::test]
    fn triage_clusters_patches_config_jsonnet_with_new_label_and_subcluster_rules()
    -> Result<(), GmailTriageError> {
        let guard = ScopedTempDir::new("e2e-test");
        InitCommand::new(Some(guard.path.clone())).run()?;

        let messages = vec![
            FetchedGmailMessage {
                id: "m1".to_string(),
                from_address: "notifications@service.example.com".to_string(),
                to_address: Some("user@example.com".to_string()),
                raw_subject: "Security Alert: New sign-in".to_string(),
                list_id: None,
                unsubscribe: UnsubscribeCapability::None,
                precedence_bulk: false,
                raw_snippet: "A new device signed in.".to_string(),
                is_unread: false,
                in_inbox: true,
            },
            FetchedGmailMessage {
                id: "m2".to_string(),
                from_address: "notifications@service.example.com".to_string(),
                to_address: Some("user@example.com".to_string()),
                raw_subject: "Invoice #900".to_string(),
                list_id: None,
                unsubscribe: UnsubscribeCapability::None,
                precedence_bulk: false,
                raw_snippet: "Your monthly invoice.".to_string(),
                is_unread: false,
                in_inbox: true,
            },
        ];
        let clusters = cluster_messages(&messages);
        assert_that!(clusters.len(), eq(2));

        let classifier = LocalClassifierRunner::new_fence_command(
            r#"cat >/dev/null && printf '{"category":"tier2_label_and_archive","proposed_new_label":"security/alerts","confidence":0.95}'"#,
        );
        let outcomes = TriageCommand::triage_clusters(&guard.path, &clusters, &classifier, true)?;
        assert_that!(outcomes.len(), eq(2));

        let patched_content = fs::read_to_string(guard.path.join("config.jsonnet"))?;
        let parsed_cfg = GmailctlConfig::parse_jsonnet(&patched_content)?;
        assert_that!(
            parsed_cfg
                .labels
                .iter()
                .any(|l| l.name == "security/alerts"),
            is_true()
        );
        assert_that!(parsed_cfg.rules.len(), eq(2));
        assert_that!(
            patched_content.contains("\"subject\": \"security\""),
            is_true()
        );
        assert_that!(
            patched_content.contains("\"subject\": \"invoice\""),
            is_true()
        );

        Ok(())
    }
}
