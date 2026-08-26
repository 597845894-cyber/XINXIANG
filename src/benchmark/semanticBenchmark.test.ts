import candidates from "../../benchmarks/semantic/candidates.json";
import report from "../../benchmarks/results/semantic-summary.json";
import { isSemanticResult } from "./semanticAdapter";

describe("semantic model benchmark report", () => {
  it("measures every declared 1.5B to 3B candidate", () => {
    expect(report.schemaVersion).toBe(1);
    expect(report.candidates.map((candidate) => candidate.candidateId).sort()).toEqual(
      candidates.candidates.map((candidate) => candidate.id).sort(),
    );
    expect(new Set(report.candidates.map((candidate) => candidate.parameters))).toEqual(
      new Set(["1.5B", "3B"]),
    );
  });

  it("contains quality, compliance, timing, memory, and package metrics", () => {
    for (const candidate of report.candidates) {
      expect(candidate.sampleCount).toBe(10);
      expect(candidate.structuredOutputCompliance).toBeGreaterThanOrEqual(0);
      expect(candidate.classificationAccuracy).toBeGreaterThanOrEqual(0);
      expect(candidate.taskCountAccuracy).toBeGreaterThanOrEqual(0);
      expect(candidate.fieldF1).toBeGreaterThanOrEqual(0);
      expect(candidate.evidenceGroundingRate).toBeGreaterThanOrEqual(0);
      expect(candidate.meanElapsedMs).toBeGreaterThan(0);
      expect(candidate.peakMemoryMb).toBeGreaterThan(0);
      expect(candidate.modelSizeMb).toBeGreaterThan(1000);
    }
  });

  it("recommends a measured candidate", () => {
    expect(
      report.candidates.some(({ candidateId }) => candidateId === report.recommendedCandidateId),
    ).toBe(true);
  });

  it("keeps the TypeScript adapter aligned with the constrained schema", () => {
    const validFixture = {
      category: "information-only",
      changeIntent: "none",
      tasks: [],
      uncertainties: [],
    };
    expect(isSemanticResult(validFixture)).toBe(true);
  });
});
