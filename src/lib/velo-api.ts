import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  DownloadTask,
  DownloadEvent,
  InspectFailure,
  MediaFormat,
  MediaInfo,
} from "./media";

function browserPreviewMedia(url: string): MediaInfo {
  return {
    sourceUrl: url,
    title: "一段等待被留住的流光",
    site: new URL(url).hostname,
    thumbnailUrl: null,
    durationSeconds: 213,
    formats: [
      {
        id: "preview-1080p",
        label: "1080p · 推荐",
        container: "mp4",
        width: 1920,
        height: 1080,
        filesizeBytes: 86 * 1024 * 1024,
        hasVideo: true,
        hasAudio: true,
      },
      {
        id: "preview-720p",
        label: "720p · 轻量",
        container: "mp4",
        width: 1280,
        height: 720,
        filesizeBytes: 48 * 1024 * 1024,
        hasVideo: true,
        hasAudio: true,
      },
    ],
  };
}

function isInspectFailure(value: unknown): value is InspectFailure {
  return (
    typeof value === "object" &&
    value !== null &&
    "message" in value &&
    typeof value.message === "string"
  );
}

export class VeloApiError extends Error {
  constructor(
    message: string,
    readonly code: string,
  ) {
    super(message);
    this.name = "VeloApiError";
  }
}

export interface InspectMediaOptions {
  requestId: string;
  signal: AbortSignal;
}

export type YoutubeCookieMode = "disabled" | "onDemand" | "always";
export type CookieFileStatus = "notConfigured" | "ready" | "missing" | "invalid";

export interface AppSettings {
  schemaVersion: number;
  sites: {
    youtube: {
      cookieMode: YoutubeCookieMode;
      cookieFilePath: string | null;
      cookieFileStatus: CookieFileStatus;
    };
  };
  downloads: {
    directoryPath: string | null;
    isCustom: boolean;
    isAvailable: boolean;
  };
}

let browserPreviewSettings: AppSettings = {
  schemaVersion: 1,
  sites: {
    youtube: {
      cookieMode: "onDemand",
      cookieFilePath: null,
      cookieFileStatus: "notConfigured",
    },
  },
  downloads: {
    directoryPath: "/Users/preview/Downloads",
    isCustom: false,
    isAvailable: true,
  },
};

function abortError() {
  return new DOMException("解析已取消。", "AbortError");
}

export function isInspectionAbort(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}

export async function inspectMedia(
  url: string,
  { requestId, signal }: InspectMediaOptions,
): Promise<MediaInfo> {
  if (signal.aborted) throw abortError();

  const runningInTauri = isTauri();
  let rejectAbort: (reason: DOMException) => void = () => undefined;
  const aborted = new Promise<never>((_, reject) => {
    rejectAbort = reject;
  });
  const handleAbort = () => {
    if (runningInTauri) {
      void invoke("cancel_inspection", { requestId }).catch(() => undefined);
    }
    rejectAbort(abortError());
  };
  signal.addEventListener("abort", handleAbort, { once: true });

  try {
    const request =
      import.meta.env.DEV && !runningInTauri
        ? Promise.resolve(browserPreviewMedia(url))
        : invoke<MediaInfo>("inspect_url", { requestId, url });

    return await Promise.race([
      Promise.all([
        request,
        new Promise((resolve) => globalThis.setTimeout(resolve, 420)),
      ]).then(([media]) => media),
      aborted,
    ]);
  } catch (error) {
    if (isInspectionAbort(error)) throw error;
    if (isInspectFailure(error)) {
      throw new VeloApiError(error.message, error.code);
    }

    throw new Error("暂时无法解析这个地址，请稍后再试。");
  } finally {
    signal.removeEventListener("abort", handleAbort);
  }
}

export async function getAppSettings(): Promise<AppSettings> {
  if (!isTauri()) return structuredClone(browserPreviewSettings);
  try {
    return await invoke<AppSettings>("get_app_settings");
  } catch (error) {
    throw settingsError(error, "无法读取应用设置。");
  }
}

export async function setYoutubeCookieMode(
  mode: YoutubeCookieMode,
): Promise<AppSettings> {
  if (!isTauri()) {
    browserPreviewSettings = {
      ...browserPreviewSettings,
      sites: {
        youtube: { ...browserPreviewSettings.sites.youtube, cookieMode: mode },
      },
    };
    return structuredClone(browserPreviewSettings);
  }
  try {
    return await invoke<AppSettings>("set_youtube_cookie_mode", { mode });
  } catch (error) {
    throw settingsError(error, "无法保存 Cookie 使用方式。");
  }
}

export async function chooseYoutubeCookieFile(): Promise<AppSettings | null> {
  if (!isTauri()) {
    browserPreviewSettings = {
      ...browserPreviewSettings,
      sites: {
        youtube: {
          ...browserPreviewSettings.sites.youtube,
          cookieFilePath: "/Users/preview/Downloads/youtube-cookies.txt",
          cookieFileStatus: "ready",
        },
      },
    };
    return structuredClone(browserPreviewSettings);
  }
  const path = await open({
    title: "选择从浏览器导出的 Cookie 文件",
    multiple: false,
    directory: false,
    filters: [{ name: "Netscape Cookie 文件", extensions: ["txt"] }],
  });
  if (!path || Array.isArray(path)) return null;
  try {
    return await invoke<AppSettings>("configure_youtube_cookie_file", { path });
  } catch (error) {
    if (isInspectFailure(error)) {
      throw new VeloApiError(error.message, error.code);
    }
    throw new Error("无法使用这个 Cookie 文件，请重新导出后再试。");
  }
}

export async function clearYoutubeCookieFile(): Promise<AppSettings> {
  if (!isTauri()) {
    browserPreviewSettings = {
      ...browserPreviewSettings,
      sites: {
        youtube: {
          ...browserPreviewSettings.sites.youtube,
          cookieFilePath: null,
          cookieFileStatus: "notConfigured",
        },
      },
    };
    return structuredClone(browserPreviewSettings);
  }
  try {
    return await invoke<AppSettings>("configure_youtube_cookie_file", { path: null });
  } catch (error) {
    throw settingsError(error, "无法清除 Cookie 文件设置。");
  }
}

export async function chooseDefaultDownloadDirectory(): Promise<AppSettings | null> {
  if (!isTauri()) {
    browserPreviewSettings = {
      ...browserPreviewSettings,
      downloads: {
        directoryPath: "/Users/preview/Movies/Velo",
        isCustom: true,
        isAvailable: true,
      },
    };
    return structuredClone(browserPreviewSettings);
  }
  const path = await open({
    title: "选择默认下载目录",
    multiple: false,
    directory: true,
  });
  if (!path || Array.isArray(path)) return null;
  try {
    return await invoke<AppSettings>("configure_download_directory", { path });
  } catch (error) {
    throw settingsError(error, "无法使用这个下载目录，请重新选择。");
  }
}

export async function resetDefaultDownloadDirectory(): Promise<AppSettings> {
  if (!isTauri()) {
    browserPreviewSettings = {
      ...browserPreviewSettings,
      downloads: {
        directoryPath: "/Users/preview/Downloads",
        isCustom: false,
        isAvailable: true,
      },
    };
    return structuredClone(browserPreviewSettings);
  }
  try {
    return await invoke<AppSettings>("configure_download_directory", { path: null });
  } catch (error) {
    throw settingsError(error, "无法恢复系统下载目录。");
  }
}

function settingsError(error: unknown, fallback: string) {
  if (isInspectFailure(error)) return new VeloApiError(error.message, error.code);
  return new Error(fallback);
}

interface DownloadFileSuggestion {
  fileName: string;
  extension: string;
}

export async function chooseDownloadTarget(
  media: MediaInfo,
  format: MediaFormat,
): Promise<DownloadTask | null> {
  if (!isTauri()) {
    throw new Error("请在 Velo 桌面开发模式中选择保存位置。");
  }

  try {
    const suggestion = await invoke<DownloadFileSuggestion>(
      "suggest_download_file_name",
      { title: media.title, extension: format.container },
    );
    const destinationPath = await save({
      title: "选择视频保存位置",
      defaultPath: suggestion.fileName,
      filters: [
        {
          name: `${format.label} (${suggestion.extension.toUpperCase()})`,
          extensions: [suggestion.extension],
        },
      ],
    });
    if (!destinationPath) return null;

    return await invoke<DownloadTask>("prepare_download_task", {
      taskId: crypto.randomUUID(),
      sourceUrl: media.sourceUrl,
      mediaTitle: media.title,
      formatId: format.id,
      destinationPath,
      expectedExtension: suggestion.extension,
      hasVideo: format.hasVideo,
      hasAudio: format.hasAudio,
    });
  } catch (error) {
    if (isInspectFailure(error)) {
      throw new Error(error.message);
    }
    throw new Error("无法准备保存位置，请重新选择后再试。");
  }
}

export async function prepareDefaultDownloadTarget(
  media: MediaInfo,
  format: MediaFormat,
): Promise<DownloadTask> {
  if (!isTauri()) {
    throw new Error("请在 Velo 桌面开发模式中测试下载。");
  }
  try {
    return await invoke<DownloadTask>("prepare_default_download_task", {
      taskId: crypto.randomUUID(),
      sourceUrl: media.sourceUrl,
      mediaTitle: media.title,
      formatId: format.id,
      expectedExtension: format.container,
      hasVideo: format.hasVideo,
      hasAudio: format.hasAudio,
    });
  } catch (error) {
    if (isInspectFailure(error)) throw new Error(error.message);
    throw new Error("无法使用默认下载目录，请在设置中重新选择。");
  }
}

export async function fetchThumbnailDataUrl(url: string): Promise<string> {
  if (!isTauri()) {
    throw new Error("浏览器预览不加载远程封面。");
  }

  return invoke<string>("fetch_thumbnail", { url });
}

const representativeFrameRequests = new Map<string, Promise<string>>();
const MAX_REPRESENTATIVE_FRAME_CACHE_ENTRIES = 8;

export function generateRepresentativeFrameDataUrl(
  url: string,
): Promise<string> {
  if (!isTauri()) {
    return Promise.reject(new Error("浏览器预览不生成视频代表帧。"));
  }

  const cached = representativeFrameRequests.get(url);
  if (cached) return cached;

  const request = invoke<string>("generate_representative_frame", { url });
  representativeFrameRequests.set(url, request);
  if (
    representativeFrameRequests.size > MAX_REPRESENTATIVE_FRAME_CACHE_ENTRIES
  ) {
    const oldest = representativeFrameRequests.keys().next().value;
    if (oldest !== undefined && oldest !== url) {
      representativeFrameRequests.delete(oldest);
    }
  }
  void request.catch(() => {
    if (representativeFrameRequests.get(url) === request) {
      representativeFrameRequests.delete(url);
    }
  });
  return request;
}

export async function startDownload(
  task: DownloadTask,
): Promise<DownloadTask> {
  try {
    return await invoke<DownloadTask>("start_download", {
      taskId: task.id,
      sourceUrl: task.sourceUrl,
      mediaTitle: task.mediaTitle,
      formatId: task.formatId,
      destinationPath: task.destinationPath,
      expectedExtension: task.outputExtension,
      hasVideo: task.hasVideo,
      hasAudio: task.hasAudio,
    });
  } catch (error) {
    if (isInspectFailure(error)) throw new Error(error.message);
    throw new Error("无法开始下载，请重新选择保存位置后再试。");
  }
}

export async function cancelDownload(taskId: string): Promise<boolean> {
  return invoke<boolean>("cancel_download", { taskId });
}

export function onDownloadEvent(
  handler: (event: DownloadEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => undefined);
  return listen<DownloadEvent>("download-event", ({ payload }) => handler(payload));
}
