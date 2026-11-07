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
import * as reports from "./pages/history/reports";
import type { Backend, CommandName } from "./lib/backend";
import { demoBackend, demoGame, demoLobby } from "./lib/demo";
import { AppStore, EMPTY_SNAPSHOT, StoreContext } from "./lib/store";
import { LivePage } from "./pages/Live";
import { ShellContext } from "./shell/navigation";
import { translate } from "./lib/i18n";
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
      ["nav.comeback", "comeback.title"],
      ["nav.phrases", "phrase.title"],
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

  it("loads the total and supports first, last and numeric page jumps", async () => {
    localStorage.removeItem("winer.history.pageSize");
    const { user, nav } = await renderApp();
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect(await screen.findByText("第 1–10 场 · 共 60 场")).toBeInTheDocument();
    const pages = screen.getByRole("navigation", { name: zhCN["history.pages"] });
    expect(within(pages).getByRole("button", { name: "首页" })).toBeDisabled();

    await user.click(within(pages).getByRole("button", { name: "尾页" }));
    expect(screen.getByText("第 51–60 场 · 共 60 场")).toBeInTheDocument();
    expect(within(pages).getByRole("button", { name: "尾页" })).toBeDisabled();

    const jump = within(pages).getByRole("spinbutton", { name: "跳转页码" });
    await user.clear(jump);
    await user.type(jump, "2");
    await user.click(within(pages).getByRole("button", { name: "跳转" }));
    expect(screen.getByText("第 11–20 场 · 共 60 场")).toBeInTheDocument();
    expect(within(pages).getByRole("button", { name: "第 2 页" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    await user.click(within(pages).getByRole("button", { name: "首页" }));
    expect(screen.getByText("第 1–10 场 · 共 60 场")).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "25" }));
    expect(screen.getByText("第 1–25 场 · 共 60 场")).toBeInTheDocument();
    expect(localStorage.getItem("winer.history.pageSize")).toBe("25");

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
      premadeInferred: false,
      note: null,
      autofilled: false,
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
    expect(screen.getByText(/^1L: 👑 峡谷通天代\|/)).toBeInTheDocument();
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
      screen.getByText(/^1L: 👑 峡谷通天代\|/),
      "the seat and tier lead the safe columns",
    ).toBeInTheDocument();
    expect(screen.getByText(/^5L: 💀 纯正牛马\|/)).toBeInTheDocument();
    const topLine = screen.getByText(/^1L: 👑 峡谷通天代\|/);
    expect(topLine).toHaveTextContent("战力7.4");
    expect(topLine, "free-form name, title and quip stay out").not.toHaveTextContent(
      /暗夜里的光|版本答案|举报代练/,
    );
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

  it("says another program launches the client, and what that leaves of winer's loader", async () => {
    const base = demoBackend();
    const [status, settings] = await Promise.all([
      base.call("get_plugin_status"),
      base.call("get_settings"),
    ]);
    const program = "Pengu Loader.exe";
    const view = async (linked: boolean, penguIfeo: boolean) => {
      const backend = demoWith({
        get_plugin_status: () => ({
          ...status,
          active: linked,
          managed: linked,
          loaderDir: linked ? status.loaderDir : null,
          foreignActivation: program,
          // An earlier refusal is not what the page leads with now.
          needsElevation: true,
        }),
        get_settings: () => ({ ...settings, plugin: { ...settings.plugin, penguIfeo } }),
      });
      const call = vi.spyOn(backend, "call");
      const { user, nav } = await renderApp(backend);
      await user.click(within(nav).getByRole("button", { name: zhCN["nav.plugin"] }));
      return { user, call };
    };
    // Off, the default: the other program is reported, and winer's loader stays as it was.
    const { user, call } = await view(true, false);
    expect(
      await screen.findByText(translate("zh-CN", "plugin.loaderForeign", { program })),
    ).toBeInTheDocument();
    expect(screen.getByText(zhCN["plugin.loaderForeignOff"])).toBeInTheDocument();
    expect(screen.queryByText(zhCN["plugin.needsAdmin"])).toBeNull();
    await user.click(screen.getByRole("switch", { name: zhCN["plugin.penguIfeo"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          plugin: expect.objectContaining({ penguIfeo: true }),
        }),
      }),
    );
    cleanup();
    // On: no link of winer's, or one left from before.
    await view(false, true);
    expect(await screen.findByText(zhCN["plugin.loaderForeignHint"])).toBeInTheDocument();
    cleanup();
    await view(true, true);
    expect(await screen.findByText(zhCN["plugin.loaderForeignBoth"])).toBeInTheDocument();
  });

  it("previews the callout as it is written: opening line, scheme and own tiers", async () => {
    const { user, nav } = await renderApp();
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^1L: 👑 峡谷通天代\|/)).toBeInTheDocument();
    expect(screen.getByText(/^5L: 💀 纯正牛马\|/)).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: zhCN["auto.template"] }),
      "a blank template shows the default, which names the seat",
    ).toHaveAttribute(
      "placeholder",
      "{seat}: {emoji}{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}",
    );

    // The compact style: one short line a player, the same columns, no emoji, title or quip.
    await user.click(screen.getByRole("radio", { name: zhCN["auto.style.compact"] }));
    expect(await screen.findByText("1L: 峡谷通天代|胜率60%|KDA4.1|战力7.4")).toBeInTheDocument();
    expect(screen.getByText("【蓝色方】winer 战绩鉴定")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: zhCN["auto.template"] })).toHaveAttribute(
      "placeholder",
      "{seat}: {standing}|胜率{winRate}|KDA{kda}|战力{score}",
    );
    await user.click(screen.getByRole("radio", { name: zhCN["auto.style.rich"] }));
    expect(await screen.findByText(/^1L: 👑 峡谷通天代\|/)).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: zhCN["auto.header"] }), "开局分析{Enter}");
    expect(await screen.findByText("📢【蓝色方】winer 战绩鉴定【开局分析】")).toBeInTheDocument();

    // The scheme lives in Settings › Rating; the callout's own row only leads there.
    await user.click(screen.getByRole("button", { name: zhCN["auto.schemeEdit"] }));
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    expect(within(dialog).getByRole("tab", { name: zhCN["settings.rating"] })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(within(dialog).getByRole("radio", { name: /^峡谷食物链/ }));
    expect(await screen.findByText(/^1L: 👑 峡谷之王\|/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("radio", { name: /^自定义/ }));
    await user.type(within(dialog).getByRole("textbox", { name: "第 1 档" }), "大腿{Enter}");
    await user.type(within(dialog).getByRole("textbox", { name: "第 2 档" }), "挂件{Enter}");
    expect(await screen.findByText(/^2L: 💀 挂件\|/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["common.close"] }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText(zhCN["rating.scheme.custom"])).toBeInTheDocument();
  });

  it("keeps titles out of the safe preview while switching them for local surfaces", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    expect(await screen.findByText(/^1L: 👑 峡谷通天代\|/)).not.toHaveTextContent("靠谱队友");

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
      expect(screen.getByText(/^1L: 👑 峡谷通天代\|/)).not.toHaveTextContent("靠谱队友"),
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

describe("updates", () => {
  it("keeps a verified update in winer until the user explicitly restarts", async () => {
    const backend = demoWith({
      get_update_status: () => ({ state: "ready", version: "0.0.8" }),
      install_update: () => null,
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);

    await user.click(
      await screen.findByRole("button", {
        name: zhCN["update.restartBadge"].replace("{version}", "0.0.8"),
      }),
    );
    const dialog = screen.getByRole("dialog", {
      name: zhCN["update.ready"].replace("{version}", "0.0.8"),
    });
    expect(within(dialog).getByText(zhCN["update.verified"])).toBeInTheDocument();
    expect(within(dialog).getByText(zhCN["update.readyHint"])).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: zhCN["update.restart"] }));
    expect(call).toHaveBeenCalledWith("install_update");
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

  it("lists the waiting rewards and claims them all, a choice only as picked", async () => {
    const backend = await idleDemo();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    expect(await screen.findByRole("button", { name: "一键领取（3）" })).toBeEnabled();
    const choice = screen.getByRole("group", { name: "三选一：守卫皮肤 的奖励" });
    expect(within(choice.parentElement!).getByText(/选 1 个/)).toBeInTheDocument();
    const wards = within(choice).getAllByRole("checkbox");
    expect(wards).toHaveLength(3);
    await user.click(wards[1]!);
    await user.click(wards[0]!);
    expect(wards[0], "one of one: the pick moves").toHaveAttribute("aria-checked", "true");
    expect(wards[1]).toHaveAttribute("aria-checked", "false");

    await user.click(screen.getByRole("button", { name: "一键领取（4）" }));
    expect(call).toHaveBeenCalledWith("claim_rewards", {
      choices: [{ grantId: "demo-grant-choice", rewardIds: ["demo-reward-ward-1"] }],
    });
    expect(await screen.findByText("已领取 4 个奖励")).toBeInTheDocument();
    expect(await screen.findByText(zhCN["rewards.none"])).toBeInTheDocument();
  });

  it("claims nothing from the queue to the end of a game, and says so", async () => {
    await openTools(demoBackend());
    expect(await screen.findByRole("button", { name: "一键领取（3）" })).toBeDisabled();
    expect(screen.getByText(zhCN["rewards.blocked"])).toBeInTheDocument();
  });

  it("switches the automatic claim on in the settings", async () => {
    const backend = await idleDemo();
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    const toggle = await screen.findByRole("switch", { name: zhCN["rewards.auto"] });
    expect(toggle, "off until switched on").toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({ claimRewards: true }),
        }),
      }),
    );
  });

  it("puts the default icon and the plain border on, each with an undo", async () => {
    const backend = await idleDemo({
      use_default_icon: () => ({ previous: 7143, current: 29 }),
      set_icon: ({ icon }: { icon: number }) => ({ previous: 29, current: icon }),
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await openTools(backend);
    await user.click(await screen.findByRole("button", { name: zhCN["identity.useIcon"] }));
    expect(call).toHaveBeenCalledWith("use_default_icon");
    expect(await screen.findByText(zhCN["identity.iconDone"])).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: zhCN["identity.undo"] }));
    expect(call).toHaveBeenCalledWith("set_icon", { icon: 7143 });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: zhCN["identity.undo"] })).toBeNull(),
    );

    await user.click(screen.getByRole("button", { name: zhCN["identity.useCrest"] }));
    expect(call).toHaveBeenCalledWith("use_plain_crest");
    await user.click(await screen.findByRole("button", { name: zhCN["identity.undo"] }));
    expect(call).toHaveBeenCalledWith("set_crest", { crest: { kind: "prestige", level: 7 } });
  });

  it("says when the client keeps the old border, and offers no undo", async () => {
    const kept = { kind: "prestige", level: 7 };
    const { user } = await openTools(
      await idleDemo({ use_plain_crest: () => ({ previous: kept, current: kept }) }),
    );
    await user.click(await screen.findByRole("button", { name: zhCN["identity.useCrest"] }));
    expect(await screen.findByText(zhCN["identity.refused"])).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: zhCN["identity.undo"] })).toBeNull();
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
    // The user's own note on a member: its tag, the text on hover.
    const noted = screen.getByRole("button", { name: "查看 峡谷清道夫#10004 的战绩" });
    expect(within(noted).getByText(zhCN["note.tag.reliable"]).closest("[title]")).toHaveAttribute(
      "title",
      "上次一起打团很靠谱",
    );
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
    // A party read from recent games says so, in words and with a dashed swatch.
    const inferred = screen.getAllByText("疑似开黑 2");
    expect(inferred).toHaveLength(2);
    for (const badge of inferred) {
      expect(badge).toHaveAttribute("title", zhCN["live.premadeInferredHint"]);
      expect(badge.querySelector("[data-group]")).toHaveClass("border-dashed", "border-group-2");
    }
  });

  it("writes, lists and deletes a note on another player, never on the user", async () => {
    const { backend, push } = demoWithEvents();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await push({ type: "openHistory", data: { puuid: "demo-3" } });
    expect(await screen.findByText("野区观光客#10005")).toBeInTheDocument();
    const tags = await screen.findByRole("radiogroup", { name: zhCN["note.tag"] });
    await user.click(within(tags).getByRole("radio", { name: zhCN["note.tag.toxic"] }));
    await user.type(screen.getByRole("textbox", { name: zhCN["note.text"] }), "  打字很凶 ");
    await user.click(screen.getByRole("button", { name: zhCN["note.save"] }));
    expect(call).toHaveBeenCalledWith("set_player_note", {
      puuid: "demo-3",
      tag: "toxic",
      text: "  打字很凶 ",
      name: { gameName: "野区观光客", tagLine: "10005" },
    });
    expect(await screen.findByText(zhCN["note.saved"])).toBeInTheDocument();

    // The list on the Tools page: the newest first, searchable, a name opening the history.
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.tools"] }));
    const list = await screen.findByRole("list", { name: zhCN["note.list"] });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(within(rows[0] as HTMLElement).getByText("野区观光客#10005")).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).getByText(zhCN["note.tag.toxic"])).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: zhCN["note.search"] }), "投降");
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
    await user.click(
      screen.getByRole("button", {
        name: translate("zh-CN", "note.deleteOf", { name: "上等马#10010" }),
      }),
    );
    expect(call).toHaveBeenCalledWith("delete_player_note", { puuid: "demo-7" });
    expect(await screen.findByText(zhCN["note.noMatch"])).toBeInTheDocument();

    // On the user's own history there is nothing to note.
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect((await screen.findAllByText("暗夜里的光#10003")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("radiogroup", { name: zhCN["note.tag"] })).toBeNull();
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
    premadeInferred: false,
    note: null,
    autofilled: false,
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
    recommendations: [],
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

  it("says the client has no room when it refuses the item set", async () => {
    const backend = demoWith({
      write_item_set: () => {
        throw { code: "full", message: "the client's item sets are full" };
      },
    });
    const { user } = await openLive(backend);
    await screen.findByText("数据：腾讯掌上英雄联盟");
    const tabs = screen.getByRole("radiogroup", { name: zhCN["loadout.tabs"] });
    await user.click(within(tabs).getByRole("radio", { name: "出装" }));
    await user.click(screen.getByRole("button", { name: zhCN["loadout.writeItemSet"] }));
    expect(await screen.findByText(zhCN["loadout.itemSetsFull"])).toBeInTheDocument();
  });

  it("shows the pick suggestions with their reasons, and nothing to click in them", async () => {
    const backend = demoWith({
      get_snapshot: () =>
        live({
          phase: "ChampSelect",
          champSelect: {
            ...ranked,
            recommendations: [
              {
                championId: 103,
                score: 0.6,
                reasons: [
                  { kind: "counters", championId: 64, win: 0.54 },
                  { kind: "tier", tier: 1 },
                  { kind: "played", games: 8, wins: 6 },
                  { kind: "inPickList" },
                ],
              },
              { championId: 86, score: 0, reasons: [{ kind: "noData" }] },
            ],
          },
        }),
    });
    await openLive(backend);
    const list = await screen.findByRole("list", { name: zhCN["suggest.title"] });
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    const first = items[0] as HTMLElement;
    expect(await within(first).findByText("克制 李青（54%）")).toBeInTheDocument();
    expect(within(first).getByText("版本 T1")).toBeInTheDocument();
    expect(within(first).getByText("你在这个分路 8 场 6 胜")).toBeInTheDocument();
    expect(within(first).getByText(zhCN["suggest.inPickList"])).toBeInTheDocument();
    expect(within(items[1] as HTMLElement).getByText(zhCN["suggest.noData"])).toBeInTheDocument();
    expect(within(list).queryByRole("button"), "shown only: nothing to hover or lock").toBeNull();
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
    expect(await screen.findByText(/^1L: 👑 峡谷通天代\|/)).toBeInTheDocument();
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

  it("runs the self-check from About and copies it, or shows the text when the clipboard refuses", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.about"] }));
    await user.click(within(dialog).getByRole("button", { name: zhCN["diag.run"] }));
    expect(call).toHaveBeenCalledWith("run_diagnostics");
    const results = await within(dialog).findByRole("list", { name: zhCN["diag.results"] });
    expect(within(results).getByText(/16\.19\.8217343/)).toBeInTheDocument();
    expect(within(results).getAllByRole("listitem")).toHaveLength(9);

    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await user.click(within(dialog).getByRole("button", { name: zhCN["diag.copy"] }));
    expect(writeText).toHaveBeenCalledWith(
      expect.stringContaining("✓ 客户端 · 正常 · 16.19.8217343"),
    );
    expect(within(dialog).queryByRole("textbox", { name: zhCN["diag.copyText"] })).toBeNull();

    writeText.mockRejectedValue(new Error("denied"));
    await user.click(within(dialog).getByRole("button", { name: zhCN["diag.copy"] }));
    const text = await within(dialog).findByRole("textbox", { name: zhCN["diag.copyText"] });
    expect((text as HTMLTextAreaElement).value).toContain("winer 0.2.0 · 诊断");
  });

  it("turns the autofilled pick rule off on the Automation page", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const toggle = await screen.findByRole("switch", { name: zhCN["auto.skipWhenFilled"] });
    expect(toggle).toBeChecked();
    await user.click(toggle);
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            pick: expect.objectContaining({ skipWhenFilled: false }),
          }),
        }),
      }),
    );
  });

  it("switches builds and each mode's data source in settings", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.general"] }));
    const saved = (builds: Record<string, unknown>) =>
      waitFor(() =>
        expect(call).toHaveBeenCalledWith("set_settings", {
          settings: expect.objectContaining({ builds: expect.objectContaining(builds) }),
        }),
      );
    const sources = within(dialog).getByRole("radiogroup", { name: zhCN["loadout.riftSource"] });
    await user.click(within(sources).getByRole("radio", { name: "OP.GG" }));
    await saved({ enabled: true, riftSource: "opGg", aramSource: "opGg" });
    const aram = within(dialog).getByRole("radiogroup", { name: zhCN["loadout.aramSource"] });
    await user.click(within(aram).getByRole("radio", { name: zhCN["loadout.source.off"] }));
    await saved({ aramSource: "off", arenaSource: "opGg", hextechFallback: true });
    await user.click(within(dialog).getByRole("switch", { name: zhCN["loadout.hextechFallback"] }));
    await saved({ aramSource: "off", hextechFallback: false });
    // The augment descriptions sit with the sources now, and still save into General.
    await user.click(within(dialog).getByRole("switch", { name: zhCN["settings.augmentDetails"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          general: expect.objectContaining({ augmentDetails: false }),
        }),
      }),
    );
    await user.click(within(dialog).getByRole("switch", { name: zhCN["loadout.recommend"] }));
    await saved({ recommend: false });
    await user.click(within(dialog).getByRole("switch", { name: zhCN["loadout.builds"] }));
    await saved({ enabled: false, riftSource: "opGg" });
    expect(within(sources).getByRole("radio", { name: "腾讯 101" })).toBeDisabled();
    expect(within(aram).getByRole("radio", { name: "OP.GG" })).toBeDisabled();
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
    expect(enemies.getByText(/^小心【卡兹克】\|档位峡谷通天代\|/)).toBeInTheDocument();
    expect(enemies.getByText(/^对面【亚索】\|档位纯正牛马\|/)).toBeInTheDocument();
    // Callout: beside them the team's own lines, as champ select's, by champion.
    const allies = within(panel().getByRole("region", { name: zhCN["callout.gameTeams.allies"] }));
    expect(allies.getByText("【我方·蓝色方】winer 战绩鉴定")).toBeInTheDocument();
    expect(
      allies.getByText(/^【阿狸】\|档位峡谷通天代\|近20场胜率60%/),
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
        comeback: { shortcut: null, active: false, error: null },
        comebackAll: { shortcut: null, active: false, error: null },
        phrase: { shortcut: null, active: false, error: null },
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
    expect(screen.getByText(/^小心【阿狸】\|档位峡谷通天代\|/)).toBeInTheDocument();
    expect(screen.getByText(/^对面【阿狸】\|档位纯正牛马\|/)).toBeInTheDocument();
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
      "小心【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}",
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

  it("sends the team one message by default, or a message a line when chosen", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const delivery = await screen.findByRole("radiogroup", { name: zhCN["auto.calloutDelivery"] });
    expect(
      within(delivery).getByRole("radio", { name: zhCN["auto.delivery.block"] }),
    ).toBeChecked();
    await user.click(within(delivery).getByRole("radio", { name: zhCN["auto.delivery.lines"] }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({
          automation: expect.objectContaining({
            callout: expect.objectContaining({ delivery: "lines" }),
          }),
        }),
      }),
    );

    const audience = screen.getByRole("radiogroup", { name: zhCN["auto.calloutAudience"] });
    await user.click(within(audience).getByRole("radio", { name: zhCN["auto.audience.me"] }));
    await waitFor(() =>
      expect(
        screen.queryByRole("radiogroup", { name: zhCN["auto.calloutDelivery"] }),
        "shown to the user alone, it is always one message",
      ).toBeNull(),
    );
  });

  it("holds the automatic callout 1.5 s after champ select opens, as long as chosen or not at all", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const delay = await screen.findByRole("switch", { name: zhCN["auto.calloutDelay"] });
    expect(delay, "on by default").toHaveAttribute("aria-checked", "true");
    const wait = screen.getByRole("slider", { name: zhCN["auto.calloutDelayTime"] });
    expect(wait).toHaveAttribute("aria-valuetext", "1.5 秒");
    const saved = (change: Partial<CalloutRule>) =>
      waitFor(() =>
        expect(call).toHaveBeenCalledWith("set_settings", {
          settings: expect.objectContaining({
            automation: expect.objectContaining({ callout: expect.objectContaining(change) }),
          }),
        }),
      );

    fireEvent.change(wait, { target: { value: "3000" } });
    await saved({ delay: true, delayMs: 3000 });
    await user.click(delay);
    await saved({ delay: false });
    expect(
      screen.queryByRole("slider", { name: zhCN["auto.calloutDelayTime"] }),
      "no time to set while it does not wait",
    ).toBeNull();
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
    expect(screen.getByText(/^【阿狸】\|档位峡谷通天代\|近20场胜率60%/)).toBeInTheDocument();
    expect(screen.queryByText("【敌方·红色方】winer 战绩鉴定")).toBeNull();

    await user.click(within(teams).getByRole("radio", { name: "双方" }));
    await saved({ gameTeams: "both" });
    // As one press would: the enemy's three lines, then the team's first line and its best two.
    const enemy = await screen.findByText("【敌方·红色方】winer 战绩鉴定");
    expect(within(enemy.closest("ol") as HTMLElement).getAllByRole("listitem")).toHaveLength(6);
    expect(screen.getByText(/^【阿狸】\|档位人形防御塔\|/)).toBeInTheDocument();
    expect(screen.queryByText(/^峡谷公务员【阿狸】，/), "cut at the limit").toBeNull();

    const ally = screen.getByRole("textbox", { name: zhCN["callout.ally"] });
    expect(ally).toHaveAttribute(
      "placeholder",
      "【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}",
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
    expect(await screen.findByText("第 1–10 场 · 共 60 场")).toBeInTheDocument();
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

  it("loads every available game before summing champions and exporting", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    const views = screen.getByRole("radiogroup", { name: zhCN["history.view"] });
    await user.click(within(views).getByRole("radio", { name: zhCN["history.champions"] }));
    const table = await screen.findByRole("table", { name: zhCN["history.champions"] });
    expect(within(table).getAllByRole("row").length).toBeGreaterThan(1);
    expect(screen.getByText("统计最近可查的 60 场（不含重开局）")).toBeInTheDocument();
    expect(asked(call, "get_match_history", { begin: 50, count: 50 })).toBe(1);

    await user.click(screen.getByRole("button", { name: zhCN["history.exportCsv"] }));
    const exported = call.mock.calls.find(([name]) => name === "save_export");
    expect(exported?.[1]).toMatchObject({ format: "csv" });
    const args = exported?.[1] as { stem: string; contents: string };
    expect(args.stem).toMatch(/^winer 战绩 暗夜里的光#10003 \d{4}-\d{2}-\d{2}$/);
    expect(args.contents.split("\r\n")[0]).toBe(zhCN["history.exportHeader"]);
    expect(await screen.findByText(/已保存到下载文件夹/)).toBeInTheDocument();
  });

  it("reads a busy server's short pages only as far as the list is looked at", async () => {
    localStorage.removeItem("winer.history.pageSize");
    const demo = demoBackend();
    // A busy server: ten games a page, whatever was asked.
    const backend = demoWith({
      get_match_history: (args: { puuid: string; begin: number; count: number }) =>
        demo.call("get_match_history", { ...args, count: Math.min(args.count, 10) }),
    });
    const call = vi.spyOn(backend, "call");
    const begins = () =>
      call.mock.calls
        .filter(([name]) => name === "get_match_history")
        .map(([, args]) => args as { begin: number; count: number })
        .filter((args) => args.count === 50)
        .map((args) => args.begin);
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect(
      await screen.findByText("第 1–10 场 · 战绩服务器繁忙，翻页时再继续读取（已读取 10 场）"),
    ).toBeInTheDocument();
    expect(begins(), "the first page fills the page in view: no more is read").toEqual([0]);

    const pages = screen.getByRole("navigation", { name: zhCN["history.pages"] });
    await user.click(within(pages).getByRole("button", { name: zhCN["history.next"] }));
    expect(
      await screen.findByText("第 11–20 场 · 战绩服务器繁忙，翻页时再继续读取（已读取 20 场）"),
    ).toBeInTheDocument();
    expect(begins(), "each page starts where the last one ended").toEqual([0, 10]);

    // The champion view counts every game: it reads on to the end.
    const views = screen.getByRole("radiogroup", { name: zhCN["history.view"] });
    await user.click(within(views).getByRole("radio", { name: zhCN["history.champions"] }));
    expect(
      await screen.findByRole("table", { name: zhCN["history.champions"] }, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(begins()).toEqual([0, 10, 20, 30, 40, 50, 60]);
  });

  it("says plainly that the match-history server is busy, and asks again on retry", async () => {
    let busy = true;
    const demo = demoBackend();
    const backend = demoWith({
      get_match_history: (args: { puuid: string; begin: number; count: number }) => {
        if (busy) {
          throw {
            code: "unavailable",
            message: "the match-history server is busy: HTTP status server error (503)",
          };
        }
        return demo.call("get_match_history", args);
      },
    });
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    expect(await screen.findByText(zhCN["history.serverBusy"])).toBeInTheDocument();
    expect(screen.getByText(zhCN["history.serverBusyHint"])).toBeInTheDocument();
    expect(screen.queryByText(/HTTP status server error/), "no request line to read").toBeNull();

    busy = false;
    await user.click(screen.getByRole("button", { name: zhCN["common.retry"] }));
    expect(await screen.findByText("第 1–10 场 · 共 60 场")).toBeInTheDocument();
  });

  it("keeps the history search available offline and explains the live credential boundary", async () => {
    const backend = demoWith({ get_snapshot: () => EMPTY_SNAPSHOT });
    const { user, nav } = await renderApp(backend);
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.history"] }));
    const search = screen.getByRole("textbox", { name: zhCN["history.search"] });
    expect(search).toBeInTheDocument();
    expect(
      screen.getByText("未连接客户端时可查看本次运行已缓存的战绩；查询新玩家仍需客户端登录凭证。"),
    ).toBeInTheDocument();
    await user.type(search, "未缓存#10000");
    await user.click(screen.getByRole("button", { name: zhCN["history.find"] }));
    expect(
      await screen.findByText("该玩家没有本次运行的缓存；连接客户端后才能查询。"),
    ).toBeInTheDocument();
  });

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
    expect(rule).toHaveTextContent("取读到的最近 30 场对局里最新的 20 场");
    expect(rule).toHaveTextContent("不只是排位");
    expect(rule).toHaveTextContent("自定义对局（这次跳过 0 场）");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();

    await user.click(screen.getByRole("button", { name: zhCN["history.standingHint"] }));
    const how = screen.getByRole("dialog", { name: zhCN["history.standingHint"] });
    expect(how).toHaveTextContent("峡谷五档在本队五人里按战力排名分档");
    expect(how).toHaveTextContent(`落在「${rating.label}」`);
  });

  it("exports complete scoreboards with rune groups directly to the remembered folder", async () => {
    const saved = vi.fn((_args: unknown) => "D:/Reports/match.png");
    const backend = demoWith({
      get_report_directory: () => "D:/Reports",
      save_report_image: saved,
    });
    const capture = vi.spyOn(reports, "captureReport").mockImplementation(async (node) => {
      expect(node.querySelectorAll("li")).toHaveLength(10);
      expect(node.querySelectorAll('[aria-label="主系符文"]')).toHaveLength(10);
      expect(node.querySelectorAll('[aria-label="属性碎片"]')).toHaveLength(10);
      return new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
    });
    try {
      const user = await openHistory(backend);
      await screen.findByText("D:/Reports");
      const checks = screen.getAllByRole("checkbox", { name: /选择对局/ });
      await user.click(checks[0]!);
      await user.click(checks[1]!);
      await user.click(screen.getByRole("button", { name: zhCN["history.exportToFolder"] }));
      await waitFor(() => expect(saved).toHaveBeenCalledTimes(2));
      expect(capture).toHaveBeenCalledTimes(2);
      expect(saved.mock.calls[0]?.[0]).toEqual(
        expect.objectContaining({
          name: expect.stringMatching(/\.png$/),
          contents: [137, 80, 78, 71, 13, 10, 26, 10],
        }),
      );
    } finally {
      capture.mockRestore();
    }
  });

  it("shows only custom games despite the hide setting and selects across pages", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    await user.type(
      screen.getByRole("textbox", { name: zhCN["history.search"] }),
      "野区观光客#10005",
    );
    await user.click(screen.getByRole("button", { name: zhCN["history.find"] }));
    await screen.findByRole("button", { name: zhCN["history.mine"] }, { timeout: 5000 });
    await screen.findByText("已隐藏 2 场");
    await user.click(screen.getByRole("radio", { name: zhCN["history.custom"] }));
    expect(await screen.findByText("第 1–2 场 · 共 2 场")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: zhCN["history.hideCustom"] })).toBeDisabled();
    await user.click(screen.getByRole("checkbox", { name: zhCN["history.selectVisible"] }));
    expect(screen.getByText("已选择 2 场（跨页保留）")).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox", { name: /选择对局/, checked: true })).toHaveLength(2);
    expect(call).not.toHaveBeenCalledWith("set_settings", expect.anything());
    await user.click(screen.getByRole("radio", { name: zhCN["history.all"] }));
    await user.click(screen.getByRole("checkbox", { name: zhCN["history.selectVisible"] }));
    expect(screen.getByText("已选择 12 场（跨页保留）")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getAllByRole("checkbox", { name: /选择对局/, checked: false })).toHaveLength(10);
    await user.click(screen.getByRole("button", { name: zhCN["history.clearSelection"] }));
    expect(screen.getByRole("button", { name: zhCN["history.exportReports"] })).toBeDisabled();
  });

  it("hides custom games by default, says how many, and shows them once switched", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    await user.click(screen.getByRole("radio", { name: "50" }));
    await user.type(
      screen.getByRole("textbox", { name: zhCN["history.search"] }),
      "野区观光客#10005",
    );
    await user.click(screen.getByRole("button", { name: zhCN["history.find"] }));
    await screen.findByRole("button", { name: zhCN["history.mine"] }, { timeout: 5000 });
    expect(await screen.findByText("已隐藏 2 场")).toBeInTheDocument();
    expect(await screen.findByText("第 1–50 场 · 共 60 场")).toBeInTheDocument();
    expect(screen.queryByText("嚎哭深渊 全随机"), "custom games are hidden").toBeNull();
    await user.click(screen.getByRole("radio", { name: zhCN["history.other"] }));
    await waitFor(() =>
      expect(screen.getAllByText("入门级"), "a game against bots is listed").toHaveLength(1),
    );
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
    await waitFor(() => expect(screen.getAllByText("嚎哭深渊 全随机")).toHaveLength(2));
    await waitFor(() => expect(screen.queryByText("已隐藏 2 场")).toBeNull());
  });

  it("goes back to a player at once, where the list was left, and reopens a scoreboard unasked", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const user = await openHistory(backend);
    const pages = screen.getByRole("navigation", { name: zhCN["history.pages"] });
    await user.click(within(pages).getByRole("button", { name: "下一页" }));
    expect(screen.getByText("第 11–20 场 · 共 60 场")).toBeInTheDocument();
    const game = screen.getAllByRole("button", { pressed: false })[0] as HTMLElement;
    await user.click(game);
    const scoreboard = await screen.findByLabelText(zhCN["history.detail"]);
    const opponent = await within(scoreboard).findAllByRole("button", { name: /·对手/ });
    expect(asked(call, "get_match_detail")).toBe(1);

    await user.click(opponent[0] as HTMLElement);
    await user.click(await screen.findByRole("button", { name: zhCN["history.mine"] }));
    // Drawn from what was shown: no skeleton, the second page, the header.
    expect(screen.getByText("第 11–20 场 · 共 60 场")).toBeInTheDocument();
    expect(screen.getByText("近 20 场 · 所有模式")).toBeInTheDocument();
    const mine = { puuid: "demo-me", begin: 0, count: 50 };
    // The newest games asked for once more, in the background: the same ones, the list stays.
    await waitFor(() => expect(asked(call, "get_match_history", mine)).toBe(2));
    expect(screen.getByText("第 11–20 场 · 共 60 场")).toBeInTheDocument();
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

// The Comebacks page (反击), the Phrases page (快捷语) and its phrases on the Live page, and the QQ
// group in About.
describe("comebacks", () => {
  async function openComebacks(backend: Backend = demoBackend()) {
    const rendered = await renderApp(backend);
    await rendered.user.click(
      within(rendered.nav).getByRole("button", { name: zhCN["nav.comeback"] }),
    );
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["comeback.title"]);
    return rendered;
  }

  async function openPhrases(backend: Backend = demoBackend()) {
    const rendered = await renderApp(backend);
    await rendered.user.click(
      within(rendered.nav).getByRole("button", { name: zhCN["nav.phrases"] }),
    );
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["phrase.title"]);
    return rendered;
  }

  /** The comeback settings the window last asked the core to save include `change`. */
  const saved = (call: { mock: { calls: unknown[][] } }, change: Record<string, unknown>) =>
    waitFor(() =>
      expect(call).toHaveBeenCalledWith("set_settings", {
        settings: expect.objectContaining({ comeback: expect.objectContaining(change) }),
      }),
    );

  /** Waits for the first answer to `command`, and for what the page did with it. */
  async function answered(
    call: { mock: { calls: unknown[][]; results: { value: unknown }[] } },
    command: CommandName,
  ) {
    await waitFor(() => expect(call).toHaveBeenCalledWith(command));
    const index = call.mock.calls.findIndex(([name]) => name === command);
    await act(async () => {
      await call.mock.results[index]?.value;
    });
  }

  /** The demo client, with winer running without administrator rights. */
  async function notElevated() {
    const info = await demoBackend().call("get_app_info");
    return demoWith({ get_app_info: () => ({ ...info, elevated: false }) });
  }

  it("opens from the sidebar and with Ctrl+7, the Phrases page with Ctrl+8, the pages before them keeping their numbers", async () => {
    const { user, nav } = await renderApp();
    const entry = within(nav).getByRole("button", { name: zhCN["nav.comeback"] });
    await user.click(entry);
    const title = screen.getByRole("heading", { level: 1 });
    expect(title).toHaveTextContent(zhCN["comeback.title"]);
    expect(entry).toHaveAttribute("aria-current", "page");
    fireEvent.keyDown(window, { key: "6", ctrlKey: true });
    expect(title).toHaveTextContent(zhCN["plugin.title"]);
    fireEvent.keyDown(window, { key: "4", ctrlKey: true });
    expect(title).toHaveTextContent(zhCN["nav.automation"]);
    fireEvent.keyDown(window, { key: "7", ctrlKey: true });
    expect(title).toHaveTextContent(zhCN["comeback.title"]);
    fireEvent.keyDown(window, { key: "8", ctrlKey: true });
    expect(title).toHaveTextContent(zhCN["phrase.title"]);
    expect(within(nav).getByRole("button", { name: zhCN["nav.phrases"] })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByText(zhCN["error.title"])).toBeNull();
    expect(screen.getByText(/^Ctrl 1–8 切换页面/)).toBeInTheDocument();
  });

  it("adds, edits and removes the user's own lines, saving each change", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    expect(screen.getByText(zhCN["comeback.linesEmpty"])).toBeInTheDocument();
    expect(screen.getByText(/^按一次发一句，随机，不会连着两次同一句。/)).toBeInTheDocument();
    expect(screen.getByText("0 / 100")).toBeInTheDocument();

    const field = screen.getByRole("textbox", { name: zhCN["comeback.newLine"] });
    await user.type(field, "  回泉水歇会儿吧  {Enter}");
    await saved(call, { lines: ["回泉水歇会儿吧"] });
    expect(field, "ready for the next line").toHaveValue("");
    await user.type(field, "下次一定");
    await user.click(
      within(field.closest("form") as HTMLElement).getByRole("button", {
        name: zhCN["list.add"],
      }),
    );
    await saved(call, { lines: ["回泉水歇会儿吧", "下次一定"] });
    expect(screen.getByText("2 / 100")).toBeInTheDocument();
    expect(screen.queryByText(zhCN["comeback.linesEmpty"])).toBeNull();

    const first = screen.getByRole("textbox", { name: "第 1 句" });
    await user.clear(first);
    await user.type(first, "泉水见{Enter}");
    await saved(call, { lines: ["泉水见", "下次一定"] });

    await user.click(screen.getByRole("button", { name: "删除第 2 句" }));
    await saved(call, { lines: ["泉水见"] });
    expect(screen.queryByRole("textbox", { name: "第 2 句" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "第 1 句" })).toHaveValue("泉水见");
  });

  it("records both shortcuts apart from the window's, the callout's and each other's", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    const record = screen.getAllByRole("button", { name: zhCN["social.hotkeyRecord"] });
    expect(
      record,
      "the team's and the all chat's; the quick phrase's is on its own page",
    ).toHaveLength(2);
    const [team] = record;
    expect(screen.getByText(zhCN["comeback.hotkey"])).toBeInTheDocument();
    expect(screen.getByText(zhCN["comeback.allHotkey"])).toBeInTheDocument();

    await user.click(team as HTMLElement);
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: true }));
    fireEvent.keyDown(window, { code: "Backquote", key: "`", altKey: true });
    expect(screen.getByText(zhCN["callout.hotkeyTakenByWindow"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "KeyF", key: "F", ctrlKey: true, shiftKey: true });
    await saved(call, { hotkey: "Ctrl+Shift+F", allHotkey: null });
    await waitFor(() => expect(call).toHaveBeenCalledWith("suspend_hotkey", { suspended: false }));
    expect(await screen.findByText(zhCN["social.hotkeyActive"])).toBeInTheDocument();

    // One recorder at a time: starting the second ends the first, and the shell's shortcuts stay
    // let go until the last one ends.
    await user.click(screen.getByRole("button", { name: zhCN["social.hotkeyChange"] }));
    expect(screen.getAllByText(zhCN["social.hotkeyListening"])).toHaveLength(1);
    const resumed = () =>
      call.mock.calls.filter(
        ([name, args]) =>
          name === "suspend_hotkey" && (args as { suspended: boolean }).suspended === false,
      ).length;
    const before = resumed();
    await user.click(
      screen.getAllByRole("button", { name: zhCN["social.hotkeyRecord"] })[0] as HTMLElement,
    );
    expect(screen.getAllByText(zhCN["social.hotkeyListening"])).toHaveLength(1);
    expect(screen.getByRole("button", { name: zhCN["social.hotkeyChange"] })).toBeInTheDocument();
    expect(resumed(), "still let go for the second recorder").toBe(before);

    fireEvent.keyDown(window, { code: "KeyF", key: "F", ctrlKey: true, shiftKey: true });
    expect(screen.getByText(zhCN["comeback.hotkeyTakenByTeam"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "KeyG", key: "G", ctrlKey: true, shiftKey: true });
    await saved(call, { hotkey: "Ctrl+Shift+F", allHotkey: "Ctrl+Shift+G" });
    await waitFor(() => expect(resumed()).toBe(before + 1));
  });

  it("records the quick phrase's shortcut on its own page, apart from the comebacks', and says when to press it", async () => {
    const backend = demoBackend();
    const settings = await backend.call("get_settings");
    await backend.call("set_settings", {
      settings: { ...settings, comeback: { ...settings.comeback, hotkey: "Alt+5" } },
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await openPhrases(backend);
    expect(screen.getByText(zhCN["phrase.hotkeyHint"])).toBeInTheDocument();
    expect(screen.getByText(zhCN["phrase.spacing"])).toBeInTheDocument();
    const record = screen.getAllByRole("button", { name: zhCN["social.hotkeyRecord"] });
    expect(record, "only the quick phrase's").toHaveLength(1);
    await user.click(record[0] as HTMLElement);
    fireEvent.keyDown(window, { code: "Digit5", key: "5", altKey: true });
    expect(screen.getByText(zhCN["comeback.hotkeyTakenByTeam"])).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "Digit6", key: "6", altKey: true });
    await saved(call, { hotkey: "Alt+5", phraseHotkey: "Alt+6" });
    expect(await screen.findByText(zhCN["social.hotkeyActive"])).toBeInTheDocument();
  });

  it("keeps champ select's quick phrases off the Comebacks page", async () => {
    await openComebacks();
    const page = within(screen.getByRole("main"));
    expect(page.queryByText(zhCN["phrase.list"])).toBeNull();
    expect(page.queryByText(zhCN["phrase.trade"])).toBeNull();
    expect(page.queryByRole("switch", { name: zhCN["phrase.tradeAsk"] })).toBeNull();
    expect(page.queryByText(zhCN["phrase.hotkey"])).toBeNull();
  });

  it("warns that the shortcuts send nothing until there is a line or the library is on", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    expect(screen.getByText(zhCN["comeback.noLinesYet"])).toBeInTheDocument();
    await user.type(
      screen.getByRole("textbox", { name: zhCN["comeback.newLine"] }),
      "泉水见{Enter}",
    );
    await saved(call, { lines: ["泉水见"] });
    expect(screen.queryByText(zhCN["comeback.noLinesYet"])).toBeNull();
  });

  it("types into the game only once switched on, and says what that risks", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    const inGame = screen.getByRole("switch", { name: zhCN["comeback.inGame"] });
    expect(inGame, "off until asked").not.toBeChecked();
    expect(screen.getByText(zhCN["comeback.inGameRisk"])).toBeInTheDocument();
    await user.click(inGame);
    await saved(call, { inGame: true });
    expect(inGame).toBeChecked();
  });

  it("says a game started from WeGame needs winer as administrator, where in-game typing is switched on", async () => {
    const backend = await notElevated();
    const call = vi.spyOn(backend, "call");
    const { user, nav } = await openComebacks(backend);
    const note = (await screen.findByText(zhCN["elevation.gameNote"])).closest("[role=note]");
    expect(note).not.toBeNull();
    await user.click(
      within(note as HTMLElement).getByRole("button", { name: zhCN["connection.relaunch"] }),
    );
    expect(call).toHaveBeenCalledWith("relaunch_elevated");

    // Automation's callout: the same line under its in-game sending switch.
    await user.click(within(nav).getByRole("button", { name: zhCN["nav.automation"] }));
    const section = await screen.findByRole("region", { name: zhCN["callout.gameSection"] });
    expect(await within(section).findByText(zhCN["elevation.gameNote"])).toBeInTheDocument();
    cleanup();

    // winer runs as administrator: nothing to say.
    const elevated = demoBackend();
    const asked = vi.spyOn(elevated, "call");
    await openComebacks(elevated);
    await answered(asked, "get_app_info");
    expect(screen.queryByText(zhCN["elevation.gameNote"])).toBeNull();
  });

  it("edits champ select's quick phrases, previewed for a sample teammate, and goes back to the default", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openPhrases(backend);
    expect(screen.getByText(zhCN["phrase.default"])).toBeInTheDocument();
    const preview = screen.getByRole("list", { name: "预览：发给 5L 玩龙王的 峡谷清道夫" });
    expect(
      within(preview).getByText("5L大佬，小弟想玩下龙王，给您磕一个了"),
      "the default, as the core writes it",
    ).toBeInTheDocument();
    const reset = screen.getByRole("button", { name: zhCN["auto.reset"] });
    expect(reset).toBeDisabled();

    // `{{` types a brace; `{Enter}` adds.
    await user.type(
      screen.getByRole("textbox", { name: zhCN["phrase.new"] }),
      "{{seat}，换【{{champion}】吗，{{name}{Enter}",
    );
    await saved(call, { phrases: ["{seat}，换【{champion}】吗，{name}"] });
    expect(within(preview).getByText("5L，换【龙王】吗，峡谷清道夫")).toBeInTheDocument();
    expect(
      within(preview).queryByText(/给您磕一个了/),
      "the user's own replace the default",
    ).toBeNull();
    expect(screen.queryByText(zhCN["phrase.default"])).toBeNull();
    expect(screen.getByRole("textbox", { name: "第 1 句快捷语" })).toHaveValue(
      "{seat}，换【{champion}】吗，{name}",
    );

    await user.click(reset);
    await saved(call, { phrases: [] });
    expect(within(preview).getByText("5L大佬，小弟想玩下龙王，给您磕一个了")).toBeInTheDocument();
  });

  it("sends the first phrase on a trade and a thank-you once it went through only when switched on", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openPhrases(backend);
    const ask = screen.getByRole("switch", { name: zhCN["phrase.tradeAsk"] });
    const done = screen.getByRole("switch", { name: zhCN["phrase.tradeDone"] });
    expect(ask, "off until asked").not.toBeChecked();
    expect(done, "off until asked").not.toBeChecked();
    expect(
      screen.getByText(zhCN["phrase.tradeFirst"]),
      "the phrase a trade sends is marked",
    ).toBeInTheDocument();
    expect(
      screen.getByText("交换成功后会发：谢谢5L大佬！"),
      "the default thank-you",
    ).toBeInTheDocument();

    await user.click(ask);
    await saved(call, { trade: { onAsk: true, onDone: false, thanks: "" } });
    await user.click(done);
    await saved(call, { trade: { onAsk: true, onDone: true, thanks: "" } });
    expect(ask).toBeChecked();
    expect(done).toBeChecked();

    const thanks = screen.getByRole("textbox", { name: zhCN["phrase.thanks"] });
    expect(thanks).toHaveAttribute("placeholder", "谢谢{seat}大佬！");
    const reset = screen.getByRole("button", { name: zhCN["phrase.thanksReset"] });
    expect(reset).toBeDisabled();
    // `{{` types a brace; `{Enter}` leaves the field, which saves it.
    await user.type(thanks, "感谢{{seat}让出{{champion}{Enter}");
    await saved(call, { trade: { onAsk: true, onDone: true, thanks: "感谢{seat}让出{champion}" } });
    expect(screen.getByText("交换成功后会发：感谢5L让出龙王")).toBeInTheDocument();

    await user.click(reset);
    await saved(call, { trade: { onAsk: true, onDone: true, thanks: "" } });
    expect(screen.getByText("交换成功后会发：谢谢5L大佬！")).toBeInTheDocument();
  });

  it("downloads the online library only after its confirmation, and keeps its switch off until asked", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    expect(screen.getByText(zhCN["comeback.libraryRisk"])).toBeInTheDocument();
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    const use = screen.getByRole("switch", { name: zhCN["comeback.libraryUse"] });
    expect(use, "nothing downloaded to use").toBeDisabled();
    expect(screen.getByText(zhCN["comeback.libraryUseNeedsDownload"])).toBeInTheDocument();

    const download = screen.getByRole("button", { name: zhCN["comeback.libraryDownload"] });
    await user.click(download);
    const dialog = screen.getByRole("dialog", { name: zhCN["comeback.libraryConfirmTitle"] });
    expect(within(dialog).getByText(zhCN["comeback.libraryRisk"])).toBeInTheDocument();
    expect(within(dialog).getByText(zhCN["comeback.libraryOptIn"])).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: zhCN["common.cancel"] }),
      "the safe choice takes the focus",
    ).toHaveFocus();
    expect(call, "opening the confirmation downloads nothing").not.toHaveBeenCalledWith(
      "download_comeback_library",
    );
    await user.click(within(dialog).getByRole("button", { name: zhCN["common.cancel"] }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(call, "nor does cancelling it").not.toHaveBeenCalledWith("download_comeback_library");

    await user.click(download);
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: zhCN["comeback.libraryConfirm"],
      }),
    );
    expect(call).toHaveBeenCalledWith("download_comeback_library");
    expect(
      await screen.findByText(/^已下载：口吐芬芳 62 句，火力全开 7 句 · /),
    ).toBeInTheDocument();
    expect(use, "opted into again after every download").toBeEnabled();
    expect(use).not.toBeChecked();
    await user.click(use);
    await saved(call, { library: { enabled: true, level: "mild" } });
    expect(use).toBeChecked();
    const levels = screen.getByRole("radiogroup", { name: zhCN["comeback.libraryLevel"] });
    await user.click(within(levels).getByRole("radio", { name: zhCN["comeback.level.both"] }));
    await saved(call, { library: { enabled: true, level: "both" } });

    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryRemove"] }));
    expect(call).toHaveBeenCalledWith("remove_comeback_library");
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    expect(use).toBeDisabled();
    expect(use).not.toBeChecked();
    expect(screen.queryByRole("radiogroup", { name: zhCN["comeback.libraryLevel"] })).toBeNull();
  });

  it("says why a download failed and leaves the switch where it was", async () => {
    const backend = demoWith({
      download_comeback_library: () => {
        throw { code: "unavailable", message: "every mirror failed" };
      },
    });
    const { user } = await openComebacks(backend);
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryDownload"] }));
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryConfirm"] }));
    expect(await screen.findByText("下载失败：every mirror failed")).toHaveAttribute(
      "role",
      "alert",
    );
    expect(screen.getByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: zhCN["comeback.libraryUse"] })).toBeDisabled();
  });

  /** Downloads the demo's library from the Comebacks page, through its confirmation. */
  async function downloadLibrary(user: ReturnType<typeof userEvent.setup>) {
    // The button is enabled once the page has read that none is downloaded.
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryDownload"] }));
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryConfirm"] }));
    expect(
      await screen.findByRole("button", { name: zhCN["comeback.libraryEdit"] }),
    ).toBeInTheDocument();
  }

  it("lists the downloaded library by set and page, searches it, and saves every edit", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    await downloadLibrary(user);
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryEdit"] }));
    const dialog = within(
      await screen.findByRole("dialog", { name: zhCN["comeback.libraryEditTitle"] }),
    );
    expect(await dialog.findByRole("radio", { name: "口吐芬芳 62" })).toBeChecked();
    expect(dialog.getByRole("radio", { name: "火力全开 7" })).not.toBeChecked();
    expect(dialog.getByText("第 1–50 句，共 62 句")).toBeInTheDocument();
    expect(dialog.getByRole("textbox", { name: "第 1 句语录" })).toHaveValue("示例语句 1");
    expect(dialog.queryByRole("textbox", { name: "第 51 句语录" })).toBeNull();
    await user.click(dialog.getByRole("button", { name: "第 2 页" }));
    expect(dialog.getByText("第 51–62 句，共 62 句")).toBeInTheDocument();
    expect(dialog.getByRole("textbox", { name: "第 51 句语录" })).toHaveValue("示例语句 51");

    await user.type(
      dialog.getByRole("searchbox", { name: zhCN["comeback.librarySearch"] }),
      "语句 6",
    );
    expect(dialog.getByText("找到 4 句"), "6, 60, 61 and 62").toBeInTheDocument();
    await user.clear(dialog.getByRole("searchbox", { name: zhCN["comeback.librarySearch"] }));
    await user.type(
      dialog.getByRole("searchbox", { name: zhCN["comeback.librarySearch"] }),
      "没有这句",
    );
    expect(dialog.getByText("没有含「没有这句」的语句")).toBeInTheDocument();
    await user.clear(dialog.getByRole("searchbox", { name: zhCN["comeback.librarySearch"] }));

    const saved = (check: (lines: { mild: string[]; strong: string[] }) => boolean) =>
      waitFor(() =>
        expect(
          call.mock.calls.some(
            ([name, args]) =>
              name === "edit_comeback_library" &&
              check((args as { lines: { mild: string[]; strong: string[] } }).lines),
          ),
        ).toBe(true),
      );
    const first = dialog.getByRole("textbox", { name: "第 1 句语录" });
    await user.clear(first);
    await user.type(first, "  回泉水  歇会儿吧 {Enter}");
    await saved((lines) => lines.mild[0] === "  回泉水  歇会儿吧 ");
    await waitFor(() => expect(first, "as the core kept it").toHaveValue("回泉水 歇会儿吧"));

    await user.click(dialog.getByRole("button", { name: "删除第 2 句语录" }));
    await saved((lines) => lines.mild.length === 61 && lines.mild[1] === "示例语句 3");
    expect(dialog.getByRole("radio", { name: "口吐芬芳 61" })).toBeChecked();

    await user.click(dialog.getByRole("radio", { name: "火力全开 7" }));
    await user.type(dialog.getByRole("textbox", { name: "新的火力全开语句" }), "下次一定{Enter}");
    await saved((lines) => lines.strong[0] === "下次一定" && lines.strong.length === 8);
    expect(dialog.getByRole("textbox", { name: "第 1 句语录" })).toHaveValue("下次一定");
    expect(dialog.getByRole("radio", { name: "火力全开 8" })).toBeChecked();

    await user.click(dialog.getByRole("button", { name: zhCN["common.close"] }));
    expect(
      await screen.findByText(/已下载：口吐芬芳 61 句，火力全开 8 句 .* · 已修改/),
    ).toBeInTheDocument();
  });

  it("asks before removing an edited library, and removes an unedited one at once", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await openComebacks(backend);
    await downloadLibrary(user);
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryRemove"] }));
    expect(screen.queryByRole("dialog"), "nothing of the user's to lose").toBeNull();
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();

    await downloadLibrary(user);
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryEdit"] }));
    const editor = within(
      await screen.findByRole("dialog", { name: zhCN["comeback.libraryEditTitle"] }),
    );
    await user.click(await editor.findByRole("button", { name: "删除第 1 句语录" }));
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("edit_comeback_library", expect.anything()),
    );
    await user.click(editor.getByRole("button", { name: zhCN["common.close"] }));
    await screen.findByText(/· 已修改/);

    const removals = () =>
      call.mock.calls.filter(([name]) => name === "remove_comeback_library").length;
    const before = removals();
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryRemove"] }));
    const confirm = within(
      screen.getByRole("dialog", { name: zhCN["comeback.libraryRemoveTitle"] }),
    );
    expect(confirm.getByText(zhCN["comeback.libraryRemoveEdited"])).toBeInTheDocument();
    expect(confirm.getByRole("button", { name: zhCN["common.cancel"] })).toHaveFocus();
    await user.click(confirm.getByRole("button", { name: zhCN["common.cancel"] }));
    expect(removals(), "cancelled").toBe(before);
    expect(screen.getByRole("button", { name: zhCN["comeback.libraryEdit"] })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryRemove"] }));
    await user.click(
      within(screen.getByRole("dialog", { name: zhCN["comeback.libraryRemoveTitle"] })).getByRole(
        "button",
        { name: zhCN["comeback.libraryRemove"] },
      ),
    );
    expect(await screen.findByText(zhCN["comeback.libraryNone"])).toBeInTheDocument();
    expect(removals()).toBe(before + 1);
  });

  /** The demo's backend, with `edit_comeback_library` held: each call waits in `held` until the
   *  test lets it go, then is carried out by the demo. */
  function holdingEdits() {
    const backend = demoBackend();
    const call = backend.call.bind(backend) as (
      command: CommandName,
      ...args: unknown[]
    ) => Promise<unknown>;
    const held: { lines: { mild: string[]; strong: string[] }; release: () => void }[] = [];
    let fail = 0;
    return {
      held,
      /** The next `n` edits are refused once released. */
      failNext: (n: number) => {
        fail = n;
      },
      backend: {
        ...backend,
        call: ((command: CommandName, ...args: unknown[]) => {
          if (command !== "edit_comeback_library") return call(command, ...args);
          const { lines } = args[0] as { lines: { mild: string[]; strong: string[] } };
          return new Promise<unknown>((resolve, reject) => {
            held.push({
              lines,
              release: () => {
                if (fail > 0) {
                  fail -= 1;
                  reject({ code: "internal", message: "the disk is full" });
                } else {
                  resolve(call(command, ...args));
                }
              },
            });
          });
        }) as Backend["call"],
      } satisfies Backend,
    };
  }

  it("keeps the newest edit on screen while the saves before it answer, one after another", async () => {
    const { backend, held } = holdingEdits();
    const { user } = await openComebacks(backend);
    await downloadLibrary(user);
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryEdit"] }));
    const dialog = within(
      await screen.findByRole("dialog", { name: zhCN["comeback.libraryEditTitle"] }),
    );
    const first = await dialog.findByRole("textbox", { name: "第 1 句语录" });
    await user.clear(first);
    await user.type(first, "第一处修改{Enter}");
    await waitFor(() => expect(held).toHaveLength(1));
    const second = dialog.getByRole("textbox", { name: "第 2 句语录" });
    await user.clear(second);
    await user.type(second, "第二处修改{Enter}");
    expect(held, "the second save waits for the first").toHaveLength(1);

    await act(async () => held[0]!.release());
    await waitFor(() => expect(held, "then goes out").toHaveLength(2));
    expect(held[1]!.lines.mild.slice(0, 2), "with both edits").toEqual([
      "第一处修改",
      "第二处修改",
    ]);
    expect(second, "the older answer does not undo the newer edit").toHaveValue("第二处修改");
    expect(first).toHaveValue("第一处修改");

    await act(async () => held[1]!.release());
    await waitFor(() =>
      expect(
        screen.getByText(/已下载：口吐芬芳 62 句，火力全开 7 句 .* · 已修改/),
      ).toBeInTheDocument(),
    );
    expect(second).toHaveValue("第二处修改");
  });

  it("says when the library's lines cannot be read, and shows the kept lines again after a failed save", async () => {
    let reads = 0;
    const holding = holdingEdits();
    const base = holding.backend.call;
    const backend: Backend = {
      ...holding.backend,
      call: ((command: CommandName, ...args: unknown[]) => {
        if (command === "comeback_library_lines" && reads++ === 0) {
          return Promise.reject({ code: "unavailable", message: "the file is locked" });
        }
        return (base as (command: CommandName, ...args: unknown[]) => Promise<unknown>)(
          command,
          ...args,
        );
      }) as Backend["call"],
    };
    const { user } = await openComebacks(backend);
    await downloadLibrary(user);
    await user.click(screen.getByRole("button", { name: zhCN["comeback.libraryEdit"] }));
    const dialog = within(
      await screen.findByRole("dialog", { name: zhCN["comeback.libraryEditTitle"] }),
    );
    expect(await dialog.findByText(zhCN["comeback.libraryLinesFailed"])).toBeInTheDocument();
    expect(dialog.getByText("the file is locked")).toBeInTheDocument();
    await user.click(dialog.getByRole("button", { name: zhCN["common.retry"] }));
    const first = await dialog.findByRole("textbox", { name: "第 1 句语录" });
    expect(first).toHaveValue("示例语句 1");

    holding.failNext(1);
    await user.click(dialog.getByRole("button", { name: "删除第 1 句语录" }));
    await waitFor(() => expect(holding.held).toHaveLength(1));
    expect(dialog.getByRole("radio", { name: "口吐芬芳 61" }), "removed at once").toBeChecked();
    await act(async () => holding.held[0]!.release());
    expect(await screen.findByText("the disk is full")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        dialog.getByRole("radio", { name: "口吐芬芳 62" }),
        "the core's lines again",
      ).toBeChecked(),
    );
    // The field takes a new value from its effect, a moment after the list renders.
    await waitFor(() =>
      expect(dialog.getByRole("textbox", { name: "第 1 句语录" })).toHaveValue("示例语句 1"),
    );
    await user.click(dialog.getByRole("button", { name: zhCN["common.close"] }));
    expect(screen.queryByText(/· 已修改/), "nothing was saved").toBeNull();
  });

  it("sends a teammate a quick phrase from champ select's board, for that champ select's cell", async () => {
    // Saved in the demo client, which renders the phrase as the core does.
    const backend = demoBackend();
    const settings = await backend.call("get_settings");
    await backend.call("set_settings", {
      settings: {
        ...settings,
        comeback: { ...settings.comeback, phrases: ["{seat}换个英雄吗", "谢谢{name}"] },
      },
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    expect(
      screen.queryByRole("button", { name: "给 暗夜里的光#10003 发快捷语" }),
      "none for the user's own seat",
    ).toBeNull();
    await user.click(screen.getByRole("button", { name: "给 峡谷清道夫#10004 发快捷语" }));
    const menu = screen.getByRole("menu", { name: zhCN["live.phrases"] });
    // As the core fills them for that seat: the second in the team's list.
    expect(await within(menu).findByRole("menuitem", { name: "2L换个英雄吗" })).toBeInTheDocument();
    await user.click(within(menu).getByRole("menuitem", { name: "谢谢峡谷清道夫" }));
    expect(call).toHaveBeenCalledWith("send_phrase", { index: 1, gameId: 1, cellId: 1 });
    expect(await screen.findByText("已发送：谢谢峡谷清道夫")).toBeInTheDocument();
    expect(screen.queryByRole("menu")).toBeNull();

    // The menu's link opens the page the phrases are edited on.
    await user.click(screen.getByRole("button", { name: "给 峡谷清道夫#10004 发快捷语" }));
    await user.click(screen.getByRole("button", { name: zhCN["live.phrasesEdit"] }));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(zhCN["phrase.title"]);
    expect(screen.getByRole("textbox", { name: "第 1 句快捷语" })).toHaveValue("{seat}换个英雄吗");
  });

  it("offers the default phrase with the teammate's champion, and says when the core refuses it", async () => {
    const backend = demoWith({
      send_phrase: () => {
        throw { code: "invalid", message: "that champ select is over" };
      },
    });
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    await user.click(await screen.findByRole("button", { name: zhCN["overview.open"] }));
    await user.click(screen.getByRole("button", { name: "给 补刀不漏一个#10006 发快捷语" }));
    await user.click(
      await screen.findByRole("menuitem", { name: "4L大佬，小弟想玩下金克丝，给您磕一个了" }),
    );
    expect(call).toHaveBeenCalledWith("send_phrase", { index: 0, gameId: 1, cellId: 3 });
    expect(await screen.findByText("that champ select is over")).toBeInTheDocument();
  });

  it("joins the QQ group from About, or copies its number", async () => {
    const backend = demoBackend();
    const call = vi.spyOn(backend, "call");
    const { user } = await renderApp(backend);
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    const dialog = screen.getByRole("dialog", { name: zhCN["settings.title"] });
    await user.click(within(dialog).getByRole("tab", { name: zhCN["settings.about"] }));
    expect(within(dialog).getByText(zhCN["settings.community"])).toBeInTheDocument();
    expect(
      within(dialog).getByText("winer交流群 · 群号 1109319256：反馈问题、交流使用"),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: zhCN["settings.communityJoin"] }));
    expect(call, "the shell opens the group's own address").toHaveBeenCalledWith("open_community");

    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await user.click(within(dialog).getByRole("button", { name: zhCN["settings.communityCopy"] }));
    expect(writeText).toHaveBeenCalledWith("1109319256");
    expect(await screen.findByText("已复制群号 1109319256")).toBeInTheDocument();

    writeText.mockRejectedValue(new Error("denied"));
    await user.click(within(dialog).getByRole("button", { name: zhCN["settings.communityCopy"] }));
    expect(await screen.findByText("无法写入剪贴板，群号是 1109319256")).toBeInTheDocument();
  });
});
