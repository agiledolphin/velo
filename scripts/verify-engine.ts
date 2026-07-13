import { YT_DLP_VERSION, verifyInstalledEngine } from "./engine-manifest";
import { installedEnginePath } from "./engine-paths";

async function main() {
  const executable = process.env.VELO_YT_DLP_PATH || installedEnginePath();
  await verifyInstalledEngine(executable);
  console.log(`yt-dlp ${YT_DLP_VERSION} 校验通过：${executable}`);
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : "yt-dlp 校验失败");
  process.exitCode = 1;
});
