import type { NoticeKind } from "@winer/shared";
import { useEffect, useRef } from "react";

import type { MessageKey, Translate } from "../lib/i18n";
import { useT } from "../lib/i18n";
import { namedStatus } from "../lib/presence";
import { type Catalog, useCatalog, useNotices } from "../lib/store";
import { toast } from "../ui";

/** One line describing what the core did, in the window's language. */
export function noticeText(kind: NoticeKind, t: Translate, catalog: Catalog | null): string {
  const champion = (id: number) => catalog?.champions.get(id)?.name ?? `#${id}`;
  switch (kind.kind) {
    case "accepted":
      return t("notice.accepted");
    case "declared":
      return t("notice.declared", { champion: champion(kind.championId) });
    case "picked":
      return t(kind.locked ? "notice.locked" : "notice.picked", {
        champion: champion(kind.championId),
      });
    case "banned":
      return t("notice.banned", { champion: champion(kind.championId) });
    case "playedAgain":
      return t("notice.playedAgain");
    case "swapped":
      return t("notice.swapped", { champion: champion(kind.championId) });
    case "calledOut":
      return t("notice.calledOut", { n: kind.lines });
    case "presenceRestored": {
      const status = namedStatus(kind.availability);
      return t("profile.notice.restored", {
        status: status ? t(`tools.status.${status}` as MessageKey) : kind.availability,
      });
    }
    case "presenceRefused":
      return t("profile.notice.refused");
    case "failed":
      return t("notice.failed", { message: kind.message });
    // Runes, spells and item sets.
    case "loadoutApplied": {
      const name = champion(kind.championId);
      const runes = kind.runes === "written";
      const what = runes && kind.spells ? "both" : runes ? "runes" : "spells";
      const applied =
        runes || kind.spells
          ? t(kind.recommended ? "loadout.notice.recommended" : "loadout.notice.remembered", {
              champion: name,
              what: t(`loadout.what.${what}`),
            })
          : null;
      if (kind.runes !== "noPage") return applied ?? "";
      return applied
        ? t("loadout.notice.noPageToo", { applied })
        : t("loadout.notice.noPage", { champion: name });
    }
    case "itemSetWritten":
      return t("loadout.notice.itemSet", { champion: champion(kind.championId) });
    // The callout's shortcut.
    case "typedInGame":
      return t("callout.notice.typed", { n: kind.lines });
    case "typingStopped":
      return t("callout.notice.stopped", {
        n: kind.lines,
        reason: t(`callout.skip.${kind.reason}`),
      });
    case "calloutSkipped":
      return t("callout.notice.skipped", { reason: t(`callout.skip.${kind.reason}`) });
  }
}

/** A notice about something winer could not do, drawn in the danger tone. */
export function isFailure(kind: NoticeKind): boolean {
  return (
    kind.kind === "failed" ||
    kind.kind === "presenceRefused" ||
    // The callout's shortcut.
    kind.kind === "typingStopped" ||
    kind.kind === "calloutSkipped"
  );
}

/** Every new notice also pops up as a toast. */
export function useNoticeToasts(): void {
  const t = useT();
  const catalog = useCatalog();
  const notices = useNotices();
  const seen = useRef(0);
  useEffect(() => {
    const fresh = notices.filter((notice) => notice.id > seen.current);
    if (fresh.length === 0) return;
    seen.current = Math.max(...fresh.map((notice) => notice.id));
    for (const notice of fresh.reverse()) {
      toast(noticeText(notice.kind, t, catalog), isFailure(notice.kind) ? "danger" : "ok");
    }
  }, [notices, t, catalog]);
}
