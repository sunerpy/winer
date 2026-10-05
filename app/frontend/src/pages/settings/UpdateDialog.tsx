// One dialog that walks an update from "available" to "installing". Closing it never cancels.
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
    case "installing":
      return t("update.installing", { version: status.version });
    case "failed":
      return t("update.failed", { message: status.message });
  }
}

/** Release notes are text: Markdown markers are dropped, links keep their words, nothing opens. */
export function plainNotes(notes: string): string[] {
  return notes
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
}

function megabytes(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function UpdateDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const store = useStore();
  const status = useUpdateStatus();
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
      : t("settings.update");
  const footer =
    status.state === "available" ? (
      <>
        <Button variant="ghost" onClick={onClose}>
          {t("update.later")}
        </Button>
        <Button variant="accent" onClick={install} data-autofocus>
          {t("update.install")}
        </Button>
      </>
    ) : status.state === "downloading" ? (
      <Button onClick={onClose}>{t("update.background")}</Button>
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
      dismissOnScrim
    >
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto px-5 py-4 text-[13px]">
        {status.state === "available" && (
          <>
            <p className="text-fg-muted">
              {t("update.current", { version: status.current })}
              {status.date && ` · ${status.date}`}
            </p>
            <div className="max-h-[280px] overflow-y-auto rounded-6 bg-inset p-3 hairline">
              <p className="eyebrow mb-2">{t("update.notes")}</p>
              {status.notes ? (
                <ul className="flex flex-col gap-1.5 text-[12.5px] leading-5 text-fg">
                  {plainNotes(status.notes).map((line, index) => (
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
          <div className="flex flex-col gap-2">
            <div
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={
                status.total ? Math.round((status.received / status.total) * 100) : undefined
              }
              className="h-1.5 overflow-hidden rounded-pill bg-inset2"
            >
              <div
                className="h-full rounded-pill bg-accent transition-[width] duration-300"
                style={{
                  width: status.total ? `${(status.received / status.total) * 100}%` : "35%",
                }}
              />
            </div>
            <p className="mono text-[12px] text-fg-muted">
              {megabytes(status.received)}
              {status.total ? ` / ${megabytes(status.total)}` : ""}
            </p>
          </div>
        )}
        {(status.state === "installing" || status.state === "checking") && (
          <p className="flex items-center gap-2 text-fg-muted">
            <Spinner />
            {updateLine(status, t)}
          </p>
        )}
        {(status.state === "idle" || status.state === "upToDate" || status.state === "failed") && (
          <p
            role={status.state === "failed" ? "alert" : undefined}
            className={status.state === "failed" ? "break-words text-danger" : "text-fg-muted"}
          >
            {updateLine(status, t)}
          </p>
        )}
      </div>
    </Dialog>
  );
}
