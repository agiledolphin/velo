import { describe, expect, it } from "vitest";

import {
  assertEngineChecksum,
  engineDownloadUrl,
  resolveEngineAsset,
  resolveEngineAssetForTarget,
  sha256,
  sidecarFileName,
} from "./engine-manifest";

describe("yt-dlp engine manifest", () => {
  it("selects the official universal macOS asset", () => {
    const arm = resolveEngineAsset("darwin", "arm64");
    const intel = resolveEngineAsset("darwin", "x64");

    expect(arm).toEqual(intel);
    expect(arm.assetName).toBe("yt-dlp_macos");
    expect(engineDownloadUrl(arm)).toContain("/2026.07.04/yt-dlp_macos");
  });

  it("selects architecture-specific Linux and Windows assets", () => {
    expect(resolveEngineAsset("linux", "arm64").assetName).toBe(
      "yt-dlp_linux_aarch64",
    );
    expect(resolveEngineAsset("win32", "x64").assetName).toBe("yt-dlp.exe");
  });

  it("rejects unsupported targets", () => {
    expect(() => resolveEngineAsset("freebsd", "x64")).toThrow(
      "暂不支持当前平台",
    );
    expect(() => resolveEngineAssetForTarget("wasm32-unknown-unknown")).toThrow(
      "暂不支持当前构建目标",
    );
  });

  it("maps Rust target triples to Tauri sidecar filenames", () => {
    expect(resolveEngineAssetForTarget("aarch64-apple-darwin").assetName).toBe(
      "yt-dlp_macos",
    );
    expect(sidecarFileName("aarch64-apple-darwin")).toBe(
      "yt-dlp-aarch64-apple-darwin",
    );
    expect(sidecarFileName("x86_64-pc-windows-msvc")).toBe(
      "yt-dlp-x86_64-pc-windows-msvc.exe",
    );
  });

  it("computes and enforces SHA-256", () => {
    const bytes = new TextEncoder().encode("velo");
    const digest = sha256(bytes);

    expect(digest).toBe(
      "cd1e45e8b94d27b2562ab5ae45b4bff61e2bc89d9ca4ffe117e70aa5ac8ef1eb",
    );
    expect(() =>
      assertEngineChecksum(bytes, {
        assetName: "fixture",
        executableName: "fixture",
        sha256: digest,
      }),
    ).not.toThrow();
    expect(() =>
      assertEngineChecksum(bytes, {
        assetName: "fixture",
        executableName: "fixture",
        sha256: "0".repeat(64),
      }),
    ).toThrow("yt-dlp 校验失败");
  });
});
