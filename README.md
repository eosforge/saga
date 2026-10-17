# Saga (萨迦)

> **全平台 + HTML 电子书阅读与听书套件**  
> *Cross-Platform Digital Book Reader & Audio Narrative Suite*

`saga` 取名自北欧神话中的历史与叙事女神。它旨在打破传统电子书与有声书的界限：通过本地下载与解析各类书籍资源，利用高品质神经网络 TTS 引擎将其自动转化为连贯的有声书音源，并借助云端同步能力，实现多端无缝的“边读边听”体验。

---

## 核心设计理念

1. **HTML 驱动的统一全平台界面（Web-First Architecture）**
   * 阅读器与听书播放器基于标准现代 HTML5 / Web 技术栈构建，保证跨桌面端（Windows / macOS / Linux）、移动端（Android / iOS）及浏览器端的极致统一与高可定制性。
2. **文本到音源的自动化流水线（TTS Engine & Alignment）**
   * 智能正文分段、排版清洗与章节解析。
   * 支持高品质云端神经语音（如 Edge-TTS）与本地极速离线推理语音（如 Piper / Sherpa-ONNX）。
   * **词句级时间戳对齐**：生成带精确行/句时间戳的音频，听书时正文自动跟进高亮滚动。
3. **本地与云端双轨同步（Local-First & Cloud Sync）**
   * 资源本地存储优先，保障无网状态下的极速阅读与离线收听。
   * 支持标准 WebDAV / S3 对象存储或自建服务端，同步阅读进度、书签、笔记与音频生成缓存。

---

## 系统模块规划

```
saga/
├── apps/
│   ├── web/               # HTML5 核心阅读器与播放器前端界面 (UI)
│   └── desktop/           # 跨平台桌面/移动端容器壳 (Tauri 2.0 / Native)
├── packages/
│   ├── reader-core/       # EPUB / TXT / Markdown 解析与排版渲染引擎
│   ├── tts-pipeline/      # 文本切片、TTS 语音合成与时间戳对齐处理器
│   └── sync-engine/       # WebDAV / S3 / 进度状态同步核心
└── docs/                  # 架构设计与技术规范文档
```

---

## 快速路线图

- [ ] **M1: 核心原型验证**
  - [ ] 基于 HTML 的 EPUB / TXT 极简翻页与排版渲染
  - [ ] 文本按句分词切片与 Edge-TTS 音频流合成
  - [ ] 音频播放与正文行级高亮联动
- [ ] **M2: 本地管理与下载器**
  - [ ] 书籍书架管理与本地资源导入
  - [ ] 批量章节静默转音频后台队列
- [ ] **M3: 云端双向同步**
  - [ ] WebDAV / 自定义后端进度与书签同步
  - [ ] 多端断点续听续读
