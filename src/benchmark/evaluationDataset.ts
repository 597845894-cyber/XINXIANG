export const EVALUATION_DATASET_SCHEMA_VERSION = 1 as const;

export type NoticeInput =
  | { kind: "text"; text: string; noticePublishedAt: string; timezone: string }
  | {
      kind: "image";
      asset: string;
      referenceText: string;
      noticePublishedAt: string;
      timezone: string;
    };

export interface ExpectedTask {
  title: string;
  deadline?: string | null;
  deadlineExpression?: string;
  start?: string;
  locationOrEntry?: string;
  materials?: string[];
  audience?: string;
  required?: boolean;
  evidence: string[];
}

export interface EvaluationSampleV1 {
  schemaVersion: typeof EVALUATION_DATASET_SCHEMA_VERSION;
  id: string;
  input: NoticeInput;
  coverage: string[];
  expected: {
    category:
      "required-action" | "schedule" | "voluntary" | "result-or-change" | "information-only";
    changeIntent: "none" | "reschedule" | "cancel";
    taskCount: number;
    tasks: ExpectedTask[];
    uncertainties: string[];
  };
}

export function assertEvaluationSample(value: unknown): asserts value is EvaluationSampleV1 {
  if (!value || typeof value !== "object") throw new Error("sample must be an object");
  const sample = value as Partial<EvaluationSampleV1>;
  if (sample.schemaVersion !== EVALUATION_DATASET_SCHEMA_VERSION) {
    throw new Error("unsupported sample schema version");
  }
  if (!sample.id || !sample.input || !sample.expected || !Array.isArray(sample.coverage)) {
    throw new Error("sample is missing required fields");
  }
  if (sample.input.kind === "text" && !sample.input.text.trim()) {
    throw new Error(`${sample.id}: text input is empty`);
  }
  if (
    sample.input.kind === "image" &&
    (!sample.input.asset || !sample.input.referenceText.trim())
  ) {
    throw new Error(`${sample.id}: image input is incomplete`);
  }
  if (sample.expected.taskCount !== sample.expected.tasks.length) {
    throw new Error(`${sample.id}: taskCount does not match tasks`);
  }
  for (const task of sample.expected.tasks) {
    if (!task.title || !Array.isArray(task.evidence) || task.evidence.length === 0) {
      throw new Error(`${sample.id}: task has no title or evidence`);
    }
  }
}
