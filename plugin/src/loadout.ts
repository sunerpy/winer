// The runes panel in champ select (符文面板), at the bottom right of the client: the runes and
// summoner spells the local player has now, winer's recommendation and their own setup (我的方案),
// with buttons that ask winer to set them up. winer works the runes and spells out from its own
// view; a request names the view it was drawn from, so one made after champ select changed does
// nothing and the panel says so.
import type {
  AssetInfo,
  Language,
  LoadoutAction,
  LoadoutTarget,
  LoadoutView,
  Rates,
  RecommendedBy,
  RunePage,
} from "@winer/shared";

import type { LoadoutResult } from "./bridge";
import { h } from "./dom";
import { text } from "./i18n";
import { removePanels } from "./lobby";

/** The panel's kind among the plugin's panels (`data-winer-panel`). */
export const LOADOUT_PANEL = "loadout";
/** Pengu's DataStore key the fold is kept under, so the panel stays folded across reloads. */
export const COLLAPSED_KEY = "winer-loadout-collapsed";
/** Longer than any write winer makes: only an answer that got lost runs into it. */
export const LOADOUT_TIMEOUT_MS = 20_000;

/** Pengu's synchronous store (`docs/platform-notes.md`); absent outside the client. */
export interface Store {
  get(key: string, fallback?: unknown): unknown;
  set(key: string, value: unknown): boolean;
}

type ActionKind = LoadoutAction["kind"];

/** What the panel says under its buttons: a request under way, or what came of the last one. */
export type Said =
  | { state: "busy"; action: ActionKind }
  | { state: "done"; text: string; ok: boolean };

/** Sends `action` on the view `target` names; the id its answer comes under, null without a
 *  connection. */
export type Request = (target: LoadoutTarget, action: LoadoutAction) => number | null;

// ---- What the panel shows ----

/** The same rune page: the two styles and the nine runes. */
export function samePage(a: RunePage | null | undefined, b: RunePage | null | undefined): boolean {
  return (
    !!a &&
    !!b &&
    a.primaryStyle === b.primaryStyle &&
    a.subStyle === b.subStyle &&
    a.perks.length === b.perks.length &&
    a.perks.every((perk, index) => perk === b.perks[index])
  );
}

/** The same two spells, on whichever keys. */
export function sameSpells(
  a: readonly number[] | null | undefined,
  b: readonly number[] | null | undefined,
): boolean {
  if (!a || !b || a.length !== 2 || b.length !== 2) return false;
  return (a[0] === b[0] && a[1] === b[1]) || (a[0] === b[1] && a[1] === b[0]);
}

/** What the player has now is: their own setup, the recommendation's first, or neither. `null`
 *  while the current page is not read yet. */
export function currentIs(view: LoadoutView): "mine" | "recommended" | "changed" | null {
  const { current, mine } = view;
  if (!current.runes) return null;
  const recommended =
    view.recommendation.state === "ready" ? view.recommendation.recommendation : null;
  const runes = (want: RunePage | null | undefined) => samePage(current.runes, want);
  const spells = (want: readonly number[] | null | undefined) => sameSpells(current.spells, want);
  const firstRunes = recommended?.runes[0]?.page;
  const firstSpells = recommended?.spells[0]?.spells;
  if (mine && runes(mine.runes ?? firstRunes) && spells(mine.spells ?? firstSpells)) return "mine";
  if (recommended && runes(firstRunes) && spells(firstSpells)) return "recommended";
  return "changed";
}

function percent(share: number | null): string | null {
  return share === null ? null : `${Math.round(share * 1000) / 10}%`;
}

/** `选用 47.2% · 胜率 51.3%`; empty for the client's own pages, which come with no rates. */
export function ratesLine(rates: Rates, language: Language): string {
  const parts: string[] = [];
  const pick = percent(rates.pick);
  const win = percent(rates.win);
  if (pick) parts.push(`${text(language, "loadoutPick")} ${pick}`);
  if (win) parts.push(`${text(language, "winRate")} ${win}`);
  return parts.join(" · ");
}

/** Where the recommendation's rune pages come from, with the patch for winer's numbers. */
export function sourceLine(by: RecommendedBy | null, patch: string, language: Language): string {
  if (!by) return "";
  const name =
    by === "tencent"
      ? text(language, "loadoutTencent")
      : by === "opGg"
        ? "OP.GG"
        : text(language, "loadoutFromClient");
  return by !== "client" && patch ? `${name} ${patch}` : name;
}

/** A rune, style or spell's icon from the view's own list; its id where the list has none. */
function icon(assets: readonly AssetInfo[], id: number, size: number): HTMLElement {
  const asset = assets.find((candidate) => candidate.id === id);
  if (!asset?.icon)
    return h(
      "span",
      {
        class: "winer-loadout-glyph",
        title: asset?.name ?? `#${id}`,
        style: `width:${size}px;height:${size}px`,
      },
      "",
    );
  return h("img", {
    class: "winer-loadout-icon",
    src: asset.icon,
    width: String(size),
    height: String(size),
    alt: asset.name,
    title: asset.name,
    draggable: "false",
  });
}

/** A page as the client shows it small: the keystone, then the two styles. */
function pageIcons(assets: readonly AssetInfo[], page: RunePage): HTMLElement {
  return h(
    "span",
    { class: "winer-loadout-page" },
    icon(assets, page.perks[0] ?? 0, 22),
    icon(assets, page.primaryStyle, 14),
    icon(assets, page.subStyle, 14),
  );
}

function spellIcons(assets: readonly AssetInfo[], spells: readonly number[]): HTMLElement {
  return h("span", { class: "winer-loadout-spells" }, ...spells.map((id) => icon(assets, id, 18)));
}

/** The panel's body for `view`: now, the recommendation, the user's own setup and what was said.
 *  `act` carries a button's request out; every button is off while one is under way. */
export function loadoutBody(
  view: LoadoutView,
  language: Language,
  said: Said | null,
  act: (action: LoadoutAction) => void,
): HTMLElement {
  const busy = said?.state === "busy";
  const assets = view.assets;
  const button = (label: string, action: LoadoutAction, kind = "winer-loadout-button") => {
    const element = h(
      "button",
      { type: "button", class: kind, "data-action": action.kind },
      busy && said.action === action.kind ? text(language, "loadoutWorking") : label,
    );
    if (busy) element.setAttribute("disabled", "");
    element.addEventListener("click", (event) => {
      event.stopPropagation();
      act(action);
    });
    return element;
  };

  // Now.
  const is = currentIs(view);
  const now = h(
    "div",
    { class: "winer-loadout-now" },
    h("span", { class: "winer-loadout-label" }, text(language, "loadoutNow")),
    view.current.runes
      ? pageIcons(assets, view.current.runes)
      : h("span", { class: "winer-loadout-muted" }, text(language, "loadoutNoPage")),
    spellIcons(
      assets,
      view.current.spells.filter((id) => id > 0),
    ),
    is
      ? h(
          "span",
          { class: `winer-loadout-tag winer-loadout-tag--${is}` },
          text(
            language,
            is === "mine"
              ? "loadoutIsMine"
              : is === "recommended"
                ? "loadoutIsRecommended"
                : "loadoutChanged",
          ),
        )
      : null,
  );

  // The recommendation.
  const recommendation = view.recommendation;
  let options: HTMLElement;
  if (recommendation.state === "ready") {
    const { runes, spells, runesFrom, patch } = recommendation.recommendation;
    const rows: HTMLElement[] = [];
    runes.forEach((option, index) => {
      const rates = ratesLine(option.rates, language);
      rows.push(
        h(
          "li",
          { class: "winer-loadout-option" },
          pageIcons(assets, option.page),
          rates ? h("span", { class: "winer-loadout-rates" }, rates) : null,
          button(text(language, "loadoutApply"), { kind: "applyRunes", index }),
        ),
      );
    });
    spells.forEach((option, index) => {
      const rates = ratesLine(option.rates, language);
      rows.push(
        h(
          "li",
          { class: "winer-loadout-option" },
          spellIcons(assets, option.spells),
          rates ? h("span", { class: "winer-loadout-rates" }, rates) : null,
          button(text(language, "loadoutApply"), { kind: "applySpells", index }),
        ),
      );
    });
    options = h(
      "div",
      { class: "winer-loadout-section" },
      h(
        "div",
        { class: "winer-loadout-title" },
        `${text(language, "loadoutRecommended")} · ${sourceLine(runesFrom, patch, language)}`,
      ),
      h("ol", { class: "winer-loadout-options" }, ...rows),
    );
  } else {
    const key =
      recommendation.state === "loading"
        ? "loadoutLoading"
        : recommendation.state === "failed"
          ? "loadoutFailed"
          : recommendation.state === "off"
            ? "loadoutOff"
            : "loadoutNothing";
    options = h(
      "div",
      { class: "winer-loadout-section" },
      h("div", { class: "winer-loadout-title" }, text(language, "loadoutRecommended")),
      h("p", { class: "winer-loadout-muted" }, text(language, key)),
    );
  }

  // The user's own setup.
  const mine = view.mine;
  const kept = mine
    ? text(
        language,
        mine.runes && mine.spells
          ? "loadoutMineBoth"
          : mine.runes
            ? "loadoutMineRunes"
            : "loadoutMineSpells",
      )
    : text(language, "loadoutMineNone");
  const own = h(
    "div",
    { class: "winer-loadout-section" },
    h(
      "div",
      { class: "winer-loadout-title" },
      `${text(language, "loadoutMine")} · `,
      h("span", { class: "winer-loadout-muted" }, kept),
    ),
    h(
      "div",
      { class: "winer-loadout-actions" },
      button(
        text(language, "loadoutSave"),
        { kind: "saveMine" },
        "winer-loadout-button winer-loadout-button--primary",
      ),
      mine ? button(text(language, "loadoutApplyMine"), { kind: "applyMine" }) : null,
      mine ? button(text(language, "loadoutRestore"), { kind: "restoreRecommended" }) : null,
    ),
  );

  const status = h(
    "p",
    {
      class:
        said?.state === "done" && !said.ok
          ? "winer-loadout-said winer-loadout-said--bad"
          : "winer-loadout-said",
      role: "status",
      "aria-live": "polite",
    },
    said?.state === "done" ? said.text : "",
  );
  return h("div", { class: "winer-loadout" }, now, options, own, status);
}

/** What the panel says of winer's answer to `action`. */
export function answerText(
  result: LoadoutResult,
  action: ActionKind,
  language: Language,
): { text: string; ok: boolean } {
  const { done, error } = result;
  if (error) {
    const key =
      error.code === "stale"
        ? "loadoutStale"
        : error.code === "busy"
          ? "loadoutBusy"
          : error.code === "notConnected"
            ? "loadoutOffline"
            : null;
    return {
      text: key
        ? text(language, key)
        : `${text(language, "loadoutFailedAction")}：${error.message}`,
      ok: false,
    };
  }
  if (done?.runes === "noPage") return { text: text(language, "loadoutNoRoom"), ok: false };
  const key =
    action === "saveMine"
      ? "loadoutSaved"
      : action === "restoreRecommended"
        ? "loadoutRestored"
        : "loadoutApplied";
  return { text: text(language, key), ok: true };
}

function readCollapsed(store: Store | undefined): boolean {
  try {
    return store?.get(COLLAPSED_KEY, false) === true;
  } catch {
    return false;
  }
}

/** The panel on the page: drawn by the context that draws, rebuilt only when what it shows
 *  changed, folded as the user left it. */
export class LoadoutPanel {
  #host: HTMLElement | null = null;
  #body: HTMLElement | null = null;
  #key = "";
  /** The view drawn last: a button's request names it. */
  #view: LoadoutView | null = null;
  #language: Language = "zh-CN";
  #pending: { id: number; action: ActionKind; timer: ReturnType<typeof setTimeout> } | null = null;
  #said: Said | null = null;
  #collapsed: boolean;

  constructor(
    private readonly doc: Document,
    private readonly context: string,
    private readonly request: Request,
    /** Draws the panel again when an answer, or the lack of one, changed what it says. */
    private readonly changed: () => void,
    private readonly store: Store | undefined = (globalThis as { DataStore?: Store }).DataStore,
  ) {
    this.#collapsed = readCollapsed(store);
  }

  get isShown(): boolean {
    return this.#host?.isConnected === true;
  }

  show(view: LoadoutView, language: Language): void {
    removePanels(this.doc, LOADOUT_PANEL, this.context);
    if (!this.#host?.isConnected) {
      const head = h(
        "button",
        { type: "button", class: "winer-head", "aria-expanded": String(!this.#collapsed) },
        h("span", {}, `winer · ${text(language, "loadoutTitle")}`),
        h("span", { class: "winer-chevron" }, "▾"),
      );
      const body = h("div", { class: "winer-loadout-body" });
      const host = h(
        "section",
        {
          class: "winer-panel",
          "data-winer-context": this.context,
          "data-winer-panel": LOADOUT_PANEL,
          "data-collapsed": String(this.#collapsed),
        },
        head,
        body,
      );
      head.addEventListener("click", (event) => {
        event.stopPropagation();
        this.#collapsed = !this.#collapsed;
        host.dataset.collapsed = String(this.#collapsed);
        head.setAttribute("aria-expanded", String(!this.#collapsed));
        try {
          this.store?.set(COLLAPSED_KEY, this.#collapsed);
        } catch {
          // Kept for this page only.
        }
      });
      this.doc.body.append(host);
      this.#host = host;
      this.#body = body;
      this.#key = "";
    }
    // A request made on another champ select or champion is not this view's to report.
    if (
      this.#view &&
      (this.#view.gameId !== view.gameId || this.#view.championId !== view.championId)
    )
      this.#forget();
    this.#view = view;
    this.#language = language;
    // Rebuilt only when what it shows changed: a rebuild is a DOM mutation, which wakes the
    // observer, which would otherwise rebuild again every frame.
    const key = JSON.stringify([language, view, this.#said]);
    if (key !== this.#key && this.#body) {
      this.#key = key;
      this.#body.replaceChildren(
        loadoutBody(view, language, this.#said, (action) => this.#act(action)),
      );
    }
  }

  hide(): void {
    removePanels(this.doc, LOADOUT_PANEL);
    this.#host = null;
    this.#body = null;
    this.#view = null;
    this.#forget();
  }

  /** winer's answer; whether it was the one awaited. */
  answer(result: LoadoutResult): boolean {
    const pending = this.#pending;
    if (!pending || pending.id !== result.requestId) return false;
    clearTimeout(pending.timer);
    this.#pending = null;
    this.#said = { state: "done", ...answerText(result, pending.action, this.#language) };
    this.changed();
    return true;
  }

  #act(action: LoadoutAction): void {
    const view = this.#view;
    if (!view || this.#pending) return;
    const target = { gameId: view.gameId, championId: view.championId, revision: view.revision };
    const id = this.request(target, action);
    if (id === null) {
      this.#said = { state: "done", text: text(this.#language, "loadoutOffline"), ok: false };
    } else {
      const timer = setTimeout(() => {
        if (this.#pending?.id !== id) return;
        this.#pending = null;
        this.#said = { state: "done", text: text(this.#language, "loadoutTimeout"), ok: false };
        this.changed();
      }, LOADOUT_TIMEOUT_MS);
      this.#pending = { id, action: action.kind, timer };
      this.#said = { state: "busy", action: action.kind };
    }
    this.changed();
  }

  #forget(): void {
    if (this.#pending) clearTimeout(this.#pending.timer);
    this.#pending = null;
    this.#said = null;
  }
}
