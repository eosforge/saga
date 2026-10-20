[English](README.md) | [简体中文](README_CN.md)

# Saga

Saga is a local TXT reader prototype built with Vue, TypeScript, Rust, and Tauri. Windows is the current host. Cross-platform reading, audio narration, and content-addressed synchronization remain planned.

## Current capabilities

- Native TXT import with encoding detection, volume/chapter recognition, and progress feedback.
- A SQLite bookshelf, chapters loaded on demand, continuous reading, and persistent retain/revoke decisions in chapter review.
- Complete TXT previews in browser memory, without native parsing or persistence across refreshes.
- A three-icon reader toolbar with font settings. Text-only books hide read-along controls.

Audio synthesis, real audio playback, timeline alignment, synchronization, EPUB and .saga import, and packaged releases are not implemented. The gateway exposes health and ping endpoints only. Local checks do not establish engineering or release qualification; see [outstanding tasks](docs/tasks.md).

## Repository

| Path | Responsibility |
| --- | --- |
| apps/windows/ | Vue client and Tauri host |
| packages/reader/ | TXT decoding, chapter recognition, and structural validation |
| protocol/ | Rust contracts and generated TypeScript bindings |
| server/gateway/ | Axum health/ping scaffold |
| tools/ | Local checks and execution receipts |
| docs/ | Architecture, standards, tasks, maintenance, and evidence |

Only README and [AGENTS](AGENTS.md) document pairs remain at the root. English is authoritative; Chinese translations are reconciled before authorized commits.

## Development and verification

Use the installed toolchain and locked dependencies. [development.json](development.json) records the verified environment and bounded check commands; [rust-toolchain.toml](rust-toolchain.toml) pins Rust. A fresh checkout requires that toolchain, Windows native build prerequisites, Node, and the dependencies locked in apps/windows/pnpm-lock.yaml.

From the repository root:

    node tools/check.cjs validate
    node tools/check.cjs collect

validate starts no tests. collect runs the registered checks once and retains local logs and source hashes under docs/evidence/. It fails on a failed check, timeout, or source drift. Generated run directories remain local and are excluded from commits; only [approved receipts](docs/documentation.md) are tracked. Collection uses installed dependencies and starts no application or gateway.

For frontend checks, run pnpm test, pnpm lint, pnpm typecheck, or pnpm build inside apps/windows/. See [maintenance](docs/maintenance.md) for execution and evidence boundaries.

## Documentation

- [Architecture](docs/architecture.md), [tasks and gaps](docs/tasks.md), [maintenance](docs/maintenance.md)
- [Chapter segmentation and retain decisions](docs/segmentation.md)
- [Engineering standard](docs/development.md), [coding standard](docs/coding.md), [project style](docs/style.md)
- [Historical technology selection](docs/selection.md), [proposals](docs/proposals.md), [milestones](docs/worklog.md)
- [Documentation governance and terminology](docs/documentation.md)

Cargo declares MIT OR Apache-2.0. License texts and third-party redistribution notices remain incomplete; this checkout is not a completed release package.
