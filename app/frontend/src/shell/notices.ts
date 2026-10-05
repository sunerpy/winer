import type { NoticeKind } from "@winer/shared";
import { useEffect, useRef } from "react";

import type { Translate } from "../lib/i18n";
import { useT } from "../lib/i18n";
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
    case "failed":
      return t("notice.failed", { message: kind.message });
  }
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
      toast(noticeText(notice.kind, t, catalog), notice.kind.kind === "failed" ? "danger" : "ok");
    }
  }, [notices, t, catalog]);
}
