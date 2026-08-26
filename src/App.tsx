import {
  Archive,
  Bell,
  CalendarDays,
  Check,
  CheckCircle2,
  ChevronRight,
  ClipboardPaste,
  Clock3,
  FileCheck2,
  Inbox,
  Info,
  ListChecks,
  MapPin,
  MonitorCog,
  Paperclip,
  Plus,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Upload,
} from "lucide-react";
import { useEffect, useState, type ComponentType } from "react";

import type { AppRouteId } from "./contracts/v1";
import { isDesktopRuntime, quitDesktopApp } from "./platform/desktop";

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

const notices = [
  {
    id: 1,
    title: "实验室安全考试通知",
    excerpt: "请各位同学于本周五 17:00 前完成实验室安全准入考试。",
    time: "今天 09:42",
    status: "待确认",
    tone: "warning",
  },
  {
    id: 2,
    title: "2026 秋季学期选课安排",
    excerpt: "第一轮选课将于 8 月 28 日开放，请提前确认培养方案。",
    time: "昨天 18:20",
    status: "待分析",
    tone: "neutral",
  },
  {
    id: 3,
    title: "图书馆暑期开放时间调整",
    excerpt: "8 月 30 日起恢复正常开放时间。",
    time: "8 月 24 日",
    status: "仅供知晓",
    tone: "success",
  },
];

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

function InboxView() {
  const [selectedId, setSelectedId] = useState(1);
  const selectedNotice = notices.find((notice) => notice.id === selectedId) ?? notices[0];
  return (
    <div className="inbox-view">
      <section className="notice-column" aria-label="通知列表">
        <div className="section-toolbar">
          <div className="segmented-control" aria-label="通知筛选">
            <button className="is-active" type="button">
              全部
            </button>
            <button type="button">待处理</button>
            <button type="button">已完成</button>
          </div>
          <button className="icon-button subtle" type="button" title="归档" aria-label="归档">
            <Archive size={18} />
          </button>
        </div>
        <div className="notice-list">
          {notices.map((notice) => (
            <button
              className={`notice-row ${selectedId === notice.id ? "is-selected" : ""}`}
              key={notice.id}
              onClick={() => setSelectedId(notice.id)}
              type="button"
            >
              <span className={`status-marker ${notice.tone}`} />
              <span className="notice-copy">
                <span className="notice-row-topline">
                  <strong>{notice.title}</strong>
                  <time>{notice.time}</time>
                </span>
                <span className="notice-excerpt">{notice.excerpt}</span>
                <span className={`status-label ${notice.tone}`}>{notice.status}</span>
              </span>
            </button>
          ))}
        </div>
      </section>
      <section className="notice-detail" aria-label="通知详情">
        <div className="detail-heading">
          <div>
            <span className="demo-label">演示数据</span>
            <h2>{selectedNotice.title}</h2>
            <p>来自文字粘贴 · {selectedNotice.time}</p>
          </div>
          <button className="secondary-button" type="button">
            查看原文 <ChevronRight size={16} />
          </button>
        </div>
        <div className="analysis-state">
          <Sparkles size={19} />
          <div>
            <strong>本地分析完成</strong>
            <span>发现 1 个任务候选，截止时间需要确认</span>
          </div>
        </div>
        <div className="candidate-section">
          <div className="section-title-row">
            <div>
              <p className="section-eyebrow">任务候选 1/1</p>
              <h3>完成实验室安全准入考试</h3>
            </div>
            <span className="confidence-label">
              <Info size={14} /> 待核对
            </span>
          </div>
          <dl className="field-list">
            <div>
              <dt>
                <Clock3 size={17} /> 截止时间
              </dt>
              <dd>8 月 28 日 17:00</dd>
            </div>
            <div>
              <dt>
                <MapPin size={17} /> 地点 / 入口
              </dt>
              <dd>实验室安全学习平台</dd>
            </div>
            <div>
              <dt>
                <Paperclip size={17} /> 所需材料
              </dt>
              <dd>校园统一身份认证</dd>
            </div>
          </dl>
          <div className="evidence-block">
            <span>原文依据</span>
            <blockquote>“请各位同学于本周五 17:00 前完成实验室安全准入考试。”</blockquote>
          </div>
        </div>
        <footer className="detail-actions">
          <button className="secondary-button" type="button">
            暂不处理
          </button>
          <button className="primary-button" type="button">
            <Check size={17} /> 前往核对
          </button>
        </footer>
      </section>
    </div>
  );
}

function QuickImportView() {
  const [mode, setMode] = useState<"text" | "image">("text");
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
            <textarea placeholder="在此粘贴通知文字" rows={10} />
          </label>
        ) : (
          <button className="upload-zone" type="button">
            <Upload size={28} />
            <strong>选择通知截图</strong>
            <span>支持 PNG、JPG 和 WebP</span>
          </button>
        )}
        <div className="import-options">
          <label>
            <span>通知发布时间</span>
            <input type="datetime-local" />
          </label>
          <span className="privacy-note">
            <ShieldCheck size={16} /> 本地处理
          </span>
        </div>
        <div className="page-actions">
          <button className="secondary-button" type="button">
            清空
          </button>
          <button className="primary-button" type="button">
            <Plus size={17} /> 创建通知
          </button>
        </div>
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

function ActiveView({ route }: { route: AppRouteId }) {
  if (route === "quickImport") return <QuickImportView />;
  if (route === "review") return <ReviewView />;
  if (route === "tasks") return <TasksView />;
  if (route === "settings") return <SettingsView />;
  return <InboxView />;
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
        <ActiveView route={activeRoute} />
      </main>
    </div>
  );
}
