// The Pengu Loader entry. Pengu v1.1 imports this module in the client page and calls `init`, then
// `load`; a page reload creates two contexts, one of which may never get `load`. Every context
// connects to the desktop app, but only one at a time draws, chosen by a heartbeat on <html>.
import type { ChampSelectView, Language } from "@winer/shared";

import { BENCH_STYLE, interceptBenchClicks, liftBenchCooldown } from "./bench";
import { Bridge } from "./bridge";
import { h } from "./dom";
import { text } from "./i18n";
import { PluginState } from "./state";
import style from "./style.css?inline";
import { ROW_SELECTOR, clearRows, decorateRows, panelRows } from "./team";

declare const WINER_PLUGIN_VERSION: string;

const STYLE_ID = "winer-style";
const TWEAKS_ID = "winer-tweaks";
const BENCH_ID = "winer-bench";
const PROMOTIONS = `iframe#tv-official-pop, section#activity-center, div.screen-root[data-screen-name="rcp-fe-lol-activity-center"] { display: none !important; }`;
/** A context whose heartbeat is older than this has died; another one takes over. */
const OWNER_TIMEOUT_MS = 5000;

export class Controller {
  readonly context = crypto.randomUUID();
  readonly state = new PluginState();
  readonly bridge: Bridge;
  #frame = 0;
  #panel: { host: HTMLElement; body: HTMLElement; collapsed: boolean } | null = null;
  #panelKey = "";
  #report = "";
  #lifted = 0;

  constructor(private readonly doc: Document = document) {
    this.bridge = new Bridge(
      {
        onHello: (snapshot, settings) => this.state.hello(snapshot, settings),
        onEvent: (event) => this.state.event(event),
        onConnection: (connected) => this.state.connection(connected),
      },
      { version: WINER_PLUGIN_VERSION, context: this.context },
    );
  }

  start(): void {
    if (!this.doc.getElementById(STYLE_ID)) {
      this.doc.head.append(h("style", { id: STYLE_ID }, style));
    }
    this.state.subscribe(() => this.schedule());
    interceptBenchClicks(this.doc, (championId) => this.swap(championId));
    new MutationObserver(() => this.schedule()).observe(this.doc.documentElement, {
      childList: true,
      subtree: true,
    });
    setInterval(() => this.schedule(), 2000);
    this.bridge.start();
  }

  schedule(): void {
    if (this.#frame) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      this.render();
    });
  }

  /** Takes or keeps the drawing role; a live owner elsewhere keeps it. */
  owns(now = Date.now()): boolean {
    const root = this.doc.documentElement;
    const owner = root.dataset.winerOwner;
    const beat = Number(root.dataset.winerBeat ?? 0);
    if (owner && owner !== this.context && now - beat < OWNER_TIMEOUT_MS) return false;
    root.dataset.winerOwner = this.context;
    root.dataset.winerBeat = String(now);
    return true;
  }

  render(): void {
    if (!this.owns()) return;
    const { snapshot, settings } = this.state;
    this.#style(TWEAKS_ID, PROMOTIONS, Boolean(settings?.plugin.hidePromotions));
    this.#bench(Boolean(settings?.plugin.benchNoCooldown) && snapshot?.phase === "ChampSelect");

    const view =
      settings?.plugin.teamPanel && snapshot?.phase === "ChampSelect" ? snapshot.champSelect : null;
    if (!view || !settings) {
      clearRows(this.doc);
      this.#hidePanel();
      return;
    }
    const language = settings.general.language;
    const rows =
      this.doc.querySelectorAll(ROW_SELECTOR).length > 0
        ? decorateRows(this.doc, view, language)
        : 0;
    if (rows > 0) this.#hidePanel();
    else this.#showPanel(view, language);
    this.#log(
      rows > 0
        ? `champ select: ${rows} party rows decorated`
        : "champ select: party rows not found, showing the panel",
    );
  }

  /** Each change of drawing mode once, so the desktop log tells how the client page looked. */
  #log(report: string): void {
    if (report === this.#report || !this.state.connected) return;
    this.#report = report;
    this.bridge.log("info", report);
  }

  #style(id: string, css: string, on: boolean): void {
    const existing = this.doc.getElementById(id);
    if (on && !existing) this.doc.head.append(h("style", { id }, css));
    if (!on) existing?.remove();
  }

  /** A click on a bench champion in the client: winer takes it at once, when the switch is on,
   *  the champion really is on the bench and the app is there to do it. Otherwise the client's
   *  own handler gets the click as usual. */
  swap(championId: number): boolean {
    const { snapshot, settings } = this.state;
    const bench = snapshot?.phase === "ChampSelect" ? snapshot.champSelect?.bench : undefined;
    const refusal = !settings?.plugin.benchNoCooldown
      ? "the switch is off"
      : !bench?.includes(championId)
        ? `not on the bench winer knows (${bench?.join(",") ?? "no champ select"})`
        : null;
    const taken = refusal === null && this.bridge.benchSwap(championId);
    // Rare and user-driven, so every click is worth a line in the desktop log.
    if (this.state.connected)
      this.bridge.log(
        "info",
        `bench click on ${championId}: ${taken ? "swapping" : (refusal ?? "winer is not connected")}`,
      );
    return taken;
  }

  /** Keeps the bench free of cooldowns while champ select lasts; logs the first lift of each. */
  #bench(on: boolean): void {
    this.#style(BENCH_ID, BENCH_STYLE, on);
    if (!on) {
      this.#lifted = 0;
      return;
    }
    const lifted = liftBenchCooldown(this.doc);
    if (lifted > 0 && this.#lifted === 0 && this.state.connected)
      this.bridge.log("info", `bench: lifted the cooldown on ${lifted} champions`);
    this.#lifted += lifted;
  }

  #showPanel(view: ChampSelectView, language: Language): void {
    if (!this.#panel?.host.isConnected) {
      const head = h(
        "button",
        { type: "button", class: "winer-head" },
        h("span", {}, `winer · ${text(language, "title")}`),
        h("span", { class: "winer-chevron" }, "▾"),
      );
      const body = h("div");
      const host = h(
        "section",
        { class: "winer-panel", "data-winer-context": this.context, "data-collapsed": "false" },
        head,
        body,
      );
      const panel = { host, body, collapsed: false };
      head.addEventListener("click", () => {
        panel.collapsed = !panel.collapsed;
        host.dataset.collapsed = String(panel.collapsed);
      });
      this.doc.body.append(host);
      this.#panel = panel;
      this.#panelKey = "";
    }
    // Rebuilt only when what it shows changed: rebuilding is a DOM mutation, which wakes the
    // observer, which would otherwise rebuild again every frame.
    const key = JSON.stringify([language, view.side, view.myTeam]);
    if (key !== this.#panelKey) {
      this.#panelKey = key;
      this.#panel.body.replaceChildren(panelRows(view, language));
    }
  }

  #hidePanel(): void {
    this.#panel?.host.remove();
    this.#panel = null;
  }
}

let controller: Controller | null = null;

function start(): void {
  if (controller || typeof document === "undefined") return;
  controller = new Controller();
  if (document.readyState === "loading")
    document.addEventListener("DOMContentLoaded", () => controller?.start(), { once: true });
  else controller.start();
}

export function init(): void {
  start();
}

export function load(): void {
  start();
}
