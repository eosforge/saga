[English](documentation.md) | [简体中文](documentation_CN.md)

# Documentation Governance

English Markdown is authoritative for implementation, recovery, and decisions. Chinese *_CN.md files are translations. Update English during ordinary work and reconcile translations immediately before an authorized commit.

## Ownership and placement

Keep only README and AGENTS document pairs at the root. Detailed documents belong under docs/:

| Document | Responsibility |
| --- | --- |
| architecture.md | Current components, dependencies, data contracts, and separately labeled target design |
| tasks.md | Active deliverables and qualification gaps |
| worklog.md | Concise milestone index linking to canonical documents |
| development.md | Engineering, verification, evidence, and acceptance baseline |
| coding.md | Cross-language coding and file organization baseline |
| style.md | Project coding differences and tool configuration |
| maintenance.md | Local workflow, failure guarantees, verification scope, and evidence limits |
| segmentation.md | TXT chapter recognition and retained review decisions |
| speech.md | Local narration constraints, portable deployment audit, and pending host verification |
| selection.md | Historical technology selection and its assumptions |
| proposals.md | Designs pending review |
| evidence/ | Selected, cited verification receipts and results |
| chronicle/ | Frozen historical records and integrity metadata |

Keep one canonical document per fact. README and the worklog link to specifications rather than repeat them. Add a document only for a distinct maintained responsibility.

## Recovery and translation

Read [AGENTS.md](../AGENTS.md), [architecture.md](architecture.md), [tasks.md](tasks.md), and [worklog.md](worklog.md), then the applicable standards and task contracts. Routine recovery excludes translations and archived evidence unless the task requires them.

Before an authorized commit:

1. Compare changed English documents with their Chinese companions.
2. Match meaning, headings, identifiers, tables, status, numeric limits, and links.
3. Verify that every maintained English document has a companion and that no translation adds a guarantee or changes a requirement.
4. Commit synchronized pairs together. Unresolved differences prevent a claim of translation completion.

This checkpoint grants no commit authority. English governs discrepancies during ongoing work. Frozen historical archives retain their original wording and bytes.

## Terminology

Use the following terms consistently. Preserve code identifiers, command names, protocol fields, and status codes.

| English | Chinese | Scope |
| --- | --- | --- |
| verification | 验证 | Checks against a stated contract |
| validation | 校验 | Input, structure, or configuration validity checks |
| acceptance | 验收 | Acceptance of a named deliverable |
| qualification | 合格性认定 | Determining whether engineering or release requirements are satisfied; distinct from running tests |
| gate | 门禁 | Required acceptance or release decision |
| evidence / receipt | 证据 / 回执 | Supporting records / a recorded execution result |
| retain / revoke retain | 保留 / 撤销保留 | Audit actions; preserve the approved UI labels |
| read-along | 伴读 | Audio-linked reading and its controls |
| native host | 原生宿主 | The Rust/Tauri host |
| native tool output | 工具原始输出 | Output observed from an actual tool process |
| content-addressed storage | 内容寻址存储 | CAS |

## Archives and evidence

Keep milestone entries to one or two lines. Existing frozen worklogs remain under chronicle/ with their source paths and SHA-256 metadata. Historical timings and readiness statements do not establish current measurements.

[.gitattributes](../.gitattributes) preserves frozen worklog bytes without text conversion; their stored content must match the registered digests.

Track evidence selectively:

1. Commit only structured verification receipts and definitive results explicitly linked from tasks.md, worklog.md, maintenance.md, or architecture.md for active milestones or gap closures.
2. Exclude pre-modification ZIP backups, redundant source snapshots such as before/, dynamically generated per-run directories, raw execution logs, and uncited intermediate records.
3. Keep original failures and other required diagnostic records in declared local or controlled storage. Excluding them from Git does not authorize deleting them. Relative references in retained receipts may identify local-only records; [maintenance.md](maintenance.md) defines that boundary.
4. Review evidence when a task is superseded or a milestone is archived. Remove expired or unnecessary records from tracking without changing historical execution results or hashes.

Do not ignore the entire evidence directory or admit files merely by extension. Stage approved paths explicitly. Local unsealed receipts are diagnostic evidence, not independent release approval.
