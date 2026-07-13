import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { resolveEngineAsset } from "./engine-manifest";

const scriptsDirectory = dirname(fileURLToPath(import.meta.url));
export const projectRoot = dirname(scriptsDirectory);
export const binariesDirectory = join(projectRoot, "binaries");
export const sidecarDirectory = join(projectRoot, "src-tauri", "binaries");

export function installedEnginePath(): string {
  return join(binariesDirectory, resolveEngineAsset().executableName);
}
