import { assertOcrResult } from "./ocrAdapter";

describe("OCR adapter contract", () => {
  it("accepts text, confidence, coordinates, and timing", () => {
    const value: unknown = {
      elapsedMs: 120,
      lines: [
        {
          text: "测试通知",
          confidence: 0.98,
          box: [
            { x: 1, y: 2 },
            { x: 10, y: 2 },
            { x: 10, y: 8 },
            { x: 1, y: 8 },
          ],
        },
      ],
    };
    expect(() => assertOcrResult(value)).not.toThrow();
  });

  it("rejects out-of-range confidence", () => {
    expect(() =>
      assertOcrResult({
        elapsedMs: 5,
        lines: [{ text: "内容", confidence: 1.2, box: [{}, {}, {}, {}] }],
      }),
    ).toThrow("OCR line is invalid");
  });
});
