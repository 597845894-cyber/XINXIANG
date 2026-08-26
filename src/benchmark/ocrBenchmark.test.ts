import candidates from "../../benchmarks/ocr/candidates.json";
import report from "../../benchmarks/results/ocr-summary.json";

describe("OCR benchmark report", () => {
  it("reports every declared candidate and required metric", () => {
    expect(report.schemaVersion).toBe(1);
    expect(report.candidates.map((candidate) => candidate.candidateId).sort()).toEqual(
      candidates.candidates.map((candidate) => candidate.id).sort(),
    );
    for (const candidate of report.candidates) {
      expect(candidate.sampleCount).toBe(2);
      expect(candidate.meanCharacterAccuracy).toBeGreaterThanOrEqual(0);
      expect(candidate.meanCharacterAccuracy).toBeLessThanOrEqual(1);
      expect(candidate.coordinateValidity).toBeGreaterThanOrEqual(0);
      expect(candidate.coordinateValidity).toBeLessThanOrEqual(1);
      expect(candidate.meanElapsedMs).toBeGreaterThan(0);
      expect(candidate.peakMemoryMb).toBeGreaterThan(0);
      expect(candidate.modelSizeMb).toBeGreaterThan(0);
    }
  });

  it("recommends one of the measured candidates", () => {
    expect(
      report.candidates.some(
        (candidate) => candidate.candidateId === report.recommendedCandidateId,
      ),
    ).toBe(true);
  });
});
