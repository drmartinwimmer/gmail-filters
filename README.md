# gmail-triage (`gmail-filters`)

[![CI](https://github.com/drmartinwimmer/gmail-filters/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/gmail-filters/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

Agentic skills and the `gmail-triage` Rust CLI tool for safely triaging emails, auditing [`gmailctl`](https://github.com/mbrt/gmailctl) configurations, and updating Gmail filters.

---

> [!IMPORTANT]
> **Disclaimer**: This is a personal project and does not represent the author's current or past employer.

---

## Guiding Principles

### 1. Strict Separation of Tooling and Personal Configuration

- **Tooling & Skills (`drmartinwimmer/gmail-filters`)**: This repository contains only generic Rust CLI/library tooling (`src/`), reusable agent skills (`skills/`), and synthetic test fixtures (`examples/config.jsonnet`). Zero personal email addresses, sender rules, or credentials live here.
- **Unified Home Configuration (`~/.config/gmail-triage/`)**: Your personal `config.jsonnet` and `gmail-triage.toml` live in `~/.config/gmail-triage/` (or a custom directory set via `$GMAIL_TRIAGE_CONFIG_DIR` / `--config <path>`), which can also be managed declaratively via the Nix Home Manager module exported by `flake.nix`.

### 2. Three-Tier Email Triage Model

| Tier                              | Mail Profile                                               | Categorization Method                                                                                                                  | Target Filter Action                                                                                 |
| :-------------------------------- | :--------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------- |
| **Tier 1: Noise / Unopened**      | Routinely unopened or marked read/archived without reading | **Deterministic (No LLM)**: engagement ratios, `List-Unsubscribe`, `List-Id`, `Precedence: bulk`                                       | **Unsubscribe** if possible; otherwise filter into `noise` label (`archive: true`, `markRead: true`) |
| **Tier 2: Labeled & Skip-Inbox**  | Informational, receipts, notifications worth filing        | **Sandboxed Local LLM** (e.g. Ollama / `opencode` in `fence`) with zero/minimal tools & strict JSON decision output (`proposed_label`) | Apply validated Gmail label and **skip inbox** (`archive: true`)                                     |
| **Tier 3: Relevant & Actionable** | Direct correspondence & actionable items                   | Deterministic allowlists + high-signal classification                                                                                  | Keep in **main mailbox** (`archive: false`)                                                          |

### 3. Security Against Untrusted Email Inputs (Prompt Injection Defense)

Email headers and bodies are untrusted external inputs that may contain prompt injection payloads designed to poison agent behavior:

- **Privilege Separation**: Raw email content is only ever processed by a quarantined, tool-less local LLM classifier whose output is constrained to a strict JSON schema (e.g., selecting a proposed label from a fixed allowlist).
- **Sanitized Control Plane**: The filter-synthesis tool and review agent operate strictly on validated structural metadata (verified sender domains, `List-Id`, engagement metrics, and allowlisted label enums)—never raw email bodies.
- **Hard Action Guardrails**: Automated tooling forbids synthesizing high-risk Gmail actions (`forward`, `delete: true`, or raw `query` strings derived from email text) and requires offline verification (`gmail-triage test`) plus human review (`gmail-triage diff`).

## Repository Layout

- **`src/`**: Rust library and CLI crate (`gmail-triage`) for unified 3-tier email triage, workspace auditing, and `gmailctl` orchestration.
- **`skills/`**: Reusable agent skills (`skills/reviewing-gmail-filters/`) for auditing and updating external `gmailctl` configurations.
- **`examples/`**: Synthetic `gmailctl` configuration (`examples/config.jsonnet`) demonstrating the three-tier triage model with unit tests.
- **`PLAN.md`**: Phased CLI implementation plan and milestone roadmap.

## Usage

### Initialize or Audit Your Unified Configuration Directory (`~/.config/gmail-triage/`)

```bash
# Scaffold ~/.config/gmail-triage/ with config.jsonnet and gmail-triage.toml
gmail-triage init

# Audit declared labels, settings, and offline gmailctl compilation
gmail-triage audit --run-gmailctl
```

### Triage Emails Through the 3-Tier Pipeline (`gws` + Sub-Clustering + Sandboxed Local LLM)

`gmail-triage triage` hides the multi-tier complexity behind a single command:

- **Live Gmail Fetch via `gws`**: `gmail-triage triage` invokes the `gws` CLI (`gws gmail users messages list` / `get`) to fetch messages matching `--query` (default `in:inbox`).
- **Sub-Cluster Grouping**: Messages from the same sender are automatically sub-clustered by `(from, list_id, recipient_to, subject_prefix)` so distinct streams (e.g., invoices vs. security alerts vs. newsletters) get distinct `and: [{ from: '...' }, { to: '...' }, { subject: '...' }]` rules.
- **Pluggable Sandboxed Local LLM (`gmail-triage.toml`)**: Supports mutually exclusive backend variants under `[backend]`: `type = "open_ai_http"` (any OpenAI-compatible `/v1/chat/completions` endpoint, defaulting to local Ollama at `http://127.0.0.1:11434/v1/chat/completions`, with `"tools": []` and `response_format` JSON Schema enforcement) or `type = "fence_command"` (running `fence -n -- ...` with the sanitized prompt piped via `stdin`).
- **New Label Proposals**: The classifier can either pick an existing label (`proposed_label`) or propose a new label slug (`proposed_new_label` validated against `^[a-z0-9]+(/[a-z0-9_-]+)*$`). Passing `--write-config` patches `<config-dir>/config.jsonnet` with both newly approved labels and synthesized rules.

```bash
# Fetch live inbox messages via gws, cluster by sender/stream, classify via local LLM, and preview outcomes
gmail-triage triage --query "in:inbox" --max-messages 50

# Fetch, triage, and patch <config-dir>/config.jsonnet in place with new labels and rules
gmail-triage triage --query "in:inbox" --max-messages 50 --write-config
```

### Wrapped `gmailctl` Commands (Auto-Injects `--config`)

Because `gmailctl` hardcodes `~/.gmailctl`, `gmail-triage` wraps `gmailctl` and automatically passes `--config <resolved-config-dir>`:

```bash
gmail-triage test
gmail-triage diff
gmail-triage apply
gmail-triage gmailctl debug
```

## Development

This repository uses [Nix](https://nixos.org/) flakes, [Direnv](https://direnv.net/), and [Jujutsu (`jj`)](https://github.com/martinvonz/jj):

```bash
# Enter development environment
direnv allow

# Check formatting & lints
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
purist
taplo fmt --check
prettier --check "**/*.{md,json}"

# Run tests
cargo test --all-targets --all-features
gmailctl test --config examples
```

## License

MIT
