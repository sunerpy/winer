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
  type SeatRating,
  type Side,
} from "@winer/shared";

import { championIcon, h } from "./dom";
import { premadeChip } from "./groups";
import { text } from "./i18n";

/** The client's party rows and the name block inside each. Class names are the client's and change
 *  between patches: a miss degrades to the panel. */
export const ROW_SELECTOR = ".party.visible .summoner-wrapper.visible.left";
export const NAME_SELECTOR = ".player-name-wrapper";
/** The row's column of text beside the champion: the client's status line, the position and the
 *  name, clipped to those three (measured on 16.19); the name's own box is one 16px line. */
export const DETAILS_SELECTOR = ".player-details";
const INLINE_CLASS = "winer-inline";
/** On a line: whose it is, for a click on it (the lobby's cards carry the same). */
const PUUID_ATTRIBUTE = "data-winer-puuid";

function summaryOf(seat: Seat): PlayerSummary | null {
  return seat.stats.state === "ready" ? seat.stats : null;
}

export function rankChip(summary: PlayerSummary, language: Language): HTMLElement {
  const rank = summary.ranked.solo ?? summary.ranked.flex;
  if (!rank) return h("span", { class: "winer-muted" }, text(language, "unranked"));
  return h(
    "span",
    { class: "winer-rank" },
    h("span", { class: "winer-tier", style: `background:${TIER_COLORS[rank.tier]}` }),
    rankLabel(rank, language, true),
  );
}

export function rateClass(ratio: number | null): string {
  if (ratio === null) return "winer-muted";
  return ratio >= 0.55 ? "winer-win" : ratio <= 0.45 ? "winer-loss" : "";
}

/** The tier, its grade letter first where the scheme grades; the score and the quip on hover. */
export function standingChip(rating: SeatRating | null): HTMLElement | null {
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
export function titleChip(rating: SeatRating | null): HTMLElement | null {
  const title = rating?.title;
  return title ? h("span", { class: "winer-title" }, title) : null;
}

/** The local player's side, on a map of two: the client's champ select never says it. */
function sideChip(seat: Seat, side: Side | null, language: Language): HTMLElement | null {
  if (!seat.isSelf || !side) return null;
  return h("span", { class: `winer-side winer-side--${side}` }, text(language, side));
}

/** `蓝色方 · 开黑 1 · 峡谷公务员 · 版本答案 · 钻石 II · 60% · KDA 3.2 · 3 连胜`, the side on the
 *  local player's line only, the party where one is known. */
export function statsLine(seat: Seat, language: Language, side: Side | null = null): HTMLElement {
  const summary = summaryOf(seat);
  const chip = sideChip(seat, side, language);
  const party = seat.premade === null ? null : premadeChip(seat.premade, language);
  if (seat.stats.state === "loading")
    return h(
      "span",
      { class: "winer-line" },
      chip,
      party,
      h("span", { class: "winer-muted" }, text(language, "loading")),
    );
  if (!summary)
    return h(
      "span",
      { class: "winer-line" },
      chip,
      party,
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
    party,
    standingChip(seat.rating),
    titleChip(seat.rating),
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
    seat.premade,
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

/** Writes one line under each party row's name. Returns how many rows it found. A line names its
 *  player (`data-winer-puuid`), so a click on it opens their history (`history.ts`). It goes at the
 *  end of the row's details column, which the style lets it show below (`style.css`): inside the
 *  name's box the client clips it away. A row without that column takes it after the name. */
export function decorateRows(root: ParentNode, view: ChampSelectView, language: Language): number {
  const rows = [...root.querySelectorAll<HTMLElement>(ROW_SELECTOR)];
  rows.forEach((row, index) => {
    const seat = view.myTeam[index];
    const details = row.querySelector<HTMLElement>(DETAILS_SELECTOR);
    const anchor = row.querySelector<HTMLElement>(NAME_SELECTOR);
    if (!seat || !(details || anchor)) return;
    const line =
      row.querySelector<HTMLElement>(`.${INLINE_CLASS}`) ?? h("div", { class: INLINE_CLASS });
    // Moved only when it is not there yet, or the client redrew the column without it.
    if (details) {
      if (line.parentElement !== details) details.append(line);
    } else if (!line.isConnected) anchor?.after(line);
    const key = lineKey(seat, language, view.side);
    if (line.dataset.key !== key) {
      line.dataset.key = key;
      line.replaceChildren(statsLine(seat, language, view.side));
    }
    // The history panel. Written only when it changed, as the line is.
    const puuid = seat.puuid ?? "";
    if ((line.getAttribute(PUUID_ATTRIBUTE) ?? "") !== puuid) {
      if (puuid) line.setAttribute(PUUID_ATTRIBUTE, puuid);
      else line.removeAttribute(PUUID_ATTRIBUTE);
    }
    const title = puuid ? text(language, "open") : "";
    if ((line.getAttribute("title") ?? "") !== title) {
      if (title) line.setAttribute("title", title);
      else line.removeAttribute("title");
    }
  });
  return rows.length;
}

export function clearRows(root: ParentNode): void {
  root.querySelectorAll(`.${INLINE_CLASS}`).forEach((line) => line.remove());
}

/** The fallback panel's rows: icon, name, rank and form, last ten results. With `open`, a known
 *  player's row is a button that opens their history, as the lobby panel's rows are. */
export function panelRows(
  view: ChampSelectView,
  language: Language,
  open?: (puuid: string, anchor: Element) => void,
): HTMLElement {
  return h(
    "ol",
    { class: "winer-rows" },
    ...view.myTeam.map((seat) => {
      const summary = summaryOf(seat);
      const content = [
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
      ];
      const kind = seat.isSelf ? "winer-row winer-row--self" : "winer-row";
      const puuid = seat.puuid;
      if (!open || !puuid) return h("li", { class: kind }, ...content);
      const button = h(
        "button",
        { type: "button", class: `${kind} winer-row--button`, title: text(language, "open") },
        ...content,
      );
      button.addEventListener("click", () => open(puuid, button));
      return h("li", {}, button);
    }),
  );
}
