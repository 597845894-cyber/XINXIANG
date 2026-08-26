export interface OcrPoint {
  x: number;
  y: number;
}

export interface OcrLine {
  text: string;
  confidence: number;
  box: [OcrPoint, OcrPoint, OcrPoint, OcrPoint];
}

export interface OcrResult {
  lines: OcrLine[];
  elapsedMs: number;
}

export interface OcrAdapter {
  readonly candidateId: string;
  recognize(imagePath: string, signal?: AbortSignal): Promise<OcrResult>;
}

export function assertOcrResult(value: unknown): asserts value is OcrResult {
  if (!value || typeof value !== "object") throw new Error("OCR result must be an object");
  const result = value as Partial<OcrResult>;
  if (!Number.isFinite(result.elapsedMs) || !Array.isArray(result.lines)) {
    throw new Error("OCR result is missing elapsedMs or lines");
  }
  for (const line of result.lines) {
    if (
      !line.text ||
      line.confidence < 0 ||
      line.confidence > 1 ||
      line.box.length !== 4 ||
      line.box.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))
    ) {
      throw new Error("OCR line is invalid");
    }
  }
}
