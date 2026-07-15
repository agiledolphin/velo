import { describe, expect, it } from "vitest";
import {
  DOWNLOAD_HISTORY_STORAGE_KEY,
  MAX_DOWNLOAD_HISTORY_ITEMS,
  clearDownloadHistory,
  readDownloadHistory,
  recordCompletedDownload,
  type DownloadHistoryItem,
} from "./download-history";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

function item(index: number): DownloadHistoryItem {
  return {
    id: `task-${index}`,
    title: `Video ${index}`,
    site: "example.com",
    formatLabel: "1080p · 音视频",
    container: "mp4",
    destinationPath: `/Downloads/video-${index}.mp4`,
    completedAt: new Date(2026, 6, 15, 10, index).toISOString(),
  };
}

describe("download history", () => {
  it("keeps the newest completed download first without duplicates", () => {
    const storage = new MemoryStorage();
    recordCompletedDownload(item(1), storage);
    recordCompletedDownload(item(2), storage);
    recordCompletedDownload({ ...item(1), title: "Updated" }, storage);

    expect(readDownloadHistory(storage).map(({ id }) => id)).toEqual(["task-1", "task-2"]);
    expect(readDownloadHistory(storage)[0].title).toBe("Updated");
  });

  it("bounds and clears the persisted history", () => {
    const storage = new MemoryStorage();
    for (let index = 0; index < MAX_DOWNLOAD_HISTORY_ITEMS + 5; index += 1) {
      recordCompletedDownload(item(index), storage);
    }
    expect(readDownloadHistory(storage)).toHaveLength(MAX_DOWNLOAD_HISTORY_ITEMS);

    clearDownloadHistory(storage);
    expect(storage.getItem(DOWNLOAD_HISTORY_STORAGE_KEY)).toBeNull();
    expect(readDownloadHistory(storage)).toEqual([]);
  });

  it("ignores malformed persisted data", () => {
    const storage = new MemoryStorage();
    storage.setItem(DOWNLOAD_HISTORY_STORAGE_KEY, "{broken");
    expect(readDownloadHistory(storage)).toEqual([]);
  });
});
