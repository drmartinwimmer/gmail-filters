use crate::GmailTriageError;
use serde::{Deserialize, Serialize};

/// Unsubscribe mechanism detected in email headers (`List-Unsubscribe`, RFC 8058 `List-Unsubscribe-Post`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsubscribeCapability {
    /// No `List-Unsubscribe` header present.
    None,
    /// Standard `List-Unsubscribe` header (`mailto:` or `https:`) present.
    HeaderPresent,
    /// RFC 8058 one-click unsubscribe (`List-Unsubscribe-Post: List-Unsubscribe=One-Click`) present.
    OneClickRfc8058,
}

/// Deterministic engagement and header metadata for a single sender or mailing list (Tier 1 input).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SenderEngagementMetrics {
    /// Verified sender address or domain (`from`).
    pub from: String,
    /// Optional `List-Id` identifier (`list`).
    pub list_id: Option<String>,
    /// Detected `List-Unsubscribe` capability.
    pub unsubscribe: UnsubscribeCapability,
    /// Whether `Precedence: bulk` or `Precedence: list` was observed.
    pub precedence_bulk: bool,
    /// Total number of messages received from this sender/list in the observation window.
    pub total_received: u32,
    /// Number of messages opened/read by the user.
    pub opened_count: u32,
    /// Number of messages archived or marked read without opening.
    pub archived_unread_count: u32,
}

impl SenderEngagementMetrics {
    /// Validates structural invariants of the engagement metrics.
    pub fn validate(&self) -> Result<(), GmailTriageError> {
        if self.from.trim().is_empty() {
            return Err(GmailTriageError::InvalidEngagementMetrics(
                "sender 'from' identifier must not be empty".to_string(),
            ));
        }
        if self.opened_count > self.total_received {
            return Err(GmailTriageError::InvalidEngagementMetrics(
                "opened_count cannot exceed total_received".to_string(),
            ));
        }
        if self.archived_unread_count > self.total_received {
            return Err(GmailTriageError::InvalidEngagementMetrics(
                "archived_unread_count cannot exceed total_received".to_string(),
            ));
        }
        Ok(())
    }
}

/// Threshold configuration for deterministic Tier 1 unopened/bulk mail classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier1ThresholdsConfig {
    /// Minimum number of received emails before low-open-rate heuristics trigger.
    pub min_messages_for_engagement: u32,
    /// Maximum open rate in basis points (0..=10_000, where 1000 = 10.00%) still considered ignored.
    pub max_open_rate_bps: u32,
    /// Minimum unread-archive rate in basis points (0..=10_000, where 8000 = 80.00%) considered ignored.
    pub min_unread_archive_rate_bps: u32,
}

impl Default for Tier1ThresholdsConfig {
    fn default() -> Self {
        Self {
            min_messages_for_engagement: 5,
            max_open_rate_bps: 1000,
            min_unread_archive_rate_bps: 8000,
        }
    }
}

/// Deterministic Tier 1 classification outcome (strictly non-LLM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Tier1Decision {
    /// Prefer unsubscribing via verified `List-Unsubscribe` header.
    Unsubscribe {
        /// Verified sender address.
        from: String,
        /// Optional `List-Id` header.
        list_id: Option<String>,
        /// Unsubscribe capability level.
        capability: UnsubscribeCapability,
    },
    /// Route to the `noise` label (`labels: ["noise"]`, `archive: true`, `markRead: true`).
    RouteToNoise {
        /// Verified sender address.
        from: String,
        /// Optional `List-Id` header.
        list_id: Option<String>,
    },
    /// Sender has active engagement; pass to Tier 2 / Tier 3 classification.
    KeepForTriage {
        /// Verified sender address.
        from: String,
        /// Optional `List-Id` header.
        list_id: Option<String>,
    },
}

/// Evaluates sender engagement and bulk header signals deterministically without invoking an LLM.
pub fn evaluate_tier1_sender(
    metrics: &SenderEngagementMetrics,
    thresholds: &Tier1ThresholdsConfig,
) -> Result<Tier1Decision, GmailTriageError> {
    metrics.validate()?;

    let is_low_engagement = is_sender_ignored(metrics, thresholds);
    let is_unopened_bulk = is_bulk_and_unopened(metrics);

    if is_low_engagement || is_unopened_bulk {
        let decision = match metrics.unsubscribe {
            UnsubscribeCapability::OneClickRfc8058 | UnsubscribeCapability::HeaderPresent => {
                Tier1Decision::Unsubscribe {
                    from: metrics.from.clone(),
                    list_id: metrics.list_id.clone(),
                    capability: metrics.unsubscribe,
                }
            }
            UnsubscribeCapability::None => Tier1Decision::RouteToNoise {
                from: metrics.from.clone(),
                list_id: metrics.list_id.clone(),
            },
        };
        return Ok(decision);
    }

    Ok(Tier1Decision::KeepForTriage {
        from: metrics.from.clone(),
        list_id: metrics.list_id.clone(),
    })
}

fn is_sender_ignored(
    metrics: &SenderEngagementMetrics,
    thresholds: &Tier1ThresholdsConfig,
) -> bool {
    if metrics.total_received < thresholds.min_messages_for_engagement
        || metrics.total_received == 0
    {
        return false;
    }
    let total = u64::from(metrics.total_received);
    let open_rate_bps = (u64::from(metrics.opened_count) * 10_000) / total;
    let unread_archive_bps = (u64::from(metrics.archived_unread_count) * 10_000) / total;

    open_rate_bps <= u64::from(thresholds.max_open_rate_bps)
        && unread_archive_bps >= u64::from(thresholds.min_unread_archive_rate_bps)
}

fn is_bulk_and_unopened(metrics: &SenderEngagementMetrics) -> bool {
    let has_bulk_signal = metrics.precedence_bulk
        || metrics.list_id.is_some()
        || metrics.unsubscribe != UnsubscribeCapability::None;
    has_bulk_signal && metrics.total_received > 0 && metrics.opened_count == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn evaluate_tier1_unopened_with_unsubscribe_returns_unsubscribe() -> Result<(), GmailTriageError>
    {
        let metrics = SenderEngagementMetrics {
            from: "newsletter@marketing.example.com".to_string(),
            list_id: Some("digest.marketing.example.com".to_string()),
            unsubscribe: UnsubscribeCapability::OneClickRfc8058,
            precedence_bulk: true,
            total_received: 10,
            opened_count: 0,
            archived_unread_count: 10,
        };
        let decision = evaluate_tier1_sender(&metrics, &Tier1ThresholdsConfig::default())?;
        assert_that!(
            decision,
            eq(&Tier1Decision::Unsubscribe {
                from: "newsletter@marketing.example.com".to_string(),
                list_id: Some("digest.marketing.example.com".to_string()),
                capability: UnsubscribeCapability::OneClickRfc8058,
            })
        );
        Ok(())
    }

    #[googletest::test]
    fn evaluate_tier1_ignored_without_unsubscribe_routes_to_noise() -> Result<(), GmailTriageError>
    {
        let metrics = SenderEngagementMetrics {
            from: "promotions@marketing.example.com".to_string(),
            list_id: None,
            unsubscribe: UnsubscribeCapability::None,
            precedence_bulk: true,
            total_received: 8,
            opened_count: 0,
            archived_unread_count: 8,
        };
        let decision = evaluate_tier1_sender(&metrics, &Tier1ThresholdsConfig::default())?;
        assert_that!(
            decision,
            eq(&Tier1Decision::RouteToNoise {
                from: "promotions@marketing.example.com".to_string(),
                list_id: None,
            })
        );
        Ok(())
    }

    #[googletest::test]
    fn evaluate_tier1_engaged_sender_keeps_for_triage() -> Result<(), GmailTriageError> {
        let metrics = SenderEngagementMetrics {
            from: "teammate@work.example.com".to_string(),
            list_id: None,
            unsubscribe: UnsubscribeCapability::None,
            precedence_bulk: false,
            total_received: 12,
            opened_count: 11,
            archived_unread_count: 0,
        };
        let decision = evaluate_tier1_sender(&metrics, &Tier1ThresholdsConfig::default())?;
        assert_that!(
            decision,
            eq(&Tier1Decision::KeepForTriage {
                from: "teammate@work.example.com".to_string(),
                list_id: None,
            })
        );
        Ok(())
    }

    #[googletest::test]
    fn evaluate_tier1_invalid_counts_returns_error() {
        let metrics = SenderEngagementMetrics {
            from: "promotions@marketing.example.com".to_string(),
            list_id: None,
            unsubscribe: UnsubscribeCapability::None,
            precedence_bulk: false,
            total_received: 3,
            opened_count: 5,
            archived_unread_count: 0,
        };
        let result = evaluate_tier1_sender(&metrics, &Tier1ThresholdsConfig::default());
        assert_that!(result, err(anything()));
    }
}
