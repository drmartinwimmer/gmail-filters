# Global Specification: `gmail-filters`

## System Overview

`gmail-filters` provides reusable agent skills (`skills/`), a Rust CLI and library toolkit (`src/`), and synthetic reference configurations (`examples/`) for auditing, categorizing, and safely updating declarative [`gmailctl`](https://github.com/mbrt/gmailctl) filter rules.

## Separation of Tooling and Personal Configuration

To prevent personal data leakage and allow the tooling and skills to be developed and shared publicly, the architecture enforces a strict two-repository split:

1. **Public Tooling & Skills Repository (`drmartinwimmer/gmail-filters`)**:
   - Houses the generic Rust CLI/library (`src/`), reusable agent skills (`skills/`), prompt-injection defense boundaries, and synthetic test fixtures (`examples/config.jsonnet` using `@example.com` addresses).
   - **Invariant**: Must never contain personal email addresses, personal sender rules, personal label taxonomies, review logs of real emails, or OAuth credentials.
2. **Private Personal Configuration Repository (e.g., `drmartinwimmer/gmail-filters-config`)**:
   - Houses the user's real `config.jsonnet`, custom label taxonomy, sender allowlists/blocklists, and review history.
   - Passed to `gmail-filters` and `gmailctl` at runtime via `--config <path>` (or `$GMAIL_FILTERS_CONFIG_DIR` / `$XDG_CONFIG_HOME/gmail-filters`).
   - OAuth credentials (`credentials.json`, `token.json`) remain strictly local (outside version control in both repositories).

## Guiding Principles: Three-Tier Email Triage Taxonomy

The tooling categorizes incoming mail and synthesizes filter rules across three distinct tiers:

### Tier 1: Unopened / Ignored Mail $\rightarrow$ Deterministic Unsubscribe or `noise` Label (No LLM)

- **Behavioral Signal**: Emails that are consistently left unopened or marked as read / archived without reading.
- **Categorization Mechanism**: Strictly **deterministic (non-LLM)** analysis of header and engagement metadata:
  - Presence of `List-Unsubscribe`, `List-Unsubscribe-Post` (RFC 8058 one-click), `List-Id`, or `Precedence: bulk` headers.
  - Historical engagement ratio (e.g., high archive-while-unread rate or zero opens over $N$ messages from the same sender/list).
- **Target Action**:
  1. Prefer automated or assisted **unsubscribe** when a verified unsubscribe mechanism (`List-Unsubscribe`) is available.
  2. If unsubscribing is not possible or desired, synthesize a deterministic `gmailctl` rule that routes the sender/list to the `noise` label (`labels: ['noise']`, `archive: true`, `markRead: true`).

### Tier 2: Informational / Categorized Mail $\rightarrow$ Apply Labels & Skip Inbox (Sandboxed Local LLM)

- **Behavioral Signal**: Emails that are worth retaining under structured topic labels (e.g., receipts, statements, project/list notifications) but do not require immediate inbox attention.
- **Categorization Mechanism**: Classified by a **local or tightly sandboxed LLM** (e.g., direct Ollama HTTP call with JSON schema enforcement or `opencode` running inside a `fence` sandbox).
- **Strict Sandboxing & Output Contract**:
  - **No or Minimal Tool Calls**: The classifier executes as an isolated, stateless function with zero shell execution, zero filesystem write access, and no external network access.
  - **Constrained Structured Output**: The LLM output must conform to a strict schema returning only a simple structured decision:
    ```json
    {
      "category": "tier2_label_and_archive",
      "proposed_label": "receipts",
      "confidence": 0.94
    }
    ```
  - **Allowlist Validation**: The Rust toolkit validates `proposed_label` against the explicit list of labels declared in the target `config.jsonnet` (or an approved new-label proposal enum) and discards any free-form text.
- **Target Action**: Apply the validated Gmail label(s) and skip the inbox (`archive: true`).

### Tier 3: Relevant & Actionable Mail $\rightarrow$ Main Mailbox

- **Behavioral Signal**: Direct personal correspondence, high-priority notifications, and emails requiring user action or reply.
- **Target Action**: Keep visible in the primary inbox (`archive: false`), optionally applying context or priority markers.

## Security Architecture: Defending Against Untrusted Email Inputs

Because email subjects, sender display names, headers, and bodies are controlled by arbitrary external senders, **all email content is treated as untrusted, potentially adversarial input** (indirect prompt injection / data poisoning).

1. **Privilege Separation (Quarantined Classifier vs. Privileged Filter Agent)**:
   - Raw or truncated email content is only ever inspected by the **quarantined Tier 2 classifier** (a tool-less local LLM with schema-constrained output).
   - High-privilege agents (those with filesystem access to edit `config.jsonnet` or run `gmailctl`) never ingest raw untrusted email bodies; they consume only sanitized, structured metadata (verified sender email/domain, `List-Id`, deterministic engagement counters, and schema-validated label enums).
2. **Input Sanitization & Truncation**:
   - HTML, hidden styles, zero-width characters, and tracking markup are stripped before classification.
   - Untrusted fields are explicitly delimited and truncated to bounded lengths before reaching the local model.
3. **Forbidden Autonomous Filter Actions**:
   - Untrusted classification outputs can **never** synthesize rules containing `forward` (exfiltration risk), `delete: true` (data loss risk), or `markSpam: false` overrides without explicit human authoring.
   - Filter rule criteria generated from triage decisions are restricted to verified structural identifiers (`from` address/domain, `to`, `list`)—never arbitrary raw Gmail `query` strings injected from email content.
4. **Offline Verification & Human-in-the-Loop Gate**:
   - Every proposed filter modification must pass `gmailctl test` and `gmailctl debug` offline, and requires human review of `gmailctl diff` before applying changes to Gmail.

## Cross-Cutting Concerns

- **Error Handling**: All library errors use typed `thiserror::Error` enums (`GmailFiltersError`). Panics, unwraps, and silent error drops are forbidden by workspace Clippy lints and Purist AST rules.
- **Determinism**: CLI exits with `0` on success and `2` on validation or path errors.
