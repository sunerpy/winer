// The Pengu Loader entry. Pengu v1.1 imports this module in the client page and calls `init`, then
// `load`; a page reload creates two contexts, one of which may never get `load`. Every context
// connects to the desktop app, but only one at a time draws, chosen by a heartbeat on <html>.
import type { ChampSelectView, Language, LobbyView, Settings, Snapshot } from "@winer/shared";

import { BENCH_STYLE, interceptBenchClicks, liftBenchCooldown } from "./bench";
import { Bridge, type HistoryResult } from "./bridge";
import { h } from "./dom";
import { decorateFriends } from "./friends";
import {
  HISTORY_PANEL,
  HistoryPanel,
  type Subject,
  type Surface,
  anchorOf,
  interceptPlayerClicks,
} from "./history";
import { type HomeState, PROMOTIONS, decorateHome, forgetHub } from "./home";
import { text } from "./i18n";
import {
  type FloatingPanel,
  clearLobby,
  decorateLobby,
  removePanels,
  floatingPanel,
  interceptAvatarClicks,
  lobbyRows,
} from "./lobby";
import { quietPengu } from "./pengu";
import { PluginState } from "./state";
import style from "./style.css?inline";
import { ROW_SELECTOR, clearRows, decorateRows, panelRows } from "./team";

declare const WINER_PLUGIN_VERSION: string;

const STYLE_ID = "winer-style";
const TWEAKS_ID = "winer-tweaks";
const BENCH_ID = "winer-bench";
/** What the desktop log says of the home page while the option is on, each change once. */
const HOME_REPORTS: Record<Exclude<HomeState, "off">, string> = {
  noted: "home: the hub is hidden, winer's note in its place",
  missing: "home: no hub found in the activity centre",
  shown: "home: the hub is shown again for this session",
};
/** A context whose heartbeat is older than this has died; another one takes over. */
const OWNER_TIMEOUT_MS = 5000;

export class Controller {
  readonly context = crypto.randomUUID();
  readonly state = new PluginState();
  readonly bridge: Bridge;
  #frame = 0;
  #panel: { host: HTMLElement; body: HTMLElement; collapsed: boolean } | null = null;
  #panelKey = "";
  /** The last report of each part of the page, so each change is logged once. */
  #reports = new Map<string, string>();
  #lifted = 0;
  // Social.
  /** A friend's game time is on screen: it is redrawn every second. */
  #ticking = false;
  #lobbyPanel: FloatingPanel | null = null;
  #lobbyKey = "";
  // The history panel.
  readonly history: HistoryPanel;

  constructor(private readonly doc: Document = document) {
    this.bridge = new Bridge(
      {
        onHello: (snapshot, settings) => this.state.hello(snapshot, settings),
        onEvent: (event) => this.state.event(event),
        onConnection: (connected) => this.state.connection(connected),
        onHistory: (result) => this.#answered(result),
      },
      { version: WINER_PLUGIN_VERSION, context: this.context },
    );
    this.history = new HistoryPanel(this.doc, this.context, {
      request: (puuid) => this.bridge.history(puuid),
      openInWiner: (puuid) => void this.openHistory(puuid),
    });
  }

  start(): void {
    if (!this.doc.getElementById(STYLE_ID)) {
      this.doc.head.append(h("style", { id: STYLE_ID }, style));
    }
    this.state.subscribe(() => this.schedule());
    interceptBenchClicks(this.doc, (championId) => this.swap(championId));
    interceptAvatarClicks(this.doc, (puuid, card) => this.pick(puuid, card));
    interceptPlayerClicks(this.doc, (puuid, line) => this.pick(puuid, line));
    new MutationObserver(() => this.schedule()).observe(this.doc.documentElement, {
      childList: true,
      subtree: true,
    });
    setInterval(() => this.schedule(), 2000);
    // A game's time in the friends list ticks by the second; nothing else needs that pace.
    setInterval(() => {
      if (this.#ticking) this.schedule();
    }, 1000);
    this.bridge.start();
  }

  schedule(): void {
    if (this.#frame) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      this.render();
    });
  }

  /** Takes or keeps the drawing role; a live owner elsewhere keeps it, unless winer is connected to
   *  this context and not to the owner. A context that never reached the bridge still beats, but
   *  draws nothing: kept by it, the role left the page without winer's drawing for minutes after
   *  the client's interface restarted (measured on a fresh install, 2026-10-06). */
  owns(now = Date.now()): boolean {
    const root = this.doc.documentElement;
    const owner = root.dataset.winerOwner;
    const beat = Number(root.dataset.winerBeat ?? 0);
    const connected = this.state.connected;
    const ownerConnected = root.dataset.winerConnected === "true";
    const live = owner && owner !== this.context && now - beat < OWNER_TIMEOUT_MS;
    if (live && (ownerConnected || !connected)) return false;
    root.dataset.winerOwner = this.context;
    root.dataset.winerBeat = String(now);
    root.dataset.winerConnected = String(connected);
    return true;
  }

  render(): void {
    if (!this.owns()) return;
    const { snapshot, settings } = this.state;
    this.#home(settings);
    this.#bench(Boolean(settings?.plugin.benchNoCooldown) && snapshot?.phase === "ChampSelect");
    this.#social(snapshot, settings);
    this.#historyPanel();

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

  /** Each change of drawing mode once per part of the page, so the desktop log tells how the
   *  client page looked. */
  #log(report: string, part = "champSelect"): void {
    if (report === this.#reports.get(part) || !this.state.connected) return;
    this.#reports.set(part, report);
    this.bridge.log("info", report);
  }

  // ---- Social: friends' games in the friends list, the party in the lobby ----

  /** Asks winer to show `puuid`'s history in its window: for a click on a player while the history
   *  panel is off, or the panel's own link. Only the drawing context acts, so one click is one
   *  request. */
  openHistory(puuid: string): boolean {
    if (this.doc.documentElement.dataset.winerOwner !== this.context) return false;
    const sent = this.bridge.openHistory(puuid);
    if (this.state.connected)
      this.bridge.log("info", `player click: ${sent ? "history asked for in winer" : "not sent"}`);
    return sent;
  }

  #social(snapshot: Snapshot | null, settings: Settings | null): void {
    const language = settings?.general.language ?? "zh-CN";
    const friends = settings?.plugin.friendStatus ? (snapshot?.friends ?? null) : null;
    const drawn = decorateFriends(this.doc, friends, Date.now(), language);
    this.#ticking = drawn.lines > 0;
    if (friends?.friends.some((friend) => friend.status.state === "inGame"))
      this.#log(
        drawn.entries > 0
          ? "friends list: entries found"
          : "friends list: no entries found for the friends in game",
        "friends",
      );

    // The core sends the lobby only while the client shows it.
    const lobby = settings?.plugin.lobbyPanel ? (snapshot?.lobby ?? null) : null;
    if (!lobby) {
      clearLobby(this.doc);
      this.#hideLobbyPanel();
      return;
    }
    const open = (puuid: string, anchor: Element) => void this.pick(puuid, anchor);
    const cards = decorateLobby(this.doc, lobby, language, open);
    // The match-found dialog has the screen to itself: the panel would only sit in its way.
    if (cards > 0 || snapshot?.phase === "ReadyCheck") this.#hideLobbyPanel();
    else this.#showLobbyPanel(lobby, language, open);
    this.#log(
      cards > 0
        ? `lobby: ${cards} member cards decorated`
        : "lobby: member cards not found, showing the panel",
      "lobby",
    );
  }

  #showLobbyPanel(
    view: LobbyView,
    language: Language,
    open: (puuid: string, anchor: Element) => void,
  ): void {
    removePanels(this.doc, "lobby", this.context);
    if (!this.#lobbyPanel?.host.isConnected) {
      this.#lobbyPanel = floatingPanel(this.doc, text(language, "lobby"), this.context);
      this.#lobbyKey = "";
    }
    // Rebuilt only when what it shows changed, as champ select's panel is.
    const key = JSON.stringify([language, view.members]);
    if (key !== this.#lobbyKey) {
      this.#lobbyKey = key;
      this.#lobbyPanel.body.replaceChildren(lobbyRows(view, language, open));
    }
  }

  #hideLobbyPanel(): void {
    removePanels(this.doc, "lobby");
    this.#lobbyPanel = null;
  }

  /** 隐藏首页推广: the pop-ups' and the hub's rules, and winer's note in the hub's place. */
  #home(settings: Settings | null): void {
    const hide = settings?.plugin.hidePromotions;
    this.#style(TWEAKS_ID, PROMOTIONS, hide === true);
    // Only a known "off" forgets a hub asked back: settings are unknown until the bridge's hello,
    // and the page's session outlives that wait.
    if (hide === false) forgetHub(this.doc);
    const language = settings?.general.language ?? "zh-CN";
    const home = decorateHome(this.doc, hide === true, language, this.context);
    if (home === "off") this.#reports.delete("home");
    else this.#log(HOME_REPORTS[home], "home");
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
    removePanels(this.doc, "team", this.context);
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
        {
          class: "winer-panel",
          "data-winer-context": this.context,
          "data-winer-panel": "team",
          "data-collapsed": "false",
        },
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
      const open = (puuid: string, anchor: Element) => void this.pick(puuid, anchor);
      this.#panel.body.replaceChildren(panelRows(view, language, open));
    }
  }

  #hidePanel(): void {
    removePanels(this.doc, "team");
    this.#panel = null;
  }

  // ---- The history panel: a player's latest games over the client page ----

  /** A click on a player in the lobby or in champ select. With 在客户端里查看战绩 on, their latest
   *  games in the panel beside `element` (a second click on them closes it); off, their history in
   *  winer's window. Only the drawing context acts, so one click is one request. Says whether it
   *  took the click. */
  pick(puuid: string, element?: Element | null): boolean {
    if (this.doc.documentElement.dataset.winerOwner !== this.context) return false;
    const settings = this.state.settings;
    if (!settings?.plugin.historyInClient) return this.openHistory(puuid);
    const surface = this.#surface();
    const subject = surface ? this.#subject(puuid) : null;
    const done =
      surface && subject
        ? this.history.toggle(subject, surface, anchorOf(element), settings.general.language)
        : null;
    if (this.state.connected)
      this.bridge.log(
        "info",
        `player click: ${done ? `history panel ${done} (${surface})` : "no history panel"}`,
      );
    return done !== null;
  }

  /** winer's answer for the panel; what it held goes to the desktop log, once per answer drawn. */
  #answered(result: HistoryResult): void {
    if (this.history.answer(result) && this.state.connected)
      this.bridge.log(
        "info",
        result.page
          ? `history panel: ${result.page.games.length} games`
          : `history panel: no games (${result.error?.code ?? "?"})`,
      );
  }

  /** The screen a click on a player comes from now: champ select or the lobby, where winer draws
   *  its lines; none while the match-found dialog has the screen. */
  #surface(): Surface | null {
    const { snapshot, settings } = this.state;
    if (!snapshot || !settings) return null;
    if (snapshot.phase === "ChampSelect")
      return settings.plugin.teamPanel && snapshot.champSelect ? "champSelect" : null;
    if (snapshot.phase === "ReadyCheck") return null;
    return settings.plugin.lobbyPanel && snapshot.lobby ? "lobby" : null;
  }

  /** The player as the view shows them, for the panel's summary. */
  #subject(puuid: string): Subject | null {
    const snapshot = this.state.snapshot;
    const member = snapshot?.lobby?.members.find((candidate) => candidate.puuid === puuid);
    if (member)
      return {
        puuid,
        name: member.name,
        iconId: member.iconId,
        stats: member.stats,
        rating: null,
        score: member.score,
      };
    const select = snapshot?.champSelect;
    const seat = select
      ? [...select.myTeam, ...select.theirTeam].find((candidate) => candidate.puuid === puuid)
      : undefined;
    if (!seat) return null;
    const summary = seat.stats.state === "ready" ? seat.stats : null;
    return {
      puuid,
      name: seat.name ?? summary?.name ?? null,
      iconId: summary?.iconId ?? 0,
      stats: seat.stats,
      rating: seat.rating,
      score: seat.rating?.score ?? null,
    };
  }

  /** Keeps the panel to the screen it was opened on: it closes when the client leaves that screen,
   *  when the match-found dialog comes up and when the option goes off; meanwhile its summary
   *  follows the view. Clears a panel the page's other context left. */
  #historyPanel(): void {
    removePanels(this.doc, HISTORY_PANEL, this.context);
    if (!this.history.isOpen) return;
    const settings = this.state.settings;
    const puuid = this.history.puuid;
    if (!settings?.plugin.historyInClient || !puuid || this.#surface() !== this.history.surface) {
      this.history.close();
      return;
    }
    this.history.refresh(this.#subject(puuid), settings.general.language);
  }
}

let controller: Controller | null = null;

function start(): void {
  if (controller || typeof document === "undefined") return;
  // At import, before window load, which is when Pengu draws its own notices.
  quietPengu();
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
