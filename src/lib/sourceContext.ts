import { invoke } from "@tauri-apps/api/core";

export interface IssueSourceContext {
  text: string;
  startLine: number;
  endLine: number;
  stale: boolean;
  redacted: boolean;
  truncated: boolean;
  canOpenNatively: boolean;
}

export function readIssueSource(runId: string, issueId: string): Promise<IssueSourceContext | null> {
  return invoke<IssueSourceContext | null>("read_issue_source", { runId, issueId }).catch(throwSourceError);
}

export function openIssueSource(runId: string, issueId: string): Promise<void> {
  return invoke<void>("open_issue_source", { runId, issueId }).catch(throwSourceError);
}

function throwSourceError(reason: unknown): never {
  if (reason instanceof Error) throw reason;
  if (typeof reason === "string") throw new Error(reason);
  const message = reason && typeof reason === "object"
    ? Object.values(reason).find((value): value is string => typeof value === "string")
    : undefined;
  throw new Error(message ?? "源文件操作失败");
}
