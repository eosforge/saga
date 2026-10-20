[English](segmentation.md) | [简体中文](segmentation_CN.md)

# TXT Chapter Segmentation

This document owns the `SEG-01` correction authorized on 2026-10-19: numbered paragraph labels inside Chapter 225 must not create chapters before Chapter 226. The scope includes parsing and replacement of a book when the user explicitly imports the same file again.

## Recognition and Selection

1. Scan lines for lexical candidates using the existing encoding, heading, quote, metadata, and narrative filters. Store candidate offsets rather than copying body text or retaining every line.
2. Select a heading tier independently within each volume. Explicit chapters (`章`, `回`, `话`, `讲`, `篇`, `Chapter`) take precedence over sections (`节`, `Section`), paragraphs (`段`), and bare numbered titles, in that order.
3. A weaker tier is selected only when no explicit chapters exist and that tier contains an increasing pair of numbered candidates. Gaps are allowed; they remain visible to the sequence validator. An isolated weak label remains body text.
4. Keep prefaces, extras, epilogues, and volume boundaries. Do not suppress genuine explicit chapter gaps, duplicates, or resets to make validation appear clean. Explicit chapter titles may end in literary ellipses or commas.
5. Assemble sections in a second linear scan. Rejected candidate lines retain their original body text and relative order. Existing newline normalization, paragraph preservation, and lazy sentence chunking remain in place. Progress is monotonic across recognition and assembly.

This is deterministic structural inference. A document containing only numbered lists can still be ambiguous, and mixed top-level/subsection conventions within a volume may need manual review. It does not promise perfect segmentation for every TXT file.

## Persistence and Compatibility

- Public parser signatures and serialized `Novel`/`Section` shapes remain unchanged. Selected boundaries and ordinal section identifiers can change.
- Reimport replaces the matching shelf item and atomically replaces its SQLite metadata and complete section set. Old trailing sections cannot survive a shorter parse; a failed replacement rolls back to the previous book.
- Reimport rebuilds chapter structure and replaces prior manual chapter edits for that file. Existing databases and cached books are not automatically reparsed or migrated by this change.

## Verification

### Retain Decisions and Navigation (`RET-01`)

The user approved the operation label **保留** (retain). It records a review decision without changing chapter titles, numbering, text, or reading order. The audit page separates pending and processed findings; retained findings remain inspectable under processed/all filters and support **撤销保留** (revoke retain). The shelf's **核验** (review) action reopens persisted findings without reimporting the source.

- Decisions take effect when the user saves. Sections and decisions commit in one SQLite transaction; failed saves keep the draft editable and display an error. Skipping or returning without saving discards draft changes. Editable section/chunk objects and undo snapshots are isolated while immutable body strings are shared.
- SQLite `reviews(book_id, fingerprint)` stores confirmed findings separately from chapter data. Fingerprints include a version, finding type/description, section position, current/preceding chapter content, and matching titles in the same volume. Changed relevant titles, bodies, numbering context, or positions invalidate the old decision. Unchanged reimports of the same content-addressed book retain confirmations; a changed source file is a different book identity and does not inherit them.
- `ValidationAnomaly.key` and `retained` are additive protocol fields with deserialization defaults. The reader's structural report remains unchanged in meaning: `is_valid` is false whenever structural findings exist, even if retained. The native review layer attaches persisted decisions. The interface derives pending items from findings without `retained`; finding a numbering conflict does not itself block reading.
- Backend save paths revalidate requested keys against the submitted sections, remove stale decisions, and propagate failures. Existing source files and live books are not automatically marked retained. The additive review table is initialized through the application's existing database setup; no source renumbering or bulk content migration is involved.
- Navigation uses the exact section identity and scrolls the heading itself into view with a direct jump. It does not match by title or fall back to a deleted section's old position. Undo locates the selected finding's current section position. A regressed volume points to its actual first section rather than section zero.

The retained-state tests use isolated databases, including closing/reopening a temporary database and an injected transaction failure. Frontend tests exercise real handlers with mocked IPC and DOM scroll targets. The original-book test independently retains the two reviewed duplicate numbers in an in-memory database, checks that a partial confirmation leaves the other pending, then verifies unchanged sections after save and reimport. See [the retention receipt](evidence/2026-10-19-retention.json). Actual Windows window interaction and pixel-level scroll positioning remain for user review; these checks do not claim that desktop UI acceptance or the formal development gate passed.

### Segmentation Evidence

`packages/reader/tests/segmentation.rs` covers the Chapter 225/226 example, nested sections and lists, prefaces, fallback formats, isolated labels, per-volume conventions, genuine sequence anomalies, bracketed/CRLF titles, and literary punctuation. Existing reader tests protect decoding, sentence/paragraph preservation, metadata, and progress.

The native in-memory SQLite tests cover shorter reimports and an injected insertion failure. Frontend handler tests cover replacement without duplicate shelf entries. Commands and results are recorded in [the evidence receipt](evidence/2026-10-19-segmentation.json); they are scoped checks, not a formal development qualification gate.

The optional read-only whole-book test uses `SAGA_SAMPLE` and is ignored in the default suite. It checks that both paragraph labels remain in Chapter 225, Chapter 226 follows it, and every explicit Arabic-numbered source chapter retains its exact title, order, and multiplicity. The user subsequently supplied the original file, `D:\texts\修真聊天群.txt` (27,432,041 bytes). The final run passed: all 3,165 explicit source headings were retained, including the 12 previously omitted titles; the parser produced 3,167 sections across 16 volume groups. One debug-build parse took 211 ms; this is not an end-to-end import latency measurement.

All reported conflicts were reviewed against the original source. There are no warnings and exactly two duplicate-number anomalies: Chapter 2656 occurs at source lines 233189 and 233253 with different titles and consecutive narrative bodies; Chapter 2680 similarly occurs at lines 234672 and 234743. These are genuine source numbering conflicts, not false section boundaries or duplicate content. Retain both sections in each pair and keep the validation alerts; do not silently delete, merge, or renumber them. Thus the preservation test passes while the validation report correctly remains `is_valid=false`. The source file and live library were not modified by this review.

Raw logs and failure records remain local. Tracked receipts preserve their original inputs and outcomes; a fresh clone does not include complete diagnostic material. See [maintenance](maintenance.md) for storage and replay limits.

## Reference

[Readest's TXT parser](https://github.com/readest/readest/blob/main/apps/readest-app/src/utils/txt.ts) tries prioritized chapter patterns and checks the resulting splits. Saga adopts that general separation of candidates and selection, with its own per-volume hierarchy implementation; no external runtime dependency or source implementation was copied.
