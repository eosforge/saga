[English](maintenance.md) | [简体中文](maintenance_CN.md)

# Maintenance and Local Verification

This document owns CLEAN-01 (2026-10-20), REPAIR-01, and RESIDENT-01 (2026-10-23): the authorized repository cleanup, defect repairs, and resident catalog/progress saving. It records implementation contracts, local checks, and evidence limits. Audio playback, synchronization, other platforms, and release approval remain outside that scope.

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

SQLite uses WAL and synchronous=NORMAL. The application opens and initializes its managed connection once at startup. Setup skips existing table creation, adds only missing progress columns, and performs neither VACUUM nor destructive legacy-table removal. Existing legacy tables are left untouched. Catalog operations run on existing blocking workers with exclusive access to the retained connection; complete archive operations may delay another command. No additional bound on the host's blocking-worker admission queue is established here. Tests use in-memory or uniquely named temporary databases, not the running application's database.

The playback timer demonstrates sentence highlighting; it is not TTS or real audio playback. Current text-only books hide playback controls. Browser previews are transient and TXT-only. Native book loading still materializes every chapter body, but only the first five chapters eagerly receive sentence arrays. The reader trims chapters outside the viewport plus a 700 px buffer while retaining at least five chapters. This limits accumulation during scrolling; it does not establish a hard DOM, RSS, or latency budget, and a single large chapter remains fully rendered. Desktop scroll compensation and long-session memory still need measurement.

Progress flush failures remain visible and retryable in the current session. Returning through the reader's shelf button and the normal close handler waits for success. Normal close also keeps the window open during an active book operation, and pending close blocks further user interaction. Unmount, hidden visibility, pagehide, and desktop focus loss request an early flush. Forced termination, system shutdown, a missed lifecycle event, or power loss can still lose unsaved positions. Automated lifecycle tests do not qualify real desktop-close delivery or mobile suspension behavior.

## Resident catalog and bounded progress saving (RESIDENT-01)

The user approved retaining SQLite after the isolated storage comparison, selected a 30-second default automatic-save wait, and requested a Settings slider with 5, 10, 15, 30, and 60 second choices. Backgrounding and normal close save earlier. The implementation and local verification are recorded in [the RESIDENT-01 result](evidence/2026-10-23-resident/result.json). The scope is connection ownership, progress scheduling/lifecycle, the save-interval preference, and an on-demand readable export; no storage-format migration, application launch, commit, or release is authorized by this milestone.

- One application-owned SQLite connection is initialized at startup. Blocking native operations take its exclusive mutex before existing archive locks; no operation recursively acquires the connection. Ordinary errors release the mutex, while panic poisoning rejects later operations. Progress updates reuse a cached prepared statement. Parsing remains outside connection ownership where already separate.
- The frontend retains the latest dirty position per book. A timer starts with the first unsaved change and is not reset by continued scrolling. The five allowed intervals are in seconds; the default is 30. The UI preference uses the existing webview `localStorage` convention and is written only when the setting changes. Missing, unavailable, or unsupported saved values use the default. Changing the interval preserves already elapsed monotonic time; shortening below that time schedules an immediate flush. Successful writes acknowledge only the issued position; newer changes remain queued. Equal positions do not create writes, failed IPC responses invalidate older acknowledgements, failures remain visible and retryable, and explicit flush waits for issued and queued writes.
- Returning to the shelf, losing desktop focus, becoming hidden, and normal window close request an earlier flush. Normal close waits for success and stays open on failure. Late listener registration is released after teardown. Forced termination, operating-system shutdown, timer throttling, blocked storage, and power loss are outside the timing/close guarantee.
- Settings offers an explicit JSON export of persisted shelf metadata, reading anchors, and retained review fingerprints after flushing pending positions. Native snapshot reads share one read transaction, then release the catalog connection before writing the user-selected `.json` destination. A sibling staging file preserves an existing destination on ordinary write/replacement failure. The version-1 `saga-library-state` JSON is a diagnostic export, not a live mirror or an import/restore format; it excludes archive contents and UI preferences. Existing `.saga` assets, SQLite schema, existing IPC field names, and WAL/NORMAL settings remain compatible.

Verification covers resident connection reuse and serialization, SQL failures, prepared progress updates, virtual-time scheduling, unchanged anchors, overlapping writes/failures, background/close event ordering and teardown, and export contents. Registered local checks use isolated fixtures; real desktop lifecycle and mobile energy measurements remain unqualified. The earlier comparison is exploratory evidence and is not an integrated application performance claim.

The 2026-10-24 pre-commit review found that global mouse back/forward navigation could still switch views while close waited for progress persistence. The close guard now covers that handler as well as keyboard input and page controls. A regression first reproduced the unwanted transition from reader to shelf. [The review result](evidence/2026-10-24-review/result.json) records subsequent verification without replacing the original RESIDENT-01 evidence.

## Deletion close protection (CLOSE-01)

The 2026-10-24 follow-up found that normal close checked the import/audit busy flag but missed pending book deletions. A delayed `delete_novel` response allowed the close handler to request window destruction before deletion finished. The library now exposes a synchronous `busy()` query combining that existing flag with its owned deletion set; the close handler uses this query before draining progress or destroying the window.

Deletion ownership starts before waiting for an issued progress write and ends in the existing `finally` block after success or failure. Duplicate requests for the same book do not release the original task. Completing one deletion cannot release another book's deletion or clear an independent import/audit operation. While any such operation remains active, close keeps the window open and asks the user to retry later; completion does not automatically close it. Failed deletion retains the book and queued progress for a later save.

Three regressions exercise the actual shelf/store/close handlers with controlled native boundaries: successful deletion with repeated close requests, failure after an in-flight progress write, and overlapping deletions with independent busy work. All three first failed against the previous implementation. [The CLOSE-01 result](evidence/2026-10-24-close/result.json) records final verification and source identities. Real desktop event delivery and process-exit data effects remain unqualified; earlier evidence is unchanged.

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

The historical cleanup index is [2026-10-20-cleanup/result.json](evidence/2026-10-20-cleanup/result.json). REPAIR-01 uses [2026-10-23-repair/result.json](evidence/2026-10-23-repair/result.json) to identify its final local checks and retained diagnostic runs. RESIDENT-01 uses [2026-10-23-resident/result.json](evidence/2026-10-23-resident/result.json). Other results are linked from [tasks.md](tasks.md), [worklog.md](worklog.md), and [architecture.md](architecture.md).

Retained receipts describe their original execution inputs and outcomes. Their log, backup, and run-directory references identify local diagnostic material, not files promised in a fresh clone. Preserve original hashes and outcomes. Missing local material limits replay; it does not become a passing check. Do not rerun tests or rewrite results merely to replace excluded artifacts.

Existing local artifacts are excluded by explicit paths in .git/info/exclude. New runs still need admission review; this local list is not a future-run policy. Stage approved paths explicitly. Git does not back up ignored or untracked diagnostics.

Original-book tests require SAGA_SAMPLE and are separate from the default suite. Scoped tests/builds do not establish desktop acceptance, complete coverage, MC/DC, targeted mutation qualification, other-platform support, or redistribution readiness. [tasks.md](tasks.md) records these gaps.

The user controls application lifecycle, commits, and releases. CLEAN-01 did not start or stop services, install dependencies, migrate live books, or approve a release. Later commits require separate authorization.
