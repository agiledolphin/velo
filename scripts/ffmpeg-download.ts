import {
  MAX_FFMPEG_DOWNLOAD_BYTES,
  type FfmpegAsset,
  assertFfmpegArchiveChecksum,
  assertFfmpegBinaryChecksum,
  extractFfmpegBinary,
  ffmpegDownloadUrl,
} from "./ffmpeg-manifest";

export async function downloadFfmpegAsset(
  asset: FfmpegAsset,
): Promise<Uint8Array> {
  const response = await fetch(ffmpegDownloadUrl(asset), {
    headers: { "User-Agent": "Velo FFmpeg installer" },
    redirect: "follow",
  });
  if (!response.ok) throw new Error(`FFmpeg 下载失败：HTTP ${response.status}`);
  const declaredSize = Number(response.headers.get("content-length"));
  if (
    Number.isFinite(declaredSize) &&
    declaredSize > MAX_FFMPEG_DOWNLOAD_BYTES
  ) {
    throw new Error("FFmpeg 下载被拒绝：文件超过大小上限");
  }
  if (!response.body) throw new Error("FFmpeg 下载失败：响应正文为空");

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let receivedBytes = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    receivedBytes += value.byteLength;
    if (receivedBytes > MAX_FFMPEG_DOWNLOAD_BYTES) {
      await reader.cancel();
      throw new Error("FFmpeg 下载被拒绝：文件超过大小上限");
    }
    chunks.push(value);
  }
  if (receivedBytes === 0) throw new Error("FFmpeg 下载被拒绝：文件为空");

  const bytes = new Uint8Array(receivedBytes);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  assertFfmpegArchiveChecksum(bytes, asset);
  const binary = extractFfmpegBinary(bytes, asset);
  assertFfmpegBinaryChecksum(binary, asset);
  return binary;
}
