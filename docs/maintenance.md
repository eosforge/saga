[English](maintenance.md) | [简体中文](maintenance_CN.md)

# Maintenance and Local Verification

This document owns CLEAN-01: the repository cleanup authorized on 2026-10-20. It records implementation contracts, local checks, and evidence limits. Audio, synchronization, other platforms, and release approval remain outside that scope.

## Contracts

Paths in the implementation column are relative to apps/windows/ unless prefixed with packages/.

| Contract | Required behavior | Implementation and checks |
| --- | --- | --- |
| CLEAN-STORAGE | Validate contiguous section/chunk positions and unique identities before writes. Reject malformed persisted JSON. Roll back database deletion when a database or cache operation fails. | native/src/storage.rs; isolated SQLite tests for rejected writes/deletes, corrupt JSON, invalid positions, and cache removal failures |
| CLEAN-PATHS | Resolve debug data relative to the compiled checkout and packaged data through the OS application-local directory. Embed no developer drive. | native/src/paths.rs; moved-checkout test; storage_location IPC |
| CLEAN-READER | Keep sentence display and navigation zero-based. Preserve the shelf on failed native deletion. Ignore late loads after reimport or deletion. | src/text.ts, src/stores/library.ts; actual-module regressions with mocked IPC |
| CLEAN-LIFECYCLE | Give each keyboard action one handler. Release subscriptions registered after unmount. Preserve complete browser TXT previews and reject unsupported formats. | src/App.vue, src/views/Reader.vue, src/views/Shelf.vue; handler/lifecycle tests and type checking |
| CLEAN-NUMERALS | Preserve the place value of positional Chinese digits. Reject malformed, mixed, and overflowing numerals without wrapping. | packages/reader/src/validator.rs; regression vectors |
| CLEAN-DOCS | Keep only README and AGENTS document pairs at the root. Give detailed documents portable links and one canonical owner. | documentation.md; link and path review |

Public parser signatures, IPC command names, serialized fields, and retain semantics remain compatible. Internal visibility is narrow. Vue filenames use PascalCase.

## Storage and lifecycle limits

SQLite stores the readable text. Original TXT bytes are cached separately at cache/<book-id>.txt. Failed import may leave an unreferenced cache. If cache deletion succeeds but database commit fails, the readable book may remain without its optional raw cache. Cross-resource crash atomicity is not guaranteed.

SQLite uses WAL and synchronous=NORMAL. Process failure and power-loss durability are distinct; power-loss durability remains unqualified. Existing additive description/review schema setup is preserved. Tests use in-memory or uniquely named temporary databases, not the running application's database.

The playback timer demonstrates sentence highlighting; it is not TTS or real audio playback. Current text-only books hide playback controls. Browser previews are transient. Continuous reading appends chapters without a bounded DOM window; long-session memory use remains unmeasured.

## Local checks

[development.json](../development.json) is a version-1 local-feedback registry validated by [tools/check.cjs](../tools/check.cjs). It records component consequence levels, the verified environment, commands, and per-process deadlines in milliseconds. Classifying a component does not establish that its obligations passed.

- validate checks fields, identities, paths, programs, levels, and deadline bounds without executing checks.
- collect requires the registered Windows x64/toolchain identity and runs each check once in sequence. It downloads no dependencies and starts no application or service.
- Each process has a deadline and an 8 MiB output cap. Windows timeout cleanup targets the owned child process tree.
- Logs, exit status, timestamps, hashes, and before/after source manifests are retained under a unique local run identity.
- Failure, timeout, incomplete output, or source drift remains a failed check. A rerun needs a diagnosed cause, changed inputs, or another concrete verification need.
- Receipts are local and unsealed. Hashes detect accidental changes; they are not independent timestamps or signatures.

Formal schemas, approval registries, obligation expansion, coverage/MC-DC and mutation accounting, trusted collection, and release gating remain GAP-04. The local runner provides no release-eligible result.

## Formatting and generated files

Rust uses the pinned toolchain, rustfmt.toml, and strict Clippy checks. The frontend uses the installed TypeScript formatter, explicit return types for named functions, and PascalCase Vue filenames. Inside apps/windows/, pnpm format applies script formatting and pnpm lint checks it. Templates/CSS and complete naming/comment enforcement still require review.

ts-rs tests generate TypeScript bindings from protocol/src/lib.rs. Do not hand-edit bindings. Native generated schemas, build output, dependencies, user books, and local data are not handwritten source.

## Tracked evidence and local diagnostics

[Documentation governance](documentation.md) defines admission. Eight cited core JSON results remain tracked. Original logs, backup ZIPs, source snapshots, intermediate JSON, and timestamped runner directories remain local; the 2026-10-20 correction removes 111 such files and one empty placeholder from tracking without deleting their local bytes.

The current cleanup index is [2026-10-20-cleanup/result.json](evidence/2026-10-20-cleanup/result.json). Other results are linked from [tasks.md](tasks.md), [worklog.md](worklog.md), and [architecture.md](architecture.md).

Retained receipts describe their original execution inputs and outcomes. Their log, backup, and run-directory references identify local diagnostic material, not files promised in a fresh clone. Preserve original hashes and outcomes. Missing local material limits replay; it does not become a passing check. Do not rerun tests or rewrite results merely to replace excluded artifacts.

Existing local artifacts are excluded by explicit paths in .git/info/exclude. New runs still need admission review; this local list is not a future-run policy. Stage approved paths explicitly. Git does not back up ignored or untracked diagnostics.

Original-book tests require SAGA_SAMPLE and are separate from the default suite. Scoped tests/builds do not establish desktop acceptance, complete coverage, MC/DC, targeted mutation qualification, other-platform support, or redistribution readiness. [tasks.md](tasks.md) records these gaps.

The user controls application lifecycle, commits, and releases. CLEAN-01 did not start or stop services, install dependencies, migrate live books, or approve a release. Later commits require separate authorization.
