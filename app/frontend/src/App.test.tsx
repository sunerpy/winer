// The whole window against the demo client: every page renders without tripping its error
// boundary, and the settings that restyle or re-word the window take effect.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ChampSelectView, GameView, Mode, Seat, Snapshot } from "@winer/shared";

import { App } from "./App";
import type { Backend, CommandName } from "./lib/backend";
import { demoBackend } from "./lib/demo";
import { AppStore, EMPTY_SNAPSHOT, StoreContext } from "./lib/store";
import { LivePage } from "./pages/Live";
import { ShellContext } from "./shell/navigation";
import { en } from "./lib/i18n/en";
import { zhCN } from "./lib/i18n/zh-CN";

async function renderApp(backend: Backend = demoBackend()) {
  const user = userEvent.setup();
  render(<App backend={backend} />);
  const nav = await screen.findByRole("navigation", { name: zhCN["nav.label"] });
  return { user, nav };
}

/** The demo client, with some commands answered differently. */
function demoWith(answers: Partial<Record<CommandName, (args: never) => unknown>>): Backend {
  const backend = demoBackend();
  const call = backend.call.bind(backend) as (
    command: CommandName,
    ...args: unknown[]
  ) => Promise<unknown>;
  return {
    ...backend,
    call: ((command: CommandName, ...args: unknown[]) => {
      const answer = answers[command];
      return answer
        ? Promise.resolve().then(() => answer(args[0] as never))
        : call(command, ...args);
    }) as Backend["call"],
  };
}

describe("App", () => {
  it("opens every page from the sidebar", async () => {
    const { user, nav } = await renderApp();
    const pages = [
      ["nav.live", "live.title"],
      ["nav.history", "history.title"],
      ["nav.automation", "nav.automation"],
      ["nav.tools", "tools.title"],
      ["nav.plugin", "plugin.title"],
      ["nav.overview", "overview.title"],
    ] as const;
    for (const [entry, title] of pages) {
      await user.click(within(nav).getByRole("button", { name: zhCN[entry] }));
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN[title]);
      expect(screen.queryByText(zhCN["error.title"])).toBeNull();
    }
  });

  it("opens each page at its top, not where the last one was scrolled to", async () => {
    const { user, nav } = await renderApp();
    const main = screen.getByRole("main");
    main.scrollTop = 600;
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(main.scrollTop).toBe(0);
  });

  it("pages through the history, filling a filtered page from as many requests as it takes", async () => {
    localStorage.removeItem("winer.history.pageSize");
    const { user, nav } = await renderApp();
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect(await screen.findByText("第 1–10 场")).toBeInTheDocument();
    const pages = screen.getByRole("navigation", { name: zhCN["history.pages"] });
    expect(within(pages).getByRole("button", { name: "上一页" })).toBeDisabled();

    await user.click(within(pages).getByRole("button", { name: "下一页" }));
    expect(screen.getByText("第 11–20 场")).toBeInTheDocument();
    expect(within(pages).getByRole("button", { name: "第 2 页" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    // The demo player has sixty games and one request brings fifty.
    await user.click(screen.getByRole("radio", { name: "25" }));
    expect(screen.getByText("第 1–25 场"), "the first game on screen stays").toBeInTheDocument();
    expect(localStorage.getItem("winer.history.pageSize")).toBe("25");
    await user.click(within(pages).getByRole("button", { name: "下一页" }));
    expect(screen.getByText("第 26–50 场")).toBeInTheDocument();
    expect(
      within(pages).queryByRole("button", { name: "第 3 页" }),
      "a page not read yet has no number",
    ).toBeNull();
    await user.click(within(pages).getByRole("button", { name: "下一页" }));
    expect(await screen.findByText("第 51–60 场 · 共 60 场")).toBeInTheDocument();
    expect(within(pages).getByRole("button", { name: "下一页" })).toBeDisabled();

    await user.click(screen.getByRole("radio", { name: zhCN["history.aram"] }));
    expect(
      screen.getByText(/^第 1–\d+ 场 · 共 \d+ 场$/),
      "a filter starts at its first page",
    ).toBeInTheDocument();
  });

  it("offers only the statuses the client takes, and says when it refused one", async () => {
    const { user, nav } = await renderApp(
      demoWith({
        set_availability: ({ availability }: { availability: string }) => {
          if (availability === "offline")
            throw new Error("the client kept chat instead of offline");
          return null;
        },
      }),
    );
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.tools"] }));
    const statuses = await screen.findByRole("radiogroup", { name: zhCN["tools.presence"] });
    expect(
      within(statuses)
        .getAllByRole("radio")
        .map((radio) => radio.textContent),
    ).toEqual(["在线", "离开", "隐身"]);
    await waitFor(() =>
      expect(within(statuses).getByRole("radio", { name: "在线" })).toBeChecked(),
    );

    await user.click(within(statuses).getByRole("radio", { name: "隐身" }));
    expect(await screen.findByText(zhCN["tools.statusRefused"])).toBeInTheDocument();
    expect(
      within(statuses).getByRole("radio", { name: "在线" }),
      "the refused choice goes back",
    ).toBeChecked();
  });

  it("names a status the client set itself, with no option chosen", async () => {
    const { user, nav } = await renderApp(
      demoWith({ get_presence: () => ({ availability: "dnd", statusMessage: "" }) }),
    );
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.tools"] }));
    expect(await screen.findByText("当前：游戏中")).toBeInTheDocument();
    const statuses = screen.getByRole("radiogroup", { name: zhCN["tools.presence"] });
    expect(within(statuses).queryByRole("radio", { checked: true })).toBeNull();
  });

  it("scopes each automation to the modes it can act in and shows one mode's at a time", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const groups = await screen.findAllByRole("group", { name: zhCN["auto.scope"] });
    const benchModes = groups.find(
      (group) => within(group).queryByRole("button", { name: "匹配" }) === null,
    ) as HTMLElement;
    expect(
      within(benchModes)
        .getAllByRole("button")
        .map((chip) => chip.textContent),
    ).toEqual(["极地大乱斗", "海克斯大乱斗"]);
    await user.click(within(benchModes).getByRole("button", { name: "海克斯大乱斗" }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            scopes: expect.objectContaining({ bench: ["aram"] }),
          }),
        }),
      }),
    );
    expect(within(benchModes).getByRole("button", { name: "海克斯大乱斗" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );

    const byMode = screen.getByRole("radiogroup", { name: zhCN["auto.byMode"] });
    await user.click(within(byMode).getByRole("radio", { name: "极地大乱斗" }));
    expect(
      screen.queryByRole("switch", { name: zhCN["auto.pick"] }),
      "no picks in ARAM",
    ).toBeNull();
    expect(screen.getByRole("switch", { name: zhCN["auto.bench"] })).toBeInTheDocument();
    expect(screen.getByText(/^在极地大乱斗中会：/)).toHaveTextContent(
      "自动选择英雄和自动禁用英雄在极地大乱斗中用不上",
    );
  });

  it("opens a game on the player's own team, its side named, and switches to the other", async () => {
    const seat = (puuid: string, isSelf = false): Seat => ({
      puuid,
      name: { gameName: puuid, tagLine: "1" },
      championId: 1,
      intent: false,
      position: null,
      spells: [4, 14],
      isSelf,
      premade: null,
      rating: null,
      stats: { state: "loading" },
    });
    const game: GameView = {
      gameId: 1,
      queueId: 2400,
      teams: [[seat("blue-1")], [seat("me", true), seat("red-2")]],
      sides: true,
    };
    const store = new AppStore(demoBackend());
    store.live.set({
      ...EMPTY_SNAPSHOT,
      connection: { status: "connected", port: 1, platformId: "NJ100" },
      phase: "InProgress",
      game,
    });
    const user = userEvent.setup();
    render(
      <StoreContext value={store}>
        <ShellContext value={{ route: { page: "live" }, navigate: vi.fn(), openSettings: vi.fn() }}>
          <LivePage />
        </ShellContext>
      </StoreContext>,
    );
    const teams = screen.getByRole("radiogroup", { name: zhCN["live.board"] });
    expect(within(teams).getByRole("radio", { name: "我方 · 红色方" })).toBeChecked();
    expect(screen.getByRole("button", { name: "查看 me#1 的战绩" })).toBeInTheDocument();
    await user.click(within(teams).getByRole("radio", { name: "敌方 · 蓝色方" }));
    expect(screen.getByRole("button", { name: "查看 blue-1#1 的战绩" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看 me#1 的战绩" })).toBeNull();
  });

  it("marks the callout as sent by itself only where its modes include this game", async () => {
    const settings = await demoBackend().call("get_settings");
    const scoped = (callout: Mode[]) =>
      demoWith({
        get_settings: () => ({
          ...settings,
          automation: {
            ...settings.automation,
            callout: { ...settings.automation.callout, auto: true },
            scopes: { ...settings.automation.scopes, callout },
          },
        }),
      });
    // The demo champ select is Hextech ARAM.
    const { user } = await renderApp(scoped(["ranked", "normal"]));
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(screen.getByText(/^峡谷通天代：阿狸/)).toBeInTheDocument();
    expect(screen.queryByText(zhCN["live.calloutAuto"])).toBeNull();
    cleanup();

    const again = await renderApp(scoped(["hextech"]));
    await again.user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(await screen.findByText(zhCN["live.calloutAuto"])).toBeInTheDocument();
  });

  it("shows the champ select the demo client is in", async () => {
    const { user } = await renderApp();
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["live.title"]);
    expect(screen.queryByText(zhCN["error.title"])).toBeNull();
  });

  it("sends the callout and swaps from the bench in an ARAM champ select", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = userEvent.setup();
    render(<App backend={backend} />);
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));

    expect(
      screen.getByText("【蓝色方】winer 战绩鉴定"),
      "the side and winer's name lead the callout",
    ).toBeInTheDocument();
    expect(screen.getByText(/^峡谷通天代：阿狸/)).toBeInTheDocument();
    expect(screen.getByText(/^纯正牛马：锤石/)).toBeInTheDocument();
    expect(
      screen.getByText(/^峡谷通天代：阿狸/),
      "the title and the quip ride along",
    ).toHaveTextContent("评分7.4「版本答案」，对面五个人准备举报代练");
    await user.click(screen.getByRole("button", { name: zhCN["live.sendTeam"] }));
    expect(call).toHaveBeenCalledWith("send_callout", { audience: "team" });
    expect(await screen.findByText("已发送 6 条喊话")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /换成 拉克丝/ }));
    expect(call).toHaveBeenCalledWith("bench_swap", { championId: 99 });
    // One bench action at a time: the reroll waits for the swap to come back.
    const reroll = screen.getByRole("button", { name: "重随（剩 1 次）" });
    await waitFor(() => expect(reroll).toBeEnabled());
    await user.click(reroll);
    expect(call).toHaveBeenCalledWith("reroll");
  });

  it("sets up the in-client features by itself, and turns them off and on", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.plugin"] }));
    expect(
      await screen.findByText("winer 自带的 1.1.6，已为当前客户端激活"),
      "the loader winer ships is set up with nothing to click",
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /安装插件/ })).toBeNull();

    await user.click(screen.getByRole("button", { name: zhCN["plugin.disable"] }));
    expect(call).toHaveBeenCalledWith("disable_plugin");
    expect(await screen.findByText(zhCN["plugin.loaderOff"])).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: zhCN["plugin.enable"] }));
    expect(call).toHaveBeenCalledWith("enable_plugin");
    expect(await screen.findByRole("button", { name: zhCN["plugin.disable"] })).toBeInTheDocument();
  });

  it("previews the callout as it is written: opening line, scheme and own tiers", async () => {
    const { user, nav } = await renderApp();
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^峡谷通天代：阿狸/)).toBeInTheDocument();
    expect(screen.getByText(/^纯正牛马：阿狸/)).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: zhCN["auto.header"] }), "开局分析{Enter}");
    expect(await screen.findByText("【蓝色方】winer 战绩鉴定 · 开局分析")).toBeInTheDocument();

    // The scheme lives in Settings › Rating; the callout's own row only leads there.
    await user.click(screen.getByRole("button", { name: zhCN["auto.schemeEdit"] }));
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    expect(within(dialog).getByRole("tab", { name: zhCN["settings.rating"] })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(within(dialog).getByRole("radio", { name: /^峡谷食物链/ }));
    expect(await screen.findByText(/^峡谷之王：阿狸/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("radio", { name: /^自定义/ }));
    await user.type(within(dialog).getByRole("textbox", { name: "第 1 档" }), "大腿{Enter}");
    await user.type(within(dialog).getByRole("textbox", { name: "第 2 档" }), "挂件{Enter}");
    expect(await screen.findByText(/^挂件：阿狸/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["common.close"] }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText(zhCN["rating.scheme.custom"])).toBeInTheDocument();
  });

  it("explains the rating in settings, switches the titles and opens the documentation", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^峡谷通天代：阿狸/)).toHaveTextContent("「版本答案」");

    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.rating"] }));
    expect(within(dialog).getByRole("radio", { name: /^峡谷五档/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(within(dialog).getByText(zhCN["rating.basis.form"])).toBeInTheDocument();
    expect(within(dialog).getByText(zhCN["rating.basis.tiers"])).toBeInTheDocument();

    await user.click(within(dialog).getByRole("switch", { name: zhCN["rating.titles"] }));
    await waitFor(() =>
      expect(screen.getByText(/^峡谷通天代：阿狸/)).not.toHaveTextContent("「版本答案」"),
    );
    expect(call).toHaveBeenCalledWith(
      "preview_callout",
      expect.objectContaining({ general: expect.objectContaining({ titles: false }) }),
    );

    await user.click(within(dialog).getByRole("button", { name: zhCN["rating.docs"] }));
    expect(call).toHaveBeenCalledWith("open_docs", { page: "rating" });
  });

  it("applies the theme and the language chosen in settings", async () => {
    const { user } = await renderApp();
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });

    await user.click(within(dialog).getByRole("radio", { name: zhCN["settings.theme.light"] }));
    expect(document.documentElement.dataset.theme).toBe("light");

    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.general"] }));
    await user.click(within(dialog).getByRole("button", { name: "English" }));
    expect(await screen.findByRole("navigation", { name: en["nav.label"] })).toBeInTheDocument();
  });
});

describe("loadout", () => {
  const seat = (championId: number, isSelf: boolean, position: Seat["position"]): Seat => ({
    puuid: isSelf ? "demo-me" : `p-${championId}`,
    name: { gameName: isSelf ? "暗夜里的光" : `p${championId}`, tagLine: "1" },
    championId,
    intent: false,
    position,
    spells: [4, 12],
    isSelf,
    premade: null,
    rating: null,
    stats: { state: "loading" },
  });
  const connected = { status: "connected", port: 1, platformId: "NJ100" } as const;

  /** The demo client in a ranked champ select, the local player on Ahri in the middle lane. */
  const ranked: ChampSelectView = {
    gameId: 2,
    queueId: 420,
    timer: { phase: "BAN_PICK", endsAt: 0, totalMs: 0 },
    myTeam: [seat(103, true, "middle"), seat(64, false, "jungle")],
    theirTeam: [],
    myBans: [],
    theirBans: [],
    benchEnabled: false,
    bench: [],
    rerollsRemaining: 0,
    callout: [],
    side: "blue",
  };
  const live = (snapshot: Partial<Snapshot>): Snapshot => ({
    ...EMPTY_SNAPSHOT,
    rev: 1,
    connection: connected,
    ...snapshot,
  });

  async function openLive(backend: Backend) {
    const rendered = await renderApp(backend);
    await rendered.user.click(within(rendered.nav).getByRole("button", { name: zhCN["nav.live"] }));
    return rendered;
  }

  it("shows the build of the champion in champ select, Hextech ARAM's augments by rarity", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openLive(backend);
    expect(await screen.findByText("数据：腾讯掌上英雄联盟")).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("get_build", {
      championId: 103,
      mode: "hextech",
      lane: null,
    });
    expect(screen.getByText("九尾妖狐 · 阿狸")).toBeInTheDocument();
    const tabs = screen.getByRole("radiogroup", { name: zhCN["loadout.tabs"] });
    expect(
      within(tabs)
        .getAllByRole("radio")
        .map((tab) => tab.textContent),
      "Tencent's Hextech numbers have no runes or spells",
    ).toEqual(["出装", "技能加点", "强化符文"]);
    expect(screen.queryByRole("radiogroup", { name: zhCN["loadout.lane"] })).toBeNull();

    await user.click(within(tabs).getByRole("radio", { name: "强化符文" }));
    const prismatic = screen.getByRole("region", { name: "棱彩" });
    expect(within(prismatic).getByText("连拨击锤")).toBeInTheDocument();
    expect(
      await within(prismatic).findByText(
        "你的终极技能已被封印。获得35%技能伤害、治疗效果、护盾和70技能急速。",
      ),
      "what it does, from ARAM.GG",
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("region").map((region) => region.getAttribute("aria-label")),
    ).toEqual(["棱彩", "金色", "银色"]);
    await user.type(screen.getByRole("textbox", { name: zhCN["loadout.augmentFilter"] }), "无尽");
    expect(screen.getByText("升级：无尽之刃")).toBeInTheDocument();
    expect(screen.queryByText("连拨击锤")).toBeNull();

    await user.click(within(tabs).getByRole("radio", { name: "出装" }));
    expect(screen.getByRole("region", { name: zhCN["loadout.core"] })).toHaveTextContent("20.2%");
    expect(screen.getAllByRole("img", { name: "卢登的伙伴" }).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: zhCN["loadout.writeItemSet"] }));
    expect(call).toHaveBeenCalledWith("write_item_set", {
      championId: 103,
      mode: "hextech",
      lane: null,
    });
    expect(await screen.findByText(zhCN["loadout.itemSetWritten"])).toBeInTheDocument();
  });

  it("sets up runes and spells from a ranked champ select, by lane, and names the matchups", async () => {
    let refuse = false;
    const backend = demoWith({
      get_snapshot: () => live({ phase: "ChampSelect", champSelect: ranked }),
      apply_runes: () => (refuse ? "noPage" : "written"),
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await openLive(backend);
    expect(await screen.findByText("数据：腾讯 101 · 16.19")).toBeInTheDocument();
    expect(screen.getByText("样本 1,241,522 场")).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("get_build", {
      championId: 103,
      mode: "ranked",
      lane: "middle",
    });
    const lanes = screen.getByRole("radiogroup", { name: zhCN["loadout.lane"] });
    expect(within(lanes).getByRole("radio", { name: "中单" })).toBeChecked();
    const tabs = screen.getByRole("radiogroup", { name: zhCN["loadout.tabs"] });

    await user.click(within(tabs).getByRole("radio", { name: "符文" }));
    expect(screen.getAllByRole("img", { name: "电刑" }).length).toBeGreaterThan(0);
    const [first, second] = screen.getAllByRole("button", { name: zhCN["loadout.applyRunes"] });
    await user.click(first as HTMLElement);
    expect(call).toHaveBeenCalledWith("apply_runes", {
      championId: 103,
      page: {
        primaryStyle: 8100,
        subStyle: 8200,
        perks: [8112, 8126, 8138, 8135, 8210, 8237, 5008, 5008, 5001],
      },
    });
    expect(await screen.findByText(zhCN["loadout.runesWritten"])).toBeInTheDocument();
    refuse = true;
    await user.click(second as HTMLElement);
    expect(await screen.findByText(zhCN["loadout.runesNoPage"])).toBeInTheDocument();
    expect(screen.queryByText(zhCN["loadout.runesWritten"]), "one outcome at a time").toBeNull();

    await user.click(within(tabs).getByRole("radio", { name: "召唤师技能" }));
    await user.click(
      screen.getAllByRole("button", { name: zhCN["loadout.applySpells"] })[0] as HTMLElement,
    );
    expect(call).toHaveBeenCalledWith("apply_spells", { spells: [4, 14] });
    expect(await screen.findByText(zhCN["loadout.spellsApplied"])).toBeInTheDocument();

    await user.click(within(tabs).getByRole("radio", { name: "对位" }));
    expect(screen.getByRole("region", { name: "优势对位" })).toHaveTextContent("探险家54.1%");
    expect(screen.getByRole("region", { name: "劣势对位" })).toHaveTextContent("影流之主44.0%");

    await user.click(within(lanes).getByRole("radio", { name: "上单" }));
    expect(call).toHaveBeenCalledWith("get_build", {
      championId: 103,
      mode: "ranked",
      lane: "top",
    });
    await waitFor(() => expect(within(lanes).getByRole("radio", { name: "上单" })).toBeChecked());
  });

  it("opens on the augments during a Hextech ARAM game, where they are picked", async () => {
    const game: GameView = {
      gameId: 3,
      queueId: 2400,
      teams: [[seat(103, true, null), seat(22, false, null)], [seat(99, false, null)]],
      sides: true,
    };
    await openLive(demoWith({ get_snapshot: () => live({ phase: "InProgress", game }) }));
    const tabs = await screen.findByRole("radiogroup", { name: zhCN["loadout.tabs"] });
    await waitFor(() =>
      expect(within(tabs).getByRole("radio", { name: "强化符文" })).toBeChecked(),
    );
    expect(await screen.findByRole("region", { name: "棱彩" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: zhCN["loadout.applyRunes"] }),
      "a running game's runes are set",
    ).toBeNull();
  });

  it("looks a champion up outside a game, Arena by its placements", async () => {
    const backend = demoWith({ get_snapshot: () => live({ phase: "None" }) });
    const call = vi.spyOn(backend, "call");
    const { user } = await openLive(backend);
    expect(await screen.findByText(zhCN["live.idleTitle"])).toBeInTheDocument();
    expect(screen.getByText(zhCN["loadout.lookupHint"])).toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("get_build", expect.anything());

    const choose = screen.getByRole("button", { name: zhCN["loadout.choose"] });
    await waitFor(() => expect(choose, "the catalog names the champions").toBeEnabled());
    await user.click(choose);
    const picker = screen.getByRole("dialog", { name: zhCN["loadout.choose"] });
    await user.type(within(picker).getByRole("textbox", { name: zhCN["auto.search"] }), "艾希");
    await user.click(within(picker).getByRole("button", { name: /艾希/ }));
    expect(screen.queryByRole("dialog", { name: zhCN["loadout.choose"] })).toBeNull();
    expect(await screen.findByText("寒冰射手 · 艾希")).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("get_build", { championId: 22, mode: "ranked", lane: null });
    const lanes = await screen.findByRole("radiogroup", { name: zhCN["loadout.lane"] });
    await waitFor(() =>
      expect(
        within(lanes).getByRole("radio", { name: "中单" }),
        "the lane the source chose",
      ).toBeChecked(),
    );

    const modes = screen.getByRole("radiogroup", { name: zhCN["loadout.mode"] });
    expect(
      within(modes)
        .getAllByRole("radio")
        .map((mode) => mode.textContent),
    ).toEqual(["召唤师峡谷", "极地大乱斗", "海克斯大乱斗", "斗魂竞技场"]);
    await user.click(within(modes).getByRole("radio", { name: "斗魂竞技场" }));
    expect(call).toHaveBeenCalledWith("get_build", { championId: 22, mode: "arena", lane: null });
    const tabs = await screen.findByRole("radiogroup", { name: zhCN["loadout.tabs"] });
    await user.click(await within(tabs).findByRole("radio", { name: "强化符文" }));
    expect(await screen.findByText("平均第 4.14 名")).toBeInTheDocument();
    expect(screen.getByText("第一名 20.1%")).toBeInTheDocument();
    expect(screen.getByText(zhCN["loadout.placementHint"])).toBeInTheDocument();
  });

  it("says when the numbers did not come, and tries again", async () => {
    let failures = 1;
    const demo = demoBackend();
    const backend = demoWith({
      get_snapshot: () => live({ phase: "ChampSelect", champSelect: ranked }),
      get_build: (args: Parameters<Backend["call"]>[1]) => {
        if (failures-- > 0) throw { code: "client", message: "lol-api-champion.op.gg: timed out" };
        return demo.call("get_build", args as never);
      },
    });
    const { user } = await openLive(backend);
    expect(await screen.findByText(zhCN["loadout.failed"])).toBeInTheDocument();
    expect(screen.getByText("lol-api-champion.op.gg: timed out")).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: zhCN["loadout.tabs"] })).toBeNull();
    await user.click(screen.getByRole("button", { name: zhCN["common.retry"] }));
    expect(await screen.findByText("数据：腾讯 101 · 16.19")).toBeInTheDocument();
  });

  it("has no panel and fetches nothing while builds are off", async () => {
    const settings = await demoBackend().call("get_settings");
    const backend = demoWith({
      get_settings: () => ({ ...settings, builds: { ...settings.builds, enabled: false } }),
    });
    const call = vi.spyOn(backend, "call");
    await openLive(backend);
    expect(await screen.findByText(/^峡谷通天代：阿狸/)).toBeInTheDocument();
    expect(screen.queryByText(zhCN["loadout.panel"])).toBeNull();
    expect(call).not.toHaveBeenCalledWith("get_build", expect.anything());
  });

  it("scopes the rune and spell memory and the item sets, and takes back what winer wrote", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));

    await user.click(await screen.findByRole("switch", { name: zhCN["loadout.rule"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            loadout: { enabled: true, recommended: true },
          }),
        }),
      }),
    );
    const scopes = screen.getAllByRole("group", { name: zhCN["auto.scope"] });
    const chips = scopes.map((group) =>
      within(group)
        .getAllByRole("button")
        .map((chip) => chip.textContent),
    );
    expect(chips, "no rune page in Arena; no shop of its own in other modes").toEqual(
      expect.arrayContaining([
        ["排位", "匹配", "极地大乱斗", "海克斯大乱斗", "其他模式"],
        ["排位", "匹配", "极地大乱斗", "海克斯大乱斗", "斗魂竞技场"],
      ]),
    );

    expect(await screen.findByText("已记住 3 套")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: zhCN["loadout.forget"] }));
    expect(call).toHaveBeenCalledWith("clear_loadouts");
    expect(await screen.findByText("已记住 0 套")).toBeInTheDocument();

    expect(screen.getByText(zhCN["loadout.experimental"])).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: zhCN["loadout.itemSets"] })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: zhCN["loadout.clearItemSets"] }));
    expect(call).toHaveBeenCalledWith("clear_item_sets");
    expect(await screen.findByText("已清除 0 个 winer 装备方案")).toBeInTheDocument();

    const byMode = screen.getByRole("radiogroup", { name: zhCN["auto.byMode"] });
    await user.click(within(byMode).getByRole("radio", { name: "斗魂竞技场" }));
    expect(screen.queryByRole("switch", { name: zhCN["loadout.rule"] })).toBeNull();
    expect(screen.getByRole("switch", { name: zhCN["loadout.itemSets"] })).toBeInTheDocument();
  });

  it("switches builds and their Summoner's Rift source in settings", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.general"] }));
    const sources = within(dialog).getByRole("radiogroup", { name: zhCN["loadout.riftSource"] });
    await user.click(within(sources).getByRole("radio", { name: "OP.GG" }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({ builds: { enabled: true, riftSource: "opGg" } }),
      }),
    );
    await user.click(within(dialog).getByRole("switch", { name: zhCN["loadout.builds"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({ builds: { enabled: false, riftSource: "opGg" } }),
      }),
    );
    expect(within(sources).getByRole("radio", { name: "腾讯 101" })).toBeDisabled();
  });
});
