[English](speech.md) | [简体中文](speech_CN.md)

# 本地朗读与可移动部署

SPEECH-01。记录日期：2026-10-24。状态：审核已记录；尚未实现；目标主机验证待完成。

用户授权记录可移动部署审核。本文档记录审核发现及已确定的朗读约束，不授权模型安装、推理、集成或发布。当前应用行为仍以[架构文档](architecture_CN.md)为准，交付工作仍在[任务台账](tasks_CN.md)中保持开放。

## 范围与已确定约束

- 优先面向用户当前的 Windows 计算机。macOS 交付暂缓。
- 整本书使用同一个固定旁白音色。情绪、语速和停顿可以变化，不为不同角色分配不同音色。
- 保留原文。文本分析提供独立的朗读标注，不改写书籍，也不以模型生成的文字替换原句。
- 将本地模型工具交付为可移动目录，不依赖系统 Python 安装或已激活的开发环境。
- 交付物必须在实际目标计算机上运行。静态兼容性发现及上游内存数据不能证明这一结果。

## 审核时观察到的目标计算机

| 资源 | 2026-10-24 的观察结果 |
| --- | --- |
| 操作系统 | Windows 11 IoT Enterprise LTSC，x64，内部版本 26100 |
| 处理器 | Intel Core i7-13700F，16 核 / 24 逻辑处理器 |
| 系统内存 | 总计约 63.8 GiB；检查时可用约 41.2 GiB |
| GPU | NVIDIA GeForce RTX 3080，总显存 10,240 MiB；计算能力 8.6 |
| 空闲显存 | 检查时为 8,522 MiB，约 8.3 GiB |
| NVIDIA 驱动 | 610.88 |
| 空闲存储 | D: 约 87 GiB，C: 约 171 GiB |

空闲内存与存储仅为快照，不是预留容量。本次审核未加载模型。权重文件大小既不是显存峰值，也不是完整包大小。

## 语音模型发现

下列顺序是评估优先级，不是实测音质排名或最终引擎选择。发布的模型变体至关重要；更大的研究变体不能与这些产物互换。

| 候选 | 已审核的部署路径 | 目标主机处置意见 |
| --- | --- | --- |
| CosyVoice 3，Fun-CosyVoice3-0.5B-2512 | 私有 Python 与基础 PyTorch 推理；禁用可选的 TensorRT 和 vLLM。官方依赖文件包含 Windows 分支。 | 优先候选。该主机上的显存峰值和持续生成尚未测量。 |
| Qwen3-TTS-12Hz-1.7B-CustomVoice | 私有 Python、BF16、单工作进程及 SDPA 注意力；该路径不强制要求 FlashAttention。包含语音 tokenizer 及其配置。 | 适用于固定预置音色与指令控制的候选。主机内存与性能尚未测量。0.6B CustomVoice 变体不提供等效指令控制。 |
| IndexTTS 2.5 | 私有 Python，显式禁用可选 CUDA 内核、DeepSpeed、加速扩展和编译。包含全部辅助模型。 | 候选。上游报告显存约 6 GB；这不是目标主机上的测量结果。 |
| VoxCPM2 | 私有 Python，使用兼容的 PyTorch/TorchCodec 版本及所需的共享 FFmpeg DLL；可选编译与降噪需另行决定。 | 暂缓首轮交付尝试。上游报告显存约 8 GB，相对观察到的空闲显存余量有限。 |

2026-10-24 检查的一手来源：

- CosyVoice：[Windows 依赖](https://github.com/QwenAudio/CosyVoice/blob/main/requirements.txt)、[模型加载](https://github.com/QwenAudio/CosyVoice/blob/main/cosyvoice/cli/cosyvoice.py)及[发布模型](https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512)。
- Qwen3-TTS：[模型说明](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice)及[注意力实现](https://github.com/QwenLM/Qwen3-TTS/blob/main/qwen_tts/core/models/modeling_qwen3_tts.py)。
- IndexTTS：[模型说明](https://huggingface.co/IndexTeam/IndexTTS-2.5)及[推理实现](https://github.com/index-tts/index-tts/blob/main/indextts/infer_v2_5.py)。
- VoxCPM2：[模型说明](https://huggingface.co/openbmb/VoxCPM2)、[加载与可选优化](https://github.com/OpenBMB/VoxCPM/blob/main/src/voxcpm/core.py)及[TorchCodec 依赖](https://github.com/meta-pytorch/torchcodec#installing-torchcodec)。

这些上游链接可能变化。后续构建必须固定模型修订、运行时版本、依赖版本及启动选项，才能声称包可复现。

## 可移动包方向

保留各引擎的 Python 推理实现，将其解释器、依赖、原生库及模型资产一起分发。Python 的[可嵌入发行版](https://docs.python.org/3.11/using/windows.html#the-embeddable-package)支持这种应用本地部署方式；复制普通虚拟环境不足以证明可移动性。

由于已审核的 PyTorch 与 Transformers 要求不同，每个引擎使用独立运行时。以下布局描述单个引擎包，并不表示仓库中已经创建这些目录：

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

启动器从包所在位置解析资源，为自身进程设置库路径和缓存，不要求修改全局 PATH。构建时安装依赖与用户启动相互分离；启动不得要求 pip、编译器或模型下载。全部必需的辅助资产和配置必须齐备。已安装的 GPU 驱动仍是宿主前提条件。

需要明确处理的部署细节：

1. IndexTTS 当前仅在总显存低于 10 GB 时启用部分低显存行为。这张恰好 10 GB 的卡不满足该条件，即使空闲显存明显更少。应限制分段大小，并独立检查实际可用显存。
2. CosyVoice 的 Windows 依赖分支使用 CPU ONNX Runtime，但其前端可根据 PyTorch 可用性选择 CUDA 提供器。须显式验证提供器选择，不能假设每个阶段都在 GPU 上运行。
3. TorchCodec 需要兼容的共享 FFmpeg 库；仅分发 ffmpeg 可执行文件不能证明 VoxCPM 兼容。
4. 缺少辅助模型或使用默认外部缓存路径可能掩盖网络和机器状态依赖。应在外部缓存不可用的情况下验证完整离线启动。

仅打包本身不要求模型量化或重写推理。音质与一致性仍需使用交付配置进行听测。

## 文本分析与资源调度

文本模型尚未选定。其职责是根据上下文推断朗读情绪、语速和停顿标注，同时保留单一旁白音色及原文。标注格式和引擎特定映射仍属设计工作；本次审核不建立生产模式。

使用 llama.cpp 运行 GGUF Q5_K_M 格式的 Qwen3.5-9B 是建议评估的候选，并非已确认依赖。其在用户书籍上的 CPU 吞吐量和标注质量尚未测量。独立文本模型是可选增强：采用额外阶段前，应比较直接 TTS 与接收标注的同一 TTS。错误或过度标注可能降低自然度和连贯性。参见[模型说明](https://huggingface.co/Qwen/Qwen3.5-9B)、[GGUF 产物](https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/tree/main)及[运行时](https://github.com/ggml-org/llama.cpp)。

GPU 用于语音生成时，优先在 CPU 上执行文本分析。任何文本分析的 GPU 加速都必须结合语音显存使用情况进行调度。GPU 同时只常驻一个语音模型；分段大小应由主机实测行为确定，而非假设通用字符数上限。

## 外部听测参考

番茄官方介绍了由火山语音团队提供、具有情感和角色区分音色的 AI 朗读。早期团队访谈描述了先理解章节，再进行角色/情绪标注及语音合成的流程。这支持相近的总体方向，不构成对产品当前内部模型或完整流程的断言。火山公开音色列表将部分音色标注为番茄同款；API 音色不能证明复现了整个产品流程。来源：[官方产品介绍](https://apps.apple.com/cn/app/id1468454200)、[团队访谈](https://developer.volcengine.com/articles/7034485471128322084)及[音色列表](https://docs.volcengine.com/docs/DoubaoVoice/Tonelist-1?lang=zh)。

针对固定旁白目标，应使用相同文本比较直接本地 TTS、标注辅助本地 TTS，以及可用的单音色参考。评估发音、连贯性、音色稳定性、漏读/重复、情绪适切性和处理成本。尚未开展比较听测，未确立优于番茄的结论。

## 必需的主机验证

下列检查全部待完成。它们定义审核所需的交付证据，不是已完成测试，也不是[开发契约](development_CN.md)规定的完整实现准入。

| 检查 | 必需观察结果 |
| --- | --- |
| 冷启动 | 在目标计算机上加载包内解释器、库、主模型和辅助资产，不使用系统 Python 环境。 |
| 完整章节 | 以有界分段生成完整章节；记录显存峰值、系统内存、耗时及生成音频时长。检查漏读、重复、发音、情绪表达及音色一致性。 |
| 离线运行 | 在网络及外部缓存不可用的情况下启动并生成；全部必需资产从包内解析。 |
| 移动目录 | 重命名并移动目录，包括含空格和中文字符的路径，然后重复启动和生成。 |
| 持续使用 | 重复生成并执行取消和重试；记录失败及资源行为，不将单个片段成功视为持续运行。 |
| 产物身份 | 随主机观察结果保留准确的引擎/模型/运行时身份及启动设置。 |

尚未确立生成速度、实时播放能力、内存峰值上限或胜出模型。可移动部署具有可行性是本次审核的结论；实际主机可用性、Saga 集成及发布合格性认定仍未完成。
