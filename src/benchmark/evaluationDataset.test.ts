import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import manifest from "../../benchmarks/dataset/manifest.json";
import { assertEvaluationSample, type EvaluationSampleV1 } from "./evaluationDataset";

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
    expect(samples).toHaveLength(10);
    expect(new Set(samples.map((sample) => sample.id))).toHaveLength(samples.length);
    for (const sample of samples) {
      expect(sample.expected.taskCount).toBe(sample.expected.tasks.length);
      for (const task of sample.expected.tasks) {
        const sourceText =
          sample.input.kind === "text" ? sample.input.text : sample.input.referenceText;
        for (const evidence of task.evidence) expect(sourceText).toContain(evidence);
      }
    }
  });

  it("references committed, readable PNG fixtures for image samples", async () => {
    const { imageSize } = await import("image-size");
    const imageSamples = samples.filter(
      (
        sample,
      ): sample is EvaluationSampleV1 & {
        input: Extract<EvaluationSampleV1["input"], { kind: "image" }>;
      } => sample.input.kind === "image",
    );
    expect(imageSamples).toHaveLength(2);
    for (const sample of imageSamples) {
      const path = resolve("benchmarks/dataset/samples", sample.input.asset);
      const dimensions = imageSize(readFileSync(path));
      expect(dimensions.width).toBeGreaterThanOrEqual(900);
      expect(dimensions.height).toBeGreaterThanOrEqual(400);
    }
  });
});
