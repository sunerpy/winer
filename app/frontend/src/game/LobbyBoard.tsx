// The party in the lobby, laid out like a team in champ select: who they are, the lanes they asked
// for and how they have been playing on the left, their latest games on the right. A member opens
// their history.
import {
  positionLabel,
  riotId,
  type LanePreference,
  type Language,
  type LobbyMember,
} from "@winer/shared";
import { ChevronRight, Crown, TriangleAlert } from "lucide-react";

import { cx } from "../lib/cx";
import { type Translate, useLanguage, useT } from "../lib/i18n";
import { Badge, Skeleton } from "../ui";
import { ProfileIcon } from "./icons";
import { KdaValue, RankBadge, StreakBadge, WinRate, bestRank } from "./stats";
import { RecentTiles } from "./TeamBoard";
import { NoteChip } from "./notes";

function laneLabel(lane: LanePreference, t: Translate, language: Language): string {
  return lane === "fill" ? t("social.fill") : positionLabel(lane, language);
}

function MemberRow({
  member,
  onPlayer,
}: {
  member: LobbyMember;
  onPlayer: (puuid: string) => void;
}) {
  const t = useT();
  const language = useLanguage();
  const summary = member.stats.state === "ready" ? member.stats : null;
  const form = summary?.recent;
  const name = riotId(member.name ?? summary?.name ?? null) || t("common.hidden");
  return (
    <li
      data-self={member.isSelf || undefined}
      className={cx(
        "grid min-w-0 grid-cols-[minmax(0,340px)_minmax(0,1fr)] items-center gap-3 rounded-10 bg-surface p-2 hairline",
        member.isSelf && "shadow-[inset_3px_0_0_0_var(--accent)]",
      )}
    >
      <button
        type="button"
        aria-label={t("live.openHistory", { name })}
        onClick={() => onPlayer(member.puuid)}
        className="group flex min-w-0 items-center gap-3 rounded-6 p-1 text-left hover-wash"
      >
        <ProfileIcon id={member.iconId} size={40} />
        <span className="flex min-w-0 flex-col gap-1">
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="truncate text-[13.5px] font-medium text-fg">{name}</span>
            {member.isSelf && <Badge>{t("social.you")}</Badge>}
            {member.leader && (
              <Badge tone="accent">
                <Crown size={11} strokeWidth={2} aria-hidden />
                {t("social.leader")}
              </Badge>
            )}
            {member.positions.map((lane) => (
              <Badge key={lane}>{laneLabel(lane, t, language)}</Badge>
            ))}
            {member.note && <NoteChip note={member.note} />}
          </span>
          {summary && form ? (
            <span className="flex min-w-0 flex-wrap items-center gap-x-2.5 gap-y-0.5 text-[11.5px] text-fg-subtle">
              <RankBadge rank={bestRank(summary.ranked).rank} short />
              {form.games > 0 && (
                <>
                  <span
                    className="inline-flex items-center gap-1"
                    title={t("history.formRuleShort", { n: form.games })}
                  >
                    <span className="mono">{t("common.recent", { n: form.games })}</span>
                    <WinRate wins={form.wins} games={form.games} />
                  </span>
                  <span className="inline-flex items-center gap-1">
                    KDA
                    <KdaValue kills={form.kills} deaths={form.deaths} assists={form.assists} />
                  </span>
                  <StreakBadge streak={form.streak} />
                </>
              )}
              {member.score !== null && (
                <span className="mono text-fg-muted">
                  {t("social.score", { score: member.score.toFixed(1) })}
                </span>
              )}
            </span>
          ) : member.stats.state === "loading" ? (
            <Skeleton className="h-3.5 w-40" />
          ) : member.stats.state === "failed" ? (
            <span
              className="inline-flex items-center gap-1.5 text-[11.5px] text-fg-subtle"
              title={member.stats.message}
            >
              <TriangleAlert size={12} strokeWidth={2} aria-hidden />
              {t("live.statsFailed")}
            </span>
          ) : null}
        </span>
        <ChevronRight
          size={14}
          strokeWidth={2}
          aria-hidden
          className="ml-auto shrink-0 text-fg-subtle opacity-0 transition-opacity duration-150 group-hover:opacity-100"
        />
      </button>
      <RecentTiles stats={member.stats} />
    </li>
  );
}

export function LobbyBoard({
  members,
  onPlayer,
}: {
  members: LobbyMember[];
  onPlayer: (puuid: string) => void;
}) {
  return (
    <ul className="flex flex-col gap-1.5">
      {members.map((member) => (
        <MemberRow key={member.puuid} member={member} onPlayer={onPlayer} />
      ))}
    </ul>
  );
}
