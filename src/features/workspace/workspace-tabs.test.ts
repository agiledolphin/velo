import { describe, expect, it } from "vitest";
import {
  activeTabAfterClose,
  createWorkspaceTab,
  updateWorkspaceTab,
  workspaceTabStatusLabel,
  workspaceTabTitle,
} from "./workspace-tabs";

describe("workspace tabs", () => {
  it("creates a named empty task", () => {
    const tab = createWorkspaceTab("tab-1", 3);
    expect(workspaceTabTitle(tab)).toBe("新任务 3");
    expect(workspaceTabStatusLabel(tab)).toBe("等待地址");
  });

  it("uses parsed media metadata and progress in the tab", () => {
    const tabs = updateWorkspaceTab([createWorkspaceTab("tab-1", 1)], "tab-1", {
      url: "https://video.example/watch",
      title: "A long-awaited stream",
      status: "downloading",
      progress: 42,
    });
    expect(workspaceTabTitle(tabs[0])).toBe("A long-awaited stream");
    expect(workspaceTabStatusLabel(tabs[0])).toBe("正在下载 42%");
  });

  it("selects the adjacent task after closing a tab", () => {
    const tabs = [
      createWorkspaceTab("tab-1", 1),
      createWorkspaceTab("tab-2", 2),
      createWorkspaceTab("tab-3", 3),
    ];
    expect(activeTabAfterClose(tabs, "tab-2")).toBe("tab-3");
    expect(activeTabAfterClose(tabs, "tab-3")).toBe("tab-2");
  });
});
