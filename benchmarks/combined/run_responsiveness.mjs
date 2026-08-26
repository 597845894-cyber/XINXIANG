import { spawn } from "node:child_process";
import { unlink, writeFile, mkdir } from "node:fs/promises";
import { dirname } from "node:path";
import process from "node:process";
import { performance } from "node:perf_hooks";
import { clearInterval, setInterval } from "node:timers";

const output = "benchmarks/results/raw/responsiveness-probe.json";
try {
  await unlink(output);
} catch {
  // The first run has no previous probe result.
}

const command = [
  "benchmarks/semantic/run_candidate.mjs",
  "--candidate-id",
  "qwen2.5-1.5b-instruct-q4_k_m",
  "--llama-cli",
  "benchmarks/cache/llama-b6392/llama-cli.exe",
  "--model-dir",
  "benchmarks/cache/models",
  "--output",
  output,
  "--sample-id",
  "image-long-multiple-tasks",
  "--threads",
  "4",
];
const child = spawn(process.execPath, command, {
  stdio: ["ignore", "pipe", "pipe"],
  windowsHide: true,
});
let stdout = "";
let stderr = "";
child.stdout.setEncoding("utf8");
child.stderr.setEncoding("utf8");
child.stdout.on("data", (chunk) => (stdout += chunk));
child.stderr.on("data", (chunk) => (stderr += chunk));

const intervalMs = 16;
let expected = performance.now() + intervalMs;
let tickCount = 0;
let maxEventLoopDelayMs = 0;
const timer = setInterval(() => {
  const now = performance.now();
  maxEventLoopDelayMs = Math.max(maxEventLoopDelayMs, now - expected);
  expected = now + intervalMs;
  tickCount += 1;
}, intervalMs);
const startedAt = performance.now();
const exitCode = await new Promise((resolveExit, reject) => {
  child.on("error", reject);
  child.on("close", resolveExit);
});
clearInterval(timer);
const elapsedMs = performance.now() - startedAt;
if (exitCode !== 0) throw new Error(`responsiveness probe failed: ${stderr.slice(-1000)}`);

const result = {
  schemaVersion: 1,
  probe: "ui-event-loop-while-semantic-child-process-runs",
  sampleId: "image-long-multiple-tasks",
  elapsedMs: Number(elapsedMs.toFixed(3)),
  intervalMs,
  tickCount,
  maxEventLoopDelayMs: Number(maxEventLoopDelayMs.toFixed(3)),
  responsiveThresholdMs: 100,
  passed: tickCount > 0 && maxEventLoopDelayMs < 100,
  childOutput: stdout.trim(),
};
await mkdir(dirname("benchmarks/results/responsiveness-summary.json"), { recursive: true });
await writeFile(
  "benchmarks/results/responsiveness-summary.json",
  `${JSON.stringify(result, null, 2)}\n`,
);
if (!result.passed) throw new Error("event loop responsiveness threshold exceeded");
