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
  id?: string | null;
  parentId?: string | null;
  dependsOn?: string[];
  relation?: "standalone" | "parent" | "preparation" | "conditional";
  condition?: string | null;
  timeScopeId?: string | null;
  summary?: string | null;
  aggregationKey?: string | null;
  detailActions?: string[];
  timeSummary?: string | null;
  needsConfirmation?: boolean;
  aggregationNote?: string | null;
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
        (task.required === null || typeof task.required === "boolean") &&
        (task.id === undefined || task.id === null || typeof task.id === "string") &&
        (task.parentId === undefined || task.parentId === null || typeof task.parentId === "string") &&
        (task.dependsOn === undefined || Array.isArray(task.dependsOn)) &&
        (task.relation === undefined || ["standalone", "parent", "preparation", "conditional"].includes(task.relation)) &&
        (task.condition === undefined || task.condition === null || typeof task.condition === "string") &&
        (task.timeScopeId === undefined || task.timeScopeId === null || typeof task.timeScopeId === "string") &&
        (task.summary === undefined || task.summary === null || typeof task.summary === "string") &&
        (task.aggregationKey === undefined || task.aggregationKey === null || typeof task.aggregationKey === "string") &&
        (task.detailActions === undefined || Array.isArray(task.detailActions)) &&
        (task.timeSummary === undefined || task.timeSummary === null || typeof task.timeSummary === "string") &&
        (task.needsConfirmation === undefined || typeof task.needsConfirmation === "boolean") &&
        (task.aggregationNote === undefined || task.aggregationNote === null || typeof task.aggregationNote === "string"),
    )
  );
}
