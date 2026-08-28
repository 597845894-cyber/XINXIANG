import { invoke } from "@tauri-apps/api/core";
import type {
  CandidateViewV1,
  AnalysisProgressV1,
  AnalysisRevisionViewV1,
  AnalysisResultV1,
  BackupSummaryV1,
  ModelResourceStatusV1,
  NoticeDetailV1,
  NoticeState,
  NoticeSummaryV1,
  SecurityStatusV1,
  TaskViewV1,
  NoticeRelationViewV1,
  TaskCandidatePayloadV1,
  TaskRevisionViewV1,
  ReminderViewV1,
} from "../contracts/v1";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isDesktopRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

export async function quitDesktopApp() {
  if (!isDesktopRuntime()) return;
  await invoke("quit_app");
}

export async function getSecurityStatus(): Promise<SecurityStatusV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<SecurityStatusV1>("get_security_status");
}

export async function getModelResourceStatus(): Promise<ModelResourceStatusV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<ModelResourceStatusV1>("get_model_resource_status");
}

export async function installModelResources(
  sourceDirectory: string,
): Promise<ModelResourceStatusV1> {
  return invoke<ModelResourceStatusV1>("install_model_resources", { sourceDirectory });
}

export async function importTextNotice(
  originalText: string,
  publishedAt: string,
): Promise<NoticeSummaryV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeSummaryV1>("import_text_notice", { originalText, publishedAt });
}

export async function listNotices(state?: NoticeState): Promise<NoticeSummaryV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeSummaryV1[]>("list_notices", { state: state ?? null });
}

export async function getNoticeDetail(noticeId: string): Promise<NoticeDetailV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeDetailV1>("get_notice_detail", { noticeId });
}

export async function updateNoticePublishedTime(
  noticeId: string,
  publishedAt: string,
): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("update_notice_published_time", { noticeId, publishedAt });
}

export async function setNoticeState(noticeId: string, state: NoticeState): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("set_notice_state", { noticeId, state });
}

export async function analyzeNotice(
  noticeId: string,
  manualText?: string,
  summaryMode?: "aggregated" | "flat_legacy",
): Promise<AnalysisResultV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<AnalysisResultV1>("analyze_notice", {
    noticeId,
    manualText: manualText ?? null,
    summaryMode: summaryMode ?? "aggregated",
  });
}

export async function cancelAnalysis(noticeId: string): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("cancel_analysis", { noticeId });
}

export async function listAnalysisRevisions(
  noticeId: string,
): Promise<AnalysisRevisionViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<AnalysisRevisionViewV1[]>("list_analysis_revisions", { noticeId });
}

export async function listReviewCandidates(): Promise<CandidateViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<CandidateViewV1[]>("list_review_candidates");
}

export async function editTaskCandidate(candidateId: string, payload: TaskCandidatePayloadV1) {
  if (!isDesktopRuntime()) return;
  await invoke("edit_task_candidate", { candidateId, payload });
}

export async function confirmTaskCandidate(
  candidateId: string,
  payload: TaskCandidatePayloadV1,
): Promise<TaskViewV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<TaskViewV1>("confirm_task_candidate", { candidateId, payload });
}

export async function ignoreTaskCandidate(candidateId: string) {
  if (!isDesktopRuntime()) return;
  await invoke("ignore_task_candidate", { candidateId });
}

export async function mergeTaskCandidates(
  targetId: string,
  sourceIds: string[],
  payload: TaskCandidatePayloadV1,
) {
  if (!isDesktopRuntime()) return;
  await invoke("merge_task_candidates", { targetId, sourceIds, payload });
}

export async function splitTaskCandidate(candidateId: string, payloads: TaskCandidatePayloadV1[]) {
  if (!isDesktopRuntime()) return;
  await invoke("split_task_candidate", { candidateId, payloads });
}

export async function listTasks(): Promise<TaskViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<TaskViewV1[]>("list_tasks");
}

export async function createManualTask(
  payload: TaskCandidatePayloadV1,
): Promise<TaskViewV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<TaskViewV1>("create_manual_task", { payload });
}

export async function updateTask(taskId: string, payload: TaskCandidatePayloadV1) {
  if (!isDesktopRuntime()) return;
  await invoke("update_task", { taskId, payload });
}

export async function setTaskState(
  taskId: string,
  state: TaskViewV1["state"],
  payload: TaskCandidatePayloadV1,
) {
  if (!isDesktopRuntime()) return;
  await invoke("set_task_state", { taskId, state, payload });
}

export async function listReminders(taskId?: string): Promise<ReminderViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<ReminderViewV1[]>("list_reminders", { taskId: taskId ?? null });
}

export async function upsertReminder(
  taskId: string,
  scheduledAt: string,
  idempotencyKey: string,
  reminderId?: string,
): Promise<ReminderViewV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<ReminderViewV1>("upsert_reminder", {
    taskId,
    reminderId: reminderId ?? crypto.randomUUID(),
    scheduledAt,
    idempotencyKey,
  });
}

export async function deleteReminder(reminderId: string) {
  if (!isDesktopRuntime()) return;
  await invoke("delete_reminder", { reminderId });
}

export async function runReminderScan(): Promise<number> {
  if (!isDesktopRuntime()) return 0;
  return invoke<number>("run_reminder_scan");
}

export async function getTaskHistory(taskId: string): Promise<TaskRevisionViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<TaskRevisionViewV1[]>("get_task_history", { taskId });
}

export async function suggestNoticeRelations(
  noticeId: string,
): Promise<NoticeRelationViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeRelationViewV1[]>("suggest_notice_relations", { noticeId });
}

export async function listNoticeRelations(
  noticeId?: string,
): Promise<NoticeRelationViewV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeRelationViewV1[]>("list_notice_relations", { noticeId: noticeId ?? null });
}

export async function resolveNoticeRelation(relationId: string, accepted: boolean) {
  if (!isDesktopRuntime()) return;
  await invoke("resolve_notice_relation", { relationId, accepted });
}

export async function createBackup(
  targetPath: string,
  password: string,
): Promise<BackupSummaryV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<BackupSummaryV1>("create_backup", { targetPath, password });
}

export async function inspectBackup(
  path: string,
  password: string,
): Promise<BackupSummaryV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<BackupSummaryV1>("inspect_backup", { path, password });
}

export async function restoreBackup(
  path: string,
  password: string,
): Promise<BackupSummaryV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<BackupSummaryV1>("restore_backup", { path, password, confirmed: true });
}

export async function deleteNoticeCascade(noticeId: string) {
  if (!isDesktopRuntime()) return;
  await invoke("delete_notice_cascade", { noticeId, confirmed: true });
}

export async function deleteNoticeKeepTasks(noticeId: string) {
  if (!isDesktopRuntime()) return;
  await invoke("delete_notice_keep_tasks", { noticeId, confirmed: true });
}

export type AnalysisProgressHandler = (progress: AnalysisProgressV1) => void;
