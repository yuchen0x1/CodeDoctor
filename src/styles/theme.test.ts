// @ts-expect-error Node types are not part of the frontend compilation target.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const tokens = readFileSync(new URL("./tokens.css", import.meta.url), "utf8") as string;
const app = readFileSync(new URL("./app.css", import.meta.url), "utf8") as string;
const architecture = readFileSync(new URL("../app/ArchitectureView.css", import.meta.url), "utf8") as string;

const [darkBlock, lightBlock] = tokens.split(':root[data-theme="light"]');
const declarations = (block: string) => Object.fromEntries(
  [...block.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((match) => [match[1], match[2].trim()]),
);
const dark = declarations(darkBlock);
const light = declarations(lightBlock);
const luminance = (hex: string) => {
  const rgb = hex.replace("#", "").match(/../g)!.map((value) => parseInt(value, 16) / 255)
    .map((value) => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
  return .2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2];
};
const contrast = (foreground: string, background: string) => {
  const values = [luminance(foreground), luminance(background)].sort((a, b) => b - a);
  return (values[0] + .05) / (values[1] + .05);
};

describe("semantic theme contract", () => {
  it("maps every theme-dependent token in both themes", () => {
    const themed = Object.keys(dark).filter((key) => key.startsWith("--color-") || key.startsWith("--shadow-"));
    expect(Object.keys(light).sort()).toEqual(themed.sort());
  });

  it("defines every component token and keeps raw colors out of component styles", () => {
    for (const css of [app, architecture]) {
      const used = [...css.matchAll(/var\((--[\w-]+)/g)].map((match) => match[1]);
      expect(used.filter((name) => !(name in dark))).toEqual([]);
      expect(css).not.toMatch(/#[\da-f]{3,8}\b|\brgba?\(/i);
    }
  });

  it.each(["dark", "light"])("keeps text, severity, status and graph contrasts readable in %s", (theme) => {
    const values = theme === "dark" ? dark : { ...dark, ...light };
    const textPairs: [string, string][] = [];
    for (const surface of ["bg", "bg-raised", "panel", "panel-muted", "control"]) {
      for (const text of ["text", "text-muted", "text-faint", "accent"]) textPairs.push([text, surface]);
    }
    for (const severity of ["critical", "high", "medium", "low", "info"]) {
      textPairs.push([`severity-${severity}`, `severity-${severity}-bg`]);
    }
    textPairs.push(["on-accent", "accent"], ["on-accent", "accent-strong"], ["code-text", "code-bg"], ["ok", "ok-bg"], ["error", "error-bg"], ["warning", "warning-bg"], ["text", "accent-soft"]);
    for (const [text, surface] of textPairs) {
      expect(contrast(values[`--color-${text}`], values[`--color-${surface}`]), `${theme}: ${text} on ${surface}`).toBeGreaterThanOrEqual(4.5);
    }
    for (const surface of ["control", "panel", "bg"]) {
      expect(contrast(values["--color-control-border"], values[`--color-${surface}`]), `${theme}: control border on ${surface}`).toBeGreaterThanOrEqual(3);
      expect(contrast(values["--color-focus"], values[`--color-${surface}`]), `${theme}: focus on ${surface}`).toBeGreaterThanOrEqual(3);
    }
    for (const graph of ["graph-edge", "graph-cycle", "graph-clone"]) {
      expect(contrast(values[`--color-${graph}`], values["--color-panel"]), `${theme}: ${graph}`).toBeGreaterThanOrEqual(3);
    }
  });

  it("keeps short-window dialogs scrollable and preserves narrow-window confidence", () => {
    expect(app).toMatch(/\.update-dialog\s*\{[^}]*max-height:\s*calc\(100dvh[^}]*overflow:\s*auto/s);
    expect(app).not.toMatch(/\.issues-view__confidence\s*\{[^}]*display:\s*none/s);
    expect(app).toContain("prefers-reduced-motion: reduce");
    expect(app).toContain("forced-colors: active");
  });

  it("keeps invalid and disabled control states ahead of the base control rules", () => {
    const base = app.indexOf('.settings input:not([type="checkbox"])');
    const invalid = app.indexOf('.settings input[aria-invalid="true"]');
    expect(base).toBeGreaterThan(-1);
    expect(invalid).toBeGreaterThan(base);
    // 禁用/待处理控件不得继承悬停或按下反馈。
    expect(app).toMatch(/\.button:not\(:disabled\):not\(\[aria-disabled="true"\]\):hover/);
    expect(app).toMatch(/\.text-button:not\(:disabled\):not\(\[aria-disabled="true"\]\):hover/);
    expect(app).toMatch(/\.button\[aria-disabled="true"\][^}]*cursor:\s*not-allowed/);
  });
});
