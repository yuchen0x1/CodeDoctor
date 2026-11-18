import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");

const { version } = JSON.parse(read("package.json"));
const { version: tauriVersion } = JSON.parse(read("src-tauri/tauri.conf.json"));
const cargoVersion = read("src-tauri/Cargo.toml").match(/^version = "([^"\r\n]+)"/m)?.[1];
const lockedVersion = read("src-tauri/Cargo.lock").match(/^name = "codedoctor"\r?\nversion = "([^"\r\n]+)"/m)?.[1];
const tag = process.env.RELEASE_TAG;

if (typeof version !== "string" || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
  throw new Error("Expected a numeric major.minor.patch product version");
}
if ([tauriVersion, cargoVersion, lockedVersion].some((candidate) => candidate !== version)) {
  throw new Error(`Version mismatch in ${fileURLToPath(root)}`);
}
if (tag && tag !== `v${version}`) throw new Error(`Expected tag v${version}, received ${tag}`);

process.stdout.write(version + "\n");
