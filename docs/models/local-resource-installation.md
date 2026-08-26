# 本地模型资源安装

首版模型资源由 `src-tauri/resources/models/selected-models.lock.json` 锁定。清单包含资源版本、许可证、相对路径、文件大小与 SHA-256；模型二进制不提交到 Git，也不得与通知样本、数据库或日志混放。

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
