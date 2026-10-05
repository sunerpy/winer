// Formatting shared by the window and the in-client plugin, so both say "钻石 II · 56 LP" the same way.
import type { Language, Position, Rank, RecentForm, RiotId, Tier } from "./bindings";

export type Locale = Language;

/** `name#tag`, or just the name where the shard has no tags. */
export function riotId(name: RiotId | null | undefined): string {
  if (!name) return "";
  return name.tagLine ? `${name.gameName}#${name.tagLine}` : name.gameName;
}

/** (kills + assists) / deaths, with a perfect game counted as if there was one death. */
export function kda(kills: number, deaths: number, assists: number): number {
  return (kills + assists) / Math.max(deaths, 1);
}

export function formatKda(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1);
}

/** Wins over games, or `null` when there is nothing to divide. */
export function winRate(wins: number, games: number): number | null {
  return games > 0 ? wins / games : null;
}

export function percent(ratio: number | null): string {
  return ratio === null ? "—" : `${Math.round(ratio * 100)}%`;
}

const TIERS: Record<Tier, [zh: string, en: string]> = {
  IRON: ["坚韧黑铁", "Iron"],
  BRONZE: ["英勇黄铜", "Bronze"],
  SILVER: ["不屈白银", "Silver"],
  GOLD: ["荣耀黄金", "Gold"],
  PLATINUM: ["华贵铂金", "Platinum"],
  EMERALD: ["流光翡翠", "Emerald"],
  DIAMOND: ["璀璨钻石", "Diamond"],
  MASTER: ["超凡大师", "Master"],
  GRANDMASTER: ["傲世宗师", "Grandmaster"],
  CHALLENGER: ["最强王者", "Challenger"],
};

/** The short form a dense row has room for: `钻石`, `Diamond`. */
const TIER_SHORT: Record<Tier, [zh: string, en: string]> = {
  IRON: ["黑铁", "Iron"],
  BRONZE: ["黄铜", "Bronze"],
  SILVER: ["白银", "Silver"],
  GOLD: ["黄金", "Gold"],
  PLATINUM: ["铂金", "Plat"],
  EMERALD: ["翡翠", "Emerald"],
  DIAMOND: ["钻石", "Diamond"],
  MASTER: ["大师", "Master"],
  GRANDMASTER: ["宗师", "GM"],
  CHALLENGER: ["王者", "Challenger"],
};

/** One colour per tier, readable on both light and dark surfaces. */
export const TIER_COLORS: Record<Tier, string> = {
  IRON: "#8c8582",
  BRONZE: "#b0764f",
  SILVER: "#8d9eab",
  GOLD: "#cfa043",
  PLATINUM: "#3fb0a3",
  EMERALD: "#2fae6e",
  DIAMOND: "#5b8cf0",
  MASTER: "#a861d9",
  GRANDMASTER: "#e05a4f",
  CHALLENGER: "#e9b949",
};

const pick = (pair: [string, string], locale: Locale) => (locale === "en" ? pair[1] : pair[0]);

export function tierLabel(tier: Tier, locale: Locale, short = false): string {
  return pick((short ? TIER_SHORT : TIERS)[tier], locale);
}

/** `璀璨钻石 II`, or `超凡大师` where there are no divisions. */
export function rankLabel(rank: Rank, locale: Locale, short = false): string {
  const tier = tierLabel(rank.tier, locale, short);
  return rank.division ? `${tier} ${rank.division}` : tier;
}

export function lpLabel(rank: Rank): string {
  return `${rank.lp} LP`;
}

const POSITIONS: Record<Position, [string, string]> = {
  top: ["上单", "Top"],
  jungle: ["打野", "Jungle"],
  middle: ["中单", "Mid"],
  bottom: ["下路", "Bot"],
  utility: ["辅助", "Support"],
};

export function positionLabel(position: Position, locale: Locale): string {
  return pick(POSITIONS[position], locale);
}

/** `3 连胜` / `2 连败`; a single game is not a streak. */
export function streakLabel(streak: number, locale: Locale): string | null {
  if (Math.abs(streak) < 2) return null;
  const count = Math.abs(streak);
  if (locale === "en") return `${count}${streak > 0 ? "W" : "L"} streak`;
  return `${count} 连${streak > 0 ? "胜" : "败"}`;
}

/** Average K / D / A of a form, one decimal each. */
export function averageLine(form: RecentForm): string {
  return [form.kills, form.deaths, form.assists].map((value) => value.toFixed(1)).join(" / ");
}

/** `32:05`, or `1:02:40` past the hour. */
export function duration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const pad = (value: number) => String(value).padStart(2, "0");
  const [h, m, rest] = [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60];
  return h > 0 ? `${h}:${pad(m)}:${pad(rest)}` : `${m}:${pad(rest)}`;
}

/** A game's length in words, so it is never read as a time of day next to a date: `23分53秒`. */
export function gameLength(seconds: number, locale: Locale): string {
  const s = Math.max(0, Math.floor(seconds));
  const [m, rest] = [Math.floor(s / 60), s % 60];
  return locale === "en" ? `${m}m ${rest}s` : `${m}分${rest}秒`;
}

/** `刚刚`, `5 分钟前`, `3 小时前`, `2 天前`, then the date and time: `9-07 21:48`. */
export function relativeTime(at: number, now: number, locale: Locale): string {
  const minutes = Math.floor((now - at) / 60_000);
  const en = locale === "en";
  if (minutes < 1) return en ? "just now" : "刚刚";
  if (minutes < 60) return en ? `${minutes}m ago` : `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return en ? `${hours}h ago` : `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 7) return en ? `${days}d ago` : `${days} 天前`;
  const date = new Date(at);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getMonth() + 1}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** `12.3k` for the big numbers of a scoreboard. */
export function compact(value: number): string {
  if (Math.abs(value) < 1000) return String(value);
  return `${(value / 1000).toFixed(value < 10_000 ? 1 : 0)}k`;
}

export type TierTone = "best" | "good" | "middle" | "weak" | "worst";

/** The grades' letters, best first, as the core numbers them (`rating::GRADE_LETTERS`). */
export const GRADE_LETTERS = ["S+", "S", "A", "B", "C", "D", "E", "F"] as const;

/** Where a tier sits between best and worst, for colour: the ends are best and worst, the exact
 *  middle is middle, the rest good or weak. Five tiers use all five tones, three use best, middle
 *  and worst. */
export function tierTone(tier: number, tiers: number): TierTone {
  if (tiers <= 1) return "middle";
  const at = tier / (tiers - 1);
  if (at <= 0) return "best";
  if (at >= 1) return "worst";
  if (Math.abs(at - 0.5) < 1e-9) return "middle";
  return at < 0.5 ? "good" : "weak";
}
