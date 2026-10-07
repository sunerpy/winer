// Friends' games, the lobby's party and premade groups, as the plugin draws them in the client.
import type {
  FriendView,
  FriendsView,
  LobbyMember,
  LobbyView,
  PlayerSummary,
  Seat,
  Settings,
  Snapshot,
} from "@winer/shared";
import { afterEach, describe, expect, it, vi } from "vitest";

import { innermost } from "./find";
import { clearFriends, decorateFriends, friendLine } from "./friends";
import { groupSlot } from "./groups";
import { Controller } from "./index";
import { clearLobby, decorateLobby, interceptAvatarClicks, lobbyLine, removePanels } from "./lobby";
import { FRIENDS_LIST, LOBBY } from "./selectors";
import { lineKey, statsLine } from "./team";

const NOW = 1_800_000_000_000;

function summary(puuid: string, wins: number, games: number): PlayerSummary {
  return {
    puuid,
    name: { gameName: puuid, tagLine: "1" },
    level: 30,
    iconId: 1,
    private: false,
    ranked: { solo: null, flex: null },
    recent: {
      games,
      wins,
      kills: 6,
      deaths: 3,
      assists: 9,
      streak: 0,
      matches: [],
      champions: [],
      score: null,
      source: null,
      family: null,
      away: 0,
    },
  };
}

function friend(
  puuid: string,
  name: string,
  status: FriendView["status"],
  group: number | null = null,
): FriendView {
  return {
    puuid,
    name: { gameName: name, tagLine: "10001" },
    iconId: 1,
    availability: "dnd",
    status,
    group,
  };
}

type InGame = Extract<FriendView["status"], { state: "inGame" }>;

const inGame = (mode: string, minutes: number): InGame => ({
  state: "inGame",
  mode,
  queueId: 450,
  startedAt: NOW - minutes * 60_000 - 4_000,
  observable: false,
});

/** The client's friends list as the selectors guess it: a roster of entries, a name in each. */
function roster(entries: { name: string; puuid?: string }[]): HTMLElement {
  const list = document.createElement("lol-social-roster");
  for (const entry of entries) {
    const member = document.createElement("lol-social-roster-member");
    if (entry.puuid) member.setAttribute("data-puuid", entry.puuid);
    const name = document.createElement("span");
    name.className = "member-name";
    name.textContent = entry.name;
    const status = document.createElement("span");
    status.className = "member-status";
    status.textContent = "游戏中";
    member.append(name, status);
    list.append(member);
  }
  return list;
}

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute("data-winer-owner");
  document.documentElement.removeAttribute("data-winer-beat");
});

describe("friends", () => {
  it("names the mode and the time a friend's game has run, and nothing outside a game", () => {
    expect(friendLine(friend("a", "Ann", inGame("极地大乱斗", 12)), NOW, "zh-CN")).toBe(
      "极地大乱斗 · 12:04",
    );
    const unknown = friend("b", "Bo", { ...inGame("", 0), startedAt: 0 });
    expect(friendLine(unknown, NOW, "en"), "no start, no time; no mode, a word").toBe("In game");
    const picking = friend("c", "Cy", {
      state: "champSelect",
      mode: "排位赛",
      queueId: 420,
      since: NOW,
    });
    expect(friendLine(picking, NOW, "zh-CN")).toBeNull();
    expect([1, 6, 7, 13].map(groupSlot)).toEqual([1, 6, 1, 1]);
  });

  it("writes the game after the name, stripes a group's entries, and takes both back after", () => {
    document.body.append(
      roster([{ name: "Ann" }, { name: "Bo" }, { name: "Cy" }, { name: "Off", puuid: "d" }]),
    );
    const view: FriendsView = {
      friends: [
        friend("a", "Ann", inGame("极地大乱斗", 12), 1),
        friend("b", "Bo", inGame("极地大乱斗", 12), 1),
        friend("c", "Cy", inGame("排位赛 单排/双排", 30)),
      ],
    };
    expect(decorateFriends(document, view, NOW, "zh-CN")).toEqual({ entries: 4, lines: 3 });
    const entries = [...document.querySelectorAll("lol-social-roster-member")];
    expect(
      entries.map((entry) => entry.querySelector(".winer-friend")?.textContent ?? null),
    ).toEqual(["极地大乱斗 · 12:04", "极地大乱斗 · 12:04", "排位赛 单排/双排 · 30:04", null]);
    expect(entries.map((entry) => entry.getAttribute("data-winer-friend-group"))).toEqual([
      "1",
      "1",
      null,
      null,
    ]);
    expect(
      entries[0]?.querySelector(".member-name")?.nextElementSibling?.className,
      "the line goes right after the name",
    ).toBe("winer-friend");

    const line = entries[0]?.querySelector(".winer-friend");
    decorateFriends(document, view, NOW + 500, "zh-CN");
    expect(entries[0]?.querySelector(".winer-friend"), "an unchanged line is kept").toBe(line);
    decorateFriends(document, view, NOW + 1_000, "zh-CN");
    expect(line?.textContent, "and ticks").toBe("极地大乱斗 · 12:05");

    // Bo's game ended; Ann is still in hers but alone now.
    decorateFriends(
      document,
      { friends: [friend("a", "Ann", inGame("极地大乱斗", 12))] },
      NOW,
      "zh-CN",
    );
    expect(document.querySelectorAll(".winer-friend")).toHaveLength(1);
    expect(document.querySelectorAll("[data-winer-friend-group]")).toHaveLength(0);

    decorateFriends(document, null, NOW, "zh-CN");
    expect(document.querySelectorAll(".winer-friend")).toHaveLength(0);
  });

  it("knows an entry by the puuid the client writes, before its name", () => {
    document.body.append(roster([{ name: "Someone else", puuid: "a" }]));
    decorateFriends(
      document,
      { friends: [friend("a", "Ann", inGame("极地大乱斗", 1))] },
      NOW,
      "zh-CN",
    );
    expect(document.querySelector(".winer-friend")?.textContent).toBe("极地大乱斗 · 1:04");
  });

  it("finds entries by name where no entry selector matches", () => {
    const list = document.createElement("div");
    list.className = "lol-social-roster";
    list.innerHTML = `<div class="row"><span>Ann</span><span>游戏中</span></div><div class="row"><span>Bo</span></div>`;
    document.body.append(list);
    expect(FRIENDS_LIST.member.some((selector) => list.querySelector(selector))).toBe(false);
    const drawn = decorateFriends(
      document,
      { friends: [friend("a", "Ann", inGame("极地大乱斗", 2))] },
      NOW,
      "zh-CN",
    );
    expect(drawn.lines).toBe(1);
    expect(list.querySelector(".row span")?.nextElementSibling?.textContent).toBe(
      "极地大乱斗 · 2:04",
    );
    clearFriends(document);
    expect(list.querySelector(".winer-friend")).toBeNull();
  });

  it("decorates each entry where a guessed class also matches the group around them", () => {
    const list = document.createElement("div");
    list.className = "lol-social-roster";
    // `[class*='roster-member']` matches the group as well as both of its entries.
    list.innerHTML = `<div class="roster-members-group"><div class="roster-member-row"><span class="member-name">Ann</span></div><div class="roster-member-row"><span class="member-name">Bo</span></div></div>`;
    document.body.append(list);
    const view: FriendsView = {
      friends: [
        friend("a", "Ann", inGame("极地大乱斗", 3), 1),
        friend("b", "Bo", inGame("极地大乱斗", 3), 1),
      ],
    };
    expect(decorateFriends(document, view, NOW, "zh-CN")).toEqual({ entries: 2, lines: 2 });
    const rows = [...list.querySelectorAll(".roster-member-row")];
    expect(rows.map((row) => row.getAttribute("data-winer-friend-group"))).toEqual(["1", "1"]);
    expect(rows.map((row) => row.querySelector(".winer-friend")?.textContent)).toEqual([
      "极地大乱斗 · 3:04",
      "极地大乱斗 · 3:04",
    ]);
    expect(
      list.querySelector(".roster-members-group")?.hasAttribute("data-winer-friend-group"),
    ).toBe(false);
  });

  it("leaves a page without the friends list alone", () => {
    document.body.innerHTML = "<main><span>Ann</span></main>";
    expect(
      decorateFriends(
        document,
        { friends: [friend("a", "Ann", inGame("极地大乱斗", 2))] },
        NOW,
        "zh-CN",
      ),
    ).toEqual({ entries: 0, lines: 0 });
    expect(document.querySelector(".winer-friend")).toBeNull();
  });
});

function member(
  puuid: string,
  name: string,
  stats: LobbyMember["stats"],
  score: number | null,
): LobbyMember {
  return {
    puuid,
    name: { gameName: name, tagLine: "7" },
    iconId: 1,
    isSelf: puuid === "me",
    leader: puuid === "me",
    positions: ["middle"],
    stats,
    score,
    note: null,
  };
}

function lobbyView(): LobbyView {
  return {
    queueId: 420,
    custom: false,
    members: [
      member("me", "Me", { state: "ready", ...summary("me", 12, 20) }, 7.4),
      member("mate", "Mate", { state: "loading" }, null),
    ],
  };
}

/** The client's lobby as the selectors guess it: one card per member, a banner and a name in it. */
function lobbyCards(names: string[]): HTMLElement {
  const party = document.createElement("div");
  for (const name of names) {
    party.insertAdjacentHTML(
      "beforeend",
      `<div class="lobby-party-member"><div class="lobby-banner"></div><div class="summoner-icon"><span class="kick-button">✕</span></div><div class="player-name">${name}</div></div>`,
    );
  }
  return party;
}

describe("lobby", () => {
  it("says each member's form in one line", () => {
    const [me, mate] = lobbyView().members;
    expect(me && lobbyLine(me, "zh-CN")).toBe("胜率 60% · KDA 5.0 · 战力 7.4");
    expect(mate && lobbyLine(mate, "en")).toBe("Loading stats");
    const fresh = member("x", "X", { state: "ready", ...summary("x", 0, 0) }, null);
    expect(lobbyLine(fresh, "zh-CN")).toBe("近期没有对局");
  });

  it("writes a line above each banner that opens the member's history", () => {
    document.body.append(lobbyCards(["Me", "Mate", "Stranger"]));
    const opened: string[] = [];
    expect(
      decorateLobby(document, lobbyView(), "zh-CN", (puuid) => opened.push(puuid)),
      "two cards show a member of the party; the stranger's gets nothing",
    ).toBe(2);
    const cards = [...document.querySelectorAll(".lobby-party-member")];
    expect(cards.map((card) => card.querySelector(".winer-lobby")?.textContent ?? null)).toEqual([
      "胜率 60% · KDA 5.0 · 战力 7.4",
      "正在读取战绩",
      null,
    ]);
    expect(cards[0]?.querySelector(".lobby-banner")?.previousElementSibling?.className).toBe(
      "winer-lobby",
    );
    cards[1]?.querySelector<HTMLElement>(".winer-lobby")?.click();
    expect(opened).toEqual(["mate"]);

    clearLobby(document);
    expect(document.querySelectorAll(".winer-lobby, [data-winer-puuid]")).toHaveLength(0);
  });

  it("puts the line under the tokens of the lobby the client draws", () => {
    // As measured on 16.19 (GZ100, a normal ARAM lobby): the banner hangs from beneath the
    // navigation bar, so a line above it would be covered.
    const party = document.createElement("div");
    party.className = "party-members-container";
    for (const name of ["Me", "Mate"]) {
      party.insertAdjacentHTML(
        "beforeend",
        `<div class="v2-banner-component"><div class="lobby-banner"><div class="lobby-banner-contents"><div class="banner-spacer"></div><div class="player-identity-container"></div><div class="player-name-container"><div class="player-name"><span class="player-name__game-name">${name}</span></div></div><div class="player-achievements-container"></div></div></div></div>`,
      );
    }
    document.body.append(party);
    expect(decorateLobby(document, lobbyView(), "zh-CN", () => undefined)).toBe(2);
    const lines = [...document.querySelectorAll(".winer-lobby")];
    expect(lines).toHaveLength(2);
    for (const line of lines) {
      expect(line.previousElementSibling?.className).toBe("player-achievements-container");
    }
    clearLobby(document);
  });

  it("clears a panel the page's other context left behind", () => {
    for (const context of ["a", "b"]) {
      document.body.insertAdjacentHTML(
        "beforeend",
        `<section class="winer-panel" data-winer-panel="lobby" data-winer-context="${context}"></section>`,
      );
    }
    document.body.insertAdjacentHTML(
      "beforeend",
      `<section class="winer-panel" data-winer-panel="team" data-winer-context="a"></section>`,
    );
    removePanels(document, "lobby", "b");
    expect(
      [...document.querySelectorAll<HTMLElement>(".winer-panel")].map((panel) => [
        panel.dataset.winerPanel,
        panel.dataset.winerContext,
      ]),
    ).toEqual([
      ["lobby", "b"],
      ["team", "a"],
    ]);
    removePanels(document, "lobby");
    expect(document.querySelectorAll('[data-winer-panel="lobby"]')).toHaveLength(0);
    document.querySelectorAll(".winer-panel").forEach((panel) => panel.remove());
  });

  it("opens a history from a click on the avatar and leaves the client's controls to it", () => {
    document.body.append(lobbyCards(["Me"]));
    decorateLobby(document, lobbyView(), "zh-CN", () => undefined);
    const opened: string[] = [];
    const stop = interceptAvatarClicks(document, (puuid) => opened.push(puuid) > 0);
    const avatar = document.querySelector(LOBBY.avatar[0]) as HTMLElement;
    let client = 0;
    avatar.addEventListener("click", () => (client += 1));
    avatar.click();
    expect([opened, client]).toEqual([["me"], 0]);
    (document.querySelector(".kick-button") as HTMLElement).click();
    expect(opened, "the kick button is the client's").toEqual(["me"]);
    (document.querySelector(".player-name") as HTMLElement).click();
    expect(opened, "the name is not the avatar").toEqual(["me"]);
    stop();
  });
});

describe("finding the client's elements", () => {
  it("keeps the inner of two matches and only those the caller can use", () => {
    document.body.innerHTML = `<ul class="x-list"><li class="x-item"><b>1</b></li><li class="x-item"></li></ul>`;
    const found = innermost(document, [".missing", "[class*='x-']"], (element) =>
      Boolean(element.querySelector("b")),
    );
    expect(
      found.map((element) => element.className),
      "the list holds the item: the item",
    ).toEqual(["x-item"]);
    expect(innermost(document, [".missing"], () => true)).toEqual([]);
  });
});

describe("premade groups", () => {
  const seat = (premade: number | null): Seat => ({
    puuid: "p",
    name: { gameName: "Ann", tagLine: "1" },
    championId: 103,
    intent: false,
    position: null,
    spells: [4, 14],
    isSelf: false,
    premade,
    premadeInferred: false,
    note: null,
    stats: { state: "loading" },
    rating: null,
  });

  it("leads a premade player's line with the party's number in its colour", () => {
    const line = statsLine(seat(2), "zh-CN");
    const chip = line.querySelector(".winer-group");
    expect(chip?.textContent).toBe("开黑 2");
    expect(chip?.getAttribute("data-winer-group")).toBe("2");
    expect(statsLine(seat(null), "zh-CN").querySelector(".winer-group")).toBeNull();
    expect(lineKey(seat(1), "zh-CN")).not.toBe(lineKey(seat(null), "zh-CN"));

    // A party read from recent games: its own words and the dashed swatch's class.
    const guessed = { ...seat(3), premadeInferred: true };
    const inferred = statsLine(guessed, "zh-CN").querySelector(".winer-group");
    expect(inferred?.textContent).toBe("疑似开黑 3");
    expect(inferred?.classList.contains("winer-group--inferred")).toBe(true);
    expect(lineKey(guessed, "zh-CN")).not.toBe(lineKey(seat(3), "zh-CN"));

    // The user's own note: its tag, the text on hover; a note of text only reads 备注.
    const noted = {
      ...seat(null),
      note: { tag: "weak" as const, text: "不看小地图", name: null, updatedAt: 1 },
    };
    const noteChip = statsLine(noted, "zh-CN").querySelector(".winer-note");
    expect(noteChip?.textContent).toBe("坑");
    expect(noteChip?.getAttribute("title")).toBe("不看小地图");
    const plain = { ...noted, note: { ...noted.note, tag: null } };
    expect(statsLine(plain, "zh-CN").querySelector(".winer-note")?.textContent).toBe("备注");
    expect(lineKey(noted, "zh-CN")).not.toBe(lineKey(plain, "zh-CN"));
  });
});

describe("controller", () => {
  const settings = {
    general: { language: "zh-CN" },
    plugin: { teamPanel: true, friendStatus: true, lobbyPanel: true },
  } as unknown as Settings;
  const snapshot = (extra: Partial<Snapshot>): Snapshot => ({
    rev: 1,
    connection: { status: "searching" },
    me: null,
    phase: "Lobby",
    champSelect: null,
    game: null,
    friends: null,
    lobby: null,
    ...extra,
  });

  it("draws friends' games and the lobby, and the panel where the lobby's cards are missing", () => {
    document.body.append(roster([{ name: "Ann" }]));
    const controller = new Controller(document);
    const open = vi.spyOn(controller.bridge, "openHistory").mockReturnValue(true);
    controller.state.hello(
      snapshot({
        friends: {
          friends: [friend("a", "Ann", { ...inGame("极地大乱斗", 1), startedAt: Date.now() })],
        },
        lobby: lobbyView(),
      }),
      settings,
    );
    controller.render();
    expect(document.querySelector(".winer-friend")?.textContent).toMatch(/^极地大乱斗 · 0:0\d$/);
    const panel = document.querySelector<HTMLElement>("[data-winer-panel='lobby']");
    expect(panel?.textContent).toContain("Mate");
    panel?.querySelectorAll<HTMLElement>(".winer-row--button")[1]?.click();
    expect(open).toHaveBeenCalledWith("mate");

    controller.state.event({
      type: "settings",
      data: { ...settings, plugin: { ...settings.plugin, friendStatus: false, lobbyPanel: false } },
    });
    controller.render();
    expect(document.querySelector(".winer-friend"), "the switch takes it off").toBeNull();
    expect(document.querySelector("[data-winer-panel='lobby']")).toBeNull();
  });

  it("shows the lobby panel where the cards it finds name no one in the party", () => {
    document.body.append(lobbyCards(["Somebody", "Nobody"]));
    const controller = new Controller(document);
    controller.state.hello(snapshot({ lobby: lobbyView() }), settings);
    controller.render();
    expect(document.querySelectorAll(".winer-lobby")).toHaveLength(0);
    expect(document.querySelector("[data-winer-panel='lobby']")?.textContent).toContain("Mate");
  });

  it("keeps the lobby panel out of the way of the match-found dialog", () => {
    const controller = new Controller(document);
    controller.state.hello(snapshot({ phase: "ReadyCheck", lobby: lobbyView() }), settings);
    controller.render();
    expect(document.querySelector("[data-winer-panel='lobby']")).toBeNull();
    controller.state.event({
      type: "update",
      data: { rev: 2, patch: { key: "phase", value: "Matchmaking" } },
    });
    controller.render();
    expect(document.querySelector("[data-winer-panel='lobby']")).not.toBeNull();
  });

  it("asks for a history only from the context that draws", () => {
    const drawing = new Controller(document);
    const other = new Controller(document);
    expect(drawing.owns()).toBe(true);
    const sent = vi.spyOn(other.bridge, "openHistory");
    expect(other.openHistory("p")).toBe(false);
    expect(sent).not.toHaveBeenCalled();
  });
});
