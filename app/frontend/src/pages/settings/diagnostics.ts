// The self-check's words: a line for each check, and the whole report as text to paste into an
// issue. The core and the shell keep ports, paths and ids out of the report, so the text holds
// none either.
import type { Check, CheckFailure, CheckStatus, DiagnosticsReport } from "@winer/shared";

import type { MessageKey, Translate } from "../../lib/i18n";
import type { LampTone } from "../../ui";

export const STATUS_TONE: Record<CheckStatus, LampTone> = {
  ok: "ok",
  warn: "warn",
  fail: "danger",
  unknown: "idle",
  skipped: "off",
};

const STATUS_MARK: Record<CheckStatus, string> = {
  ok: "✓",
  warn: "!",
  fail: "✗",
  unknown: "?",
  skipped: "–",
};

export function failureText(failure: CheckFailure, t: Translate): string {
  return failure.kind === "status"
    ? t("diag.failure.status", { code: failure.code })
    : t(`diag.failure.${failure.kind}` as MessageKey);
}

/** Whether the check's detail is a block of its own (the missing routes) rather than part of its
 *  sentence. */
export function detailBlock(check: Check): string | null {
  return check.reason === "routesMissing" ? check.detail : null;
}

/** What one check came to, in a sentence: the reason, the client's version where it has one, and
 *  how a request failed. */
export function checkLine(check: Check, t: Translate): string {
  let line = t(`diag.reason.${check.reason}` as MessageKey, { detail: check.detail ?? "" });
  if (check.id === "client" && check.status === "ok" && check.detail) line += ` · ${check.detail}`;
  if (check.failure) line += ` · ${failureText(check.failure, t)}`;
  return line;
}

/** The report as plain text, one line a check and the missing routes indented under theirs. */
export function reportText(report: DiagnosticsReport, version: string, t: Translate): string {
  const at = new Date(report.at);
  const pad = (value: number) => String(value).padStart(2, "0");
  const when = `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())} ${pad(at.getHours())}:${pad(at.getMinutes())}`;
  const lines = [`winer ${version} · ${t("diag.title")} · ${when}`];
  if (report.clientVersion) lines.push(`${t("diag.id.client")} ${report.clientVersion}`);
  for (const check of report.checks) {
    lines.push(`${STATUS_MARK[check.status]} ${t(`diag.id.${check.id}`)} · ${checkLine(check, t)}`);
    const block = detailBlock(check);
    if (block) lines.push(...block.split("\n").map((route) => `    ${route}`));
  }
  return lines.join("\n");
}
