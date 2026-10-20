[English](AGENTS.md) | [简体中文](AGENTS_CN.md)

# Saga Agent Rules and Operating Governance

These instructions apply across the entire `Saga` repository. Subdirectory configurations may add constraints but must not weaken these rules.

---

## 1. Rule Priority

Follow instructions in the following strict hierarchy of precedence:
1. The user's explicit instructions in the current conversation.
2. This file (`AGENTS.md`).
3. Baseline engineering and coding contracts: [docs/development.md](docs/development.md) and [docs/coding.md](docs/coding.md).
4. Project specifications: [docs/architecture.md](docs/architecture.md), [docs/style.md](docs/style.md), and [docs/documentation.md](docs/documentation.md).
5. Advisory agent skills.

---

## 2. Zero-Memory Recovery Protocol

When initiating a new session, an agent or contributor must recover complete operational context directly from the repository Markdown files without requiring previous conversation transcripts. The mandatory reading sequence is:

1. **`AGENTS.md`**: Understand operational boundaries, approval gates, and authority.
2. **`docs/architecture.md`**: Distinguish the current implementation from target architecture and platform boundaries.
3. **`docs/tasks.md`**: Review active deliverables, open tasks, and technical gaps.
4. **`docs/worklog.md`**: Inspect the latest milestone index and historical decisions.

Then read applicable engineering/coding sections, [project style](docs/style.md), and task-specific contracts. Read English sources only for routine recovery; exclude `*_CN.md` from ordinary searches unless translation review or an explicit user request requires them.

---

## 3. Strict Operating Guardrails

### 3.1 Explicit User Approval Before Disk Writes
- **No Unsolicited Modifications**: The agent must never arbitrarily create or edit repository Markdown files or production source code.
- **Review Gate**: Proposed architectures, technical designs, document drafts, and code modifications must first be presented clearly in chat for user review. Writing to disk is permitted only after explicit user confirmation.

### 3.2 English Authority and Pre-Commit Translation
- The **English version** (`*.md`) is the authoritative source of truth for reading, reasoning, and modification.
- The **Chinese version** (`*_CN.md`) serves strictly as a 1:1 translation and may temporarily lag during ongoing work.
- Ordinary document edits update English only. Do not automatically read, synchronize, or edit `_CN` companions at that time.
- Immediately before an authorized commit, compare changed English documents with their `_CN` companions and update the translations, including meaning, status, identifiers, tables, and links. Commit synchronized pairs together.
- This checkpoint does not authorize committing. Until the checkpoint, English governs any discrepancy. See [documentation governance](docs/documentation.md).

### 3.3 Naming and Architectural Conventions
- Strictly enforce the single-complete-word naming principle from Section 2 of `docs/coding.md`.
- Eliminate redundant scope prefixes or suffixes (e.g., `server/cas` rather than `server/service-cas`).
- Avoid speculative layers or empty abstractions; design strictly for immediate concrete deliverables.

### 3.4 Durable Context and Milestone Indexing
- Every confirmed design decision, task closure, or architectural change must be recorded in its canonical document.
- Append a concise 1–2 line milestone entry to `docs/worklog.md` indexing the update.
- Keep only README and AGENTS document pairs at the root; detailed documents belong under `docs/`.
