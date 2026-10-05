import type { ChampSelectView, PlayerSummary, Seat, Settings, Snapshot } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { benchChampion, interceptBenchClicks, liftBenchCooldown } from "./bench";
import { backoff, parseBootstrap } from "./bridge";
import { Controller } from "./index";
import { PluginState } from "./state";
import { ROW_SELECTOR, clearRows, decorateRows, lineKey, panelRows } from "./team";

function summary(wins: number, games: number, streak: number): PlayerSummary {
  return {
    puuid: "p",
    name: { gameName: "Ann", tagLine: "1" },
    level: 30,
    iconId: 1,
    private: false,
    ranked: { solo: { tier: "DIAMOND", division: "II", lp: 50, wins: 10, losses: 9 }, flex: null },
    recent: { games, wins, kills: 5, deaths: 2, assists: 7, streak, matches: [], champions: [] },
  };
}

function seat(stats: Seat["stats"], championId = 103, rating: Seat["rating"] = null): Seat {
  return {
    puuid: "p",
    name: { gameName: "Ann", tagLine: "1" },
    championId,
    intent: false,
    position: "middle",
    spells: [4, 14],
    isSelf: false,
    premade: null,
    stats,
    rating,
  };
}

function view(seats: Seat[]): ChampSelectView {
  return {
    gameId: 1,
    queueId: 420,
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

function partyRows(count: number): HTMLElement {
  const party = document.createElement("div");
  party.className = "party visible";
  for (let index = 0; index < count; index += 1) {
    const row = document.createElement("div");
    row.className = "summoner-wrapper visible left";
    const name = document.createElement("div");
    name.className = "player-name-wrapper";
    row.append(name);
    party.append(row);
  }
  return party;
}

describe("bootstrap", () => {
  it("accepts only a port and a token", () => {
    expect(parseBootstrap({ port: 4242, token: "t" })).toEqual({ port: 4242, token: "t" });
    expect(parseBootstrap({ port: 0, token: "t" })).toBeNull();
    expect(parseBootstrap({ port: 4242, token: "" })).toBeNull();
    expect(parseBootstrap("4242")).toBeNull();
  });

  it("backs off to ten seconds", () => {
    expect([0, 1, 2, 3, 4, 9].map(backoff)).toEqual([1000, 2000, 4000, 8000, 10_000, 10_000]);
  });
});

describe("state", () => {
  const snapshot: Snapshot = {
    rev: 2,
    connection: { status: "searching" },
    me: null,
    phase: "None",
    champSelect: null,
    game: null,
  };

  it("applies newer patches only", () => {
    const state = new PluginState();
    state.hello(snapshot, {} as Settings);
    state.event({ type: "update", data: { rev: 2, patch: { key: "phase", value: "Lobby" } } });
    expect(state.snapshot?.phase).toBe("None");
    state.event({
      type: "update",
      data: { rev: 3, patch: { key: "phase", value: "ChampSelect" } },
    });
    expect(state.snapshot).toMatchObject({ rev: 3, phase: "ChampSelect" });
  });

  it("forgets the snapshot when the bridge drops", () => {
    const state = new PluginState();
    state.hello(snapshot, {} as Settings);
    state.connection(false);
    expect(state.snapshot).toBeNull();
  });
});

describe("team", () => {
  it("writes one line under each party row and leaves an unchanged one alone", () => {
    document.body.replaceChildren(partyRows(2));
    const team = view([
      seat({ state: "ready", ...summary(12, 20, 3) }),
      seat({ state: "loading" }),
    ]);
    expect(decorateRows(document, team, "zh-CN")).toBe(2);
    const lines = [...document.querySelectorAll<HTMLElement>(".winer-inline")];
    expect(lines.map((line) => line.textContent)).toEqual([
      "钻石 II60%KDA 6.03 连胜",
      "正在读取战绩",
    ]);

    const first = lines[0]?.firstChild;
    decorateRows(document, team, "zh-CN");
    expect(document.querySelector(".winer-inline")?.firstChild).toBe(first);

    clearRows(document);
    expect(document.querySelectorAll(".winer-inline")).toHaveLength(0);
  });

  it("leads a rated line with the tier's own name", () => {
    document.body.replaceChildren(partyRows(1));
    const rated = seat({ state: "ready", ...summary(12, 20, 0) }, 103, {
      score: 7.4,
      tier: 0,
      tiers: 5,
      label: "上等马",
      grade: null,
      title: null,
      quip: null,
    });
    decorateRows(document, view([rated]), "zh-CN");
    const chip = document.querySelector(".winer-standing--best");
    expect(chip?.textContent).toBe("上等马");
    expect(chip?.getAttribute("title")).toBe("7.4");
    expect(document.querySelector(".winer-title")).toBeNull();
    expect(lineKey(rated, "zh-CN")).not.toBe(lineKey({ ...rated, rating: null }, "zh-CN"));
  });

  it("puts a grade's letter before its name and the roast title after it", () => {
    document.body.replaceChildren(partyRows(1));
    const rating = {
      score: 7.8,
      tier: 0,
      tiers: 8,
      label: "峡谷通天代",
      grade: 0,
      title: "版本答案",
      quip: "对面五个人举报代练的水平",
    };
    const graded = seat({ state: "ready", ...summary(12, 20, 0) }, 103, rating);
    decorateRows(document, view([graded]), "zh-CN");
    const chip = document.querySelector(".winer-standing--best");
    expect(chip?.textContent).toBe("S+ 峡谷通天代");
    expect(chip?.getAttribute("title")).toBe("7.8 · 对面五个人举报代练的水平");
    expect(chip?.nextElementSibling?.textContent).toBe("版本答案");
    expect(lineKey(graded, "zh-CN"), "a new title rewrites the line").not.toBe(
      lineKey({ ...graded, rating: { ...rating, title: null } }, "zh-CN"),
    );
  });

  it("names the side on the local player's line only, loading or not", () => {
    document.body.replaceChildren(partyRows(2));
    const me = { ...seat({ state: "loading" }), isSelf: true };
    const mate = seat({ state: "ready", ...summary(12, 20, 0) });
    decorateRows(document, { ...view([me, mate]), side: "red" }, "zh-CN");
    const lines = [...document.querySelectorAll<HTMLElement>(".winer-inline")];
    expect(lines[0]?.querySelector(".winer-side--red")?.textContent).toBe("红色方");
    expect(lines[1]?.querySelector(".winer-side")).toBeNull();

    decorateRows(document, { ...view([me, mate]), side: "blue" }, "en");
    expect(document.querySelector(".winer-side--blue")?.textContent).toBe("Blue side");
    decorateRows(document, view([me, mate]), "en");
    expect(document.querySelector(".winer-side"), "a mode without sides shows none").toBeNull();
  });

  it("keys a line by what it shows", () => {
    const ready = seat({ state: "ready", ...summary(1, 2, 0) });
    expect(lineKey(ready, "zh-CN")).toBe(
      lineKey(seat({ state: "ready", ...summary(1, 2, 0) }), "zh-CN"),
    );
    expect(lineKey(ready, "zh-CN")).not.toBe(lineKey(ready, "en"));
    expect(lineKey(ready, "zh-CN")).not.toBe(lineKey(seat({ state: "loading" }), "zh-CN"));
  });

  it("names hidden players in the panel", () => {
    const rows = panelRows(view([{ ...seat({ state: "hidden" }), name: null, puuid: null }]), "en");
    expect(rows.textContent).toContain("Hidden player");
  });

  it("finds the party rows by the client's own classes", () => {
    expect(ROW_SELECTOR).toBe(".party.visible .summoner-wrapper.visible.left");
  });
});

describe("bench", () => {
  it("takes the cooldown classes off bench champions and nothing else", () => {
    document.body.innerHTML = `
      <div class="bench-container">
        <div class="champion-bench-item on-cooldown"><div class="cooldown-mask"></div></div>
        <div class="champion-bench-item bench-item-on-cooldown-state"></div>
        <div class="champion-bench-item"></div>
      </div>
      <div class="elsewhere on-cooldown"></div>`;
    expect(liftBenchCooldown(document)).toBe(2);
    expect(document.querySelectorAll('.bench-container [class*="on-cooldown"]')).toHaveLength(0);
    expect(document.querySelector(".elsewhere")?.classList.contains("on-cooldown")).toBe(true);
    expect(document.querySelectorAll(".champion-bench-item")).toHaveLength(3);
    expect(liftBenchCooldown(document)).toBe(0);
  });

  it("reads the champion from the bench item's icon, image or background", () => {
    document.body.innerHTML = `
      <div class="bench-container">
        <div class="champion-bench-item" id="a"><img src="/lol-game-data/assets/v1/champion-icons/22.png"></div>
        <div class="champion-bench-item" id="b"><div style="background-image: url(/lol-game-data/assets/v1/champion-icons/157.png)"></div></div>
        <div class="champion-bench-item" id="c"></div>
      </div>`;
    const id = (name: string) => benchChampion(document.getElementById(name) as Element);
    expect([id("a"), id("b"), id("c")]).toEqual([22, 157, null]);
  });

  it("takes a bench click away from the client only when winer carries it out", () => {
    document.body.innerHTML = `<div class="bench-container"><div class="champion-bench-item"><img src="/x/champion-icons/22.png"></div></div>`;
    const icon = document.querySelector("img") as HTMLElement;
    let client = 0;
    icon.addEventListener("click", () => (client += 1));
    let accept = true;
    const swapped: number[] = [];
    const stop = interceptBenchClicks(document, (champion) => {
      swapped.push(champion);
      return accept;
    });
    icon.click();
    expect([swapped, client]).toEqual([[22], 0]);
    accept = false;
    icon.click();
    expect([swapped, client]).toEqual([[22, 22], 1]);
    stop();
    icon.click();
    expect([swapped.length, client]).toEqual([2, 2]);
  });

  it("finds the bench champion under an overlay that took the click", () => {
    document.body.innerHTML = `<div class="bench-container"><div class="champion-bench-item"><img src="/x/champion-icons/36.png"></div></div><div id="pengu-root"></div>`;
    const overlay = document.getElementById("pengu-root") as HTMLElement;
    const item = document.querySelector(".champion-bench-item") as Element;
    // jsdom has no layout: say what a hit test at the click's point would find.
    Object.defineProperty(document, "elementsFromPoint", {
      configurable: true,
      value: () => [overlay, item, document.body],
    });
    const swapped: number[] = [];
    const stop = interceptBenchClicks(document, (champion) => swapped.push(champion) > 0);
    overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 10, clientY: 10 }));
    stop();
    Reflect.deleteProperty(document, "elementsFromPoint");
    expect(swapped).toEqual([36]);
  });

  it("is a no-op without a bench", () => {
    document.body.innerHTML = "<main></main>";
    expect(liftBenchCooldown(document)).toBe(0);
  });
});

describe("controller", () => {
  it("lets one live context draw and hands over when it goes quiet", () => {
    const first = new Controller(document);
    const second = new Controller(document);
    expect(first.owns(1000)).toBe(true);
    expect(second.owns(2000)).toBe(false);
    expect(first.owns(3000)).toBe(true);
    expect(second.owns(9000)).toBe(true);
    expect(first.owns(9500)).toBe(false);
  });
});
