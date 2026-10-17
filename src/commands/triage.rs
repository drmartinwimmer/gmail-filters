use crate::GmailTriageError;
use crate::classify::{ValidatedClassifierDecision, parse_and_validate_classifier_json};
use crate::config::{LabelAllowlistConfig, TriageWorkspaceConfig, resolve_config_dir};
use crate::synthesize::{
    SynthesizedFilterRule, synthesize_classifier_rule, synthesize_tier1_noise_rule,
};
use crate::tier1::{
    SenderEngagementMetrics, Tier1Decision, Tier1ThresholdsConfig, UnsubscribeCapability,
    evaluate_tier1_sender,
};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unified outcome of running the 3-tier `triage` pipeline for a sender or message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TriageOutcome {
    /// Tier 1: Unsubscribe via verified `List-Unsubscribe` header (no filter rule needed).
    Unsubscribe {
        /// Verified sender address.
        from: String,
        /// Optional `List-Id` identifier.
        list_id: Option<String>,
        /// Detected unsubscribe capability.
        capability: UnsubscribeCapability,
    },
    /// Tier 1 / Tier 2 / Tier 3: Synthesized a validated `gmailctl` filter rule.
    SynthesizedRule {
        /// Structured rule representation.
        rule: SynthesizedFilterRule,
        /// Rendered `gmailctl` Jsonnet snippet.
        jsonnet: String,
    },
}

/// Arguments for the unified `triage` subcommand.
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct TriageCommand {
    /// Path to configuration directory (defaults to `$GMAIL_TRIAGE_CONFIG_DIR` or `~/.config/gmail-triage`)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Verified sender address (`from`)
    #[arg(long)]
    from: String,

    /// Optional mailing list identifier (`List-Id`)
    #[arg(long)]
    list_id: Option<String>,

    /// Total messages received from this sender in the observation window
    #[arg(long, default_value_t = 0)]
    total_received: u32,

    /// Number of messages opened by the user
    #[arg(long, default_value_t = 0)]
    opened_count: u32,

    /// Number of messages archived without reading
    #[arg(long, default_value_t = 0)]
    archived_unread_count: u32,

    /// Whether `List-Unsubscribe` header is present
    #[arg(long)]
    has_unsubscribe: bool,

    /// Whether `Precedence: bulk` header is present
    #[arg(long)]
    precedence_bulk: bool,

    /// Optional JSON decision from the sandboxed local LLM classifier for Tier 2 / Tier 3
    #[arg(long)]
    decision_json: Option<String>,
}

impl TriageCommand {
    /// Creates a new `TriageCommand` from a `SenderEngagementMetrics` value and optional classifier JSON.
    pub fn from_metrics(
        config: Option<PathBuf>,
        metrics: SenderEngagementMetrics,
        decision_json: Option<String>,
    ) -> Self {
        Self {
            config,
            from: metrics.from,
            list_id: metrics.list_id,
            total_received: metrics.total_received,
            opened_count: metrics.opened_count,
            archived_unread_count: metrics.archived_unread_count,
            has_unsubscribe: metrics.unsubscribe != UnsubscribeCapability::None,
            precedence_bulk: metrics.precedence_bulk,
            decision_json,
        }
    }

    /// Executes the unified 3-tier triage pipeline, hiding internal Tier 1 / Tier 2 / synthesis steps.
    pub fn run(self) -> Result<TriageOutcome, GmailTriageError> {
        let target_dir = resolve_config_dir(self.config)?;
        let workspace_cfg = TriageWorkspaceConfig::load_from_dir(&target_dir)?;
        let allowlist = LabelAllowlistConfig::load_from_path(&target_dir)?;

        let unsubscribe = if self.has_unsubscribe {
            UnsubscribeCapability::HeaderPresent
        } else {
            UnsubscribeCapability::None
        };
        let metrics = SenderEngagementMetrics {
            from: self.from.clone(),
            list_id: self.list_id.clone(),
            unsubscribe,
            precedence_bulk: self.precedence_bulk,
            total_received: self.total_received,
            opened_count: self.opened_count,
            archived_unread_count: self.archived_unread_count,
        };
        let thresholds = Tier1ThresholdsConfig {
            min_messages_for_engagement: workspace_cfg.min_messages_for_engagement,
            max_open_rate_bps: workspace_cfg.max_open_rate_bps,
            min_unread_archive_rate_bps: workspace_cfg.min_unread_archive_rate_bps,
        };

        let tier1_decision = evaluate_tier1_sender(&metrics, &thresholds)?;
        match tier1_decision {
            Tier1Decision::Unsubscribe {
                from,
                list_id,
                capability,
            } => Ok(TriageOutcome::Unsubscribe {
                from,
                list_id,
                capability,
            }),
            Tier1Decision::RouteToNoise { .. } => {
                let rule =
                    synthesize_tier1_noise_rule(&tier1_decision, &allowlist)?.ok_or_else(|| {
                        GmailTriageError::ForbiddenFilterRule(
                            "Failed to synthesize Tier 1 noise rule".to_string(),
                        )
                    })?;
                let jsonnet = rule.to_jsonnet(&allowlist)?;
                Ok(TriageOutcome::SynthesizedRule { rule, jsonnet })
            }
            Tier1Decision::KeepForTriage { .. } => {
                let raw_json = self.decision_json.ok_or_else(|| {
                    GmailTriageError::MissingClassifierDecision(self.from.clone())
                })?;
                let validated: ValidatedClassifierDecision = parse_and_validate_classifier_json(
                    &raw_json,
                    &allowlist,
                    workspace_cfg.min_confidence_bps,
                )?;
                let rule = synthesize_classifier_rule(
                    &self.from,
                    self.list_id.as_deref(),
                    &validated,
                    &allowlist,
                )?;
                let jsonnet = rule.to_jsonnet(&allowlist)?;
                Ok(TriageOutcome::SynthesizedRule { rule, jsonnet })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuditCommand, GmailTriageCommand, GmailTriageSubcommand};
    use googletest::prelude::*;

    #[googletest::test]
    fn user_journey_triage_and_audit_subcommands_succeed() -> Result<(), GmailTriageError> {
        let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");

        let audit_cmd = GmailTriageCommand::with_subcommand(
            GmailTriageSubcommand::Audit(AuditCommand::new(Some(examples.clone()), false)),
            true,
        );
        assert_that!(audit_cmd.run(), ok(anything()));

        // Tier 1 unopened sender with unsubscribe -> Unsubscribe outcome
        let tier1_unsub = TriageCommand::from_metrics(
            Some(examples.clone()),
            SenderEngagementMetrics {
                from: "newsletter@marketing.example.com".to_string(),
                list_id: Some("digest.marketing.example.com".to_string()),
                unsubscribe: UnsubscribeCapability::HeaderPresent,
                precedence_bulk: true,
                total_received: 10,
                opened_count: 0,
                archived_unread_count: 10,
            },
            None,
        );
        let unsub_outcome = tier1_unsub.run()?;
        assert_that!(
            unsub_outcome,
            eq(&TriageOutcome::Unsubscribe {
                from: "newsletter@marketing.example.com".to_string(),
                list_id: Some("digest.marketing.example.com".to_string()),
                capability: UnsubscribeCapability::HeaderPresent,
            })
        );

        // Tier 1 unopened sender without unsubscribe -> Synthesized noise rule
        let tier1_noise = TriageCommand::from_metrics(
            Some(examples.clone()),
            SenderEngagementMetrics {
                from: "promotions@marketing.example.com".to_string(),
                list_id: None,
                unsubscribe: UnsubscribeCapability::None,
                precedence_bulk: true,
                total_received: 10,
                opened_count: 0,
                archived_unread_count: 10,
            },
            None,
        );
        let noise_outcome = tier1_noise.run()?;
        let TriageOutcome::SynthesizedRule { jsonnet, .. } = noise_outcome else {
            return Err(GmailTriageError::ForbiddenFilterRule(
                "Expected SynthesizedRule".to_string(),
            ));
        };
        assert_that!(jsonnet.contains("\"noise\""), is_true());

        // Tier 2 engaged sender with sandboxed classifier JSON -> Synthesized skip-inbox rule
        let raw_json =
            r#"{"category":"tier2_label_and_archive","proposed_label":"receipts","confidence":0.94}"#
                .to_string();
        let tier2_cmd = TriageCommand::from_metrics(
            Some(examples),
            SenderEngagementMetrics {
                from: "billing@service.example.com".to_string(),
                list_id: None,
                unsubscribe: UnsubscribeCapability::None,
                precedence_bulk: false,
                total_received: 5,
                opened_count: 4,
                archived_unread_count: 1,
            },
            Some(raw_json),
        );
        let tier2_outcome = tier2_cmd.run()?;
        let TriageOutcome::SynthesizedRule { jsonnet, .. } = tier2_outcome else {
            return Err(GmailTriageError::ForbiddenFilterRule(
                "Expected SynthesizedRule".to_string(),
            ));
        };
        assert_that!(jsonnet.contains("\"receipts\""), is_true());
        Ok(())
    }
}
