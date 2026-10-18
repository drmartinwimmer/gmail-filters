# CLI Implementation Plan (`gmail-triage`)

## Overview

This document defines the user-journey-driven CLI design, unified home configuration directory (`~/.config/gmail-triage/`), NixOS / Home Manager integration, and phased implementation roadmap for the `gmail-triage` Rust crate (`src/`).

Every milestone enforces the three-tier email triage taxonomy, strict separation of public tooling from private configuration, and the prompt-injection security boundaries defined in [`SPEC.md`](SPEC.md).

## Crate Naming Analysis

We evaluated crates.io availability and semantic fit for this tool:

| Candidate Name     | crates.io Available? | Assessment                                                                                                    |
| :----------------- | :------------------- | :------------------------------------------------------------------------------------------------------------ |
| **`gmail-triage`** | **Yes (selected)**   | Clearly communicates the core workflow: triaging incoming Gmail across the 3-tier model into `gmailctl` rules |
| `gmail-filters`    | Yes                  | Descriptive of the output artifact, but sounds like a static config collection rather than an active CLI tool |
| `gmail-sieve`      | Yes                  | Evocative, though "Sieve" is an RFC 5228 mail filtering language that Gmail does not natively use             |
| `gmailctl-triage`  | Yes                  | Explicitly pairs with `gmailctl`, but slightly more verbose to type                                           |

**Decision**: Rename the crate and binary to **`gmail-triage`** (library `gmail_triage`), using `$GMAIL_TRIAGE_CONFIG_DIR` and `~/.config/gmail-triage/` for configuration discovery.

## User Journey & Ergonomic CLI Surface

Rather than exposing low-level internal pipeline stages (`tier1`, `classify`, `synthesize`) to the user, `gmail-triage` is organized around the actions a user actually takes—while wrapping `gmailctl` so the user never has to manually pass `--config ~/.config/gmail-triage`:

1. **Initialize or inspect the configuration directory (`~/.config/gmail-triage/`)**:
   - `gmail-triage init` — Scaffolds `~/.config/gmail-triage/` with a starter `config.jsonnet` and `gmail-triage.toml` (or runs `gmailctl init --config <dir>`).
2. **Audit existing filter rules & label hygiene**:
   - `gmail-triage audit` — Inspects `config.jsonnet`, validates declared label allowlists (`LabelAllowlistConfig`), checks rule safety invariants, and runs `gmailctl test` + `gmailctl debug` offline.
3. **Triage senders / messages through the 3-tier pipeline**:
   - `gmail-triage triage` — Orchestrates the complete three-tier triage pipeline behind a single command:
     1. **Tier 1 (Deterministic, Non-LLM)**: Evaluates sender engagement (`opened_count`, `archived_unread_count`, `List-Unsubscribe`, `List-Id`, `Precedence: bulk`) using thresholds from `gmail-triage.toml`. Recommends unsubscribing or synthesizes a `noise` filter rule (`archive: true`, `markRead: true`, `labels: ['noise']`).
     2. **Tier 2 (Sandboxed Local LLM)**: For engaged informational mail, sanitizes and truncates untrusted subject/body fields, validates the structured JSON decision against the `config.jsonnet` label allowlist and confidence threshold, and synthesizes a skip-inbox rule (`archive: true`, `labels: [<validated_label>]`).
     3. **Tier 3 (Actionable Main Inbox)**: Keeps high-signal correspondence in the main inbox (`archive: false`, `markImportant: true`).
4. **Test, Diff, and Apply via wrapped `gmailctl` commands**:
   - `gmail-triage test` — Runs `gmailctl test --config <resolved-config-dir>`.
   - `gmail-triage diff` — Runs `gmailctl diff --config <resolved-config-dir>`.
   - `gmail-triage apply` — Runs `gmailctl test` verification first, then `gmailctl apply --config <resolved-config-dir>`.
   - `gmail-triage gmailctl <args...>` — Passes arbitrary subcommands/flags to `gmailctl` with `--config <resolved-config-dir>` automatically injected (since `gmailctl` hardcodes `~/.gmailctl` and does not support XDG or environment variable overrides).

## Unified Home Configuration & NixOS / Home Manager Integration

### Directory Layout (`~/.config/gmail-triage/`)

All user-specific configuration lives in a single XDG-compliant directory (`$GMAIL_TRIAGE_CONFIG_DIR` or `$XDG_CONFIG_HOME/gmail-triage`, defaulting to `~/.config/gmail-triage/`):

```text
~/.config/gmail-triage/
├── config.jsonnet       # Declarative gmailctl filter configuration & unit tests
├── gmail-triage.toml    # Triage thresholds, sandboxed classifier endpoint & settings
├── credentials.json     # Local OAuth client credentials (untracked)
└── token.json           # Local OAuth token (untracked)
```

### Declarative NixOS / Home Manager Module (`flake.nix`)

`flake.nix` exports `homeManagerModules.default` (and `nixosModules.default`) under `programs.gmail-triage`, enabling users to declaratively configure:

- `enable`: Installs `gmail-triage` and `gmailctl`.
- `configDir`: Target config directory (defaults to `~/.config/gmail-triage`).
- `settings`: Generates `gmail-triage.toml` (Tier 1 engagement thresholds, Tier 2 classifier model/confidence settings).
- `configJsonnet`: Optional source path or inline Jsonnet for `config.jsonnet`.
- Sets `GMAIL_TRIAGE_CONFIG_DIR` in the user session environment so `gmail-triage` automatically locates the managed configuration.

## Architecture Diagram

```
+-----------------------------------------------------------------------+
|                           gmail-triage CLI                            |
|  User Commands: triage | audit | test | diff | apply | init | gmailctl|
+-----------------------------------------------------------------------+
         |                   |                   |                  |
         v                   v                   v                  v
+----------------+  +----------------+  +----------------+  +---------------+
|   config.rs    |  |   tier1.rs     |  |  classify.rs   |  | synthesize.rs |
|                |  |                |  |                |  |               |
| - XDG ~/.config|  | - Non-LLM      |  | - Sanitize &   |  | - Structural  |
|   /gmail-triage|  |   heuristics   |  |   truncate     |  |   criteria    |
| - TriageConfig |  | - Unsubscribe  |  |   untrusted    |  |   only (from, |
| - LabelAllow-  |  |   vs. noise    |  |   inputs       |  |   to, list)   |
|   listConfig   |  |   routing      |  | - Allowlist &  |  | - Forbid      |
+----------------+  +----------------+  |   confidence   |  |   forward,    |
         |                              |   validation   |  |   delete, raw |
         v                              +----------------+  |   query       |
+----------------+                                          +---------------+
|  tools/mod.rs  |
|                |
| - Gmailctl     |
|   wrapper with |
|   auto --config|
+----------------+
```

## Milestones & Stacked PR Roadmap

### Milestone 1: Crate Rename (`gmail-triage`), Unified XDG Config Directory, NixOS/Home Manager Module & `gmailctl` Wrapper

- **Modules**: `Cargo.toml`, `flake.nix`, `nix/hm-module.nix`, `src/config.rs`, `src/tools/mod.rs`, `src/commands.rs`, `src/lib.rs`, `src/main.rs`
- **Capabilities**:
  - Rename package and binary to `gmail-triage` (`gmail_triage` library).
  - Resolve the unified configuration directory (`--config` / `--path`, `$GMAIL_TRIAGE_CONFIG_DIR`, `$XDG_CONFIG_HOME/gmail-triage`, `~/.config/gmail-triage`).
  - Parse `gmail-triage.toml` settings (`TriageConfig`) and `config.jsonnet` declared labels (`LabelAllowlistConfig`).
  - Provide `GmailctlRunner` in `src/tools/mod.rs` that wraps `gmailctl` (`test`, `debug`, `diff`, `apply`, `init`, and arbitrary passthrough arguments) and automatically injects `--config <resolved-dir>`.
  - Export `homeManagerModules.default` and `nixosModules.default` in `flake.nix`.

### Milestone 2: Tier 1 Deterministic Unopened / Bulk Mail Analyzer (Non-LLM)

- **Modules**: `src/tier1.rs`, `src/lib.rs`
- **Capabilities**:
  - Deterministic (zero-LLM) evaluation of sender engagement (`SenderEngagementMetrics`) and bulk headers (`List-Unsubscribe`, RFC 8058 `List-Unsubscribe-Post`, `List-Id`, `Precedence: bulk`).
  - Configurable `Tier1ThresholdsConfig` (minimum messages, maximum open rate bps, minimum unread-archive rate bps).
  - Classify senders into `Tier1Decision::Unsubscribe`, `Tier1Decision::RouteToNoise`, or `Tier1Decision::KeepForTriage`.

### Milestone 3: Tier 2 Untrusted Email Sanitizer & Schema-Constrained Classifier Validator

- **Modules**: `src/classify.rs`, `src/lib.rs`
- **Capabilities**:
  - Strip HTML tags, `<style>`/`<script>` blocks, zero-width characters, and control characters from untrusted subjects and body snippets (`sanitize_untrusted_email`), truncating to `MAX_SUBJECT_CHARS` and `MAX_BODY_SNIPPET_CHARS`.
  - Validate structured JSON decisions from the sandboxed local LLM (`ClassifierRawOutput` $\rightarrow$ `ValidatedClassifierDecision`) against `LabelAllowlistConfig` and `min_confidence_bps`.

### Milestone 4: Safe Filter Rule Synthesizer & User-Journey CLI Subcommands (`triage`, `audit`, `test`, `diff`, `apply`, `init`, `gmailctl`)

- **Modules**: `src/synthesize.rs`, `src/commands.rs`, `src/lib.rs`, `src/main.rs`, `README.md`
- **Capabilities**:
  - Synthesize safe `gmailctl` Jsonnet rules (`SynthesizedFilterRule`) using strictly structural criteria (`From`, `To`, `List`, `Subject`, `Or`, `And`) and validated dispositions (`ArchiveAndMarkRead`, `SkipInbox`, `KeepInInboxImportant`), rejecting `forward`, `delete: true`, `markSpam: false`, and raw `query` strings.
  - Orchestrate the full 3-tier pipeline in `gmail-triage triage` (running Tier 1 first, then Tier 2/3 classification and synthesis when needed, hiding internal steps behind one command).
  - Expose user-journey subcommands: `triage`, `audit`, `test`, `diff`, `apply`, `init`, and `gmailctl`.

### Milestone 5: `gws` Email Retrieval & Multi-Stream Sub-Clustering

- **Modules**: `src/cluster.rs`, `src/tools/mod.rs`, `src/synthesize.rs`, `src/lib.rs`
- **Capabilities**:
  - Fetch Gmail message IDs and metadata/snippets via the `gws` CLI (`GwsGmailRunner`: `gws gmail users messages list` and `gws gmail users messages get`).
  - Group messages into distinct `EmailCluster` streams keyed by `(from_address, list_id, recipient_to, subject_prefix)` so different message streams from the same sender (e.g., invoices vs. security alerts vs. digests) are triaged separately and synthesize compound `and: [{ from: '...' }, { to: '...' }, { subject: '...' }]` rules.

### Milestone 6: Pluggable Local/Sandboxed LLM Backends (`open_ai_http` & `fence_command`) and New Label Proposals

- **Modules**: `src/classify.rs`, `src/config.rs`, `src/tools/mod.rs`, `src/tools/gmailctl.rs`, `src/tools/gws.rs`, `src/tools/classifier.rs`, `src/lib.rs`
- **Capabilities**:
  - Support mutually exclusive `ClassifierBackendConfig::OpenAiHttp(OpenAiBackendConfig)` (any OpenAI-compatible `/v1/chat/completions` endpoint, defaulting to local Ollama at `http://127.0.0.1:11434/v1/chat/completions`) and `ClassifierBackendConfig::FenceCommand(FenceCommandBackendConfig)` in `gmail-triage.toml` (deserialized via `serde` and `toml`), backed by the `LocalClassifierRunner` enum (`OpenAiHttpClassifierRunner` and `FenceCommandClassifierRunner`).
  - Allow the classifier to either select an existing label (`proposed_label` from `LabelAllowlistConfig`) or propose a new label slug (`proposed_new_label`) strictly validated against `^[a-z0-9]+(/[a-z0-9_-]+)*$`.

### Milestone 7: End-to-End `gws` Triage & `config.jsonnet` Patching

- **Modules**: `src/synthesize.rs`, `src/commands.rs`, `src/lib.rs`, `README.md`, `skills/reviewing-gmail-filters/SKILL.md`
- **Capabilities**:
  - Wire `GwsGmailRunner::fetch_and_cluster`, `LocalClassifierRunner`, and `patch_config_jsonnet` into `gmail-triage triage` (`--write-config`) so newly proposed labels and synthesized sub-cluster rules are automatically inserted into `<config-dir>/config.jsonnet`.
