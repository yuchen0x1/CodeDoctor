import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [directory, version] = process.argv.slice(2);
assert(directory && /^\d+\.\d+\.\d+$/.test(version ?? ""), "Missing release inputs");
const tag = `v${version}`;
assert.equal(process.env.GITHUB_REF, `refs/tags/${tag}`);
assert.equal(process.env.GITHUB_REPOSITORY, "yuchen0x1/CodeDoctor");
const gh = (args) => {
  const result = spawnSync("gh", args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`GitHub operation failed: ${result.stderr.trim()}`);
  return result.stdout;
};

const allNotes = await readFile("docs/releases/release-notes.md", "utf8");
const notes = allNotes.split(/^## /m).slice(1).find((section) => section.startsWith(`${version}（`));
assert(notes, `Missing release notes for ${tag}`);
const notesPath = join(process.env.RUNNER_TEMP, "codedoctor-release-notes.md");
await writeFile(notesPath, `## ${notes.trim()}\n\nWindows x64 EXE、MSI 和免安装版，以及 Linux x64 CLI 归档见附件。使用 SHA256SUMS.txt 校验下载。Windows 安装包附带 Tauri 更新签名。Windows Authenticode 状态及各平台验证见仓库维护文档。\n`);

const names = await readdir(directory);
const existing = spawnSync("gh", ["release", "view", tag, "--json", "isDraft"], { encoding: "utf8" });
assert.notEqual(existing.status, 0, `${tag} already has a release or draft. Inspect it before retrying.`);
gh(["release", "create", tag, ...names.map((name) => join(directory, name)),
  "--draft", "--verify-tag", "--title", `CodeDoctor ${tag}`, "--notes-file", notesPath]);

const view = () => JSON.parse(gh(["release", "view", tag, "--json", "tagName,isDraft,url,assets"]));
const draft = view();
assert(draft.isDraft && draft.tagName === tag);
assert.equal(draft.assets.length, names.length);
for (const name of names) {
  const asset = draft.assets.find((item) => item.name === name);
  const path = join(directory, name);
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  assert.equal(asset?.state, "uploaded", name);
  assert.equal(asset.size, (await stat(path)).size, name);
  assert.equal(asset.digest, `sha256:${hash.digest("hex")}`, name);
}

gh(["release", "edit", tag, "--draft=false", "--latest"]);
const published = view();
assert(!published.isDraft && published.tagName === tag);
console.log(published.url);
