import { open, save } from "@tauri-apps/plugin-dialog";
import type { ReportFormat } from "./contracts";

function singlePath(result: string | string[] | null): string | null {
  return typeof result === "string" ? result : null;
}

export async function pickProjectDirectory(): Promise<string | null> {
  return singlePath(await open({
    directory: true,
    multiple: false,
    title: "选择项目文件夹",
  }));
}

export async function pickProjectArchive(): Promise<string | null> {
  return singlePath(await open({
    directory: false,
    multiple: false,
    title: "选择项目 ZIP",
    filters: [{ name: "ZIP archive", extensions: ["zip"] }],
  }));
}

const reportOptions: Record<ReportFormat, { defaultPath: string; name: string; extension: string }> = {
  json: { defaultPath: "codedoctor-report.json", name: "JSON report", extension: "json" },
  markdown: { defaultPath: "codedoctor-report.md", name: "Markdown report", extension: "md" },
  txt: { defaultPath: "codedoctor-report.txt", name: "Text report", extension: "txt" },
  html: { defaultPath: "codedoctor-report.html", name: "HTML report", extension: "html" },
  sarif: { defaultPath: "codedoctor-report.sarif", name: "SARIF report", extension: "sarif" },
};

export async function pickReportDestination(format: ReportFormat): Promise<string | null> {
  const options = reportOptions[format];
  return await save({
    defaultPath: options.defaultPath,
    title: "保存检测报告",
    filters: [{ name: options.name, extensions: [options.extension] }],
  });
}

export function pickPromptDestination(format: "md" | "txt"): Promise<string | null> {
  return save({
    defaultPath: `codedoctor-governance.${format}`,
    title: "保存治理提示词",
    filters: [{ name: format === "md" ? "Markdown" : "纯文本", extensions: [format] }],
  });
}

export function pickIssueListDestination(): Promise<string | null> {
  return save({
    defaultPath: "codedoctor-issues.json",
    title: "保存筛选后的问题清单",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
}
