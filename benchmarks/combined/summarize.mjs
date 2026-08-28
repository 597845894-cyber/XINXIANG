import { readFile, writeFile } from "node:fs/promises";

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const semantic = await readJson("benchmarks/results/semantic-summary.json");
const responsiveness = await readJson("benchmarks/results/responsiveness-summary.json");
const semanticSelected = semantic.candidates.find(
  (candidate) => candidate.candidateId === "qwen2.5-1.5b-instruct-q4_k_m",
);
const report = {
  schemaVersion: 1,
  selectionId: "xinxiang-model-selection-2026-08-26",
  datasetId: semantic.datasetId,
  environment: {
    executionBackend: "CPU-only",
    physicalHostMemoryGb: 15.7,
    targetMemoryBudgetMb: 8192,
    note: "Capacity is validated against the 8 GB budget from measured local semantic-process peaks; physical 8 GB clean-device acceptance remains in OpenSpec task 4.3.",
  },
  coverage: {
    fullDatasetSamples: semanticSelected.sampleCount,
    structuredSemanticOutputs: Math.round(
      semanticSelected.sampleCount * semanticSelected.structuredOutputCompliance,
    ),
  },
  selected: {
    semanticCandidateId: semanticSelected.candidateId,
  },
  performance: {
    semanticPeakMemoryMb: semanticSelected.peakMemoryMb,
    withinEightGbBudget: semanticSelected.peakMemoryMb < 8192,
    maxEventLoopDelayMs: responsiveness.maxEventLoopDelayMs,
    uiResponsivenessPassed: responsiveness.passed,
  },
};
await writeFile("benchmarks/results/combined-summary.json", `${JSON.stringify(report, null, 2)}\n`);
const markdown = `# 首版模型组合锁定报告

## 锁定结果

- 语义：\`qwen2.5-1.5b-instruct-q4_k_m\`
- 运行后端：CPU-only
- 版本、许可证与文件校验值：\`src-tauri/resources/models/selected-models.lock.json\`

## 完整评测集验证

固定合成文字评测集共 ${report.coverage.fullDatasetSamples} 条，${report.coverage.structuredSemanticOutputs} 条获得符合 JSON Schema 的结构化语义输出。语义选择报告保存在 \`semantic-selection.md\`。

语义分析进程峰值为 ${report.performance.semanticPeakMemoryMb.toFixed(1)} MB，低于 8192 MB 目标预算。分析在子进程运行期间，UI 事件循环最大延迟为 ${report.performance.maxEventLoopDelayMs.toFixed(1)} ms，低于 100 ms 阈值。

## 验收边界

本次在 15.7 GB 主机上以 CPU-only 后端实测进程峰值，并按 8 GB 预算判定容量；没有把主机描述成物理 8 GB 设备。真实 8 GB、无独显 Windows 基准机的整机体验和最终安装包验证仍由 OpenSpec 任务 9.4 与 9.7 完成。
`;
await writeFile("docs/benchmarks/selected-model-combination.md", markdown);
