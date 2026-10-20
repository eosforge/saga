[English](tasks.md) | [简体中文](tasks_CN.md)

# 任务与合格性认定缺口

本台账负责当前交付任务与合格性认定缺口。已完成里程碑仅描述原有范围，不代表当前具备发布资格。对尚未测量的合格性认定缺口，未批准截止日期或例外。

## 工程任务

| 编号 | 范围 | 状态 | 登记日期 | 交付内容 | 验收条件 |
| --- | --- | --- | --- | --- | --- |
| TASK-01 | 工作区初始化 | CLOSED | 2026-10-18 | Rust 工作区、含 ts-rs 绑定和 BLAKE3 的 protocol、网关及 Vue/Tauri Windows 宿主。 | 初始化时：7 项 Rust 测试及前端构建通过，无错误。 |
| TASK-02 | packages/reader | OPEN | 2026-10-18 | EPUB/TXT 流式解析、分页与虚拟滚动。 | 渲染样例 EPUB，支持字体、边距、行高及多列设置。 |
| TASK-03 | packages/player, packages/speech | OPEN | 2026-10-18 | 分句、音频生成、时间轴对齐与伴读滚动。 | 音频驱动 .active-sentence 高亮及平滑居中。 |
| TASK-04 | server/cas, packages/storage | OPEN | 2026-10-18 | CAS 哈希及 POST /api/cas/check 预检协议。 | 重复哈希在一秒内完成处理，无需重新上传实体内容。 |
| TASK-05 | tools, packages/speech | OPEN | 2026-10-18 | SpeechProvider 支持云服务（Edge-TTS、OpenAI/Minimax）及本地端点（CosyVoice/GPT-SoVITS）。 | 使用默认 Edge-TTS 及本地模拟/OpenAI 兼容语音端点，分别验证生成和句子对齐。 |
| TASK-06 | protocol, packages/storage | OPEN | 2026-10-19 | 提案 2 中的存储引擎与紧凑 .sgtl 时间轴。 | 二进制序列化往返验证、相对 JSON 超过 95% 的压缩率及 O(1) 章节定位基准测试。 |
| TASK-07 | 仓库整理 | IMPLEMENTED; LOCAL VERIFICATION | 2026-10-20 | CLEAN-01 存储/错误修复、可移植路径、分句/生命周期整合、本地检查和文档。 | [契约与证据](maintenance_CN.md)。正式合格性认定仍属于 GAP-04。 |

## 缺口与剩余验证

| 编号 | 范围 | 状态 | 契约与证据 |
| --- | --- | --- | --- |
| GAP-01 | 原生选择框与导入反馈 | 用户反馈已解决；原生延迟未测量 | 选择框完成后才开始导入；取消不启动工作；重复点击只启动一次操作。读取/解析从零上报处理字节，不可计量阶段报告活动状态。持久化和 IPC 结果完成后才显示成功。在所记录的里程碑中，31 项 Rust 测试、5 项前端测试及前端构建通过。用户确认延迟由远程文件导致且已解决。[证据](evidence/2026-10-19-import.json)。 |
| GAP-02 | TXT 层级与重导入 | 已实现；样本验证完成 | SEG-01 保留明确章节内的列表标签，使用具有一致性的降级层级，原子替换章节并避免重复书架项目。在所记录的里程碑中，28 项 Rust 测试、6 项前端测试及构建通过。全书检查保留全部 3,165 个明确标题；真实重复章号 2656、2680 连同提示一并保留。[契约](segmentation_CN.md)、[证据](evidence/2026-10-19-segmentation.json)。 |
| GAP-03 | 保留决策与定位 | 已实现；桌面审核待完成 | RET-01 持久化保留/撤销保留决策，原子保存章节和决策，定位确切标题并隔离草稿。范围内自动检查及原书隔离持久化检查通过。[契约](segmentation_CN.md)、[证据](evidence/2026-10-19-retention.json)。 |
| GAP-04 | 规范采纳与发布合格性认定 | OPEN | development.json 已登记组件等级。正式模式/审批注册表、可信证据封签、义务展开、实测覆盖率/MC-DC、定向变异测试结果、完整命名/注释约束及发布门禁仍然缺失。桌面交互、打包存储、有界阅读内存、崩溃/断电持久性及其他平台尚未完成合格性认定。未声明通过分母、已接受例外或截止日期。[维护文档](maintenance_CN.md)。 |
| GAP-05 | 开源分发 | OPEN | Cargo 声明为 MIT OR Apache-2.0。许可证文本及依赖/资产声明尚未补齐。安装包、签名、支持矩阵和发布产物尚未完成合格性认定。 |
