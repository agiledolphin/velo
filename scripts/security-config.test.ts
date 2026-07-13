import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

interface SecurityConfig {
  capabilities: string[];
  csp: Record<string, string>;
  devCsp: Record<string, string>;
  assetProtocol?: { enable?: boolean };
}

async function loadSecurityConfig(): Promise<SecurityConfig> {
  const directory = dirname(fileURLToPath(import.meta.url));
  const path = resolve(directory, "../src-tauri/tauri.conf.json");
  const config = JSON.parse(await readFile(path, "utf8")) as {
    app: { security: SecurityConfig };
  };
  return config.app.security;
}

describe("Tauri security policy", () => {
  it("keeps the production WebView closed to remote resources", async () => {
    const { capabilities, csp, assetProtocol } = await loadSecurityConfig();

    expect(capabilities).toEqual(["default"]);
    expect(csp["default-src"]).toBe("'none'");
    expect(csp["connect-src"]).toBe("ipc: http://ipc.localhost");
    expect(csp["img-src"]).toBe("'self'");
    expect(csp["object-src"]).toBe("'none'");
    expect(csp["frame-ancestors"]).toBe("'none'");
    expect(Object.values(csp).join(" ")).not.toMatch(
      /https:|wss?:|\*|'unsafe-inline'|'unsafe-eval'/,
    );
    expect(assetProtocol?.enable).not.toBe(true);
  });

  it("limits development exceptions to the local Vite server", async () => {
    const { devCsp } = await loadSecurityConfig();

    expect(devCsp["default-src"]).toBe("'none'");
    expect(devCsp["script-src"]).toBe("'self'");
    expect(devCsp["style-src"]).toContain("'unsafe-inline'");
    expect(devCsp["connect-src"]).toContain("ws://localhost:1420");
    expect(Object.values(devCsp).join(" ")).not.toMatch(
      /https:|wss:|\*|'unsafe-eval'/,
    );
  });
});
