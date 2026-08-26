import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

function parseArguments(argv) {
  const jsonIndex = argv.indexOf("--json-output");
  const markdownIndex = argv.indexOf("--markdown-output");
  return {
    inputs: argv.slice(0, Math.min(jsonIndex, markdownIndex)),
    jsonOutput: argv[jsonIndex + 1],
    markdownOutput: argv[markdownIndex + 1],
  };
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

const mean = (values) => values.reduce((sum, value) => sum + value, 0) / values.length;
const round = (value, digits = 6) => Number(value.toFixed(digits));
const normalize = (value) =>
  value == null ? "" : String(value).toLowerCase().replaceAll(/\s/g, "");

function expectedTimeExpression(task) {
  if (task.deadlineExpression) return task.deadlineExpression;
  const evidence = (task.evidence ?? []).join(" ");
  for (const marker of ["前", "举行", "参加", "培训"]) {
    const index = evidence.indexOf(marker);
    if (index >= 0) return evidence.slice(0, index + (marker === "前" ? 1 : 0));
  }
  return "";
}

function fieldValues(task, expected) {
  return [
    normalize(task.title),
    normalize(expected ? expectedTimeExpression(task) : task.timeExpression),
    normalize(task.locationOrEntry),
    normalize((task.materials ?? []).join("|")),
    normalize(task.audience),
    normalize(task.required),
  ];
}

function fieldF1(expectedTasks, actualTasks) {
  const expected = expectedTasks.flatMap((task) => fieldValues(task, true)).filter(Boolean);
  const actual = actualTasks.flatMap((task) => fieldValues(task, false)).filter(Boolean);
  if (expected.length === 0 && actual.length === 0) return 1;
  const matched = new Set();
  let matches = 0;
  for (const expectedValue of expected) {
    const index = actual.findIndex(
      (actualValue, actualIndex) =>
        !matched.has(actualIndex) &&
        (expectedValue === actualValue ||
          expectedValue.includes(actualValue) ||
          actualValue.includes(expectedValue)),
    );
    if (index >= 0) {
      matched.add(index);
      matches += 1;
    }
  }
  const precision = actual.length ? matches / actual.length : 0;
  const recall = expected.length ? matches / expected.length : 0;
  return precision + recall ? (2 * precision * recall) / (precision + recall) : 0;
}

async function main() {
  const args = parseArguments(process.argv.slice(2));
  const repoRoot = resolve(import.meta.dirname, "../..");
  const manifest = await readJson(join(repoRoot, "benchmarks/dataset/manifest.json"));
  const fixtures = await Promise.all(
    manifest.samples.map((name) => readJson(join(repoRoot, "benchmarks/dataset/samples", name))),
  );
  const expected = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  const summaries = [];
  for (const path of args.inputs) {
    const result = await readJson(path);
    const valid = result.samples.filter((sample) => sample.status === "ok");
    const metrics = valid.map((sample) => {
      const fixture = expected.get(sample.sampleId);
      const source = fixture.input.text ?? fixture.input.referenceText;
      const evidence = sample.output.tasks.flatMap((task) => task.evidence);
      return {
        category: sample.output.category === fixture.expected.category ? 1 : 0,
        taskCount: sample.output.tasks.length === fixture.expected.taskCount ? 1 : 0,
        field: fieldF1(fixture.expected.tasks, sample.output.tasks),
        grounded: evidence.every((item) => source.includes(item)) ? 1 : 0,
      };
    });
    summaries.push({
      candidateId: result.candidate.id,
      parameters: result.candidate.parameters,
      structuredOutputCompliance: round(valid.length / result.samples.length),
      classificationAccuracy: round(mean(metrics.map((metric) => metric.category))),
      taskCountAccuracy: round(mean(metrics.map((metric) => metric.taskCount))),
      fieldF1: round(mean(metrics.map((metric) => metric.field))),
      evidenceGroundingRate: round(mean(metrics.map((metric) => metric.grounded))),
      meanElapsedMs: round(mean(result.samples.map((sample) => sample.elapsedMs)), 3),
      maxElapsedMs: round(Math.max(...result.samples.map((sample) => sample.elapsedMs)), 3),
      peakMemoryMb: result.peakMemoryMb,
      modelSizeMb: result.modelSizeMb,
      sampleCount: result.samples.length,
    });
  }
  const ranked = [...summaries].sort(
    (left, right) =>
      right.structuredOutputCompliance - left.structuredOutputCompliance ||
      right.classificationAccuracy - left.classificationAccuracy ||
      right.taskCountAccuracy - left.taskCountAccuracy ||
      right.fieldF1 - left.fieldF1 ||
      right.evidenceGroundingRate - left.evidenceGroundingRate ||
      left.meanElapsedMs - right.meanElapsedMs,
  );
  const report = {
    schemaVersion: 1,
    datasetId: manifest.datasetId,
    rankingRule:
      "JSON compliance, classification, task count, field F1, evidence grounding desc; latency asc",
    candidates: summaries,
    recommendedCandidateId: ranked[0].candidateId,
  };
  await mkdir(dirname(args.jsonOutput), { recursive: true });
  await writeFile(args.jsonOutput, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  const rows = [
    "# 本地语义模型基准",
    "",
    "本报告使用固定合成通知、固定提示词、JSON Schema 约束和 CPU-only llama.cpp 运行时生成。逐样本输出位于 Git 忽略目录 `benchmarks/results/raw/`。",
    "",
    "| 候选 | JSON 合规率 | 分类准确率 | 任务数准确率 | 字段 F1 | 依据落地率 | 平均耗时 | 峰值内存 | 体积 |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ...summaries.map(
      (item) =>
        `| ${item.candidateId} | ${(item.structuredOutputCompliance * 100).toFixed(0)}% | ${(item.classificationAccuracy * 100).toFixed(0)}% | ${(item.taskCountAccuracy * 100).toFixed(0)}% | ${(item.fieldF1 * 100).toFixed(1)}% | ${(item.evidenceGroundingRate * 100).toFixed(0)}% | ${(item.meanElapsedMs / 1000).toFixed(1)} s | ${item.peakMemoryMb.toFixed(0)} MB | ${item.modelSizeMb.toFixed(0)} MB |`,
    ),
    "",
    "## 结论",
    "",
    `按预先声明的排序规则，推荐 \`${ranked[0].candidateId}\`。字段 F1 是原型比较指标；生产适配器还须在第五阶段加入规则提取、确定性日期解析、证据校验与失败恢复。`,
    "",
    "## 复现",
    "",
    "1. 在 Git 忽略的 `benchmarks/cache/` 准备 `candidates.json` 锁定的 GGUF 文件和 llama.cpp b6392。",
    "2. 分别运行 `run_candidate.mjs`，传入候选、运行时与模型目录；脚本按样本写检查点并支持续跑。",
    "3. 使用 `summarize.mjs` 从原始结果重新生成 JSON 与本报告。",
    "",
  ];
  await mkdir(dirname(args.markdownOutput), { recursive: true });
  await writeFile(args.markdownOutput, rows.join("\n"), "utf8");
}

await main();
