import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  readDeferredUpdateVersion,
  readLastUpdateCheckAt,
  readUpdatePeriod,
  writeDeferredUpdateVersion,
  writeLastUpdateCheckAt,
  writeUpdatePeriod,
} from "./updatePreferences";

const periodKey = "codedoctor.update-period-days";
const lastCheckKey = "codedoctor.update-last-check";
const laterKey = "codedoctor.update-later";

let originalStorage: PropertyDescriptor | undefined;

function setStorage(storage: Storage): void {
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: storage,
  });
}

function setValues(values: Record<string, string>): void {
  setStorage({
    getItem: (key: string) => values[key] ?? null,
    setItem: (key: string, value: string) => { values[key] = value; },
  } as Storage);
}

beforeEach(() => {
  originalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
});

afterEach(() => {
  if (originalStorage) {
    Object.defineProperty(globalThis, "localStorage", originalStorage);
  } else {
    Reflect.deleteProperty(globalThis, "localStorage");
  }
});

describe("update preferences", () => {
  it.each([
    ["0", 0],
    ["1", 1],
    ["7", 7],
    ["2", 0],
    ["-1", 0],
    ["invalid", 0],
  ])("accepts only supported check periods (%s)", (stored, expected) => {
    setValues({ [periodKey]: stored });
    expect(readUpdatePeriod()).toBe(expected);
  });

  it.each([
    ["not-a-timestamp", null],
    ["", null],
    ["1.5", null],
    ["9007199254740992", null],
    ["101", null],
    ["100", 100],
  ])("validates last-check timestamp %s", (stored, expected) => {
    setValues({ [lastCheckKey]: stored });
    expect(readLastUpdateCheckAt(100)).toBe(expected);
  });

  it("round trips update preferences through localStorage", () => {
    const values: Record<string, string> = {};
    setValues(values);

    writeUpdatePeriod(7);
    writeLastUpdateCheckAt(100);
    writeDeferredUpdateVersion("2.0.0");

    expect(values).toEqual({
      [periodKey]: "7",
      [lastCheckKey]: "100",
      [laterKey]: "2.0.0",
    });
    expect(readUpdatePeriod()).toBe(7);
    expect(readLastUpdateCheckAt(100)).toBe(100);
    expect(readDeferredUpdateVersion()).toBe("2.0.0");
  });

  it("treats a throwing localStorage getter as unavailable", () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() { throw new Error("storage getter failed"); },
    });

    expect(readUpdatePeriod()).toBe(0);
    expect(readLastUpdateCheckAt(100)).toBeNull();
    expect(readDeferredUpdateVersion()).toBeNull();
    expect(() => writeUpdatePeriod(1)).not.toThrow();
    expect(() => writeLastUpdateCheckAt(100)).not.toThrow();
    expect(() => writeDeferredUpdateVersion("2.0.0")).not.toThrow();
  });

  it("treats throwing localStorage methods as unavailable", () => {
    setStorage({
      getItem() { throw new Error("storage read failed"); },
      setItem() { throw new Error("storage write failed"); },
    } as unknown as Storage);

    expect(readUpdatePeriod()).toBe(0);
    expect(readLastUpdateCheckAt(100)).toBeNull();
    expect(readDeferredUpdateVersion()).toBeNull();
    expect(() => writeUpdatePeriod(1)).not.toThrow();
    expect(() => writeLastUpdateCheckAt(100)).not.toThrow();
    expect(() => writeDeferredUpdateVersion("2.0.0")).not.toThrow();
  });
});
