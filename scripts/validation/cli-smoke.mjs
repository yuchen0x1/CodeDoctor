import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const executable = resolve(process.argv[2]);
const run = (args) => spawnSync(executable, args, { encoding: "utf8" });
const version = run(["--version"]);
assert.equal(version.status, 0, version.stderr);
assert.match(version.stdout, /^CodeDoctor CLI \d+\.\d+\.\d+/);
const sarif = run(["scan", "fixtures", "--format", "sarif", "--fail-on", "high"]);
assert.equal(sarif.status, 2, sarif.stderr);
const report = JSON.parse(sarif.stdout);
assert.equal(report.version, "2.1.0");
const { tool: { driver }, results } = report.runs[0];
assert.equal(driver.name, "CodeDoctor");
assert(results.some((finding) => finding.level === "error"));
assert(results.some((finding) => (finding.locations?.[0]?.physicalLocation.region?.startLine ?? 0) > 0));
for (const finding of results) {
  assert(Number.isInteger(finding.ruleIndex));
  assert.equal(driver.rules[finding.ruleIndex]?.id, finding.ruleId);
  for (const { physicalLocation } of finding.locations ?? []) {
    const { uri } = physicalLocation.artifactLocation;
    assert(!uri.startsWith("/") && !uri.includes(":") && !uri.includes("\\"));
    assert(!uri.split("/").some((part) => ["", ".", ".."].includes(part)));
  }
}
const bad = run(["scan", "fixtures/non-existent-directory"]);
assert.equal(bad.status, 1);
assert.equal(bad.stdout, "");
console.log(`CLI verified ${report.runs[0].results.length} findings and the quality gate exit code`);
