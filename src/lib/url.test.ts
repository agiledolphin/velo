import { describe, expect, it } from "vitest";
import { normalizeWebUrl } from "./url";

describe("normalizeWebUrl", () => {
  it("normalizes valid https URLs", () => {
    expect(normalizeWebUrl("  https://example.com/watch?v=1  ")).toBe(
      "https://example.com/watch?v=1",
    );
  });

  it("rejects non-web protocols", () => {
    expect(normalizeWebUrl("file:///tmp/video.mp4")).toBeNull();
  });

  it("rejects empty and malformed input", () => {
    expect(normalizeWebUrl(" ")).toBeNull();
    expect(normalizeWebUrl("not a url")).toBeNull();
  });
});
