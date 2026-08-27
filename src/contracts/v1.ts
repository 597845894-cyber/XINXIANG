export const CONTRACT_VERSION = 1 as const;

export type AppRouteId = "inbox" | "quickImport" | "review" | "tasks" | "settings";

export type CommandName =
  | "getAppBootstrap"
  | "getSecurityStatus"
  | "getModelResourceStatus"
  | "installModelResources"
  | "importTextNotice"
  | "importImageNotice"
  | "listNotices"
  | "getNoticeDetail"
  | "getNoticeImagePreview"
  | "updateNoticePublishedTime"
  | "setNoticeState"
  | "analyzeNotice"
  | "cancelAnalysis"
  | "listReviewCandidates"
  | "editTaskCandidate"
  | "confirmTaskCandidate"
  | "ignoreTaskCandidate"
  | "mergeTaskCandidates"
  | "splitTaskCandidate"
  | "listTasks"
  | "createManualTask"
  | "updateTask"
  | "setTaskState"
  | "getTaskHistory"
  | "suggestNoticeRelations"
  | "listNoticeRelations"
  | "resolveNoticeRelation"
  | "openQuickImport"
  | "quitApp";

export type ModelResourceState = "available" | "missing" | "corrupt";

export interface ModelResourceIssueV1 {
  resourceId: string;
  path: string;
  reason: string;
}

export interface ModelResourceStatusV1 {
  schemaVersion: 1;
  selectionId: string;
  state: ModelResourceState;
  issues: ModelResourceIssueV1[];
  recoveryAction: string | null;
  networkRequired: false;
}

export interface SecurityStatusV1 {
  schemaVersion: 1;
  masterKey: "currentWindowsUserProtected";
  database: "sqlCipherVerified";
  attachments: "aes256Gcm";
  businessNetworking: "blocked";
  updater: "disabled";
}

export type EventName =
  | "quickImportRequested"
  | "windowHiddenToTray"
  | "relativeDateRecalculationRequested"
  | "analysisProgress"
  | "analysisCompleted";

export interface OcrPointV1 {
  x: number;
  y: number;
}

export interface OcrLineV1 {
  text: string;
  confidence: number;
  boxPoints: [OcrPointV1, OcrPointV1, OcrPointV1, OcrPointV1];
}

export interface OcrResultV1 {
  adapter: string;
  elapsedMs: number;
  lines: OcrLineV1[];
  lowConfidence: boolean;
}

export interface ExtractedFieldV1 {
  name: string;
  value: string | null;
  confidence: number;
  evidence: string[];
  status: "trusted" | "needsReview" | "missing" | "conflict";
}

export interface TaskCandidatePayloadV1 {
  title: string;
  startAt: string | null;
  dueAt: string | null;
  dueExpression: string | null;
  location: string | null;
  submissionUrl: string | null;
  materials: string[];
  audience: string | null;
  required: boolean | null;
  confidence: number;
  evidence: string[];
  status: "trusted" | "needsReview" | "missing" | "conflict";
}

export interface AnalysisResultV1 {
  schemaVersion: 1;
  revisionId: string;
  classifierVersion: string;
  normalizedText: string;
  ocr: OcrResultV1 | null;
  category: string;
  categoryConfidence: number;
  fields: ExtractedFieldV1[];
  candidates: TaskCandidatePayloadV1[];
  warnings: string[];
  requiresReview: boolean;
}

export interface AnalysisProgressV1 {
  noticeId: string;
  stage: string;
  progressPercent: number;
}

export interface CandidateViewV1 {
  id: string;
  noticeId: string;
  analysisRevisionId: string;
  state: "pending" | "confirmed" | "ignored" | "merged";
  payload: TaskCandidatePayloadV1;
  createdAt: string;
}

export interface TaskViewV1 {
  id: string;
  noticeId: string | null;
  state: "todo" | "completed" | "cancelled";
  payload: TaskCandidatePayloadV1;
  createdAt: string;
  updatedAt: string;
}

export interface NoticeRelationViewV1 {
  id: string;
  noticeId: string;
  relatedNoticeId: string;
  relationType: "duplicate" | "supplement" | "reschedule" | "cancel";
  relationState: "suggested" | "accepted" | "rejected";
  evidence: Record<string, unknown>;
  createdAt: string;
}

export type NoticeState =
  | "pendingAnalysis"
  | "pendingReview"
  | "partiallyProcessed"
  | "processed"
  | "informationOnly"
  | "failed";

export interface NoticeSummaryV1 {
  id: string;
  sourceKind: "text" | "image";
  inboxState: NoticeState;
  publishedAt: string;
  publishedTimeSource: "importTimeTentative" | "userConfirmed";
  publishedTimeCandidate: string | null;
  publishedTimeCandidateSource: "embeddedMetadata" | "embeddedText" | null;
  createdAt: string;
  excerpt: string;
}

export interface SourceAssetInfoV1 {
  id: string;
  mediaType: "image/png" | "image/jpeg" | "image/webp";
  byteSize: number;
  pixelWidth: number | null;
  pixelHeight: number | null;
}

export interface NoticeDetailV1 extends NoticeSummaryV1 {
  originalText: string | null;
  sourceAsset: SourceAssetInfoV1 | null;
}

export interface ImagePreviewV1 {
  mediaType: SourceAssetInfoV1["mediaType"];
  bytes: number[];
}

export interface RouteDescriptorV1 {
  id: AppRouteId;
  label: string;
}

export interface AppBootstrapV1 {
  contractVersion: typeof CONTRACT_VERSION;
  appVersion: string;
  routes: RouteDescriptorV1[];
  commands: CommandName[];
  events: EventName[];
}

export function isAppBootstrapV1(value: unknown): value is AppBootstrapV1 {
  if (!value || typeof value !== "object") return false;

  const candidate = value as Partial<AppBootstrapV1>;
  return (
    candidate.contractVersion === CONTRACT_VERSION &&
    typeof candidate.appVersion === "string" &&
    Array.isArray(candidate.routes) &&
    Array.isArray(candidate.commands) &&
    Array.isArray(candidate.events)
  );
}
