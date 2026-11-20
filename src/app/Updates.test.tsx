// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Updates from "./Updates";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => true }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "0.2.1" }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));

let container: HTMLDivElement;
let root: Root;
let originalStorage: PropertyDescriptor | undefined;
let originalShowModal: PropertyDescriptor | undefined;
let originalClose: PropertyDescriptor | undefined;

beforeEach(() => {
  vi.resetAllMocks();
  originalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  vi.stubGlobal("localStorage", {
    getItem: () => null,
    setItem: () => { throw new DOMException("Full storage", "QuotaExceededError"); },
  });
  originalShowModal = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, "showModal");
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value(this: HTMLDialogElement) { this.open = true; },
  });
  originalClose = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, "close");
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value(this: HTMLDialogElement) { this.open = false; },
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});
afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  if (originalShowModal) Object.defineProperty(HTMLDialogElement.prototype, "showModal", originalShowModal);
  else Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  if (originalClose) Object.defineProperty(HTMLDialogElement.prototype, "close", originalClose);
  else Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
  if (originalStorage) Object.defineProperty(globalThis, "localStorage", originalStorage);
  else Reflect.deleteProperty(globalThis, "localStorage");
});
const mount = () => act(async () => { root.render(<Updates visible scanActive={false} />); });
const click = (text: string) => act(async () => {
  const button = [...container.querySelectorAll("button")].find((item) => item.textContent === text);
  if (!button) throw new Error(`Button not found: ${text}`);
  button.click();
});

describe("updates with unavailable preference storage", () => {
  it("mounts and checks even if accessing localStorage throws", async () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get: () => { throw new DOMException("Denied storage", "SecurityError"); },
    });
    vi.mocked(invoke).mockResolvedValue(null);
    await mount();
    await click("检查更新");
    expect(invoke).toHaveBeenCalledWith("check_for_update");
    expect(container.textContent).toContain("你已使用最新版本。");
  });

  it("does not get stuck checking after a quota failure and remains retryable", async () => {
    vi.mocked(invoke).mockRejectedValueOnce({ Validation: "Temporary failure" }).mockResolvedValue(null);
    await mount();
    await click("检查更新");
    expect(container.textContent).toContain("Temporary failure");
    expect(container.textContent).not.toContain("正在检查…");
    await click("检查更新");
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(container.textContent).toContain("你已使用最新版本。");
  });

  it("can dismiss an update and check again when remembering the dismissal fails", async () => {
    vi.mocked(invoke).mockResolvedValue({ version: "0.2.2", notes: "test", publishedAt: "2026-11-20", sizeBytes: 100, canInstall: false });
    await mount();
    await click("检查更新");
    expect(container.querySelector("dialog")?.open).toBe(true);
    await click("稍后更新");
    expect(container.querySelector("dialog")).toBeNull();
    await click("检查更新");
    expect(container.querySelector("dialog")?.open).toBe(true);
  });

  it("labels its dialog, focuses its title and returns focus after Escape", async () => {
    vi.mocked(invoke).mockResolvedValue({ version: "0.2.2", notes: "Long release notes\n".repeat(100), publishedAt: "2026-11-20", sizeBytes: 100, canInstall: false });
    await mount();
    const trigger = [...container.querySelectorAll("button")].find((button) => button.textContent === "检查更新")!;
    trigger.focus();
    await click("检查更新");
    const modal = container.querySelector("dialog")!;
    const heading = container.querySelector("dialog h2")!;
    expect(document.activeElement).toBe(heading);
    expect(modal.getAttribute("aria-labelledby")).toBe(heading.id);
    expect(modal.getAttribute("aria-describedby")!.split(" ").every((id) => document.getElementById(id))).toBe(true);
    expect(modal.querySelector<HTMLPreElement>('[aria-label="发布说明"]')?.tabIndex).toBe(0);
    const cancel = new Event("cancel", { bubbles: false, cancelable: true });
    await act(async () => modal.dispatchEvent(cancel));
    expect(cancel.defaultPrevented).toBe(true);
    expect(container.querySelector("dialog")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("keeps installation modal open during handoff and exposes a retry after failure", async () => {
    let rejectInstall!: (reason: unknown) => void;
    const pending = new Promise((_, reject) => { rejectInstall = reject; });
    vi.mocked(invoke).mockImplementation((command) => command === "install_update" ? pending : Promise.resolve({
      version: "0.2.2", notes: "notes", publishedAt: "2026-11-20", sizeBytes: 100, canInstall: true,
    }));
    await mount();
    await click("检查更新");
    await click("立即更新");
    const modal = container.querySelector("dialog")!;
    const progress = modal.querySelector("progress")!;
    expect(progress.getAttribute("aria-label")).toBe("更新下载进度");
    expect(progress.hasAttribute("value")).toBe(false);
    expect(document.getElementById(progress.getAttribute("aria-describedby")!)?.textContent).toContain("正在连接 GitHub");
    expect([...modal.querySelectorAll("button")].every((button) => button.disabled)).toBe(true);
    const cancel = new Event("cancel", { cancelable: true });
    await act(async () => modal.dispatchEvent(cancel));
    expect(modal.open).toBe(true);
    await act(async () => rejectInstall({ Validation: "Verification failed" }));
    expect(modal.querySelector('[role="alert"]')?.textContent).toContain("Verification failed");
    expect([...modal.querySelectorAll("button")].every((button) => !button.disabled)).toBe(true);
    expect(modal.querySelector("progress")).toBeNull();
  });
});
