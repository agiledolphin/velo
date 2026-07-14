import { describe, expect, it } from "vitest";

import {
  assertFfmpegBinaryChecksum,
  ffmpegDownloadUrl,
  ffmpegSidecarFileName,
  resolveFfmpegAssetForTarget,
} from "./ffmpeg-manifest";

describe("FFmpeg manifest", () => {
  it("maps supported targets to fixed release assets", () => {
    const arm = resolveFfmpegAssetForTarget("aarch64-apple-darwin");
    expect(arm.assetName).toBe("ffmpeg-macos-arm64-8.1.2.zip");
    expect(arm.version).toBe("8.1.2");
    expect(ffmpegDownloadUrl(arm)).toBe(
      "https://ffmpeg.martin-riedl.de/download/macos/arm64/1783011502_8.1.2/ffmpeg.zip",
    );
    expect(resolveFfmpegAssetForTarget("x86_64-unknown-linux-gnu").version).toBe(
      "8.1.2",
    );
    expect(resolveFfmpegAssetForTarget("x86_64-pc-windows-msvc").assetName).toBe(
      "ffmpeg-win32-x64",
    );
  });

  it("uses Tauri target-triple sidecar names", () => {
    expect(ffmpegSidecarFileName("aarch64-apple-darwin")).toBe(
      "ffmpeg-aarch64-apple-darwin",
    );
    expect(ffmpegSidecarFileName("x86_64-pc-windows-msvc")).toBe(
      "ffmpeg-x86_64-pc-windows-msvc.exe",
    );
  });

  it("rejects unsupported targets and checksum mismatches", () => {
    expect(() => resolveFfmpegAssetForTarget("wasm32-unknown-unknown")).toThrow(
      "FFmpeg 暂不支持",
    );
    expect(() =>
      assertFfmpegBinaryChecksum(
        new TextEncoder().encode("wrong"),
        resolveFfmpegAssetForTarget("aarch64-apple-darwin"),
      ),
    ).toThrow("FFmpeg 校验失败");
  });
});
