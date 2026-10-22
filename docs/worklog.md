[English](worklog.md) | [简体中文](worklog_CN.md)

# Milestone Index

The linked documents own current facts. Earlier results describe their original inputs and scope; they do not establish current readiness.

| Date | Milestone | Canonical document |
| --- | --- | --- |
| 2026-10-19 | SEG-01: per-volume heading selection and replacement reimport retained all 3,165 explicit source headings and two genuine duplicate chapter numbers. | [Contract](segmentation.md), [receipt](evidence/2026-10-19-segmentation.json) |
| 2026-10-19 | RET-01: persistent retain/revoke decisions, isolated drafts, atomic saves, and exact heading navigation; desktop review remains pending. | [Contract](segmentation.md), [receipt](evidence/2026-10-19-retention.json) |
| 2026-10-20 | CLEAN-01: corrected storage failures, paths, sentence/state handling, prototype claims, and local check entry points. | [Maintenance](maintenance.md), [gaps](tasks.md) |
| 2026-10-20 | Moved detailed documents under docs/ and adopted English authority with translation reconciliation before authorized commits. | [Governance](documentation.md), [agent rules](../AGENTS.md) |
| 2026-10-20 | Preserved existing icon assets and centered the sidebar icon/wordmark as one group; the collapsed sidebar centers the icon alone. | [Architecture](architecture.md), [application](../apps/windows/src/App.vue) |
| 2026-10-20 | ICON-01: refreshed stale Windows executable icon resources; six rebuilt frames matched that milestone's ICO. | [Architecture](architecture.md), [receipt](evidence/2026-10-20-icons.json) |
| 2026-10-20 | ICON-02: exported transparent rounded icons and checked PNG/ICO corners and executable resources; native/frontend builds passed. | [Architecture](architecture.md), [receipt](evidence/2026-10-20-rounded-icons/result.json) |
| 2026-10-20 | ICON-03: cropped the master, removed export padding, retained transparent corners, and aligned sidebar rendering with the asset silhouette. | [Architecture](architecture.md), [receipt](evidence/2026-10-20-filled-icons/result.json) |
| 2026-10-20 | READ-01: three reader toolbar icons, font settings, and audio-dependent read-along controls. TXT hides read-along bars; .saga import and real playback remain unimplemented. | [Architecture](architecture.md), [receipt](evidence/2026-10-20-reader/result.json) |
| 2026-10-20 | DOC-01: selected eight cited evidence results, removed transient artifacts from tracking, and reconciled project documentation and terminology before the authorized amendment. | [Governance](documentation.md), [maintenance](maintenance.md) |
| 2026-10-20 | STORE-01: segmented 256 KB chapter packfile storage, index-only SQLite sections, progress synchronization, and streamlined shelf toolbar layout. | [Architecture](architecture.md), [receipt](evidence/2026-10-20T09-37-13-999Z-f9b0218c/receipt.json) |
| 2026-10-20 | STORE-02: multi-chapter unified Deflate compression strictly bounded by 256 KB per chunk file, streaming greedy packing without recompression, and cached chunk decompression. | [Architecture](architecture.md), [receipt](evidence/2026-10-20T10-54-23-331Z-ba198f65/receipt.json) |
| 2026-10-21 | SPEC-01: defined .saga standard ZIP container specification with binary manifest, 256 KB text slices, and lazy audio timeline. | [Architecture](architecture.md), [tasks](tasks.md) |
| 2026-10-21 | PKG-01: integrated .saga ZIP container format, native import/export engine, fixed reader chapter desync, and enabled seamless drag-and-drop ingestion. | [Architecture](architecture.md), [tasks](tasks.md) |
| 2026-10-21 | PKG-02: pure single-file .saga on-disk storage architecture without directory unpacking, in-memory chunk packaging, and elimination of legacy compatibility fallbacks. | [Architecture](architecture.md), [receipt](evidence/2026-10-21T09-40-46-764Z-64234f34/receipt.json) |
| 2026-10-22 | PERF-01: headroom-based zero-clone chunk compression, O(1) audit finding index, full-screen interaction blocking overlay during persistence, determinate 0-100% progress for import and audit saving, complete post-audit lifecycle teardown, and direct back-to-shelf navigation from reader. | [Architecture](architecture.md), [application](../apps/windows/src/App.vue) |
| 2026-10-22 | STORE-03: eliminated SQLite sections table, shifted chapter TOC directly to .saga manifest.bin with zero-copy volume lookup, reduced saga.db from 1,444 KB to ~8 KB, and implemented startup background pre-warming for instant reader launch. | [Architecture](architecture.md), [receipt](evidence/2026-10-22T10-22-08-229Z-6df95b30/receipt.json) |

## Historical archive

The [English](chronicle/2026-10-19-worklog.md) and [Chinese](chronicle/2026-10-19-worklog_CN.md) worklogs are frozen. [Integrity metadata](chronicle/2026-10-19-worklog.json) records source paths and SHA-256 digests. Their wording and links remain unchanged; use this index for current navigation.
