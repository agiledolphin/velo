import { createHash } from "node:crypto";
import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export const YT_DLP_VERSION = "2026.07.04";
export const MAX_ENGINE_DOWNLOAD_BYTES = 100 * 1024 * 1024;
export const ENGINE_VERSION_TIMEOUT_MS = 60_000;

export interface EngineAsset {
  assetName: string;
  executableName: string;
  sha256: string;
}

const ASSETS: Record<string, EngineAsset> = {
  "darwin-arm64": {
    assetName: "yt-dlp_macos",
    executableName: "yt-dlp",
    sha256: "498bd0dae17855c599d371d68ec5bafc439a9d8640e838be25c765a9792f261b",
  },
  "darwin-x64": {
    assetName: "yt-dlp_macos",
    executableName: "yt-dlp",
    sha256: "498bd0dae17855c599d371d68ec5bafc439a9d8640e838be25c765a9792f261b",
  },
  "linux-arm64": {
    assetName: "yt-dlp_linux_aarch64",
    executableName: "yt-dlp",
    sha256: "b6ce97646773070d7a7ffd6bbbdcaecb47c48483909c54c915bf08a7a9b5e0b1",
  },
  "linux-x64": {
    assetName: "yt-dlp_linux",
    executableName: "yt-dlp",
    sha256: "6bbb3d314cde4febe36e5fa1d55462e29c974f63444e707871834f6d8cc210ae",
  },
  "win32-arm64": {
    assetName: "yt-dlp_arm64.exe",
    executableName: "yt-dlp.exe",
    sha256: "1525690b037ecc0bb677e38e7147b0025179cbc9a8d0c57264e3100b18099280",
  },
  "win32-x64": {
    assetName: "yt-dlp.exe",
    executableName: "yt-dlp.exe",
    sha256: "52fe3c26dcf71fbdc85b528589020bb0b8e383155cfa81b64dd447bbe35e24b8",
  },
};

const TARGET_PLATFORMS: Record<string, string> = {
  "aarch64-apple-darwin": "darwin-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
  "aarch64-pc-windows-msvc": "win32-arm64",
  "x86_64-pc-windows-msvc": "win32-x64",
};

export function resolveEngineAsset(
  platform = process.platform,
  architecture = process.arch,
): EngineAsset {
  const key = `${platform}-${architecture}`;
  const asset = ASSETS[key];
  if (!asset) {
    throw new Error(`暂不支持当前平台：${key}`);
  }
  return asset;
}

export function resolveEngineAssetForTarget(targetTriple: string): EngineAsset {
  const platformKey = TARGET_PLATFORMS[targetTriple];
  const asset = platformKey ? ASSETS[platformKey] : undefined;
  if (!asset) {
    throw new Error(`暂不支持当前构建目标：${targetTriple}`);
  }
  return asset;
}

export function sidecarFileName(
  targetTriple: string,
  asset = resolveEngineAssetForTarget(targetTriple),
): string {
  const extension = asset.executableName.endsWith(".exe") ? ".exe" : "";
  return `yt-dlp-${targetTriple}${extension}`;
}

export function engineDownloadUrl(asset: EngineAsset): string {
  return `https://github.com/yt-dlp/yt-dlp/releases/download/${YT_DLP_VERSION}/${asset.assetName}`;
}

export function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export function assertEngineChecksum(
  bytes: Uint8Array,
  asset: EngineAsset,
): void {
  const actual = sha256(bytes);
  if (actual !== asset.sha256) {
    throw new Error(`yt-dlp 校验失败：期望 ${asset.sha256}，实际 ${actual}`);
  }
}

export async function verifyInstalledEngine(
  executablePath: string,
  asset = resolveEngineAsset(),
): Promise<void> {
  await access(
    executablePath,
    process.platform === "win32" ? constants.R_OK : constants.X_OK,
  );
  const bytes = await readFile(executablePath);
  assertEngineChecksum(bytes, asset);

  let stdout: string;
  try {
    ({ stdout } = await execFileAsync(executablePath, ["--version"], {
      timeout: ENGINE_VERSION_TIMEOUT_MS,
      maxBuffer: 1024 * 1024,
      windowsHide: true,
    }));
  } catch (error) {
    const failure = error as Error & {
      code?: number | string;
      signal?: string;
      stderr?: string;
    };
    const details = [
      failure.code === undefined ? undefined : `退出码 ${failure.code}`,
      failure.signal ? `信号 ${failure.signal}` : undefined,
      failure.stderr?.trim() || undefined,
    ].filter(Boolean);
    throw new Error(
      `yt-dlp 无法启动${details.length ? `（${details.join("；")}）` : ""}`,
      { cause: error },
    );
  }
  const version = stdout.trim();
  if (version !== YT_DLP_VERSION) {
    throw new Error(
      `yt-dlp 版本不匹配：期望 ${YT_DLP_VERSION}，实际 ${version || "未知"}`,
    );
  }
}
