import type { LoadoutView, Settings, Snapshot } from "@winer/shared";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { LoadoutResult } from "./bridge";
import { interceptPlayerClicks } from "./history";
import { Controller } from "./index";
import {
  COLLAPSED_KEY,
  augmentCard,
  LOADOUT_PANEL,
  LOADOUT_TIMEOUT_MS,
  LoadoutPanel,
  type Store,
  answerText,
  currentIs,
  loadoutBody,
  ratesLine,
  sameSpells,
  samePage,
  sourceLine,
} from "./loadout";

const NO_RATES = { pick: null, win: null, games: null, placement: null, first: null };
const ELECTROCUTE = {
  primaryStyle: 8100,
  subStyle: 8000,
  perks: [8112, 8139, 8138, 8135, 8009, 9103, 5008, 5008, 5001],
};
const COMET = {
  primaryStyle: 8200,
  subStyle: 8100,
  perks: [8229, 8226, 8210, 8237, 8139, 8135, 5008, 5008, 5001],
};

/** An ARAM view: the client's recommended pages, Flash and Snowball put up by winer. */
function loadout(change: Partial<LoadoutView> = {}): LoadoutView {
  return {
    gameId: 9,
    championId: 103,
    mode: "aram",
    lane: null,
    held: true,
    revision: 4,
    auto: "mineOrRecommended",
    takesRunes: true,
    current: { runes: ELECTROCUTE, page: "winer · 阿狸", spells: [4, 32] },
    mine: null,
    recommendation: {
      state: "ready",
      recommendation: {
        runes: [
          { page: ELECTROCUTE, rates: NO_RATES },
          { page: COMET, rates: NO_RATES },
        ],
        spells: [
          { spells: [4, 32], rates: NO_RATES },
          { spells: [4, 6], rates: NO_RATES },
        ],
        runesFrom: "client",
        spellsFrom: "client",
        patch: "",
        augments: [],
        core: [],
        boots: [],
        buildFrom: null,
      },
    },
    applied: { runes: "client", spells: "client", noPage: false },
    assets: [
      { id: 8112, name: "电刑", icon: "/lol-game-data/assets/v1/perk-images/electrocute.png" },
      { id: 4, name: "闪现", icon: "/lol-game-data/assets/DATA/Spells/Icons2D/flash.png" },
    ],
    ...change,
  };
}

function memoryStore(): Store & { values: Map<string, unknown> } {
  const values = new Map<string, unknown>();
  return {
    values,
    get: (key, fallback) => (values.has(key) ? values.get(key) : fallback),
    set: (key, value) => {
      values.set(key, value);
      return true;
    },
  };
}

afterEach(() => {
  document.body.innerHTML = "";
  delete document.documentElement.dataset.winerOwner;
  delete document.documentElement.dataset.winerBeat;
  delete document.documentElement.dataset.winerConnected;
  delete document.documentElement.dataset.winerTeamPanel;
  document.documentElement.style.removeProperty("--winer-team-bottom");
  vi.useRealTimers();
});

describe("what the runes panel shows", () => {
  it("tells the player's own setup and the recommendation from a change of theirs", () => {
    expect(samePage(ELECTROCUTE, { ...ELECTROCUTE })).toBe(true);
    expect(samePage(ELECTROCUTE, COMET)).toBe(false);
    expect(samePage(null, ELECTROCUTE)).toBe(false);
    expect(sameSpells([4, 32], [32, 4])).toBe(true);
    expect(sameSpells([4, 32], [4, 6])).toBe(false);

    expect(currentIs(loadout())).toBe("recommended");
    // Snowball swapped for Ghost and kept as their own: theirs.
    const ghost = { runes: null, spells: [4, 6] as [number, number], savedAt: 1 };
    const mine = loadout({
      mine: ghost,
      current: { runes: ELECTROCUTE, page: "winer · 阿狸", spells: [6, 4] },
    });
    expect(currentIs(mine), "the runes from the recommendation, the spells their own").toBe("mine");
    expect(currentIs(loadout({ current: { runes: COMET, page: "我的", spells: [4, 32] } }))).toBe(
      "changed",
    );
    const other = loadoutBody(
      loadout({ current: { runes: COMET, page: "我的", spells: [4, 32] } }),
      "zh-CN",
      null,
      () => undefined,
    );
    expect(
      other.querySelector(".winer-loadout-tag")?.textContent,
      "neither theirs nor the recommendation, which may be the page they always had",
    ).toBe("其他");
    expect(currentIs(loadout({ current: { runes: null, page: "", spells: [4, 32] } }))).toBeNull();
  });

  it("names where the numbers come from and says the rates the source gives", () => {
    expect(sourceLine("tencent", "16.19", "zh-CN")).toBe("腾讯 101 16.19");
    expect(sourceLine("opGg", "16.19", "en")).toBe("OP.GG 16.19");
    expect(sourceLine("client", "", "zh-CN")).toBe("客户端推荐");
    expect(sourceLine(null, "", "zh-CN")).toBe("");
    expect(ratesLine({ ...NO_RATES, pick: 0.4718, win: 0.51 }, "zh-CN")).toBe(
      "选用 47.2% · 胜率 51%",
    );
    expect(ratesLine(NO_RATES, "zh-CN"), "the client's pages have none").toBe("");
  });

  it("draws now, the recommendation and the player's setup, every button off while one is under way", () => {
    const acted: string[] = [];
    const body = loadoutBody(loadout(), "zh-CN", null, (action) => acted.push(action.kind));
    expect(body.querySelector(".winer-loadout-now")?.textContent).toContain("= 推荐");
    const icon = body.querySelector<HTMLImageElement>("img.winer-loadout-icon");
    expect(icon?.getAttribute("src")).toBe("/lol-game-data/assets/v1/perk-images/electrocute.png");
    expect(body.querySelector(".winer-loadout-glyph"), "an id without an icon").not.toBeNull();
    expect(body.textContent).toContain("推荐 · 客户端推荐");
    const apply = [...body.querySelectorAll<HTMLButtonElement>('[data-action="applyRunes"]')];
    expect(apply).toHaveLength(2);
    expect(body.querySelectorAll('[data-action="applySpells"]')).toHaveLength(2);
    expect(body.textContent).toContain("未保存，锁定后用推荐");
    expect(body.querySelector('[data-action="applyMine"]'), "nothing of theirs yet").toBeNull();
    apply[1]?.click();
    (body.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    expect(acted).toEqual(["applyRunes", "saveMine"]);
    // It promises only what the rule does.
    const off = loadoutBody(loadout({ auto: "off" }), "zh-CN", null, () => undefined);
    expect(off.textContent).toContain("我的方案 · 未保存");
    expect(off.textContent, "the rule is off").not.toContain("锁定后");
    const keep = loadoutBody(loadout({ auto: "mine" }), "zh-CN", null, () => undefined);
    expect(keep.textContent).toContain("未保存，锁定后不自动更换");

    const busy = loadoutBody(
      loadout({ mine: { runes: COMET, spells: [4, 6], savedAt: 1 } }),
      "zh-CN",
      { state: "busy", action: { kind: "saveMine" } },
      () => acted.push("never"),
    );
    const buttons = [...busy.querySelectorAll<HTMLButtonElement>("button")];
    expect(buttons.every((button) => button.disabled)).toBe(true);
    expect(busy.querySelector('[data-action="saveMine"]')?.textContent).toBe("正在设置…");
    // Of several options, only the one asked for says so.
    const second = loadoutBody(
      loadout(),
      "zh-CN",
      { state: "busy", action: { kind: "applyRunes", index: 1 } },
      () => undefined,
    );
    expect(
      [...second.querySelectorAll('[data-action="applyRunes"]')].map(
        (button) => button.textContent,
      ),
    ).toEqual(["应用", "正在设置…"]);
    expect(busy.querySelector('[data-action="restoreRecommended"]')).not.toBeNull();
    expect(busy.textContent).toContain("已保存符文和召唤师技能");

    const loading = loadoutBody(
      loadout({ recommendation: { state: "loading" } }),
      "zh-CN",
      { state: "done", text: "选人已经变化，请再点一次", ok: false },
      () => undefined,
    );
    expect(loading.textContent).toContain("正在读取推荐");
    expect(loading.querySelector(".winer-loadout-said--bad")?.textContent).toBe(
      "选人已经变化，请再点一次",
    );
  });

  it("shows Hextech ARAM's spells, augment tiers and items, and no runes", () => {
    const rates = (pick: number, win: number) => ({ ...NO_RATES, pick, win });
    const view = loadout({
      mode: "hextech",
      takesRunes: false,
      current: { runes: null, page: "", spells: [4, 32] },
      recommendation: {
        state: "ready",
        recommendation: {
          runes: [],
          spells: [{ spells: [4, 32], rates: NO_RATES }],
          runesFrom: null,
          spellsFrom: "client",
          patch: "16.23",
          augments: [
            {
              id: 1201,
              rarity: "prismatic",
              tier: "S",
              pick: 0.124,
              description: "技能造成额外伤害",
            },
            { id: 1202, rarity: "gold", tier: "A", pick: 0.08, description: null },
          ],
          core: [{ items: [6655, 3157, 4645], rates: rates(0.21, 0.53) }],
          boots: [{ items: [3020], rates: rates(0.6, 0.51) }],
          buildFrom: "tencent",
        },
      },
      assets: [{ id: 1201, name: "法术巨像", icon: "/augment.png" }],
    });
    const acted: string[] = [];
    const body = loadoutBody(view, "zh-CN", null, (action) => acted.push(action.kind));
    expect(currentIs(view), "the spells alone tell").toBe("recommended");
    expect(body.querySelector(".winer-loadout-now")?.textContent).not.toContain("符文页");
    expect(body.querySelectorAll('[data-action="applyRunes"]')).toHaveLength(0);
    expect(body.textContent).toContain("召唤师技能 · 客户端推荐");
    expect(body.textContent).toContain("强化符文评级 · 腾讯 101 16.23");
    const augments = [...body.querySelectorAll<HTMLElement>(".winer-loadout-augment")];
    expect(
      augments.map((augment) => augment.className.split(" ")[1]),
      "each in a ring of its rarity, as the History page draws them",
    ).toEqual(["winer-loadout-augment--prismatic", "winer-loadout-augment--gold"]);
    expect(
      augments.map((augment) => augment.querySelector(".winer-loadout-tier")?.textContent),
    ).toEqual(["S", "A"]);
    expect(
      augments.map((augment) => augment.querySelector(".winer-loadout-augment-name")?.textContent),
      "the name under each icon",
    ).toEqual(["法术巨像", "#1202"]);
    const recommended =
      view.recommendation.state === "ready" ? view.recommendation.recommendation : null;
    const first = recommended?.augments[0];
    expect(first && augmentCard(first, "法术巨像", "zh-CN")).toEqual({
      name: "法术巨像",
      facts: "棱彩 · S 级 · 选取 12.4%",
      description: "技能造成额外伤害",
    });
    expect(
      JSON.stringify(first && augmentCard(first, "法术巨像", "zh-CN")),
      "no augment win rate, as Riot's policy says",
    ).not.toContain("胜率");

    // The hover card, beside the panel, from the pointer's place.
    const panel = new LoadoutPanel(
      document,
      "c-1",
      () => null,
      () => undefined,
      memoryStore(),
    );
    panel.show(view, "zh-CN");
    const cell = document.querySelector<HTMLElement>('[data-winer-augment="1201"]');
    panel.showTip(cell);
    const tip = document.querySelector<HTMLElement>(".winer-tip");
    expect(tip?.textContent).toBe("法术巨像棱彩 · S 级 · 选取 12.4%技能造成额外伤害");
    expect(tip?.className).toContain("winer-tip--prismatic");
    panel.showTip(null);
    expect(document.querySelector(".winer-tip")).toBeNull();
    panel.hide();
    expect(body.textContent).toContain("推荐出装");
    expect(body.textContent).toContain("选用 21% · 胜率 53%");
    (body.querySelector('[data-action="writeItemSet"]') as HTMLButtonElement).click();
    expect(acted).toEqual(["writeItemSet"]);
    expect(
      answerText(
        {
          type: "loadoutResult",
          requestId: 1,
          done: { runes: null, spells: false, itemSet: true },
          error: null,
        },
        "writeItemSet",
        "zh-CN",
      ).text,
    ).toBe("已写入装备方案，游戏内商店里能找到");
    expect(
      answerText(
        {
          type: "loadoutResult",
          requestId: 1,
          done: null,
          error: { code: "full", message: "full" },
        },
        "writeItemSet",
        "zh-CN",
      ).text,
    ).toBe("装备方案放满了：在客户端里删掉一些后再试");
  });

  it("says what came of a request, and that one made on an older view did nothing", () => {
    const result = (change: Partial<LoadoutResult>): LoadoutResult => ({
      type: "loadoutResult",
      requestId: 1,
      done: null,
      error: null,
      ...change,
    });
    expect(
      answerText(
        result({ done: { runes: null, spells: false, itemSet: false } }),
        "saveMine",
        "zh-CN",
      ),
    ).toEqual({
      text: "已保存为我的方案",
      ok: true,
    });
    expect(
      answerText(
        result({ done: { runes: "noPage", spells: true, itemSet: false } }),
        "applyMine",
        "zh-CN",
      ).ok,
    ).toBe(false);
    expect(
      answerText(result({ error: { code: "stale", message: "old" } }), "applyRunes", "zh-CN"),
    ).toEqual({ text: "选人已经变化，请再点一次", ok: false, stale: true });
    expect(
      answerText(result({ error: { code: "invalid", message: "no page" } }), "saveMine", "en").text,
    ).toBe("It did not work：no page");
  });
});

describe("the runes panel on the page", () => {
  it("asks for what a button says on the view it drew, and says what came of it", () => {
    const store = memoryStore();
    const requests: unknown[] = [];
    let redraws = 0;
    const panel = new LoadoutPanel(
      document,
      "c-1",
      (target, action) => requests.push([target, action]),
      () => (redraws += 1),
      store,
    );
    panel.show(loadout(), "zh-CN");
    const host = document.querySelector<HTMLElement>(
      `.winer-panel[data-winer-panel="${LOADOUT_PANEL}"]`,
    );
    expect(host?.dataset.winerContext).toBe("c-1");
    (host?.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    expect(requests).toEqual([[{ gameId: 9, championId: 103, revision: 4 }, { kind: "saveMine" }]]);
    expect(redraws).toBe(1);
    // One at a time: a second click while the first is under way asks nothing.
    panel.show(loadout(), "zh-CN");
    (host?.querySelector('[data-action="applyRunes"]') as HTMLButtonElement).click();
    expect(requests).toHaveLength(1);

    expect(
      panel.answer({ type: "loadoutResult", requestId: 9, done: null, error: null }),
      "not ours",
    ).toBe(false);
    expect(
      panel.answer({
        type: "loadoutResult",
        requestId: 1,
        done: { runes: null, spells: false, itemSet: false },
        error: null,
      }),
    ).toBe(true);
    panel.show(loadout({ revision: 5 }), "zh-CN");
    expect(host?.querySelector(".winer-loadout-said")?.textContent).toBe("已保存为我的方案");
    panel.hide();
    expect(document.querySelector(".winer-panel")).toBeNull();
  });

  it("gives up on an answer that never comes, and on a request of another champion's", () => {
    vi.useFakeTimers();
    const panel = new LoadoutPanel(
      document,
      "c-1",
      () => 7,
      () => undefined,
      memoryStore(),
    );
    panel.show(loadout(), "en");
    (document.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    vi.advanceTimersByTime(LOADOUT_TIMEOUT_MS);
    panel.show(loadout(), "en");
    expect(document.querySelector(".winer-loadout-said")?.textContent).toBe(
      "winer did not answer in time",
    );
    (
      document.querySelector(
        '[data-action="applyMine"], [data-action="saveMine"]',
      ) as HTMLButtonElement
    ).click();
    // The champion changed before the answer: what it did for the other one is not reported.
    panel.show(loadout({ championId: 22 }), "en");
    expect(panel.answer({ type: "loadoutResult", requestId: 7, done: null, error: null })).toBe(
      true,
    );
    panel.show(loadout({ championId: 22 }), "en");
    expect(document.querySelector(".winer-loadout-said")?.textContent).toBe("");
  });

  it("asks to click again when a request made on the champion before did nothing", () => {
    const panel = new LoadoutPanel(
      document,
      "c-1",
      () => 3,
      () => undefined,
      memoryStore(),
    );
    const stale = { type: "loadoutResult", requestId: 3, done: null } as const;
    const said = () => document.querySelector(".winer-loadout-said")?.textContent;
    // The new view first, then the answer.
    panel.show(loadout(), "zh-CN");
    (document.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    panel.show(loadout({ championId: 22, revision: 5 }), "zh-CN");
    expect(
      document.querySelector<HTMLButtonElement>('[data-action="saveMine"]')?.disabled,
      "the new champion's buttons are free",
    ).toBe(false);
    panel.answer({ ...stale, error: { code: "stale", message: "old" } });
    panel.show(loadout({ championId: 22, revision: 5 }), "zh-CN");
    expect(said()).toBe("选人已经变化，请再点一次");
    // One more change, nothing pressed: it goes.
    panel.show(loadout({ championId: 103, revision: 6 }), "zh-CN");
    expect(said()).toBe("");
    // The answer first, then the new view: the prompt stays for that view only.
    (document.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    panel.answer({ ...stale, error: { code: "stale", message: "old" } });
    panel.show(loadout({ championId: 22, revision: 7 }), "zh-CN");
    expect(said()).toBe("选人已经变化，请再点一次");
    panel.show(loadout({ championId: 103, revision: 8 }), "zh-CN");
    expect(said()).toBe("");
  });

  it("stays folded as the user left it, across a reload of the page", () => {
    const store = memoryStore();
    const panel = new LoadoutPanel(
      document,
      "c-1",
      () => null,
      () => undefined,
      store,
    );
    panel.show(loadout(), "zh-CN");
    const head = document.querySelector<HTMLButtonElement>(".winer-head");
    head?.click();
    expect(store.values.get(COLLAPSED_KEY)).toBe(true);
    expect(document.querySelector<HTMLElement>(".winer-panel")?.dataset.collapsed).toBe("true");
    panel.hide();
    const again = new LoadoutPanel(
      document,
      "c-2",
      () => null,
      () => undefined,
      store,
    );
    again.show(loadout(), "zh-CN");
    expect(document.querySelector<HTMLElement>(".winer-panel")?.dataset.collapsed).toBe("true");
    expect(head?.isConnected).toBe(false);
    // Unfolded by that one while this one's panel was gone: drawn again, it reads the store anew.
    store.values.set(COLLAPSED_KEY, false);
    again.hide();
    panel.show(loadout(), "zh-CN");
    expect(document.querySelector<HTMLElement>(".winer-panel")?.dataset.collapsed).toBe("false");
  });

  it("hands a click Pengu's root took over one of its buttons to that button", () => {
    const requests: unknown[] = [];
    const panel = new LoadoutPanel(
      document,
      "c-1",
      (_target, action) => requests.push(action),
      () => undefined,
      memoryStore(),
    );
    panel.show(loadout(), "zh-CN");
    const overlay = document.createElement("div");
    overlay.id = "pengu-root";
    document.body.append(overlay);
    const save = document.querySelector('[data-action="saveMine"]') as Element;
    Object.defineProperty(document, "elementsFromPoint", {
      configurable: true,
      value: () => [overlay, save, document.body],
    });
    const stop = interceptPlayerClicks(document, () => false);
    overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 10, clientY: 10 }));
    stop();
    Reflect.deleteProperty(document, "elementsFromPoint");
    expect(requests).toEqual([{ kind: "saveMine" }]);
  });
});

describe("the runes panel in the controller", () => {
  const settings = (loadoutPanel: boolean) =>
    ({
      general: { language: "zh-CN" },
      plugin: {
        teamPanel: false,
        benchNoCooldown: false,
        hidePromotions: false,
        friendStatus: false,
        lobbyPanel: false,
        historyInClient: false,
        loadoutPanel,
      },
    }) as unknown as Settings;
  const snapshot = (phase: Snapshot["phase"], view: LoadoutView | undefined): Snapshot => ({
    rev: 1,
    connection: { status: "connected", port: 1, platformId: "HN1" },
    me: null,
    phase,
    champSelect: {
      gameId: 9,
      queueId: 2400,
      timer: { phase: "BAN_PICK", endsAt: 0, totalMs: 0 },
      myTeam: [],
      theirTeam: [],
      myBans: [],
      theirBans: [],
      benchEnabled: true,
      bench: [],
      rerollsRemaining: 0,
      callout: [],
      side: null,
      recommendations: [],
      loadout: view,
    },
    game: null,
    friends: null,
    lobby: null,
  });
  const panel = () => document.querySelector(`.winer-panel[data-winer-panel="${LOADOUT_PANEL}"]`);

  it("draws it in champ select while its option is on and the view has runes, and only then", () => {
    const controller = new Controller(document);
    controller.state.hello(snapshot("ChampSelect", loadout()), settings(true));
    controller.render();
    expect(panel()).not.toBeNull();
    controller.state.hello(snapshot("ChampSelect", loadout()), settings(false));
    controller.render();
    expect(panel(), "switched off").toBeNull();
    controller.state.hello(snapshot("ChampSelect", undefined), settings(true));
    controller.render();
    expect(panel(), "Arena, or no champion yet").toBeNull();
    controller.state.hello(snapshot("InProgress", loadout()), settings(true));
    controller.render();
    expect(panel(), "the game started").toBeNull();
  });

  it("takes away a runes panel the page's other context left once there is nothing to show", () => {
    const controller = new Controller(document);
    const left = document.createElement("section");
    left.className = "winer-panel";
    left.dataset.winerPanel = LOADOUT_PANEL;
    left.dataset.winerContext = "the-other-context";
    document.body.append(left);
    controller.state.hello(snapshot("Lobby", undefined), settings(true));
    controller.render();
    expect(panel()).toBeNull();
  });

  it("keeps the runes panel clear of the team panel on the same edge", () => {
    const controller = new Controller(document);
    const both = settings(true);
    both.plugin.teamPanel = true;
    // No party rows on this page: the team panel shows at the top right.
    controller.state.hello(snapshot("ChampSelect", loadout()), both);
    controller.render();
    const root = document.documentElement;
    expect(root.dataset.winerTeamPanel).toBe("true");
    expect(root.style.getPropertyValue("--winer-team-bottom")).toBe("0px");
    controller.state.hello(snapshot("Lobby", undefined), both);
    controller.render();
    expect(root.dataset.winerTeamPanel).toBeUndefined();
    expect(root.style.getPropertyValue("--winer-team-bottom")).toBe("");
  });

  it("asks winer only from the context that draws", () => {
    const controller = new Controller(document);
    const sent = vi.spyOn(controller.bridge, "loadout").mockReturnValue(1);
    controller.state.hello(snapshot("ChampSelect", loadout()), settings(true));
    controller.render();
    (panel()?.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    expect(sent).toHaveBeenCalledWith(
      { gameId: 9, championId: 103, revision: 4 },
      { kind: "saveMine" },
    );
    // Another context took the drawing role meanwhile: this one asks nothing.
    document.documentElement.dataset.winerOwner = "someone-else";
    controller.loadout.answer({ type: "loadoutResult", requestId: 1, done: null, error: null });
    controller.loadout.show(loadout({ revision: 5 }), "zh-CN");
    (panel()?.querySelector('[data-action="saveMine"]') as HTMLButtonElement).click();
    expect(sent).toHaveBeenCalledTimes(1);
  });
});
