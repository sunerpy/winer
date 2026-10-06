// The whole window against the demo client: every page renders without tripping its error
// boundary, and the settings that restyle or re-word the window take effect.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type {
  CalloutRule,
  ChampSelectView,
  Event,
  GameView,
  MatchSummary,
  Mode,
  Seat,
  Snapshot,
} from "@winer/shared";

import { App } from "./App";
import type { Backend, CommandName } from "./lib/backend";
import { demoBackend, demoGame, demoLobby } from "./lib/demo";
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
          return { availability, statusMessage: "今晚上分" };
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
      callout: [],
      allyCallout: [],
    };
    const store = new AppStore(demoBackend());
    // As the shell does before any page: the settings are loaded first.
    store.settings.set(await demoBackend().call("get_settings"));
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
    expect(screen.getByText(/^👑 峡谷通天代：1L 暗夜里的光/)).toBeInTheDocument();
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
      screen.getByText("📢【蓝色方】winer 战绩鉴定"),
      "the side and winer's name lead the callout",
    ).toBeInTheDocument();
    expect(
      screen.getByText(/^👑 峡谷通天代：1L 暗夜里的光/),
      "the seat and the player, not the champion",
    ).toBeInTheDocument();
    expect(screen.getByText(/^💀 纯正牛马：5L 眼位守护者/)).toBeInTheDocument();
    expect(
      screen.getByText(/^👑 峡谷通天代：1L/),
      "the title and the quip ride along",
    ).toHaveTextContent("战力7.4「版本答案」，对面五个人准备举报代练");
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

  /** The demo's loader, not linked: the last setup failed with `setupError`. */
  async function unlinked(setupError: string, needsElevation: boolean) {
    const status = await demoBackend().call("get_plugin_status");
    const backend = demoWith({
      get_plugin_status: () => ({
        ...status,
        loaderDir: null,
        active: false,
        managed: false,
        installedVersion: null,
        current: false,
        connected: 0,
        setupError,
        needsElevation,
      }),
    });
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.plugin"] }));
    return { user, call };
  }

  it("says why activating the loader needs administrator rights once, and restarts elevated", async () => {
    const refused = "客户端没有所需的特权。 (os error 1314)";
    const { user, call } = await unlinked(refused, true);
    expect(await screen.findByText(zhCN["plugin.loaderNeedsAdmin"])).toBeInTheDocument();
    expect(screen.getByText(zhCN["plugin.needsAdmin"])).toBeInTheDocument();
    expect(
      screen.queryByText(new RegExp(refused.replace(/[()]/g, "\\$&"))),
      "not the system's words, which call winer the client",
    ).toBeNull();
    await user.click(screen.getByRole("button", { name: zhCN["connection.relaunch"] }));
    expect(call).toHaveBeenCalledWith("relaunch_elevated");
  });

  it("keeps the system's words for any other setup failure, with a line of context", async () => {
    await unlinked("拒绝访问。 (os error 5)", false);
    expect(await screen.findByText("没有装好：拒绝访问。 (os error 5)")).toBeInTheDocument();
    expect(screen.getByText(zhCN["plugin.loaderFailedHint"])).toBeInTheDocument();
    expect(screen.queryByText(zhCN["plugin.needsAdmin"])).toBeNull();
    expect(screen.queryByRole("button", { name: zhCN["connection.relaunch"] })).toBeNull();
  });

  it("previews the callout as it is written: opening line, scheme and own tiers", async () => {
    const { user, nav } = await renderApp();
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^👑 峡谷通天代：1L /)).toBeInTheDocument();
    expect(screen.getByText(/^💀 纯正牛马：5L /)).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: zhCN["auto.template"] }),
      "a blank template shows the default, which names the seat",
    ).toHaveAttribute(
      "placeholder",
      "{emoji}{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    );

    // The compact style: one short line a player, the same columns, no emoji, title or quip.
    await user.click(screen.getByRole("radio", { name: zhCN["auto.style.compact"] }));
    expect(
      await screen.findByText("1L 峡谷通天代｜胜率60%｜KDA 4.1｜战力7.4｜暗夜里的光"),
    ).toBeInTheDocument();
    expect(screen.getByText("【蓝色方】winer 战绩鉴定")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: zhCN["auto.template"] })).toHaveAttribute(
      "placeholder",
      "{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜{name}",
    );
    await user.click(screen.getByRole("radio", { name: zhCN["auto.style.rich"] }));
    expect(await screen.findByText(/^👑 峡谷通天代：1L /)).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: zhCN["auto.header"] }), "开局分析{Enter}");
    expect(await screen.findByText("📢【蓝色方】winer 战绩鉴定 · 开局分析")).toBeInTheDocument();

    // The scheme lives in Settings › Rating; the callout's own row only leads there.
    await user.click(screen.getByRole("button", { name: zhCN["auto.schemeEdit"] }));
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    expect(within(dialog).getByRole("tab", { name: zhCN["settings.rating"] })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(within(dialog).getByRole("radio", { name: /^峡谷食物链/ }));
    expect(await screen.findByText(/^👑 峡谷之王：1L /)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("radio", { name: /^自定义/ }));
    await user.type(within(dialog).getByRole("textbox", { name: "第 1 档" }), "大腿{Enter}");
    await user.type(within(dialog).getByRole("textbox", { name: "第 2 档" }), "挂件{Enter}");
    expect(await screen.findByText(/^💀 挂件：2L /)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["common.close"] }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText(zhCN["rating.scheme.custom"])).toBeInTheDocument();
  });

  it("explains the rating in settings, switches the titles and opens the documentation", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^👑 峡谷通天代：1L /)).toHaveTextContent("「靠谱队友」");

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
      expect(screen.getByText(/^👑 峡谷通天代：1L /)).not.toHaveTextContent("「靠谱队友」"),
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

describe("storage", () => {
  async function openStorage(backend: Backend) {
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.about"] }));
    const storage = await within(dialog).findByRole("region", { name: zhCN["storage.title"] });
    return { user, storage };
  }

  it("shows what winer keeps with its limits, and clears the cache saying what went", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, storage } = await openStorage(backend);
    // The demo install's figures, as measured on a Windows PC, with the limits the shell keeps.
    expect(
      await within(storage).findByText(
        "保留最近 7 天、合计最多 50 MB，单个文件最大 10 MB，超出时先删除最旧的",
      ),
    ).toBeInTheDocument();
    expect(
      within(storage).getByText(/^其中可清理的缓存 9.5 MB，网页缓存上限 32 MB/),
    ).toBeInTheDocument();
    expect(within(storage).getByText("最多保留 10 份，清理缓存不会删除")).toBeInTheDocument();
    expect(within(storage).getByText("523 项 · 图片 12 MB")).toBeInTheDocument();
    expect(within(storage).queryByText(zhCN["storage.webviewPending"])).toBeNull();

    await user.click(within(storage).getByRole("button", { name: zhCN["storage.clear"] }));
    const status = await within(storage).findByRole("status");
    expect(status).toHaveTextContent(
      "已清理旧日志 1 个（863 B）、更新安装包 1 个（6 MB）、内存缓存 523 项。",
    );
    expect(status).toHaveTextContent("网页缓存 9.5 MB 将在下次启动 winer 时清除。");
    expect(call).toHaveBeenCalledWith("clear_caches");
    // Read again: the log being written, nothing in memory, the WebView's cache due to go.
    expect(await within(storage).findByText("0 项 · 图片 0 B")).toBeInTheDocument();
    expect(within(storage).getByText(zhCN["storage.webviewPending"])).toBeInTheDocument();
  });

  it("says when what winer keeps cannot be read, and reads it again", async () => {
    let refused = true;
    const demo = demoBackend();
    const backend = demoWith({
      get_storage: () => {
        if (refused) throw { code: "internal", message: "access denied" };
        return demo.call("get_storage");
      },
    });
    const { user, storage } = await openStorage(backend);
    expect(await within(storage).findByText(zhCN["storage.loadFailed"])).toBeInTheDocument();
    expect(within(storage).getByText("access denied")).toBeInTheDocument();
    refused = false;
    await user.click(within(storage).getByRole("button", { name: zhCN["common.retry"] }));
    expect(await within(storage).findByText("523 项 · 图片 12 MB")).toBeInTheDocument();
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
            presence: {
              remember: true,
              availability: "chat",
              statusMessage: "今晚上分",
              mobileMessage: false,
            },
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
      bannerId: null,
    });
    expect(await screen.findByText(zhCN["profile.challenges.done"])).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "第 1 个徽章: 射手收藏家" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "第 2 个徽章: 雪球大战" })).toBeInTheDocument();
  });

  it("flies a banner the player owns, or the default, and sends it only when it changed", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const current = await screen.findByRole("button", { name: /^魄罗之王的旗帜/ });
    await user.click(current);
    const picker = screen.getByRole("dialog", { name: zhCN["profile.challenges.chooseBanner"] });
    expect(
      within(picker)
        .getAllByRole("button")
        .map((tile) => tile.getAttribute("aria-label")),
      "the default first, then the banners owned",
    ).toEqual(["默认旗帜", "北极星(2023)贵族旗帜", "魄罗之王的旗帜"]);
    expect(within(picker).getByRole("button", { name: "魄罗之王的旗帜" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(picker).getByRole("button", { name: "北极星(2023)贵族旗帜" }));
    const apply = screen.getByRole("button", { name: zhCN["profile.challenges.apply"] });
    await user.click(apply);
    expect(call).toHaveBeenCalledWith("set_challenge_profile", {
      challengeIds: [101304, 505005],
      titleId: 1436,
      bannerId: "6",
    });
    // The client's answer is in once the button is no longer busy. (A toast of the test before may
    // still be up, so the panel says it, not the toast.)
    await waitFor(() => expect(apply).not.toHaveAttribute("aria-busy"));
    expect(screen.queryByText(zhCN["profile.challenges.partly"])).toBeNull();
    expect(apply, "nothing left to apply").toBeDisabled();

    await user.click(screen.getByRole("button", { name: /^北极星\(2023\)贵族旗帜/ }));
    await user.click(
      within(
        screen.getByRole("dialog", { name: zhCN["profile.challenges.chooseBanner"] }),
      ).getByRole("button", { name: "默认旗帜" }),
    );
    await user.click(apply);
    expect(call).toHaveBeenCalledWith("set_challenge_profile", {
      challengeIds: [101304, 505005],
      titleId: 1436,
      bannerId: "",
    });
    await waitFor(() => expect(apply).not.toHaveAttribute("aria-busy"));
    expect(screen.getByRole("button", { name: /^默认旗帜/ })).toBeInTheDocument();
  });

  it("says 手机在线 in the status message while the mobile state is chosen, if asked", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const statuses = await screen.findByRole("radiogroup", { name: zhCN["tools.presence"] });
    const message = screen.getByRole("textbox", { name: zhCN["tools.message"] });
    await waitFor(() => expect(message).toHaveValue("今晚上分"));
    expect(
      screen.queryByRole("switch", { name: zhCN["profile.mobileMessage"] }),
      "a part of the mobile state only",
    ).toBeNull();

    // No message of the user's own.
    await user.clear(message);
    await user.click(screen.getByRole("button", { name: zhCN["common.save"] }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("set_status_message", { message: "" }));

    await user.click(within(statuses).getByRole("radio", { name: "手机在线" }));
    const toggle = await screen.findByRole("switch", { name: zhCN["profile.mobileMessage"] });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(message, "off until switched on").toHaveValue("");

    await user.click(toggle);
    await waitFor(() => expect(message).toHaveValue("手机在线"));
    expect(call).toHaveBeenCalledWith("set_settings", {
      settings: expect.objectContaining({
        profile: expect.objectContaining({
          presence: expect.objectContaining({ mobileMessage: true }),
        }),
      }),
    });
    expect(call).toHaveBeenCalledWith("apply_mobile_message");

    await user.click(within(statuses).getByRole("radio", { name: "离开" }));
    await waitFor(() => expect(message, "another state takes it down").toHaveValue(""));
    expect(screen.queryByRole("switch", { name: zhCN["profile.mobileMessage"] })).toBeNull();
    await user.click(within(statuses).getByRole("radio", { name: "手机在线" }));
    await waitFor(() => expect(message, "and the state puts it back").toHaveValue("手机在线"));
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
    expect(within(dialog).getByText("Alt")).toBeInTheDocument();
    expect(within(dialog).getByText("`")).toBeInTheDocument();
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
          shortcut: "Alt+Backquote",
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
      callout: [],
      allyCallout: [],
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
    expect(await screen.findByText(/^👑 峡谷通天代：1L /)).toBeInTheDocument();
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

// The history panel in the client.
describe("the history panel in the client", () => {
  it("is switched on its own on the in-client page, on to begin with", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.plugin"] }));
    const toggle = await screen.findByRole("switch", { name: zhCN["overlay.historyInClient"] });
    expect(toggle).toBeChecked();
    expect(screen.getByText(zhCN["overlay.historyInClientHint"])).toBeInTheDocument();
    await user.click(toggle);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          plugin: expect.objectContaining({ historyInClient: false, lobbyPanel: true }),
        }),
      }),
    );
    expect(screen.getByRole("switch", { name: zhCN["social.pluginLobby"] })).toBeChecked();
  });
});

describe("callout", () => {
  /** The demo settings, the callout's shortcut and in-game sending as given. */
  async function settingsWith(hotkey: string | null, inGame: boolean) {
    const settings = await demoBackend().call("get_settings");
    return {
      ...settings,
      automation: {
        ...settings.automation,
        callout: { ...settings.automation.callout, hotkey, inGame },
      },
    };
  }

  /** The Live page alone, over a store holding `settings` and `snapshot`. */
  async function livePage(snapshot: Partial<Snapshot>, hotkey: string | null, inGame: boolean) {
    const store = new AppStore(demoBackend());
    store.settings.set(await settingsWith(hotkey, inGame));
    store.live.set({
      ...EMPTY_SNAPSHOT,
      rev: 1,
      connection: { status: "connected", port: 1, platformId: "NJ100" },
      ...snapshot,
    });
    const navigate = vi.fn();
    render(
      <StoreContext value={store}>
        <ShellContext value={{ route: { page: "live" }, navigate, openSettings: vi.fn() }}>
          <LivePage />
        </ShellContext>
      </StoreContext>,
    );
    return { store, navigate };
  }

  /** The callout's panel, by its eyebrow. */
  const panel = () => {
    const card = screen
      .getByRole("heading", { name: zhCN["live.callout"] })
      .closest("div.rounded-10");
    expect(card).not.toBeNull();
    return within(card as HTMLElement);
  };

  it("names the shortcut under the champ-select callout, or says where to set one", async () => {
    const without = await renderApp();
    await without.user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(panel().getByText(zhCN["callout.liveNoHotkey"])).toBeInTheDocument();
    cleanup();

    const hotkey = await settingsWith("Ctrl+Shift+X", false);
    const { user } = await renderApp(demoWith({ get_settings: () => hotkey }));
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(panel().getByText(zhCN["callout.liveHotkey"])).toBeInTheDocument();
    expect(panel().getByText("Shift")).toBeInTheDocument();
    expect(panel().getByText("X")).toBeInTheDocument();
    expect(
      panel().getByRole("button", { name: zhCN["live.sendTeam"] }),
      "the button stays: the shortcut sends what it sends",
    ).toBeInTheDocument();
  });

  it("shows both teams' lines in the game by champion, with no send button, and says what the shortcut types", async () => {
    const { store, navigate } = await livePage(
      { phase: "InProgress", game: demoGame() },
      "Ctrl+Shift+X",
      false,
    );
    const enemies = within(
      panel().getByRole("region", { name: zhCN["callout.gameTeams.enemies"] }),
    );
    expect(enemies.getByText("【敌方·红色方】winer 战绩鉴定")).toBeInTheDocument();
    expect(enemies.getByText(/^小心 卡兹克：峡谷通天代/)).toBeInTheDocument();
    expect(enemies.getByText(/^对面 亚索：纯正牛马/)).toBeInTheDocument();
    // Callout: beside them the team's own lines, as champ select's, by champion.
    const allies = within(panel().getByRole("region", { name: zhCN["callout.gameTeams.allies"] }));
    expect(allies.getByText("【我方·蓝色方】winer 战绩鉴定")).toBeInTheDocument();
    expect(
      allies.getByText(/^峡谷通天代：阿狸，近20场胜率60%/),
      "the champion where champ select names the seat and the player",
    ).toBeInTheDocument();
    expect(allies.getAllByRole("listitem")).toHaveLength(6);
    expect(panel().getByText(zhCN["callout.inGameOff"])).toBeInTheDocument();
    expect(panel().getByText(zhCN["callout.liveInGameOff"])).toBeInTheDocument();
    expect(
      panel().queryByRole("button", { name: zhCN["live.sendTeam"] }),
      "the game's chat has no API to post to",
    ).toBeNull();

    const choose = (change: Partial<CalloutRule>) =>
      act(() => {
        const settings = store.settings.get();
        if (settings)
          store.settings.set({
            ...settings,
            automation: {
              ...settings.automation,
              callout: { ...settings.automation.callout, ...change },
            },
          });
      });
    choose({ inGame: true });
    expect(panel().getByText(zhCN["callout.inGameOn"])).toBeInTheDocument();
    expect(
      panel().getByText(zhCN["callout.liveHotkeyGame.enemies"]),
      "the enemy lines by default",
    ).toBeInTheDocument();
    expect(panel().getByText("Ctrl")).toBeInTheDocument();
    choose({ gameTeams: "allies" });
    expect(panel().getByText(zhCN["callout.liveHotkeyGame.allies"])).toBeInTheDocument();
    choose({ gameTeams: "both" });
    expect(
      panel().getByText("在游戏里按快捷键，winer 先输入对面、再输入我方，最多 6 行："),
    ).toBeInTheDocument();

    act(() =>
      store.hotkey.set({
        shortcut: "Alt+Backquote",
        active: true,
        suspended: false,
        error: null,
        callout: { shortcut: "Ctrl+Shift+X", active: false, error: "HotKey already registered" },
      }),
    );
    expect(panel().getByText(zhCN["callout.liveHotkeyFailed"])).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(panel().getByRole("button", { name: zhCN["callout.configure"] }));
    expect(navigate).toHaveBeenCalledWith({ page: "automation" });
  });

  it("says what comes in the game before the lines are there, and has no panel without two sides", async () => {
    await livePage(
      { phase: "InProgress", game: { ...demoGame(), callout: [], allyCallout: [] } },
      null,
      false,
    );
    expect(panel().getByText(zhCN["callout.liveGameEmpty"])).toBeInTheDocument();
    expect(panel().getByText(zhCN["callout.liveAlliesEmpty"])).toBeInTheDocument();
    expect(panel().getByText(zhCN["callout.liveNoHotkeyGame"])).toBeInTheDocument();
    cleanup();

    // Arena's pairs: no one other team to talk about.
    await livePage(
      {
        phase: "InProgress",
        game: { ...demoGame(), sides: false, callout: [], allyCallout: [] },
      },
      "Ctrl+Shift+X",
      true,
    );
    expect(screen.getByRole("radiogroup", { name: zhCN["live.board"] })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: zhCN["live.callout"] })).toBeNull();
  });

  it("records the callout's shortcut apart from the window's and opts in to typing in the game", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(
      await screen.findByText("【敌方·红色方】winer 战绩鉴定"),
      "what the shortcut would type in the game, previewed",
    ).toBeInTheDocument();
    expect(screen.getByText(/^小心 阿狸：峡谷通天代/)).toBeInTheDocument();
    expect(screen.getByText(/^对面 阿狸：纯正牛马/)).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("preview_game_callout", {
      rule: expect.objectContaining({ hotkey: null, inGame: false }),
      general: expect.objectContaining({ language: "zh-CN" }),
    });

    await user.click(screen.getByRole("button", { name: zhCN["social.hotkeyRecord"] }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: true }));
    fireEvent.keyDown(window, { code: "Backquote", key: "`", altKey: true });
    expect(screen.getByText(zhCN["callout.hotkeyTakenByWindow"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "KeyX", key: "X", ctrlKey: true, shiftKey: true });
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            callout: expect.objectContaining({ hotkey: "Ctrl+Shift+X" }),
          }),
          general: expect.objectContaining({ hotkey: "Alt+Backquote" }),
        }),
      }),
    );
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: false }));
    expect(await screen.findByText(zhCN["social.hotkeyActive"])).toBeInTheDocument();

    const inGame = screen.getByRole("switch", { name: zhCN["callout.inGame"] });
    expect(inGame, "off until asked").not.toBeChecked();
    expect(screen.getByText(zhCN["callout.inGameRisk"])).toBeInTheDocument();
    await user.click(inGame);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            callout: expect.objectContaining({ hotkey: "Ctrl+Shift+X", inGame: true }),
          }),
        }),
      }),
    );

    const watch = screen.getByRole("textbox", { name: zhCN["callout.watch"] });
    expect(watch).toHaveAttribute(
      "placeholder",
      "小心 {champion}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}",
    );
    // `{{` types a brace; `{Enter}` commits.
    await user.type(watch, "注意 {{champion}{Enter}");
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            callout: expect.objectContaining({ watchTemplate: "注意 {champion}" }),
          }),
        }),
      }),
    );
  });

  it("chooses whose lines a press types in the game, six at most, and takes the team's own line", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const teams = await screen.findByRole("radiogroup", { name: zhCN["callout.gameTeams"] });
    expect(
      within(teams).getByRole("radio", { name: "对面" }),
      "the enemy lines by default",
    ).toBeChecked();
    expect(screen.getByText(/每按一次最多输入 6 行/), "the cap, in the hint").toBeInTheDocument();
    const saved = (change: Partial<CalloutRule>) =>
      waitFor(() =>
        expect(call).toHaveBeenCalledWith("set_settings", {
          settings: expect.objectContaining({
            automation: expect.objectContaining({ callout: expect.objectContaining(change) }),
          }),
        }),
      );

    await user.click(within(teams).getByRole("radio", { name: "我方" }));
    await saved({ gameTeams: "allies" });
    expect(
      await screen.findByText("【我方·蓝色方】winer 战绩鉴定"),
      "the preview types the team",
    ).toBeInTheDocument();
    expect(screen.getByText(/^峡谷通天代：阿狸，近20场胜率60%/)).toBeInTheDocument();
    expect(screen.queryByText("【敌方·红色方】winer 战绩鉴定")).toBeNull();

    await user.click(within(teams).getByRole("radio", { name: "双方" }));
    await saved({ gameTeams: "both" });
    // As one press would: the enemy's three lines, then the team's first line and its best two.
    const enemy = await screen.findByText("【敌方·红色方】winer 战绩鉴定");
    expect(within(enemy.closest("ol") as HTMLElement).getAllByRole("listitem")).toHaveLength(6);
    expect(screen.getByText(/^人形防御塔：阿狸，/)).toBeInTheDocument();
    expect(screen.queryByText(/^峡谷公务员：阿狸，/), "cut at the limit").toBeNull();

    const ally = screen.getByRole("textbox", { name: zhCN["callout.ally"] });
    expect(ally).toHaveAttribute(
      "placeholder",
      "{standing}：{champion}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    );
    await user.type(ally, "{{champion} {{standing}{Enter}");
    await saved({ allyTemplate: "{champion} {standing}" });
    const section = within(screen.getByRole("region", { name: zhCN["callout.gameSection"] }));
    await user.click(section.getByRole("button", { name: zhCN["auto.reset"] }));
    await saved({ watchTemplate: "", targetTemplate: "", allyTemplate: "" });
  });

  it("keeps the window's shortcut off the callout's combination too", async () => {
    const hotkey = await settingsWith("Ctrl+Shift+X", false);
    const backend = demoWith({ get_settings: () => hotkey });
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.general"] }));
    await user.click(within(dialog).getByRole("button", { name: zhCN["social.hotkeyChange"] }));
    fireEvent.keyDown(window, { code: "KeyX", key: "X", ctrlKey: true, shiftKey: true });
    expect(within(dialog).getByText(zhCN["callout.hotkeyTakenByCallout"])).toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("set_settings", expect.anything());
  });
});

describe("history", () => {
  async function openHistory(backend: Backend) {
    localStorage.removeItem("winer.history.pageSize");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect(await screen.findByText("第 1–10 场")).toBeInTheDocument();
    return user;
  }

  /** How many times `command` went to the backend, with arguments like `like` where given. */
  const asked = (
    call: { mock: { calls: unknown[][] } },
    command: CommandName,
    like: Record<string, unknown> = {},
  ) =>
    call.mock.calls.filter(
      ([name, args]) =>
        name === command &&
        Object.entries(like).every(
          ([key, value]) => (args as Record<string, unknown> | undefined)?.[key] === value,
        ),
    ).length;

  it("says what the form counts and rates the player alone, with the rule behind each", async () => {
    const backend = demoBackend();
    const user = await openHistory(backend);
    expect(await screen.findByText("近 20 场 · 所有模式")).toBeInTheDocument();
    const standing = await backend.call("get_player_standing", { puuid: "demo-me" });
    const rating = standing.rating;
    if (!rating) throw new Error("the demo player has games to rate");
    expect(await screen.findByText(rating.label)).toBeInTheDocument();
    if (rating.title) expect(screen.getByText(rating.title)).toBeInTheDocument();
    if (rating.quip) expect(screen.getByText(`“${rating.quip}”`)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: zhCN["history.formHint"] }));
    const rule = screen.getByRole("dialog", { name: zhCN["history.formHint"] });
    expect(rule).toHaveTextContent("取客户端列出的最近 30 场对局里最新的 20 场");
    expect(rule).toHaveTextContent("不只是排位");
    expect(rule).toHaveTextContent("自定义对局（这次跳过 0 场）");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();

    await user.click(screen.getByRole("button", { name: zhCN["history.standingHint"] }));
    const how = screen.getByRole("dialog", { name: zhCN["history.standingHint"] });
    expect(how).toHaveTextContent("峡谷五档在本队五人里按战力排名分档");
    expect(how).toHaveTextContent(`落在「${rating.label}」`);
  });

  it("hides custom games by default, says how many, and shows them once switched", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    await user.type(
      screen.getByRole("textbox", { name: zhCN["history.search"] }),
      "野区观光客#10005",
    );
    await user.click(screen.getByRole("button", { name: zhCN["history.find"] }));
    expect(await screen.findByText("已隐藏 2 场")).toBeInTheDocument();
    expect(screen.queryByText("嚎哭深渊 全随机"), "custom games are hidden").toBeNull();
    expect(screen.getAllByText("入门级"), "a game against bots is listed").toHaveLength(1);
    const standing = await backend.call("get_player_standing", { puuid: "demo-3" });
    expect(standing.scope).toMatchObject({ custom: 2, bots: 1 });

    const hide = screen.getByRole("switch", { name: zhCN["history.hideCustom"] });
    expect(hide).toHaveAttribute("aria-checked", "true");
    await user.click(hide);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({ history: { hideCustomGames: false } }),
      }),
    );
    expect(screen.getAllByText("嚎哭深渊 全随机")).toHaveLength(2);
    expect(screen.queryByText("已隐藏 2 场")).toBeNull();
  });

  it("goes back to a player at once, where the list was left, and reopens a scoreboard unasked", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    const pages = screen.getByRole("navigation", { name: zhCN["history.pages"] });
    await user.click(within(pages).getByRole("button", { name: "下一页" }));
    expect(screen.getByText("第 11–20 场")).toBeInTheDocument();
    const game = screen.getAllByRole("button", { pressed: false })[0] as HTMLElement;
    await user.click(game);
    const scoreboard = await screen.findByLabelText(zhCN["history.detail"]);
    const opponent = await within(scoreboard).findAllByRole("button", { name: /·对手/ });
    expect(asked(call, "get_match_detail")).toBe(1);

    await user.click(opponent[0] as HTMLElement);
    await user.click(await screen.findByRole("button", { name: zhCN["history.mine"] }));
    // Drawn from what was shown: no skeleton, the second page, the header.
    expect(screen.getByText("第 11–20 场")).toBeInTheDocument();
    expect(screen.getByText("近 20 场 · 所有模式")).toBeInTheDocument();
    const mine = { puuid: "demo-me", begin: 0, count: 50 };
    // The newest games asked for once more, in the background: the same ones, the list stays.
    await waitFor(() => expect(asked(call, "get_match_history", mine)).toBe(2));
    expect(screen.getByText("第 11–20 场")).toBeInTheDocument();
    const again = screen.getAllByRole("button", { pressed: false })[0] as HTMLElement;
    expect(again).toHaveTextContent(game.textContent ?? "");
    await user.click(again);
    expect(screen.getByLabelText(zhCN["history.detail"])).toBeInTheDocument();
    expect(asked(call, "get_match_detail"), "a finished game is read once").toBe(1);
  });

  it("names what the overview's numbers count and leaves custom games out of its short list", async () => {
    const custom = (game: MatchSummary): MatchSummary => ({
      ...game,
      kind: "custom",
      queueId: 3220,
    });
    const demo = demoBackend();
    const backend = demoWith({
      get_match_history: async (args: { puuid: string; begin: number; count: number }) => {
        const page = await demo.call("get_match_history", args);
        return {
          ...page,
          games: page.games.map((game, index) => (index < 2 ? custom(game) : game)),
        };
      },
    });
    await renderApp(backend);
    expect(await screen.findByText("近 20 场 · 所有模式")).toBeInTheDocument();
    const recent = (await screen.findByText(zhCN["overview.recentMatches"])).closest(
      "div.rounded-10",
    ) as HTMLElement;
    await waitFor(() =>
      expect(within(recent).getAllByRole("button", { pressed: false })).toHaveLength(5),
    );
    expect(within(recent).queryByText("嚎哭深渊 全随机")).toBeNull();
    expect(screen.getAllByTitle(/单双排的胜负场次，来自客户端的段位数据/)).toHaveLength(1);
  });
});
