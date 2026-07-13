import { describe, expect, it } from "vitest";
import { inspectMedia, isInspectionAbort } from "./velo-api";

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
