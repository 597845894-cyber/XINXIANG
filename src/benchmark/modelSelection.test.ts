import combined from "../../benchmarks/results/combined-summary.json";
import lock from "../../src-tauri/resources/models/selected-models.lock.json";

describe("selected local model combination", () => {
  it("locks the semantic model, runtime, licenses, and SHA-256 values", () => {
    expect(lock.schemaVersion).toBe(1);
    expect(lock.components.map((component) => component.kind)).toEqual([
      "semantic",
      "runtime",
    ]);
    for (const component of lock.components) {
      expect(component.license).toMatch(/Apache-2.0|MIT/);
      expect(component.licenseSource).toMatch(/^https:\/\//);
      for (const file of component.files) {
        expect(file.size).toBeGreaterThan(0);
        expect(file.sha256).toMatch(/^[a-f0-9]{64}$/);
      }
    }
  });

  it("completes the full dataset within the memory budget while the UI stays responsive", () => {
    expect(combined.environment.executionBackend).toBe("CPU-only");
    expect(combined.coverage.fullDatasetSamples).toBeGreaterThan(0);
    expect(combined.coverage.structuredSemanticOutputs).toBeGreaterThan(0);
    expect(combined.performance.withinEightGbBudget).toBe(true);
    expect(combined.performance.semanticPeakMemoryMb).toBeLessThan(8192);
    expect(combined.performance.uiResponsivenessPassed).toBe(true);
    expect(combined.performance.maxEventLoopDelayMs).toBeLessThan(100);
  });
});
