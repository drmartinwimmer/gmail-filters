# gmail-filters

[![CI](https://github.com/drmartinwimmer/gmail-filters/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/gmail-filters/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

Agentic skills and Rust CLI tools for safely categorizing emails, auditing [`gmailctl`](https://github.com/mbrt/gmailctl) configurations, and updating Gmail filters.

---

> [!IMPORTANT]
> **Disclaimer**: This is a personal project and does not represent the author's current or past employer.

---

## Guiding Principles

### 1. Strict Separation of Tooling and Personal Configuration

- **Tooling & Skills (`drmartinwimmer/gmail-filters`)**: This repository contains only generic Rust CLI/library tooling (`src/`), reusable agent skills (`skills/`), and synthetic test fixtures (`examples/config.jsonnet`). Zero personal email addresses, sender rules, or credentials live here.
- **Personal Configuration (Separate Private Repository)**: Your personal `config.jsonnet` and label taxonomy live in a dedicated private repository (or `~/.config/gmail-filters/`) and are passed at runtime via `--config <path>`.

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
- **Hard Action Guardrails**: Automated tooling forbids synthesizing high-risk Gmail actions (`forward`, `delete: true`, or raw `query` strings derived from email text) and requires offline verification (`gmailctl test`, `gmailctl debug`) plus human review (`gmailctl diff`).

## Repository Layout

- **`src/`**: Rust library and CLI crate (`gmail-filters`) for workspace validation, deterministic noise detection, and sandboxed classification orchestration.
- **`skills/`**: Reusable agent skills (`skills/reviewing-gmail-filters/`) for auditing and updating external `gmailctl` configurations.
- **`examples/`**: Synthetic `gmailctl` configuration (`examples/config.jsonnet`) demonstrating the three-tier triage model with unit tests.
- **`PLAN.md`**: Phased CLI implementation plan and milestone roadmap.

## Usage

### Inspect & Verify a Filter Configuration Directory

```bash
cargo run -- --path /path/to/private/gmail-filters-config
```

### Run `gmailctl` Tests on the Synthetic Example

```bash
gmailctl test --config examples
gmailctl debug --config examples
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
