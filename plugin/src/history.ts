// The history panel: a player's latest games in a card over the client page, opened by a click on
// a lobby member or a champ-select teammate while 在客户端里查看战绩 is on. The games come from winer
// over the bridge (`history`, answered by `historyResult`); the summary at the top is what the
// clicked line shows, from the view winer already sends, and follows it while the panel is open.
// One panel at a time: it carries the context that drew it, and the drawing context removes any
// other's (`removePanels`), as with the other panels.
import {
  formatKda,
  gameLength,
  kda,
  percent,
  relativeTime,
  streakLabel,
  winRate,
  type IpcError,
  type Language,
  type PanelGame,
  type PanelHistory,
  type PlayerStats,
  type RiotId,
  type SeatRating,
} from "@winer/shared";

import type { HistoryResult } from "./bridge";
import { championIcon, h, profileIcon } from "./dom";
import { text } from "./i18n";
import { ROW_SELECTOR, rankChip, rateClass, standingChip, titleChip } from "./team";

/** The panel's kind among the plugin's panels (`data-winer-panel`). */
export const HISTORY_PANEL = "history";
const PANEL_SELECTOR = `.winer-panel[data-winer-panel="${HISTORY_PANEL}"]`;
/** On a lobby member's card and on a champ-select line: whose it is. */
const PUUID_ATTRIBUTE = "data-winer-puuid";
/** winer's own player lines: the lobby's, under a member's tokens, and champ select's. */
const PLAYER_LINES = ".winer-lobby, .winer-inline";
/** Pengu Loader's root, measured lying over parts of the client page (docs/platform-notes.md). */
const OVERLAY = "#pengu-root";
/** Longer than winer's own limit on one lookup (30 s): only an answer that got lost runs into it. */
export const ANSWER_TIMEOUT_MS = 40_000;
/** The panel's width in `style.css`, for placing it before it has been laid out. */
const WIDTH = 320;
/** About the height of the panel with its ten games: it is placed for that height from the start,
 *  so it does not move when the games arrive. */
const RESERVED_HEIGHT = 480;
/** Between the panel and what it sits beside, and between the panel and the window's edges. */
const GAP = 8;
const MARGIN = 8;

/** Whom the panel is about, as the view shows them. */
export interface Subject {
  puuid: string;
  name: RiotId | null;
  /** The profile icon, 0 where the view has none. */
  iconId: number;
  stats: PlayerStats;
  /** The tier within the team: in champ select only. */
  rating: SeatRating | null;
  /** Recent form, 0–10. */
  score: number | null;
}

/** Where the panel was opened: it closes when the client leaves that screen. */
export type Surface = "lobby" | "champSelect";

export interface HistoryActions {
  /** Asks winer for the player's games: the request's id, or null while there is no connection. */
  request(puuid: string): number | null;
  /** Shows the player's history in winer's window. */
  openInWiner(puuid: string): void;
}

type Content =
  | { state: "loading" }
  | { state: "ready"; page: PanelHistory }
  | { state: "failed"; error: IpcError | "timeout" };

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Where the panel goes: beside `anchor` on the side with room for it, its top level with the
 *  anchor's; under or over it where neither side has room; always inside the window. Without an
 *  anchor, at the top right, where the other panels sit. */
export function placement(
  anchor: Box | null,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number } {
  const fit = (value: number, length: number, room: number) =>
    Math.round(Math.max(MARGIN, Math.min(value, room - length - MARGIN)));
  if (!anchor) {
    return {
      left: fit(viewport.width - size.width - 12, size.width, viewport.width),
      top: fit(76, size.height, viewport.height),
    };
  }
  const after = anchor.left + anchor.width + GAP;
  const before = anchor.left - GAP - size.width;
  if (after + size.width <= viewport.width - MARGIN || before >= MARGIN) {
    const left = after + size.width <= viewport.width - MARGIN ? after : before;
    return {
      left: fit(left, size.width, viewport.width),
      top: fit(anchor.top, size.height, viewport.height),
    };
  }
  const below = anchor.top + anchor.height + GAP;
  const above = anchor.top - GAP - size.height;
  const top =
    below + size.height <= viewport.height - MARGIN ? below : above >= MARGIN ? above : anchor.top;
  return {
    left: fit(anchor.left + (anchor.width - size.width) / 2, size.width, viewport.width),
    top: fit(top, size.height, viewport.height),
  };
}

function box(rect: DOMRect): Box | null {
  if (rect.width === 0 && rect.height === 0) return null;
  return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
}

/** What the panel goes beside: champ select's whole row, a lobby member's card, else what was
 *  clicked (a fallback panel's row). */
export function anchorOf(element: Element | null | undefined): Element | null {
  if (!element) return null;
  return (
    element.closest(ROW_SELECTOR) ??
    element.closest(`[${PUUID_ATTRIBUTE}]:not(.winer-inline)`) ??
    element
  );
}

/** The element `selector` finds at a click: from its target, or, where Pengu's root took the click,
 *  under the click's point (as the bench's clicks are found, `bench.ts`). */
function clicked(doc: Document, event: MouseEvent, selector: string): Element | null {
  const target = event.target instanceof Element ? event.target : null;
  const direct = target?.closest(selector) ?? null;
  if (direct || !target?.closest(OVERLAY) || typeof doc.elementsFromPoint !== "function") {
    return direct;
  }
  for (const element of doc.elementsFromPoint(event.clientX, event.clientY)) {
    const found = element.closest(selector);
    if (found) return found;
  }
  return null;
}

/** Hands clicks on winer's player lines to `pick` in the capture phase, with the player's puuid and
 *  the line; when `pick` takes one, nothing else sees it. Each context registers its own and only
 *  the drawing context's `pick` takes a click, so a line the page's other context drew still
 *  answers. A click Pengu's root took over a button of the history panel goes to that button. */
export function interceptPlayerClicks(
  doc: Document,
  pick: (puuid: string, line: Element) => boolean,
): () => void {
  const onClick = (event: MouseEvent) => {
    const line = clicked(doc, event, PLAYER_LINES);
    const puuid = line?.closest(`[${PUUID_ATTRIBUTE}]`)?.getAttribute(PUUID_ATTRIBUTE);
    if (line && puuid) {
      if (!pick(puuid, line)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    if (!(event.target instanceof Element) || !event.target.closest(OVERLAY)) return;
    const button = clicked(doc, event, `${PANEL_SELECTOR} button`);
    if (!(button instanceof HTMLElement)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    button.click();
  };
  doc.addEventListener("click", onClick, true);
  return () => doc.removeEventListener("click", onClick, true);
}

/** `峡谷通天代 · 版本答案 · 钻石 II · 胜率 60% · KDA 3.2 · 战力 7.4 · 3 连胜` as chips: the clicked
 *  line's summary, the tier and title where the view rates the team (champ select). */
export function summaryLine(subject: Subject, language: Language): HTMLElement {
  const stats = subject.stats;
  if (stats.state !== "ready") {
    const word =
      stats.state === "loading" ? "loading" : stats.state === "hidden" ? "hidden" : "failed";
    return h(
      "span",
      { class: "winer-line winer-history-summary" },
      h("span", { class: "winer-muted" }, text(language, word)),
    );
  }
  const form = stats.recent;
  const ratio = winRate(form.wins, form.games);
  const streak = streakLabel(form.streak, language);
  const played = form.games > 0;
  return h(
    "span",
    { class: "winer-line winer-history-summary" },
    standingChip(subject.rating),
    titleChip(subject.rating),
    rankChip(stats, language),
    played &&
      h("span", { class: rateClass(ratio) }, `${text(language, "winRate")} ${percent(ratio)}`),
    played && h("span", {}, `KDA ${formatKda(kda(form.kills, form.deaths, form.assists))}`),
    subject.score !== null &&
      h("span", {}, `${text(language, "score")} ${subject.score.toFixed(1)}`),
    streak && h("span", { class: form.streak > 0 ? "winer-win" : "winer-loss" }, streak),
    !played && h("span", { class: "winer-muted" }, text(language, "noGames")),
  );
}

function displayName(subject: Subject, language: Language): RiotId | string {
  const summary = subject.stats.state === "ready" ? subject.stats : null;
  return subject.name ?? summary?.name ?? text(language, "hidden");
}

/** The icon, the name with its tag, and the summary. */
function identity(subject: Subject, language: Language): HTMLElement[] {
  const summary = subject.stats.state === "ready" ? subject.stats : null;
  const name = displayName(subject, language);
  const icon = profileIcon(subject.iconId || summary?.iconId || 0, 32);
  const who = h(
    "span",
    { class: "winer-who" },
    typeof name === "string"
      ? h("span", { class: "winer-name" }, name)
      : h(
          "span",
          { class: "winer-name" },
          name.gameName,
          name.tagLine ? h("span", { class: "winer-tag" }, `#${name.tagLine}`) : null,
        ),
    summaryLine(subject, language),
  );
  return icon ? [icon, who] : [who];
}

/** One game: champion, result, queue, when and how long, K / D / A. */
export function gameRow(game: PanelGame, language: Language, now: number): HTMLElement {
  const result = game.remake ? "remake" : game.win ? "win" : "loss";
  const word = text(
    language,
    result === "win" ? "victory" : result === "loss" ? "defeat" : "remake",
  );
  const queue = game.queue || text(language, "otherMode");
  const length = gameLength(game.duration, language);
  const place =
    game.placement === null
      ? null
      : text(language, "placement").replace("{n}", String(game.placement));
  return h(
    "li",
    {
      class: `winer-game winer-game--${result}`,
      title: [queue, word, `${game.kills}/${game.deaths}/${game.assists}`, length].join(" · "),
    },
    championIcon(game.championId, 28),
    h(
      "span",
      { class: "winer-game-what" },
      h(
        "span",
        { class: "winer-game-head" },
        h("b", { class: "winer-game-result" }, word),
        h("span", { class: "winer-game-queue" }, queue),
        place && h("span", { class: "winer-game-place" }, place),
        game.award &&
          h(
            "span",
            { class: `winer-award winer-award--${game.award}` },
            game.award === "mvp" ? "MVP" : "SVP",
          ),
      ),
      h(
        "span",
        { class: "winer-game-when" },
        `${relativeTime(game.startedAt, now, language)} · ${length}`,
      ),
    ),
    h(
      "span",
      { class: "winer-game-kda" },
      String(game.kills),
      h("span", { class: "winer-game-slash" }, " / "),
      h("span", { class: "winer-loss" }, String(game.deaths)),
      h("span", { class: "winer-game-slash" }, " / "),
      String(game.assists),
    ),
  );
}

/** What a failure says: in plain words, and winer's own where those say no more. */
function failure(
  error: IpcError | "timeout",
  language: Language,
): { message: string; detail: string | null } {
  if (error === "timeout") return { message: text(language, "historyTimeout"), detail: null };
  switch (error.code) {
    case "notConnected":
      return { message: text(language, "historyOffline"), detail: null };
    case "invalid":
      return { message: text(language, "historyInvalid"), detail: null };
    case "busy":
      return { message: text(language, "historyBusy"), detail: null };
    default:
      return { message: text(language, "historyFailed"), detail: error.message || null };
  }
}

function content(shown: Content, language: Language, now: number, retry: () => void): HTMLElement {
  if (shown.state === "loading") {
    return h(
      "p",
      { class: "winer-history-note", role: "status" },
      text(language, "historyLoading"),
    );
  }
  if (shown.state === "failed") {
    const { message, detail } = failure(shown.error, language);
    const again = h(
      "button",
      { type: "button", class: "winer-history-retry" },
      text(language, "historyRetry"),
    );
    again.addEventListener("click", retry);
    return h(
      "div",
      { class: "winer-history-note winer-history-note--failed", role: "alert" },
      h("span", { class: "winer-history-message" }, message),
      detail && h("span", { class: "winer-history-detail", title: detail }, detail),
      again,
    );
  }
  const games = shown.page.games;
  if (games.length === 0) return h("p", { class: "winer-history-note" }, text(language, "noGames"));
  return h("ol", { class: "winer-games" }, ...games.map((game) => gameRow(game, language, now)));
}

/** The panel of one context: at most one open, on one player. */
export class HistoryPanel {
  #host: HTMLElement | null = null;
  #who: HTMLElement | null = null;
  #body: HTMLElement | null = null;
  #close: HTMLElement | null = null;
  #link: HTMLElement | null = null;
  #subject: Subject | null = null;
  #surface: Surface | null = null;
  #anchor: Element | null = null;
  /** Where the anchor was when the panel opened, for when the client has redrawn it since. */
  #anchorBox: Box | null = null;
  #language: Language = "zh-CN";
  #shown: Content = { state: "loading" };
  /** The request whose answer the panel waits for; an answer to any other is stale. */
  #request: number | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** What the identity block shows, so an unchanged one is not rewritten (`team.ts`'s `lineKey`). */
  #whoKey = "";
  /** What had the focus before the panel took it. */
  #previous: Element | null = null;

  constructor(
    private readonly doc: Document,
    private readonly context: string,
    private readonly actions: HistoryActions,
  ) {}

  get isOpen(): boolean {
    return this.#host?.isConnected === true;
  }

  /** The open panel's player. */
  get puuid(): string | null {
    return this.isOpen ? (this.#subject?.puuid ?? null) : null;
  }

  get surface(): Surface | null {
    return this.isOpen ? this.#surface : null;
  }

  /** Opens the panel on `subject` beside `anchor`, or closes it when it already shows them. Null,
   *  and nothing drawn, while winer cannot be asked. */
  toggle(
    subject: Subject,
    surface: Surface,
    anchor: Element | null,
    language: Language,
  ): "opened" | "closed" | null {
    if (this.isOpen && this.#subject?.puuid === subject.puuid) {
      this.close();
      return "closed";
    }
    const request = this.actions.request(subject.puuid);
    if (request === null) return null;
    this.#mount(language);
    this.#subject = subject;
    this.#surface = surface;
    this.#anchor = anchor;
    this.#anchorBox = anchor ? box(anchor.getBoundingClientRect()) : null;
    this.#drawWho();
    this.#ask(request);
    return "opened";
  }

  /** winer's answer: drawn if it is the one the panel waits for. */
  answer(result: HistoryResult): boolean {
    if (this.#request === null || result.requestId !== this.#request) return false;
    this.#request = null;
    clearTimeout(this.#timer);
    if (!this.isOpen) {
      this.#forget();
      return false;
    }
    this.#show(
      result.page
        ? { state: "ready", page: result.page }
        : { state: "failed", error: result.error ?? { code: "internal", message: "" } },
    );
    return true;
  }

  /** The view changed: the summary follows it, the panel the language. */
  refresh(subject: Subject | null, language: Language): void {
    if (!this.isOpen) return;
    if (subject && subject.puuid === this.#subject?.puuid) this.#subject = subject;
    if (language !== this.#language) {
      this.#language = language;
      this.#label();
      this.#drawBody();
    }
    this.#drawWho();
  }

  close(): void {
    const host = this.#host;
    const focused = host?.contains(this.doc.activeElement) ?? false;
    const previous = this.#previous;
    host?.remove();
    this.#forget();
    if (focused && previous instanceof HTMLElement && previous.isConnected) {
      previous.focus({ preventScroll: true });
    }
  }

  readonly #onKey = (event: KeyboardEvent): void => {
    if (event.key !== "Escape") return;
    // Taken off the page by the other context: there is nothing of this one's to close.
    if (!this.isOpen) return this.#forget();
    event.preventDefault();
    event.stopPropagation();
    this.close();
  };

  #mount(language: Language): void {
    if (this.isOpen) {
      if (language !== this.#language) {
        this.#language = language;
        this.#label();
      }
      return;
    }
    this.#forget();
    this.#language = language;
    const who = h("div", { class: "winer-history-who" });
    const close = h("button", { type: "button", class: "winer-history-close" }, "✕");
    close.addEventListener("click", () => this.close());
    const body = h("div", { class: "winer-history-body" });
    const link = h("button", { type: "button", class: "winer-history-link" });
    link.addEventListener("click", () => {
      const puuid = this.#subject?.puuid;
      this.close();
      if (puuid) this.actions.openInWiner(puuid);
    });
    const host = h(
      "section",
      {
        class: "winer-panel winer-history",
        "data-winer-panel": HISTORY_PANEL,
        "data-winer-context": this.context,
        role: "dialog",
        tabindex: "-1",
      },
      h("div", { class: "winer-history-top" }, who, close),
      body,
      h("div", { class: "winer-history-foot" }, link),
    );
    // A click in the panel is winer's alone: the client's own handlers around it never see it.
    host.addEventListener("click", (event) => event.stopPropagation());
    this.#host = host;
    this.#who = who;
    this.#body = body;
    this.#close = close;
    this.#link = link;
    this.#label();
    this.#previous = this.doc.activeElement;
    this.doc.body.append(host);
    this.doc.addEventListener("keydown", this.#onKey, true);
    host.focus({ preventScroll: true });
  }

  /** The words of the panel's own controls, in the panel's language. */
  #label(): void {
    const close = text(this.#language, "historyClose");
    this.#close?.setAttribute("aria-label", close);
    this.#close?.setAttribute("title", `${close} (Esc)`);
    if (this.#link) this.#link.textContent = text(this.#language, "historyInWiner");
    this.#whoKey = "";
  }

  #drawWho(): void {
    const subject = this.#subject;
    if (!subject || !this.#who || !this.#host) return;
    const key = JSON.stringify([this.#language, subject]);
    if (key === this.#whoKey) return;
    this.#whoKey = key;
    this.#who.replaceChildren(...identity(subject, this.#language));
    const name = displayName(subject, this.#language);
    const shown = typeof name === "string" ? name : name.gameName;
    this.#host.setAttribute("aria-label", `${shown} · ${text(this.#language, "historyTitle")}`);
    this.#place();
  }

  #ask(request: number): void {
    this.#request = request;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      if (this.#request !== request) return;
      this.#request = null;
      if (this.isOpen) this.#show({ state: "failed", error: "timeout" });
      else this.#forget();
    }, ANSWER_TIMEOUT_MS);
    this.#show({ state: "loading" });
  }

  #retry(): void {
    const puuid = this.#subject?.puuid;
    if (!puuid || !this.isOpen) return;
    const request = this.actions.request(puuid);
    if (request === null)
      this.#show({ state: "failed", error: { code: "notConnected", message: "" } });
    else this.#ask(request);
  }

  #show(shown: Content): void {
    this.#shown = shown;
    this.#drawBody();
  }

  #drawBody(): void {
    if (!this.#body) return;
    this.#body.replaceChildren(
      content(this.#shown, this.#language, Date.now(), () => this.#retry()),
    );
    this.#place();
  }

  /** Beside the anchor, inside the window; written only when it moved. */
  #place(): void {
    const host = this.#host;
    if (!host) return;
    const view = this.doc.defaultView;
    const viewport = { width: view?.innerWidth ?? 1280, height: view?.innerHeight ?? 720 };
    const anchor = this.#anchor?.isConnected
      ? (box(this.#anchor.getBoundingClientRect()) ?? this.#anchorBox)
      : this.#anchorBox;
    const size = host.getBoundingClientRect();
    const { left, top } = placement(
      anchor,
      { width: size.width || WIDTH, height: Math.max(size.height, RESERVED_HEIGHT) },
      viewport,
    );
    if (host.style.left !== `${left}px`) host.style.left = `${left}px`;
    if (host.style.top !== `${top}px`) host.style.top = `${top}px`;
  }

  /** Forgets the panel: its parts, its request and timer, and its key. */
  #forget(): void {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    this.#request = null;
    this.doc.removeEventListener("keydown", this.#onKey, true);
    this.#host = null;
    this.#who = null;
    this.#body = null;
    this.#close = null;
    this.#link = null;
    this.#subject = null;
    this.#surface = null;
    this.#anchor = null;
    this.#anchorBox = null;
    this.#previous = null;
    this.#whoKey = "";
    this.#shown = { state: "loading" };
  }
}
