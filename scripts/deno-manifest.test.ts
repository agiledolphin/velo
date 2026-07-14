import { zipSync } from "fflate";
import { describe, expect, it } from "vitest";

import {
  DENO_VERSION,
  assertDenoBinaryChecksum,
  denoDownloadUrl,
  denoSidecarFileName,
  extractDenoBinary,
  resolveDenoAssetForTarget,
} from "./deno-manifest";

describe("Deno runtime manifest", () => {
  it("maps supported targets to fixed official assets", () => {
    const mac = resolveDenoAssetForTarget("aarch64-apple-darwin");
    expect(DENO_VERSION).toBe("2.9.2");
    expect(mac.assetName).toBe("deno-aarch64-apple-darwin.zip");
    expect(denoDownloadUrl(mac)).toContain(
      "/denoland/deno/releases/download/v2.9.2/",
    );
    expect(
      resolveDenoAssetForTarget("x86_64-pc-windows-msvc").executableName,
    ).toBe("deno.exe");
  });

  it("extracts only the runtime executable", () => {
    const asset = resolveDenoAssetForTarget("aarch64-apple-darwin");
    const expected = new TextEncoder().encode("deno");
    const archive = zipSync({
      deno: expected,
      "README.md": new TextEncoder().encode("ignored"),
    });
    expect(extractDenoBinary(archive, asset)).toEqual(expected);
  });

  it("uses Tauri sidecar names and rejects invalid data", () => {
    expect(denoSidecarFileName("x86_64-unknown-linux-gnu")).toBe(
      "deno-x86_64-unknown-linux-gnu",
    );
    expect(denoSidecarFileName("x86_64-pc-windows-msvc")).toBe(
      "deno-x86_64-pc-windows-msvc.exe",
    );
    expect(() => resolveDenoAssetForTarget("wasm32-unknown-unknown")).toThrow(
      "Deno 暂不支持",
    );
    expect(() =>
      assertDenoBinaryChecksum(
        new TextEncoder().encode("wrong"),
        resolveDenoAssetForTarget("aarch64-apple-darwin"),
      ),
    ).toThrow("Deno校验失败");
  });
});
