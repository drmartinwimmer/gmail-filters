use crate::{
    ClassifierCategory, GmailTriageError, LabelAllowlistConfig, Tier1Decision,
    ValidatedClassifierDecision,
};
use serde::{Deserialize, Serialize};

/// Structural filter criteria allowed in synthesized `gmailctl` rules.
/// Raw `query` expressions are strictly forbidden to prevent prompt injection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralFilterCriteria {
    /// Matches verified `from` sender address or domain.
    From(String),
    /// Matches verified `to` recipient address.
    To(String),
    /// Matches verified `list` (`List-Id`) identifier.
    List(String),
    /// Matches a verified safe structural subject token/prefix (`subject: "..."`).
    Subject(String),
    /// Matches any of the enclosed structural criteria (`or: [...]`).
    Or(Vec<StructuralFilterCriteria>),
    /// Matches all of the enclosed structural criteria (`and: [...]`), enabling sub-cluster rules.
    And(Vec<StructuralFilterCriteria>),
}

impl StructuralFilterCriteria {
    /// Validates that all structural identifiers are non-empty and free of injection characters.
    pub fn validate(&self) -> Result<(), GmailTriageError> {
        match self {
            Self::From(val) | Self::To(val) | Self::List(val) => {
                validate_structural_identifier(val)
            }
            Self::Subject(val) => validate_subject_prefix(val),
            Self::Or(items) | Self::And(items) => {
                if items.is_empty() {
                    return Err(GmailTriageError::ForbiddenFilterRule(
                        "compound criteria list must not be empty".to_string(),
                    ));
                }
                for item in items {
                    item.validate()?;
                }
                Ok(())
            }
        }
    }

    /// Renders the structural filter criteria as a `gmailctl` JSON/Jsonnet expression via `serde_json`.
    pub fn to_jsonnet(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Disposition profile for a `gmailctl` rule corresponding to the three-tier triage model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDisposition {
    /// Tier 1: Archive and mark read (`archive: true`, `markRead: true`).
    ArchiveAndMarkRead,
    /// Tier 2: Skip the inbox (`archive: true`).
    SkipInbox,
    /// Tier 3: Keep in main inbox and mark important (`archive: false`, `markImportant: true`).
    KeepInInboxImportant,
}

/// Filter actions for a `gmailctl` rule, serialized directly into `gmailctl` `v1alpha3` schema via `serde`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "GmailctlActionsWire", from = "GmailctlActionsWire")]
pub struct FilterRuleActions {
    /// Triage disposition controlling `archive`, `markRead`, and `markImportant`.
    pub disposition: RuleDisposition,
    /// Labels to apply.
    pub labels: Vec<String>,
    /// Optional `forward` target (always rejected by `validate_actions_safety`).
    pub forward: Option<String>,
    /// Whether `delete: true` is set (always rejected by `validate_actions_safety`).
    pub delete: bool,
    /// Optional `markSpam` override (`Some(false)` is rejected by `validate_actions_safety`).
    pub mark_spam: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GmailctlActionsWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mark_read: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mark_important: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    forward: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mark_spam: Option<bool>,
}

impl From<FilterRuleActions> for GmailctlActionsWire {
    fn from(actions: FilterRuleActions) -> Self {
        let (archive, mark_read, mark_important) = match actions.disposition {
            RuleDisposition::ArchiveAndMarkRead => (Some(true), Some(true), None),
            RuleDisposition::SkipInbox => (Some(true), None, None),
            RuleDisposition::KeepInInboxImportant => (Some(false), None, Some(true)),
        };
        Self {
            archive,
            mark_read,
            mark_important,
            labels: actions.labels,
            forward: actions.forward,
            delete: actions.delete.then_some(true),
            mark_spam: actions.mark_spam,
        }
    }
}

impl From<GmailctlActionsWire> for FilterRuleActions {
    fn from(wire: GmailctlActionsWire) -> Self {
        let disposition = if wire.archive == Some(false) || wire.mark_important == Some(true) {
            RuleDisposition::KeepInInboxImportant
        } else if wire.mark_read == Some(true) {
            RuleDisposition::ArchiveAndMarkRead
        } else {
            RuleDisposition::SkipInbox
        };
        Self {
            disposition,
            labels: wire.labels,
            forward: wire.forward,
            delete: wire.delete.unwrap_or(false),
            mark_spam: wire.mark_spam,
        }
    }
}

/// A synthesized `gmailctl` rule backed by structural criteria and validated actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesizedFilterRule {
    /// Structural match criteria.
    pub filter: StructuralFilterCriteria,
    /// Validated filter actions.
    pub actions: FilterRuleActions,
}

impl SynthesizedFilterRule {
    /// Validates the rule against all prompt-injection and action safety invariants.
    pub fn validate(&self, allowlist: &LabelAllowlistConfig) -> Result<(), GmailTriageError> {
        self.filter.validate()?;
        validate_actions_safety(&self.actions, allowlist)
    }

    /// Renders the validated rule as a `gmailctl` JSON/Jsonnet object snippet via `serde_json`.
    pub fn to_jsonnet(&self, allowlist: &LabelAllowlistConfig) -> Result<String, GmailTriageError> {
        self.validate(allowlist)?;
        serde_json::to_string_pretty(self)
            .map_err(|err| GmailTriageError::ForbiddenFilterRule(err.to_string()))
    }
}

/// Author metadata in `config.jsonnet`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailctlAuthorConfig {
    /// Author display name.
    pub name: String,
    /// Author email address.
    pub email: String,
}

/// Label declaration entry in `config.jsonnet`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailctlLabelConfig {
    /// Label name (e.g. `noise`, `receipts`, `lists/github`).
    pub name: String,
}

/// Typed `gmailctl` `v1alpha3` configuration serialized and round-tripped via `serde_json`
/// (valid Jsonnet since Jsonnet is a strict superset of JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailctlConfig {
    /// Schema version (`v1alpha3`).
    pub version: String,
    /// Optional author metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<GmailctlAuthorConfig>,
    /// Declared Gmail labels.
    #[serde(default)]
    pub labels: Vec<GmailctlLabelConfig>,
    /// Filter rules.
    #[serde(default)]
    pub rules: Vec<SynthesizedFilterRule>,
    /// Declarative unit tests preserved across round-trips.
    #[serde(default)]
    pub tests: Vec<serde_json::Value>,
}

impl GmailctlConfig {
    /// Builds a starter `GmailctlConfig` used by `gmail-triage init`.
    pub fn starter() -> Self {
        Self {
            version: "v1alpha3".to_string(),
            author: Some(GmailctlAuthorConfig {
                name: "Example User".to_string(),
                email: "user@example.com".to_string(),
            }),
            labels: vec![
                GmailctlLabelConfig {
                    name: "noise".to_string(),
                },
                GmailctlLabelConfig {
                    name: "receipts".to_string(),
                },
            ],
            rules: Vec::new(),
            tests: Vec::new(),
        }
    }

    /// Parses a `GmailctlConfig` from a JSON/Jsonnet configuration string via `serde_json`.
    pub fn parse_jsonnet(content: &str) -> Result<Self, GmailTriageError> {
        serde_json::from_str(content).map_err(|err| {
            GmailTriageError::ForbiddenFilterRule(format!("Invalid config.jsonnet JSON: {err}"))
        })
    }

    /// Serializes this `GmailctlConfig` to pretty-printed JSON/Jsonnet with a trailing newline.
    pub fn to_jsonnet_pretty(&self) -> Result<String, GmailTriageError> {
        let mut rendered = serde_json::to_string_pretty(self)
            .map_err(|err| GmailTriageError::ForbiddenFilterRule(err.to_string()))?;
        rendered.push('\n');
        Ok(rendered)
    }
}

/// Builds a structural filter criterion from sender, optional list-id, optional recipient, and optional subject prefix.
pub fn build_cluster_filter_criteria(
    from: &str,
    list_id: Option<&str>,
    recipient_to: Option<&str>,
    subject_prefix: Option<&str>,
) -> StructuralFilterCriteria {
    let base = match list_id {
        Some(list) if !list.trim().is_empty() => {
            StructuralFilterCriteria::List(list.trim().to_string())
        }
        _ => StructuralFilterCriteria::From(from.trim().to_string()),
    };

    let mut clauses = vec![base];
    if let Some(to) = recipient_to
        && !to.trim().is_empty()
    {
        clauses.push(StructuralFilterCriteria::To(to.trim().to_string()));
    }
    if let Some(prefix) = subject_prefix
        && !prefix.trim().is_empty()
    {
        clauses.push(StructuralFilterCriteria::Subject(prefix.trim().to_string()));
    }

    if clauses.len() == 1 {
        clauses
            .into_iter()
            .next()
            .unwrap_or_else(|| StructuralFilterCriteria::From(from.trim().to_string()))
    } else {
        StructuralFilterCriteria::And(clauses)
    }
}

/// Synthesizes a deterministic `noise` filter rule from a `Tier1Decision::RouteToNoise` outcome.
pub fn synthesize_tier1_noise_rule(
    decision: &Tier1Decision,
    allowlist: &LabelAllowlistConfig,
) -> Result<Option<SynthesizedFilterRule>, GmailTriageError> {
    match decision {
        Tier1Decision::RouteToNoise { from, list_id } => {
            let filter = build_cluster_filter_criteria(from, list_id.as_deref(), None, None);
            let rule = SynthesizedFilterRule {
                filter,
                actions: FilterRuleActions {
                    disposition: RuleDisposition::ArchiveAndMarkRead,
                    labels: vec!["noise".to_string()],
                    forward: None,
                    delete: false,
                    mark_spam: None,
                },
            };
            rule.validate(allowlist)?;
            Ok(Some(rule))
        }
        Tier1Decision::Unsubscribe { .. } | Tier1Decision::KeepForTriage { .. } => Ok(None),
    }
}

/// Synthesizes a `gmailctl` rule from a validated Tier 2 / Tier 3 classifier decision and structural sender metadata.
pub fn synthesize_classifier_rule(
    from: &str,
    list_id: Option<&str>,
    decision: &ValidatedClassifierDecision,
    allowlist: &LabelAllowlistConfig,
) -> Result<SynthesizedFilterRule, GmailTriageError> {
    let filter = build_cluster_filter_criteria(from, list_id, None, None);
    synthesize_criteria_classifier_rule(filter, decision, allowlist)
}

/// Synthesizes a `gmailctl` rule from an explicit `StructuralFilterCriteria` (such as a compound sub-cluster) and a validated decision.
pub fn synthesize_criteria_classifier_rule(
    filter: StructuralFilterCriteria,
    decision: &ValidatedClassifierDecision,
    allowlist: &LabelAllowlistConfig,
) -> Result<SynthesizedFilterRule, GmailTriageError> {
    let actions = match decision.category {
        ClassifierCategory::Tier2LabelAndArchive => {
            let label = decision.validated_label.clone().ok_or_else(|| {
                GmailTriageError::DisallowedClassifierLabel("<missing>".to_string())
            })?;
            FilterRuleActions {
                disposition: RuleDisposition::SkipInbox,
                labels: vec![label],
                forward: None,
                delete: false,
                mark_spam: None,
            }
        }
        ClassifierCategory::Tier3Actionable => FilterRuleActions {
            disposition: RuleDisposition::KeepInInboxImportant,
            labels: decision.validated_label.iter().cloned().collect(),
            forward: None,
            delete: false,
            mark_spam: None,
        },
    };

    let rule = SynthesizedFilterRule { filter, actions };
    rule.validate(allowlist)?;
    Ok(rule)
}

fn validate_actions_safety(
    actions: &FilterRuleActions,
    allowlist: &LabelAllowlistConfig,
) -> Result<(), GmailTriageError> {
    if actions.forward.is_some() {
        return Err(GmailTriageError::ForbiddenFilterRule(
            "autonomous 'forward' action is forbidden (exfiltration risk)".to_string(),
        ));
    }
    if actions.delete {
        return Err(GmailTriageError::ForbiddenFilterRule(
            "autonomous 'delete: true' action is forbidden (data loss risk)".to_string(),
        ));
    }
    if actions.mark_spam == Some(false) {
        return Err(GmailTriageError::ForbiddenFilterRule(
            "autonomous 'markSpam: false' override is forbidden".to_string(),
        ));
    }
    for label in &actions.labels {
        if !allowlist.contains(label) {
            return Err(GmailTriageError::DisallowedClassifierLabel(label.clone()));
        }
    }
    Ok(())
}

fn validate_structural_identifier(identifier: &str) -> Result<(), GmailTriageError> {
    let trimmed = identifier.trim();
    if trimmed.is_empty() {
        return Err(GmailTriageError::ForbiddenFilterRule(
            "structural criteria identifier must not be empty".to_string(),
        ));
    }
    if trimmed != identifier
        || trimmed.chars().any(|ch| {
            ch.is_whitespace()
                || ch.is_control()
                || matches!(ch, '\'' | '"' | '(' | ')' | '{' | '}' | ':' | ';')
        })
    {
        return Err(GmailTriageError::ForbiddenFilterRule(format!(
            "identifier '{identifier}' contains whitespace or query syntax characters"
        )));
    }
    Ok(())
}

fn validate_subject_prefix(prefix: &str) -> Result<(), GmailTriageError> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(GmailTriageError::ForbiddenFilterRule(
            "subject prefix must be 1..=64 characters".to_string(),
        ));
    }
    if trimmed != prefix
        || trimmed.chars().any(|ch| {
            ch.is_control() || matches!(ch, '\'' | '"' | '(' | ')' | '{' | '}' | ':' | ';')
        })
    {
        return Err(GmailTriageError::ForbiddenFilterRule(format!(
            "subject prefix '{prefix}' contains forbidden query syntax characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GmailctlRunner;
    use googletest::prelude::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct ScopedTempDir {
        path: PathBuf,
    }

    impl ScopedTempDir {
        fn new(suffix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "gmail-triage-synth-{suffix}-{}",
                std::process::id()
            ));
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
    fn synthesize_tier1_noise_rule_builds_valid_jsonnet() -> Result<(), GmailTriageError> {
        let allowlist = LabelAllowlistConfig::from_labels(["noise", "receipts"]);
        let decision = Tier1Decision::RouteToNoise {
            from: "promotions@marketing.example.com".to_string(),
            list_id: None,
        };
        let rule = synthesize_tier1_noise_rule(&decision, &allowlist)?
            .ok_or_else(|| GmailTriageError::ForbiddenFilterRule("missing rule".to_string()))?;
        let jsonnet = rule.to_jsonnet(&allowlist)?;
        let reparsed: SynthesizedFilterRule = serde_json::from_str(&jsonnet)
            .map_err(|e| GmailTriageError::ForbiddenFilterRule(e.to_string()))?;
        assert_that!(reparsed, eq(&rule));
        assert_that!(
            jsonnet.contains("\"from\": \"promotions@marketing.example.com\""),
            is_true()
        );
        assert_that!(jsonnet.contains("\"markRead\": true"), is_true());
        assert_that!(jsonnet.contains("\"noise\""), is_true());
        Ok(())
    }

    #[googletest::test]
    fn synthesize_compound_subcluster_rule_builds_and_filter() -> Result<(), GmailTriageError> {
        let allowlist = LabelAllowlistConfig::from_labels(["noise", "receipts", "lists/github"]);
        let decision = ValidatedClassifierDecision {
            category: ClassifierCategory::Tier2LabelAndArchive,
            validated_label: Some("receipts".to_string()),
            confidence_bps: 9500,
        };
        let filter = build_cluster_filter_criteria(
            "notifications@service.example.com",
            None,
            Some("user@example.com"),
            Some("Invoice"),
        );
        let rule = synthesize_criteria_classifier_rule(filter, &decision, &allowlist)?;
        let jsonnet = rule.to_jsonnet(&allowlist)?;
        let reparsed: SynthesizedFilterRule = serde_json::from_str(&jsonnet)
            .map_err(|e| GmailTriageError::ForbiddenFilterRule(e.to_string()))?;
        assert_that!(reparsed, eq(&rule));
        assert_that!(jsonnet.contains("\"and\": ["), is_true());
        assert_that!(jsonnet.contains("\"subject\": \"Invoice\""), is_true());
        assert_that!(jsonnet.contains("\"receipts\""), is_true());
        Ok(())
    }

    #[googletest::test]
    fn roundtrip_and_synthesized_configs_pass_gmailctl_test_and_debug()
    -> Result<(), GmailTriageError> {
        let examples_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("config.jsonnet");
        let raw_examples = fs::read_to_string(&examples_path)?;
        let roundtripped = GmailctlConfig::parse_jsonnet(&raw_examples)?.to_jsonnet_pretty()?;

        let roundtrip_dir = ScopedTempDir::new("roundtrip-examples");
        fs::create_dir_all(&roundtrip_dir.path)?;
        fs::write(roundtrip_dir.path.join("config.jsonnet"), &roundtripped)?;

        let mut synth_cfg = GmailctlConfig::starter();
        let allowlist = LabelAllowlistConfig::from_labels(["noise", "receipts"]);
        let noise_rule = synthesize_tier1_noise_rule(
            &Tier1Decision::RouteToNoise {
                from: "promotions@marketing.example.com".to_string(),
                list_id: None,
            },
            &allowlist,
        )?
        .ok_or_else(|| GmailTriageError::ForbiddenFilterRule("missing noise rule".to_string()))?;
        let compound_rule = synthesize_criteria_classifier_rule(
            build_cluster_filter_criteria(
                "notifications@service.example.com",
                None,
                Some("user@example.com"),
                Some("Invoice"),
            ),
            &ValidatedClassifierDecision {
                category: ClassifierCategory::Tier2LabelAndArchive,
                validated_label: Some("receipts".to_string()),
                confidence_bps: 9500,
            },
            &allowlist,
        )?;
        synth_cfg.rules.push(noise_rule);
        synth_cfg.rules.push(compound_rule);

        let synth_dir = ScopedTempDir::new("synthesized-starter");
        fs::create_dir_all(&synth_dir.path)?;
        fs::write(
            synth_dir.path.join("config.jsonnet"),
            synth_cfg.to_jsonnet_pretty()?,
        )?;

        if Command::new("gmailctl").arg("--help").output().is_ok() {
            let runner = GmailctlRunner::default();
            runner.verify_offline(&roundtrip_dir.path)?;
            runner.verify_offline(&synth_dir.path)?;
        }
        Ok(())
    }

    #[googletest::test]
    fn validate_rule_rejects_forward_delete_and_query_injection() {
        let allowlist = LabelAllowlistConfig::from_labels(["noise", "receipts"]);

        let forward_rule = SynthesizedFilterRule {
            filter: StructuralFilterCriteria::From("billing@service.example.com".to_string()),
            actions: FilterRuleActions {
                disposition: RuleDisposition::SkipInbox,
                labels: vec!["receipts".to_string()],
                forward: Some("exfil@attacker.example.com".to_string()),
                delete: false,
                mark_spam: None,
            },
        };
        assert_that!(forward_rule.validate(&allowlist), err(anything()));

        let delete_rule = SynthesizedFilterRule {
            filter: StructuralFilterCriteria::From("billing@service.example.com".to_string()),
            actions: FilterRuleActions {
                disposition: RuleDisposition::KeepInInboxImportant,
                labels: Vec::new(),
                forward: None,
                delete: true,
                mark_spam: None,
            },
        };
        assert_that!(delete_rule.validate(&allowlist), err(anything()));

        let injected_criteria_rule = SynthesizedFilterRule {
            filter: StructuralFilterCriteria::From(
                "billing@service.example.com OR subject:secret".to_string(),
            ),
            actions: FilterRuleActions {
                disposition: RuleDisposition::SkipInbox,
                labels: vec!["receipts".to_string()],
                forward: None,
                delete: false,
                mark_spam: None,
            },
        };
        assert_that!(injected_criteria_rule.validate(&allowlist), err(anything()));
    }
}
