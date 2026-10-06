// The whole window against the demo client: every page renders without tripping its error
// boundary, and the settings that restyle or re-word the window take effect.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { GameView, Mode, Seat } from "@winer/shared";

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
    ).toEqual(["在线", "离开", "手机在线", "隐身"]);
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

describe("Tools: profile", () => {
  async function openTools(backend: Backend = demoBackend()) {
    const rendered = await renderApp(backend);
    await rendered.user.click(
      within(rendered.nav).getByRole("button", { name: zhCN["nav.tools"] }),
    );
    return rendered;
  }

  /** The demo client outside any game, on the home screen. */
  async function idleDemo(answers: Parameters<typeof demoWith>[0] = {}) {
    const snapshot = await demoBackend().call("get_snapshot");
    return demoWith({
      get_snapshot: () => ({ ...snapshot, phase: "None", champSelect: null }),
      ...answers,
    });
  }

  it("remembers the status and the message, and keeps them in step with later changes", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const statuses = await screen.findByRole("radiogroup", { name: zhCN["tools.presence"] });
    await waitFor(() =>
      expect(within(statuses).getByRole("radio", { name: "在线" })).toBeChecked(),
    );
    const remember = screen.getByRole("switch", { name: zhCN["profile.remember"] });
    expect(remember, "off until switched on").toHaveAttribute("aria-checked", "false");

    await user.click(remember);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          profile: expect.objectContaining({
            presence: { remember: true, availability: "chat", statusMessage: "今晚上分" },
          }),
        }),
      }),
    );

    await user.click(within(statuses).getByRole("radio", { name: "手机在线" }));
    expect(call).toHaveBeenCalledWith("set_availability", { availability: "mobile" });
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          profile: expect.objectContaining({
            presence: expect.objectContaining({ remember: true, availability: "mobile" }),
          }),
        }),
      }),
    );
  });

  it("sets the background from every skin and says when the client keeps the old one", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    await user.type(
      await screen.findByRole("textbox", { name: zhCN["profile.background.search"] }),
      "ahri",
    );
    // Found by the champion's English name: the base skin and three more, owned or not.
    expect(await screen.findByText("4 款")).toBeInTheDocument();
    const unowned = screen.getByRole("button", { name: "K/DA 阿狸 · 未拥有" });
    expect(screen.getByRole("button", { name: "灵魂莲华 阿狸" })).toBeInTheDocument();

    await user.click(unowned);
    expect(unowned).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: zhCN["profile.background.apply"] }));
    expect(call).toHaveBeenCalledWith("set_profile_background", { skinId: 103002 });
    expect(
      await screen.findByText(
        "客户端没有换成 K/DA 阿狸，背景仍是 灵魂莲华 阿狸。未拥有的皮肤可能不被接受。",
      ),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "九尾妖狐" }));
    await user.click(screen.getByRole("button", { name: zhCN["profile.background.apply"] }));
    expect(await screen.findByText("已把 九尾妖狐 设为生涯背景")).toBeInTheDocument();
    expect(screen.queryByText(/^客户端没有换成/)).toBeNull();

    await user.click(screen.getByRole("switch", { name: zhCN["profile.background.owned"] }));
    expect(screen.queryByRole("button", { name: "K/DA 阿狸 · 未拥有" })).toBeNull();
    expect(screen.getByText("2 款")).toBeInTheDocument();
  });

  it("puts challenge tokens in slot order with a title, and shows what the client reports", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    await screen.findByRole("button", { name: "第 1 个徽章: 闪电战" });
    expect(screen.getByRole("button", { name: "第 2 个徽章: 射手收藏家" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "清空第 1 个徽章" }));
    expect(screen.getByRole("button", { name: "第 1 个徽章: 选择徽章" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "第 3 个徽章: 选择徽章" }));
    const picker = screen.getByRole("dialog", { name: "第 3 个徽章" });
    expect(
      within(picker).queryByRole("button", { name: /^射手收藏家/ }),
      "a token shows once",
    ).toBeNull();
    await user.click(within(picker).getByRole("button", { name: /^雪球大战/ }));

    await user.click(screen.getByRole("button", { name: /^日光浴恶魔/ }));
    await user.click(
      within(
        screen.getByRole("dialog", { name: zhCN["profile.challenges.chooseTitle"] }),
      ).getByRole("button", { name: "雪球狙神" }),
    );
    await user.click(screen.getByRole("button", { name: zhCN["profile.challenges.apply"] }));
    expect(call).toHaveBeenCalledWith("set_challenge_profile", {
      challengeIds: [505005, 101203],
      titleId: 10120303,
    });
    expect(await screen.findByText(zhCN["profile.challenges.done"])).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "第 1 个徽章: 射手收藏家" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "第 2 个徽章: 雪球大战" })).toBeInTheDocument();
  });

  it("says so when the client takes only part of the tokens", async () => {
    const before = await demoBackend().call("get_challenge_profile");
    const { user } = await openTools(demoWith({ set_challenge_profile: () => before }));
    await user.click(await screen.findByRole("button", { name: "清空第 1 个徽章" }));
    await user.click(screen.getByRole("button", { name: zhCN["profile.challenges.apply"] }));
    expect(await screen.findByText(zhCN["profile.challenges.partly"])).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "第 1 个徽章: 闪电战" }),
      "the slots show what the client kept",
    ).toBeInTheDocument();
  });

  it("disguises the rank only once switched on, with no division from Master up", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const toggle = await screen.findByRole("switch", { name: zhCN["profile.rank.enable"] });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText(zhCN["profile.rank.offPreview"])).toBeInTheDocument();

    await user.click(toggle);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          profile: expect.objectContaining({
            rankDisguise: { enabled: true, queue: "solo", tier: "DIAMOND", division: "I" },
          }),
        }),
      }),
    );
    expect(screen.getByText("好友看到：璀璨钻石 I · 单双排")).toBeInTheDocument();
    expect(
      screen.getByRole("radiogroup", { name: zhCN["profile.rank.division"] }),
    ).toBeInTheDocument();

    const tiers = screen.getByRole("radiogroup", { name: zhCN["profile.rank.tier"] });
    await user.click(within(tiers).getByRole("radio", { name: "大师" }));
    expect(screen.queryByRole("radiogroup", { name: zhCN["profile.rank.division"] })).toBeNull();
    await user.click(screen.getByRole("radio", { name: zhCN["common.flex"] }));
    expect(screen.getByText("好友看到：超凡大师 · 灵活组排")).toBeInTheDocument();
  });

  it("backs up the game settings, and restores them only outside a game", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const list = await screen.findByRole("list", { name: zhCN["profile.backup.title"] });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    // The demo client is in champ select.
    for (const button of within(list).getAllByRole("button", {
      name: zhCN["profile.backup.restore"],
    }))
      expect(button).toBeDisabled();
    expect(screen.getByText(zhCN["profile.backup.busy"])).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: zhCN["profile.backup.create"] }));
    expect(call).toHaveBeenCalledWith("create_game_settings_backup");
    expect(await screen.findByText(zhCN["profile.backup.created"])).toBeInTheDocument();
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);

    await user.click(
      within(list).getAllByRole("button", { name: zhCN["profile.backup.delete"] })[0]!,
    );
    await user.click(
      within(list).getByRole("button", { name: zhCN["profile.backup.confirmDelete"] }),
    );
    expect(await screen.findByText(zhCN["profile.backup.deleted"])).toBeInTheDocument();
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);

    const snapshot = JSON.stringify({
      format: "winer-game-settings",
      version: 1,
      takenAt: 1,
      gameSettings: { General: {} },
    });
    await user.upload(
      screen.getByLabelText(zhCN["profile.backup.import"]),
      new File([snapshot], "backup.json", { type: "application/json" }),
    );
    expect(await screen.findByText(zhCN["profile.backup.imported"])).toBeInTheDocument();
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
    await user.upload(
      screen.getByLabelText(zhCN["profile.backup.import"]),
      new File(["{}"], "notes.json", { type: "application/json" }),
    );
    expect(
      await screen.findByText("这个文件不是 winer 的设置备份：not a winer settings backup"),
    ).toBeInTheDocument();
  });

  it("restores the chosen half from the home screen, and says why it cannot in a game", async () => {
    const backend = await idleDemo();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const list = await screen.findByRole("list", { name: zhCN["profile.backup.title"] });
    const [newest] = within(list).getAllByRole("button", { name: zhCN["profile.backup.restore"] });
    expect(newest).toBeEnabled();
    await user.click(newest!);
    await user.click(screen.getByRole("menuitem", { name: zhCN["profile.backup.hotkeys"] }));
    expect(call).toHaveBeenCalledWith("restore_game_settings_backup", {
      id: expect.any(Number),
      channels: ["hotkeys"],
    });
    expect(await screen.findByText(zhCN["profile.backup.restoredHotkeys"])).toBeInTheDocument();
    cleanup();

    // The phase moved on before the window heard: the core refuses, and the window says why.
    const refusing = await idleDemo({
      restore_game_settings_backup: () => {
        throw { code: "busy", message: "game settings are restored only outside a game" };
      },
    });
    const again = await openTools(refusing);
    const rows = await screen.findByRole("list", { name: zhCN["profile.backup.title"] });
    await again.user.click(
      within(rows).getAllByRole("button", { name: zhCN["profile.backup.restore"] })[0]!,
    );
    await again.user.click(screen.getByRole("menuitem", { name: zhCN["profile.backup.all"] }));
    expect(await screen.findByText(zhCN["profile.backup.busy"])).toBeInTheDocument();
  });
});
