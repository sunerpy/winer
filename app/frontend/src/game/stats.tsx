// Small readouts of rank and form, shared by every page that shows a player.
import {
  TIER_COLORS,
  formatKda,
  kda,
  lpLabel,
  percent,
  rankLabel,
  streakLabel,
  winRate,
  type Rank,
  type Ranked,
  type RecentForm,
  type RecentMatch,
} from "@winer/shared";

import { Flame } from "lucide-react";

import { useLanguage, useT } from "../lib/i18n";
import { cx } from "../lib/cx";
import { useCatalog } from "../lib/store";
import { Badge } from "../ui";

/** Tier dot, tier and division, and LP. */
export function RankBadge({
  rank,
  short = false,
  showLp = true,
  className,
}: {
  rank: Rank | null | undefined;
  short?: boolean;
  showLp?: boolean;
  className?: string;
}) {
  const t = useT();
  const language = useLanguage();
  if (!rank)
    return (
      <span className={cx("text-[12px] text-fg-subtle", className)}>{t("common.unranked")}</span>
    );
  return (
    <span
      className={cx(
        "inline-flex min-w-0 items-center gap-1.5 text-[12.5px] whitespace-nowrap",
        className,
      )}
    >
      <span
        aria-hidden
        className="size-2 shrink-0 rotate-45 rounded-[2px]"
        style={{ background: TIER_COLORS[rank.tier] }}
      />
      <span className="truncate font-medium text-fg">{rankLabel(rank, language, short)}</span>
      {showLp && <span className="mono text-[11px] text-fg-subtle">{lpLabel(rank)}</span>}
    </span>
  );
}

/** Solo rank, or flex when there is no solo rank. */
export function bestRank(ranked: Ranked): { rank: Rank | null; queue: "solo" | "flex" } {
  if (ranked.solo) return { rank: ranked.solo, queue: "solo" };
  return { rank: ranked.flex, queue: "flex" };
}

/** A win rate coloured only where it says something: clearly above or below half. */
export function WinRate({
  wins,
  games,
  className,
}: {
  wins: number;
  games: number;
  className?: string;
}) {
  const ratio = winRate(wins, games);
  const tone =
    ratio === null
      ? "text-fg-subtle"
      : ratio >= 0.55
        ? "text-win"
        : ratio <= 0.45
          ? "text-loss"
          : "text-fg";
  return <span className={cx("mono font-medium", tone, className)}>{percent(ratio)}</span>;
}

export function KdaValue({
  kills,
  deaths,
  assists,
  className,
}: {
  kills: number;
  deaths: number;
  assists: number;
  className?: string;
}) {
  const value = kda(kills, deaths, assists);
  const tone = value >= 4 ? "text-accent-text" : value < 2 ? "text-fg-muted" : "text-fg";
  return <span className={cx("mono font-medium", tone, className)}>{formatKda(value)}</span>;
}

/** A roast title (`版本答案`), with what earned it as the tooltip when that is known. */
export function TitleChip({
  name,
  why,
  small = false,
}: {
  name: string;
  why?: string;
  small?: boolean;
}) {
  return (
    // `relative` keeps the hidden reason inside the chip; loose, it would widen the document.
    <span
      title={why}
      className={cx(
        "relative inline-flex shrink-0 items-center gap-0.5 rounded-4 border border-dashed border-border-strong font-medium whitespace-nowrap text-fg-muted",
        small ? "h-4 px-1 text-[10px]" : "h-5 px-1.5 text-[11px]",
      )}
    >
      <Flame size={small ? 10 : 11} strokeWidth={2} aria-hidden className="text-warning" />
      {name}
      {why && <span className="sr-only">{`：${why}`}</span>}
    </span>
  );
}

export function StreakBadge({ streak }: { streak: number }) {
  const label = streakLabel(streak, useLanguage());
  if (!label) return null;
  return <Badge tone={streak > 0 ? "win" : "loss"}>{label}</Badge>;
}

/** The last games as ticks, newest first: blue a win, red a loss, grey a remake. */
export function ResultStrip({
  matches,
  limit = 10,
  className,
}: {
  matches: RecentMatch[];
  limit?: number;
  className?: string;
}) {
  const t = useT();
  const catalog = useCatalog();
  const shown = matches.slice(0, limit);
  return (
    <span
      className={cx("inline-flex items-end gap-[3px]", className)}
      aria-label={shown
        .map((game) =>
          game.remake ? t("common.remake") : game.win ? t("common.win") : t("common.loss"),
        )
        .join(" ")}
    >
      {shown.map((game) => (
        <span
          key={game.gameId}
          title={`${catalog?.champions.get(game.championId)?.name ?? ""} ${game.kills}/${game.deaths}/${game.assists}`}
          className={cx(
            "block h-3.5 w-1.5 rounded-[2px]",
            game.remake ? "bg-border-strong" : game.win ? "bg-win" : "bg-loss",
          )}
        />
      ))}
    </span>
  );
}

/** `12 胜 8 负 · 60%`. */
export function FormLine({ form }: { form: RecentForm }) {
  const t = useT();
  return (
    <span className="inline-flex items-center gap-1.5 text-[12px] text-fg-muted">
      <span className="mono text-fg">{form.wins}</span>
      {t("common.win")}
      <span className="mono text-fg">{form.games - form.wins}</span>
      {t("common.loss")}
      <span aria-hidden>·</span>
      <WinRate wins={form.wins} games={form.games} />
    </span>
  );
}
