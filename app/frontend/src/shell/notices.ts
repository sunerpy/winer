import type { NoticeKind, PhraseTrigger } from "@winer/shared";
import { useEffect, useRef } from "react";

import type { MessageKey, Translate } from "../lib/i18n";
import { useT } from "../lib/i18n";
import { namedStatus } from "../lib/presence";
import { type Catalog, useCatalog, useNotices } from "../lib/store";
import { toast } from "../ui";

/** What a quick phrase that went out says, by what sent it. */
const PHRASE_SENT: Record<PhraseTrigger, MessageKey> = {
  shortcut: "phrase.notice.sent",
  asked: "phrase.notice.asked",
  done: "phrase.notice.thanked",
};

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
    case "foreignLoader":
      return t(kind.yielded ? "plugin.notice.foreign" : "plugin.notice.foreignOff", {
        program: kind.program,
      });
    case "diagnosticsFailed":
      return t("diag.notice.failed", { version: kind.version, failed: kind.failed });
    case "failed":
      return t("notice.failed", { message: kind.message });
    // Runes, spells and item sets.
    case "loadoutApplied": {
      const name = champion(kind.championId);
      // Each half set up, with where it came from: 我的方案, the recommendation, the client's own.
      const parts = (
        [
          ["runes", kind.runes],
          ["spells", kind.spells],
        ] as const
      )
        .filter(([, origin]) => origin !== null)
        .map(([what, origin]) =>
          t("loadout.notice.part", {
            what: t(`loadout.what.${what}`),
            origin: t(`loadout.origin.${origin ?? "mine"}`),
          }),
        );
      let line =
        parts.length > 0
          ? t("loadout.notice.applied", {
              champion: name,
              parts: parts.join(t("loadout.notice.and")),
            })
          : null;
      if (kind.noPage)
        line = line
          ? t("loadout.notice.noPageToo", { applied: line })
          : t("loadout.notice.noPage", { champion: name });
      if (kind.kept.length > 0) {
        const what = t(
          `loadout.what.${kind.kept.length > 1 ? "both" : kind.kept[0] === "runes" ? "runes" : "spells"}`,
        );
        line = line
          ? `${line}；${t("loadout.notice.kept", { what })}`
          : t("loadout.notice.keptOnly", { champion: name, what });
      }
      return line ?? "";
    }
    case "itemSetWritten":
      return t("loadout.notice.itemSet", { champion: champion(kind.championId) });
    case "itemSetsFull":
      return t("loadout.itemSetsFull");
    // Rewards.
    case "rewardsClaimed":
      return kind.left > 0
        ? t("rewards.notice.claimedLeft", { n: kind.claimed, left: kind.left })
        : t("rewards.notice.claimed", { n: kind.claimed });
    case "rewardsFailed":
      return t("rewards.notice.failed", { n: kind.failed });
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
    // The Quick chat page's shortcuts: each says which line, a quick message or a random one, and
    // where it went, or would have gone.
    case "comebackSent": {
      const place = t(`comeback.place.${kind.place}`);
      const m = kind.message;
      if (m === null)
        return kind.messages > 1
          ? t("comeback.notice.sentParts", { place, n: kind.messages })
          : t("comeback.notice.sent", { place });
      return kind.messages > 1
        ? t("comeback.notice.messageSentParts", { m, place, n: kind.messages })
        : t("comeback.notice.messageSent", { m, place });
    }
    case "comebackStopped": {
      const values = {
        place: t(`comeback.place.${kind.place}`),
        n: kind.sent,
        reason: t(`callout.skip.${kind.reason}`),
      };
      return kind.message === null
        ? t("comeback.notice.stopped", values)
        : t("comeback.notice.messageStopped", { ...values, m: kind.message });
    }
    case "comebackSkipped": {
      const values = {
        place: t(`comeback.place.${kind.place}`),
        reason: t(`callout.skip.${kind.reason}`),
      };
      return kind.message === null
        ? t("comeback.notice.skipped", values)
        : t("comeback.notice.messageSkipped", { ...values, m: kind.message });
    }
    // Champ select's quick phrases, sent by their shortcut or by a trade's switches.
    case "phraseSent":
      return t(PHRASE_SENT[kind.trigger]);
    case "phraseSkipped":
      return t(kind.trigger === "done" ? "phrase.notice.thanksSkipped" : "phrase.notice.skipped", {
        reason: t(`phrase.skip.${kind.reason}`),
      });
  }
}

/** A notice about something winer could not do, drawn in the danger tone. */
export function isFailure(kind: NoticeKind): boolean {
  return (
    kind.kind === "failed" ||
    kind.kind === "presenceRefused" ||
    kind.kind === "diagnosticsFailed" ||
    kind.kind === "itemSetsFull" ||
    kind.kind === "rewardsFailed" ||
    // The callout's shortcut.
    kind.kind === "typingStopped" ||
    kind.kind === "calloutSkipped" ||
    // The Quick chat and Phrases pages' shortcuts.
    kind.kind === "comebackStopped" ||
    kind.kind === "comebackSkipped" ||
    kind.kind === "phraseSkipped"
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
