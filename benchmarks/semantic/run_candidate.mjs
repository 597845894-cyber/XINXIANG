import { execFile, spawn } from "node:child_process";
import { readFile, stat, writeFile, mkdir } from "node:fs/promises";
import { cpus } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import process from "node:process";
import { clearInterval, clearTimeout, setInterval, setTimeout } from "node:timers";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) values.set(argv[index], argv[index + 1]);
  for (const name of ["--candidate-id", "--llama-cli", "--model-dir", "--output"]) {
    if (!values.has(name)) throw new Error(`missing ${name}`);
  }
  return Object.fromEntries(values);
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function readSamples(repoRoot) {
  const manifest = await readJson(join(repoRoot, "benchmarks/dataset/manifest.json"));
  return Promise.all(
    manifest.samples.map((name) => readJson(join(repoRoot, "benchmarks/dataset/samples", name))),
  );
}

function sourceText(sample) {
  return sample.input.text ?? sample.input.referenceText;
}

function parseJsonOutput(stdout) {
  const start = stdout.indexOf("{");
  const end = stdout.lastIndexOf("}");
  if (start < 0 || end < start) throw new Error("model output does not contain a JSON object");
  return JSON.parse(stdout.slice(start, end + 1));
}

async function workingSetBytes(pid) {
  try {
    const { stdout } = await execFileAsync("powershell.exe", [
      "-NoProfile",
      "-Command",
      `(Get-Process -Id ${pid} -ErrorAction Stop).WorkingSet64`,
    ]);
    return Number(stdout.trim()) || 0;
  } catch {
    return 0;
  }
}

async function runInference(command, timeoutMs) {
  const child = spawn(command[0], command.slice(1), {
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let stdout = "";
  let stderr = "";
  let peakMemoryBytes = 0;
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => (stdout += chunk));
  child.stderr.on("data", (chunk) => (stderr += chunk));
  const sampler = setInterval(async () => {
    peakMemoryBytes = Math.max(peakMemoryBytes, await workingSetBytes(child.pid));
  }, 50);
  const timeout = setTimeout(() => child.kill(), timeoutMs);
  const exitCode = await new Promise((resolveExit, reject) => {
    child.on("error", reject);
    child.on("close", resolveExit);
  });
  clearInterval(sampler);
  clearTimeout(timeout);
  peakMemoryBytes = Math.max(peakMemoryBytes, await workingSetBytes(child.pid));
  return { stdout, stderr, exitCode, peakMemoryBytes };
}

async function main() {
  const args = parseArguments(process.argv.slice(2));
  const repoRoot = resolve(import.meta.dirname, "../..");
  const config = await readJson(join(repoRoot, "benchmarks/semantic/candidates.json"));
  const candidate = config.candidates.find((item) => item.id === args["--candidate-id"]);
  if (!candidate) throw new Error("unknown candidate");
  const modelPath = join(args["--model-dir"], candidate.file);
  const outputPath = resolve(args["--output"]);
  const modelStats = await stat(modelPath);
  let report;
  try {
    report = await readJson(outputPath);
  } catch {
    report = {
      schemaVersion: 1,
      candidate,
      runtime: config.runtime,
      environment: { platform: process.platform, cpuCount: cpus().length },
      modelSizeMb: Number((modelStats.size / 1024 / 1024).toFixed(3)),
      peakMemoryMb: 0,
      samples: [],
    };
  }
  const completed = new Set(report.samples.map((sample) => sample.sampleId));
  await mkdir(dirname(outputPath), { recursive: true });

  for (const sample of await readSamples(repoRoot)) {
    if (args["--sample-id"] && args["--sample-id"] !== sample.id) continue;
    if (completed.has(sample.id)) continue;
    const prompt = [
      "请分析以下通知。通知发布时间和时区仅作为上下文，不要自行计算绝对日期。",
      `发布时间：${sample.input.noticePublishedAt}`,
      `时区：${sample.input.timezone}`,
      "通知原文：",
      sourceText(sample),
    ].join("\n");
    const promptPath = join("benchmarks/cache/prompts", candidate.id, `${sample.id}.txt`);
    await mkdir(dirname(promptPath), { recursive: true });
    await writeFile(promptPath, prompt, "utf8");
    const command = [
      resolve(args["--llama-cli"]),
      "--model",
      modelPath,
      "--system-prompt-file",
      "benchmarks/semantic/system-prompt.txt",
      "--json-schema-file",
      "benchmarks/semantic/output-schema.json",
      "--file",
      promptPath,
      "--ctx-size",
      "4096",
      "--threads",
      args["--threads"] ?? String(Math.min(cpus().length, 8)),
      "--predict",
      "768",
      "--temp",
      "0",
      "--seed",
      "42",
      "--no-display-prompt",
      "--simple-io",
    ];
    const startedAt = performance.now();
    const run = await runInference(command, 120_000);
    const elapsedMs = Number((performance.now() - startedAt).toFixed(3));
    let output = null;
    let status = run.exitCode === 0 ? "ok" : "runtime-error";
    let error = null;
    try {
      output = parseJsonOutput(run.stdout);
    } catch (caught) {
      status = "invalid-json";
      error = caught instanceof Error ? caught.message : String(caught);
    }
    report.peakMemoryMb = Math.max(
      report.peakMemoryMb,
      Number((run.peakMemoryBytes / 1024 / 1024).toFixed(3)),
    );
    report.samples.push({
      sampleId: sample.id,
      status,
      elapsedMs,
      output,
      error,
      runtimeDiagnostics: run.stderr.slice(-2000),
    });
    await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    process.stdout.write(`${basename(modelPath)}: ${report.samples.length}/10 ${sample.id}\n`);
  }
}

await main();
