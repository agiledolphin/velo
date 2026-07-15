import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

interface TauriConfig {
  version: string;
  build: {
    beforeDevCommand: string;
    beforeBuildCommand: string;
  };
  bundle: {
    externalBin: string[];
    resources: string[];
  };
}

async function loadTauriConfig(): Promise<TauriConfig> {
  const directory = dirname(fileURLToPath(import.meta.url));
  const path = resolve(directory, "../src-tauri/tauri.conf.json");
  return JSON.parse(await readFile(path, "utf8")) as TauriConfig;
}

async function loadPlatformWorkflow(): Promise<string> {
  const directory = dirname(fileURLToPath(import.meta.url));
  return readFile(
    resolve(directory, "../.github/workflows/platform-release-check.yml"),
    "utf8",
  );
}

async function loadFastCiWorkflow(): Promise<string> {
  const directory = dirname(fileURLToPath(import.meta.url));
  return readFile(resolve(directory, "../.github/workflows/ci.yml"), "utf8");
}

async function loadCompatibilityWorkflow(): Promise<string> {
  const directory = dirname(fileURLToPath(import.meta.url));
  return readFile(
    resolve(directory, "../.github/workflows/site-compatibility-check.yml"),
    "utf8",
  );
}

async function loadProjectFile(path: string): Promise<string> {
  const directory = dirname(fileURLToPath(import.meta.url));
  return readFile(resolve(directory, `../${path}`), "utf8");
}

describe("Tauri sidecar bundle configuration", () => {
  it("keeps release metadata consistent without displaying it in the header", async () => {
    const config = await loadTauriConfig();
    const packageJson = JSON.parse(
      await loadProjectFile("package.json"),
    ) as { version: string };
    const cargoManifest = await loadProjectFile("src-tauri/Cargo.toml");
    const app = await loadProjectFile("src/App.tsx");

    expect(packageJson.version).toBe("0.2.0");
    expect(config.version).toBe(packageJson.version);
    expect(cargoManifest).toContain(`version = "${packageJson.version}"`);
    expect(app).not.toContain("build-label");
    expect(app).not.toContain("PREVIEW ·");
  });

  it("prepares the pinned engine before desktop development and builds", async () => {
    const config = await loadTauriConfig();

    expect(config.build.beforeDevCommand).toContain("engine:prepare-sidecar");
    expect(config.build.beforeBuildCommand).toContain("engine:prepare-sidecar");
    expect(config.bundle.externalBin).toEqual([
      "binaries/yt-dlp",
      "binaries/ffmpeg",
      "binaries/deno",
    ]);
    expect(config.bundle.resources).toContain("resources/THIRD_PARTY_NOTICES.md");
  });

  it("builds and inspects native Windows and Linux packages in CI", async () => {
    const workflow = await loadPlatformWorkflow();

    expect(workflow).toContain("x86_64-pc-windows-msvc");
    expect(workflow).toContain("x86_64-unknown-linux-gnu");
    expect(workflow).toContain("TAURI_ENV_TARGET_TRIPLE: ${{ matrix.target }}");
    expect(workflow).toContain("bun run engine:prepare-sidecar");
    expect(workflow).toContain(
      "--ci --target ${{ matrix.target }} --bundles ${{ matrix.bundles }}",
    );
    expect(workflow).toContain("dpkg-deb --contents");
    expect(workflow).toContain("7z l");
    expect(workflow).toContain("ffmpeg.exe");
    expect(workflow).toContain("deno.exe");
    expect(workflow).toContain("grep -q '/ffmpeg$'");
    expect(workflow).toContain("grep -q '/deno$'");
    expect(workflow).toContain("actions/upload-artifact@v7");
    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain('      - "v*"');
    expect(workflow).toContain('      - "src-tauri/tauri.conf.json"');
    expect(workflow).not.toContain("pull_request:");
    expect(workflow).toContain("Swatinem/rust-cache@v2");
  });

  it("runs fast checks for source pushes without building installers", async () => {
    const workflow = await loadFastCiWorkflow();

    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("branches: [main]");
    expect(workflow).toContain('      - "**/*.md"');
    expect(workflow).toContain("bun run build");
    expect(workflow).toContain("cargo test");
    expect(workflow).toContain("cargo clippy");
    expect(workflow).toContain("Swatinem/rust-cache@v2");
    expect(workflow).not.toContain("tauri build");
    expect(workflow).not.toContain("upload-artifact");
  });

  it("runs authorized site checks without storing the URL in the workflow", async () => {
    const workflow = await loadCompatibilityWorkflow();

    expect(workflow).toContain("macos-latest");
    expect(workflow).toContain("windows-latest");
    expect(workflow).toContain("ubuntu-latest");
    expect(workflow).toContain("secrets.VELO_INTEGRATION_TEST_URL");
    expect(workflow).toContain("bun run test:integration");
    expect(workflow).toContain("Swatinem/rust-cache@v2");
    expect(workflow).not.toMatch(/https?:\/\/[^\s]+\/video/);
  });
});
