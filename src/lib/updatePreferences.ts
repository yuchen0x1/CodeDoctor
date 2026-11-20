export type UpdatePeriod = 0 | 1 | 7;

const periodKey = "codedoctor.update-period-days";
const lastCheckKey = "codedoctor.update-last-check";
const laterKey = "codedoctor.update-later";

function readPreference(key: string): string | null {
  try {
    const storage = globalThis.localStorage;
    return storage ? storage.getItem(key) : null;
  } catch {
    return null;
  }
}

function writePreference(key: string, value: string): void {
  try {
    const storage = globalThis.localStorage;
    if (storage) storage.setItem(key, value);
  } catch {
    // Preferences are optional when storage is unavailable.
  }
}

export function isUpdatePeriod(value: unknown): value is UpdatePeriod {
  return value === 0 || value === 1 || value === 7;
}

export function readUpdatePeriod(): UpdatePeriod {
  const stored = readPreference(periodKey);
  const period = stored === null ? 0 : Number(stored);
  return isUpdatePeriod(period) ? period : 0;
}

export function writeUpdatePeriod(period: unknown): void {
  if (isUpdatePeriod(period)) writePreference(periodKey, String(period));
}

export function readLastUpdateCheckAt(now = Date.now()): number | null {
  const stored = readPreference(lastCheckKey);
  if (stored === null || !/^\d+$/.test(stored)) return null;

  const timestamp = Number(stored);
  if (!Number.isSafeInteger(timestamp) || timestamp > now) return null;
  return timestamp;
}

export function writeLastUpdateCheckAt(timestamp: number): void {
  if (Number.isSafeInteger(timestamp) && timestamp >= 0) {
    writePreference(lastCheckKey, String(timestamp));
  }
}

export function readDeferredUpdateVersion(): string | null {
  return readPreference(laterKey);
}

export function writeDeferredUpdateVersion(version: string): void {
  writePreference(laterKey, version);
}
