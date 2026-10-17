# Specification: Synthetic Example Configuration (`examples/`)

## Module Purpose

Provides a synthetic, non-personal `gmailctl` (`v1alpha3`) configuration (`examples/config.jsonnet`) illustrating the three-tier email triage taxonomy for offline CI testing and skill validation.

## Public API / Contracts

- **Fixture Entrypoint**: `examples/config.jsonnet`
- **Schema Version**: `v1alpha3`
- **Triage Tiers Demonstrated**:
  1. **Tier 1 (`noise`)**: Deterministic routing of unopened/bulk senders (`archive: true`, `markRead: true`, `labels: ['noise']`).
  2. **Tier 2 (`lists/*`, `receipts`)**: Labeled messages that skip the inbox (`archive: true`, `labels: [...]`).
  3. **Tier 3 (Main Inbox)**: Actionable messages that remain in the inbox (`archive: false`).

## Invariants

- Must never contain real personal email addresses, personal sender domains, or personal filter rules (use `@example.com` domains only).
- Must pass `gmailctl test --config examples` and `gmailctl debug --config examples`.

## Verification Plan

- Run `gmailctl test --config examples` and `gmailctl debug --config examples > /dev/null` in local verification and CI.
