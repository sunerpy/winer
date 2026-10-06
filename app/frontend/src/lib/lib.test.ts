import {
  GRADE_LETTERS,
  type AugmentDetail,
  type Event,
  type PlayerLine,
  type Settings,
  type Snapshot,
  type Update,
} from "@winer/shared";
import { describe, expect, it, vi } from "vitest";

import { applyAppearance, modeOf, resolveTheme } from "./appearance";
import type { Backend } from "./backend";
import { errorCode, errorMessage } from "./backend";
import { shouldSuppressContextMenu } from "./contextMenu";
import { translate } from "./i18n";
import { en } from "./i18n/en";
import { zhCN } from "./i18n/zh-CN";
import { defaultScopes } from "./modes";
import { detectPlatform } from "./platform";
import { AppStore, EMPTY_SNAPSHOT, applyPatch, catalogOf } from "./store";
import { SCHEMES, TIER_NAMES, gameTitle } from "./tiers";

const SETTINGS = {
  appearance: {
    theme: "hextech",
    accent: "default",
    density: "comfortable",
    fontSize: 13,
    reduceMotion: false,
  },
  general: {
    closeToTray: true,
    language: "zh-CN",
    augmentDetails: true,
    titles: true,
    hotkey: "Alt+Backquote",
  },
  automation: {
    accept: { enabled: false, delayMs: 1500 },
    pick: {
      enabled: false,
      lockIn: true,
      declareIntent: true,
      champions: { any: [], top: [], jungle: [], middle: [], bottom: [], utility: [] },
    },
    ban: {
      enabled: false,
      champions: { any: [], top: [], jungle: [], middle: [], bottom: [], utility: [] },
    },
    playAgain: false,
    callout: {
      auto: false,
      audience: "team",
      includeSelf: true,
      header: "",
      template: "",
      tiers: "horsesFive",
      customTiers: [],
      hotkey: null,
      inGame: false,
      watchTemplate: "",
      targetTemplate: "",
    },
    bench: { enabled: false, champions: [] },
    scopes: defaultScopes(),
    loadout: { enabled: false, recommended: true },
    itemSets: false,
  },
  plugin: {
    auto: true,
    teamPanel: true,
    hidePromotions: false,
    benchNoCooldown: true,
    loaderDir: null,
    friendStatus: true,
    lobbyPanel: true,
    // The history panel in the client.
    historyInClient: true,
  },
  profile: {
    rankDisguise: { enabled: false, queue: "solo", tier: "DIAMOND", division: "I" },
    presence: { remember: false, availability: "chat", statusMessage: null, mobileMessage: false },
  },
  builds: { enabled: true, riftSource: "tencent" },
  history: { hideCustomGames: true },
} satisfies Settings;

/** A backend whose events the test fires by hand, with each command's answer settable. */
function fakeBackend(answers: Partial<Record<string, unknown>> = {}) {
  let emit: (event: Event) => void = () => undefined;
  const calls: [string, unknown][] = [];
  const backend: Backend = {
    call: vi.fn(async (command: string, args?: unknown) => {
      calls.push([command, args]);
      const answer = answers[command];
      if (answer instanceof Error) throw answer;
      return typeof answer === "function" ? (answer as (args: unknown) => unknown)(args) : answer;
    }) as Backend["call"],
    onEvent: (handler) => {
      emit = handler;
      return () => undefined;
    },
    onResync: () => () => undefined,
    onUpdate: () => () => undefined,
    onHotkey: () => () => undefined,
  };
  return { backend, calls, emit: (event: Event) => emit(event) };
}

describe("store", () => {
  it("replaces exactly the patched field", () => {
    const next = applyPatch(EMPTY_SNAPSHOT, { key: "phase", value: "ChampSelect" });
    expect(next).toEqual({ ...EMPTY_SNAPSHOT, phase: "ChampSelect" });
    expect(next.connection).toBe(EMPTY_SNAPSHOT.connection);
  });

  it("holds updates that arrive before the snapshot and applies only the newer ones", async () => {
    let release: (snapshot: Snapshot) => void = () => undefined;
    const pending = new Promise<Snapshot>((resolve) => (release = resolve));
    const fake = fakeBackend({
      get_snapshot: () => pending,
      get_settings: SETTINGS,
      get_update_status: { state: "idle" },
      get_game_data: null,
    });
    const store = new AppStore(fake.backend);
    const started = store.start();

    const update = (rev: number, phase: Snapshot["phase"]): Event => ({
      type: "update",
      data: { rev, patch: { key: "phase", value: phase } } satisfies Update,
    });
    fake.emit(update(4, "Lobby"));
    fake.emit(update(6, "ChampSelect"));
    release({ ...EMPTY_SNAPSHOT, rev: 5, phase: "None" });
    await started;

    expect(store.live.get()).toMatchObject({ rev: 6, phase: "ChampSelect" });
    fake.emit(update(6, "InProgress"));
    expect(store.live.get().phase).toBe("ChampSelect");
  });

  it("puts settings back when the core refuses them", async () => {
    const fake = fakeBackend({ set_settings: new Error("disk full") });
    const store = new AppStore(fake.backend);
    store.settings.set(SETTINGS);
    const changed = { ...SETTINGS, general: { ...SETTINGS.general, closeToTray: false } };
    await expect(store.saveSettings(changed)).rejects.toThrow("disk full");
    expect(store.settings.get()).toBe(SETTINGS);
  });

  it("builds each settings change on the newest value", async () => {
    const fake = fakeBackend({ set_settings: ({ settings }: { settings: Settings }) => settings });
    const store = new AppStore(fake.backend);
    store.settings.set(SETTINGS);
    await Promise.all([
      store.updateSettings((settings) => ({
        ...settings,
        automation: { ...settings.automation, playAgain: true },
      })),
      store.updateSettings((settings) => ({
        ...settings,
        general: { ...settings.general, closeToTray: false },
      })),
    ]);
    expect(store.settings.get()?.automation.playAgain).toBe(true);
    expect(store.settings.get()?.general.closeToTray).toBe(false);
  });

  it("saves one edit at a time and never lets an older answer undo a newer edit", async () => {
    const sent: Settings[] = [];
    const answer: ((settings: Settings) => void)[] = [];
    const fake = fakeBackend({
      get_snapshot: EMPTY_SNAPSHOT,
      get_settings: SETTINGS,
      get_update_status: { state: "idle" },
      get_game_data: null,
      set_settings: ({ settings }: { settings: Settings }) => {
        sent.push(settings);
        return new Promise<Settings>((resolve) => answer.push(resolve));
      },
    });
    const store = new AppStore(fake.backend);
    await store.start();

    const first = store.updateSettings((value) => ({
      ...value,
      general: { ...value.general, closeToTray: false },
    }));
    const second = store.updateSettings((value) => ({
      ...value,
      automation: { ...value.automation, playAgain: true },
    }));
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(store.settings.get()?.automation.playAgain, "the second edit shows at once").toBe(true);

    answer[0]?.(sent[0] as Settings);
    await vi.waitFor(() => expect(sent).toHaveLength(2), { timeout: 1000 });
    fake.emit({ type: "settings", data: sent[0] as Settings });
    expect(
      store.settings.get()?.automation.playAgain,
      "the first answer and its broadcast do not undo it",
    ).toBe(true);

    answer[1]?.(sent[1] as Settings);
    await Promise.all([first, second]);
    expect(sent[1]).toMatchObject({
      general: { closeToTray: false },
      automation: { playAgain: true },
    });
    expect(store.settings.get()).toEqual(sent[1]);
  });

  it("reads augment descriptions once, and again when the switch or the language moves", async () => {
    let asked = 0;
    const fake = fakeBackend({
      get_snapshot: EMPTY_SNAPSHOT,
      get_settings: SETTINGS,
      get_update_status: { state: "idle" },
      get_game_data: null,
      get_augment_details: () => {
        asked += 1;
        return [{ id: 1004, description: "封印终极技能" }];
      },
    });
    const store = new AppStore(fake.backend);
    await store.start();
    await Promise.all([store.loadAugmentDetails(), store.loadAugmentDetails()]);
    expect(store.augmentDetails.get()?.get(1004)).toBe("封印终极技能");
    await store.loadAugmentDetails();
    expect(asked, "one request, however many augments ask").toBe(1);

    fake.emit({
      type: "settings",
      data: { ...SETTINGS, general: { ...SETTINGS.general, language: "en" } },
    });
    expect(store.augmentDetails.get()).toBeNull();
  });

  it("drops descriptions that answer a language the user has since left", async () => {
    const answers: ((details: AugmentDetail[]) => void)[] = [];
    const fake = fakeBackend({
      get_snapshot: EMPTY_SNAPSHOT,
      get_settings: SETTINGS,
      get_update_status: { state: "idle" },
      get_game_data: null,
      get_augment_details: () => new Promise<AugmentDetail[]>((resolve) => answers.push(resolve)),
    });
    const store = new AppStore(fake.backend);
    await store.start();
    const stale = store.loadAugmentDetails();
    fake.emit({
      type: "settings",
      data: { ...SETTINGS, general: { ...SETTINGS.general, language: "en" } },
    });
    const fresh = store.loadAugmentDetails();
    expect(answers, "the new language is asked for at once").toHaveLength(2);

    answers[1]?.([{ id: 1004, description: "Your ultimate is sealed." }]);
    await fresh;
    answers[0]?.([{ id: 1004, description: "封印终极技能" }]);
    await stale;
    expect(store.augmentDetails.get()?.get(1004)).toBe("Your ultimate is sealed.");
  });

  it("offers each champion once, by the name pickers show, and still names a mode's copy", () => {
    const champion = (id: number, name: string, shortName: string, alias: string) => ({
      id,
      name,
      shortName,
      alias,
      icon: "",
    });
    const catalog = catalogOf({
      champions: [
        champion(1, "黑暗之女", "安妮", "Annie"),
        champion(60001, "黑暗之女", "安妮", "Jade_Annie"),
        champion(103, "九尾妖狐", "阿狸", "Ahri"),
      ],
      items: [],
      spells: [],
      perks: [],
      queues: [],
      augments: [],
    });
    expect(catalog.championList.map((entry) => entry.id)).toEqual([103, 1]);
    expect(catalog.champions.get(60001)?.shortName).toBe("安妮");
  });

  it("keeps the thirty newest notices", () => {
    const store = new AppStore(fakeBackend().backend);
    for (let at = 0; at < 35; at += 1) store.pushNotice({ at, kind: { kind: "accepted" } });
    expect(store.notices.get()).toHaveLength(30);
    expect(store.notices.get()[0]?.at).toBe(34);
  });
});

describe("tiers", () => {
  const player = (change: Partial<PlayerLine> = {}): PlayerLine => ({
    puuid: "p",
    name: null,
    iconId: 1,
    championId: 1,
    championLevel: 18,
    position: null,
    spells: [4, 14],
    items: [0, 0, 0, 0, 0, 0, 0],
    augments: [],
    keystone: 0,
    subStyle: 0,
    kills: 5,
    deaths: 5,
    assists: 5,
    cs: 100,
    gold: 10_000,
    damage: 20_000,
    damageTaken: 20_000,
    vision: 10,
    largestMultiKill: 1,
    win: true,
    remake: false,
    placement: null,
    damageShare: 0.2,
    killParticipation: 0.6,
    score: 6,
    grade: 3,
    award: null,
    feats: [],
    ...change,
  });
  /** `change` on one line of a team whose other four are all the same middling line. */
  const title = (change: Partial<PlayerLine>) => {
    const line = player(change);
    return gameTitle(line, [line, player(), player(), player(), player()]);
  };

  it("names a game's most telling title first, and nothing for a middling line", () => {
    expect(title({})).toBeNull();
    expect(title({ kills: 1, deaths: 8 })).toBe("bodhisattva");
    expect(title({ deaths: 0, kills: 6, assists: 6 })).toBe("immortal");
    expect(title({ position: "bottom", deaths: 2, damageShare: 0.3 })).toBe("carryAlive");
    expect(title({ position: "bottom", deaths: 9 })).toBe("carryFeeding");
    expect(title({ win: false, damageShare: 0.32 })).toBe("dean");
    expect(title({ damageShare: 0.1 })).toBe("puzzle");
    expect(title({ kills: 10, deaths: 10 })).toBe("trader");
    expect(title({ deaths: 11 })).toBe("greyScreen");
    expect(title({ deaths: 1, assists: 10, damageShare: 0.14 })).toBe("kSaver");
    expect(title({ damageTaken: 40_000 })).toBe("turret");
    expect(title({ gold: 14_000, damageShare: 0.16 })).toBe("banker");
    expect(title({ gold: 6_000, damageShare: 0.26 })).toBe("underdog");
    expect(title({ kills: 4, assists: 15 })).toBe("helper");
    expect(title({ killParticipation: 0.3 })).toBe("solo");
    expect(title({ remake: true, kills: 1, deaths: 8 }), "a remake earns nothing").toBeNull();
    expect(gameTitle(player({ kills: 1, deaths: 8 }), [player()]), "nor a team of one").toBeNull();
  });

  it("names as many tiers as the core's sets have, a grade for every letter", () => {
    const counts = Object.fromEntries(
      SCHEMES.filter((scheme) => scheme !== "custom").map((scheme) => [
        scheme,
        TIER_NAMES[scheme as Exclude<typeof scheme, "custom">]["zh-CN"].length,
      ]),
    );
    expect(counts).toEqual({
      riftFive: 5,
      grades: 8,
      horseUniverse: 7,
      horsesFive: 5,
      horses: 3,
      rift: 5,
    });
    expect(TIER_NAMES.grades.en).toHaveLength(GRADE_LETTERS.length);
    expect(SCHEMES[0], "the default comes first").toBe("riftFive");
  });
});

describe("i18n", () => {
  it("fills parameters and leaves unknown ones visible", () => {
    expect(translate("zh-CN", "common.level", { level: 30 })).toBe("30 级");
    expect(translate("en", "live.premade", {})).toBe("Party {n}");
  });

  it("has every key in both languages, none empty", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(zhCN).sort());
    for (const catalog of [en, zhCN])
      expect(Object.values(catalog).filter((text) => text.trim() === "")).toEqual([]);
  });
});

describe("appearance", () => {
  it("resolves the system theme and its mode", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(modeOf("hextech")).toBe("dark");
    expect(modeOf("light")).toBe("light");
  });

  it("writes the attributes every token keys on, and the zoom", () => {
    const root = document.createElement("html");
    applyAppearance(
      { ...SETTINGS.appearance, theme: "light", accent: "teal", fontSize: 15 },
      false,
      root,
    );
    expect(root.dataset).toMatchObject({
      theme: "light",
      mode: "light",
      accent: "teal",
      density: "comfortable",
      reduceMotion: "false",
    });
    expect(Number(root.style.getPropertyValue("--ui-zoom"))).toBeCloseTo(15 / 13);
  });
});

describe("platform and chrome", () => {
  it("screens mobile first and never reads WebKitGTK as macOS", () => {
    expect(detectPlatform("Mozilla/5.0 (Linux; Android 14)")).toBe("unknown");
    expect(
      detectPlatform("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 Safari/605.1.15"),
    ).toBe("linux");
    expect(detectPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/140")).toBe("windows");
    expect(detectPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe("macos");
  });

  it("keeps the context menu only where text is edited", () => {
    const input = document.createElement("input");
    const editable = document.createElement("div");
    editable.contentEditable = "true";
    expect(shouldSuppressContextMenu(input)).toBe(false);
    expect(shouldSuppressContextMenu(document.createElement("button"))).toBe(true);
    expect(shouldSuppressContextMenu(null)).toBe(true);
  });

  it("reads IPC errors and plain ones alike", () => {
    expect(errorMessage({ code: "notConnected", message: "no client" })).toBe("no client");
    expect(errorCode({ code: "notFound", message: "x" })).toBe("notFound");
    expect(errorMessage(new Error("boom"))).toBe("boom");
    expect(errorCode("text")).toBeNull();
  });
});
