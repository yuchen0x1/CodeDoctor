import { beforeEach, describe, expect, it, vi } from "vitest";

const open = vi.fn();
const save = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({ open, save }));

describe("native file dialogs", () => {
  beforeEach(() => {
    open.mockReset();
    save.mockReset();
  });

  it("opens a single project directory and preserves cancellation", async () => {
    open.mockResolvedValueOnce("C:/work/project").mockResolvedValueOnce(null);
    const { pickProjectDirectory } = await import("./nativeDialogs");

    await expect(pickProjectDirectory()).resolves.toBe("C:/work/project");
    await expect(pickProjectDirectory()).resolves.toBeNull();
    expect(open).toHaveBeenNthCalledWith(1, {
      directory: true,
      multiple: false,
      title: "选择项目文件夹",
    });
  });

  it("filters project archive selection to ZIP files", async () => {
    open.mockResolvedValue("C:/work/project.zip");
    const { pickProjectArchive } = await import("./nativeDialogs");

    await expect(pickProjectArchive()).resolves.toBe("C:/work/project.zip");
    expect(open).toHaveBeenCalledWith({
      directory: false,
      multiple: false,
      title: "选择项目 ZIP",
      filters: [{ name: "ZIP archive", extensions: ["zip"] }],
    });
  });

  it.each([
    ["json", "codedoctor-report.json", "JSON report", "json"],
    ["markdown", "codedoctor-report.md", "Markdown report", "md"],
    ["txt", "codedoctor-report.txt", "Text report", "txt"],
    ["html", "codedoctor-report.html", "HTML report", "html"],
  ] as const)("saves %s reports with a matching filter", async (format, defaultPath, name, extension) => {
    save.mockResolvedValue("C:/reports/" + defaultPath);
    const { pickReportDestination } = await import("./nativeDialogs");

    await expect(pickReportDestination(format)).resolves.toBe("C:/reports/" + defaultPath);
    expect(save).toHaveBeenCalledWith({
      defaultPath,
      title: "保存检测报告",
      filters: [{ name, extensions: [extension] }],
    });
  });

  it.each([
    ["json", "codedoctor-history.json", "JSON report", "json"],
    ["markdown", "codedoctor-history.md", "Markdown report", "md"],
  ] as const)("saves %s history comparisons with a matching filter", async (format, defaultPath, name, extension) => {
    save.mockResolvedValue("C:/reports/" + defaultPath);
    const { pickHistoryReportDestination } = await import("./nativeDialogs");

    await expect(pickHistoryReportDestination(format)).resolves.toBe("C:/reports/" + defaultPath);
    expect(save).toHaveBeenCalledWith({
      defaultPath,
      title: "保存基线对比报告",
      filters: [{ name, extensions: [extension] }],
    });
  });
});
