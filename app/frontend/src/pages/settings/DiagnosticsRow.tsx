// Settings › About: the self-check on demand, its results a line a check, and a copy of them as
// text for a bug report. A clipboard the webview refuses falls back to the text, selectable.
import type { DiagnosticsReport } from "@winer/shared";
import { Copy, Stethoscope } from "lucide-react";
import { useState } from "react";

import { errorMessage } from "../../lib/backend";
import { useT } from "../../lib/i18n";
import { useStore } from "../../lib/store";
import { Button, Lamp, Row, toast } from "../../ui";
import { STATUS_TONE, checkLine, detailBlock, reportText } from "./diagnostics";

export function DiagnosticsRow({ version }: { version: string }) {
  const t = useT();
  const store = useStore();
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [manual, setManual] = useState(false);
  const text = report ? reportText(report, version, t) : "";

  const run = async () => {
    setBusy(true);
    try {
      setReport(await store.backend.call("run_diagnostics"));
      setManual(false);
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      toast(t("diag.copied"), "ok");
    } catch {
      setManual(true);
    }
  };

  return (
    <>
      <Row label={t("diag.title")} help={t("diag.hint")}>
        {report && (
          <Button size="sm" variant="ghost" icon={Copy} onClick={() => void copy()}>
            {t("diag.copy")}
          </Button>
        )}
        <Button size="sm" icon={Stethoscope} loading={busy} onClick={() => void run()}>
          {t("diag.run")}
        </Button>
      </Row>
      {report && (
        <ul aria-label={t("diag.results")} className="flex flex-col border-b border-border pb-3">
          {report.checks.map((check) => {
            const block = detailBlock(check);
            return (
              <li key={check.id} className="flex items-start gap-2.5 py-1.5">
                <Lamp
                  tone={STATUS_TONE[check.status]}
                  size={6}
                  label={t(`diag.status.${check.status}`)}
                  className="mt-[7px] shrink-0"
                />
                <div className="min-w-0 flex-1">
                  <p className="text-[12.5px] leading-5 text-fg">
                    <span className="font-medium">{t(`diag.id.${check.id}`)}</span>
                    <span className="text-fg-muted"> · {checkLine(check, t)}</span>
                  </p>
                  {block && (
                    <pre className="mono mt-1 overflow-x-auto rounded-6 bg-inset px-2.5 py-1.5 text-[11px] leading-4 whitespace-pre text-fg-muted hairline">
                      {block}
                    </pre>
                  )}
                </div>
                {check.tookMs > 0 && (
                  <span className="mono shrink-0 pt-0.5 text-[11px] text-fg-subtle">
                    {check.tookMs} ms
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {manual && (
        <div className="border-b border-border py-3">
          <p className="mb-1.5 text-[12px] text-fg-muted">{t("diag.copyFailed")}</p>
          <textarea
            readOnly
            value={text}
            aria-label={t("diag.copyText")}
            rows={Math.min(14, text.split("\n").length)}
            onFocus={(event) => event.currentTarget.select()}
            className="mono w-full resize-none rounded-6 bg-inset p-2.5 text-[11px] leading-4 text-fg hairline"
          />
        </div>
      )}
    </>
  );
}
