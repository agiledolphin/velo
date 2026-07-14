import { createHash } from "node:crypto";
import { unzipSync } from "fflate";

export const MAX_FFMPEG_DOWNLOAD_BYTES = 128 * 1024 * 1024;

export interface FfmpegAsset {
  assetName: string;
  executableName: string;
  version: string;
  downloadUrl: string;
  archiveSha256: string;
  binarySha256: string;
  archiveEntry?: string;
}

const MARTIN_RIEDL_RELEASE = "https://ffmpeg.martin-riedl.de/download";
const GYAN_RELEASE =
  "https://github.com/GyanD/codexffmpeg/releases/download/8.1.2";

const ASSETS: Record<string, FfmpegAsset> = {
  "darwin-arm64": martinRiedlAsset(
    "macos",
    "arm64",
    "1783011502_8.1.2",
    "ef1aa60006c7b77ce170c1608c08d8e4ba1c30c5746f2ac986ded932d0ac2c3c",
    "eaf91238e104dd0e262bc6510e25061855cc99a6955a721b0ac99660d58c473d",
  ),
  "darwin-x64": martinRiedlAsset(
    "macos",
    "amd64",
    "1783018342_8.1.2",
    "a52ef43883f44c219766d4b3bdde4e635b35465d0b704c01c3a0566b59775df9",
    "1ca59dda73668c59898a0b305afd8a88817a989187f222ec62d64e775d614d23",
  ),
  "linux-arm64": martinRiedlAsset(
    "linux",
    "arm64",
    "1783010599_8.1.2",
    "ab9e16864b6bf4ae7e13bbdbdc29621be11a5c547c57af8d4250e9fa2f5e6461",
    "93a3684e7467d33881f8fa39e3b8408248d4f95fb2e9f6b18383edcdbd70f163",
  ),
  "linux-x64": martinRiedlAsset(
    "linux",
    "amd64",
    "1783011670_8.1.2",
    "56452c0bfc4ee0325cd615d62f46ba8264f62eed34f727c2224c6c84fa7b8719",
    "bea0dfb96f7223b1be497cf11ccda9ddd9a39103b948b342bb6db1c60a56be12",
  ),
  "win32-x64": {
    assetName: "ffmpeg-8.1.2-essentials_build.zip",
    executableName: "ffmpeg.exe",
    version: "8.1.2",
    downloadUrl: `${GYAN_RELEASE}/ffmpeg-8.1.2-essentials_build.zip`,
    archiveSha256: "db580001caa24ac104c8cb856cd113a87b0a443f7bdf47d8c12b1d740584a2ec",
    binarySha256: "1326dde4c84ff1f96fe6b8916c5bed29e163e9b5dccf995f6f3db069d143ec5e",
    archiveEntry: "ffmpeg-8.1.2-essentials_build/bin/ffmpeg.exe",
  },
};

function martinRiedlAsset(
  platform: "linux" | "macos",
  architecture: "amd64" | "arm64",
  release: string,
  archiveSha256: string,
  binarySha256: string,
): FfmpegAsset {
  return {
    assetName: `ffmpeg-${platform}-${architecture}-8.1.2.zip`,
    executableName: "ffmpeg",
    version: "8.1.2",
    downloadUrl: `${MARTIN_RIEDL_RELEASE}/${platform}/${architecture}/${release}/ffmpeg.zip`,
    archiveSha256,
    binarySha256,
    archiveEntry: "ffmpeg",
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
  const binary = unzipSync(bytes, {
    filter: (file) => file.name === asset.archiveEntry,
  })[asset.archiveEntry];
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
