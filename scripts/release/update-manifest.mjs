import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [directory, version] = process.argv.slice(2);
assert(directory && /^\d+\.\d+\.\d+$/.test(version ?? ""));
const history = await readFile("docs/releases/release-notes.md", "utf8");
const notes = history.split(/^## /m).slice(1).find((section) => section.startsWith(`${version}（`));
assert(notes, "Release notes are missing");

const manifest = {
  version,
  pub_date: new Date().toISOString(),
  notes: notes.trim().split("\n").slice(1).join("\n").trim(),
  channel: "stable",
  mandatory: false,
  minimum_client_version: "0.2.0",
  minimum_system: "Windows 10 22H2 or Windows 11, x64",
  platforms: {},
};

const packages = {
  "windows-x86_64-nsis": `CodeDoctor_${version}_x64-setup.exe`,
  "windows-x86_64-msi": `CodeDoctor_${version}_x64_en-US.msi`,
};
for (const [target, name] of Object.entries(packages)) {
  const data = await readFile(join(directory, name));
  const signature = (await readFile(join(directory, `${name}.sig`), "utf8")).trim();
  const trusted = Buffer.from(signature, "base64").toString("utf8").split(/\r?\n/)[2];
  assert(trusted?.split("\t").includes(`version:${version}`), `Signature version for ${name}`);
  manifest.platforms[target] = {
    signature,
    url: `https://github.com/yuchen0x1/CodeDoctor/releases/download/v${version}/${name}`,
    size: data.byteLength,
    sha256: createHash("sha256").update(data).digest("hex"),
  };
}

await writeFile(join(directory, "latest.json"), JSON.stringify(manifest, null, 2) + "\n");
