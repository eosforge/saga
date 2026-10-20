[English](style.md) | [简体中文](style_CN.md)

# 编码风格

[编码规范](coding.md)约束手写生产代码、测试、模式和工具。[中文译文](coding_CN.md)在获得授权的提交前同步。

## 项目差异

当前未登记项目覆盖规则或例外。所有支持的语言沿用基线。后续获批的差异须在此记录范围、授权依据、替代规则及执行方式。

## 工具配置与限制

| 配置 | 职责 |
| --- | --- |
| [.editorconfig](../.editorconfig) | UTF-8、LF、空格；常规缩进 2 空格，Rust 4 空格 |
| [.gitattributes](../.gitattributes) | Git 中的文本使用 LF；冻结工作日志保留原始字节 |
| [rustfmt.toml](../rustfmt.toml)、[rust-toolchain.toml](../rust-toolchain.toml) | 锁定 Rust 格式化器及 160 列宽目标 |
| [前端脚本检查器](../apps/windows/tools/style.cjs) | 已安装的 TypeScript 格式化器、命名函数显式返回类型、PascalCase Vue 文件名 |
| [development.json](../development.json) | 有时限的本地检查命令与环境 |

模板/CSS 格式及完整的语义命名/注释约束仍需审核。这些工具不替代基线义务，也不将缺失检查视为通过。合格性认定缺口见[维护文档](maintenance_CN.md)。
