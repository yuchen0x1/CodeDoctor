import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { compareStableVersions, publishRelease } from "./publish.mjs";

const API = "repos/yuchen0x1/CodeDoctor/releases";
const asset = { name: "mock-installer.exe", size: 42, digest: `sha256:${"a".repeat(64)}` };
const stable = (version) => ({ tag_name: `v${version}`, draft: false, prerelease: false });
const response = (status, body, lineEnding = "\r\n") => ({
  status: status === 200 ? 0 : 1,
  stdout: `HTTP/2.0 ${status} Mock${lineEnding}Content-Type: application/json${lineEnding}${lineEnding}${JSON.stringify(body)}`,
  stderr: status === 200 ? "" : `gh: HTTP ${status}`,
});

// Every subprocess call is intercepted. No temporary files, credentials, network,
// executable gh, or actual release/asset mutations are used by these tests.
function mockGitHub(options = {}) {
  const calls = [];
  let created = false;
  let promoted = false;
  const runGh = (args) => {
    calls.push(args);
    if (args[0] === "api") {
      assert.equal(args[1], "--include");
      const endpoint = args[2];
      if (endpoint === `${API}/tags/v0.2.3`) {
        return options.lookup ?? response(404, { message: "Not Found" });
      }
      const page = new RegExp(`^${API}\\?per_page=100&page=(\\d+)$`).exec(endpoint)?.[1];
      assert(page, `Unexpected API endpoint: ${endpoint}`);
      const pages = created ? options.afterPages ?? options.beforePages : options.beforePages;
      return (created ? options.afterResponse : options.beforeResponse)
        ?? response(200, pages?.[Number(page) - 1] ?? (page === "1" ? [stable("0.2.2")] : []));
    }
    assert.equal(args[0], "release", "Unexpected gh command");
    if (args[1] === "create") {
      assert(!created, "Must never create twice");
      if (options.createResponse) return options.createResponse;
      created = true;
      return { status: 0, stdout: "", stderr: "" };
    }
    if (args[1] === "edit") {
      assert(created, "Must create a draft first");
      promoted = true;
      return options.editResponse ?? { status: 0, stdout: "", stderr: "" };
    }
    assert.equal(args[1], "view", "Unexpected release mutation");
    const release = {
      tagName: "v0.2.3", isDraft: !promoted, url: "mock://release/v0.2.3",
      assets: [{ ...asset, state: "uploaded" }],
    };
    return { status: 0, stdout: JSON.stringify({ ...release, ...options.draft }), stderr: "" };
  };
  return {
    calls,
    invoke: (overrides = {}) => publishRelease({
      directory: "mock-staging", version: "0.2.3", notesPath: "mock-notes.md", assets: [asset], runGh,
      env: { GITHUB_REF: "refs/tags/v0.2.3", GITHUB_REPOSITORY: "yuchen0x1/CodeDoctor" }, ...overrides,
    }),
    mutations: () => calls.filter(([command, action]) => command === "release" && ["create", "edit", "delete", "upload"].includes(action)),
  };
}

for (const [left, right, expected] of [
  ["0.2.3", "0.2.2", 1], ["0.2.3", "0.2.3", 0], ["0.2.3", "0.2.4", -1],
  ["0.2.10", "0.2.9", 1], ["0.10.0", "0.9.99", 1], ["1.0.0", "0.99.99", 1],
  ["0.0.0", "0.0.1", -1], ["9007199254740993.0.0", "9007199254740992.0.0", 1],
]) {
  test(`stable SemVer compares ${left} against ${right} numerically`, () => {
    assert.equal(compareStableVersions(left, right), expected);
  });
}

test("stable SemVer rejects noncanonical, prerelease and metadata product versions", () => {
  for (const invalid of ["01.2.3", "0.02.3", "0.2.03", "0.2", "v0.2.3", "-1.2.3", "0.2.3-beta.1", "0.2.3+build", "0.2.3\n", " 0.2.3", "0.2.3 ", "0.2.3.4", "1e3.2.3", "", null]) {
    assert.throws(() => compareStableVersions(invalid, "0.2.3"), /canonical stable SemVer/);
    assert.throws(() => compareStableVersions("0.2.3", invalid), /canonical stable SemVer/);
  }
});

for (const version of ["0.2.3", "0.2.4", "0.10.0", "1.0.0"]) {
  test(`rejects candidate at or below official v${version} before draft creation`, () => {
    const mock = mockGitHub({ beforePages: [[stable(version)]] });
    assert.throws(() => mock.invoke(), /strictly newer/);
    assert.deepEqual(mock.mutations(), []);
  });
}

test("allows first release when the successful published inventory is empty", () => {
  const mock = mockGitHub({ beforePages: [[]] });
  assert.equal(mock.invoke(), "mock://release/v0.2.3");
  assert.deepEqual(mock.mutations().map((args) => args[1]), ["create", "edit"]);
});

test("publishes only a verified newer draft, checking the inventory on both sides of upload", () => {
  const mock = mockGitHub({ afterPages: [[stable("0.2.2"), { ...stable("0.2.3"), draft: true }]] });
  assert.equal(mock.invoke(), "mock://release/v0.2.3");
  assert.deepEqual(mock.calls.map((args) => args[0] === "api" ? args[2] : args[1]), [
    `${API}/tags/v0.2.3`, `${API}?per_page=100&page=1`, "create", "view",
    `${API}?per_page=100&page=1`, "edit", "view",
  ]);
  const [create, edit] = mock.mutations();
  assert(create.includes("--draft") && create.includes("--verify-tag"));
  assert.deepEqual(edit, ["release", "edit", "v0.2.3", "--draft=false", "--latest"]);
});

for (const version of ["0.2.3", "0.2.4"]) {
  test(`a newly published v${version} during upload prevents promotion`, () => {
    const mock = mockGitHub({ afterPages: [[stable(version)]] });
    assert.throws(() => mock.invoke(), /strictly newer/);
    assert.deepEqual(mock.mutations().map((args) => args[1]), ["create"]);
  });
}

test("checks every published version rather than date/order/latest designation", () => {
  const mock = mockGitHub({ beforePages: [[stable("0.2.2"), stable("1.0.0"), stable("0.1.1")]] });
  assert.throws(() => mock.invoke(), /published v1.0.0/);
  assert.deepEqual(mock.mutations(), []);
});

for (const stage of ["before", "after"]) {
  test(`checks later inventory pages ${stage} creating a draft`, () => {
    const first = Array.from({ length: 100 }, () => stable("0.1.0"));
    const mock = mockGitHub({ [`${stage}Pages`]: [first, [stable("0.2.4")]] });
    assert.throws(() => mock.invoke(), /published v0.2.4/);
    assert(mock.calls.some((args) => args[2] === `${API}?per_page=100&page=2`));
    assert.deepEqual(mock.mutations().map((args) => args[1]), stage === "before" ? [] : ["create"]);
  });
}

test("ignores draft and prerelease versions in the official stable comparison", () => {
  const mock = mockGitHub({ beforePages: [[
    stable("0.2.2"), { ...stable("9.0.0"), draft: true },
    { tag_name: "v10.0.0-rc.1", draft: false, prerelease: true },
  ]] });
  assert.equal(mock.invoke(), "mock://release/v0.2.3");
});

for (const draft of [false, true]) {
  test(`preserves an existing ${draft ? "draft" : "formal"} release and its assets`, () => {
    const mock = mockGitHub({ lookup: response(200, { ...stable("0.2.3"), draft }) });
    assert.throws(() => mock.invoke(), /already has a release or draft/);
    assert.deepEqual(mock.mutations(), []);
  });
}

test("does not duplicate a same-tag draft omitted by the tag lookup", () => {
  const mock = mockGitHub({ beforePages: [[stable("0.2.2"), { ...stable("0.2.3"), draft: true }]] });
  assert.throws(() => mock.invoke(), /already has a release or draft/);
  assert.deepEqual(mock.mutations(), []);
});

test("a malformed HTTP 200 null lookup is not equivalent to HTTP 404", () => {
  const mock = mockGitHub({ lookup: response(200, null) });
  assert.throws(() => mock.invoke(), /already has a release or draft/);
  assert.deepEqual(mock.mutations(), []);
});

for (const status of [401, 403, 429, 500, 503]) {
  test(`HTTP ${status} on target lookup is not an absent release`, () => {
    const mock = mockGitHub({ lookup: response(status, { message: "404 mentioned in an error is not the HTTP status" }) });
    assert.throws(() => mock.invoke(), new RegExp(`HTTP ${status}`));
    assert.deepEqual(mock.mutations(), []);
  });
}

for (const lookup of [
  { status: 1, stdout: "", stderr: "request failed (HTTP 404)" },
  { status: null, stdout: "", stderr: "", error: new Error("spawn gh ENOENT") },
  { status: null, stdout: "", stderr: "connection timeout" },
  { ...response(404, {}), status: null, signal: "SIGTERM", stderr: "terminated" },
]) {
  test(`unstructured/process/network lookup failure aborts (${lookup.error?.message ?? lookup.stderr})`, () => {
    const mock = mockGitHub({ lookup });
    assert.throws(() => mock.invoke(), /GitHub read failed/);
    assert.deepEqual(mock.mutations(), []);
  });
}

for (const stage of ["before", "after"]) {
  for (const status of [404, 429, 503]) {
    test(`HTTP ${status} reading the ${stage} inventory fails closed`, () => {
      const mock = mockGitHub({ [`${stage}Response`]: response(status, { message: "unavailable" }) });
      assert.throws(() => mock.invoke(), new RegExp(`HTTP ${status}`));
      assert.deepEqual(mock.mutations().map((args) => args[1]), stage === "before" ? [] : ["create"]);
    });
  }
}

for (const inventory of [null, {}, [{ tag_name: "v0.2.2" }], [{ ...stable("0.2.2"), tag_name: "v01.2.2" }], [{ ...stable("0.2.2"), tag_name: "unversioned" }]]) {
  test(`malformed official inventory aborts (${JSON.stringify(inventory)})`, () => {
    const mock = mockGitHub({ beforeResponse: response(200, inventory) });
    assert.throws(() => mock.invoke());
    assert.deepEqual(mock.mutations(), []);
  });
}

test("malformed JSON cannot turn a successful read into permission to publish", () => {
  const mock = mockGitHub({ beforeResponse: { ...response(200, []), stdout: "HTTP/1.1 200 OK\n\n{" } });
  assert.throws(() => mock.invoke(), SyntaxError);
  assert.deepEqual(mock.mutations(), []);
});

test("HTTP headers with LF line endings are accepted without weakening 404 classification", () => {
  const mock = mockGitHub({ lookup: response(404, { message: "Not Found" }, "\n"), beforeResponse: response(200, [], "\n") });
  assert.equal(mock.invoke(), "mock://release/v0.2.3");
});

for (const changed of [{ size: 41 }, { digest: `sha256:${"b".repeat(64)}` }, { state: "new" }]) {
  test(`retains remote asset validation before promotion (${Object.keys(changed)[0]})`, () => {
    const mock = mockGitHub({ draft: { assets: [{ ...asset, state: "uploaded", ...changed }] } });
    assert.throws(() => mock.invoke());
    assert.deepEqual(mock.mutations().map((args) => args[1]), ["create"]);
  });
}

test("draft creation failure aborts without retry, edit, deletion or upload", () => {
  const mock = mockGitHub({ createResponse: { status: 1, stdout: "", stderr: "create failed" } });
  assert.throws(() => mock.invoke(), /GitHub operation failed/);
  assert.deepEqual(mock.mutations().map((args) => args[1]), ["create"]);
});

test("repository and exact tag authorization remain mandatory before any gh command", () => {
  for (const env of [
    { GITHUB_REF: "refs/heads/main", GITHUB_REPOSITORY: "yuchen0x1/CodeDoctor" },
    { GITHUB_REF: "refs/tags/v0.2.2", GITHUB_REPOSITORY: "yuchen0x1/CodeDoctor" },
    { GITHUB_REF: "refs/tags/v0.2.3", GITHUB_REPOSITORY: "other/repository" },
  ]) {
    const mock = mockGitHub();
    assert.throws(() => mock.invoke({ env }));
    assert.deepEqual(mock.calls, []);
  }
});

test("workflow queues release runs without replacing pending runs and tests guards before builds", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/release.yml", import.meta.url), "utf8");
  assert.match(workflow, /^concurrency:\r?\n  group: codedoctor-release\r?\n  cancel-in-progress: false\r?\n  queue: max$/m);
  const sourceJob = workflow.slice(workflow.indexOf("  source:"), workflow.indexOf("  windows:"));
  assert.match(sourceJob, /run: node --test scripts\/release\/publish\.test\.mjs/);
});
