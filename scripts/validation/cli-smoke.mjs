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
const [runReport] = report.runs;
const { tool: { driver }, results } = runReport;
assert.equal(driver.name, "CodeDoctor");
assert.equal(runReport.invocations[0].executionSuccessful, true, "Gate failure is not an analysis failure");
assert.equal(typeof runReport.properties.ruleVersion, "string");
assert(results.some((finding) => finding.level === "error"));
assert(results.some((finding) => finding.locations?.[0]?.physicalLocation.region?.startLine > 0));
for (const finding of results) {
  assert(Number.isInteger(finding.ruleIndex));
  assert.equal(driver.rules[finding.ruleIndex]?.id, finding.ruleId);
  for (const { physicalLocation: { artifactLocation: { uri }, region } } of finding.locations ?? []) {
    assert(!uri.includes(":") && !uri.includes("\\"));
    assert(!decodeURIComponent(uri).split("/").some((part) => ["", ".", ".."].includes(part)));
    if (region) assert(Number.isInteger(region.startLine) && region.startLine > 0);
  }
}
const bad = run(["scan", "fixtures/non-existent-directory"]);
assert.equal(bad.status, 1);
assert.equal(bad.stdout, "");
console.log(`CLI verified ${report.runs[0].results.length} findings and the quality gate exit code`);
