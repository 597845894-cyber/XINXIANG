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
  "quickImportRequested" | "windowHiddenToTray" | "relativeDateRecalculationRequested";

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
