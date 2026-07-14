import { createHash } from "node:crypto";
import { unzipSync } from "fflate";

export const MAX_FFMPEG_DOWNLOAD_BYTES = 100 * 1024 * 1024;

export interface FfmpegAsset {
  assetName: string;
  executableName: string;
  version: string;
  downloadUrl: string;
  archiveSha256: string;
  binarySha256: string;
  archiveEntry?: string;
}

const STATIC_RELEASE =
  "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1";

const ASSETS: Record<string, FfmpegAsset> = {
  "darwin-arm64": {
    assetName: "ffmpeg81arm.zip",
    executableName: "ffmpeg",
    version: "8.1",
    downloadUrl: "https://www.osxexperts.net/ffmpeg81arm.zip",
    archiveSha256: "ebb82529562b71170807bbc6b0e7eb4f0b13af8cbb0e085bb9e8f6fe709598ad",
    binarySha256: "9a08d61f9328e8164ba560ee7a79958e357307fcfeea6fe626b7d66cdc287028",
    archiveEntry: "ffmpeg",
  },
  "darwin-x64": rawStaticAsset(
    "ffmpeg-darwin-x64",
    "ebdddc936f61e14049a2d4b549a412b8a40deeff6540e58a9f2a2da9e6b18894",
  ),
  "linux-arm64": rawStaticAsset(
    "ffmpeg-linux-arm64",
    "6bb182d0d75d23028db82e9e4f723ca69b853d055698486e6984ddb2c06fb8ce",
  ),
  "linux-x64": rawStaticAsset(
    "ffmpeg-linux-x64",
    "e7e7fb30477f717e6f55f9180a70386c62677ef8a4d4d1a5d948f4098aa3eb99",
  ),
  "win32-x64": rawStaticAsset(
    "ffmpeg-win32-x64",
    "04e1307997530f9cf2fe35cba2ca7e8875ca91da02f89d6c7243df819c94ad00",
    "ffmpeg.exe",
  ),
};

function rawStaticAsset(
  assetName: string,
  sha256: string,
  executableName = "ffmpeg",
): FfmpegAsset {
  return {
    assetName,
    executableName,
    version: "6.1.1",
    downloadUrl: `${STATIC_RELEASE}/${assetName}`,
    archiveSha256: sha256,
    binarySha256: sha256,
  };
}

const TARGET_PLATFORMS: Record<string, string> = {
  "aarch64-apple-darwin": "darwin-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
  "x86_64-pc-windows-msvc": "win32-x64",
};

export function resolveFfmpegAssetForTarget(targetTriple: string): FfmpegAsset {
  const platformKey = TARGET_PLATFORMS[targetTriple];
  const asset = platformKey ? ASSETS[platformKey] : undefined;
  if (!asset) throw new Error(`FFmpeg 暂不支持当前构建目标：${targetTriple}`);
  return asset;
}

export function ffmpegSidecarFileName(
  targetTriple: string,
  asset = resolveFfmpegAssetForTarget(targetTriple),
): string {
  const extension = asset.executableName.endsWith(".exe") ? ".exe" : "";
  return `ffmpeg-${targetTriple}${extension}`;
}

export function ffmpegDownloadUrl(asset: FfmpegAsset): string {
  return asset.downloadUrl;
}

export function assertFfmpegArchiveChecksum(
  bytes: Uint8Array,
  asset: FfmpegAsset,
): void {
  assertChecksum(bytes, asset.archiveSha256, "FFmpeg 压缩包");
}

export function extractFfmpegBinary(
  bytes: Uint8Array,
  asset: FfmpegAsset,
): Uint8Array {
  if (!asset.archiveEntry) return bytes;
  const binary = unzipSync(bytes)[asset.archiveEntry];
  if (!binary?.byteLength) throw new Error("FFmpeg 压缩包中缺少可执行文件");
  return binary;
}

export function assertFfmpegBinaryChecksum(
  bytes: Uint8Array,
  asset: FfmpegAsset,
): void {
  assertChecksum(bytes, asset.binarySha256, "FFmpeg");
}

function assertChecksum(bytes: Uint8Array, expected: string, label: string): void {
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== expected) {
    throw new Error(`${label} 校验失败：期望 ${expected}，实际 ${actual}`);
  }
}
