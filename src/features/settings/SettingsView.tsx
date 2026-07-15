import { useEffect, useState } from "react";
import {
  type AppSettings,
  type CookieFileStatus,
  type YoutubeCookieMode,
  chooseYoutubeCookieFile,
  clearYoutubeCookieFile,
  getAppSettings,
  setYoutubeCookieMode,
} from "../../lib/velo-api";

interface SettingsViewProps {
  onBack: () => void;
}

const statusCopy: Record<CookieFileStatus, string> = {
  notConfigured: "尚未配置",
  ready: "文件可用",
  missing: "文件已移动或删除",
  invalid: "文件格式无效",
};

const settingsSections = [{ id: "sites", label: "站点" }] as const;

function fileName(path: string | null) {
  if (!path) return "未选择 Cookie 文件";
  return path.split(/[\\/]/).pop() || path;
}

export function SettingsView({ onBack }: SettingsViewProps) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
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
          <p>管理站点认证与 Velo 的运行方式。</p>
        </div>
      </div>

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类">
          {settingsSections.map((section) => (
            <button
              className="is-active"
              type="button"
              aria-current="page"
              key={section.id}
            >
              {section.label}
            </button>
          ))}
          <span>更多设置将在这里扩展</span>
        </nav>

        <div className="settings-content">
          <header className="settings-section-heading">
            <div>
              <h2>站点</h2>
              <p>认证信息只会发送给对应的网站。</p>
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
        </div>
      </div>
    </section>
  );
}
