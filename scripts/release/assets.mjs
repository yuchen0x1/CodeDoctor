import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readdir, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [root, version] = process.argv.slice(2);
if (!root || !/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  throw new Error("Usage: node scripts/release/assets.mjs <directory> <version>");
}
const names = (await readdir(root)).filter((name) => name !== "SHA256SUMS.txt").sort();
const nsis = `CodeDoctor_${version}_x64-setup.exe`;
const msi = `CodeDoctor_${version}_x64_en-US.msi`;
const required = [nsis, msi, `${nsis}.sig`, `${msi}.sig`, "codedoctor.exe", "latest.json",
  "THIRD-PARTY-NOTICES-Windows.txt", `CodeDoctor_${version}_linux_x64-cli.tar.gz`].sort();
if (JSON.stringify(names) !== JSON.stringify(required)) throw new Error("Release assets do not match the expected inventory");

const assets = [];
for (const name of names) {
  if (!/^[A-Za-z0-9._-]+$/.test(name)) throw new Error(`Invalid asset name ${name}`);
  const path = join(root, name);
  const info = await stat(path);
  if (!info.isFile() || info.size === 0) throw new Error(`Missing asset data for ${name}`);
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  assets.push(`${hash.digest("hex")}  ${name}`);
}

await writeFile(join(root, "SHA256SUMS.txt"), assets.join("\n") + "\n");
