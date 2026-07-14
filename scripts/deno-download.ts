import {
  MAX_DENO_DOWNLOAD_BYTES,
  type DenoAsset,
  assertDenoArchiveChecksum,
  assertDenoBinaryChecksum,
  denoDownloadUrl,
  extractDenoBinary,
} from "./deno-manifest";

export async function downloadDenoAsset(asset: DenoAsset): Promise<Uint8Array> {
  const response = await fetch(denoDownloadUrl(asset), {
    headers: { "User-Agent": "Velo Deno installer" },
    redirect: "follow",
  });
  if (!response.ok) throw new Error(`Deno 下载失败：HTTP ${response.status}`);
  const declaredSize = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredSize) && declaredSize > MAX_DENO_DOWNLOAD_BYTES) {
    throw new Error("Deno 下载被拒绝：文件超过大小上限");
  }
  if (!response.body) throw new Error("Deno 下载失败：响应正文为空");

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let receivedBytes = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    receivedBytes += value.byteLength;
    if (receivedBytes > MAX_DENO_DOWNLOAD_BYTES) {
      await reader.cancel();
      throw new Error("Deno 下载被拒绝：文件超过大小上限");
    }
    chunks.push(value);
  }
  if (receivedBytes === 0) throw new Error("Deno 下载被拒绝：文件为空");

  const archive = new Uint8Array(receivedBytes);
  let offset = 0;
  for (const chunk of chunks) {
    archive.set(chunk, offset);
    offset += chunk.byteLength;
  }
  assertDenoArchiveChecksum(archive, asset);
  const binary = extractDenoBinary(archive, asset);
  assertDenoBinaryChecksum(binary, asset);
  return binary;
}
