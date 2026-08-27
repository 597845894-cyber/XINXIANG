import { invoke } from "@tauri-apps/api/core";
import type {
  ImagePreviewV1,
  ModelResourceStatusV1,
  NoticeDetailV1,
  NoticeState,
  NoticeSummaryV1,
  SecurityStatusV1,
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

export async function importImageNotice(
  bytes: number[],
  declaredMediaType: string | null,
  publishedAt: string,
): Promise<NoticeSummaryV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeSummaryV1>("import_image_notice", {
    bytes,
    declaredMediaType,
    publishedAt,
  });
}

export async function listNotices(state?: NoticeState): Promise<NoticeSummaryV1[] | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeSummaryV1[]>("list_notices", { state: state ?? null });
}

export async function getNoticeDetail(noticeId: string): Promise<NoticeDetailV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<NoticeDetailV1>("get_notice_detail", { noticeId });
}

export async function getNoticeImagePreview(noticeId: string): Promise<ImagePreviewV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<ImagePreviewV1>("get_notice_image_preview", { noticeId });
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
