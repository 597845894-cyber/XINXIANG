# 本地模型资源安装

首版模型资源由 `src-tauri/resources/models/selected-models.lock.json` 锁定。清单包含资源版本、许可证、相对路径、文件大小与 SHA-256；模型二进制不提交到 Git，也不得与通知样本、数据库或日志混放。

当前开发安装固定使用 E 盘的 `E:\University\校园信箱\local-models\xinxiang-model-selection-2026-08-26`，避免占用 C 盘。该目录已由 `.gitignore` 排除；应用只会读取清单列出的本地文件，通知内容不会参与资源下载或校验。

## 已核验的下载来源

- OCR：PyPI 的 `rapidocr-onnxruntime` 1.2.3 资源包，包内的三个 PP-OCRv3 ONNX 文件。
- 语义模型：[Qwen/Qwen2.5-1.5B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF) 的 `qwen2.5-1.5b-instruct-q4_k_m.gguf`。
- 本地运行时：[llama.cpp b6392 Windows CPU x64 发布包](https://github.com/ggml-org/llama.cpp/releases/tag/b6392)。

下载完成后必须按锁定清单检查文件长度和 SHA-256。2026-08-27 的本地核验中，五个文件均与清单匹配；模型和运行时没有被提交到仓库。

## 离线资源包结构

用户选择的本地文件夹必须包含以下结构：

```text
资源包/
├── ocr/
│   ├── ch_PP-OCRv3_det_infer.onnx
│   ├── ch_PP-OCRv3_rec_infer.onnx
│   └── ch_ppocr_mobile_v2.0_cls_infer.onnx
├── semantic/
│   └── qwen2.5-1.5b-instruct-q4_k_m.gguf
└── runtime/
    └── llama-b6392-bin-win-cpu-x64.zip
```

## 状态与恢复

- `available`：所有文件存在，大小和 SHA-256 与清单一致，可以离线分析。
- `missing`：至少一个文件不存在。应用提示用户选择完整的本地资源文件夹重新安装。
- `corrupt`：文件大小或 SHA-256 不一致。应用拒绝加载并提示重新安装，避免运行被截断或替换的权重。

安装过程只从用户选择的本地目录复制清单列出的文件。应用先复制到同一数据目录下的临时位置，完成全量校验后再原子替换当前资源；来源无效、复制中断或校验失败时保留现有安装。该边界没有下载、登录、许可检查或通用网络客户端，状态合同中的 `networkRequired` 固定为 `false`。

## 开发验证

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/rust-check.ps1 -Action Test
```

测试使用不含真实模型或个人信息的小型合成文件，覆盖缺失、篡改、可用、离线安装及失败回滚。正式资源包仍应在发布前按锁定清单逐文件核验。
