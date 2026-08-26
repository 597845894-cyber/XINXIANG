export type NoticeCategory =
  "required-action" | "schedule" | "voluntary" | "result-or-change" | "information-only";

export interface SemanticTask {
  title: string;
  timeExpression: string | null;
  locationOrEntry: string | null;
  materials: string[];
  audience: string | null;
  required: boolean | null;
  evidence: string[];
}

export interface SemanticResult {
  category: NoticeCategory;
  changeIntent: "none" | "reschedule" | "cancel";
  tasks: SemanticTask[];
  uncertainties: string[];
}

export interface SemanticAdapter {
  readonly candidateId: string;
  analyze(text: string, signal?: AbortSignal): Promise<SemanticResult>;
}

const categories = new Set<NoticeCategory>([
  "required-action",
  "schedule",
  "voluntary",
  "result-or-change",
  "information-only",
]);

export function isSemanticResult(value: unknown): value is SemanticResult {
  if (!value || typeof value !== "object") return false;
  const result = value as Partial<SemanticResult>;
  return (
    categories.has(result.category as NoticeCategory) &&
    ["none", "reschedule", "cancel"].includes(result.changeIntent ?? "") &&
    Array.isArray(result.tasks) &&
    Array.isArray(result.uncertainties) &&
    result.tasks.every(
      (task) =>
        typeof task.title === "string" &&
        Array.isArray(task.materials) &&
        Array.isArray(task.evidence) &&
        (task.required === null || typeof task.required === "boolean"),
    )
  );
}
