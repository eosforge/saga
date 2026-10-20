[English](style.md) | [简体中文](style_CN.md)

# Code Style

[The coding standard](coding.md) governs handwritten production code, tests, schemas, and tools. [Its Chinese translation](coding_CN.md) is synchronized before authorized commits.

## Project differences

No project overrides or exceptions are registered. All supported languages inherit the baseline. Record any subsequently approved difference here with its scope, authority, replacement rule, and enforcement.

## Tool configuration and limits

| Configuration | Responsibility |
| --- | --- |
| [.editorconfig](../.editorconfig) | UTF-8, LF, spaces; general indentation 2, Rust 4 |
| [.gitattributes](../.gitattributes) | LF text in Git; frozen worklogs retain their original bytes |
| [rustfmt.toml](../rustfmt.toml), [rust-toolchain.toml](../rust-toolchain.toml) | Pinned Rust formatter and 160-column target |
| [Frontend script checker](../apps/windows/tools/style.cjs) | Installed TypeScript formatting, explicit named-function return types, PascalCase Vue filenames |
| [development.json](../development.json) | Bounded local check commands and environment |

Template/CSS formatting and complete semantic naming/comment enforcement still require review. These tools do not replace baseline obligations or count missing checks as passed. See [maintenance](maintenance.md) for qualification gaps.
