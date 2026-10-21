[English](worklog.md) | [简体中文](worklog_CN.md)

# 里程碑索引

当前事实由所链接的文档负责。早期结果仅描述原有输入及范围，不代表当前就绪状态。

| 日期 | 里程碑 | 权威文档 |
| --- | --- | --- |
| 2026-10-19 | SEG-01：分卷选择标题层级及重导入替换，保留全部 3,165 个明确原文标题和两处真实重复章号。 | [契约](segmentation_CN.md)、[回执](evidence/2026-10-19-segmentation.json) |
| 2026-10-19 | RET-01：持久化保留/撤销保留决策、隔离草稿、原子保存及确切标题定位；桌面审核仍待完成。 | [契约](segmentation_CN.md)、[回执](evidence/2026-10-19-retention.json) |
| 2026-10-20 | CLEAN-01：修正存储失败处理、路径、句子/状态处理、原型功能声明及本地检查入口。 | [维护](maintenance_CN.md)、[缺口](tasks_CN.md) |
| 2026-10-20 | 详细文档移入 docs/；采用英文权威来源规则，在获得授权的提交前校准翻译。 | [治理](documentation_CN.md)、[Agent 规则](../AGENTS.md) |
| 2026-10-20 | 保留现有图标资产，将侧边栏图标/字标整体居中；折叠后图标单独居中。 | [架构](architecture_CN.md)、[应用](../apps/windows/src/App.vue) |
| 2026-10-20 | ICON-01：更新陈旧的 Windows 可执行文件图标资源；重建后的六个图像帧与该里程碑的 ICO 匹配。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20-icons.json) |
| 2026-10-20 | ICON-02：导出透明圆角图标，检查 PNG/ICO 角点及可执行文件资源；原生/前端构建通过。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20-rounded-icons/result.json) |
| 2026-10-20 | ICON-03：裁切母版，去除导出留白，保留透明圆角，使侧边栏渲染遵循资产轮廓。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20-filled-icons/result.json) |
| 2026-10-20 | READ-01：三个阅读工具栏图标、字号设置及依音频状态显示的伴读控件。TXT 隐藏伴读条；.saga 导入与真实播放尚未实现。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20-reader/result.json) |
| 2026-10-20 | DOC-01：筛选八份已引用的证据结果，将临时产物移出追踪，并在获批的修正提交前校准项目文档与术语。 | [治理](documentation_CN.md)、[维护](maintenance_CN.md) |
| 2026-10-20 | STORE-01：256 KB 章节分块段式存储、SQLite 纯索引切片、阅读进度同步及书架工具栏精简布局。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20T09-37-13-999Z-f9b0218c/receipt.json) |
| 2026-10-20 | STORE-02：多章整体 Deflate 压缩分块严格限制于 256 KB、无重复试压流式贪心打包及按块缓存解压。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-20T10-54-23-331Z-ba198f65/receipt.json) |
| 2026-10-21 | SPEC-01：定义 .saga 标准 ZIP 容器规范，包含二进制清单、256 KB 正文切片及懒加载伴读时间轴。 | [架构](architecture_CN.md)、[任务台账](tasks_CN.md) |
| 2026-10-21 | PKG-01：接入 .saga ZIP 容器格式、原生导入导出引擎、修复阅读视口章节错位并支持无缝拖拽导入。 | [架构](architecture_CN.md)、[任务台账](tasks_CN.md) |
| 2026-10-21 | PKG-02：纯单文件 .saga 磁盘存储架构（无目录解压）、全内存切片打包入库，并彻底清除历史兼容回退代码。 | [架构](architecture_CN.md)、[回执](evidence/2026-10-21T09-40-46-764Z-64234f34/receipt.json) |

## 历史归档

[英文](chronicle/2026-10-19-worklog.md)和[中文](chronicle/2026-10-19-worklog_CN.md)工作日志已冻结。[完整性元数据](chronicle/2026-10-19-worklog.json)记录源路径和 SHA-256 摘要。其措辞和链接保持不变；当前导航使用本索引。
