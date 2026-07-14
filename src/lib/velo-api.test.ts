import { describe, expect, it } from "vitest";
import type { MediaInfo } from "./media";
import { chooseDownloadTarget, inspectMedia, isInspectionAbort } from "./velo-api";

describe("inspectMedia cancellation", () => {
  it("rejects a request whose signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();

    const request = inspectMedia("https://video.example/watch", {
      requestId: "already-aborted",
      signal: controller.signal,
    });

    await expect(request).rejects.toSatisfy(isInspectionAbort);
  });

  it("cancels a browser preview while it is waiting", async () => {
    const controller = new AbortController();
    const request = inspectMedia("https://video.example/watch", {
      requestId: "preview-request",
      signal: controller.signal,
    });

    controller.abort();

    await expect(request).rejects.toSatisfy(isInspectionAbort);
  });
});

describe("download target selection", () => {
  it("keeps native save dialogs out of the browser preview", async () => {
    const media: MediaInfo = {
      sourceUrl: "https://video.example/watch",
      title: "Example",
      site: "video.example",
      thumbnailUrl: null,
      durationSeconds: null,
      formats: [
        {
          id: "format-1",
          label: "1080p",
          container: "mp4",
          width: 1920,
          height: 1080,
          filesizeBytes: null,
          hasVideo: true,
          hasAudio: true,
        },
      ],
    };

    await expect(chooseDownloadTarget(media, media.formats[0])).rejects.toThrow(
      "桌面开发模式",
    );
  });
});
