import { useCallback, useEffect, useRef, useState } from "react";
import { BrandMark } from "./components/BrandMark";
import { MediaInspector } from "./features/downloads/MediaInspector";
import { SettingsView } from "./features/settings/SettingsView";
import {
  MAX_WORKSPACE_TABS,
  activeTabAfterClose,
  createWorkspaceTab,
  updateWorkspaceTab,
  workspaceTabStatusLabel,
  workspaceTabTitle,
  type InspectorTabSnapshot,
  type WorkspaceTab,
} from "./features/workspace/workspace-tabs";
import { isTauri } from "@tauri-apps/api/core";

export default function App() {
  const [view, setView] = useState<"home" | "settings">("home");
  const nextTabSequence = useRef(2);
  const [tabs, setTabs] = useState<WorkspaceTab[]>(() => [
    createWorkspaceTab(crypto.randomUUID(), 1),
  ]);
  const [activeTabId, setActiveTabId] = useState(() => tabs[0].id);
  const isMacTauri =
    isTauri() && navigator.userAgent.toLowerCase().includes("mac");

  const addTab = useCallback(() => {
    if (tabs.length >= MAX_WORKSPACE_TABS) return;
    const tab = createWorkspaceTab(
      crypto.randomUUID(),
      nextTabSequence.current++,
    );
    setTabs((current) => [...current, tab]);
    setActiveTabId(tab.id);
    setView("home");
  }, [tabs.length]);

  const closeTab = useCallback(
    (id: string) => {
      const target = tabs.find((tab) => tab.id === id);
      if (!target || target.status === "downloading") return;

      const suggestedActiveId = activeTabAfterClose(tabs, id);
      const remaining = tabs.filter((tab) => tab.id !== id);
      if (remaining.length === 0) {
        const replacement = createWorkspaceTab(
          crypto.randomUUID(),
          nextTabSequence.current++,
        );
        setTabs([replacement]);
        setActiveTabId(replacement.id);
        return;
      }

      setTabs(remaining);
      if (activeTabId === id && suggestedActiveId) {
        setActiveTabId(suggestedActiveId);
      }
    },
    [activeTabId, tabs],
  );

  const handleTabSnapshot = useCallback(
    (id: string, snapshot: InspectorTabSnapshot) => {
      setTabs((current) => updateWorkspaceTab(current, id, snapshot));
    },
    [],
  );

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        setView("settings");
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "t") {
        event.preventDefault();
        addTab();
      }
      if (
        view === "home" &&
        (event.metaKey || event.ctrlKey) &&
        event.key.toLowerCase() === "w"
      ) {
        event.preventDefault();
        closeTab(activeTabId);
      }
      if (event.key === "Escape" && view === "settings") {
        setView("home");
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [activeTabId, addTab, closeTab, view]);

  return (
    <main className={`app-shell${isMacTauri ? " is-macos-frame" : ""}`}>
      <div className="ambient-flow" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>

      <header className="app-header" data-tauri-drag-region>
        <BrandMark />
        <button
          className="settings-trigger"
          type="button"
          aria-label="打开设置"
          aria-pressed={view === "settings"}
          title="设置（⌘, / Ctrl+,）"
          onClick={() => setView(view === "settings" ? "home" : "settings")}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="12" cy="12" r="6.25" />
            <circle cx="12" cy="12" r="2.5" />
            <path d="M12 2.5v2.1M12 19.4v2.1M2.5 12h2.1M19.4 12h2.1M5.28 5.28l1.49 1.49M17.23 17.23l1.49 1.49M18.72 5.28l-1.49 1.49M6.77 17.23l-1.49 1.49" />
          </svg>
        </button>
      </header>

      <section
        className="workspace"
        aria-labelledby="workspace-title"
        hidden={view !== "home"}
      >
        <div className="intro-copy">
          <h1 id="workspace-title">
            <span className="headline-line">轻取流光</span>
            <span className="headline-line headline-accent">留住此刻</span>
          </h1>
          <p className="slogan-en">
            <span>Catch the stream,</span>
            <span className="slogan-accent">Keep the moment.</span>
          </p>
        </div>

        <div className="task-workspace">
          <div className="task-tabs">
            <div className="task-tab-list" role="tablist" aria-label="视频任务">
              {tabs.map((tab) => {
                const active = tab.id === activeTabId;
                const title = workspaceTabTitle(tab);
                const status = workspaceTabStatusLabel(tab);
                return (
                  <div
                    className={`task-tab is-${tab.status}${active ? " is-active" : ""}`}
                    key={tab.id}
                  >
                    <button
                      className="task-tab-select"
                      id={`task-tab-${tab.id}`}
                      type="button"
                      role="tab"
                      aria-selected={active}
                      aria-controls={`task-panel-${tab.id}`}
                      tabIndex={active ? 0 : -1}
                      title={`${title} · ${status}`}
                      onClick={() => setActiveTabId(tab.id)}
                      onKeyDown={(event) => {
                        const index = tabs.findIndex(({ id }) => id === tab.id);
                        let nextIndex: number | null = null;
                        if (event.key === "ArrowRight") {
                          nextIndex = (index + 1) % tabs.length;
                        } else if (event.key === "ArrowLeft") {
                          nextIndex = (index - 1 + tabs.length) % tabs.length;
                        } else if (event.key === "Home") {
                          nextIndex = 0;
                        } else if (event.key === "End") {
                          nextIndex = tabs.length - 1;
                        }
                        if (nextIndex === null) return;
                        event.preventDefault();
                        const nextTab = tabs[nextIndex];
                        setActiveTabId(nextTab.id);
                        requestAnimationFrame(() => {
                          document.getElementById(`task-tab-${nextTab.id}`)?.focus();
                        });
                      }}
                    >
                      <span className="task-status-dot" aria-hidden="true" />
                      <span className="task-tab-title">{title}</span>
                      {tab.status === "downloading" && tab.progress !== null && (
                        <span className="task-tab-progress">{tab.progress}%</span>
                      )}
                      <span className="sr-only">，{status}</span>
                    </button>
                    <button
                      className="task-tab-close"
                      type="button"
                      aria-label={`关闭 ${title}`}
                      disabled={tab.status === "downloading"}
                      title={
                        tab.status === "downloading"
                          ? "请先取消下载，再关闭任务"
                          : "关闭任务（⌘W / Ctrl+W）"
                      }
                      onClick={() => closeTab(tab.id)}
                    >
                      ×
                    </button>
                  </div>
                );
              })}
            </div>
            <button
              className="task-tab-add"
              type="button"
              aria-label="新建视频任务"
              title="新建任务（⌘T / Ctrl+T）"
              disabled={tabs.length >= MAX_WORKSPACE_TABS}
              onClick={addTab}
            >
              +
            </button>
          </div>

          <div className="task-panels">
            {tabs.map((tab) => {
              const active = tab.id === activeTabId;
              return (
                <div
                  id={`task-panel-${tab.id}`}
                  key={tab.id}
                  role="tabpanel"
                  aria-labelledby={`task-tab-${tab.id}`}
                  hidden={!active}
                >
                  <MediaInspector
                    tabId={tab.id}
                    active={active && view === "home"}
                    onOpenSettings={() => setView("settings")}
                    onSnapshotChange={handleTabSnapshot}
                  />
                </div>
              );
            })}
          </div>
        </div>
      </section>

      {view === "settings" && <SettingsView onBack={() => setView("home")} />}
    </main>
  );
}
