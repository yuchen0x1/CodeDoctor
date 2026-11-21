import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY = "yuchen0x1/CodeDoctor";
const RELEASES_API = `repos/${REPOSITORY}/releases`;
const NOT_FOUND = Symbol("GitHub HTTP 404");
const defaultRunGh = (args) => spawnSync("gh", args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });

function stableVersion(version) {
  // Product versions intentionally exclude prerelease/build suffixes, as version.mjs does.
  const match = typeof version === "string" && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  assert(match && match[0] === version, `Expected a canonical stable SemVer, received ${JSON.stringify(version)}`);
  return match.slice(1).map((part) => BigInt(part));
}

export function compareStableVersions(left, right) {
  const a = stableVersion(left);
  const b = stableVersion(right);
  for (let index = 0; index < a.length; index++) {
    if (a[index] !== b[index]) return a[index] > b[index] ? 1 : -1;
  }
  return 0;
}

function gh(args, runGh) {
  const result = runGh(args);
  if (result.error || result.status !== 0) {
    throw new Error(`GitHub operation failed: ${result.error?.message ?? result.stderr?.trim() ?? result.status}`);
  }
  return result.stdout;
}

function apiJson(endpoint, runGh, allowNotFound = false) {
  // Use the actual HTTP status, never a generic gh failure or an error-message substring.
  const result = runGh(["api", "--include", endpoint]);
  const output = result.stdout ?? "";
  const status = /^HTTP\/[\d.]+\s+(\d{3})[^\r\n]*\r?\n/.exec(output)?.[1];
  if (!result.error && result.status === 1 && status === "404" && allowNotFound) return NOT_FOUND;
  if (result.error || result.status !== 0) {
    throw new Error(`GitHub read failed (HTTP ${status ?? "unknown"}): ${result.error?.message ?? result.stderr?.trim() ?? result.status}`);
  }
  assert.equal(status, "200", "GitHub read did not return HTTP 200");
  const separator = /\r?\n\r?\n/.exec(output);
  assert(separator, "GitHub response headers are missing");
  return JSON.parse(output.slice(separator.index + separator[0].length));
}

function assertNewerThanPublished(version, runGh, allowCurrentDraft = false) {
  // Check every published stable release, not GitHub's mutable 'latest' designation.
  // Paginate explicitly so an older/higher version cannot hide beyond the first page.
  for (let page = 1; ; page++) {
    const releases = apiJson(`${RELEASES_API}?per_page=100&page=${page}`, runGh);
    assert(Array.isArray(releases), "Expected a GitHub releases array");
    for (const release of releases) {
      assert(release && typeof release.draft === "boolean" && typeof release.prerelease === "boolean", "Invalid GitHub release metadata");
      if (!allowCurrentDraft && (release.draft || release.prerelease)) {
        // The tag endpoint can omit drafts; also check the authenticated inventory.
        assert.notEqual(release.tag_name, `v${version}`, `v${version} already has a release or draft. Inspect it before retrying.`);
      }
      if (release.draft || release.prerelease) continue;
      assert(typeof release.tag_name === "string" && release.tag_name.startsWith("v"), "Invalid published release tag");
      assert(compareStableVersions(version, release.tag_name.slice(1)) > 0,
        `Refusing v${version}: it must be strictly newer than published ${release.tag_name}`);
    }
    if (releases.length < 100) return;
  }
}

function requireReleaseContext(version, env) {
  stableVersion(version);
  assert.equal(env.GITHUB_REF, `refs/tags/v${version}`);
  assert.equal(env.GITHUB_REPOSITORY, REPOSITORY);
}

export function publishRelease({ directory, version, notesPath, assets, runGh = defaultRunGh, env = process.env }) {
  requireReleaseContext(version, env);
  const tag = `v${version}`;
  const existing = apiJson(`${RELEASES_API}/tags/${tag}`, runGh, true);
  assert.equal(existing, NOT_FOUND, `${tag} already has a release or draft. Inspect it before retrying.`);
  assertNewerThanPublished(version, runGh);
  gh(["release", "create", tag, ...assets.map(({ name }) => join(directory, name)),
    "--draft", "--verify-tag", "--title", `CodeDoctor ${tag}`, "--notes-file", notesPath], runGh);

  const view = () => JSON.parse(gh(["release", "view", tag, "--json", "tagName,isDraft,url,assets"], runGh));
  const draft = view();
  assert(draft.isDraft && draft.tagName === tag);
  assert.equal(draft.assets.length, assets.length);
  for (const expected of assets) {
    const asset = draft.assets.find((item) => item.name === expected.name);
    assert.equal(asset?.state, "uploaded", expected.name);
    assert.equal(asset.size, expected.size, expected.name);
    assert.equal(asset.digest, expected.digest, expected.name);
  }

  // A newer release may have appeared while uploading/verifying the draft.
  // The workflow also serializes publication across all version tags.
  assertNewerThanPublished(version, runGh, true);
  gh(["release", "edit", tag, "--draft=false", "--latest"], runGh);
  const published = view();
  assert(!published.isDraft && published.tagName === tag);
  return published.url;
}

async function main() {
  const [directory, version] = process.argv.slice(2);
  assert(directory, "Missing release directory");
  requireReleaseContext(version, process.env);
  const allNotes = await readFile("docs/releases/release-notes.md", "utf8");
  const notes = allNotes.split(/^## /m).slice(1).find((section) => section.startsWith(`${version}（`));
  assert(notes, `Missing release notes for v${version}`);
  const notesPath = join(process.env.RUNNER_TEMP, "codedoctor-release-notes.md");
  await writeFile(notesPath, `## ${notes.trim()}\n\nWindows x64 EXE、MSI 和免安装版，以及 Linux x64 CLI 归档见附件。使用 SHA256SUMS.txt 校验下载。Windows 安装包附带 Tauri 更新签名。Windows Authenticode 状态及各平台验证见仓库维护文档。\n`);

  const assets = [];
  for (const name of await readdir(directory)) {
    const path = join(directory, name);
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(path)) hash.update(chunk);
    assets.push({ name, size: (await stat(path)).size, digest: `sha256:${hash.digest("hex")}` });
  }
  console.log(publishRelease({ directory, version, notesPath, assets }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
