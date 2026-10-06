// The party in the client's lobby: above each member's banner, one compact line of their recent
// form (win rate, KDA, form score) that opens their history in winer when clicked, as a click on the
// member's avatar does. Where the members cannot be found, a compact panel lists the same.
import {
  formatKda,
  kda,
  percent,
  riotId,
  winRate,
  type Language,
  type LobbyMember,
  type LobbyView,
} from "@winer/shared";

import { h } from "./dom";
import { first, innermost } from "./find";
import { text } from "./i18n";
import { LOBBY } from "./selectors";

const LINE_CLASS = "winer-lobby";
/** On a member's card: whose card it is, for a click on the avatar. */
const CARD_ATTRIBUTE = "data-winer-puuid";
/** What a click on these is for is the client's own business. */
const CONTROLS =
  "button, a, input, select, [role='button'], [class*='button'], [class*='dropdown']";

const ID_SELECTOR = LOBBY.ids.map((id) => `[${id}]`).join(",");

/** A card that can be told apart: it shows a name, or holds an id the client wrote. */
function identifiable(card: Element): boolean {
  return (
    first(card, LOBBY.name) !== null ||
    card.matches(ID_SELECTOR) ||
    card.querySelector(ID_SELECTOR) !== null
  );
}

/** `胜率 60% · KDA 3.2 · 战力 7.4`, or what stands in while there are no stats. */
export function lobbyLine(member: LobbyMember, language: Language): string {
  const stats = member.stats;
  if (stats.state === "loading") return text(language, "loading");
  if (stats.state === "hidden") return text(language, "hidden");
  if (stats.state === "failed") return text(language, "failed");
  const form = stats.recent;
  const parts = [];
  if (form.games > 0) {
    parts.push(`${text(language, "winRate")} ${percent(winRate(form.wins, form.games))}`);
    parts.push(`KDA ${formatKda(kda(form.kills, form.deaths, form.assists))}`);
  }
  if (member.score !== null) parts.push(`${text(language, "score")} ${member.score.toFixed(1)}`);
  return parts.length > 0 ? parts.join(" · ") : text(language, "noGames");
}

/** The member a card shows: by an id the client wrote into it, else by the name on it. */
function memberOf(card: Element, members: LobbyMember[]): LobbyMember | null {
  for (const holder of [card, ...card.querySelectorAll(ID_SELECTOR)]) {
    for (const attribute of LOBBY.ids) {
      const value = holder.getAttribute(attribute)?.trim();
      const member = value ? members.find((candidate) => candidate.puuid === value) : undefined;
      if (member) return member;
    }
  }
  const shown = first(card, LOBBY.name)?.textContent?.trim().toLowerCase();
  if (!shown) return null;
  return (
    members.find((member) => {
      const name = member.name;
      if (!name) return false;
      const plain = name.gameName.trim().toLowerCase();
      return shown === plain || shown === riotId(name).toLowerCase();
    }) ?? null
  );
}

/** Writes one line on each member's banner, under the tokens. Returns how many cards it decorated:
 *  the cards it could not tell apart, or that show no one in `view`, get nothing. */
export function decorateLobby(
  root: ParentNode,
  view: LobbyView,
  language: Language,
  open: (puuid: string) => void,
): number {
  const cards = innermost(root, LOBBY.member, identifiable);
  const kept = new Set<Element>();
  for (const card of cards) {
    const member = memberOf(card, view.members);
    if (!member) {
      card.removeAttribute(CARD_ATTRIBUTE);
      continue;
    }
    if (card.getAttribute(CARD_ATTRIBUTE) !== member.puuid)
      card.setAttribute(CARD_ATTRIBUTE, member.puuid);
    let line = card.querySelector<HTMLElement>(`.${LINE_CLASS}`);
    if (!line) {
      line = h("button", { type: "button", class: LINE_CLASS, title: text(language, "open") });
      // The card it sits on says whose it is at the moment of the click.
      line.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        const puuid = line?.closest(`[${CARD_ATTRIBUTE}]`)?.getAttribute(CARD_ATTRIBUTE);
        if (puuid) open(puuid);
      });
      const under = first(card, LOBBY.under);
      const banner = first(card, LOBBY.banner);
      if (under) under.after(line);
      else if (banner) banner.before(line);
      else card.prepend(line);
    }
    const said = lobbyLine(member, language);
    // Written only when it changed: every write wakes the observer that drives rendering.
    if (line.textContent !== said) line.textContent = said;
    kept.add(line);
  }
  root.querySelectorAll(`.${LINE_CLASS}`).forEach((line) => {
    if (!kept.has(line)) line.remove();
  });
  return kept.size;
}

export function clearLobby(root: ParentNode): void {
  root.querySelectorAll(`.${LINE_CLASS}`).forEach((line) => line.remove());
  root
    .querySelectorAll(`[${CARD_ATTRIBUTE}]`)
    .forEach((card) => card.removeAttribute(CARD_ATTRIBUTE));
}

/** Opens a member's history from a click on their avatar, in the capture phase; a click on the
 *  client's own controls there passes through. `open` says whether it took the click. */
export function interceptAvatarClicks(doc: Document, open: (puuid: string) => boolean): () => void {
  const onClick = (event: MouseEvent) => {
    if (!(event.target instanceof Element)) return;
    const card = event.target.closest(`[${CARD_ATTRIBUTE}]`);
    const puuid = card?.getAttribute(CARD_ATTRIBUTE);
    if (!card || !puuid || event.target.closest(CONTROLS)) return;
    const avatar = LOBBY.avatar
      .map((selector) => event.target instanceof Element && event.target.closest(selector))
      .find((found) => found && card.contains(found));
    if (!avatar || !open(puuid)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  doc.addEventListener("click", onClick, true);
  return () => doc.removeEventListener("click", onClick, true);
}

export interface FloatingPanel {
  host: HTMLElement;
  body: HTMLElement;
}

/** The fallback panel's frame, as champ select's: a header that folds it, a body for the rows. */
export function floatingPanel(doc: Document, title: string, context: string): FloatingPanel {
  const head = h(
    "button",
    { type: "button", class: "winer-head" },
    h("span", {}, `winer · ${title}`),
    h("span", { class: "winer-chevron" }, "▾"),
  );
  const body = h("div");
  const host = h(
    "section",
    {
      class: "winer-panel",
      "data-winer-context": context,
      "data-winer-panel": "lobby",
      "data-collapsed": "false",
    },
    head,
    body,
  );
  head.addEventListener("click", () => {
    host.dataset.collapsed = String(host.dataset.collapsed !== "true");
  });
  doc.body.append(host);
  return { host, body };
}

/** The fallback panel's rows: each member's name and line, a button that opens their history. */
export function lobbyRows(
  view: LobbyView,
  language: Language,
  open: (puuid: string) => void,
): HTMLElement {
  return h(
    "ol",
    { class: "winer-rows" },
    ...view.members.map((member) => {
      const button = h(
        "button",
        {
          type: "button",
          class: member.isSelf
            ? "winer-row winer-row--self winer-row--button"
            : "winer-row winer-row--button",
          title: text(language, "open"),
        },
        h(
          "span",
          { class: "winer-who" },
          h("span", { class: "winer-name" }, riotId(member.name) || text(language, "hidden")),
          h("span", { class: "winer-line" }, lobbyLine(member, language)),
        ),
      );
      button.addEventListener("click", () => open(member.puuid));
      return h("li", {}, button);
    }),
  );
}
