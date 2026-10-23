[English](maintenance.md) | [简体中文](maintenance_CN.md)

# Maintenance and Local Verification

This document owns CLEAN-01 (2026-10-20) and REPAIR-01 (2026-10-23): the authorized repository cleanup and follow-up defect repairs. It records implementation contracts, local checks, and evidence limits. Audio playback, synchronization, other platforms, and release approval remain outside that scope.

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
| REPAIR-PUBLISH | Prepare archive replacements before publication; restore the old archive and catalog on ordinary replacement or SQLite commit failure. Preserve exact archive bytes on review-only saves and additional resource bytes on text edits. | native/src/storage.rs and native/src/regression.rs; Windows sharing denial, deferred foreign-key commit failure, no-op bytes, resource and audio-route regressions |
| REPAIR-PACKAGE | Reject invalid manifest arithmetic/references/UTF-8 and incomplete Deflate streams before publication. Preserve empty chapters and empty volumes. Count Unicode scalar values correctly. | protocol/src/package.rs and native/src/regression.rs; boundary encodings, truncated/corrupt streams, resource CRC, chapter round-trips |
| REPAIR-PROGRESS | Serialize pending anchors per book, retain the latest failed write for retry, join issued writes on flush, cancel writes for successful deletion, and reject stale asynchronous completions. | src/stores/library.ts, src/views/Reader.vue, tests/regression.cjs; ordered IPC, failure/retry, reimport, deletion, hydration, and navigation schedules |
| REPAIR-PROJECTION | Regenerate derived sentences after merges; match native and browser splitting. Defer later chapter sentence arrays and discard distant offscreen chapter nodes without moving the retained anchor. | native/src/storage.rs, src/text.ts, src/views/Audit.vue, src/views/Reader.vue; lazy/eager content equivalence and mocked layout checks |

IPC command names, serialized fields, and retain semantics remain compatible. `PackageManifest::encode` now returns a checked `Result` because version-1 field limits can reject input. The binary format version remains 1. Internal visibility is narrow. Vue filenames use PascalCase.

## Storage and lifecycle limits

SQLite stores shelf metadata, reading anchors, and review fingerprints. Chapter metadata and text are packaged in `.saga` archives under `books/<book-id>.saga`. Staging, backup rename, and an immediate SQLite transaction provide rollback for ordinary file replacement and commit failures within one host process. A process-wide read/write lock covers archive reads and mutations. It does not coordinate other processes or guarantee crash/power-loss atomicity. If restoration itself fails, the error includes the retained backup path; failed post-commit backup cleanup is logged without undoing a successful commit.

The confirmed packing rule is compressed payload size <256 KiB for multi-chapter chunks. An individual chapter whose compressed size is at least 256 KiB is accepted as its own chunk, without rejection, splitting, or a new format version. Uncompressed size is not capped at 256 KiB. The earlier oversized-chapter audit case was synthetic and is retained as an acceptance regression, not a defect. No oversized chunk was found in the inspected existing development book.

Package import/load validates all declared text streams and ZIP resource integrity before returning the book. Decompression is bounded by the declared text span, not by a global book-memory budget. Resource bytes survive text edits; only uniquely matched unchanged bodies retain audio routes. Conflicts between an additional resource name and a newly generated chunk reject the edit. Codec, timeline, duplicate-body audio matching, and hostile-input resource budgets remain outside the qualified scope.

SQLite uses WAL and synchronous=NORMAL. Connection setup is serialized, skips existing table creation, adds only missing progress columns, and no longer performs per-open VACUUM or destructive legacy-table removal. Existing legacy tables are left untouched. Tests use in-memory or uniquely named temporary databases, not the running application's database.

The playback timer demonstrates sentence highlighting; it is not TTS or real audio playback. Current text-only books hide playback controls. Browser previews are transient and TXT-only. Native book loading still materializes every chapter body, but only the first five chapters eagerly receive sentence arrays. The reader trims chapters outside the viewport plus a 700 px buffer while retaining at least five chapters. This limits accumulation during scrolling; it does not establish a hard DOM, RSS, or latency budget, and a single large chapter remains fully rendered. Desktop scroll compensation and long-session memory still need measurement.

Progress flush failures remain visible and retryable in the current session. Returning through the reader's shelf button waits for success. Unmount starts a best-effort flush; abrupt process/window termination can lose debounced or failed positions, so desktop-close durability remains unqualified.

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

The historical cleanup index is [2026-10-20-cleanup/result.json](evidence/2026-10-20-cleanup/result.json). REPAIR-01 uses [2026-10-23-repair/result.json](evidence/2026-10-23-repair/result.json) to identify its final local checks and retained diagnostic runs. Other results are linked from [tasks.md](tasks.md), [worklog.md](worklog.md), and [architecture.md](architecture.md).

Retained receipts describe their original execution inputs and outcomes. Their log, backup, and run-directory references identify local diagnostic material, not files promised in a fresh clone. Preserve original hashes and outcomes. Missing local material limits replay; it does not become a passing check. Do not rerun tests or rewrite results merely to replace excluded artifacts.

Existing local artifacts are excluded by explicit paths in .git/info/exclude. New runs still need admission review; this local list is not a future-run policy. Stage approved paths explicitly. Git does not back up ignored or untracked diagnostics.

Original-book tests require SAGA_SAMPLE and are separate from the default suite. Scoped tests/builds do not establish desktop acceptance, complete coverage, MC/DC, targeted mutation qualification, other-platform support, or redistribution readiness. [tasks.md](tasks.md) records these gaps.

The user controls application lifecycle, commits, and releases. CLEAN-01 did not start or stop services, install dependencies, migrate live books, or approve a release. Later commits require separate authorization.
