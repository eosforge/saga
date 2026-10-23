[English](tasks.md) | [简体中文](tasks_CN.md)

# Tasks and Qualification Gaps

This ledger owns active deliverables and qualification gaps. Completed milestones describe their original scope, not current release eligibility. No deadline or exception has been approved for unmeasured qualification gaps.

## Engineering tasks

| ID | Scope | Status | Registered | Deliverable | Exit criteria |
| --- | --- | --- | --- | --- | --- |
| TASK-01 | Workspace setup | CLOSED | 2026-10-18 | Rust workspace, protocol with ts-rs bindings and BLAKE3, gateway, and Vue/Tauri Windows host. | At setup: 7 Rust tests and the frontend build passed without errors. |
| TASK-02 | packages/reader | OPEN | 2026-10-18 | EPUB/TXT streaming parsing, pagination, and virtual scrolling. | Render a sample EPUB with font, margin, line-height, and multi-column settings. |
| TASK-03 | packages/player, packages/speech | OPEN | 2026-10-18 | Sentence chunking, audio generation, timeline alignment, and read-along scrolling. | Audio drives .active-sentence highlighting and smooth centering. |
| TASK-04 | server/cas, packages/storage | OPEN | 2026-10-18 | CAS hashing and POST /api/cas/check pre-flight protocol. | A duplicate hash completes in less than one second without reuploading physical content. |
| TASK-05 | tools, packages/speech | OPEN | 2026-10-18 | SpeechProvider support for cloud services (Edge-TTS, OpenAI/Minimax) and local endpoints (CosyVoice/GPT-SoVITS). | Verify generation and sentence alignment with both default Edge-TTS and a local mock/OpenAI-compatible speech endpoint. |
| TASK-06 | protocol, packages/storage | OPEN | 2026-10-19 | The storage engine and compact .sgtl timeline in Proposal 2. | Round-trip binary serialization, over 95% compression versus JSON, and an O(1) chapter-seek benchmark. |
| TASK-07 | Repository cleanup | IMPLEMENTED; LOCAL VERIFICATION | 2026-10-20 | CLEAN-01 storage/error fixes, portable paths, sentence/lifecycle consolidation, local checks, and documentation. | [Contracts and evidence](maintenance.md). Formal qualification remains GAP-04. |
| TASK-08 | protocol, apps/windows/native | IMPLEMENTED; LOCAL VERIFICATION | 2026-10-21 | The .saga package format, manifest engine, and single-file library persistence. | ZIP container creation/reading, SGMF binary manifest round-trip, O(1) section lookup, compressed multi-chapter chunks <256 KiB with oversized single chapters accepted, and a single archive per book. Full-book loading currently decodes all bodies; cross-toolchain archive determinism and crash atomicity are not established. |
| TASK-09 | Repository repairs | IMPLEMENTED; LOCAL VERIFICATION | 2026-10-23 | REPAIR-01 archive/catalog rollback, package validation and resource retention, audit projection, reading progress, stale-response guards, parser boundaries, and documentation corrections. | [Contracts and limits](maintenance.md), [local evidence](evidence/2026-10-23-repair/result.json). Desktop interaction, abrupt-close recovery, total memory bounds, and release qualification remain open. |

## Gaps and remaining verification

| ID | Scope | Status | Contract and evidence |
| --- | --- | --- | --- |
| GAP-01 | Native picker and import feedback | User-reported resolved; native latency unmeasured | Picker completion precedes import; cancellation starts no work; repeated clicks start one operation. Reading/parsing reports processed bytes from zero; unmeasured phases report activity. Success waits for persistence and the IPC result. At the recorded milestone, 31 Rust tests, 5 frontend tests, and the frontend build passed. The user attributed the delay to a remote file and confirmed resolution. [Evidence](evidence/2026-10-19-import.json). |
| GAP-02 | TXT hierarchy and reimport | Implemented; sample verified | SEG-01 preserves list labels inside explicit chapters, uses coherent fallback tiers, atomically replaces sections, and avoids duplicate shelf items. At the recorded milestone, 28 Rust tests, 6 frontend tests, and the build passed. The whole-book check retained all 3,165 explicit headings; genuine duplicate numbers 2656 and 2680 remain with alerts. [Contract](segmentation.md), [evidence](evidence/2026-10-19-segmentation.json). |
| GAP-03 | Retain decisions and navigation | Implemented; desktop review pending | RET-01 persists retain/revoke decisions, saves sections and decisions atomically, navigates to exact headings, and isolates drafts. Scoped automated and isolated original-book persistence checks passed. [Contract](segmentation.md), [evidence](evidence/2026-10-19-retention.json). |
| GAP-04 | Standard adoption and release qualification | OPEN | development.json registers component levels. Formal schemas/approval registries, trusted evidence sealing, obligation expansion, measured coverage/MC-DC, targeted mutation results, complete naming/comment enforcement, and a release gate remain missing. Desktop interaction, packaged storage, bounded reading memory, crash/power-loss durability, and other platforms remain unqualified. No passing denominator, accepted exception, or deadline is asserted. [Maintenance](maintenance.md). |
| GAP-05 | Open-source distribution | OPEN | Cargo declares MIT OR Apache-2.0. License texts and dependency/asset notices remain incomplete. Installers, signing, the support matrix, and release artifacts remain unqualified. |
