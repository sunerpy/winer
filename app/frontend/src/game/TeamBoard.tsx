// One team, one row per player: who they are and how they have been playing on the left, their
// latest games as tiles on the right. A player's name opens their history.
import {
  formatKda,
  kda,
  relativeTime,
  riotId,
  type PlayerSummary,
  type RecentMatch,
  type Seat,
} from "@winer/shared";
import { ChevronRight, EyeOff, TriangleAlert } from "lucide-react";

import { cx } from "../lib/cx";
import { useLanguage, useT } from "../lib/i18n";
import { MODE_SHORT, modeOf } from "../lib/modes";
import { useCatalog } from "../lib/store";
import { useNow } from "../lib/useNow";
import { Skeleton } from "../ui";
import { GroupBadge } from "./groups";
import { NoteChip } from "./notes";
import { ChampionIcon } from "./icons";
import { KdaValue, RankBadge, StreakBadge, TierBadge, TitleChip, WinRate, bestRank } from "./stats";

/** Tiles beyond the sixth show only where the row has room for them (container widths). */
const TILE_ROOM = [
  "",
  "",
  "",
  "",
  "",
  "",
  "hidden @[540px]:flex",
  "hidden @[540px]:flex",
  "hidden @[680px]:flex",
  "hidden @[680px]:flex",
  "hidden @[820px]:flex",
  "hidden @[820px]:flex",
];

function GameTile({ game, index }: { game: RecentMatch; index: number }) {
  const t = useT();
  const language = useLanguage();
  const catalog = useCatalog();
  const now = useNow(60_000);
  const queue = catalog?.queues.get(game.queueId);
  const mode = queue ? t(MODE_SHORT[modeOf(queue.gameMode, queue.ranked)]) : "";
  const result = game.remake ? "remake" : game.win ? "win" : "loss";
  const line = `${game.kills}/${game.deaths}/${game.assists}`;
  return (
    <li
      title={t("live.tile", {
        mode: queue?.name ?? mode,
        result: t(`live.${result}`),
        kda: `${line} (${formatKda(kda(game.kills, game.deaths, game.assists))})`,
        when: relativeTime(game.startedAt, now, language),
      })}
      className={cx(
        "flex w-16 shrink-0 flex-col items-center gap-0.5 rounded-6 border-b-2 px-1 pt-1.5 pb-1",
        result === "win" && "border-win bg-win-soft",
        result === "loss" && "border-loss bg-loss-soft",
        result === "remake" && "border-border-strong bg-inset",
        TILE_ROOM[index],
      )}
    >
      <ChampionIcon id={game.championId} size={26} />
      <span className="mono text-[10.5px] leading-4 text-fg">{line}</span>
      <span className="max-w-full truncate text-[9.5px] leading-3 text-fg-subtle">{mode}</span>
    </li>
  );
}

function Summary({ seat, summary }: { seat: Seat; summary: PlayerSummary | null }) {
  const t = useT();
  const form = summary?.recent;
  const rating = seat.rating;
  return (
    <span className="flex min-w-0 flex-col gap-1">
      <span className="flex min-w-0 items-center gap-1.5">
        {seat.stats.state === "hidden" ? (
          <span className="inline-flex items-center gap-1.5 text-[13px] text-fg-subtle">
            <EyeOff size={13} strokeWidth={2} aria-hidden />
            {t("common.hidden")}
          </span>
        ) : (
          <span className="truncate text-[13.5px] font-medium text-fg">
            {riotId(seat.name ?? summary?.name ?? null) || t("common.hidden")}
          </span>
        )}
        {seat.premade !== null &&
          (seat.premadeInferred ? (
            <GroupBadge
              group={seat.premade}
              inferred
              label={t("live.premadeInferred", { n: seat.premade })}
              title={t("live.premadeInferredHint")}
            />
          ) : (
            <GroupBadge group={seat.premade} label={t("live.premade", { n: seat.premade })} />
          ))}
        {seat.note && <NoteChip note={seat.note} />}
      </span>
      {/* The tier and its title get a line of their own: beside the name they squeezed it away. */}
      {rating && (
        <span className="flex min-w-0 items-center gap-1.5">
          <TierBadge rating={rating} />
          {rating.title && <TitleChip name={rating.title} />}
        </span>
      )}
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
        </span>
      ) : seat.stats.state === "loading" ? (
        <Skeleton className="h-3.5 w-40" />
      ) : seat.stats.state === "failed" ? (
        <span
          className="inline-flex items-center gap-1.5 text-[11.5px] text-fg-subtle"
          title={seat.stats.message}
        >
          <TriangleAlert size={12} strokeWidth={2} aria-hidden />
          {t("live.statsFailed")}
        </span>
      ) : null}
      {rating?.quip && (
        <span title={rating.quip} className="truncate text-[11px] text-fg-subtle">
          {t("live.quip", { quip: rating.quip })}
        </span>
      )}
    </span>
  );
}

/** A row's right half: the latest games as tiles, skeletons while the stats load. */
export function RecentTiles({ stats }: { stats: Seat["stats"] }) {
  const t = useT();
  const summary = stats.state === "ready" ? stats : null;
  const games = summary?.recent.matches.slice(0, TILE_ROOM.length) ?? [];
  return (
    <div className="@container min-w-0">
      {games.length > 0 ? (
        <ol aria-label={t("live.recentGames")} className="flex gap-1 overflow-hidden">
          {games.map((game, index) => (
            <GameTile key={game.gameId} game={game} index={index} />
          ))}
        </ol>
      ) : stats.state === "loading" ? (
        <span className="flex gap-1">
          {[0, 1, 2, 3, 4, 5].map((key) => (
            <Skeleton key={key} className="h-[58px] w-16" />
          ))}
        </span>
      ) : summary ? (
        <span className="text-[11.5px] text-fg-subtle">{t("live.noGames")}</span>
      ) : null}
    </div>
  );
}

function PlayerRow({ seat, onPlayer }: { seat: Seat; onPlayer: (puuid: string) => void }) {
  const t = useT();
  const summary = seat.stats.state === "ready" ? seat.stats : null;
  const name = riotId(seat.name ?? summary?.name ?? null);
  const who = (
    <>
      <ChampionIcon id={seat.championId} size={40} intent={seat.intent} />
      <Summary seat={seat} summary={summary} />
    </>
  );
  return (
    <li
      data-self={seat.isSelf || undefined}
      className={cx(
        "grid min-w-0 grid-cols-[minmax(0,340px)_minmax(0,1fr)] items-center gap-3 rounded-10 bg-surface p-2 hairline",
        seat.isSelf && "shadow-[inset_3px_0_0_0_var(--accent)]",
      )}
    >
      {seat.puuid ? (
        <button
          type="button"
          aria-label={t("live.openHistory", { name: name || t("common.hidden") })}
          onClick={() => seat.puuid && onPlayer(seat.puuid)}
          className="group flex min-w-0 items-center gap-3 rounded-6 p-1 text-left hover-wash"
        >
          {who}
          <ChevronRight
            size={14}
            strokeWidth={2}
            aria-hidden
            className="ml-auto shrink-0 text-fg-subtle opacity-0 transition-opacity duration-150 group-hover:opacity-100"
          />
        </button>
      ) : (
        <span className="flex min-w-0 items-center gap-3 p-1">{who}</span>
      )}
      <RecentTiles stats={seat.stats} />
    </li>
  );
}

export function TeamBoard({
  seats,
  onPlayer,
}: {
  seats: Seat[];
  onPlayer: (puuid: string) => void;
}) {
  return (
    <ul className="flex flex-col gap-1.5">
      {seats.map((seat, index) => (
        <PlayerRow key={seat.puuid ?? `seat-${index}`} seat={seat} onPlayer={onPlayer} />
      ))}
    </ul>
  );
}
