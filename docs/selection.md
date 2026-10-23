[English](selection.md) | [简体中文](selection_CN.md)

# Technology Selection

This historical record preserves the selected direction and evaluation assumptions. It does not describe completed integration. Sizes, timings, prices, and platform comparisons are unverified historical estimates, not current service guarantees. [Architecture](architecture.md) records the implementation; release qualification remains open.

## Requirements and alternatives

The target covers Android, iOS, iPadOS, macOS, Windows, Linux, and standard Web/PWA. It requires Web-compatible EPUB/TXT/HTML typography, background audio with system media controls, a small runtime footprint, local persistence, and high-throughput CAS hashing.

| Candidate | Evaluated approach | Main tradeoffs recorded |
| --- | --- | --- |
| Tauri 2.0 + Web Core | Rust host with system WebViews | Selected for shared Web typography and native capabilities; mobile-specific behavior may require plugins or Swift/Kotlin bindings |
| Flutter | Dart with Impeller/Skia rendering | Web text integration and EPUB require additional integration; larger runtime |
| React Native / Expo | JavaScript and native widgets | Linux coverage and separate Windows/macOS/Web toolchains were concerns; EPUB still needs WebView integration |
| Capacitor / Ionic + Electron | Mobile WebViews and desktop Chromium/Node | Separate host toolchains and a larger desktop footprint |
| KMP + Compose | Shared Kotlin logic and Compose UI | Web maturity and rich EPUB integration were concerns |

The original ratings were qualitative judgments, not benchmark results. The evaluation used these unverified estimates: Tauri 3–15 MB with cold memory below 60 MB; Flutter 30 MB+; Electron installers 100–200 MB and runtime memory 200–400 MB. Target criteria included sub-second startup, bundles below 15 MB, memory below 60 MB, and code reuse above 95%. None is a measured Saga result.

## Client and server choices

The selected client direction is Tauri 2.0 + Web Core using TypeScript, Vue 3, and Tailwind CSS.

- apps/web and packages/* would own typography, pagination, virtual scrolling, playback coordination, sentence highlighting, the shelf, and community interaction.
- apps/desktop and apps/mobile would own native SQLite, hashing, file associations, tray/window integration, and platform audio services such as Android foreground services and iOS AVAudioSession.
- The aim is one Web core reused across seven targets, with iPadOS layouts and touch behavior adapted where necessary.

The frontend direction uses Vue 3 Composition API, Pinia, and Vite, with Shadcn-Vue/Radix-Vue and Tailwind for accessible primitives and source-owned styling. Mobile drawers and desktop/tablet popovers would share behavior. These are selection decisions, not a list of installed dependencies.

Historical frontend assumptions were 3–10 minute speech tracks, 200–500 active sentence nodes, 10–30 Hz playback updates, highlight updates below 0.2 ms and 0.3% CPU, and CSS below 20 KB. They require measurement. Current continuous reading trims distant offscreen chapters, but a large individual chapter remains fully rendered and no total DOM/memory budget is established; see [maintenance](maintenance.md).

The selected server direction is Rust with Tokio, Axum, and SQLx. Shared protocol types, CAS hashing, and HLC logic would reduce duplicated contracts. Rust's type and ownership checks support that choice without proving all runtime behavior. Historical throughput and memory assumptions were multiple GB/s and 10–20 MB per instance; they remain unmeasured.

## Speech and reading

SpeechProvider would support default cloud synthesis and configurable user-hosted models:

- Edge-TTS was selected as the default direction. Additional candidates include OpenAI, Minimax, Volcengine/Doubao, and Azure Speech.
- Local endpoints may expose CosyVoice, ChatTTS, GPT-SoVITS, or F5-TTS over HTTP/REST/WebSocket. Models run separately; the client does not bundle PyTorch.
- Output would be Opus or M4A/AAC audio identified by content hash, with SentenceChunk IDs mapped to [start_ms, end_ms].
- The proposed deduplication key is AudioGenHash = Hash(CleanedText + EngineId + VoiceId + Speed + Pitch).

Cost, availability, timestamp support, and setup assumptions for cloud providers require review before integration.

The selected reader direction is an in-house packages/reader engine using Web DOM and Vue instead of an iframe-based library such as epub.js. The evaluation identified gesture propagation, theming, format separation, and cross-frame highlighting as concerns.

EPUB HTML and TXT chapters would normalize into Chapter { id, title, chunks: SentenceChunk[] }. Punctuation-aware spans would link text to audio. CSS multi-column pagination would use column-width: 100vw, column-gap: 0, column-fill: auto, and scroll-snap-type: x mandatory. Continuous mode would virtualize visible paragraphs, including long single chapters of 100,000 words. activeChunkId and .active-sentence would drive highlighting; scrollIntoView would run at sentence transitions. These planned limits are not current implementation claims.

## Hosting and object access

The selected hosting direction separates:

| Layer | Planned responsibility |
| --- | --- |
| GitHub | Release distribution and public-domain/OPDS 1.2/2.0 catalogs |
| Cloudflare R2 | Private CAS book/audio objects and HTTP Range streaming |
| Cloudflare Pages | Web/PWA hosting |
| Rust Axum on VPS/Docker or AWS Lambda | Authentication, HLC sync, CAS metadata, and presigned URLs |

Historical cost assumptions included a 2 GB GitHub release-asset limit, AWS S3 egress near $0.09/GB, and R2 egress at $0.00/GB. They are not current pricing or permanent guarantees.

The target keeps ACCESS_KEY_ID and SECRET_ACCESS_KEY on the server, with a private bucket. After JWT authorization, GET /api/audio/stream?chapter_id=... would return a one-hour HMAC-SHA256 presigned GET URL. Clients would stream directly with Range requests and 206 Partial Content responses. A CAS miss after POST /api/cas/check would return a 15-minute presigned PUT URL for direct upload.

No account, sync, CAS service, cloud deployment, or multi-platform release is established by this record.


