import { useEffect, useState } from "react";
import {
  type AppSettings,
  type CookieFileStatus,
  type YoutubeCookieMode,
  chooseYoutubeCookieFile,
  chooseDefaultDownloadDirectory,
  clearYoutubeCookieFile,
  getAppSettings,
  setYoutubeCookieMode,
  resetDefaultDownloadDirectory,
} from "../../lib/velo-api";
import {
  DOWNLOAD_HISTORY_UPDATED_EVENT,
  clearDownloadHistory,
  readDownloadHistory,
  type DownloadHistoryItem,
} from "../history/download-history";

interface SettingsViewProps {
  onBack: () => void;
  initialSection: SettingsSection;
  onSectionChange: (section: SettingsSection) => void;
}

export type SettingsSection = "storage" | "access" | "history";

const statusCopy: Record<CookieFileStatus, string> = {
  notConfigured: "尚未配置",
  ready: "文件可用",
  missing: "文件已移动或删除",
  invalid: "文件格式无效",
};

const settingsSections = [
  { id: "storage", label: "保存位置" },
  { id: "access", label: "网站访问" },
  { id: "history", label: "历史记录" },
] as const;

function fileName(path: string | null) {
  if (!path) return "未选择 Cookie 文件";
  return path.split(/[\\/]/).pop() || path;
}

export function SettingsView({ onBack, initialSection, onSectionChange }: SettingsViewProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>(initialSection);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [history, setHistory] = useState<DownloadHistoryItem[]>(() => readDownloadHistory());
  const [confirmClear, setConfirmClear] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getAppSettings()
      .then((value) => {
        if (active) setSettings(value);
      })
      .catch(() => {
        if (active) setError("无法读取应用设置。");
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const refresh = () => setHistory(readDownloadHistory());
    globalThis.addEventListener(DOWNLOAD_HISTORY_UPDATED_EVENT, refresh);
    return () => globalThis.removeEventListener(DOWNLOAD_HISTORY_UPDATED_EVENT, refresh);
  }, []);

  async function update(action: () => Promise<AppSettings | null>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const next = await action();
      if (next) setSettings(next);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "无法保存设置。");
    } finally {
      setBusy(false);
    }
  }

  const youtube = settings?.sites.youtube;

  return (
    <section className="settings-view" aria-labelledby="settings-title">
      <div className="settings-heading">
        <button className="settings-back" type="button" onClick={onBack}>
          <span aria-hidden="true">←</span>
          返回
        </button>
        <div>
          <h1 id="settings-title">设置</h1>
          <p>管理保存位置、网站访问与本地记录。</p>
        </div>
      </div>

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类">
          {settingsSections.map((section) => (
            <button
              className={activeSection === section.id ? "is-active" : undefined}
              type="button"
              aria-current={activeSection === section.id ? "page" : undefined}
              key={section.id}
              onClick={() => {
                setActiveSection(section.id);
                onSectionChange(section.id);
                setConfirmClear(false);
              }}
            >
              {section.label}
            </button>
          ))}
          <span>设置与记录仅保存在这台设备</span>
        </nav>

        <div className="settings-content">
          {activeSection === "access" ? (
            <>
              <header className="settings-section-heading">
                <div>
                  <h2>网站访问</h2>
                  <p>管理 YouTube 等网站的登录与 Cookie。</p>
                </div>
              </header>

              <section className="site-settings" aria-labelledby="youtube-settings-title">
            <div className="site-settings-title">
              <div className="site-monogram" aria-hidden="true">YT</div>
              <div>
                <h3 id="youtube-settings-title">YouTube</h3>
                <p>用于需要登录、年龄验证或会员权限的内容。</p>
              </div>
            </div>

            <label className="settings-field" htmlFor="youtube-cookie-mode">
              <span>
                <strong>Cookie 使用方式</strong>
                <small>“按需使用”会先匿名解析，仅在身份验证失败后重试。</small>
              </span>
              <select
                id="youtube-cookie-mode"
                value={youtube?.cookieMode ?? "onDemand"}
                disabled={!settings || busy}
                onChange={(event) =>
                  void update(() =>
                    setYoutubeCookieMode(event.target.value as YoutubeCookieMode),
                  )
                }
              >
                <option value="onDemand">按需使用（推荐）</option>
                <option value="always">始终使用</option>
                <option value="disabled">停用</option>
              </select>
            </label>

            <div className="settings-field cookie-file-field">
              <span>
                <strong>Cookie 文件</strong>
                <small className="cookie-file-name" title={youtube?.cookieFilePath ?? undefined}>
                  {fileName(youtube?.cookieFilePath ?? null)}
                </small>
              </span>
              <div className="cookie-file-actions">
                <span className={`cookie-file-status is-${youtube?.cookieFileStatus ?? "notConfigured"}`}>
                  {statusCopy[youtube?.cookieFileStatus ?? "notConfigured"]}
                </span>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void update(chooseYoutubeCookieFile)}
                >
                  {youtube?.cookieFilePath ? "更换" : "选择文件"}
                </button>
                {youtube?.cookieFilePath && (
                  <button
                    className="is-secondary"
                    type="button"
                    disabled={busy}
                    onClick={() => void update(clearYoutubeCookieFile)}
                  >
                    清除
                  </button>
                )}
              </div>
            </div>

            <p className="settings-security-note">
              Velo 只保存文件路径，不复制 Cookie 内容；该文件不会用于其他站点。
            </p>
            {error && <p className="settings-error" role="alert">{error}</p>}
              </section>
            </>
          ) : activeSection === "storage" ? (
            <DownloadSettings
              settings={settings}
              busy={busy}
              error={error}
              onChoose={() => void update(chooseDefaultDownloadDirectory)}
              onReset={() => void update(resetDefaultDownloadDirectory)}
            />
          ) : (
            <HistorySettings
              history={history}
              confirmClear={confirmClear}
              onRequestClear={() => setConfirmClear(true)}
              onCancelClear={() => setConfirmClear(false)}
              onClear={() => {
                clearDownloadHistory();
                setHistory([]);
                setConfirmClear(false);
              }}
            />
          )}
        </div>
      </div>
    </section>
  );
}

function DownloadSettings({
  settings,
  busy,
  error,
  onChoose,
  onReset,
}: {
  settings: AppSettings | null;
  busy: boolean;
  error: string | null;
  onChoose: () => void;
  onReset: () => void;
}) {
  const downloads = settings?.downloads;
  return (
    <>
      <header className="settings-section-heading">
        <div>
          <h2>保存位置</h2>
          <p>设置一键下载使用的默认目录。</p>
        </div>
      </header>
      <section className="site-settings download-settings" aria-labelledby="download-directory-title">
        <div className="site-settings-title">
          <div className="site-monogram" aria-hidden="true">↓</div>
          <div>
            <h3 id="download-directory-title">默认保存位置</h3>
            <p>文件重名时会自动添加序号，不会覆盖已有文件。</p>
          </div>
        </div>
        <div className="settings-field download-directory-field">
          <span>
            <strong>{downloads?.isCustom ? "自定义目录" : "系统下载目录"}</strong>
            <small title={downloads?.directoryPath ?? undefined}>
              {downloads?.directoryPath ?? "未找到可用的下载目录"}
            </small>
          </span>
          <div className="cookie-file-actions">
            <span className={`cookie-file-status is-${downloads?.isAvailable ? "ready" : "missing"}`}>
              {downloads?.isAvailable ? "目录可用" : "需要重新选择"}
            </span>
            <button type="button" disabled={busy} onClick={onChoose}>更换目录</button>
            {downloads?.isCustom && (
              <button className="is-secondary" type="button" disabled={busy} onClick={onReset}>
                恢复系统目录
              </button>
            )}
          </div>
        </div>
        <p className="settings-security-note">
          Velo 只保存目录路径；使用“另存为”仍可为单个任务选择其他位置。
        </p>
        {error && <p className="settings-error" role="alert">{error}</p>}
      </section>
    </>
  );
}

function HistorySettings({
  history,
  confirmClear,
  onRequestClear,
  onCancelClear,
  onClear,
}: {
  history: DownloadHistoryItem[];
  confirmClear: boolean;
  onRequestClear: () => void;
  onCancelClear: () => void;
  onClear: () => void;
}) {
  return (
    <>
      <header className="settings-section-heading history-heading">
        <div>
          <h2>下载历史</h2>
          <p>最多保留最近 50 条已完成下载，不保存失败或取消任务。</p>
        </div>
        {history.length > 0 && (
          <div className="history-clear-actions">
            {confirmClear ? (
              <>
                <button type="button" className="is-secondary" onClick={onCancelClear}>取消</button>
                <button type="button" className="is-danger" onClick={onClear}>确认清空</button>
              </>
            ) : (
              <button type="button" className="is-secondary" onClick={onRequestClear}>清空记录</button>
            )}
          </div>
        )}
      </header>

      {history.length === 0 ? (
        <div className="history-empty">
          <span aria-hidden="true">↓</span>
          <strong>还没有下载记录</strong>
          <p>文件成功保存后，会在这里显示标题、格式、完成时间和保存位置。</p>
        </div>
      ) : (
        <ol className="history-list" aria-label="已完成下载">
          {history.map((item) => (
            <li key={item.id}>
              <div className="history-item-main">
                <strong title={item.title}>{item.title}</strong>
                <span>{item.site} · {item.formatLabel} · {item.container.toUpperCase()}</span>
              </div>
              <time dateTime={item.completedAt}>
                {new Intl.DateTimeFormat("zh-CN", {
                  month: "2-digit",
                  day: "2-digit",
                  hour: "2-digit",
                  minute: "2-digit",
                }).format(new Date(item.completedAt))}
              </time>
              <p title={item.destinationPath}>{item.destinationPath}</p>
            </li>
          ))}
        </ol>
      )}
    </>
  );
}
