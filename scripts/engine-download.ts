import {
  MAX_ENGINE_DOWNLOAD_BYTES,
  type EngineAsset,
  assertEngineChecksum,
  engineDownloadUrl,
} from "./engine-manifest";

export async function downloadEngineAsset(
  asset: EngineAsset,
): Promise<Uint8Array> {
  const response = await fetch(engineDownloadUrl(asset), {
    headers: { "User-Agent": "Velo engine installer" },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`yt-dlp 下载失败：HTTP ${response.status}`);
  }

  const declaredSize = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredSize) && declaredSize > MAX_ENGINE_DOWNLOAD_BYTES) {
    throw new Error("yt-dlp 下载被拒绝：文件超过大小上限");
  }
  if (!response.body) {
    throw new Error("yt-dlp 下载失败：响应正文为空");
  }

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let receivedBytes = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    receivedBytes += value.byteLength;
    if (receivedBytes > MAX_ENGINE_DOWNLOAD_BYTES) {
      await reader.cancel();
      throw new Error("yt-dlp 下载被拒绝：文件超过大小上限");
    }
    chunks.push(value);
  }

  if (receivedBytes === 0) {
    throw new Error("yt-dlp 下载被拒绝：文件为空");
  }

  const bytes = new Uint8Array(receivedBytes);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  assertEngineChecksum(bytes, asset);
  return bytes;
}
