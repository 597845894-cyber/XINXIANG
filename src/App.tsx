import {
  Bell,
  CalendarDays,
  Check,
  ChevronRight,
  ClipboardPaste,
  Clock3,
  FileCheck2,
  Inbox,
  ListChecks,
  MonitorCog,
  Plus,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type ComponentType } from "react";

import type {
  AnalysisResultV1,
  AnalysisRevisionViewV1,
  AppRouteId,
  CandidateViewV1,
  NoticeDetailV1,
  NoticeRelationViewV1,
  NoticeState,
  NoticeSummaryV1,
  TaskCandidatePayloadV1,
  TaskRevisionViewV1,
  TaskViewV1,
  NotificationEventV1,
} from "./contracts/v1";
import {
  getNoticeDetail,
  getNoticeImagePreview,
  getSecurityStatus,
  analyzeNotice,
  cancelAnalysis,
  confirmTaskCandidate,
  createManualTask,
  editTaskCandidate,
  getTaskHistory,
  importImageNotice,
  importTextNotice,
  ignoreTaskCandidate,
  isDesktopRuntime,
  listReviewCandidates,
  listTasks,
  listNotices,
  listAnalysisRevisions,
  mergeTaskCandidates,
  quitDesktopApp,
  setTaskState,
  setNoticeState,
  splitTaskCandidate,
  suggestNoticeRelations,
  resolveNoticeRelation,
  updateTask,
  updateNoticePublishedTime,
  listReminders,
  upsertReminder,
  deleteReminder,
  createBackup,
  inspectBackup,
  restoreBackup,
  deleteNoticeCascade,
  deleteNoticeKeepTasks,
} from "./platform/desktop";

type NavItem = {
  id: AppRouteId;
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  badge?: number;
};

const navItems: NavItem[] = [
  { id: "inbox", label: "收件箱", icon: Inbox, badge: 3 },
  { id: "quickImport", label: "快速导入", icon: ClipboardPaste },
  { id: "review", label: "任务核对", icon: FileCheck2, badge: 2 },
  { id: "tasks", label: "任务表", icon: ListChecks },
  { id: "settings", label: "设置", icon: Settings },
];

const noticeStates: Array<{ id: NoticeState | "all"; label: string }> = [
  { id: "all", label: "全部" },
  { id: "pendingAnalysis", label: "待分析" },
  { id: "pendingReview", label: "待确认" },
  { id: "partiallyProcessed", label: "部分处理" },
  { id: "processed", label: "已处理" },
  { id: "informationOnly", label: "仅供知晓" },
  { id: "failed", label: "处理失败" },
];

const noticeStateLabels: Record<NoticeState, string> = {
  pendingAnalysis: "待分析",
  pendingReview: "待确认",
  partiallyProcessed: "部分处理",
  processed: "已处理",
  informationOnly: "仅供知晓",
  failed: "处理失败",
};

function noticeTone(state: NoticeState) {
  if (state === "informationOnly" || state === "processed") return "success";
  if (state === "pendingReview" || state === "partiallyProcessed" || state === "failed")
    return "warning";
  return "neutral";
}

function displayNoticeTitle(notice: NoticeSummaryV1) {
  if (notice.sourceKind === "image") return "通知截图";
  return notice.excerpt.split(/\r?\n/, 1)[0] || "文字通知";
}

function toDateTimeLocal(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "";
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.valueOf() - offset).toISOString().slice(0, 16);
}

function initialPublishedTime() {
  return toDateTimeLocal(new Date().toISOString());
}

function toPublishedTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? "" : date.toISOString();
}

function AppHeader({
  activeRoute,
  onNotifications,
  notificationCount,
}: {
  activeRoute: AppRouteId;
  onNotifications: () => void;
  notificationCount: number;
}) {
  const titles: Record<AppRouteId, string> = {
    inbox: "收件箱",
    quickImport: "快速导入",
    review: "任务核对",
    tasks: "任务表",
    settings: "设置",
  };
  return (
    <header className="app-header">
      <div>
        <p className="section-eyebrow">个人工作区</p>
        <h1>{titles[activeRoute]}</h1>
      </div>
      <div className="header-actions">
        <label className="search-field">
          <Search size={17} aria-hidden="true" />
          <span className="sr-only">搜索通知和任务</span>
          <input type="search" placeholder="搜索通知和任务" />
        </label>
        <button
          className="icon-button"
          type="button"
          title="通知中心"
          aria-label="通知中心"
          onClick={onNotifications}
        >
          <Bell size={19} />
          {notificationCount > 0 ? <span className="notification-dot" /> : null}
        </button>
      </div>
    </header>
  );
}

function InboxView({ openImport }: { openImport: () => void }) {
  const [filter, setFilter] = useState<NoticeState | "all">("all");
  const [notices, setNotices] = useState<NoticeSummaryV1[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<NoticeDetailV1 | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [sourceOpen, setSourceOpen] = useState(false);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisResultV1 | null>(null);
  const [analysisState, setAnalysisState] = useState<"idle" | "running" | "failed">("idle");
  const [ocrEditorOpen, setOcrEditorOpen] = useState(false);
  const [manualOcrText, setManualOcrText] = useState("");
  const [revisions, setRevisions] = useState<AnalysisRevisionViewV1[]>([]);
  const [revisionsOpen, setRevisionsOpen] = useState(false);
  const [deleteMode, setDeleteMode] = useState<"cascade" | "keepTasks" | null>(null);
  const [deleteAcknowledged, setDeleteAcknowledged] = useState(false);

  async function refresh() {
    if (!isDesktopRuntime()) return;
    setLoading(true);
    setError("");
    try {
      const next = await listNotices(filter === "all" ? undefined : filter);
      setNotices(next ?? []);
      setSelectedId((current) =>
        current && next?.some((notice) => notice.id === current)
          ? current
          : (next?.[0]?.id ?? null),
      );
    } catch {
      setError("无法读取本地通知，请确认应用数据目录可用。");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    const refreshTimer = window.setTimeout(() => void refresh(), 0);
    return () => window.clearTimeout(refreshTimer);
    // The filter identifies the requested local query; refresh itself changes on each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter]);

  useEffect(() => {
    if (!selectedId || !isDesktopRuntime()) return;
    void getNoticeDetail(selectedId)
      .then((next) => setDetail(next))
      .catch(() => setError("无法读取该通知的原始依据。"));
    void listAnalysisRevisions(selectedId)
      .then((next) => setRevisions(next ?? []))
      .catch(() => setRevisions([]));
  }, [selectedId]);

  useEffect(() => {
    return () => {
      if (previewUrl) URL.revokeObjectURL(previewUrl);
    };
  }, [previewUrl]);

  async function openSource() {
    if (!detail) return;
    setError("");
    if (detail.sourceAsset) {
      try {
        const preview = await getNoticeImagePreview(detail.id);
        if (preview) {
          const nextUrl = URL.createObjectURL(
            new Blob([new Uint8Array(preview.bytes)], { type: preview.mediaType }),
          );
          setPreviewUrl((current) => {
            if (current) URL.revokeObjectURL(current);
            return nextUrl;
          });
        }
      } catch {
        setError("截图无法读取，文件可能已损坏。");
        return;
      }
    }
    setSourceOpen(true);
  }

  async function savePublishedTime(value: string) {
    if (!detail) return;
    const publishedAt = toPublishedTime(value);
    if (!publishedAt) return;
    try {
      await updateNoticePublishedTime(detail.id, publishedAt);
      setDetail({
        ...detail,
        publishedAt,
        publishedTimeSource: "userConfirmed",
        publishedTimeCandidate: null,
        publishedTimeCandidateSource: null,
      });
      setNotices((current) =>
        current.map((notice) =>
          notice.id === detail.id
            ? {
                ...notice,
                publishedAt,
                publishedTimeSource: "userConfirmed",
                publishedTimeCandidate: null,
                publishedTimeCandidateSource: null,
              }
            : notice,
        ),
      );
    } catch {
      setError("发布时间未能保存。");
    }
  }

  async function deleteSelectedNotice() {
    if (!detail || !deleteMode || !deleteAcknowledged) return;
    try {
      if (deleteMode === "cascade") await deleteNoticeCascade(detail.id);
      else await deleteNoticeKeepTasks(detail.id);
      setDeleteMode(null);
      setDeleteAcknowledged(false);
      setDetail(null);
      setSelectedId(null);
      await refresh();
    } catch {
      setError("删除未完成。本地数据保持不变，请稍后重试。");
    }
  }

  async function markInformationOnly() {
    if (!detail) return;
    try {
      await setNoticeState(detail.id, "informationOnly");
      await refresh();
    } catch {
      setError("通知状态未能更新。");
    }
  }

  async function runAnalysis(manualText?: string) {
    if (!detail) return;
    setAnalysisState("running");
    setError("");
    try {
      const result = await analyzeNotice(detail.id, manualText);
      setAnalysis(result);
      const nextRevisions = await listAnalysisRevisions(detail.id);
      setRevisions(nextRevisions ?? []);
      await refresh();
      if (result) {
        setDetail((current) =>
          current
            ? { ...current, inboxState: result.requiresReview ? "pendingReview" : "processed" }
            : current,
        );
      }
      setAnalysisState("idle");
    } catch (error) {
      setAnalysisState("failed");
      setError(
        String(error).includes("ANALYSIS_OCR_NO_TEXT")
          ? "未能从截图提取文字，可先手工录入后重试。"
          : "本地分析未完成，请稍后重试。",
      );
    }
  }

  async function stopAnalysis() {
    if (!detail) return;
    try {
      await cancelAnalysis(detail.id);
      setError("已请求取消本地分析。");
    } catch {
      setError("取消分析请求未能发送。");
    }
  }

  function openOcrEditor() {
    setManualOcrText(
      (current) => current || analysis?.normalizedText || revisions[0]?.ocrText || "",
    );
    setOcrEditorOpen(true);
  }

  function selectNotice(noticeId: string) {
    setAnalysis(null);
    setAnalysisState("idle");
    setOcrEditorOpen(false);
    setManualOcrText("");
    setRevisionsOpen(false);
    setSelectedId(noticeId);
  }

  const currentRevision = revisions[0];
  const previousRevision = revisions[1];
  const candidateTitles = (revision: AnalysisRevisionViewV1 | undefined) =>
    revision?.candidates.map((candidate) => candidate.title).join("、") || "未生成任务候选";

  return (
    <div className="inbox-view">
      <section className="notice-column" aria-label="通知列表">
        <div className="section-toolbar">
          <div className="segmented-control" aria-label="通知筛选">
            {noticeStates.map((state) => (
              <button
                className={filter === state.id ? "is-active" : ""}
                key={state.id}
                onClick={() => setFilter(state.id)}
                type="button"
              >
                {state.label}
              </button>
            ))}
          </div>
          <button
            className="icon-button subtle"
            onClick={openImport}
            type="button"
            title="导入通知"
            aria-label="导入通知"
          >
            <Plus size={18} />
          </button>
        </div>
        <div className="notice-list">
          {loading ? <p className="empty-notice">正在读取本地通知...</p> : null}
          {!loading && !notices.length ? (
            <p className="empty-notice">这里还没有通知。导入文字或截图后会显示在这里。</p>
          ) : null}
          {notices.map((notice) => (
            <button
              className={`notice-row ${selectedId === notice.id ? "is-selected" : ""}`}
              key={notice.id}
              onClick={() => selectNotice(notice.id)}
              type="button"
            >
              <span className={`status-marker ${noticeTone(notice.inboxState)}`} />
              <span className="notice-copy">
                <span className="notice-row-topline">
                  <strong>{displayNoticeTitle(notice)}</strong>
                  <time>{new Date(notice.publishedAt).toLocaleString()}</time>
                </span>
                <span className="notice-excerpt">
                  {notice.excerpt || "已加密保存的截图，等待本地识别。"}
                </span>
                <span className={`status-label ${noticeTone(notice.inboxState)}`}>
                  {noticeStateLabels[notice.inboxState]}
                </span>
              </span>
            </button>
          ))}
        </div>
      </section>
      <section className="notice-detail" aria-label="通知详情">
        {detail ? (
          <>
            <div className="detail-heading">
              <div>
                <span className={`status-label ${noticeTone(detail.inboxState)}`}>
                  {noticeStateLabels[detail.inboxState]}
                </span>
                <h2>{displayNoticeTitle(detail)}</h2>
                <p>
                  {detail.sourceKind === "text" ? "来自文字粘贴" : "来自截图"} ·{" "}
                  {new Date(detail.publishedAt).toLocaleString()}
                </p>
              </div>
              <button className="secondary-button" onClick={() => void openSource()} type="button">
                查看原文 <ChevronRight size={16} />
              </button>
            </div>
            <div className="analysis-state">
              <Sparkles size={19} />
              <div>
                <strong>
                  {analysisState === "running"
                    ? "正在本地分析"
                    : analysisState === "failed"
                      ? "分析失败，可修正后重试"
                      : analysis
                        ? "分析完成，等待核对"
                        : "等待本地分析"}
                </strong>
                <span>原始内容已加密保存，分析结果会生成新的版本。</span>
              </div>
              {!analysis && detail.inboxState !== "processed" ? (
                <button
                  className="secondary-button"
                  disabled={analysisState === "running"}
                  onClick={() => void runAnalysis()}
                  type="button"
                >
                  {analysisState === "running" ? "分析中" : "开始分析"}
                </button>
              ) : null}
              {analysisState === "running" ? (
                <button
                  className="secondary-button"
                  onClick={() => void stopAnalysis()}
                  type="button"
                >
                  <X size={16} />
                  取消分析
                </button>
              ) : null}
            </div>
            {detail.sourceKind === "image" ? (
              <section className="ocr-correction" aria-label="OCR 文字修正">
                <div>
                  <strong>识别文字修正</strong>
                  <p>原始截图保持不变。修正后的文字会作为新的分析版本保存。</p>
                </div>
                <button className="secondary-button" onClick={openOcrEditor} type="button">
                  修正识别文字
                </button>
                {ocrEditorOpen ? (
                  <div className="ocr-editor">
                    <label>
                      手工输入或修正后的文字
                      <textarea
                        aria-label="手工输入或修正后的文字"
                        onChange={(event) => setManualOcrText(event.target.value)}
                        placeholder="OCR 失败时可在这里录入截图中的通知文字"
                        value={manualOcrText}
                      />
                    </label>
                    <div className="ocr-editor-actions">
                      <button
                        className="secondary-button"
                        disabled={!manualOcrText.trim() || analysisState === "running"}
                        onClick={() => void runAnalysis(manualOcrText.trim())}
                        type="button"
                      >
                        使用修正文字重新分析
                      </button>
                      <button
                        className="icon-button"
                        aria-label="关闭文字修正"
                        onClick={() => setOcrEditorOpen(false)}
                        title="关闭"
                        type="button"
                      >
                        <X size={16} />
                      </button>
                    </div>
                  </div>
                ) : null}
              </section>
            ) : null}
            {revisions.length ? (
              <section className="analysis-revisions" aria-label="分析版本差异">
                <div>
                  <strong>分析版本</strong>
                  <p>重新分析只会新增版本，不会覆盖已确认的任务。</p>
                </div>
                <button
                  className="secondary-button"
                  onClick={() => setRevisionsOpen((open) => !open)}
                  type="button"
                >
                  {revisionsOpen ? "收起版本记录" : `查看版本差异（${revisions.length}）`}
                </button>
                {revisionsOpen ? (
                  <div className="revision-list">
                    {currentRevision && previousRevision ? (
                      <p className="revision-diff">
                        与上一版对比：任务候选由“{candidateTitles(previousRevision)}”变为“
                        {candidateTitles(currentRevision)}”。
                      </p>
                    ) : null}
                    {revisions.map((revision, index) => (
                      <article className="revision-row" key={revision.id}>
                        <strong>版本 {revision.revisionNumber}</strong>
                        <span>{new Date(revision.createdAt).toLocaleString()}</span>
                        <span>{index === 0 ? "当前分析结果" : revision.classifierVersion}</span>
                        <p>任务候选：{candidateTitles(revision)}</p>
                        {revision.ocrText ? <pre>{revision.ocrText}</pre> : null}
                      </article>
                    ))}
                  </div>
                ) : null}
              </section>
            ) : null}
            {analysis ? (
              <div className="analysis-result" aria-label="本地分析结果">
                <div className="analysis-result-heading">
                  <strong>{analysis.category}</strong>
                  <span>{Math.round(analysis.categoryConfidence * 100)}% 可信</span>
                </div>
                {analysis.ocr ? <pre className="ocr-text">{analysis.normalizedText}</pre> : null}
                {analysis.candidates.length ? (
                  <div className="candidate-list">
                    {analysis.candidates.map((candidate) => (
                      <article
                        key={`${analysis.revisionId}-${candidate.title}`}
                        className="candidate-card"
                      >
                        <strong>{candidate.title}</strong>
                        <span>
                          {candidate.dueAt
                            ? new Date(candidate.dueAt).toLocaleString()
                            : "时间待确认"}
                        </span>
                        <em>{candidate.status === "trusted" ? "可快速核对" : "需要核对"}</em>
                      </article>
                    ))}
                  </div>
                ) : (
                  <p className="source-meta">未生成任务候选，可标记为仅供知晓。</p>
                )}
              </div>
            ) : null}
            <div className="capture-details">
              <label className="import-input">
                <span>
                  通知发布时间{" "}
                  {detail.publishedTimeSource === "importTimeTentative"
                    ? "（暂定，建议核对）"
                    : "（已确认）"}
                </span>
                <input
                  aria-label="通知发布时间"
                  defaultValue={toDateTimeLocal(detail.publishedAt)}
                  onBlur={(event) => void savePublishedTime(event.target.value)}
                  type="datetime-local"
                />
              </label>
              {detail.publishedTimeCandidate ? (
                <div className="time-candidate" role="status">
                  <div>
                    <strong>发现截图时间候选</strong>
                    <span>
                      {new Date(detail.publishedTimeCandidate).toLocaleString()} ·{" "}
                      {detail.publishedTimeCandidateSource === "embeddedMetadata"
                        ? "图片元数据"
                        : "图片内嵌时间串"}
                    </span>
                  </div>
                  <button
                    className="secondary-button"
                    onClick={() =>
                      void savePublishedTime(toDateTimeLocal(detail.publishedTimeCandidate!))
                    }
                    type="button"
                  >
                    采用候选时间
                  </button>
                </div>
              ) : null}
              {detail.sourceAsset ? (
                <p className="source-meta">
                  截图已加密保存 · {detail.sourceAsset.pixelWidth} ×{" "}
                  {detail.sourceAsset.pixelHeight}
                </p>
              ) : (
                <p className="source-meta">文字原文已加密保存于本地数据库。</p>
              )}
            </div>
            <footer className="detail-actions">
              <button
                className="secondary-button"
                onClick={() => void markInformationOnly()}
                type="button"
              >
                标记为仅供知晓
              </button>
              <button
                className="danger-button"
                onClick={() => setDeleteMode("cascade")}
                type="button"
              >
                删除通知
              </button>
            </footer>
          </>
        ) : (
          <div className="empty-detail">
            <Inbox size={25} />
            <p>选择一条通知查看原始依据和发布时间。</p>
          </div>
        )}
        {error ? <p className="form-message error">{error}</p> : null}
        {sourceOpen && detail ? (
          <div className="source-dialog" role="dialog" aria-modal="true" aria-label="原始通知">
            <div className="source-dialog-content">
              <div className="detail-heading">
                <h2>原始通知</h2>
                <button
                  className="icon-button"
                  onClick={() => {
                    setSourceOpen(false);
                    setPreviewUrl((current) => {
                      if (current) URL.revokeObjectURL(current);
                      return null;
                    });
                  }}
                  type="button"
                  aria-label="关闭原文"
                >
                  ×
                </button>
              </div>
              {detail.originalText ? (
                <pre className="original-text">{detail.originalText}</pre>
              ) : null}
              {previewUrl ? (
                <img className="source-image" src={previewUrl} alt="原始通知截图" />
              ) : null}
            </div>
          </div>
        ) : null}
        {deleteMode && detail ? (
          <div className="source-dialog" role="dialog" aria-modal="true" aria-label="删除通知确认">
            <div className="source-dialog-content delete-dialog">
              <div className="detail-heading">
                <div>
                  <h2>{deleteMode === "cascade" ? "永久删除通知" : "保留任务并删除原文"}</h2>
                  <p>
                    {deleteMode === "cascade"
                      ? "将同时删除该通知的分析记录、关联任务、未来提醒和加密附件。"
                      : "任务将保留，但会明确标记为缺少原文依据，之后不能再查看通知内容或截图。"}
                  </p>
                </div>
                <button
                  className="icon-button"
                  onClick={() => setDeleteMode(null)}
                  type="button"
                  aria-label="关闭删除确认"
                >
                  ×
                </button>
              </div>
              <div className="delete-options">
                <button
                  className="secondary-button"
                  onClick={() => setDeleteMode("cascade")}
                  type="button"
                >
                  删除全部关联数据
                </button>
                <button
                  className="secondary-button"
                  onClick={() => setDeleteMode("keepTasks")}
                  type="button"
                >
                  保留任务，只删原文
                </button>
              </div>
              <label className="confirm-check">
                <input
                  checked={deleteAcknowledged}
                  onChange={(event) => setDeleteAcknowledged(event.target.checked)}
                  type="checkbox"
                />
                我理解此操作无法撤销
              </label>
              <div className="page-actions">
                <button
                  className="secondary-button"
                  onClick={() => setDeleteMode(null)}
                  type="button"
                >
                  取消
                </button>
                <button
                  className="danger-button"
                  disabled={!deleteAcknowledged}
                  onClick={() => void deleteSelectedNotice()}
                  type="button"
                >
                  确认删除
                </button>
              </div>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}

function QuickImportView({ imported }: { imported: () => void }) {
  const [mode, setMode] = useState<"text" | "image">("text");
  const [text, setText] = useState("");
  const [publishedAt, setPublishedAt] = useState(initialPublishedTime);
  const [image, setImage] = useState<{
    bytes: number[];
    mediaType: string;
    previewUrl: string;
  } | null>(null);
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    return () => {
      if (image) URL.revokeObjectURL(image.previewUrl);
    };
  }, [image]);

  async function setImageFromBlob(blob: Blob) {
    const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
    const previewUrl = URL.createObjectURL(blob);
    setImage((current) => {
      if (current) URL.revokeObjectURL(current.previewUrl);
      return { bytes, mediaType: blob.type, previewUrl };
    });
    setMessage("");
  }

  async function chooseImage(file: File | null) {
    if (!file) return;
    await setImageFromBlob(file);
  }

  async function readClipboardImage() {
    setMessage("");
    try {
      const clipboardItems = await navigator.clipboard.read();
      const item = clipboardItems.find((candidate) =>
        candidate.types.some((type) => ["image/png", "image/jpeg", "image/webp"].includes(type)),
      );
      const type = item?.types.find((candidate) =>
        ["image/png", "image/jpeg", "image/webp"].includes(candidate),
      );
      if (!item || !type) {
        setMessage("剪贴板中没有可导入的 PNG、JPG 或 WebP 图片。");
        return;
      }
      await setImageFromBlob(await item.getType(type));
    } catch {
      setMessage("无法读取剪贴板图片。请允许访问后重试，或直接选择本地截图。");
    }
  }

  async function createNotice() {
    const normalizedPublishedAt = toPublishedTime(publishedAt);
    if (!normalizedPublishedAt) {
      setMessage("请填写有效的通知发布时间。");
      return;
    }
    if (!isDesktopRuntime()) {
      setMessage("请在 Windows 桌面应用中导入通知。");
      return;
    }
    setSaving(true);
    setMessage("");
    try {
      if (mode === "text") await importTextNotice(text, normalizedPublishedAt);
      else if (image)
        await importImageNotice(image.bytes, image.mediaType || null, normalizedPublishedAt);
      else {
        setMessage("请先选择或粘贴一张通知截图。");
        return;
      }
      setText("");
      setImage((current) => {
        if (current) URL.revokeObjectURL(current.previewUrl);
        return null;
      });
      setMessage("通知已加密保存到本机，等待本地分析。");
      imported();
    } catch (error) {
      const code = String(error);
      setMessage(
        code.includes("NOTICE_TEXT_REQUIRED")
          ? "请粘贴有效的通知文字。"
          : code.includes("NOTICE_IMAGE")
            ? "图片格式、大小或内容无效，请更换后重试。"
            : "通知未能保存，请检查本地数据目录。",
      );
    } finally {
      setSaving(false);
    }
  }

  function clearImport() {
    setText("");
    setImage((current) => {
      if (current) URL.revokeObjectURL(current.previewUrl);
      return null;
    });
    setMessage("");
  }

  return (
    <section className="content-page import-page" aria-labelledby="import-title">
      <div className="page-intro">
        <p className="section-eyebrow">新建通知</p>
        <h2 id="import-title">导入微信通知</h2>
        <p>内容只在当前设备中处理和保存。</p>
      </div>
      <div className="import-workspace">
        <div className="segmented-control large" aria-label="导入方式">
          <button
            className={mode === "text" ? "is-active" : ""}
            onClick={() => setMode("text")}
            type="button"
          >
            <ClipboardPaste size={17} /> 粘贴文字
          </button>
          <button
            className={mode === "image" ? "is-active" : ""}
            onClick={() => setMode("image")}
            type="button"
          >
            <Upload size={17} /> 上传截图
          </button>
        </div>
        {mode === "text" ? (
          <label className="import-input">
            <span>通知原文</span>
            <textarea
              value={text}
              onChange={(event) => setText(event.target.value)}
              placeholder="在此粘贴通知文字"
              rows={10}
            />
          </label>
        ) : (
          <>
            <input
              ref={fileInput}
              className="sr-only"
              accept="image/png,image/jpeg,image/webp"
              onChange={(event) => void chooseImage(event.target.files?.[0] ?? null)}
              type="file"
            />
            {image ? (
              <div className="image-import-preview">
                <img src={image.previewUrl} alt="待导入通知截图" />
                <button
                  className="secondary-button"
                  onClick={() => fileInput.current?.click()}
                  type="button"
                >
                  更换截图
                </button>
              </div>
            ) : (
              <button
                aria-label="选择通知截图"
                className="upload-zone"
                onClick={() => fileInput.current?.click()}
                type="button"
              >
                <Upload size={28} />
                <strong>选择通知截图</strong>
                <span>支持 PNG、JPG 和 WebP</span>
              </button>
            )}
            <button
              className="clipboard-image-button"
              onClick={() => void readClipboardImage()}
              type="button"
            >
              <ClipboardPaste size={16} /> 粘贴剪贴板图片
            </button>
          </>
        )}
        <div className="import-options">
          <label>
            <span>通知发布时间</span>
            <input
              aria-label="通知发布时间"
              value={publishedAt}
              onChange={(event) => setPublishedAt(event.target.value)}
              type="datetime-local"
            />
          </label>
          <span className="privacy-note">
            <ShieldCheck size={16} /> 本地处理
          </span>
        </div>
        <div className="page-actions">
          <button className="secondary-button" onClick={clearImport} type="button">
            清空
          </button>
          <button
            className="primary-button"
            disabled={saving}
            onClick={() => void createNotice()}
            type="button"
          >
            <Plus size={17} /> {saving ? "保存中" : "创建通知"}
          </button>
        </div>
        {message ? (
          <p className={`form-message ${message.includes("已加密") ? "success" : "error"}`}>
            {message}
          </p>
        ) : null}
      </div>
    </section>
  );
}

const demoCandidatePayload: TaskCandidatePayloadV1 = {
  title: "完成实验室安全准入考试",
  startAt: null,
  dueAt: "2026-08-28T17:00:00Z",
  dueExpression: "8 月 28 日 17:00 前",
  location: "线上考试平台",
  submissionUrl: "https://example.invalid/exam",
  materials: [],
  audience: "2026 级本科生",
  required: true,
  confidence: 0.56,
  evidence: ["请于 8 月 28 日 17:00 前完成实验室安全准入考试"],
  status: "needsReview",
};

const demoTaskPayload: TaskCandidatePayloadV1 = {
  ...demoCandidatePayload,
  confidence: 1,
  status: "trusted",
};

function ReviewView() {
  const [candidates, setCandidates] = useState<CandidateViewV1[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editedTitle, setEditedTitle] = useState("");
  const [message, setMessage] = useState("");
  const [relations, setRelations] = useState<NoticeRelationViewV1[]>([]);
  const selected = candidates.find((candidate) => candidate.id === selectedId) ?? candidates[0];

  async function refresh() {
    const next = await listReviewCandidates();
    if (next) {
      setCandidates(next);
      if (!selectedId && next[0]) {
        setSelectedId(next[0].id);
        setEditedTitle(next[0].payload.title);
      }
    } else if (!candidates.length) {
      const demoCandidates: CandidateViewV1[] = [
        {
          id: "demo-candidate-1",
          noticeId: "demo-notice",
          analysisRevisionId: "demo-revision",
          state: "pending",
          payload: demoCandidatePayload,
          createdAt: new Date().toISOString(),
        },
        {
          id: "demo-candidate-2",
          noticeId: "demo-notice",
          analysisRevisionId: "demo-revision",
          state: "pending",
          payload: {
            ...demoCandidatePayload,
            title: "提交报名材料",
            dueAt: null,
            dueExpression: null,
            status: "missing",
            confidence: 0.48,
            evidence: ["请提交报名材料"],
          },
          createdAt: new Date().toISOString(),
        },
      ];
      setCandidates(demoCandidates);
      setSelectedId(demoCandidates[0].id);
      setEditedTitle(demoCandidates[0].payload.title);
    }
  }

  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 0);
    return () => window.clearTimeout(timer);
    // Candidate list is loaded once when entering the review view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function selectCandidate(candidate: CandidateViewV1 | undefined) {
    setSelectedId(candidate?.id ?? null);
    setEditedTitle(candidate?.payload.title ?? "");
  }

  async function editCandidate() {
    if (!selected || !editedTitle.trim()) return;
    const payload = { ...selected.payload, title: editedTitle.trim() };
    try {
      await editTaskCandidate(selected.id, payload);
      setCandidates((current) =>
        current.map((item) => (item.id === selected.id ? { ...item, payload } : item)),
      );
      setMessage("候选已保存，原始通知和分析版本仍保留。");
    } catch {
      setMessage("候选修改未能保存。");
    }
  }

  async function confirmCandidate() {
    if (!selected) return;
    try {
      await confirmTaskCandidate(selected.id, {
        ...selected.payload,
        title: editedTitle.trim() || selected.payload.title,
      });
      const remaining = candidates.filter((item) => item.id !== selected.id);
      setCandidates(remaining);
      selectCandidate(remaining[0]);
      setMessage("已创建正式任务，并记录确认操作。");
    } catch {
      setMessage("确认失败，请先补全任务名称。");
    }
  }

  async function ignoreCandidate() {
    if (!selected) return;
    try {
      await ignoreTaskCandidate(selected.id);
      const remaining = candidates.filter((item) => item.id !== selected.id);
      setCandidates(remaining);
      selectCandidate(remaining[0]);
      setMessage("候选已忽略，不会重复提示。");
    } catch {
      setMessage("忽略操作未能保存。");
    }
  }

  async function mergeWithNext() {
    if (!selected || candidates.length < 2) return;
    const source = candidates.find((item) => item.id !== selected.id);
    if (!source) return;
    try {
      await mergeTaskCandidates(selected.id, [source.id], {
        ...selected.payload,
        title: `${selected.payload.title}（含${source.payload.title}）`,
      });
      setCandidates((current) => current.filter((item) => item.id !== source.id));
      setMessage("候选已合并，保留双方原文依据。");
    } catch {
      setMessage("合并失败，请确认候选仍处于待处理状态。");
    }
  }

  async function splitSelected() {
    if (!selected) return;
    try {
      await splitTaskCandidate(selected.id, [
        selected.payload,
        {
          ...selected.payload,
          title: `${selected.payload.title}（后续）`,
          dueAt: null,
          dueExpression: null,
        },
      ]);
      setCandidates((current) => current.filter((item) => item.id !== selected.id));
      setMessage("候选已拆分为两个独立候选。");
    } catch {
      setMessage("拆分失败。");
    }
  }

  async function suggestRelations() {
    if (!selected) return;
    try {
      const relations = await suggestNoticeRelations(selected.noticeId);
      setRelations(relations ?? []);
      setMessage(
        relations?.length
          ? `发现 ${relations.length} 条本地关联建议，请在通知详情中确认。`
          : "暂未发现可确认的重复或更新关系。",
      );
    } catch {
      setMessage("关联建议暂时不可用。");
    }
  }

  async function resolveRelation(relation: NoticeRelationViewV1, accepted: boolean) {
    try {
      await resolveNoticeRelation(relation.id, accepted);
      setRelations((current) =>
        current.map((item) =>
          item.id === relation.id
            ? { ...item, relationState: accepted ? "accepted" : "rejected" }
            : item,
        ),
      );
      setMessage(
        accepted
          ? relation.relationType === "cancel"
            ? "已确认取消，关联待办任务和未来提醒已在同一操作中停止。"
            : relation.relationType === "reschedule"
              ? "已确认改期，关联待办任务已生成新修订并重排未来提醒。"
              : "已接受关联建议，现有任务保持不变。"
          : "已拒绝关联建议，现有任务保持不变。",
      );
    } catch {
      setMessage("关联建议处理失败。");
    }
  }

  return (
    <section className="content-page" aria-labelledby="review-title">
      <div className="page-intro inline-intro">
        <div>
          <p className="section-eyebrow">{candidates.length} 项待处理</p>
          <h2 id="review-title">核对任务候选</h2>
        </div>
        {!isDesktopRuntime() ? <span className="demo-label">浏览器演示数据</span> : null}
      </div>
      <div className="review-table table-shell">
        <div className="table-header">
          <span>任务候选</span>
          <span>来源版本</span>
          <span>可信状态</span>
          <span>截止时间</span>
          <span />
        </div>
        {candidates.length ? (
          candidates.map((candidate) => (
            <button
              className={`table-row ${selected?.id === candidate.id ? "is-selected" : ""}`}
              key={candidate.id}
              onClick={() => selectCandidate(candidate)}
              type="button"
            >
              <strong>{candidate.payload.title}</strong>
              <span>
                {candidate.noticeId} · {candidate.analysisRevisionId}
              </span>
              <span
                className={`status-label ${candidate.payload.status === "trusted" ? "success" : "warning"}`}
              >
                {candidate.payload.status === "trusted"
                  ? "可信"
                  : candidate.payload.status === "missing"
                    ? "缺失待补"
                    : "需要核对"}
              </span>
              <time>
                {candidate.payload.dueAt
                  ? new Date(candidate.payload.dueAt).toLocaleString()
                  : "未确定"}
              </time>
              <ChevronRight size={18} />
            </button>
          ))
        ) : (
          <p className="empty-notice">没有待核对候选。导入并分析一条通知后会出现在这里。</p>
        )}
      </div>
      {selected ? (
        <div className="review-detail" aria-label="候选核对详情">
          <div className="detail-heading">
            <div>
              <span className="section-eyebrow">字段依据与操作记录</span>
              <h3>{selected.payload.title}</h3>
            </div>
            <button
              className="secondary-button"
              onClick={() => void suggestRelations()}
              type="button"
            >
              查找通知关联
            </button>
          </div>
          <label className="import-input">
            <span>任务名称</span>
            <input value={editedTitle} onChange={(event) => setEditedTitle(event.target.value)} />
          </label>
          <div className="review-field-grid">
            <div>
              <span>截止时间</span>
              <strong>
                {selected.payload.dueAt
                  ? new Date(selected.payload.dueAt).toLocaleString()
                  : "未确定"}
              </strong>
            </div>
            <div>
              <span>地点 / 入口</span>
              <strong>
                {selected.payload.location ?? selected.payload.submissionUrl ?? "未提取"}
              </strong>
            </div>
            <div>
              <span>适用对象</span>
              <strong>{selected.payload.audience ?? "未提取"}</strong>
            </div>
            <div>
              <span>是否必须</span>
              <strong>
                {selected.payload.required === null
                  ? "待确认"
                  : selected.payload.required
                    ? "必须完成"
                    : "自愿参与"}
              </strong>
            </div>
          </div>
          <div className="evidence-panel">
            <strong>原文依据</strong>
            {selected.payload.evidence.map((evidence) => (
              <p key={evidence}>{evidence}</p>
            ))}
          </div>
          {relations.length ? (
            <div className="relation-panel" aria-label="通知关联建议">
              <strong>通知关联建议</strong>
              {relations.map((relation) => {
                const evidence = relation.evidence as {
                  existingPayload?: Partial<TaskCandidatePayloadV1>;
                  proposedPayload?: Partial<TaskCandidatePayloadV1>;
                  reason?: string;
                };
                const relationLabel = {
                  duplicate: "可能重复",
                  supplement: "补充通知",
                  reschedule: "改期建议",
                  cancel: "取消建议",
                }[relation.relationType];
                return (
                  <div className="relation-row" key={relation.id}>
                    <strong>{relationLabel}</strong>
                    <small>
                      现有：{evidence.existingPayload?.title ?? "未提供"}
                      {evidence.existingPayload?.dueAt
                        ? `，${new Date(evidence.existingPayload.dueAt).toLocaleString()}`
                        : ""}
                    </small>
                    <small>
                      新通知：{evidence.proposedPayload?.title ?? "未提供"}
                      {evidence.proposedPayload?.dueAt
                        ? `，${new Date(evidence.proposedPayload.dueAt).toLocaleString()}`
                        : ""}
                    </small>
                    <span className="status-label neutral">{relation.relationState}</span>
                    {relation.relationState === "suggested" ? (
                      <>
                        <button
                          className="secondary-button"
                          onClick={() => void resolveRelation(relation, true)}
                          type="button"
                        >
                          接受
                        </button>
                        <button
                          className="secondary-button"
                          onClick={() => void resolveRelation(relation, false)}
                          type="button"
                        >
                          拒绝
                        </button>
                      </>
                    ) : null}
                  </div>
                );
              })}
            </div>
          ) : null}
          <div className="page-actions">
            <button className="secondary-button" onClick={() => void editCandidate()} type="button">
              保存修改
            </button>
            <button className="secondary-button" onClick={() => void mergeWithNext()} type="button">
              合并候选
            </button>
            <button className="secondary-button" onClick={() => void splitSelected()} type="button">
              拆分候选
            </button>
            <button
              className="secondary-button"
              onClick={() => void ignoreCandidate()}
              type="button"
            >
              忽略
            </button>
            <button
              className="primary-button"
              onClick={() => void confirmCandidate()}
              type="button"
            >
              <Check size={16} />
              确认创建任务
            </button>
          </div>
        </div>
      ) : null}
      {message ? <p className="form-message success">{message}</p> : null}
    </section>
  );
}

function TasksView() {
  const filters = ["今天", "本周", "即将截止", "无确定日期", "已完成", "全部任务"];
  const [filter, setFilter] = useState("本周");
  const [tasks, setTasks] = useState<TaskViewV1[]>([]);
  const [message, setMessage] = useState("");
  const [history, setHistory] = useState<TaskRevisionViewV1[] | null>(null);
  const [historyTask, setHistoryTask] = useState<TaskViewV1 | null>(null);
  const [sourceDetail, setSourceDetail] = useState<NoticeDetailV1 | null>(null);
  const [sourceImageUrl, setSourceImageUrl] = useState<string | null>(null);
  const [now] = useState(() => Date.now());

  useEffect(() => {
    return () => {
      if (sourceImageUrl) URL.revokeObjectURL(sourceImageUrl);
    };
  }, [sourceImageUrl]);

  async function refresh() {
    const next = await listTasks();
    if (next) setTasks(next);
    else if (!tasks.length)
      setTasks([
        {
          id: "demo-task-1",
          noticeId: "demo-notice",
          state: "todo",
          payload: demoTaskPayload,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          sourceRemovedAt: null,
        },
        {
          id: "demo-task-2",
          noticeId: null,
          state: "todo",
          payload: {
            ...demoTaskPayload,
            title: "确认秋季学期培养方案",
            dueAt: null,
            dueExpression: null,
            status: "missing",
          },
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          sourceRemovedAt: null,
        },
        {
          id: "demo-task-3",
          noticeId: "demo-notice",
          state: "completed",
          payload: { ...demoTaskPayload, title: "提交宿舍住宿确认" },
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          sourceRemovedAt: null,
        },
      ]);
  }
  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 0);
    return () => window.clearTimeout(timer);
    // The task loader intentionally runs once when entering this view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function visible(task: TaskViewV1) {
    if (filter === "全部任务") return true;
    if (filter === "已完成") return task.state === "completed";
    if (task.state !== "todo") return false;
    if (filter === "无确定日期") return !task.payload.dueAt;
    if (!task.payload.dueAt) return false;
    const due = new Date(task.payload.dueAt);
    const now = new Date();
    if (filter === "今天") return due.toDateString() === now.toDateString();
    if (filter === "本周") {
      const weekStart = new Date(now);
      const day = weekStart.getDay() || 7;
      weekStart.setHours(0, 0, 0, 0);
      weekStart.setDate(weekStart.getDate() - day + 1);
      const weekEnd = new Date(weekStart);
      weekEnd.setDate(weekEnd.getDate() + 7);
      return due >= weekStart && due < weekEnd;
    }
    return due.getTime() >= now.getTime() && due.getTime() - now.getTime() < 3 * 86400000;
  }

  async function toggle(task: TaskViewV1) {
    const state = task.state === "completed" ? "todo" : "completed";
    try {
      await setTaskState(task.id, state, task.payload);
      setTasks((current) =>
        current.map((item) => (item.id === task.id ? { ...item, state } : item)),
      );
      setMessage(state === "completed" ? "任务已完成，未来提醒将取消。" : "任务已重新打开。");
    } catch {
      setMessage("任务状态未能保存。");
    }
  }

  async function newTask() {
    try {
      const created = await createManualTask({
        ...demoTaskPayload,
        title: "新建个人任务",
        dueAt: null,
        dueExpression: null,
      });
      if (created) setTasks((current) => [created, ...current]);
      else
        setTasks((current) => [
          {
            id: `demo-${Date.now()}`,
            noticeId: null,
            state: "todo",
            payload: {
              ...demoTaskPayload,
              title: "新建个人任务",
              dueAt: null,
              dueExpression: null,
            },
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            sourceRemovedAt: null,
          },
          ...current,
        ]);
      setMessage("已新增任务，可从详情继续编辑。");
    } catch {
      setMessage("新建任务失败。");
    }
  }

  async function showHistory(task: TaskViewV1) {
    try {
      const revisions = await getTaskHistory(task.id);
      setHistory(revisions);
      setHistoryTask(task);
      setSourceDetail(null);
      setSourceImageUrl((current) => {
        if (current) URL.revokeObjectURL(current);
        return null;
      });
      setMessage(
        revisions ? `已加载 ${revisions.length} 个修订版本。` : "浏览器演示数据暂不连接历史记录。",
      );
    } catch {
      setMessage("无法读取任务历史。");
    }
  }

  async function showSource(task: TaskViewV1) {
    if (!task.noticeId || task.sourceRemovedAt) {
      setMessage("该任务的原始通知已删除，无法再查看来源。历史修订仍保留在本机。");
      return;
    }
    try {
      const detail = await getNoticeDetail(task.noticeId);
      setSourceDetail(detail);
      const preview = detail?.sourceAsset ? await getNoticeImagePreview(task.noticeId) : null;
      setSourceImageUrl((current) => {
        if (current) URL.revokeObjectURL(current);
        return preview
          ? URL.createObjectURL(
              new Blob([new Uint8Array(preview.bytes)], { type: preview.mediaType }),
            )
          : null;
      });
      setMessage(detail ? "已加载原始通知依据。" : "原始通知已不可访问。");
    } catch {
      setMessage("无法读取原始通知依据。");
    }
  }

  async function editTaskTitle(task: TaskViewV1) {
    const nextTitle = window.prompt("修改任务名称", task.payload.title)?.trim();
    if (!nextTitle || nextTitle === task.payload.title) return;
    const payload = { ...task.payload, title: nextTitle };
    try {
      await updateTask(task.id, payload);
      setTasks((current) =>
        current.map((item) =>
          item.id === task.id ? { ...item, payload, updatedAt: new Date().toISOString() } : item,
        ),
      );
      setMessage("任务名称已更新，并保留新的修订记录。");
    } catch {
      setMessage("任务修改未能保存。");
    }
  }

  async function cancelTask(task: TaskViewV1) {
    if (task.state === "cancelled") return;
    try {
      await setTaskState(task.id, "cancelled", task.payload);
      setTasks((current) =>
        current.map((item) => (item.id === task.id ? { ...item, state: "cancelled" } : item)),
      );
      setMessage("任务已取消，未来提醒将停止。");
    } catch {
      setMessage("任务取消未能保存。");
    }
  }

  async function manageReminder(task: TaskViewV1) {
    try {
      const current = await listReminders(task.id);
      if (current?.length) {
        await deleteReminder(current[0].id);
        setMessage("已删除一条任务提醒。");
        return;
      }
      const raw = window.prompt("输入提醒时间（例如 2026-08-27T18:30）");
      if (!raw) return;
      const scheduledAt = new Date(raw).toISOString();
      await upsertReminder(task.id, scheduledAt, `${task.id}:${scheduledAt}`);
      setMessage("提醒已保存到本机。");
    } catch {
      setMessage("提醒未能保存，请确认时间和任务状态。");
    }
  }

  return (
    <section className="content-page" aria-labelledby="tasks-title">
      <div className="page-intro inline-intro">
        <div>
          <p className="section-eyebrow">{filter}</p>
          <h2 id="tasks-title">我的任务</h2>
        </div>
        <button className="primary-button" onClick={() => void newTask()} type="button">
          <Plus size={17} /> 新建任务
        </button>
      </div>
      <div className="task-layout">
        <aside className="task-filters" aria-label="任务筛选">
          {filters.map((label) => (
            <button
              className={filter === label ? "is-active" : ""}
              key={label}
              onClick={() => setFilter(label)}
              type="button"
            >
              <span>{label}</span>
              <span>
                {
                  tasks.filter(
                    (task) =>
                      label === "全部任务" ||
                      (label === "已完成"
                        ? task.state === "completed"
                        : label === "无确定日期"
                          ? task.state === "todo" && !task.payload.dueAt
                          : true),
                  ).length
                }
              </span>
            </button>
          ))}
        </aside>
        <div className="task-list">
          {historyTask ? (
            <section className="task-history-panel" aria-label="任务修订与来源">
              <div className="page-intro inline-intro">
                <div>
                  <p className="section-eyebrow">{historyTask.payload.title}</p>
                  <h3>修订与依据</h3>
                </div>
                <button
                  className="icon-button"
                  aria-label="关闭修订记录"
                  onClick={() => {
                    setHistoryTask(null);
                    setHistory(null);
                    setSourceDetail(null);
                    setSourceImageUrl((current) => {
                      if (current) URL.revokeObjectURL(current);
                      return null;
                    });
                  }}
                  type="button"
                >
                  <X size={16} />
                </button>
              </div>
              {history?.map((revision) => (
                <div className="task-history-row" key={revision.id}>
                  <strong>版本 {revision.revisionNumber}</strong>
                  <span>{new Date(revision.createdAt).toLocaleString()}</span>
                  <span>
                    {revision.analysisRevisionId
                      ? `分析版本 ${revision.analysisRevisionId.slice(0, 8)}`
                      : "用户手工修订"}
                  </span>
                  <p>{revision.payload.title}</p>
                  {revision.payload.evidence?.length ? (
                    <small>依据：{revision.payload.evidence.join("；")}</small>
                  ) : (
                    <small>无自动依据</small>
                  )}
                </div>
              ))}
              <button
                className="secondary-button"
                onClick={() => void showSource(historyTask)}
                type="button"
              >
                查看原始通知
              </button>
              {sourceDetail ? (
                <div className="task-source-detail">
                  <strong>原始通知</strong>
                  {sourceDetail.originalText ? <p>{sourceDetail.originalText}</p> : null}
                  {sourceImageUrl ? (
                    <img alt="任务来源截图" className="task-source-image" src={sourceImageUrl} />
                  ) : null}
                  {!sourceDetail.originalText && !sourceImageUrl ? <p>该来源已无法读取。</p> : null}
                </div>
              ) : null}
            </section>
          ) : null}
          {tasks
            .filter(visible)
            .sort((left, right) => {
              if (!left.payload.dueAt) return 1;
              if (!right.payload.dueAt) return -1;
              return (
                new Date(left.payload.dueAt).getTime() - new Date(right.payload.dueAt).getTime()
              );
            })
            .map((task) => {
              const expired =
                task.state === "todo" &&
                task.payload.dueAt &&
                new Date(task.payload.dueAt).getTime() < now;
              return (
                <article
                  className={`task-row ${expired ? "urgent" : ""} ${task.state === "completed" ? "complete" : ""}`}
                  key={task.id}
                >
                  <button
                    className={`task-check ${task.state === "completed" ? "checked" : ""}`}
                    aria-label={task.state === "completed" ? "重新打开任务" : "标记完成"}
                    onClick={() => void toggle(task)}
                    type="button"
                  >
                    {task.state === "completed" ? <Check size={14} /> : null}
                  </button>
                  <div>
                    <h3>{task.payload.title}</h3>
                    <p>
                      {task.payload.dueAt ? (
                        <>
                          <Clock3 size={15} /> {new Date(task.payload.dueAt).toLocaleString()} 截止
                        </>
                      ) : (
                        <>
                          <CalendarDays size={15} /> 日期待确认
                        </>
                      )}
                    </p>
                    {task.sourceRemovedAt ? <p className="source-removed">原文依据已删除</p> : null}
                  </div>
                  <div className="task-actions">
                    <button
                      className="status-label neutral"
                      onClick={() => void showHistory(task)}
                      type="button"
                    >
                      {task.state === "completed" ? "已完成" : expired ? "已过期" : "查看历史"}
                    </button>
                    {task.state === "todo" ? (
                      <>
                        <button
                          className="status-label neutral"
                          onClick={() => void editTaskTitle(task)}
                          type="button"
                        >
                          编辑
                        </button>
                        <button
                          className="status-label neutral"
                          onClick={() => void cancelTask(task)}
                          type="button"
                        >
                          取消
                        </button>
                        <button
                          className="status-label neutral"
                          onClick={() => void manageReminder(task)}
                          type="button"
                        >
                          提醒
                        </button>
                      </>
                    ) : null}
                  </div>
                </article>
              );
            })}
          {!tasks.filter(visible).length ? (
            <p className="empty-notice">当前视图没有任务。</p>
          ) : null}
        </div>
      </div>
      {message ? <p className="form-message success">{message}</p> : null}
    </section>
  );
}

function SettingsView() {
  const [securityStatus, setSecurityStatus] = useState<"idle" | "checking" | "verified" | "failed">(
    "idle",
  );
  const [remindersEnabled, setRemindersEnabled] = useState(
    () => localStorage.getItem("remindersEnabled") !== "false",
  );
  const [launchAtLogin, setLaunchAtLogin] = useState(
    () => localStorage.getItem("launchAtLogin") === "true",
  );
  const [backupPath, setBackupPath] = useState("");
  const [backupPassword, setBackupPassword] = useState("");
  const [restorePath, setRestorePath] = useState("");
  const [restorePassword, setRestorePassword] = useState("");
  const [backupPreview, setBackupPreview] = useState("");
  const [backupConfirmed, setBackupConfirmed] = useState(false);
  const [backupMessage, setBackupMessage] = useState("");

  function toggleReminders(enabled: boolean) {
    setRemindersEnabled(enabled);
    localStorage.setItem("remindersEnabled", String(enabled));
  }

  function toggleLaunchAtLogin(enabled: boolean) {
    setLaunchAtLogin(enabled);
    localStorage.setItem("launchAtLogin", String(enabled));
  }

  async function runSecurityCheck() {
    if (!isDesktopRuntime()) return;
    setSecurityStatus("checking");
    try {
      await getSecurityStatus();
      setSecurityStatus("verified");
    } catch {
      setSecurityStatus("failed");
    }
  }

  async function createLocalBackup() {
    if (!backupPath.trim() || backupPassword.length < 8) {
      setBackupMessage("请输入本地备份文件路径和至少 8 位的口令。");
      return;
    }
    try {
      const summary = await createBackup(backupPath.trim(), backupPassword);
      setBackupMessage(
        summary
          ? `备份已创建，包含 ${summary.noticeCount} 条通知和 ${summary.taskCount} 个任务。`
          : "浏览器演示模式不创建文件。",
      );
      setBackupPassword("");
    } catch {
      setBackupMessage("备份未创建。请确认路径可写、文件不存在且口令有效。");
    }
  }

  async function inspectLocalBackup() {
    if (!restorePath.trim() || !restorePassword) {
      setBackupMessage("请输入备份路径和口令后再预检。");
      return;
    }
    try {
      const summary = await inspectBackup(restorePath.trim(), restorePassword);
      if (!summary) return;
      setBackupPreview(
        `该备份包含 ${summary.noticeCount} 条通知、${summary.taskCount} 个任务和 ${summary.attachmentCount} 个附件。`,
      );
      setBackupConfirmed(false);
      setBackupMessage("预检完成。恢复会替换当前本地数据。");
    } catch {
      setBackupPreview("");
      setBackupMessage("备份无效或口令不正确；当前数据没有改动。");
    }
  }

  async function restoreLocalBackup() {
    if (!backupConfirmed) return;
    try {
      await restoreBackup(restorePath.trim(), restorePassword);
      setBackupMessage("恢复完成。请返回收件箱核对已恢复的数据。");
      setRestorePassword("");
      setBackupConfirmed(false);
    } catch {
      setBackupMessage("恢复未完成。当前数据仍保持不变。");
    }
  }

  return (
    <section className="content-page settings-page" aria-labelledby="settings-title">
      <div className="page-intro">
        <p className="section-eyebrow">应用偏好</p>
        <h2 id="settings-title">设置</h2>
      </div>
      <div className="settings-sections">
        <section>
          <div className="settings-icon">
            <MonitorCog size={20} />
          </div>
          <div className="setting-copy">
            <h3>窗口与后台运行</h3>
            <p>关闭主窗口后，校园信箱继续在系统托盘运行并按时提醒。</p>
          </div>
          <label className="switch">
            <input
              type="checkbox"
              checked={launchAtLogin}
              onChange={(event) => toggleLaunchAtLogin(event.target.checked)}
            />
            <span />
          </label>
        </section>
        <section>
          <div className="settings-icon">
            <Bell size={20} />
          </div>
          <div className="setting-copy">
            <h3>本地提醒</h3>
            <p>任务提醒通过 Windows 通知和应用内通知中心显示。</p>
          </div>
          <label className="switch">
            <input
              type="checkbox"
              checked={remindersEnabled}
              onChange={(event) => toggleReminders(event.target.checked)}
            />
            <span />
          </label>
        </section>
        <section>
          <div className="settings-icon">
            <ShieldCheck size={20} />
          </div>
          <div className="setting-copy">
            <h3>本地数据</h3>
            <p>通知、截图和任务保存在当前设备，不连接云端账户。</p>
          </div>
          <button className="secondary-button" type="button">
            查看存储位置
          </button>
        </section>
        <section className="security-check">
          <div className="settings-icon">
            <ShieldCheck size={20} />
          </div>
          <div className="setting-copy">
            <h3>本地安全检查</h3>
            <p>
              {securityStatus === "verified"
                ? "已确认：当前用户保护、SQLCipher、附件加密和业务断网边界均可用。"
                : securityStatus === "failed"
                  ? "检查未完成。请确认应用数据目录可写后再试。"
                  : "主密钥受当前 Windows 用户保护；通知数据不连接云端。"}
            </p>
          </div>
          <button
            className="secondary-button"
            disabled={!isDesktopRuntime() || securityStatus === "checking"}
            onClick={() => void runSecurityCheck()}
            type="button"
          >
            {securityStatus === "checking" ? "检查中" : "运行检查"}
          </button>
        </section>
      </div>
      <p className="form-message">
        {remindersEnabled
          ? "提醒权限：已启用；若 Windows 拒绝系统通知，应用内通知中心仍会保留记录。"
          : "提醒已关闭，后台不会产生新的提醒通知。"}
      </p>
      <section className="backup-panel" aria-labelledby="backup-title">
        <div className="page-intro">
          <p className="section-eyebrow">本地数据保护</p>
          <h2 id="backup-title">备份与恢复</h2>
          <p>
            备份文件由你设置的口令保护，不会上传到网络。请将备份保存到本机或自己控制的移动存储设备。
          </p>
        </div>
        <div className="backup-grid">
          <div>
            <h3>创建加密备份</h3>
            <label>
              备份文件路径
              <input
                value={backupPath}
                onChange={(event) => setBackupPath(event.target.value)}
                placeholder="D:\\校园信箱备份.xinxiang"
              />
            </label>
            <label>
              备份口令
              <input
                value={backupPassword}
                onChange={(event) => setBackupPassword(event.target.value)}
                type="password"
              />
            </label>
            <button
              className="secondary-button"
              onClick={() => void createLocalBackup()}
              type="button"
            >
              创建备份
            </button>
          </div>
          <div>
            <h3>恢复本地备份</h3>
            <label>
              备份文件路径
              <input value={restorePath} onChange={(event) => setRestorePath(event.target.value)} />
            </label>
            <label>
              备份口令
              <input
                value={restorePassword}
                onChange={(event) => setRestorePassword(event.target.value)}
                type="password"
              />
            </label>
            <div className="backup-actions">
              <button
                className="secondary-button"
                onClick={() => void inspectLocalBackup()}
                type="button"
              >
                预检备份
              </button>
            </div>
            {backupPreview ? <p className="backup-preview">{backupPreview}</p> : null}
            <label className="confirm-check">
              <input
                checked={backupConfirmed}
                disabled={!backupPreview}
                onChange={(event) => setBackupConfirmed(event.target.checked)}
                type="checkbox"
              />
              我理解恢复将替换当前本地数据
            </label>
            <button
              className="danger-button"
              disabled={!backupConfirmed}
              onClick={() => void restoreLocalBackup()}
              type="button"
            >
              确认恢复
            </button>
          </div>
        </div>
        <p className="form-message">
          {backupMessage ||
            "积累通知和任务后，请尽早创建一份口令保护的本地备份。卸载应用时，请选择保留加密数据；选择永久删除数据后无法恢复。"}
        </p>
      </section>
      <div className="quit-panel">
        <div>
          <strong>彻底退出应用</strong>
          <p>退出后，任务提醒将在下次启动前暂停。</p>
        </div>
        <button className="danger-button" onClick={() => void quitDesktopApp()} type="button">
          退出校园信箱
        </button>
      </div>
    </section>
  );
}

function ActiveView({
  route,
  imported,
  openImport,
}: {
  route: AppRouteId;
  imported: () => void;
  openImport: () => void;
}) {
  if (route === "quickImport") return <QuickImportView imported={imported} />;
  if (route === "review") return <ReviewView />;
  if (route === "tasks") return <TasksView />;
  if (route === "settings") return <SettingsView />;
  return <InboxView openImport={openImport} />;
}

export function App() {
  const [activeRoute, setActiveRoute] = useState<AppRouteId>("inbox");
  const [notifications, setNotifications] = useState<NotificationEventV1[]>([]);
  const [notificationsOpen, setNotificationsOpen] = useState(false);

  useEffect(() => {
    if (!isDesktopRuntime()) return;

    let dispose: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const stopQuickImport = await listen("quickImportRequested", () =>
        setActiveRoute("quickImport"),
      );
      const stopReminder = await listen<NotificationEventV1>("reminderTriggered", (event) => {
        if (localStorage.getItem("remindersEnabled") === "false") return;
        setNotifications((current) => [event.payload, ...current].slice(0, 50));
      });
      dispose = () => {
        stopQuickImport();
        stopReminder();
      };
    });

    return () => dispose?.();
  }, []);
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">
            <Inbox size={21} />
          </span>
          <div>
            <strong>校园信箱</strong>
            <span>本地通知助理</span>
          </div>
        </div>
        <nav aria-label="主导航">
          {navItems.map(({ id, label, icon: Icon, badge }) => (
            <button
              className={activeRoute === id ? "is-active" : ""}
              key={id}
              onClick={() => setActiveRoute(id)}
              type="button"
            >
              <Icon size={19} strokeWidth={1.9} />
              <span>{label}</span>
              {badge ? <span className="nav-badge">{badge}</span> : null}
            </button>
          ))}
        </nav>
        <div className="sidebar-status">
          <ShieldCheck size={17} />
          <div>
            <strong>设备内处理</strong>
            <span>通知数据未离开本机</span>
          </div>
        </div>
      </aside>
      <main className="main-area">
        <AppHeader
          activeRoute={activeRoute}
          onNotifications={() => setNotificationsOpen((open) => !open)}
          notificationCount={notifications.length}
        />
        {notificationsOpen ? (
          <aside className="notification-center" aria-label="通知中心">
            <div className="notification-center-header">
              <strong>本地提醒</strong>
              <button
                className="icon-button"
                type="button"
                aria-label="关闭通知中心"
                title="关闭"
                onClick={() => setNotificationsOpen(false)}
              >
                <X size={16} />
              </button>
            </div>
            {notifications.length ? (
              notifications.map((item) => (
                <button
                  className="notification-item"
                  type="button"
                  key={`${item.reminderId}-${item.scheduledAt}`}
                  onClick={() => {
                    setActiveRoute("tasks");
                    setNotificationsOpen(false);
                  }}
                >
                  <Bell size={15} />
                  <span>有 {item.missedCount} 条任务提醒需要查看</span>
                </button>
              ))
            ) : (
              <p className="empty-notice">暂无提醒</p>
            )}
            {notifications.length ? (
              <button
                className="secondary-button"
                type="button"
                onClick={() => setNotifications([])}
              >
                清空已读提醒
              </button>
            ) : null}
          </aside>
        ) : null}
        <ActiveView
          route={activeRoute}
          imported={() => setActiveRoute("inbox")}
          openImport={() => setActiveRoute("quickImport")}
        />
      </main>
    </div>
  );
}
