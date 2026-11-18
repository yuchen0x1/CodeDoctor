import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

const periodKey = "codedoctor.update-period-days";
const lastCheckKey = "codedoctor.update-last-check";
const laterKey = "codedoctor.update-later";
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
  const [period, setPeriod] = useState(() => Number(localStorage.getItem(periodKey)) || 0);
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [stage, setStage] = useState<"idle" | "checking" | "installing">("idle");
  const [status, setStatus] = useState("");
  const [failure, setFailure] = useState("");
  const [transferredAt, setTransferredAt] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);

  async function check(manual: boolean) {
    if (stage !== "idle") return;
    setStatus("");
    setFailure("");
    setStage("checking");
    localStorage.setItem(lastCheckKey, String(Date.now()));
    try {
      const candidate = await invoke<Update | null>("check_for_update");
      if (candidate && (manual || localStorage.getItem(laterKey) !== candidate.version)) {
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
    if (period > 0 && Date.now() - Number(localStorage.getItem(lastCheckKey)) >= period * daysToMs) {
      void check(false);
    }
    return () => {
      active = false;
      void subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    if (update && !dialog.current?.open) dialog.current?.showModal();
  }, [update]);

  const dismiss = () => {
    if (!update || stage === "installing") return;
    localStorage.setItem(laterKey, update.version);
    setUpdate(null);
    setFailure("");
    setProgress(null);
  };

  const install = async () => {
    if (!update?.canInstall || stage !== "idle" || scanActive) return;
    setFailure("");
    setProgress(null);
    setTransferredAt(Date.now());
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
  return <>
    {visible && <section className="panel settings update-settings" aria-labelledby="update-title">
      <div className="section-heading"><h2 id="update-title">软件更新</h2><span className="pill">{version ? `v${version}` : "CodeDoctor"}</span></div>
      <label>检查频率<select value={period} onChange={(event) => {
        const next = Number(event.target.value);
        localStorage.setItem(periodKey, String(next));
        setPeriod(next);
      }}><option value={0}>手动检查</option><option value={1}>每天启动时检查</option><option value={7}>每周启动时检查</option></select></label>
      <div className="button-row">
        <button className="button button--primary" disabled={stage !== "idle"} onClick={() => void check(true)}>{stage === "checking" ? "正在检查…" : "检查更新"}</button>
        <button className="button button--quiet" onClick={() => void details()}>查看发布记录</button>
      </div>
      <p className="muted">检查更新时连接 GitHub Releases。</p>
      {status && <p role="status">{status}</p>}
      {!update && failure && <p role="alert">{failure}</p>}
    </section>}
    {update && <dialog ref={dialog} className="panel update-dialog" aria-labelledby="update-dialog-title" onCancel={(event) => { event.preventDefault(); dismiss(); }}>
      <div className="section-heading"><h2 id="update-dialog-title">CodeDoctor {update.version} 可用</h2><span className="pill">{mb(update.sizeBytes)}</span></div>
      <p className="muted">当前 v{version} · {new Date(update.publishedAt).toLocaleDateString("zh-CN")}</p>
      {update.notes && <pre className="update-notes">{update.notes}</pre>}
      {stage === "installing" && <div role="status" aria-live="polite" className="update-progress">
        <progress aria-label="下载进度" value={progress?.received} max={progress?.total ?? update.sizeBytes} />
        <span>{progress && progress.received < progress.total
          ? `${mb(progress.received)} / ${mb(progress.total)} · ${mb(progress.received / Math.max((Date.now() - transferredAt) / 1000, 0.1))}/秒`
          : progress ? "正在验证更新并备份本地数据…" : "正在连接 GitHub…"}</span>
      </div>}
      {!update.canInstall && <p className="muted">免安装版请从发布页下载并替换旧文件。</p>}
      {scanActive && update.canInstall && <p className="muted">结束当前扫描后可安装更新。</p>}
      {failure && <p role="alert">{failure}</p>}
      <div className="button-row">
        {update.canInstall && <button className="button button--primary" disabled={stage !== "idle" || scanActive} onClick={() => void install()}>立即更新</button>}
        <button className="button button--quiet" disabled={stage === "installing"} onClick={() => void details()}>查看详情</button>
        <button className="button button--quiet" disabled={stage === "installing"} onClick={dismiss}>稍后更新</button>
      </div>
    </dialog>}
  </>;
}
