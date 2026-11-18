import { spawnSync } from "node:child_process";
import { readFile, readdir, realpath, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const [destination, target, flavor] = process.argv.slice(2);
if (!destination || !target || !["desktop", "cli"].includes(flavor)) {
  throw new Error("Usage: licenses.mjs <destination> <rust-target> <desktop|cli>");
}

const cargo = spawnSync("cargo", ["metadata", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1", "--locked", "--filter-platform", target,
  "--no-default-features", "--features", flavor], { maxBuffer: 32 * 1024 * 1024, encoding: "utf8" });
if (cargo.status !== 0) throw new Error(cargo.stderr);
const metadata = JSON.parse(cargo.stdout);
const components = metadata.packages.filter((p) => p.source && metadata.resolve.nodes.some((node) => node.id === p.id))
  .map((p) => ({ name: p.name, version: p.version, license: p.license, licenseFile: p.license_file, directory: dirname(p.manifest_path) }));
if (flavor === "desktop") {
  // The webview bundle depends on the four declared production packages and React Scheduler.
  const rootPackage = JSON.parse(await readFile("package.json", "utf8"));
  for (const name of Object.keys(rootPackage.dependencies)) {
    const directory = join("node_modules", name);
    const pkg = JSON.parse(await readFile(join(directory, "package.json"), "utf8"));
    components.push({ name, version: pkg.version, license: pkg.license, directory });
    if (name === "react-dom") {
      const resolved = await realpath(directory);
      const scheduler = join(dirname(resolved), "scheduler");
      const p = JSON.parse(await readFile(join(scheduler, "package.json"), "utf8"));
      components.push({ name: p.name, version: p.version, license: p.license, directory: scheduler });
    }
  }
}

components.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const sections = ["CodeDoctor third-party notices", `Target: ${target} (${flavor})`, (await readFile("LICENSE", "utf8")).trim(),
  "The following sections reproduce license and notice files distributed by dependencies."];
for (const { name, version, license, licenseFile, directory } of components) {
  if (!license && !licenseFile) throw new Error(`Missing license declaration for ${name}@${version}`);
  sections.push(`\n${name} ${version} — ${license ?? "See license file"}`);
  const entries = await readdir(directory, { withFileTypes: true });
  const candidates = new Set(entries.filter((entry) => entry.isFile() && /^(licen[cs]e|notice|copying|copyright|unlicense)([-_.]|$)/i.test(entry.name)).map((entry) => entry.name));
  if (licenseFile) candidates.add(licenseFile);
  for (const file of [...candidates].sort()) {
    sections.push(`[${file}]\n${(await readFile(join(directory, file), "utf8")).trim()}`);
  }
}

await writeFile(destination, sections.join("\n\n") + "\n");
