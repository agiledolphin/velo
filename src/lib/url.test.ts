import { describe, expect, it } from "vitest";
import { isYoutubeUrl, normalizeWebUrl } from "./url";

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

describe("isYoutubeUrl", () => {
  it("recognizes YouTube hosts without matching lookalike domains", () => {
    expect(isYoutubeUrl("https://www.youtube.com/watch?v=1")).toBe(true);
    expect(isYoutubeUrl("https://music.youtube.com/watch?v=1")).toBe(true);
    expect(isYoutubeUrl("https://youtu.be/1")).toBe(true);
    expect(isYoutubeUrl("https://youtube.com.example/watch?v=1")).toBe(false);
    expect(isYoutubeUrl("https://example.com/video")).toBe(false);
  });
});
