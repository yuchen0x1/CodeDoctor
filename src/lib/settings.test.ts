import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, parseSettings, type AppSettings } from "./settings";

describe("application settings", () => {
  it("uses local scan defaults when no valid preference exists", () => {
    expect(parseSettings(null)).toEqual(DEFAULT_SETTINGS);
  });

  it("keeps supported thresholds and rejects invalid exclusion path fragments", () => {
    const settings = parseSettings(JSON.stringify({
      theme: "light",
      excludedDirectories: ["generated", "..", "nested/cache", "vendor"],
      enabledAnalyzers: ["ruff", "unknown"],
      thresholds: { maxFunctionLines: 80, maxNesting: -1 },
    }));

    expect(settings.theme).toBe("light");
    expect(settings.excludedDirectories).toEqual(["generated", "vendor"]);
    expect(settings.enabledAnalyzers).toEqual(["ruff"]);
    expect(settings.thresholds.maxFunctionLines).toBe(80);
    expect(settings.thresholds.maxNesting).toBe(DEFAULT_SETTINGS.thresholds.maxNesting);
  });

  it("round trips a complete preference set", () => {
    const settings: AppSettings = {
      ...DEFAULT_SETTINGS,
      theme: "light",
      excludedDirectories: ["generated"],
      enabledAnalyzers: ["ruff"],
      thresholds: { ...DEFAULT_SETTINGS.thresholds, maxCyclomatic: 18 },
    };
    expect(parseSettings(JSON.stringify(settings))).toEqual(settings);
  });
});
