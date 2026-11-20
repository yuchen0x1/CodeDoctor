import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { DEFAULT_SETTINGS, parseExcludedDirectories, thresholdRanges, type AppSettings, type ScanThresholds } from "../lib/settings";

const thresholdLabels: Record<keyof ScanThresholds, string> = {
  maxLineLength: "单行字符数",
  maxFunctionLines: "函数行数",
  maxCyclomatic: "圈复杂度",
  maxCognitive: "认知复杂度",
  maxNesting: "嵌套层级",
  maxParameters: "函数参数数",
  maxClassMethods: "类方法数",
  maxClassLines: "类行数",
  maxFileLines: "文件行数",
  duplicateMinLines: "重复片段最少行数",
};

export default function SettingsView({ settings, onSave }: {
  settings: AppSettings;
  onSave: (value: AppSettings) => void;
}) {
  const [theme, setTheme] = useState(settings.theme);
  const [directoryText, setDirectoryText] = useState(settings.excludedDirectories.join(", "));
  const [ruff, setRuff] = useState(settings.enabledAnalyzers.includes("ruff"));
  const [thresholds, setThresholds] = useState(settings.thresholds);
  const [error, setError] = useState("");
  const fieldId = useId();
  const directoryInput = useRef<HTMLInputElement>(null);
  const directoryHintId = `${fieldId}-directory-hint`;
  const directoryErrorId = `${fieldId}-directory-error`;

  useEffect(() => {
    if (error) directoryInput.current?.focus();
  }, [error]);

  const save = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const entries = directoryText.split(/[,，\n]/u).map((item) => item.trim()).filter(Boolean);
    if (entries.some((entry) => !parseExcludedDirectories([entry]).length)) {
      setError("排除项需要填写目录名称，例如 generated。");
      if (error) directoryInput.current?.focus();
      return;
    }
    setError("");
    onSave({
      theme,
      excludedDirectories: parseExcludedDirectories(entries),
      enabledAnalyzers: ruff ? ["ruff"] : [],
      thresholds,
    });
  };

  return <form className="panel settings" onSubmit={save}>
    <label>
      <span id={`${fieldId}-theme-label`}>主题</span>
      <select aria-labelledby={`${fieldId}-theme-label`} aria-describedby={`${fieldId}-theme-hint`}
        value={theme} onChange={(event) => setTheme(event.target.value === "light" ? "light" : "dark")}>
        <option value="dark">深色工作台</option>
        <option value="light">浅色工作台</option>
      </select>
      <small id={`${fieldId}-theme-hint`}>点击“保存设置”后应用主题。</small>
    </label>
    <label>
      <span id={`${fieldId}-directory-label`}>额外排除目录</span>
      <input ref={directoryInput} value={directoryText} aria-labelledby={`${fieldId}-directory-label`}
        aria-describedby={error ? `${directoryHintId} ${directoryErrorId}` : directoryHintId} aria-invalid={Boolean(error)}
        onChange={(event) => { setDirectoryText(event.target.value); setError(""); }} placeholder="例如 generated, vendor" />
      <small id={directoryHintId}>按目录名称匹配，使用逗号分隔。默认仍排除 .git、node_modules、build、dist、target 等。</small>
    </label>
    <fieldset>
      <legend>扫描阈值</legend>
      <div className="threshold-grid">
        {(Object.keys(thresholdLabels) as (keyof ScanThresholds)[]).map((key) => {
          const [min, max] = thresholdRanges[key];
          return <label key={key}>
            {thresholdLabels[key]}
            <input type="number" required min={min} max={max} value={Number.isNaN(thresholds[key]) ? "" : thresholds[key]}
              onChange={(event) => setThresholds({ ...thresholds, [key]: event.target.valueAsNumber })} />
          </label>;
        })}
      </div>
    </fieldset>
    <fieldset>
      <legend>本机分析器</legend>
      <label className="analyzer-toggle"><input type="checkbox" checked={ruff} onChange={(event) => setRuff(event.target.checked)} /> 启用已安装的 Ruff</label>
      <p className="muted">Ruff 使用隔离配置，只读扫描。ESLint、TypeScript、golangci-lint 和 Clippy 已接入结果适配，当前执行策略会跳过可能执行项目代码的命令。</p>
    </fieldset>
    {error && <p id={directoryErrorId} role="alert">{error}</p>}
    <div className="button-row">
      <button className="button button--primary" type="submit">保存设置</button>
      <button className="button button--quiet" type="button" onClick={() => {
        setTheme(DEFAULT_SETTINGS.theme);
        setDirectoryText("");
        setRuff(false);
        setThresholds(DEFAULT_SETTINGS.thresholds);
        setError("");
      }}>恢复默认值</button>
    </div>
  </form>;
}
