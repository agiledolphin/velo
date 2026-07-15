export const MAX_WORKSPACE_TABS = 8;

export type WorkspaceTabStatus =
  | "idle"
  | "parseQueued"
  | "loading"
  | "ready"
  | "error"
  | "downloadQueued"
  | "downloading"
  | "completed";

export interface InspectorTabSnapshot {
  url: string;
  title: string | null;
  status: WorkspaceTabStatus;
  progress: number | null;
}

export interface WorkspaceTab extends InspectorTabSnapshot {
  id: string;
  fallbackTitle: string;
}

export function createWorkspaceTab(id: string, sequence: number): WorkspaceTab {
  return {
    id,
    fallbackTitle: `新任务 ${sequence}`,
    url: "",
    title: null,
    status: "idle",
    progress: null,
  };
}

export function updateWorkspaceTab(
  tabs: WorkspaceTab[],
  id: string,
  snapshot: InspectorTabSnapshot,
) {
  return tabs.map((tab) => {
    if (tab.id !== id) return tab;
    if (
      tab.url === snapshot.url &&
      tab.title === snapshot.title &&
      tab.status === snapshot.status &&
      tab.progress === snapshot.progress
    ) {
      return tab;
    }
    return { ...tab, ...snapshot };
  });
}

export function workspaceTabTitle(tab: WorkspaceTab) {
  return tab.title?.trim() || tab.fallbackTitle;
}

export function activeTabAfterClose(tabs: WorkspaceTab[], closingId: string) {
  const index = tabs.findIndex(({ id }) => id === closingId);
  if (index < 0) return tabs[0]?.id ?? null;
  return tabs[index + 1]?.id ?? tabs[index - 1]?.id ?? null;
}

export function workspaceTabStatusLabel(tab: WorkspaceTab) {
  switch (tab.status) {
    case "parseQueued":
      return "等待解析";
    case "loading":
      return "正在解析";
    case "ready":
      return "已解析";
    case "error":
      return "需要处理";
    case "downloadQueued":
      return "等待下载";
    case "downloading":
      return tab.progress === null ? "正在下载" : `正在下载 ${tab.progress}%`;
    case "completed":
      return "下载完成";
    default:
      return "等待地址";
  }
}
