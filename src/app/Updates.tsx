import { useEffect, useId, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  isUpdatePeriod,
  readDeferredUpdateVersion,
  readLastUpdateCheckAt,
  readUpdatePeriod,
  writeDeferredUpdateVersion,
  writeLastUpdateCheckAt,
  writeUpdatePeriod,
} from "../lib/updatePreferences";

interface Update {
  version: string;
  notes: string;
  publishedAt: string;
  sizeBytes: number;
  canInstall: boolean;
}

interface Progress {
  received: number;
  total: number;
}

const daysToMs = 86_400_000;
const messageFrom = (error: unknown) => {
  if (typeof error === "object" && error && "Validation" in error && typeof error.Validation === "string") {
    return error.Validation;
  }
  return "更新服务暂时不可用，请稍后重试。";
};
const mb = (bytes: number) => `${(bytes / 1_048_576).toFixed(1)} MB`;

export default function Updates({ visible, scanActive }: { visible: boolean; scanActive: boolean }) {
  const [version, setVersion] = useState("");
  const [period, setPeriod] = useState(readUpdatePeriod);
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [stage, setStage] = useState<"idle" | "checking" | "installing">("idle");
  const [status, setStatus] = useState("");
  const [failure, setFailure] = useState("");
  const [transferredAt, setTransferredAt] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const dialogTitle = useRef<HTMLHeadingElement>(null);
  const checkButton = useRef<HTMLButtonElement>(null);
  const returnToCheckButton = useRef(false);
  const id = useId();
  const summaryId = `${id}-summary`;
  const guidanceId = `${id}-guidance`;
  const progressStatusId = `${id}-progress-status`;

  async function check(manual: boolean) {
    if (stage !== "idle") return;
    setStatus("");
    setFailure("");
    setStage("checking");
    writeLastUpdateCheckAt(Date.now());
    try {
      const candidate = await invoke<Update | null>("check_for_update");
      if (candidate && (manual || readDeferredUpdateVersion() !== candidate.version)) {
        returnToCheckButton.current = manual;
        setUpdate(candidate);
      }
      if (manual && !candidate) setStatus("你已使用最新版本。");
    } catch (error) {
      if (manual) setFailure(messageFrom(error));
    } finally {
      setStage("idle");
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    const subscription = listen<Progress>("update://progress", (event) => {
      if (active) setProgress(event.payload);
    });
    void subscription.catch(() => {});
    void getVersion().then((value) => { if (active) setVersion(value); }).catch(() => {});
    const now = Date.now();
    const lastCheck = readLastUpdateCheckAt(now);
    if (period > 0 && (lastCheck === null || now - lastCheck >= period * daysToMs)) {
      void check(false);
    }
    return () => {
      active = false;
      void subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const modal = dialog.current;
    if (!update || !modal || modal.open) return;
    modal.showModal();
    dialogTitle.current?.focus();
  }, [update]);

  const dismiss = () => {
    if (!update || stage === "installing") return;
    writeDeferredUpdateVersion(update.version);
    dialog.current?.close();
    // A manual check temporarily disables its trigger, so it may not be the native return target.
    if (returnToCheckButton.current) checkButton.current?.focus();
    setUpdate(null);
    setFailure("");
    setProgress(null);
  };

  const install = async () => {
    if (!update?.canInstall || stage !== "idle" || scanActive) return;
    setFailure("");
    setProgress(null);
    setTransferredAt(Date.now());
    dialogTitle.current?.focus();
    setStage("installing");
    try {
      await invoke("install_update", { version: update.version });
    } catch (error) {
      setFailure(messageFrom(error));
      setProgress(null);
    } finally {
      setStage("idle");
    }
  };

  const details = async () => {
    try { await invoke("open_release_page"); }
    catch (error) { setFailure(messageFrom(error)); }
  };

  if (!isTauri()) return null;
  const guidance = update && (!update.canInstall ? "免安装版请从发布页下载并替换旧文件。"
    : scanActive ? "结束当前扫描后可安装更新。" : "");
  const downloading = progress && progress.received < progress.total;
  const downloadStatus = !progress ? "正在连接 GitHub…" : downloading ? "正在下载更新…" : "正在验证更新并备份本地数据…";
  return <>
    {visible && <section className="panel settings update-settings" aria-labelledby="update-title">
      <div className="section-heading"><h2 id="update-title">软件更新</h2><span className="pill">{version ? `v${version}` : "CodeDoctor"}</span></div>
      <label>检查频率<select value={period} onChange={(event) => {
        const next = Number(event.target.value);
        if (!isUpdatePeriod(next)) return;
        writeUpdatePeriod(next);
        setPeriod(next);
      }}><option value={0}>手动检查</option><option value={1}>每天启动时检查</option><option value={7}>每周启动时检查</option></select></label>
      <div className="button-row">
        <button ref={checkButton} className="button button--primary" disabled={stage !== "idle"} onClick={() => void check(true)}>{stage === "checking" ? "正在检查…" : "检查更新"}</button>
        <button className="button button--quiet" onClick={() => void details()}>查看发布记录</button>
      </div>
      <p className="muted">检查更新时连接 GitHub Releases。</p>
      {status && <p role="status">{status}</p>}
      {!update && failure && <p role="alert">{failure}</p>}
    </section>}
    {update && <dialog ref={dialog} className="panel update-dialog" aria-labelledby={`${id}-title`}
      aria-describedby={guidance ? `${summaryId} ${guidanceId}` : summaryId}
      onCancel={(event) => { event.preventDefault(); dismiss(); }}>
      <div className="section-heading"><h2 id={`${id}-title`} ref={dialogTitle} tabIndex={-1}>CodeDoctor {update.version} 可用</h2><span className="pill">{mb(update.sizeBytes)}</span></div>
      <p className="muted" id={summaryId}>当前 v{version} · {new Date(update.publishedAt).toLocaleDateString("zh-CN")}</p>
      {update.notes && <pre className="update-notes" tabIndex={0} role="group" aria-label="发布说明">{update.notes}</pre>}
      <div className={stage === "installing" ? "update-progress" : undefined}>
        {stage === "installing" && <progress aria-label="更新下载进度" aria-describedby={progressStatusId}
          value={progress?.received} max={progress?.total ?? update.sizeBytes} />}
        <span id={progressStatusId} role="status" className={stage !== "installing" || downloading ? "visually-hidden" : undefined}>
          {stage === "installing" ? downloadStatus : ""}
        </span>
        {stage === "installing" && downloading && <span>{mb(progress.received)} / {mb(progress.total)} · {mb(progress.received / Math.max((Date.now() - transferredAt) / 1000, 0.1))}/秒</span>}
      </div>
      {guidance && <p className="muted" id={guidanceId}>{guidance}</p>}
      {failure && <p role="alert">{failure}</p>}
      <div className="button-row">
        {update.canInstall && <button className="button button--primary" disabled={stage !== "idle" || scanActive} onClick={() => void install()}>立即更新</button>}
        <button className="button button--quiet" disabled={stage === "installing"} onClick={() => void details()}>查看详情</button>
        <button className="button button--quiet" disabled={stage === "installing"} onClick={dismiss}>稍后更新</button>
      </div>
    </dialog>}
  </>;
}
