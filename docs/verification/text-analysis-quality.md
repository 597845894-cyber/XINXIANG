# 文字通知分析质量报告

本报告对应 `focus-text-notice-task-analysis` 的文字版验收范围。评测数据全部为项目内合成通知，不包含真实姓名、群聊、账号或个人信息。

## 覆盖范围

`benchmarks/dataset/manifest.json` 锁定了以下场景：单任务、多个独立任务、多阶段通知、缺失时间、相对日期、字段冲突、改期、取消和仅供知晓。每条样本都要求候选任务带有原文依据。

## 可复现入口

```text
pnpm test
pnpm test:rust
```

本地语义模型基准使用 `benchmarks/semantic/run_candidate.mjs` 生成逐样本结果，再由 `benchmarks/semantic/summarize.mjs` 生成 `benchmarks/results/semantic-summary.json` 和 `docs/benchmarks/semantic-selection.md`。原始输出和模型缓存均位于 Git 忽略目录，不进入版本库。

## 当前结论

- 结构化输出必须通过版本化合同、字段类型、日期解析和证据回指校验。
- 任一候选缺少高风险字段或证据不在原文中时，候选会进入待核对状态，不会自动成为正式任务。
- 本地模型不可用、超时、取消或输出无效时，系统保留原文并提供低可信规则回退、重试和手工建任务入口。
- 真实 CPU-only 耗时、峰值内存、离线桌面安装、托盘、升级/回滚和卸载仍需在用户 Windows 主机完成 4.2–4.4 验收。
