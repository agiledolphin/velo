import { mkdir, rename, rm, writeFile } from "node:fs/promises";

import { downloadEngineAsset } from "./engine-download";
import {
  YT_DLP_VERSION,
  resolveEngineAsset,
  verifyInstalledEngine,
} from "./engine-manifest";
import { binariesDirectory, installedEnginePath } from "./engine-paths";

async function main() {
  const asset = resolveEngineAsset();
  const destination = installedEnginePath();
  const temporary = `${destination}.${process.pid}.tmp`;
  const backup = `${destination}.previous`;
  let hasBackup = false;

  console.log(`正在安装 yt-dlp ${YT_DLP_VERSION} (${asset.assetName})…`);
  await mkdir(binariesDirectory, { recursive: true });
  await rm(temporary, { force: true });
  await rm(backup, { force: true });

  try {
    const bytes = await downloadEngineAsset(asset);
    await writeFile(temporary, bytes, { mode: 0o755, flag: "wx" });
    try {
      await rename(destination, backup);
      hasBackup = true;
    } catch (error) {
      if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
        throw error;
      }
    }
    await rename(temporary, destination);
    try {
      await verifyInstalledEngine(destination, asset);
      await rm(backup, { force: true });
      hasBackup = false;
      console.log(`yt-dlp 已安装并验证：${destination}`);
    } catch (error) {
      await rm(destination, { force: true });
      if (hasBackup) {
        await rename(backup, destination);
        hasBackup = false;
      }
      throw error;
    }
  } finally {
    await rm(temporary, { force: true });
    if (hasBackup) {
      await rename(backup, destination);
    }
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : "yt-dlp 安装失败");
  process.exitCode = 1;
});
