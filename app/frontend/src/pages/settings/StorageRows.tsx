// Settings › About: what winer keeps on disk and in memory, the limit of each, and the cleanup.
import type { DiskUse, StorageReport } from "@winer/shared";
import { BrushCleaning } from "lucide-react";
import { type ReactNode, useId, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { type Translate, useT } from "../../lib/i18n";
import { useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import { Button, ErrorNote, Lamp, Skeleton, toast } from "../../ui";
import { cleanupLines, formatSize, sizeParts } from "./storage";

/** A size, its figure in mono and its unit in `fg-subtle`. */
function Size({ bytes }: { bytes: number }) {
  const [figure, unit] = sizeParts(bytes);
  return (
    <span className="whitespace-nowrap">
      <span className="mono text-fg">{figure}</span> <span className="text-fg-subtle">{unit}</span>
    </span>
  );
}

function Files({ use, label }: { use: DiskUse; label: string }) {
  return (
    <>
      <Size bytes={use.bytes} />
      <span className="text-fg-subtle"> · {label}</span>
    </>
  );
}

interface Line {
  label: string;
  value: ReactNode;
  note: string;
}

function linesOf(report: StorageReport, t: Translate): Line[] {
  const { limits } = report;
  const files = (use: DiskUse) => <Files use={use} label={t("storage.files", { n: use.files })} />;
  return [
    {
      label: t("storage.logs"),
      value: files(report.logs),
      note: t("storage.logsNote", {
        days: limits.logDays,
        total: formatSize(limits.logBytes),
        file: formatSize(limits.logFileBytes),
      }),
    },
    {
      label: t("storage.webview"),
      value: <Size bytes={report.webview.bytes} />,
      note: t("storage.webviewNote", {
        cache: formatSize(report.webviewCache.bytes),
        limit: formatSize(limits.webviewCacheBytes),
      }),
    },
    {
      label: t("storage.backups"),
      value: (
        <Files use={report.backups} label={t("storage.copies", { n: report.backups.files })} />
      ),
      note: t("storage.backupsNote", { n: limits.backups }),
    },
    {
      label: t("storage.pengu"),
      value: <Size bytes={report.pengu.bytes} />,
      note: t("storage.penguNote"),
    },
    {
      label: t("storage.settings"),
      value: <Size bytes={report.settings.bytes} />,
      note: t("storage.settingsNote"),
    },
    {
      label: t("storage.updates"),
      value: files(report.updates),
      note: t("storage.updatesNote"),
    },
    {
      label: t("storage.memory"),
      value: (
        <span className="text-fg-subtle">
          {t("storage.memoryValue", {
            n: report.memory.entries,
            images: formatSize(report.memory.imageBytes),
          })}
        </span>
      ),
      note: t("storage.memoryNote", { limit: formatSize(limits.imageBytes) }),
    },
  ];
}

export function StorageRows() {
  const t = useT();
  const store = useStore();
  const title = useId();
  const report = useAsync(() => store.backend.call("get_storage"), []);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string[] | null>(null);

  const clear = async () => {
    setBusy(true);
    try {
      const cleaned = await store.backend.call("clear_caches");
      // The window's own: the history it has shown.
      store.history.clear();
      setResult(cleanupLines(cleaned, t));
      report.reload();
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section aria-labelledby={title} className="border-b border-border py-3">
      <div className="flex items-start justify-between gap-6">
        <div className="min-w-0 flex-1">
          <h3 id={title} className="text-[14px] font-medium text-fg">
            {t("storage.title")}
          </h3>
          <p className="mt-0.5 text-[12px] leading-4 text-fg-muted">{t("storage.hint")}</p>
        </div>
        <Button size="sm" icon={BrushCleaning} loading={busy} onClick={() => void clear()}>
          {t("storage.clear")}
        </Button>
      </div>
      {report.data ? (
        <dl className="mt-3 flex flex-col rounded-6 bg-inset px-3.5 py-0.5 hairline">
          {linesOf(report.data, t).map((line) => (
            <div
              key={line.label}
              className="flex items-baseline gap-3 border-b border-border py-2 last:border-b-0"
            >
              <dt className="w-[132px] shrink-0 text-[12.5px] text-fg">{line.label}</dt>
              <dd className="min-w-0 flex-1 text-[12.5px]">
                {line.value}
                <span className="block text-[11.5px] leading-4 text-fg-subtle">{line.note}</span>
              </dd>
            </div>
          ))}
        </dl>
      ) : report.error !== undefined ? (
        <ErrorNote
          title={t("storage.loadFailed")}
          detail={errorMessage(report.error)}
          retryLabel={t("common.retry")}
          onRetry={report.reload}
        />
      ) : (
        <div className="mt-3 flex flex-col gap-2">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-full" />
        </div>
      )}
      {report.data?.webviewClearPending && (
        <p className="mt-2 flex items-center gap-2 text-[12px] text-fg-muted">
          <Lamp tone="warn" size={6} />
          {t("storage.webviewPending")}
        </p>
      )}
      {result && (
        <p role="status" className="mt-2 text-[12px] leading-5 text-fg-muted">
          {result.map((line) => (
            <span key={line} className="block">
              {line}
            </span>
          ))}
        </p>
      )}
    </section>
  );
}
