// The history panel in the client: a player's latest games, asked of winer over the bridge and drawn
// over the client page beside the line that was clicked.
import type {
  ChampSelectView,
  LobbyMember,
  LobbyView,
  PanelGame,
  PanelHistory,
  Patch,
  PlayerSummary,
  Seat,
  Settings,
  Snapshot,
} from "@winer/shared";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Bridge, type BridgeHandlers, type HistoryResult } from "./bridge";
import {
  ANSWER_TIMEOUT_MS,
  HistoryPanel,
  type Subject,
  anchorOf,
  gameRow,
  interceptPlayerClicks,
  placement,
} from "./history";
import { Controller } from "./index";
import { decorateLobby } from "./lobby";
import { decorateRows, panelRows } from "./team";

const HOUR = 3_600_000;

function summary(puuid: string, wins: number, games: number): PlayerSummary {
  return {
    puuid,
    name: { gameName: puuid, tagLine: "1" },
    level: 30,
    iconId: 7,
    private: false,
    ranked: { solo: { tier: "GOLD", division: "II", lp: 20, wins: 5, losses: 5 }, flex: null },
    recent: {
      games,
      wins,
      kills: 6,
      deaths: 3,
      assists: 9,
      streak: 3,
      matches: [],
      champions: [],
      score: null,
      source: null,
      family: null,
      away: 0,
    },
  };
}

function member(puuid: string, name: string, stats: LobbyMember["stats"]): LobbyMember {
  return {
    puuid,
    name: { gameName: name, tagLine: "7" },
    iconId: 29,
    isSelf: puuid === "me",
    leader: puuid === "me",
    positions: [],
    stats,
    score: stats.state === "ready" ? 7.4 : null,
  };
}

function lobbyView(): LobbyView {
  return {
    queueId: 450,
    custom: false,
    members: [
      member("me", "Me", { state: "ready", ...summary("me", 12, 20) }),
      member("mate", "Mate", { state: "ready", ...summary("mate", 12, 20) }),
    ],
  };
}

function subject(
  puuid = "mate",
  stats: Subject["stats"] = { state: "ready", ...summary(puuid, 12, 20) },
): Subject {
  return {
    puuid,
    name: { gameName: "Mate", tagLine: "7" },
    iconId: 29,
    stats,
    rating: null,
    score: stats.state === "ready" ? 7.4 : null,
  };
}

function game(id: number, extra: Partial<PanelGame> = {}): PanelGame {
  return {
    gameId: id,
    queueId: 450,
    queue: "极地大乱斗",
    championId: 22,
    win: true,
    remake: false,
    kills: 8,
    deaths: 2,
    assists: 11,
    startedAt: Date.now() - 3 * HOUR - 60_000,
    duration: 1100,
    award: null,
    placement: null,
    ...extra,
  };
}

function page(puuid: string, games: PanelGame[]): PanelHistory {
  return { puuid, games };
}

const answer = (requestId: number, found: PanelHistory | HistoryResult["error"]): HistoryResult =>
  found && "games" in found
    ? { type: "historyResult", requestId, page: found, error: null }
    : { type: "historyResult", requestId, page: null, error: found };

/** A panel whose requests and links the test sees. */
function testPanel(connected = true) {
  const requests: string[] = [];
  const opened: string[] = [];
  const panel = new HistoryPanel(document, "context-a", {
    request: (puuid) => (connected ? requests.push(puuid) : null),
    openInWiner: (puuid) => void opened.push(puuid),
  });
  return { panel, requests, opened };
}

const host = () => document.querySelector<HTMLElement>("[data-winer-panel='history']");
const rows = () => [...document.querySelectorAll<HTMLElement>(".winer-game")];
const escape = (target: EventTarget = document.body) => {
  const event = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  return event;
};

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute("data-winer-owner");
  document.documentElement.removeAttribute("data-winer-beat");
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("where the panel goes", () => {
  const viewport = { width: 1280, height: 720 };
  const size = { width: 320, height: 400 };

  it("sits beside what was clicked, on the side with room, level with its top", () => {
    const row = { left: 20, top: 200, width: 230, height: 60 };
    expect(placement(row, size, viewport)).toEqual({ left: 258, top: 200 });
    const card = { left: 1000, top: 100, width: 200, height: 40 };
    expect(placement(card, size, viewport), "no room on the right: the left").toEqual({
      left: 672,
      top: 100,
    });
  });

  it("stays inside the window", () => {
    const low = { left: 20, top: 600, width: 230, height: 60 };
    expect(placement(low, size, viewport).top, "lifted to fit").toBe(312);
    const tall = { width: 320, height: 900 };
    expect(placement(low, tall, viewport).top, "taller than the window: from the top").toBe(8);
    const wide = { left: 100, top: 100, width: 1100, height: 40 };
    expect(placement(wide, size, viewport), "no room either side: under it").toEqual({
      left: 490,
      top: 148,
    });
    expect(placement(null, size, viewport), "nothing to sit beside: the top right").toEqual({
      left: 948,
      top: 76,
    });
  });
});

describe("the panel", () => {
  it("shows what the line shows, asks winer for the games and lists them", () => {
    const { panel, requests } = testPanel();
    expect(panel.toggle(subject(), "lobby", null, "zh-CN")).toBe("opened");
    expect(requests).toEqual(["mate"]);
    const shown = host();
    expect(shown?.dataset.winerContext).toBe("context-a");
    expect(shown?.getAttribute("role")).toBe("dialog");
    expect(shown?.getAttribute("aria-label")).toBe("Mate · 最近战绩");
    expect(document.activeElement, "it takes the focus, for Esc and screen readers").toBe(shown);
    expect(shown?.querySelector(".winer-name")?.textContent).toBe("Mate#7");
    expect(shown?.querySelector("img")?.getAttribute("src")).toBe(
      "/lol-game-data/assets/v1/profile-icons/29.jpg",
    );
    const said = shown?.querySelector(".winer-history-summary")?.textContent;
    for (const part of ["黄金 II", "胜率 60%", "KDA 5.0", "战力 7.4", "3 连胜"])
      expect(said).toContain(part);
    expect(shown?.querySelector(".winer-history-body")?.textContent).toBe("正在读取最近的对局");

    panel.answer(
      answer(1, page("mate", [game(1), game(2, { win: false, queue: "", award: "mvp" })])),
    );
    const [won, lost] = rows();
    expect(rows()).toHaveLength(2);
    expect(won?.className).toBe("winer-game winer-game--win");
    expect(won?.querySelector("img")?.getAttribute("src")).toBe(
      "/lol-game-data/assets/v1/champion-icons/22.png",
    );
    expect(won?.querySelector(".winer-game-head")?.textContent).toBe("胜极地大乱斗");
    expect(won?.querySelector(".winer-game-kda")?.textContent).toBe("8 / 2 / 11");
    expect(won?.querySelector(".winer-game-when")?.textContent).toBe("3 小时前 · 18分20秒");
    expect(lost?.className).toBe("winer-game winer-game--loss");
    expect(lost?.querySelector(".winer-game-head")?.textContent, "a queue without a name").toBe(
      "负其他模式MVP",
    );
  });

  it("draws only the answer it waits for", () => {
    const { panel } = testPanel();
    panel.toggle(subject(), "lobby", null, "zh-CN");
    panel.answer(answer(7, page("mate", [game(1)])));
    expect(rows(), "an answer to another request").toHaveLength(0);
    panel.answer(answer(1, page("mate", [game(1)])));
    expect(rows()).toHaveLength(1);
    panel.answer(answer(1, page("mate", [game(1), game(2)])));
    expect(rows(), "answered once").toHaveLength(1);
  });

  it("says plainly why there are no games and asks again", () => {
    const { panel, requests } = testPanel();
    panel.toggle(subject(), "lobby", null, "zh-CN");
    panel.answer(
      answer(1, { code: "notConnected", message: "the League client is not connected" }),
    );
    const body = () => host()?.querySelector(".winer-history-body");
    expect(body()?.textContent).toBe("winer 没有连上客户端，读不到战绩重试");
    expect(body()?.querySelector("[role='alert']")).not.toBeNull();

    body()?.querySelector<HTMLElement>(".winer-history-retry")?.click();
    expect(requests).toEqual(["mate", "mate"]);
    expect(body()?.textContent).toBe("正在读取最近的对局");
    panel.answer(answer(2, { code: "client", message: "HTTP 500 from the client" }));
    expect(body()?.querySelector(".winer-history-message")?.textContent).toBe("没有读到最近的对局");
    expect(body()?.querySelector(".winer-history-detail")?.textContent).toBe(
      "HTTP 500 from the client",
    );

    body()?.querySelector<HTMLElement>(".winer-history-retry")?.click();
    panel.answer(answer(3, page("mate", [])));
    expect(body()?.textContent).toBe("近期没有对局");
  });

  it("gives up on an answer that never comes", () => {
    vi.useFakeTimers();
    const { panel } = testPanel();
    panel.toggle(subject(), "lobby", null, "zh-CN");
    vi.advanceTimersByTime(ANSWER_TIMEOUT_MS - 1);
    expect(host()?.textContent).toContain("正在读取最近的对局");
    vi.advanceTimersByTime(1);
    expect(host()?.querySelector(".winer-history-message")?.textContent).toBe("winer 没有及时回应");
    panel.answer(answer(1, page("mate", [game(1)])));
    expect(rows(), "too late").toHaveLength(0);
  });

  it("is one panel, which another player takes over and a second click closes", () => {
    const { panel, requests } = testPanel();
    panel.toggle(subject("mate"), "lobby", null, "zh-CN");
    expect(panel.toggle(subject("other"), "lobby", null, "zh-CN")).toBe("opened");
    expect(document.querySelectorAll("[data-winer-panel='history']")).toHaveLength(1);
    expect(requests).toEqual(["mate", "other"]);
    panel.answer(answer(1, page("mate", [game(1)])));
    expect(rows(), "the first player's games no longer belong here").toHaveLength(0);
    expect(panel.puuid).toBe("other");
    expect(panel.toggle(subject("other"), "lobby", null, "zh-CN")).toBe("closed");
    expect(host()).toBeNull();
    expect(panel.puuid).toBeNull();
  });

  it("closes on Esc and on its ✕, and leaves Esc to the client while closed", () => {
    const { panel } = testPanel();
    let client = 0;
    document.body.addEventListener("keydown", () => (client += 1));
    panel.toggle(subject(), "lobby", null, "zh-CN");
    const pressed = escape();
    expect([host(), pressed.defaultPrevented, client]).toEqual([null, true, 0]);
    expect(escape().defaultPrevented, "nothing open: the client's").toBe(false);
    expect(client).toBe(1);

    panel.toggle(subject(), "lobby", null, "zh-CN");
    const close = host()?.querySelector<HTMLElement>(".winer-history-close");
    expect(close?.getAttribute("aria-label")).toBe("关闭");
    close?.click();
    expect(host()).toBeNull();
  });

  it("gives the focus back where it was", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    const { panel } = testPanel();
    panel.toggle(subject(), "lobby", null, "zh-CN");
    expect(document.activeElement).toBe(host());
    escape();
    expect(document.activeElement).toBe(input);
  });

  it("opens the whole history in winer from its link", () => {
    const { panel, opened } = testPanel();
    panel.toggle(subject(), "lobby", null, "zh-CN");
    const link = host()?.querySelector<HTMLElement>(".winer-history-link");
    expect(link?.textContent).toBe("在 winer 中查看完整战绩");
    link?.click();
    expect(opened).toEqual(["mate"]);
    expect(host()).toBeNull();
  });

  it("opens beside what was clicked, already where its games will fit", () => {
    const row = document.createElement("div");
    document.body.append(row);
    // jsdom has no layout: the row's box as the client would lay it out, low in a 1024 × 768 page.
    row.getBoundingClientRect = () => DOMRect.fromRect({ x: 20, y: 600, width: 230, height: 60 });
    const { panel } = testPanel();
    panel.toggle(subject(), "champSelect", row, "zh-CN");
    expect([host()?.style.left, host()?.style.top], "lifted for the list still to come").toEqual([
      "258px",
      "280px",
    ]);
    row.remove();
    panel.answer(answer(1, page("mate", [game(1)])));
    expect(host()?.style.top, "the row gone, the panel stays where it opened").toBe("280px");
  });

  it("draws nothing while winer cannot be asked", () => {
    const { panel } = testPanel(false);
    expect(panel.toggle(subject(), "lobby", null, "zh-CN")).toBeNull();
    expect(host()).toBeNull();
  });

  it("follows the view and the language", () => {
    const { panel } = testPanel();
    panel.toggle(subject("mate", { state: "loading" }), "lobby", null, "zh-CN");
    expect(host()?.querySelector(".winer-history-summary")?.textContent).toBe("正在读取战绩");
    const name = host()?.querySelector(".winer-name");
    panel.refresh(subject("mate", { state: "loading" }), "zh-CN");
    expect(host()?.querySelector(".winer-name"), "unchanged: not rewritten").toBe(name);
    panel.refresh(subject(), "zh-CN");
    expect(host()?.querySelector(".winer-history-summary")?.textContent).toContain("胜率 60%");
    panel.refresh(subject("someone else"), "en");
    expect(host()?.querySelector(".winer-history-summary")?.textContent).toContain("Win rate 60%");
    expect(host()?.querySelector(".winer-history-body")?.textContent).toBe("Reading recent games");
    expect(host()?.querySelector(".winer-history-link")?.textContent).toBe("Full history in winer");
    expect(host()?.querySelector(".winer-history-close")?.getAttribute("aria-label")).toBe("Close");
    expect(panel.puuid, "another player's view does not take the panel over").toBe("mate");
  });

  it("names a game's place in Arena, its award and a remake", () => {
    const now = Date.now();
    const arena = gameRow(
      game(1, { queue: "斗魂竞技场", placement: 2, award: "svp" }),
      "zh-CN",
      now,
    );
    expect(arena.querySelector(".winer-game-head")?.textContent).toBe("胜斗魂竞技场第 2 名SVP");
    const english = gameRow(game(1, { placement: 2 }), "en", now);
    expect(english.querySelector(".winer-game-place")?.textContent).toBe("#2");
    const remade = gameRow(game(1, { remake: true, win: false }), "zh-CN", now);
    expect([remade.className, remade.querySelector(".winer-game-result")?.textContent]).toEqual([
      "winer-game winer-game--remake",
      "重开",
    ]);
  });
});

/** Champ select's party rows as the client draws them, a name block in each. */
function partyRows(count: number): HTMLElement {
  const party = document.createElement("div");
  party.className = "party visible";
  for (let index = 0; index < count; index += 1) {
    party.insertAdjacentHTML(
      "beforeend",
      `<div class="summoner-wrapper visible left"><div class="player-name-wrapper">P${index}</div><button class="trade">⇄</button></div>`,
    );
  }
  return party;
}

function seat(puuid: string | null, stats: Seat["stats"], rating: Seat["rating"] = null): Seat {
  return {
    puuid,
    name: puuid ? { gameName: puuid, tagLine: "1" } : null,
    championId: 103,
    intent: false,
    position: null,
    spells: [4, 14],
    isSelf: false,
    premade: null,
    premadeInferred: false,
    stats,
    rating,
  };
}

function champSelect(seats: Seat[]): ChampSelectView {
  return {
    gameId: 1,
    queueId: 450,
    timer: { phase: "BAN_PICK", endsAt: 0, totalMs: 0 },
    myTeam: seats,
    theirTeam: [],
    myBans: [],
    theirBans: [],
    benchEnabled: false,
    bench: [],
    rerollsRemaining: 0,
    callout: [],
    side: null,
  };
}

const RATING = {
  score: 7.4,
  tier: 0,
  tiers: 5,
  label: "峡谷通天代",
  grade: null,
  title: "版本答案",
  quip: null,
};

describe("clicks on winer's lines", () => {
  it("names the player on champ select's lines and takes a click on one, not on the row", () => {
    document.body.append(partyRows(2));
    const ready = { state: "ready" as const, ...summary("p", 12, 20) };
    decorateRows(
      document,
      champSelect([seat("p", ready), seat(null, { state: "hidden" })]),
      "zh-CN",
    );
    const lines = [...document.querySelectorAll<HTMLElement>(".winer-inline")];
    expect(lines.map((line) => line.getAttribute("data-winer-puuid"))).toEqual(["p", null]);
    expect(lines[0]?.title).toBe("查看战绩");

    const picked: [string, Element][] = [];
    let take = true;
    const stop = interceptPlayerClicks(
      document,
      (puuid, line) => picked.push([puuid, line]) > 0 && take,
    );
    let client = 0;
    document.querySelector(".summoner-wrapper")?.addEventListener("click", () => (client += 1));
    lines[0]?.querySelector<HTMLElement>("span span")?.click();
    expect([picked.map(([puuid]) => puuid), client]).toEqual([["p"], 0]);
    expect(picked[0]?.[1]).toBe(lines[0]);
    expect(anchorOf(lines[0]), "the panel goes beside the whole row").toBe(
      document.querySelector(".summoner-wrapper"),
    );
    document.querySelector<HTMLElement>(".trade")?.click();
    expect([picked.length, client], "the client's own button stays the client's").toEqual([1, 1]);
    lines[1]?.click();
    expect(picked.length, "a hidden player's line names nobody").toBe(1);
    take = false;
    lines[0]?.click();
    expect([picked.length, client], "not taken: the client gets it").toEqual([2, 2]);
    stop();
  });

  it("takes a click on the lobby's line for whichever context draws, the card as its anchor", () => {
    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="lobby-party-member"><div class="lobby-banner"></div><div class="player-name">Mate</div></div>`,
    );
    const drawnBy: string[] = [];
    decorateLobby(document, lobbyView(), "zh-CN", () => void drawnBy.push("the line's own"));
    const picked: string[] = [];
    const stop = interceptPlayerClicks(document, (puuid, line) => {
      picked.push(puuid);
      expect(anchorOf(line)?.className).toBe("lobby-party-member");
      return true;
    });
    document.querySelector<HTMLElement>(".winer-lobby")?.click();
    expect([picked, drawnBy]).toEqual([["mate"], []]);
    stop();
  });

  it("finds a line, and the panel's buttons, under Pengu's root", () => {
    document.body.append(partyRows(1));
    decorateRows(document, champSelect([seat("p", { state: "loading" })]), "zh-CN");
    const overlay = document.createElement("div");
    overlay.id = "pengu-root";
    document.body.append(overlay);
    const line = document.querySelector(".winer-inline") as Element;
    let under: Element = line;
    // jsdom has no layout: say what a hit test at the click's point would find.
    Object.defineProperty(document, "elementsFromPoint", {
      configurable: true,
      value: () => [overlay, under, document.body],
    });
    const picked: string[] = [];
    const stop = interceptPlayerClicks(document, (puuid) => picked.push(puuid) > 0);
    overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 5, clientY: 5 }));
    expect(picked).toEqual(["p"]);

    const { panel } = testPanel();
    panel.toggle(subject(), "champSelect", null, "zh-CN");
    under = host()?.querySelector(".winer-history-close") as Element;
    overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 5, clientY: 5 }));
    expect(host(), "the ✕ under the root closed the panel").toBeNull();
    stop();
    Reflect.deleteProperty(document, "elementsFromPoint");
  });

  it("makes the fallback panel's rows open their player", () => {
    const opened: string[] = [];
    const list = panelRows(
      champSelect([seat("p", { state: "loading" }), seat(null, { state: "hidden" })]),
      "zh-CN",
      (puuid, anchor) => void opened.push(`${puuid}:${anchor.tagName}`),
    );
    const buttons = list.querySelectorAll<HTMLElement>("button.winer-row--button");
    expect(buttons, "a hidden player has no history to open").toHaveLength(1);
    buttons[0]?.click();
    expect(opened).toEqual(["p:BUTTON"]);
    expect(
      panelRows(champSelect([seat("p", { state: "loading" })]), "zh-CN").querySelector("button"),
    ).toBeNull();
  });
});

describe("the controller", () => {
  const settings = (historyInClient = true): Settings =>
    ({
      general: { language: "zh-CN" },
      plugin: { teamPanel: true, friendStatus: false, lobbyPanel: true, historyInClient },
    }) as unknown as Settings;
  const snapshot = (extra: Partial<Snapshot>): Snapshot => ({
    rev: 1,
    connection: { status: "searching" },
    me: null,
    phase: "Lobby",
    champSelect: null,
    game: null,
    friends: null,
    lobby: lobbyView(),
    ...extra,
  });
  /** Moves the view on by one patch and draws it. */
  const update = (controller: Controller, rev: number, patch: Patch) => {
    controller.state.event({ type: "update", data: { rev, patch } });
    controller.render();
  };

  /** A controller that draws the lobby, its requests answered by the test. */
  function drawing(extra: Partial<Snapshot> = {}, on = true) {
    const controller = new Controller(document);
    controller.state.hello(snapshot(extra), settings(on));
    controller.render();
    let next = 0;
    const asked = vi.spyOn(controller.bridge, "history").mockImplementation(() => (next += 1));
    const inWiner = vi.spyOn(controller.bridge, "openHistory").mockReturnValue(true);
    return { controller, asked, inWiner };
  }

  it("opens the panel on a lobby member and keeps their summary as the view has it", () => {
    const { controller, asked, inWiner } = drawing();
    expect(controller.pick("mate", null)).toBe(true);
    expect(asked).toHaveBeenCalledWith("mate");
    expect(inWiner).not.toHaveBeenCalled();
    expect(host()?.querySelector(".winer-history-summary")?.textContent).toContain("战力 7.4");
    controller.history.answer(answer(1, page("mate", [game(1)])));
    expect(rows()).toHaveLength(1);
    controller.render();
    expect(rows(), "a render leaves the games alone").toHaveLength(1);
    expect(controller.pick("mate", null), "the same player again: closed").toBe(true);
    expect(host()).toBeNull();
  });

  it("closes when the client leaves the lobby or the match-found dialog comes up", () => {
    const { controller } = drawing();
    controller.pick("mate", null);
    update(controller, 2, { key: "phase", value: "ReadyCheck" });
    expect(host(), "the dialog has the screen").toBeNull();

    update(controller, 3, { key: "phase", value: "Lobby" });
    controller.pick("mate", null);
    expect(host()).not.toBeNull();
    update(controller, 4, { key: "lobby", value: null });
    expect(host(), "the lobby is gone").toBeNull();
  });

  it("closes when the option goes off, and with it off opens winer's window as before", () => {
    const { controller, asked, inWiner } = drawing();
    controller.pick("mate", null);
    controller.state.event({ type: "settings", data: settings(false) });
    controller.render();
    expect(host()).toBeNull();
    expect(controller.pick("mate", null)).toBe(true);
    expect(inWiner).toHaveBeenCalledWith("mate");
    expect(asked).toHaveBeenCalledTimes(1);
    expect(host()).toBeNull();
  });

  it("opens only in the context that draws, and clears a panel the other context left", () => {
    const { controller } = drawing();
    const other = new Controller(document);
    other.state.hello(snapshot({}), settings());
    const asked = vi.spyOn(other.bridge, "history");
    expect(other.pick("mate", null)).toBe(false);
    expect(asked).not.toHaveBeenCalled();

    document.body.insertAdjacentHTML(
      "beforeend",
      `<section class="winer-panel" data-winer-panel="history" data-winer-context="${other.context}"></section>`,
    );
    controller.render();
    expect(document.querySelectorAll("[data-winer-panel='history']")).toHaveLength(0);
  });

  it("opens on a teammate in champ select with their tier and title", () => {
    const ready = { state: "ready" as const, ...summary("p", 12, 20) };
    const { controller } = drawing({
      phase: "ChampSelect",
      lobby: null,
      champSelect: champSelect([seat("p", ready, RATING)]),
    });
    expect(controller.pick("p", null)).toBe(true);
    const said = host()?.querySelector(".winer-history-summary");
    expect(said?.querySelector(".winer-standing--best")?.textContent).toBe("峡谷通天代");
    expect(said?.querySelector(".winer-title")?.textContent).toBe("版本答案");
    expect(said?.textContent).toContain("战力 7.4");
    expect(controller.pick("nobody", null), "a player the view does not have").toBe(false);
    update(controller, 2, { key: "phase", value: "InProgress" });
    expect(host(), "champ select is over").toBeNull();
  });
});

/** The bridge's socket, driven by the test. */
class FakeSocket extends EventTarget {
  static readonly OPEN = 1;
  static sockets: FakeSocket[] = [];
  readyState = 0;
  readonly sent: string[] = [];

  constructor(readonly url: string) {
    super();
    FakeSocket.sockets.push(this);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }

  open(): void {
    this.readyState = FakeSocket.OPEN;
    this.dispatchEvent(new Event("open"));
  }

  receive(data: unknown): void {
    this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(data) }));
  }
}

describe("the bridge", () => {
  it("asks for a player's games under an id and hands the answer to the panel", async () => {
    FakeSocket.sockets = [];
    vi.stubGlobal("WebSocket", FakeSocket);
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: true, json: async () => ({ port: 4242, token: "t" }) })),
    );
    const onEvent = vi.fn();
    const onHistory = vi.fn();
    const handlers: BridgeHandlers = {
      onHello: vi.fn(),
      onEvent,
      onConnection: vi.fn(),
      onHistory,
    };
    const bridge = new Bridge(
      handlers,
      { version: "9.9.9", context: "c" },
      "http://x/bootstrap.json",
    );
    expect(bridge.history("p"), "no connection, no request").toBeNull();
    bridge.start();
    await vi.waitFor(() => expect(FakeSocket.sockets).toHaveLength(1));
    const socket = FakeSocket.sockets[0] as FakeSocket;
    socket.open();
    expect([bridge.history("p"), bridge.history("q")]).toEqual([1, 2]);
    expect(socket.sent.slice(1).map((sent) => JSON.parse(sent))).toEqual([
      { type: "history", puuid: "p", requestId: 1 },
      { type: "history", puuid: "q", requestId: 2 },
    ]);

    const result = answer(2, page("q", [game(1)]));
    socket.receive(result);
    socket.receive({ type: "event", event: { type: "gameData" } });
    expect(onHistory).toHaveBeenCalledWith(result);
    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(onEvent).toHaveBeenCalledWith({ type: "gameData" });
    bridge.stop();
  });
});
