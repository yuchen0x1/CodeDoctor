import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");

const { version } = JSON.parse(read("package.json"));
const { version: tauriVersion } = JSON.parse(read("src-tauri/tauri.conf.json"));
const cargoVersion = read("src-tauri/Cargo.toml").match(/^version = "([^"\r\n]+)"/m)?.[1];
const lockedVersion = read("src-tauri/Cargo.lock").match(/^name = "codedoctor"\r?\nversion = "([^"\r\n]+)"/m)?.[1];
const readme = read("README.md");
const releaseNotes = read("docs/releases/release-notes.md");
const tag = process.env.RELEASE_TAG;

if (typeof version !== "string" || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
  throw new Error("Expected a numeric major.minor.patch product version");
}
if ([tauriVersion, cargoVersion, lockedVersion].some((candidate) => candidate !== version)) {
  throw new Error(`Version mismatch in ${fileURLToPath(root)}`);
}
const latestReadmeRelease = readme.match(/^### ([^\r\n]+)/m)?.[1];
const latestNotesRelease = releaseNotes.match(/^## ([^\r\n]+)/m)?.[1];
if (
  !latestReadmeRelease?.startsWith(`${version}（`) ||
  !latestNotesRelease?.startsWith(`${version}（`) ||
  !readme.includes(`CodeDoctor_${version}_x64-setup.exe`) ||
  !readme.includes(`CodeDoctor_${version}_x64_en-US.msi`)
) {
  throw new Error("README and release notes must describe the current product version");
}
if (tag && tag !== `v${version}`) throw new Error(`Expected tag v${version}, received ${tag}`);

process.stdout.write(version + "\n");
