// Teammates' form inside the client's champ select: a line under each name in the client's own
// party rows, or, when those rows cannot be found, a compact panel of the same facts.
import {
  GRADE_LETTERS,
  TIER_COLORS,
  formatKda,
  kda,
  percent,
  rankLabel,
  riotId,
  streakLabel,
  tierTone,
  winRate,
  type ChampSelectView,
  type Language,
  type PlayerSummary,
  type Seat,
  type Side,
} from "@winer/shared";

import { championIcon, h } from "./dom";
import { text } from "./i18n";

/** The client's party rows and the name block inside each. Class names are the client's and change
 *  between patches: a miss degrades to the panel. */
export const ROW_SELECTOR = ".party.visible .summoner-wrapper.visible.left";
export const NAME_SELECTOR = ".player-name-wrapper";
const INLINE_CLASS = "winer-inline";

function summaryOf(seat: Seat): PlayerSummary | null {
  return seat.stats.state === "ready" ? seat.stats : null;
}

function rankChip(summary: PlayerSummary, language: Language): HTMLElement {
  const rank = summary.ranked.solo ?? summary.ranked.flex;
  if (!rank) return h("span", { class: "winer-muted" }, text(language, "unranked"));
  return h(
    "span",
    { class: "winer-rank" },
    h("span", { class: "winer-tier", style: `background:${TIER_COLORS[rank.tier]}` }),
    rankLabel(rank, language, true),
  );
}

function rateClass(ratio: number | null): string {
  if (ratio === null) return "winer-muted";
  return ratio >= 0.55 ? "winer-win" : ratio <= 0.45 ? "winer-loss" : "";
}

/** The tier, its grade letter first where the scheme grades; the score and the quip on hover. */
function standingChip(seat: Seat): HTMLElement | null {
  const rating = seat.rating;
  if (!rating) return null;
  const grade = rating.grade === null ? undefined : GRADE_LETTERS[rating.grade];
  return h(
    "span",
    {
      class: `winer-standing winer-standing--${tierTone(rating.tier, rating.tiers)}`,
      title: [rating.score.toFixed(1), rating.quip].filter(Boolean).join(" · "),
    },
    grade ? `${grade} ${rating.label}` : rating.label,
  );
}

/** The roast title recent games earned (`版本答案`), while titles are on. */
function titleChip(seat: Seat): HTMLElement | null {
  const title = seat.rating?.title;
  return title ? h("span", { class: "winer-title" }, title) : null;
}

/** The local player's side, on a map of two: the client's champ select never says it. */
function sideChip(seat: Seat, side: Side | null, language: Language): HTMLElement | null {
  if (!seat.isSelf || !side) return null;
  return h("span", { class: `winer-side winer-side--${side}` }, text(language, side));
}

/** `蓝色方 · 峡谷公务员 · 版本答案 · 钻石 II · 60% · KDA 3.2 · 3 连胜`, the side on the local
 *  player's line only. */
export function statsLine(seat: Seat, language: Language, side: Side | null = null): HTMLElement {
  const summary = summaryOf(seat);
  const chip = sideChip(seat, side, language);
  if (seat.stats.state === "loading")
    return h(
      "span",
      { class: "winer-line" },
      chip,
      h("span", { class: "winer-muted" }, text(language, "loading")),
    );
  if (!summary)
    return h(
      "span",
      { class: "winer-line" },
      chip,
      h(
        "span",
        { class: "winer-muted" },
        seat.stats.state === "hidden" ? text(language, "hidden") : text(language, "failed"),
      ),
    );
  const form = summary.recent;
  const ratio = winRate(form.wins, form.games);
  const streak = streakLabel(form.streak, language);
  return h(
    "span",
    { class: "winer-line" },
    chip,
    standingChip(seat),
    titleChip(seat),
    rankChip(summary, language),
    form.games > 0 && h("span", { class: rateClass(ratio) }, percent(ratio)),
    form.games > 0 && h("span", {}, `KDA ${formatKda(kda(form.kills, form.deaths, form.assists))}`),
    streak && h("span", { class: form.streak > 0 ? "winer-win" : "winer-loss" }, streak),
  );
}

function resultTicks(summary: PlayerSummary): HTMLElement {
  return h(
    "span",
    { class: "winer-ticks" },
    ...summary.recent.matches
      .slice(0, 10)
      .map((game) =>
        h("i", { class: game.remake ? "" : game.win ? "winer-tick-win" : "winer-tick-loss" }),
      ),
  );
}

/** What a line shows, so an unchanged line is not rewritten: every write is a DOM mutation, and
 *  the observer that drives rendering would answer it with another render. */
export function lineKey(seat: Seat, language: Language, side: Side | null = null): string {
  const summary = summaryOf(seat);
  const rank = summary ? (summary.ranked.solo ?? summary.ranked.flex) : null;
  const form = summary?.recent;
  return [
    language,
    seat.isSelf ? side : null,
    seat.stats.state,
    seat.rating?.label,
    seat.rating?.grade,
    seat.rating?.score,
    seat.rating?.title,
    seat.rating?.quip,
    rank?.tier,
    rank?.division,
    form?.wins,
    form?.games,
    form?.streak,
    form?.kills,
    form?.deaths,
    form?.assists,
  ].join("|");
}

/** Writes one line under each party row's name. Returns how many rows it found. */
export function decorateRows(root: ParentNode, view: ChampSelectView, language: Language): number {
  const rows = [...root.querySelectorAll<HTMLElement>(ROW_SELECTOR)];
  rows.forEach((row, index) => {
    const seat = view.myTeam[index];
    const anchor = row.querySelector<HTMLElement>(NAME_SELECTOR);
    if (!seat || !anchor) return;
    let line = row.querySelector<HTMLElement>(`.${INLINE_CLASS}`);
    if (!line) {
      line = h("div", { class: INLINE_CLASS });
      anchor.after(line);
    }
    const key = lineKey(seat, language, view.side);
    if (line.dataset.key !== key) {
      line.dataset.key = key;
      line.replaceChildren(statsLine(seat, language, view.side));
    }
  });
  return rows.length;
}

export function clearRows(root: ParentNode): void {
  root.querySelectorAll(`.${INLINE_CLASS}`).forEach((line) => line.remove());
}

/** The fallback panel's rows: icon, name, rank and form, last ten results. */
export function panelRows(view: ChampSelectView, language: Language): HTMLElement {
  return h(
    "ol",
    { class: "winer-rows" },
    ...view.myTeam.map((seat) => {
      const summary = summaryOf(seat);
      return h(
        "li",
        { class: seat.isSelf ? "winer-row winer-row--self" : "winer-row" },
        championIcon(seat.championId, 28),
        h(
          "span",
          { class: "winer-who" },
          h(
            "span",
            { class: "winer-name" },
            riotId(seat.name ?? summary?.name ?? null) || text(language, "hidden"),
          ),
          statsLine(seat, language, view.side),
        ),
        summary ? resultTicks(summary) : null,
      );
    }),
  );
}
