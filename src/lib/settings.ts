export interface ScanThresholds {
  maxLineLength: number;
  maxFunctionLines: number;
  maxCyclomatic: number;
  maxCognitive: number;
  maxNesting: number;
  maxParameters: number;
  maxClassMethods: number;
  maxClassLines: number;
  maxFileLines: number;
  duplicateMinLines: number;
}

export interface AppSettings {
  theme: "dark" | "light";
  excludedDirectories: string[];
  enabledAnalyzers: string[];
  thresholds: ScanThresholds;
}

export const SETTINGS_STORAGE_KEY = "codedoctor.settings.v1";

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  excludedDirectories: [],
  enabledAnalyzers: [],
  thresholds: {
    maxLineLength: 120,
    maxFunctionLines: 60,
    maxCyclomatic: 15,
    maxCognitive: 25,
    maxNesting: 4,
    maxParameters: 5,
    maxClassMethods: 20,
    maxClassLines: 300,
    maxFileLines: 600,
    duplicateMinLines: 6,
  },
};

export const thresholdRanges: Record<keyof ScanThresholds, readonly [number, number]> = {
  maxLineLength: [40, 1000],
  maxFunctionLines: [5, 1000],
  maxCyclomatic: [1, 200],
  maxCognitive: [1, 500],
  maxNesting: [1, 32],
  maxParameters: [1, 100],
  maxClassMethods: [1, 500],
  maxClassLines: [20, 10000],
  maxFileLines: [50, 100000],
  duplicateMinLines: [3, 200],
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseExcludedDirectories(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const names = new Map<string, string>();
  for (const item of value) {
    if (typeof item !== "string") continue;
    const name = item.trim();
    if (!name || name.length > 128 || name === "." || name === "..") continue;
    if (/[\\/:\u0000-\u001f]/.test(name)) continue;
    const key = name.toLocaleLowerCase();
    if (!names.has(key)) names.set(key, name);
  }
  return [...names.values()];
}

function parseThresholds(value: unknown): ScanThresholds {
  if (!isRecord(value)) return { ...DEFAULT_SETTINGS.thresholds };
  const result = { ...DEFAULT_SETTINGS.thresholds };
  for (const key of Object.keys(thresholdRanges) as (keyof ScanThresholds)[]) {
    const candidate = value[key];
    if (typeof candidate !== "number" || !Number.isFinite(candidate)) continue;
    const [minimum, maximum] = thresholdRanges[key];
    if (Number.isInteger(candidate) && candidate >= minimum && candidate <= maximum) {
      result[key] = candidate;
    }
  }
  return result;
}

export function parseSettings(raw: string | null): AppSettings {
  if (!raw) return { ...DEFAULT_SETTINGS, thresholds: { ...DEFAULT_SETTINGS.thresholds } };
  try {
    const value: unknown = JSON.parse(raw);
    if (!isRecord(value)) return { ...DEFAULT_SETTINGS, thresholds: { ...DEFAULT_SETTINGS.thresholds } };
    return {
      theme: value.theme === "light" ? "light" : "dark",
      excludedDirectories: parseExcludedDirectories(value.excludedDirectories),
      enabledAnalyzers: Array.isArray(value.enabledAnalyzers)
        ? [...new Set(value.enabledAnalyzers.filter((item): item is string => item === "ruff"))]
        : [],
      thresholds: parseThresholds(value.thresholds),
    };
  } catch {
    return { ...DEFAULT_SETTINGS, thresholds: { ...DEFAULT_SETTINGS.thresholds } };
  }
}

export function readSettings(): AppSettings {
  try {
    return parseSettings(typeof localStorage === "undefined" ? null : localStorage.getItem(SETTINGS_STORAGE_KEY));
  } catch {
    return parseSettings(null);
  }
}

export function writeSettings(settings: AppSettings): void {
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
    }
  } catch {
    // Storage can be unavailable in restricted webview profiles.
  }
}
