import { beforeEach, describe, expect, it, vi } from "vitest";
import { openIssueSource, readIssueSource } from "./sourceContext";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

describe("issue source bridge", () => {
  beforeEach(() => invoke.mockReset());

  it("loads and opens a persisted issue by run and issue IDs", async () => {
    const context = {
      text: "line",
      startLine: 3,
      endLine: 3,
      stale: false,
      redacted: false,
      truncated: false,
      canOpenNatively: true,
    };
    invoke.mockResolvedValueOnce(context).mockResolvedValueOnce(undefined);

    await expect(readIssueSource("run-1", "issue-2")).resolves.toEqual(context);
    await expect(openIssueSource("run-1", "issue-2")).resolves.toBeUndefined();
    expect(invoke.mock.calls).toEqual([
      ["read_issue_source", { runId: "run-1", issueId: "issue-2" }],
      ["open_issue_source", { runId: "run-1", issueId: "issue-2" }],
    ]);
  });

  it("surfaces the native validation message for failed source operations", async () => {
    invoke.mockRejectedValueOnce({ Validation: "source file changed since the scan" });
    await expect(openIssueSource("run-1", "issue-2")).rejects.toThrow("source file changed since the scan");
  });
});
