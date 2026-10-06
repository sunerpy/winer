// The whole window against the demo client: every page renders without tripping its error
// boundary, and the settings that restyle or re-word the window take effect.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Event, GameView, Mode, Seat } from "@winer/shared";

import { App } from "./App";
import type { Backend, CommandName } from "./lib/backend";
import { demoBackend, demoLobby } from "./lib/demo";
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

describe("social", () => {
  /** The demo client, with the core's events in the test's hand. */
  function demoWithEvents(): { backend: Backend; push: (event: Event) => Promise<void> } {
    const base = demoBackend();
    const handlers = new Set<(event: Event) => void>();
    return {
      backend: {
        ...base,
        onEvent: (handler) => {
          handlers.add(handler);
          const off = base.onEvent(handler);
          return () => {
            handlers.delete(handler);
            off();
          };
        },
      },
      push: (event) =>
        act(async () => {
          handlers.forEach((handler) => handler(event));
        }),
    };
  }

  async function openGeneralSettings(user: ReturnType<typeof userEvent.setup>) {
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.general"] }));
    return dialog;
  }

  it("lists the friends at play on the overview, grouped, each opening their history", async () => {
    const { user } = await renderApp();
    const panel = (await screen.findByText(zhCN["social.friends"])).closest("div.rounded-10");
    expect(panel).not.toBeNull();
    const friends = within(panel as HTMLElement);
    expect(friends.getByText("4 位在选人或游戏中")).toBeInTheDocument();
    const rows = friends.getAllByRole("button", { name: /^查看 .+ 的战绩$/ });
    expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual([
      "查看 上分小能手#20008 的战绩",
      "查看 峡谷夜行者#20008 的战绩",
      "查看 补兵机器#20008 的战绩",
      "查看 辅助永不死#20008 的战绩",
    ]);
    expect(friends.queryByText("周末玩家"), "a friend at home is not listed").toBeNull();
    expect(rows[0]).toHaveTextContent(/排位赛 单排\/双排 · 游戏中.*25:1\d/);
    expect(rows[3]).toHaveTextContent("海克斯大乱斗 · 选英雄中");
    const grouped = rows.filter((row) => row.closest("li")?.dataset.group === "1");
    expect(grouped, "the two in one game share a group").toHaveLength(2);
    expect(within(grouped[0] as HTMLElement).getByTitle("一起玩 1")).toHaveTextContent("1");

    await user.click(rows[1] as HTMLElement);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["history.title"]);
  });

  it("shows the party in the lobby with their lanes and form", async () => {
    const store = new AppStore(demoBackend());
    store.live.set({
      ...EMPTY_SNAPSHOT,
      connection: { status: "connected", port: 1, platformId: "NJ100" },
      phase: "Lobby",
      lobby: demoLobby(),
    });
    const navigate = vi.fn();
    const user = userEvent.setup();
    render(
      <StoreContext value={store}>
        <ShellContext value={{ route: { page: "live" }, navigate, openSettings: vi.fn() }}>
          <LivePage />
        </ShellContext>
      </StoreContext>,
    );
    expect(screen.getByText(zhCN["social.lobby"])).toBeInTheDocument();
    const me = screen.getByRole("button", { name: "查看 暗夜里的光#10003 的战绩" });
    expect(within(me).getByText(zhCN["social.leader"])).toBeInTheDocument();
    expect(within(me).getByText("中单")).toBeInTheDocument();
    expect(within(me).getByText(zhCN["social.fill"])).toBeInTheDocument();
    expect(within(me).getByText("战力 7.4")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "查看 新来的队友#10009 的战绩" }));
    expect(navigate).toHaveBeenCalledWith({ page: "history", puuid: "demo-6" });
  });

  it("colours a premade party's badges by its group and keeps the number", async () => {
    const { user } = await renderApp();
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    const badges = screen.getAllByText("开黑 1");
    expect(badges).toHaveLength(2);
    for (const badge of badges)
      expect(badge.querySelector("[data-group]")).toHaveClass("bg-group-1");
  });

  it("opens the history the client asked for, over whatever was open", async () => {
    const { backend, push } = demoWithEvents();
    const { user } = await renderApp(backend);
    await openGeneralSettings(user);
    await push({ type: "openHistory", data: { puuid: "demo-3" } });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["history.title"]);
    expect(await screen.findByText("野区观光客#10005")).toBeInTheDocument();
  });

  it("records the hotkey with the old one let go, refuses a bare key, cancels and clears", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    const dialog = await openGeneralSettings(user);
    expect(within(dialog).getByText("Ctrl")).toBeInTheDocument();
    expect(await within(dialog).findByText(zhCN["social.hotkeyActive"])).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["social.hotkeyChange"] }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: true }));
    expect(within(dialog).getByText(zhCN["social.hotkeyListening"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "KeyQ", key: "Q", shiftKey: true });
    expect(within(dialog).getByText(zhCN["social.hotkeyInvalid"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "ControlLeft", key: "Control", ctrlKey: true });
    expect(within(dialog).getByText("…")).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "KeyQ", key: "q", ctrlKey: true, altKey: true });
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          general: expect.objectContaining({ hotkey: "Ctrl+Alt+Q" }),
        }),
      }),
    );
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: false }));
    expect(within(dialog).getByText("Q")).toBeInTheDocument();
    expect(screen.getByRole("dialog"), "the recorder's keys never reach the dialog").toBeVisible();

    await user.click(within(dialog).getByRole("button", { name: zhCN["social.hotkeyChange"] }));
    // From inside the dialog, where its own Escape handler would hear it.
    const cancel = within(dialog).getByRole("button", { name: zhCN["common.cancel"] });
    cancel.focus();
    fireEvent.keyDown(cancel, { code: "Escape", key: "Escape" });
    expect(screen.getByRole("dialog"), "Esc cancels the recording, not the settings").toBeVisible();
    expect(within(dialog).getByText("Q")).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["social.hotkeyClear"] }));
    expect(await within(dialog).findByText(zhCN["social.hotkeyOff"])).toBeInTheDocument();
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          general: expect.objectContaining({ hotkey: null }),
        }),
      }),
    );
  });

  it("says when another program holds the combination", async () => {
    const { user } = await renderApp(
      demoWith({
        get_hotkey_status: () => ({
          shortcut: "Ctrl+Shift+W",
          active: false,
          suspended: false,
          error: "HotKey already registered",
        }),
      }),
    );
    const dialog = await openGeneralSettings(user);
    expect(await within(dialog).findByText(zhCN["social.hotkeyFailed"])).toHaveAttribute(
      "title",
      "HotKey already registered",
    );
  });

  it("switches the friends list and the lobby in the client on their own", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.plugin"] }));
    await user.click(await screen.findByRole("switch", { name: zhCN["social.pluginFriends"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          plugin: expect.objectContaining({ friendStatus: false, lobbyPanel: true }),
        }),
      }),
    );
    expect(screen.getByRole("switch", { name: zhCN["social.pluginLobby"] })).toBeChecked();
  });
});
