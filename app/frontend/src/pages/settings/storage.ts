// Settings › About › Storage: sizes as the window says them, and what a cleanup removed.
import type { CleanupReport } from "@winer/shared";

import type { Translate } from "../../lib/i18n";

const UNITS = ["B", "KB", "MB", "GB"] as const;

/** A size as its figure and unit, in multiples of 1024: `["9.3", "MB"]`, `["512", "B"]`. Under ten
 *  of a unit it keeps one decimal. */
export function sizeParts(bytes: number): [figure: string, unit: string] {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const figure =
    unit === 0 || value >= 10 ? String(Math.round(value)) : value.toFixed(1).replace(/\.0$/, "");
  return [figure, UNITS[unit] ?? "B"];
}

export function formatSize(bytes: number): string {
  return sizeParts(bytes).join(" ");
}

/** What a cleanup removed, one sentence a line: what went now, then what goes at the next start. */
export function cleanupLines(report: CleanupReport, t: Translate): string[] {
  const went: string[] = [];
  if (report.logs.files > 0)
    went.push(
      t("storage.cleared.logs", { n: report.logs.files, size: formatSize(report.logs.bytes) }),
    );
  if (report.updates.files > 0)
    went.push(
      t("storage.cleared.updates", {
        n: report.updates.files,
        size: formatSize(report.updates.bytes),
      }),
    );
  if (report.memory.entries > 0)
    went.push(t("storage.cleared.memory", { n: report.memory.entries }));
  const lines: string[] = [];
  if (went.length > 0)
    lines.push(t("storage.cleared", { items: went.join(t("storage.cleared.separator")) }));
  if (report.webviewCache.files > 0)
    lines.push(t("storage.cleared.webview", { size: formatSize(report.webviewCache.bytes) }));
  return lines.length > 0 ? lines : [t("storage.cleared.nothing")];
}
