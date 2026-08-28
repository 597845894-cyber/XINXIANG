import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import manifest from "../../benchmarks/dataset/manifest.json";
import { assertEvaluationSample } from "./evaluationDataset";

function readSample(file: string) {
  const path = resolve("benchmarks/dataset/samples", file);
  const value: unknown = JSON.parse(readFileSync(path, "utf8"));
  assertEvaluationSample(value);
  return value;
}

describe("synthetic campus notice evaluation dataset", () => {
  const samples = manifest.samples.map(readSample);

  it("is explicitly synthetic and covers every required scenario", () => {
    expect(manifest.schemaVersion).toBe(1);
    expect(manifest.privacy).toBe("synthetic-only");
    expect(new Set(samples.flatMap((sample) => sample.coverage))).toEqual(
      new Set(manifest.requiredCoverage),
    );
  });

  it("contains unique, structurally valid samples with grounded task evidence", () => {
    expect(samples).toHaveLength(8);
    expect(new Set(samples.map((sample) => sample.id))).toHaveLength(samples.length);
    for (const sample of samples) {
      expect(sample.expected.taskCount).toBe(sample.expected.tasks.length);
      for (const task of sample.expected.tasks) {
        const sourceText = sample.input.text;
        for (const evidence of task.evidence) expect(sourceText).toContain(evidence);
      }
    }
  });
});
