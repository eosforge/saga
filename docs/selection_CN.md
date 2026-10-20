[English](selection.md) | [简体中文](selection_CN.md)

# 技术选型

本历史记录保留已选定的方向和评估假设，不代表集成已经完成。体积、耗时、价格及平台对比均为未经验证的历史估计，不是当前服务保证。[架构文档](architecture_CN.md)记录实际实现；发布合格性认定尚未完成。

## 要求与备选方案

目标覆盖 Android、iOS、iPadOS、macOS、Windows、Linux 和标准 Web/PWA，要求具备兼容 Web 的 EPUB/TXT/HTML 排版、后台音频及系统媒体控制、较小的运行时占用、本地持久化和高吞吐 CAS 哈希计算。

| 候选 | 评估路线 | 已记录的主要取舍 |
| --- | --- | --- |
| Tauri 2.0 + Web Core | Rust 宿主与系统 WebView | 基于共享 Web 排版及原生能力选定；移动端特有行为可能需要插件或 Swift/Kotlin 绑定 |
| Flutter | Dart 与 Impeller/Skia 渲染 | Web 文本及 EPUB 需要额外集成；运行时较大 |
| React Native / Expo | JavaScript 与原生控件 | 评估关注 Linux 覆盖及独立的 Windows/macOS/Web 工具链；EPUB 仍需集成 WebView |
| Capacitor / Ionic + Electron | 移动 WebView 与桌面 Chromium/Node | 宿主工具链分离，桌面占用较大 |
| KMP + Compose | 共享 Kotlin 逻辑与 Compose 界面 | 评估关注 Web 成熟度及复杂 EPUB 集成 |

原评分是定性判断，不是基准测试结果。评估采用的未验证估计为：Tauri 3–15 MB、冷启动内存低于 60 MB；Flutter 30 MB 以上；Electron 安装包 100–200 MB、运行内存 200–400 MB。目标条件包括低于一秒的启动时间、低于 15 MB 的包体、低于 60 MB 的内存及超过 95% 的代码复用率。以上均非 Saga 实测结果。

## 客户端与服务端选型

客户端选定方向为 Tauri 2.0 + Web Core，使用 TypeScript、Vue 3 和 Tailwind CSS。

- apps/web 与 packages/* 负责排版、分页、虚拟滚动、播放协调、句子高亮、书架及社区交互。
- apps/desktop 与 apps/mobile 负责原生 SQLite、哈希、文件关联、托盘/窗口集成，以及 Android 前台服务、iOS AVAudioSession 等平台音频服务。
- 目标是七个平台复用一个 Web 核心，并按需适配 iPadOS 布局和触控行为。

前端方向采用 Vue 3 Composition API、Pinia 和 Vite，通过 Shadcn-Vue/Radix-Vue 与 Tailwind 提供无障碍基础组件及可直接维护源码的样式。移动端抽屉与桌面/平板浮层共享行为。这些是选型决策，不是已安装依赖清单。

历史前端假设为：3–10 分钟语音片段、200–500 个活动句子节点、10–30 Hz 播放更新、高亮更新时间低于 0.2 ms 且 CPU 占用低于 0.3%、CSS 低于 20 KB。以上需要测量；当前连续阅读尚未实现有界 DOM 窗口。

服务端选定方向为 Rust、Tokio、Axum 和 SQLx。共享协议类型、CAS 哈希与 HLC 逻辑可减少契约重复。Rust 的类型及所有权检查支持这一选择，但不证明全部运行时行为。历史吞吐和内存假设为每秒数 GB、每实例 10–20 MB，尚未测量。

## 语音与阅读

SpeechProvider 计划支持默认云端合成及可配置的用户自建模型：

- Edge-TTS 为选定的默认方向。其他候选包括 OpenAI、Minimax、Volcengine/Doubao 和 Azure Speech。
- 本地端点可通过 HTTP/REST/WebSocket 提供 CosyVoice、ChatTTS、GPT-SoVITS 或 F5-TTS。模型独立运行，客户端不内置 PyTorch。
- 输出为内容哈希标识的 Opus 或 M4A/AAC 音频，并将 SentenceChunk ID 映射到 [start_ms, end_ms]。
- 提议的去重键为 AudioGenHash = Hash(CleanedText + EngineId + VoiceId + Speed + Pitch)。

云服务的成本、可用性、时间戳支持及配置假设须在集成前复核。

阅读器选定方向为基于 Web DOM 和 Vue 的自研 packages/reader 引擎，而非 epub.js 等 iframe 库。评估关注手势传播、主题设置、格式分离和跨 iframe 边界的高亮。

EPUB HTML 与 TXT 章节计划统一为 Chapter { id, title, chunks: SentenceChunk[] }。按标点切分的片段关联文本与音频。CSS 多列分页使用 column-width: 100vw、column-gap: 0、column-fill: auto 和 scroll-snap-type: x mandatory。连续模式对可见段落进行虚拟化，包括 100,000 词的单个长章节。activeChunkId 与 .active-sentence 驱动高亮；scrollIntoView 在句子切换时执行。这些规划限制不代表当前实现。

## 托管与对象访问

托管选定方向按以下职责划分：

| 层次 | 规划职责 |
| --- | --- |
| GitHub | 发布分发及公版/OPDS 1.2/2.0 目录 |
| Cloudflare R2 | 私有 CAS 书籍/音频对象及 HTTP Range 流式传输 |
| Cloudflare Pages | Web/PWA 托管 |
| VPS/Docker 或 AWS Lambda 上的 Rust Axum | 认证、HLC 同步、CAS 元数据及预签名 URL |

历史成本假设包括 GitHub 发布资产上限 2 GB、AWS S3 出站流量约 $0.09/GB、R2 出站流量 $0.00/GB。这些不是当前价格或永久保证。

目标要求 ACCESS_KEY_ID 和 SECRET_ACCESS_KEY 仅保存在服务端，存储桶保持私有。完成 JWT 授权后，GET /api/audio/stream?chapter_id=... 返回有效期一小时、采用 HMAC-SHA256 的预签名 GET URL。客户端通过 Range 请求及 206 Partial Content 响应直接获取音频。POST /api/cas/check 未命中后，返回有效期 15 分钟的预签名 PUT URL，供客户端直接上传。

本记录不代表账号、同步、CAS 服务、云部署或多平台发布已经完成。


