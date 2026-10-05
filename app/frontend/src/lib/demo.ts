// A believable client for `pnpm dev` in a browser: one summoner in champ select with a full team,
// twenty games of history and a working settings round trip. Never part of a release bundle.
import type {
  ChampSelectView,
  Event,
  Feat,
  GameData,
  MatchDetail,
  MatchPage,
  MatchSummary,
  PlayerLine,
  PlayerSummary,
  PluginStatus,
  Rank,
  RecentMatch,
  Seat,
  Settings,
  Snapshot,
  UpdateStatus,
} from "@winer/shared";

import type { ArgsOf, Backend, CommandName, Commands } from "./backend";
import { FEAT_ORDER } from "./feats";
import { defaultScopes } from "./modes";
import { TIER_NAMES } from "./tiers";

const CHAMPIONS: [number, string, string, string][] = [
  [1, "黑暗之女", "安妮", "Annie"],
  [22, "寒冰射手", "艾希", "Ashe"],
  [64, "盲僧", "李青", "LeeSin"],
  [86, "德玛西亚之力", "盖伦", "Garen"],
  [103, "九尾妖狐", "阿狸", "Ahri"],
  [157, "疾风剑豪", "亚索", "Yasuo"],
  [222, "暴走萝莉", "金克丝", "Jinx"],
  [412, "魂锁典狱长", "锤石", "Thresh"],
  [238, "影流之主", "劫", "Zed"],
  [89, "曙光女神", "蕾欧娜", "Leona"],
  [11, "无极剑圣", "易", "MasterYi"],
  [121, "虚空掠夺者", "卡兹克", "Khazix"],
  [266, "暗裔剑魔", "亚托克斯", "Aatrox"],
  [99, "光辉女郎", "拉克丝", "Lux"],
  [81, "探险家", "伊泽瑞尔", "Ezreal"],
];

const GAME_DATA: GameData = {
  champions: CHAMPIONS.map(([id, name, shortName, alias]) => ({
    id,
    name,
    shortName,
    alias,
    icon: "",
  })),
  items: [],
  spells: [],
  perks: [],
  augments: [
    { id: 1004, name: "回归基本功", icon: "", rarity: "prismatic" },
    { id: 2103, name: "狙神飞星", icon: "", rarity: "gold" },
    { id: 1116, name: "闪现向前", icon: "", rarity: "gold" },
    { id: 2102, name: "高压锅", icon: "", rarity: "silver" },
  ],
  queues: [
    { id: 420, name: "排位赛 单排/双排", gameMode: "CLASSIC", ranked: true },
    { id: 440, name: "排位赛 灵活排位", gameMode: "CLASSIC", ranked: true },
    { id: 430, name: "匹配模式", gameMode: "CLASSIC", ranked: false },
    { id: 450, name: "极地大乱斗", gameMode: "ARAM", ranked: false },
    { id: 2400, name: "海克斯大乱斗", gameMode: "KIWI", ranked: false },
  ],
};

/** A deterministic sequence, so the demo looks the same on every reload. */
function random(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state / 2_147_483_648;
  };
}

const NOW = Date.now();
const champion = (pick: () => number) => CHAMPIONS[Math.floor(pick() * CHAMPIONS.length)]?.[0] ?? 1;

/** Stand-in for the core's score (`rating.rs`): enough to tell a good line from a bad one. */
function standIn(line: PlayerLine): number {
  const value = 3 + (line.kills + line.assists) / Math.max(1, line.deaths) + line.damage / 20_000;
  return Math.round(Math.min(10, value) * 10) / 10;
}

/** The core's single-game bands (`rating::GAME_GRADES`), S+ to E; below them F. */
const GAME_GRADES = [9, 8, 7, 6, 5, 4, 3];
const gameGrade = (score: number) => {
  const grade = GAME_GRADES.findIndex((floor) => score >= floor);
  return grade === -1 ? GAME_GRADES.length : grade;
};

const MULTIKILLS: Feat[] = ["double", "triple", "quadra", "penta"];
/** The feats a line earns on its own, here only its multikill (`analysis::own_feats`). */
const ownFeats = (line: PlayerLine): Feat[] =>
  line.largestMultiKill >= 2
    ? [MULTIKILLS[Math.min(line.largestMultiKill, 5) - 2] ?? "double"]
    : [];
const byOrder = (feats: Feat[]) =>
  [...new Set(feats)].sort((a, b) => FEAT_ORDER.indexOf(a) - FEAT_ORDER.indexOf(b));
/** The game's leads, as the core names them: the best of each stat where somebody leads. */
const LEADS: [Feat, (line: PlayerLine) => number][] = [
  ["mostKills", (line) => line.kills],
  ["mostDamage", (line) => line.damage],
  ["mostAssists", (line) => line.assists],
  ["mostGold", (line) => line.gold],
  ["mostTaken", (line) => line.damageTaken],
  ["mostCs", (line) => line.cs],
];

function line(
  pick: () => number,
  puuid: string,
  name: string,
  win: boolean,
  championId: number,
): PlayerLine {
  const kills = Math.floor(pick() * 12);
  const deaths = Math.floor(pick() * 9);
  return {
    puuid,
    name: { gameName: name, tagLine: String(10_000 + Math.floor(pick() * 89_999)) },
    iconId: 29,
    championId,
    championLevel: 12 + Math.floor(pick() * 6),
    position: null,
    spells: [4, 14],
    items: [3031, 3006, 3094, 0, 0, 0, 3340],
    augments: [],
    keystone: 8005,
    subStyle: 8100,
    kills,
    deaths,
    assists: Math.floor(pick() * 14),
    cs: 120 + Math.floor(pick() * 120),
    gold: 9000 + Math.floor(pick() * 6000),
    damage: 9000 + Math.floor(pick() * 26_000),
    damageTaken: 12_000 + Math.floor(pick() * 15_000),
    vision: 10 + Math.floor(pick() * 30),
    largestMultiKill: pick() > 0.85 ? 3 : 1,
    win,
    remake: false,
    placement: null,
    damageShare: null,
    killParticipation: null,
    score: null,
    grade: null,
    award: null,
    feats: [],
  };
}

/** The demo player's games of ten that stood out, so a page of their history shows the feats the
 *  core names: the numbers that earn a lead, and the feats a line earns alone. */
const STANDOUT: ({ line: Partial<PlayerLine>; own: Feat[] } | undefined)[] = [
  { line: { largestMultiKill: 3, kills: 14, damage: 52_000 }, own: [] },
  undefined,
  { line: {}, own: ["firstBlood"] },
  { line: { largestMultiKill: 2, assists: 24 }, own: [] },
  undefined,
  {
    line: { largestMultiKill: 5, kills: 19, deaths: 2, gold: 19_500, damage: 48_000 },
    own: ["legendary"],
  },
  { line: { damageTaken: 44_000 }, own: [] },
  undefined,
  { line: { largestMultiKill: 4, kills: 15, cs: 268 }, own: [] },
  { line: { kills: 16 }, own: ["firstBlood"] },
];
const DEMO_PLAYER = "demo-me";

/** One game's scoreboard, scored and awarded as the core would; the demo player's line is the
 *  one their history list shows for the game. */
function scoreboard(gameId: number): MatchDetail {
  const pick = random(gameId % 1000);
  const won = pick() > 0.45;
  const standout = STANDOUT[(gameId % 100) % STANDOUT.length];
  const team = (teamId: number, win: boolean) => ({
    teamId,
    win,
    bans: teamId === 100 ? [157, 238, 11] : [121, 266, 99],
    kills: 0,
    gold: 0,
    towers: win ? 9 : 3,
    dragons: win ? 3 : 1,
    barons: win ? 1 : 0,
    players: TEAM.map(([puuid, name]) => {
      const played = line(
        pick,
        teamId === 100 ? puuid : `${puuid}-x`,
        teamId === 100 ? name : `${name}·对手`,
        win,
        champion(pick),
      );
      if (played.puuid !== DEMO_PLAYER) return played;
      return { ...played, name: { gameName: name, tagLine: "10003" }, ...standout?.line };
    }),
  });
  const teams = [team(100, won), team(200, !won)].map((value) => {
    const kills = value.players.reduce((sum, player) => sum + player.kills, 0);
    const damage = value.players.reduce((sum, player) => sum + player.damage, 0);
    const players = value.players.map((player) => {
      const score = standIn(player);
      return {
        ...player,
        damageShare: damage > 0 ? player.damage / damage : null,
        killParticipation: kills > 0 ? Math.min(1, (player.kills + player.assists) / kills) : null,
        score,
        grade: gameGrade(score),
      };
    });
    const best = Math.max(...players.map((player) => player.score));
    return {
      ...value,
      kills,
      gold: value.players.reduce((sum, player) => sum + player.gold, 0),
      players: players.map((player, index) => ({
        ...player,
        award:
          index === players.findIndex((other) => other.score === best)
            ? value.win
              ? ("mvp" as const)
              : ("svp" as const)
            : null,
      })),
    };
  });
  const lines = teams.flatMap((team) => team.players);
  for (const [feat, read] of LEADS) {
    const best = Math.max(...lines.map(read));
    for (const line of lines) if (read(line) === best) line.feats.push(feat);
  }
  for (const line of lines) {
    const own = line.puuid === DEMO_PLAYER ? (standout?.own ?? []) : [];
    line.feats = byOrder([...ownFeats(line), ...own, ...line.feats]);
  }
  return {
    gameId,
    queueId: 420,
    gameMode: "CLASSIC",
    gameVersion: "16.19.821.7343",
    startedAt: NOW - 3_600_000,
    duration: 1745,
    teams,
  };
}

function history(puuid: string, name: string, seed: number, count: number): MatchSummary[] {
  const pick = random(seed);
  return Array.from({ length: count }, (_, index) => {
    const win = pick() > 0.45;
    const queueId = [420, 2400, 430, 2400, 440][Math.floor(pick() * 5)] ?? 420;
    const gameId = 9_000_000_000 + seed * 100 + index;
    const own = line(pick, puuid, name, win, champion(pick));
    const score = standIn(own);
    // The demo player's games are taken from their scoreboards, as the core's summaries are; the
    // others' are drawn here, every fourth one the best of its side.
    const played =
      puuid === DEMO_PLAYER
        ? (scoreboard(gameId)
            .teams.flatMap((team) => team.players)
            .find((player) => player.puuid === puuid) ?? own)
        : {
            ...own,
            score,
            grade: gameGrade(score),
            award: index % 4 === 1 ? (win ? ("mvp" as const) : ("svp" as const)) : null,
            feats: ownFeats(own),
          };
    return {
      gameId,
      queueId,
      gameMode: queueId === 2400 ? "KIWI" : "CLASSIC",
      startedAt: NOW - (index + 1) * 3_600_000 * (1 + pick() * 5),
      duration: 1200 + Math.floor(pick() * 900),
      // Hextech ARAM is played with augments instead of runes.
      line: queueId === 2400 ? { ...played, augments: [1004, 2103, 1116, 2102] } : played,
    };
  });
}

function summary(puuid: string, name: string, seed: number, rank: Rank | null): PlayerSummary {
  const games = history(puuid, name, seed, 20);
  const matches: RecentMatch[] = games.map((game) => ({
    gameId: game.gameId,
    queueId: game.queueId,
    championId: game.line.championId,
    win: game.line.win,
    remake: false,
    kills: game.line.kills,
    deaths: game.line.deaths,
    assists: game.line.assists,
    startedAt: game.startedAt,
  }));
  const wins = matches.filter((game) => game.win).length;
  const first = matches[0];
  let streak = 0;
  for (const game of matches) {
    if (game.win !== first?.win) break;
    streak += 1;
  }
  const pool = new Map<number, { championId: number; games: number; wins: number }>();
  for (const game of matches) {
    const entry = pool.get(game.championId) ?? { championId: game.championId, games: 0, wins: 0 };
    entry.games += 1;
    entry.wins += game.win ? 1 : 0;
    pool.set(game.championId, entry);
  }
  const average = (pick: (game: RecentMatch) => number) =>
    matches.reduce((sum, game) => sum + pick(game), 0) / matches.length;
  return {
    puuid,
    name: { gameName: name, tagLine: String(10_000 + seed) },
    level: 80 + seed * 7,
    iconId: 29,
    private: false,
    ranked: { solo: rank, flex: null },
    recent: {
      games: matches.length,
      wins,
      kills: average((game) => game.kills),
      deaths: average((game) => game.deaths),
      assists: average((game) => game.assists),
      streak: first?.win ? streak : -streak,
      matches,
      champions: [...pool.values()].sort((a, b) => b.games - a.games).slice(0, 5),
    },
  };
}

const rank = (tier: Rank["tier"], division: string | null, lp: number): Rank => ({
  tier,
  division,
  lp,
  wins: 120 + lp,
  losses: 110,
});

const TEAM: [string, string, Rank | null, Seat["position"]][] = [
  ["demo-me", "暗夜里的光", rank("DIAMOND", "II", 56), "middle"],
  ["demo-2", "峡谷清道夫", rank("EMERALD", "I", 12), "top"],
  ["demo-3", "野区观光客", rank("DIAMOND", "IV", 88), "jungle"],
  ["demo-4", "补刀不漏一个", rank("MASTER", null, 233), "bottom"],
  ["demo-5", "眼位守护者", null, "utility"],
];

const SUMMARIES = new Map(
  TEAM.map(([puuid, name, tierRank], index) => [puuid, summary(puuid, name, index + 3, tierRank)]),
);

/** Five players in the default five Rift tiers, as the core ranks them: one in each tier. */
const RATINGS: Seat["rating"][] = [
  {
    score: 7.4,
    tier: 0,
    tiers: 5,
    label: "峡谷通天代",
    grade: null,
    title: "版本答案",
    quip: "对面五个人准备举报代练",
  },
  {
    score: 5.2,
    tier: 2,
    tiers: 5,
    label: "峡谷公务员",
    grade: null,
    title: null,
    quip: "按时上班，准时打卡",
  },
  {
    score: 6.8,
    tier: 1,
    tiers: 5,
    label: "人形防御塔",
    grade: null,
    title: null,
    quip: "塔在人在，人在塔也在",
  },
  {
    score: 4.6,
    tier: 3,
    tiers: 5,
    label: "移动眼位",
    grade: null,
    title: "峡谷慈善家",
    quip: "站在哪里，哪里就有视野",
  },
  {
    score: 3.9,
    tier: 4,
    tiers: 5,
    label: "纯正牛马",
    grade: null,
    title: "黑白电视机资深会员",
    quip: "勤勤恳恳地给对面创造游戏体验",
  },
];

function seats(): Seat[] {
  return TEAM.map(([puuid, name, , position], index) => {
    const stats = SUMMARIES.get(puuid);
    return {
      puuid,
      name: { gameName: name, tagLine: String(10_003 + index) },
      championId: [103, 86, 64, 222, 412][index] ?? 0,
      intent: index === 4,
      position,
      spells: [4, 14],
      isSelf: index === 0,
      premade: index === 3 || index === 4 ? 1 : null,
      stats: stats ? { state: "ready", ...stats } : { state: "loading" },
      rating: RATINGS[index] ?? null,
    };
  });
}

/** Hextech ARAM: one team, a bench and a reroll, the callout already written. */
function champSelect(): ChampSelectView {
  return {
    gameId: 1,
    queueId: 2400,
    timer: { phase: "BAN_PICK", endsAt: Date.now() + 47_000, totalMs: 60_000 },
    myTeam: seats(),
    theirTeam: [],
    myBans: [],
    theirBans: [],
    benchEnabled: true,
    bench: [99, 81, 22, 157],
    rerollsRemaining: 1,
    side: "blue",
    callout: [
      "【蓝色方】winer 战绩鉴定",
      "峡谷通天代：阿狸 暗夜里的光 近20场胜率60% KDA 4.1 评分7.4「版本答案」，对面五个人准备举报代练",
      "人形防御塔：李青 野区观光客 近20场胜率55% KDA 3.6 评分6.8，塔在人在，人在塔也在",
      "峡谷公务员：盖伦 峡谷清道夫 近20场胜率50% KDA 2.9 评分5.2，按时上班，准时打卡",
      "移动眼位：金克丝 补刀不漏一个 近20场胜率45% KDA 2.4 评分4.6「峡谷慈善家」，站在哪里，哪里就有视野",
      "纯正牛马：锤石 眼位守护者 近20场胜率40% KDA 2.0 评分3.9「黑白电视机资深会员」，勤勤恳恳地给对面创造游戏体验",
    ],
  };
}

const DEFAULT_SETTINGS: Settings = {
  appearance: {
    theme: "hextech",
    accent: "default",
    density: "comfortable",
    fontSize: 13,
    reduceMotion: false,
  },
  general: { closeToTray: true, language: "zh-CN", augmentDetails: true, titles: true },
  automation: {
    accept: { enabled: true, delayMs: 1500 },
    pick: {
      enabled: true,
      lockIn: true,
      declareIntent: true,
      champions: {
        any: [103, 1],
        top: [],
        jungle: [],
        middle: [103, 238],
        bottom: [],
        utility: [],
      },
    },
    ban: {
      enabled: false,
      champions: { any: [157], top: [], jungle: [], middle: [], bottom: [], utility: [] },
    },
    playAgain: false,
    callout: {
      auto: false,
      audience: "team",
      includeSelf: true,
      header: "",
      template: "",
      tiers: "riftFive",
      customTiers: [],
    },
    bench: { enabled: true, champions: [103, 99, 22] },
    scopes: defaultScopes(),
  },
  plugin: {
    auto: true,
    teamPanel: true,
    hidePromotions: false,
    benchNoCooldown: true,
    loaderDir: null,
  },
};

export function demoBackend(): Backend {
  let settings = DEFAULT_SETTINGS;
  const listeners = new Set<(event: Event) => void>();
  const updateListeners = new Set<(status: UpdateStatus) => void>();
  const emit = (event: Event) => listeners.forEach((listener) => listener(event));
  let update: UpdateStatus = { state: "upToDate", version: "0.2.0", checkedAt: NOW };
  let plugin: PluginStatus = {
    loaderDir: "C:\\Users\\Player\\AppData\\Local\\app.winer.desktop\\pengu",
    active: true,
    managed: true,
    bundledLoader: "1.1.6",
    occupied: false,
    setupError: null,
    installedVersion: "0.2.0",
    bundledVersion: "0.2.0",
    current: true,
    connected: 2,
  };

  const snapshot: Snapshot = {
    rev: 1,
    connection: { status: "connected", port: 62194, platformId: "HN10" },
    me: {
      puuid: "demo-me",
      name: { gameName: "暗夜里的光", tagLine: "10003" },
      level: 312,
      iconId: 29,
      ranked: { solo: rank("DIAMOND", "II", 56), flex: rank("PLATINUM", "I", 75) },
    },
    phase: "ChampSelect",
    champSelect: champSelect(),
    game: null,
  };

  setTimeout(
    () => emit({ type: "notice", data: { at: Date.now(), kind: { kind: "accepted" } } }),
    1200,
  );
  setTimeout(
    () =>
      emit({
        type: "notice",
        data: { at: Date.now(), kind: { kind: "declared", championId: 103 } },
      }),
    2400,
  );

  const handlers: { [K in CommandName]: (args: Commands[K]["args"]) => Commands[K]["result"] } = {
    get_snapshot: () => ({ ...snapshot, champSelect: champSelect() }),
    get_settings: () => settings,
    set_settings: ({ settings: next }) => {
      settings = next;
      return settings;
    },
    get_game_data: () => GAME_DATA,
    get_match_history: ({ puuid, begin, count }): MatchPage => {
      const all = history(puuid, SUMMARIES.get(puuid)?.name?.gameName ?? "对手", puuid.length, 60);
      return {
        puuid,
        begin,
        games: all.slice(begin, begin + count),
        hasMore: begin + count < all.length,
        source: "server",
      };
    },
    get_match_detail: ({ gameId }) => scoreboard(gameId),
    get_augment_details: () => [
      {
        id: 1004,
        description: "你的终极技能已被封印。获得35%技能伤害、治疗效果、护盾和70技能急速。",
      },
      { id: 2103, description: "狙击敌方英雄会引发一个延迟伤害爆炸。" },
      { id: 1116, description: "你的闪现有3层充能。在120秒后，再次获得所有充能。" },
      { id: 2102, description: "每一秒，对附近的敌方英雄们施加一层可叠加的灼烧。" },
    ],
    find_player: ({ riotId }) => {
      const found = SUMMARIES.get("demo-3");
      if (!found || !riotId.includes("#"))
        throw { code: "notFound", message: `no player is called ${riotId}` };
      return {
        puuid: found.puuid,
        name: found.name,
        level: found.level,
        iconId: found.iconId,
        private: false,
        ranked: found.ranked,
      };
    },
    get_player_summary: ({ puuid }) =>
      SUMMARIES.get(puuid) ?? summary(puuid, "对手", puuid.length, rank("GOLD", "III", 40)),
    get_presence: () => ({ availability: "chat", statusMessage: "今晚上分" }),
    set_availability: () => null,
    set_status_message: () => null,
    restart_client_ui: () => null,
    send_callout: () => champSelect().callout.length,
    preview_callout: ({ rule, general }) => {
      const own = rule.customTiers.map((name) => name.trim()).filter(Boolean);
      const names =
        rule.tiers !== "custom"
          ? TIER_NAMES[rule.tiers]["zh-CN"]
          : own.length >= 2
            ? own
            : TIER_NAMES.horses["zh-CN"];
      const title = general.titles ? "「版本答案」" : "";
      const lines = names.map(
        (name) => `${name}：阿狸 暗夜里的光 近20场胜率60% KDA 4.1 评分7.4${title}`,
      );
      // As the core does: the side and winer's name lead the first line, the opening line after.
      const header = rule.header.trim();
      return [`【蓝色方】winer 战绩鉴定${header ? ` · ${header}` : ""}`, ...lines];
    },
    bench_swap: () => null,
    reroll: () => null,
    get_plugin_status: () => plugin,
    enable_plugin: () => {
      settings = { ...settings, plugin: { ...settings.plugin, auto: true } };
      emit({ type: "settings", data: settings });
      plugin = {
        ...plugin,
        active: true,
        installedVersion: plugin.bundledVersion,
        current: true,
        connected: 2,
      };
      return plugin;
    },
    disable_plugin: () => {
      settings = { ...settings, plugin: { ...settings.plugin, auto: false } };
      emit({ type: "settings", data: settings });
      plugin = { ...plugin, active: false, installedVersion: null, current: false, connected: 0 };
      return plugin;
    },
    get_app_info: () => ({
      version: "0.2.0",
      elevated: true,
      logDir: "C:\\Users\\demo\\AppData\\Local\\app.winer.desktop\\logs",
      settingsPath: "settings.json",
      notices:
        "# Third-party notices\n\n## Pengu Loader\n\nMIT License\n\nCopyright (c) 2024 Pengu Loader",
    }),
    relaunch_elevated: () => null,
    reveal_logs: () => null,
    get_autostart: () => false,
    set_autostart: ({ enabled }) => enabled,
    get_update_status: () => update,
    check_update: () => {
      update = { state: "upToDate", version: "0.2.0", checkedAt: Date.now() };
      updateListeners.forEach((listener) => listener(update));
      return update;
    },
    install_update: () => null,
    open_releases: () => null,
    open_docs: () => null,
  };

  return {
    call: async <K extends CommandName>(command: K, ...args: ArgsOf<K>) => {
      await new Promise((resolve) => setTimeout(resolve, 120));
      const handler = handlers[command] as (args: Commands[K]["args"]) => Commands[K]["result"];
      return handler(args[0] as Commands[K]["args"]);
    },
    onEvent: (handler) => {
      listeners.add(handler);
      return () => listeners.delete(handler);
    },
    onResync: () => () => undefined,
    onUpdate: (handler) => {
      updateListeners.add(handler);
      return () => updateListeners.delete(handler);
    },
  };
}
