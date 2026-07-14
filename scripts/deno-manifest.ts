import { createHash } from "node:crypto";
import { unzipSync } from "fflate";

export const DENO_VERSION = "2.9.2";
export const MAX_DENO_DOWNLOAD_BYTES = 64 * 1024 * 1024;

export interface DenoAsset {
  assetName: string;
  executableName: string;
  archiveSha256: string;
  binarySha256: string;
}

const ASSETS: Record<string, DenoAsset> = {
  "aarch64-apple-darwin": asset(
    "deno-aarch64-apple-darwin.zip",
    "deno",
    "687ae485168ba73a4f1ee3a954eb4f077eca82f2fefd236a6a83a3889287876c",
    "218ab752ae8f64f0a7822af710886488f15169fdae153a3aada4861f9635b266",
  ),
  "x86_64-apple-darwin": asset(
    "deno-x86_64-apple-darwin.zip",
    "deno",
    "c953379e5a85a0a30e99aa51b807633e380e809a1181f53e4904d5fa73785bff",
    "201651c6e72bd0df2dbe994b4f8ca0f935631e08c27290a3a92342e02ad0e865",
  ),
  "aarch64-unknown-linux-gnu": asset(
    "deno-aarch64-unknown-linux-gnu.zip",
    "deno",
    "310b8f48e59964ff18890d35e64f64fb90e8b1cc5d9ebff8c818327d5afb16d2",
    "1a2b9903943f9741c4f5f0afc1e2002e0c5c7320b8487a7f192f7695cd36c9a1",
  ),
  "x86_64-unknown-linux-gnu": asset(
    "deno-x86_64-unknown-linux-gnu.zip",
    "deno",
    "934d1bd5cb09eaed7f2e4a4fc58208d04a3c5c0fcde9f319d93d735265c67a4a",
    "5bc8a7a4a628360b391ddeac2efac7dec9e670b33156d831bf1e899070655173",
  ),
  "x86_64-pc-windows-msvc": asset(
    "deno-x86_64-pc-windows-msvc.zip",
    "deno.exe",
    "5fe194d26ac5ef77fcc5288c2c438c7a0465f3b6180440ebf04092714bf2dcdf",
    "a5270c2bb75a2ec12fef53185730327267d9e9fe6be6a962c5d1d5a050f93c88",
  ),
};

function asset(
  assetName: string,
  executableName: string,
  archiveSha256: string,
  binarySha256: string,
): DenoAsset {
  return { assetName, executableName, archiveSha256, binarySha256 };
}

export function resolveDenoAssetForTarget(target: string): DenoAsset {
  const resolved = ASSETS[target];
  if (!resolved) throw new Error(`Deno 暂不支持当前构建目标：${target}`);
  return resolved;
}

export function denoDownloadUrl(asset: DenoAsset): string {
  return `https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/${asset.assetName}`;
}

export function denoSidecarFileName(
  target: string,
  asset = resolveDenoAssetForTarget(target),
): string {
  const extension = asset.executableName.endsWith(".exe") ? ".exe" : "";
  return `deno-${target}${extension}`;
}

export function assertDenoArchiveChecksum(
  bytes: Uint8Array,
  asset: DenoAsset,
): void {
  assertChecksum(bytes, asset.archiveSha256, "Deno 压缩包");
}

export function extractDenoBinary(
  bytes: Uint8Array,
  asset: DenoAsset,
): Uint8Array {
  const entries = unzipSync(bytes, {
    filter: (file) => file.name === asset.executableName,
  });
  const binary = entries[asset.executableName];
  if (!binary?.byteLength) throw new Error("Deno 压缩包中缺少可执行文件");
  return binary;
}

export function assertDenoBinaryChecksum(
  bytes: Uint8Array,
  asset: DenoAsset,
): void {
  assertChecksum(bytes, asset.binarySha256, "Deno");
}

function assertChecksum(bytes: Uint8Array, expected: string, label: string) {
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== expected) {
    throw new Error(`${label}校验失败：期望 ${expected}，实际 ${actual}`);
  }
}
