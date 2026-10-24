[English](speech.md) | [简体中文](speech_CN.md)

# Local Narration and Portable Deployment

SPEECH-01. Recorded: 2026-10-24. Status: AUDIT RECORDED; NOT IMPLEMENTED; HOST VERIFICATION PENDING.

The user authorized recording the portable deployment audit. This document records its findings and the established narration constraints; it does not authorize model installation, inference, integration, or release. Current application behavior remains in [architecture.md](architecture.md), and delivery work remains open in [tasks.md](tasks.md).

## Scope and established constraints

- Target the user's current Windows computer first. macOS delivery is deferred.
- Use one fixed narrator voice throughout a book. Emotion, pace, and pauses may vary without assigning different voices to characters.
- Preserve the original text. Text analysis supplies separate narration annotations; it does not rewrite the book or replace source sentences with model-generated prose.
- Deliver local model tools as relocatable directories, without depending on a system Python installation or an activated development environment.
- A deliverable must run on the actual target computer. Static compatibility findings and upstream memory figures do not establish that result.

## Target computer observed during the audit

| Resource | Observation on 2026-10-24 |
| --- | --- |
| Operating system | Windows 11 IoT Enterprise LTSC, x64, build 26100 |
| Processor | Intel Core i7-13700F, 16 cores / 24 logical processors |
| System memory | Approximately 63.8 GiB total; 41.2 GiB available at inspection |
| GPU | NVIDIA GeForce RTX 3080, 10,240 MiB total VRAM; compute capability 8.6 |
| Free VRAM | 8,522 MiB, approximately 8.3 GiB, at inspection |
| NVIDIA driver | 610.88 |
| Free storage | Approximately 87 GiB on D: and 171 GiB on C: |

Free memory and storage are snapshots, not reserved capacity. No model was loaded during this audit. Weight-file size is neither peak VRAM nor total package size.

## Speech model findings

The order below is an evaluation priority, not a measured sound-quality ranking or a final engine selection. The released model variant matters; larger research variants are not interchangeable with these artifacts.

| Candidate | Audited deployment path | Target-host disposition |
| --- | --- | --- |
| CosyVoice 3, Fun-CosyVoice3-0.5B-2512 | Private Python and baseline PyTorch inference; optional TensorRT and vLLM disabled. The official dependency file includes a Windows branch. | Priority candidate. Peak VRAM and sustained generation on this host remain unmeasured. |
| Qwen3-TTS-12Hz-1.7B-CustomVoice | Private Python, BF16, one worker, and SDPA attention; FlashAttention is not mandatory for this path. Include the speech tokenizer and its configuration. | Candidate for fixed preset voices with instruction control. Host memory and performance remain unmeasured. The 0.6B CustomVoice variant does not provide equivalent instruction control. |
| IndexTTS 2.5 | Private Python with optional CUDA kernels, DeepSpeed, acceleration extensions, and compilation disabled explicitly. Include all auxiliary models. | Candidate. Upstream reports roughly 6 GB VRAM; this is not a measurement on the target host. |
| VoxCPM2 | Private Python with compatible PyTorch/TorchCodec versions and required shared FFmpeg DLLs; optional compilation and denoising require separate decisions. | Defer the first delivery attempt. Upstream reports roughly 8 GB VRAM, leaving limited margin against the observed free VRAM. |

Primary sources inspected on 2026-10-24:

- CosyVoice: [Windows dependencies](https://github.com/QwenAudio/CosyVoice/blob/main/requirements.txt), [model loading](https://github.com/QwenAudio/CosyVoice/blob/main/cosyvoice/cli/cosyvoice.py), and [released model](https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512).
- Qwen3-TTS: [model card](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice) and [attention implementation](https://github.com/QwenLM/Qwen3-TTS/blob/main/qwen_tts/core/models/modeling_qwen3_tts.py).
- IndexTTS: [model card](https://huggingface.co/IndexTeam/IndexTTS-2.5) and [inference implementation](https://github.com/index-tts/index-tts/blob/main/indextts/infer_v2_5.py).
- VoxCPM2: [model card](https://huggingface.co/openbmb/VoxCPM2), [loading and optional optimization](https://github.com/OpenBMB/VoxCPM/blob/main/src/voxcpm/core.py), and [TorchCodec dependencies](https://github.com/meta-pytorch/torchcodec#installing-torchcodec).

These upstream links can change. A future build must pin model revisions, runtime versions, dependency versions, and launch options before claiming a reproducible package.

## Portable package direction

Retain each engine's Python inference implementation and distribute its interpreter, dependencies, native libraries, and model assets together. Python's [embeddable distribution](https://docs.python.org/3.11/using/windows.html#the-embeddable-package) supports this application-local approach; copying an ordinary virtual environment is not sufficient evidence of relocatability.

Each engine has an independent runtime because the audited PyTorch and Transformers requirements differ. The following layout describes one engine package, not directories already created in the repository:

```text
speech/
  launch.exe
  runtime/
  engine/
  models/
  data/
    cache/
    temp/
    logs/
    audio/
```

The launcher resolves resources from the package location and sets library paths and caches for its own process. It does not require a global PATH change. Build-time dependency installation is separate from user startup; launch must not require pip, a compiler, or a model download. All required auxiliary assets and configuration must be present. The installed GPU driver remains a host prerequisite.

Deployment details requiring explicit handling:

1. IndexTTS currently enables part of its low-VRAM behavior only when total VRAM is below 10 GB. This exactly 10 GB card does not satisfy that condition, even when substantially less memory is free. Limit segment size and check actual available VRAM independently.
2. CosyVoice's Windows dependency branch uses CPU ONNX Runtime while its frontend can select a CUDA provider based on PyTorch availability. Verify provider selection explicitly rather than assuming every stage runs on the GPU.
3. TorchCodec requires compatible shared FFmpeg libraries; distributing only an ffmpeg executable does not establish VoxCPM compatibility.
4. Missing auxiliary models or default external cache paths can conceal network and machine-state dependencies. Verify a complete offline launch with unrelated caches unavailable.

Packaging alone does not require model quantization or an inference rewrite. Sound quality and consistency still require listening tests using the delivered configuration.

## Text analysis and resource scheduling

The text model remains unselected. Its responsibility is to infer narration emotion, pace, and pause annotations from context while retaining one narrator voice and unchanged source text. Annotation format and engine-specific mapping remain design work; this audit does not establish a production schema.

Qwen3.5-9B in GGUF Q5_K_M format with llama.cpp is a proposed evaluation candidate, not a confirmed dependency. Its CPU throughput and annotation quality on the user's books have not been measured. An independent text model is an optional enhancement: compare direct TTS with the same TTS receiving annotations before adopting the extra stage. Incorrect or excessive annotations can reduce naturalness and continuity. See the [model card](https://huggingface.co/Qwen/Qwen3.5-9B), [GGUF artifacts](https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/tree/main), and [runtime](https://github.com/ggml-org/llama.cpp).

Prefer CPU text analysis while the GPU is occupied by speech generation. Any GPU acceleration of text analysis must be scheduled against speech memory use. Keep one speech model resident on the GPU at a time; determine segment size through measured host behavior rather than an assumed universal character limit.

## External listening reference

Fanqie officially describes AI narration supplied by the Volcano speech team, with emotion and role-dependent voices. An earlier team interview described chapter understanding followed by role/emotion annotations and speech synthesis. This supports a similar overall direction, not a claim about the product's current internal models or complete pipeline. The public Volcano voice list labels some voices as Fanqie equivalents; an API voice is not evidence that the entire product workflow is reproduced. Sources: [official product description](https://apps.apple.com/cn/app/id1468454200), [team interview](https://developer.volcengine.com/articles/7034485471128322084), and [voice list](https://docs.volcengine.com/docs/DoubaoVoice/Tonelist-1?lang=zh).

For the fixed-narrator goal, compare identical text across direct local TTS, annotation-assisted local TTS, and an available single-voice reference. Assess pronunciation, continuity, voice stability, omissions/repetition, emotional appropriateness, and processing cost. No comparative listening test has been performed, and no superiority over Fanqie is established.

## Required host verification

All checks below are pending. They define the audit's delivery evidence needs, not completed tests or full implementation admission under [development.md](development.md).

| Check | Required observation |
| --- | --- |
| Cold start | Load the packaged interpreter, libraries, main model, and auxiliary assets on the target computer without using a system Python environment. |
| Complete chapter | Generate a complete chapter in bounded segments; record peak VRAM, system memory, elapsed time, and generated audio duration. Inspect omissions, repetition, pronunciation, emotional delivery, and voice consistency. |
| Offline operation | Start and generate with networking unavailable and external caches unavailable; all necessary assets resolve within the package. |
| Relocation | Rename and move the directory, including paths containing spaces and Chinese characters, then repeat startup and generation. |
| Continued use | Repeat generation and exercise cancellation and retry; record failures and resource behavior instead of treating one successful clip as sustained operation. |
| Artifact identity | Retain exact engine/model/runtime identities and launch settings with the host observations. |

No generation speed, real-time playback capability, peak-memory ceiling, or winning model has been established. Portable feasibility is the audit conclusion; actual host usability, Saga integration, and release qualification remain open.
