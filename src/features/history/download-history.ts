export const DOWNLOAD_HISTORY_STORAGE_KEY = "velo.download-history.v1";
export const MAX_DOWNLOAD_HISTORY_ITEMS = 50;
export const DOWNLOAD_HISTORY_UPDATED_EVENT = "velo:download-history-updated";

export interface DownloadHistoryItem {
  id: string;
  title: string;
  site: string;
  formatLabel: string;
  container: string;
  destinationPath: string;
  completedAt: string;
}

interface DownloadHistoryDocument {
  schemaVersion: 1;
  items: DownloadHistoryItem[];
}

function isHistoryItem(value: unknown): value is DownloadHistoryItem {
  if (!value || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  return ["id", "title", "site", "formatLabel", "container", "destinationPath", "completedAt"]
    .every((key) => typeof item[key] === "string");
}

function browserStorage() {
  try {
    return globalThis.localStorage;
  } catch {
    return null;
  }
}

function notifyHistoryChanged() {
  try {
    globalThis.dispatchEvent?.(new CustomEvent(DOWNLOAD_HISTORY_UPDATED_EVENT));
  } catch {
    // Persistence remains useful even if this document cannot dispatch DOM events.
  }
}

export function readDownloadHistory(storage: Storage | null = browserStorage()) {
  if (!storage) return [];
  try {
    const raw = storage.getItem(DOWNLOAD_HISTORY_STORAGE_KEY);
    if (!raw) return [];
    const document = JSON.parse(raw) as Partial<DownloadHistoryDocument>;
    if (document.schemaVersion !== 1 || !Array.isArray(document.items)) return [];
    return document.items.filter(isHistoryItem).slice(0, MAX_DOWNLOAD_HISTORY_ITEMS);
  } catch {
    return [];
  }
}

export function recordCompletedDownload(
  item: DownloadHistoryItem,
  storage: Storage | null = browserStorage(),
) {
  if (!storage) return false;
  const items = readDownloadHistory(storage).filter(({ id }) => id !== item.id);
  const document: DownloadHistoryDocument = {
    schemaVersion: 1,
    items: [item, ...items].slice(0, MAX_DOWNLOAD_HISTORY_ITEMS),
  };
  try {
    storage.setItem(DOWNLOAD_HISTORY_STORAGE_KEY, JSON.stringify(document));
    notifyHistoryChanged();
    return true;
  } catch {
    return false;
  }
}

export function clearDownloadHistory(storage: Storage | null = browserStorage()) {
  if (!storage) return false;
  try {
    storage.removeItem(DOWNLOAD_HISTORY_STORAGE_KEY);
    notifyHistoryChanged();
    return true;
  } catch {
    return false;
  }
}
