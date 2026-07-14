import { execFile } from "node:child_process";
import { constants } from "node:fs";
import { access, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";

import { downloadEngineAsset } from "./engine-download";
import { downloadDenoAsset } from "./deno-download";
import {
  DENO_VERSION,
  assertDenoBinaryChecksum,
  denoSidecarFileName,
  resolveDenoAssetForTarget,
} from "./deno-manifest";
import { downloadFfmpegAsset } from "./ffmpeg-download";
import {
  assertFfmpegBinaryChecksum,
  ffmpegSidecarFileName,
  resolveFfmpegAssetForTarget,
} from "./ffmpeg-manifest";
import {
  YT_DLP_VERSION,
  assertEngineChecksum,
  resolveEngineAsset,
  resolveEngineAssetForTarget,
  sidecarFileName,
} from "./engine-manifest";
import { installedEnginePath, sidecarDirectory } from "./engine-paths";

const execFileAsync = promisify(execFile);

async function targetTriple(): Promise<string> {
  const configured = process.env.TAURI_ENV_TARGET_TRIPLE?.trim();
  if (configured) return configured;

  const { stdout } = await execFileAsync("rustc", ["--print", "host-tuple"]);
  const host = stdout.trim();
  if (!host) throw new Error("无法确定 Rust 构建目标");
  return host;
}

async function localEngineBytes(expectedChecksum: string) {
  try {
    const localAsset = resolveEngineAsset();
    if (localAsset.sha256 !== expectedChecksum) return undefined;
    const path = installedEnginePath();
    await access(path, process.platform === "win32" ? constants.R_OK : constants.X_OK);
    const bytes = new Uint8Array(await readFile(path));
    assertEngineChecksum(bytes, localAsset);
    return bytes;
  } catch {
    return undefined;
  }
}

async function main() {
  const target = await targetTriple();
  await prepareYtDlp(target);
  await prepareFfmpeg(target);
  await prepareDeno(target);
}

async function prepareDeno(target: string) {
  const asset = resolveDenoAssetForTarget(target);
  const destination = join(sidecarDirectory, denoSidecarFileName(target, asset));
  const temporary = `${destination}.${process.pid}.tmp`;

  await mkdir(sidecarDirectory, { recursive: true });
  try {
    const existing = new Uint8Array(await readFile(destination));
    assertDenoBinaryChecksum(existing, asset);
    console.log(`Deno ${DENO_VERSION} sidecar 已就绪：${destination}`);
    return;
  } catch {
    await rm(destination, { force: true });
  }

  const bytes = await downloadDenoAsset(asset);
  assertDenoBinaryChecksum(bytes, asset);
  await rm(temporary, { force: true });
  try {
    await writeFile(temporary, bytes, { mode: 0o755, flag: "wx" });
    await rename(temporary, destination);
  } finally {
    await rm(temporary, { force: true });
  }
  console.log(`Deno ${DENO_VERSION} sidecar 已准备：${destination}`);
}

async function prepareYtDlp(target: string) {
  const asset = resolveEngineAssetForTarget(target);
  const destination = join(sidecarDirectory, sidecarFileName(target, asset));
  const temporary = `${destination}.${process.pid}.tmp`;

  await mkdir(sidecarDirectory, { recursive: true });
  try {
    const existing = new Uint8Array(await readFile(destination));
    assertEngineChecksum(existing, asset);
    console.log(`yt-dlp ${YT_DLP_VERSION} sidecar 已就绪：${destination}`);
    return;
  } catch {
    await rm(destination, { force: true });
  }

  const local = await localEngineBytes(asset.sha256);
  const bytes = local ?? (await downloadEngineAsset(asset));
  assertEngineChecksum(bytes, asset);

  await rm(temporary, { force: true });
  try {
    await writeFile(temporary, bytes, { mode: 0o755, flag: "wx" });
    await rename(temporary, destination);
  } finally {
    await rm(temporary, { force: true });
  }
  console.log(`yt-dlp ${YT_DLP_VERSION} sidecar 已准备：${destination}`);
}

async function prepareFfmpeg(target: string) {
  const asset = resolveFfmpegAssetForTarget(target);
  const destination = join(sidecarDirectory, ffmpegSidecarFileName(target, asset));
  const temporary = `${destination}.${process.pid}.tmp`;

  await mkdir(sidecarDirectory, { recursive: true });
  try {
    const existing = new Uint8Array(await readFile(destination));
    assertFfmpegBinaryChecksum(existing, asset);
    console.log(`FFmpeg ${asset.version} sidecar 已就绪：${destination}`);
    return;
  } catch {
    await rm(destination, { force: true });
  }

  const bytes = await downloadFfmpegAsset(asset);
  assertFfmpegBinaryChecksum(bytes, asset);
  await rm(temporary, { force: true });
  try {
    await writeFile(temporary, bytes, { mode: 0o755, flag: "wx" });
    await rename(temporary, destination);
  } finally {
    await rm(temporary, { force: true });
  }
  console.log(`FFmpeg ${asset.version} sidecar 已准备：${destination}`);
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : "媒体 sidecar 准备失败");
  process.exitCode = 1;
});
