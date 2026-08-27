import {
  Bell,
  CalendarDays,
  Check,
  CheckCircle2,
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
  AppRouteId,
  NoticeDetailV1,
  NoticeState,
  NoticeSummaryV1,
} from "./contracts/v1";
import {
  getNoticeDetail,
  getNoticeImagePreview,
  getSecurityStatus,
  analyzeNotice,
  cancelAnalysis,
  importImageNotice,
  importTextNotice,
  isDesktopRuntime,
  listNotices,
  quitDesktopApp,
  setNoticeState,
  updateNoticePublishedTime,
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

function AppHeader({ activeRoute }: { activeRoute: AppRouteId }) {
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
        <button className="icon-button" type="button" title="通知中心" aria-label="通知中心">
          <Bell size={19} />
          <span className="notification-dot" />
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

  async function markInformationOnly() {
    if (!detail) return;
    try {
      await setNoticeState(detail.id, "informationOnly");
      await refresh();
    } catch {
      setError("通知状态未能更新。");
    }
  }

  async function runAnalysis() {
    if (!detail) return;
    setAnalysisState("running");
    setError("");
    try {
      const result = await analyzeNotice(detail.id);
      setAnalysis(result);
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
              onClick={() => setSelectedId(notice.id)}
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

function ReviewView() {
  return (
    <section className="content-page" aria-labelledby="review-title">
      <div className="page-intro inline-intro">
        <div>
          <p className="section-eyebrow">2 项待处理</p>
          <h2 id="review-title">核对任务候选</h2>
        </div>
        <span className="demo-label">演示数据</span>
      </div>
      <div className="review-table table-shell">
        <div className="table-header">
          <span>任务候选</span>
          <span>来源</span>
          <span>可信状态</span>
          <span>截止时间</span>
          <span />
        </div>
        <div className="table-row">
          <strong>完成实验室安全准入考试</strong>
          <span>实验室安全考试通知</span>
          <span className="status-label warning">时间待核对</span>
          <time>8 月 28 日 17:00</time>
          <button className="icon-button subtle" aria-label="打开任务候选" title="打开任务候选">
            <ChevronRight size={18} />
          </button>
        </div>
        <div className="table-row">
          <strong>确认秋季学期培养方案</strong>
          <span>2026 秋季学期选课安排</span>
          <span className="status-label neutral">缺少截止时间</span>
          <time>未确定</time>
          <button className="icon-button subtle" aria-label="打开任务候选" title="打开任务候选">
            <ChevronRight size={18} />
          </button>
        </div>
      </div>
    </section>
  );
}

function TasksView() {
  const filters = [
    ["今天", "2"],
    ["本周", "5"],
    ["即将截止", "3"],
    ["无确定日期", "1"],
    ["已完成", "12"],
    ["全部任务", "18"],
  ];
  return (
    <section className="content-page" aria-labelledby="tasks-title">
      <div className="page-intro inline-intro">
        <div>
          <p className="section-eyebrow">本周</p>
          <h2 id="tasks-title">我的任务</h2>
        </div>
        <button className="primary-button" type="button">
          <Plus size={17} /> 新建任务
        </button>
      </div>
      <div className="task-layout">
        <aside className="task-filters" aria-label="任务筛选">
          {filters.map(([label, count], index) => (
            <button className={index === 1 ? "is-active" : ""} key={label} type="button">
              <span>{label}</span>
              <span>{count}</span>
            </button>
          ))}
        </aside>
        <div className="task-list">
          <article className="task-row urgent">
            <button className="task-check" aria-label="标记完成" type="button" />
            <div>
              <h3>完成实验室安全准入考试</h3>
              <p>
                <Clock3 size={15} /> 周五 17:00 截止
              </p>
            </div>
            <span className="status-label warning">2 天后</span>
          </article>
          <article className="task-row">
            <button className="task-check" aria-label="标记完成" type="button" />
            <div>
              <h3>确认秋季学期培养方案</h3>
              <p>
                <CalendarDays size={15} /> 日期待确认
              </p>
            </div>
            <span className="status-label neutral">待安排</span>
          </article>
          <article className="task-row complete">
            <button className="task-check checked" aria-label="重新打开任务" type="button">
              <Check size={14} />
            </button>
            <div>
              <h3>提交宿舍住宿确认</h3>
              <p>
                <CheckCircle2 size={15} /> 今天 08:20 完成
              </p>
            </div>
            <span className="status-label success">已完成</span>
          </article>
        </div>
      </div>
    </section>
  );
}

function SettingsView() {
  const [securityStatus, setSecurityStatus] = useState<"idle" | "checking" | "verified" | "failed">(
    "idle",
  );

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
            <input type="checkbox" defaultChecked />
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
            <input type="checkbox" defaultChecked />
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

  useEffect(() => {
    if (!isDesktopRuntime()) return;

    let dispose: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen("quickImportRequested", () => setActiveRoute("quickImport")).then((unlisten) => {
        dispose = unlisten;
      }),
    );

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
        <AppHeader activeRoute={activeRoute} />
        <ActiveView
          route={activeRoute}
          imported={() => setActiveRoute("inbox")}
          openImport={() => setActiveRoute("quickImport")}
        />
      </main>
    </div>
  );
}
