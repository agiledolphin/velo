import { invoke, isTauri } from "@tauri-apps/api/core";
import type { InspectFailure, MediaInfo } from "./media";

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

export async function inspectMedia(url: string): Promise<MediaInfo> {
  try {
    const request =
      import.meta.env.DEV && !isTauri()
        ? Promise.resolve(browserPreviewMedia(url))
        : invoke<MediaInfo>("inspect_url", { url });

    const [media] = await Promise.all([
      request,
      new Promise((resolve) => window.setTimeout(resolve, 420)),
    ]);
    return media;
  } catch (error) {
    if (isInspectFailure(error)) {
      throw new Error(error.message);
    }

    throw new Error("暂时无法解析这个地址，请稍后再试。");
  }
}
