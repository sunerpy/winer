// One dialog that walks an update from discovery through a verified download to restart.
// Closing it never cancels a download; installation begins only after an explicit second action.
import { ArrowUpCircle, CheckCircle2, Download } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { UpdateStatus } from "@winer/shared";

import { errorMessage } from "../../lib/backend";
import { type Translate, useT } from "../../lib/i18n";
import { useStore, useUpdateStatus } from "../../lib/store";
import { Button, Dialog, Spinner, toast } from "../../ui";

/** The status as one line, for settings and the dialog alike. */
export function updateLine(status: UpdateStatus, t: Translate): string {
  switch (status.state) {
    case "idle":
      return t("update.idle");
    case "checking":
      return t("update.checking");
    case "upToDate":
      return t("update.upToDate", { version: status.version });
    case "available":
      return t("update.available", { version: status.version });
    case "downloading":
      return t("update.downloading", { version: status.version });
    case "ready":
      return t("update.ready", { version: status.version });
    case "installing":
      return t("update.installing", { version: status.version });
    case "failed":
      return t("update.failed", { message: status.message });
  }
}

/** Release notes are text: Markdown markers are dropped, links keep their words, nothing opens. */
export function plainNotes(notes: string, version?: string): string[] {
  const lines = notes
    .split(/\r?\n/)
    .map((line) =>
      line
        .replace(/^#+\s*/, "")
        .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
        .replace(/\(\s*\)/g, "")
        .replace(/\*\*|`/g, "")
        .trim(),
    )
    .filter(Boolean);
  const normalized = (text: string) => text.trim().replace(/^v/i, "");
  const heading = lines[0] ? normalized(lines[0]) : "";
  const current = version ? normalized(version) : "";
  return current && (heading === current || heading.startsWith(`${current} (`))
    ? lines.slice(1)
    : lines;
}

function megabytes(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function rate(bytesPerSecond: number): string {
  return bytesPerSecond >= 1024 * 1024
    ? `${(bytesPerSecond / 1024 / 1024).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytesPerSecond / 1024))} KB`;
}

function duration(seconds: number): string {
  if (seconds < 60) return `${Math.max(1, Math.ceil(seconds))} 秒`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} 分 ${Math.ceil(seconds - minutes * 60)} 秒`;
}

interface DownloadMetrics {
  speed: number;
  eta: number | null;
}

/** Smooth speed over the latest three seconds. Status events themselves drive the sampling. */
function useDownloadMetrics(status: UpdateStatus): DownloadMetrics | null {
  const samples = useRef<Array<{ at: number; bytes: number }>>([]);
  const [metrics, setMetrics] = useState<DownloadMetrics | null>(null);

  useEffect(() => {
    if (status.state !== "downloading") {
      samples.current = [];
      setMetrics(null);
      return;
    }
    const at = performance.now();
    samples.current.push({ at, bytes: status.received });
    samples.current = samples.current.filter((sample) => at - sample.at <= 3_000);
    const first = samples.current[0];
    const elapsed = first ? (at - first.at) / 1_000 : 0;
    const received = first ? status.received - first.bytes : 0;
    if (elapsed < 0.5 || received <= 0) {
      setMetrics(null);
      return;
    }
    const speed = received / elapsed;
    setMetrics({
      speed,
      eta: status.total ? Math.max(0, (status.total - status.received) / speed) : null,
    });
  }, [status]);

  return metrics;
}

export function UpdateDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const store = useStore();
  const status = useUpdateStatus();
  const metrics = useDownloadMetrics(status);
  const install = () =>
    void store.backend
      .call("install_update")
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
  const check = () =>
    void store.backend
      .call("check_update")
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  const title =
    status.state === "available" || status.state === "downloading"
      ? t("update.available", { version: status.version })
      : status.state === "ready"
        ? t("update.ready", { version: status.version })
        : status.state === "failed"
          ? t("update.failedTitle")
          : t("settings.update");
  const footer =
    status.state === "available" ? (
      <>
        <Button variant="ghost" onClick={onClose}>
          {t("update.later")}
        </Button>
        <Button variant="accent" icon={Download} onClick={install} data-autofocus>
          {t("update.install")}
        </Button>
      </>
    ) : status.state === "downloading" ? (
      <Button onClick={onClose}>{t("update.background")}</Button>
    ) : status.state === "ready" ? (
      <>
        <Button variant="ghost" onClick={onClose}>
          {t("update.later")}
        </Button>
        <Button variant="accent" icon={ArrowUpCircle} onClick={install} data-autofocus>
          {t("update.restart")}
        </Button>
      </>
    ) : status.state === "installing" ? (
      <Button loading disabled>
        {t("update.installingAction")}
      </Button>
    ) : status.state === "failed" ? (
      <>
        <Button variant="ghost" onClick={onClose}>
          {t("common.close")}
        </Button>
        <Button variant="primary" onClick={check}>
          {t("common.retry")}
        </Button>
      </>
    ) : (
      <Button onClick={onClose}>{t("common.close")}</Button>
    );

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      closeLabel={t("common.close")}
      footer={footer}
      dismissOnScrim={status.state !== "installing"}
      className="w-[min(500px,calc(100vw-64px))]"
    >
      <div className="flex min-h-0 flex-col gap-4 overflow-y-auto px-5 py-4 text-[13px]">
        {status.state === "available" && (
          <>
            <div className="flex items-start gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-10 bg-accent-soft text-accent-text">
                <ArrowUpCircle size={20} strokeWidth={2} aria-hidden />
              </span>
              <div className="min-w-0 pt-0.5">
                <p className="font-medium text-fg">{t("update.availableHint")}</p>
                <p className="mt-1 text-[12px] text-fg-muted">
                  {t("update.current", { version: status.current })}
                  {status.date && ` · ${status.date}`}
                </p>
              </div>
            </div>
            <div className="max-h-[320px] overflow-y-auto rounded-10 bg-inset p-3 hairline">
              <p className="eyebrow mb-2">{t("update.notes")}</p>
              {status.notes ? (
                <ul className="flex flex-col gap-1.5 text-[12.5px] leading-5 text-fg">
                  {plainNotes(status.notes, status.version).map((line, index) => (
                    <li key={index}>{line.replace(/^[-*]\s*/, "· ")}</li>
                  ))}
                </ul>
              ) : (
                <p className="text-fg-subtle">{t("update.noNotes")}</p>
              )}
            </div>
          </>
        )}
        {status.state === "downloading" && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-10 bg-accent-soft text-accent-text">
                <Download size={20} strokeWidth={2} aria-hidden />
              </span>
              <div>
                <p className="font-medium text-fg">{t("update.downloadingHint")}</p>
                <p className="mt-1 text-[12px] text-fg-muted">{t("update.backgroundHint")}</p>
              </div>
            </div>
            <div
              role="progressbar"
              aria-label={t("update.progress")}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={
                status.total ? Math.round((status.received / status.total) * 100) : undefined
              }
              className="h-2 overflow-hidden rounded-pill bg-inset2"
            >
              <div
                className="h-full rounded-pill bg-accent transition-[width] duration-300"
                style={{
                  width: status.total ? `${(status.received / status.total) * 100}%` : "35%",
                }}
              />
            </div>
            <div className="flex items-center justify-between gap-3 text-[12px] text-fg-muted">
              <span className="mono">
                {megabytes(status.received)}
                {status.total ? ` / ${megabytes(status.total)}` : ""}
              </span>
              {metrics && (
                <span className="text-right">
                  {t("update.speed", { speed: rate(metrics.speed) })}
                  {metrics.eta !== null && ` · ${t("update.eta", { time: duration(metrics.eta) })}`}
                </span>
              )}
            </div>
          </div>
        )}
        {status.state === "ready" && (
          <div className="flex items-start gap-3 rounded-10 bg-inset p-4 hairline">
            <span className="grid size-9 shrink-0 place-items-center rounded-10 bg-ok-soft text-ok">
              <CheckCircle2 size={20} strokeWidth={2} aria-hidden />
            </span>
            <div className="pt-0.5">
              <p className="font-medium text-fg">{t("update.verified")}</p>
              <p className="mt-1 text-[12.5px] leading-5 text-fg-muted">{t("update.readyHint")}</p>
            </div>
          </div>
        )}
        {(status.state === "installing" || status.state === "checking") && (
          <div className="flex items-start gap-3 rounded-10 bg-inset p-4 hairline">
            <Spinner />
            <div>
              <p className="font-medium text-fg">{updateLine(status, t)}</p>
              <p className="mt-1 text-[12px] text-fg-muted">
                {status.state === "installing"
                  ? t("update.installingHint")
                  : t("update.checkingHint")}
              </p>
            </div>
          </div>
        )}
        {(status.state === "idle" || status.state === "upToDate" || status.state === "failed") && (
          <p
            role={status.state === "failed" ? "alert" : undefined}
            className={
              status.state === "failed"
                ? "break-words rounded-10 bg-inset p-4 text-danger hairline"
                : "text-fg-muted"
            }
          >
            {updateLine(status, t)}
          </p>
        )}
      </div>
    </Dialog>
  );
}
