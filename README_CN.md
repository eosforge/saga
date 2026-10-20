[English](README.md) | [简体中文](README_CN.md)

# Saga

Saga 是基于 Vue、TypeScript、Rust 和 Tauri 的本地 TXT 阅读器原型，当前宿主为 Windows。跨平台阅读、音频朗读和内容寻址同步仍处于规划阶段。

## 当前功能

- 原生 TXT 导入，支持编码检测、卷章识别及进度反馈。
- SQLite 书架、按需加载章节、连续阅读，以及章节核验中的持久化保留/撤销保留决策。
- 在浏览器内存中预览完整 TXT，不经过原生解析，刷新后不保留。
- 三个图标的阅读工具栏及字号设置。纯文本书籍隐藏伴读控件。

音频合成、真实音频播放、时间轴对齐、同步、EPUB 和 .saga 导入及打包发布尚未实现。网关仅提供健康检查和 ping 端点。本地检查不代表完成工程或发布合格性认定；详见[待办任务](docs/tasks_CN.md)。

## 仓库结构

| 路径 | 职责 |
| --- | --- |
| apps/windows/ | Vue 客户端与 Tauri 宿主 |
| packages/reader/ | TXT 解码、章节识别及结构校验 |
| protocol/ | Rust 契约与生成的 TypeScript 绑定 |
| server/gateway/ | Axum 健康检查/ping 骨架 |
| tools/ | 本地检查与执行回执 |
| docs/ | 架构、规范、任务、维护与证据 |

根目录仅保留 README 和 [AGENTS](AGENTS.md) 两组中英文文档。英文为权威来源，中文译文在获得授权的提交前校准。

## 开发与验证

使用已安装的工具链和锁定依赖。[development.json](development.json) 登记已验证环境及有时限的检查命令；[rust-toolchain.toml](rust-toolchain.toml) 锁定 Rust 版本。全新检出需要相应工具链、Windows 原生构建前置条件、Node，以及 apps/windows/pnpm-lock.yaml 锁定的依赖。

在仓库根目录执行：

    node tools/check.cjs validate
    node tools/check.cjs collect

validate 不启动测试。collect 单次执行已登记检查，并在 docs/evidence/ 下保留本地日志和源内容哈希。检查失败、超时或源内容漂移时返回失败。生成的运行目录仅保存在本地，不纳入提交；仅追踪[符合准入规则的回执](docs/documentation_CN.md)。采集使用已安装依赖，不启动应用或网关。

前端检查可在 apps/windows/ 下运行 pnpm test、pnpm lint、pnpm typecheck 或 pnpm build。执行及证据边界见[维护文档](docs/maintenance_CN.md)。

## 文档索引

- [架构](docs/architecture_CN.md)、[任务与缺口](docs/tasks_CN.md)、[维护](docs/maintenance_CN.md)
- [章节切分与保留决策](docs/segmentation_CN.md)
- [工程规范](docs/development_CN.md)、[编码规范](docs/coding_CN.md)、[项目风格](docs/style_CN.md)
- [历史技术选型](docs/selection_CN.md)、[提案](docs/proposals_CN.md)、[里程碑](docs/worklog_CN.md)
- [文档治理与术语](docs/documentation_CN.md)

Cargo 声明为 MIT OR Apache-2.0。许可证文本和第三方再分发声明尚未补齐；当前检出并非完整的发布包。
