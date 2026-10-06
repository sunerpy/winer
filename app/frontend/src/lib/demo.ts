// A believable client for `pnpm dev` in a browser: one summoner in champ select with a full team,
// twenty games of history and a working settings round trip. Never part of a release bundle.
import type {
  BackupInfo,
  BannerChoice,
  CalloutRule,
  ChallengeProfile,
  ChallengeToken,
  ChampSelectView,
  Event,
  Feat,
  FormScope,
  FriendsView,
  GameData,
  GameKind,
  GameView,
  HotkeyStatus,
  LobbyView,
  MatchDetail,
  MatchPage,
  MatchSummary,
  PlayerLine,
  PlayerStanding,
  PlayerSummary,
  PluginStatus,
  Presence,
  Rank,
  RecentMatch,
  Seat,
  Settings,
  SkinChoice,
  Snapshot,
  TitleChoice,
  UpdateStatus,
} from "@winer/shared";

import type { ArgsOf, Backend, CommandName, Commands } from "./backend";
import { typedLines } from "./callout";
import {
  DEMO_AUGMENTS,
  DEMO_ITEMS,
  DEMO_PERKS,
  DEMO_SPELLS,
  demoLoadoutHandlers,
} from "./demoLoadout";
import { demoStorageHandlers } from "./demoStorage";
import { FEAT_ORDER } from "./feats";
import { defaultScopes } from "./modes";
import { MOBILE_MESSAGE } from "./presence";
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
  items: DEMO_ITEMS,
  spells: DEMO_SPELLS,
  perks: DEMO_PERKS,
  augments: [
    { id: 1004, name: "回归基本功", icon: "", rarity: "prismatic" },
    { id: 2103, name: "狙神飞星", icon: "", rarity: "gold" },
    { id: 1116, name: "闪现向前", icon: "", rarity: "gold" },
    { id: 2102, name: "高压锅", icon: "", rarity: "silver" },
    ...DEMO_AUGMENTS,
  ],
  queues: [
    { id: 420, name: "排位赛 单排/双排", gameMode: "CLASSIC", ranked: true },
    { id: 440, name: "排位赛 灵活排位", gameMode: "CLASSIC", ranked: true },
    { id: 430, name: "匹配模式", gameMode: "CLASSIC", ranked: false },
    { id: 450, name: "极地大乱斗", gameMode: "ARAM", ranked: false },
    { id: 2400, name: "海克斯大乱斗", gameMode: "KIWI", ranked: false },
    // History: a custom lobby's queue and co-op vs AI, as the Tencent client names them.
    { id: 3220, name: "嚎哭深渊 全随机", gameMode: "ARAM", ranked: false },
    { id: 870, name: "入门级", gameMode: "SWIFTPLAY", ranked: false },
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

// ---- History: games form leaves out, and a player rated alone ----

/** Games of a demo player's history not played against players through matchmaking, by place:
 *  the demo player's past their first fifty, the player the search finds at the top. */
const LEFT_OUT: Record<string, Record<number, GameKind>> = {
  [DEMO_PLAYER]: { 52: "custom", 55: "bots", 60: "custom" },
  "demo-3": { 0: "custom", 3: "custom", 7: "bots" },
};

const kindOf = (puuid: string, index: number): GameKind => LEFT_OUT[puuid]?.[index] ?? "matched";

/** Sixty games to show whatever is hidden: a player's custom games come on top of them. */
function historyLength(puuid: string): number {
  return 60 + Object.values(LEFT_OUT[puuid] ?? {}).filter((kind) => kind === "custom").length;
}

/** The newest twenty games against players among `games`, and what was passed over on the way,
 *  as the core reads form (`analysis::recent_form`). */
function formGames(games: MatchSummary[]): { counted: MatchSummary[]; scope: FormScope } {
  const scope: FormScope = { listed: games.length, custom: 0, bots: 0, remakes: 0 };
  const counted: MatchSummary[] = [];
  for (const game of games) {
    if (counted.length === 20) break;
    if (game.kind === "custom") scope.custom += 1;
    else if (game.kind === "bots") scope.bots += 1;
    else counted.push(game);
  }
  return { counted, scope };
}

/** The core's form bands (`rating::FORM_GRADES`), S+ to E; below them F. */
const FORM_GRADES = [7.6, 6.8, 5.9, 5.3, 4.8, 4.3, 3.8];
/** One quip per tier of the default five, from the core's own (`callout.rs`). */
const RIFT_FIVE_QUIPS = [
  "对面五个人准备举报代练",
  "稳得离谱，能C还能活",
  "无功无过，绩效合格",
  "站在哪里，哪里就有视野",
  "队友看完战绩陷入沉思",
];

type Lean = "above" | "middle" | "below";

/** Where a tier stands against its scheme's middle, as the core's `rating::lean` reads it. */
function leanOf(tier: number, tiers: number, graded: boolean): Lean {
  if (graded) return tier <= 2 ? "above" : tier >= 5 ? "below" : "middle";
  const order = 2 * tier + 1 - tiers;
  return order < 0 ? "above" : order > 0 ? "below" : "middle";
}

/** The demo's title for each leaning, as `rating::form_title` gives an unremarkable player. */
const DEMO_TITLES: Record<Lean, string> = {
  above: "靠谱队友",
  middle: "正常发挥",
  below: "陪跑选手",
};

/** As the core's `callout::tier_emoji`: the best tier crowned, the worst done for. */
function tierEmoji(tier: number, tiers: number, graded: boolean): string {
  if (tier === 0) return "👑";
  if (tier === tiers - 1) return "💀";
  return { above: "🔥", middle: "👌", below: "😅" }[leanOf(tier, tiers, graded)];
}

/** A player rated alone as the core does it (`history::rate_alone`): the fixed band of the form
 *  score, spread over the scheme's tiers the way a team is. */
function standingOf(summary: PlayerSummary, scope: FormScope, settings: Settings): PlayerStanding {
  const form = summary.recent;
  if (form.games === 0) return { scope, rating: null, band: null };
  const kda = (form.kills + form.assists) / Math.max(1, form.deaths);
  const raw = 10 * (0.5 * (form.wins / form.games) + 0.5 * (1 - Math.exp(-kda / 3)));
  const confidence = form.games / (form.games + 5);
  const score = Math.round((confidence * raw + (1 - confidence) * 5) * 10) / 10;
  const found = FORM_GRADES.findIndex((floor) => score >= floor);
  const band = found === -1 ? FORM_GRADES.length : found;
  const rule = settings.automation.callout;
  const own = rule.customTiers.map((name) => name.trim()).filter(Boolean);
  const names =
    rule.tiers !== "custom"
      ? TIER_NAMES[rule.tiers]["zh-CN"]
      : own.length >= 2
        ? own
        : TIER_NAMES.horses["zh-CN"];
  const graded = rule.tiers === "grades";
  const tier = graded
    ? Math.min(band, names.length - 1)
    : Math.min(names.length - 1, Math.floor(((band + 0.5) / 8) * names.length - 1e-9));
  const lean = leanOf(tier, names.length, graded);
  const title =
    lean === "above"
      ? form.streak >= 3
        ? "版本答案"
        : "靠谱队友"
      : lean === "below"
        ? form.streak <= -3
          ? "排位慈善家"
          : "陪跑选手"
        : "正常发挥";
  return {
    scope,
    band,
    rating: {
      score,
      tier,
      tiers: names.length,
      label: names[tier] ?? "",
      grade: graded ? tier : null,
      title: settings.general.titles && form.games >= 5 ? title : null,
      quip: rule.tiers === "riftFive" ? (RIFT_FIVE_QUIPS[tier] ?? null) : null,
    },
  };
}

function history(puuid: string, name: string, seed: number, count: number): MatchSummary[] {
  const pick = random(seed);
  return Array.from({ length: count }, (_, index) => {
    const win = pick() > 0.45;
    const kind = kindOf(puuid, index);
    const drawn = [420, 2400, 430, 2400, 440][Math.floor(pick() * 5)] ?? 420;
    const queueId = kind === "custom" ? 3220 : kind === "bots" ? 870 : drawn;
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
      gameMode:
        queueId === 2400
          ? "KIWI"
          : queueId === 3220
            ? "ARAM"
            : queueId === 870
              ? "SWIFTPLAY"
              : "CLASSIC",
      startedAt: NOW - (index + 1) * 3_600_000 * (1 + pick() * 5),
      duration: 1200 + Math.floor(pick() * 900),
      // Hextech ARAM is played with augments instead of runes.
      line: queueId === 2400 ? { ...played, augments: [1004, 2103, 1116, 2102] } : played,
      kind,
    };
  });
}

/** What a demo player's form counts: their thirty newest games, as a client lists them. */
const scopeOf = (puuid: string, name: string, seed: number): FormScope =>
  formGames(history(puuid, name, seed, 30)).scope;

function summary(puuid: string, name: string, seed: number, rank: Rank | null): PlayerSummary {
  const games = formGames(history(puuid, name, seed, 30)).counted;
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

/** The seed a demo player's summary is drawn from. */
const seedOf = (puuid: string): number => {
  const index = TEAM.findIndex(([id]) => id === puuid);
  return index === -1 ? puuid.length : index + 3;
};

const SUMMARIES = new Map(
  TEAM.map(([puuid, name, tierRank]) => [puuid, summary(puuid, name, seedOf(puuid), tierRank)]),
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

/** Hextech ARAM: one team, a bench and a reroll, the callout already written: best tier first,
 *  each line naming the seat (the place in the team as champ select lists it) and the player. */
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
      "📢【蓝色方】winer 战绩鉴定",
      "👑 峡谷通天代：1L【暗夜里的光】，近20场胜率60%，KDA 4.1，战力7.4【版本答案】，对面五个人准备举报代练",
      "🔥 人形防御塔：3L【野区观光客】，近20场胜率55%，KDA 3.6，战力6.8【靠谱队友】，塔在人在，人在塔也在",
      "👌 峡谷公务员：2L【峡谷清道夫】，近20场胜率50%，KDA 2.9，战力5.2【正常发挥】，按时上班，准时打卡",
      "😅 移动眼位：4L【补刀不漏一个】，近20场胜率45%，KDA 2.4，战力4.6【峡谷慈善家】，站在哪里，哪里就有视野",
      "💀 纯正牛马：5L【眼位守护者】，近20场胜率40%，KDA 2.0，战力3.9【黑白电视机资深会员】，勤勤恳恳地给对面创造游戏体验",
    ],
  };
}

// Callout: a running game, with both teams' lines the shortcut can type into the game's chat.

/** The demo team in a ranked game on the blue side against five rated players on the red side,
 *  the enemy to watch, the one to go after and the team's own lines written as the core writes
 *  them: every player by champion. */
export function demoGame(): GameView {
  const enemies: Seat[] = (
    [
      ["demo-r1", "红方上单", 157, 4, 3.4, "纯正牛马"],
      ["demo-r2", "红方打野", 121, 0, 7.6, "峡谷通天代"],
      ["demo-r3", "红方中单", 238, 2, 5.4, "峡谷公务员"],
      ["demo-r4", "红方射手", 81, 1, 6.6, "人形防御塔"],
      ["demo-r5", "红方辅助", 89, 3, 4.5, "移动眼位"],
    ] as const
  ).map(([puuid, name, championId, tier, score, label]) => ({
    puuid,
    name: { gameName: name, tagLine: "20001" },
    championId,
    intent: false,
    position: null,
    spells: [4, 14],
    isSelf: false,
    premade: null,
    stats: { state: "ready", ...summary(puuid, name, championId % 7, null) },
    rating: { score, tier, tiers: 5, label, grade: null, title: null, quip: null },
  }));
  return {
    gameId: 2,
    queueId: 420,
    teams: [seats().map((seat) => ({ ...seat, intent: false })), enemies],
    sides: true,
    callout: [
      "【敌方·红色方】winer 战绩鉴定",
      "小心【卡兹克】：峡谷通天代，近20场胜率65%，KDA 4.6",
      "对面【亚索】：纯正牛马，近20场胜率35%，可以多抓",
    ],
    // As champ select's lines, the champion in brackets where the seat and the name were.
    allyCallout: [
      "【我方·蓝色方】winer 战绩鉴定",
      "峡谷通天代【阿狸】，近20场胜率60%，KDA 4.1，战力7.4【版本答案】，对面五个人准备举报代练",
      "人形防御塔【李青】，近20场胜率55%，KDA 3.6，战力6.8【靠谱队友】，塔在人在，人在塔也在",
      "峡谷公务员【盖伦】，近20场胜率50%，KDA 2.9，战力5.2【正常发挥】，按时上班，准时打卡",
      "移动眼位【金克丝】，近20场胜率45%，KDA 2.4，战力4.6【峡谷慈善家】，站在哪里，哪里就有视野",
      "纯正牛马【锤石】，近20场胜率40%，KDA 2.0，战力3.9【黑白电视机资深会员】，勤勤恳恳地给对面创造游戏体验",
    ],
  };
}

// ---- The profile tools: a wardrobe, challenges with levels, and settings backed up before. ----

/** Skin lines the demo's champions come in, after their base skin. */
const SKIN_LINES = ["星之守护者", "源计划", "K/DA", "灵魂莲华", "西部魔影", "未来战士", "冰雪节"];

/** Every demo champion's base skin and two to four more, some owned, as the client lists them. */
const SKINS: SkinChoice[] = CHAMPIONS.flatMap(([id, title, short, alias], index) => {
  const art = (number: number) => {
    const folder = number === 0 ? "Base" : `Skin${String(number).padStart(2, "0")}`;
    const images = `/lol-game-data/assets/ASSETS/Characters/${alias}/Skins/${folder}/Images`;
    const file = alias.toLowerCase();
    return {
      tile: `${images}/${file}_splash_tile_${number}.jpg`,
      splash: `${images}/${file}_splash_centered_${number}.jpg`,
    };
  };
  const lines = SKIN_LINES.slice(index % 3, (index % 3) + 2 + (index % 3));
  return [
    { id: id * 1000, championId: id, name: title, owned: true, base: true, ...art(0) },
    ...lines.map((line, at) => ({
      id: id * 1000 + at + 1,
      championId: id,
      name: `${line} ${short}`,
      owned: (index + at) % 3 === 0,
      base: false,
      ...art(at + 1),
    })),
  ];
});

const CHALLENGE_CHOICES: ChallengeToken[] = [
  ["101304", "闪电战", "赢得【极地大乱斗】对局且对局时长低于13分钟", "MASTER"],
  ["101101", "伤害爆表", "在【极地大乱斗】中造成超过1800点每分钟伤害", "MASTER"],
  ["505005", "射手收藏家", "使用不同的射手英雄获得S-或更高评分", "DIAMOND"],
  ["505006", "辅助收藏家", "使用不同的辅助英雄获得S-或更高评分", "DIAMOND"],
  [
    "101000",
    "极地权威",
    "获取来自【极地斗士】、【极地妙手】、【极地战士】等分组中的成就进度",
    "PLATINUM",
  ],
  ["101203", "雪球大战", "在【极地大乱斗】中用雪球命中英雄", "PLATINUM"],
  ["101104", "回血不如回温泉", "在【极地大乱斗】中击杀近期获得过治疗包的对手", "GOLD"],
  ["101206", "魄罗破咯", "在【极地大乱斗】中导致一个魄罗爆炸", "BRONZE"],
].map(([id, name, description, level]) => ({
  id: Number(id),
  name: name ?? "",
  description: description ?? "",
  level: level as ChallengeToken["level"],
  icon: `/lol-game-data/assets/ASSETS/Challenges/Config/${id}/Tokens/${level}.png`,
}));

const TITLE_CHOICES: TitleChoice[] = [
  { id: 1, name: "初窥门径" },
  { id: 10120601, name: "魄罗饲养员" },
  { id: 10120303, name: "雪球狙神" },
  { id: 1435, name: "混沌代理人" },
  { id: 1436, name: "日光浴恶魔" },
];

/** The client's default banner, then the banners the demo player owns, as the core lists them. */
const BANNER_CHOICES: BannerChoice[] = [
  ["", "default", "", "default.png"],
  ["6", "event", "北极星(2023)贵族旗帜", "wn2023.png"],
  ["24", "event", "魄罗之王的旗帜", "ARAM_Banner.png"],
].map(([id, kind, name, file]) => ({
  id: id ?? "",
  kind: kind as BannerChoice["kind"],
  name: name ?? "",
  art: `/lol-game-data/assets/ASSETS/Regalia/BannerSkins/${file}`,
}));

/** What the status message becomes when the client shows `availability` with `current`, as the
 *  core's `mobile_message_for` decides it; `null` leaves it. */
function mobileMessageFor(availability: string, current: string, on: boolean): string | null {
  if (!["chat", "away", "mobile", "offline"].includes(availability)) return null;
  if (on && availability === "mobile") return current.trim() ? null : MOBILE_MESSAGE;
  return current === MOBILE_MESSAGE ? "" : null;
}

// Social: friends at play and a party in the lobby, as the core draws them.

/** Friends in game and in champ select: two in one ARAM game (group 1), one in a ranked game that
 *  can be spectated, one picking; and one at the home screen, whom the panel leaves out. */
function friends(): FriendsView {
  const now = Date.now();
  const friend = (
    puuid: string,
    gameName: string,
    status: FriendsView["friends"][number]["status"],
    group: number | null = null,
  ): FriendsView["friends"][number] => ({
    puuid,
    name: { gameName, tagLine: String(20_000 + puuid.length) },
    iconId: 29,
    availability: status.state === "outOfGame" ? "chat" : "dnd",
    status,
    group,
  });
  return {
    friends: [
      friend("friend-1", "上分小能手", {
        state: "inGame",
        mode: "排位赛 单排/双排",
        queueId: 420,
        startedAt: now - 25 * 60_000 - 12_000,
        observable: true,
      }),
      friend(
        "friend-2",
        "峡谷夜行者",
        {
          state: "inGame",
          mode: "极地大乱斗",
          queueId: 450,
          startedAt: now - 12 * 60_000 - 34_000,
          observable: false,
        },
        1,
      ),
      friend(
        "friend-3",
        "补兵机器",
        {
          state: "inGame",
          mode: "极地大乱斗",
          queueId: 450,
          startedAt: now - 12 * 60_000 - 33_000,
          observable: false,
        },
        1,
      ),
      friend("friend-4", "辅助永不死", {
        state: "champSelect",
        mode: "海克斯大乱斗",
        queueId: 2400,
        since: now - 40_000,
      }),
      friend("friend-5", "周末玩家", { state: "outOfGame" }),
    ],
  };
}

/** A party of three in a ranked lobby, the local player leading it; one member still loading. */
export function demoLobby(): LobbyView {
  const ready = (puuid: string): LobbyView["members"][number]["stats"] => {
    const stats = SUMMARIES.get(puuid);
    return stats ? { state: "ready", ...stats } : { state: "loading" };
  };
  return {
    queueId: 420,
    custom: false,
    members: [
      {
        puuid: "demo-me",
        name: { gameName: "暗夜里的光", tagLine: "10003" },
        iconId: 29,
        isSelf: true,
        leader: true,
        positions: ["middle", "fill"],
        stats: ready("demo-me"),
        score: 7.4,
      },
      {
        puuid: "demo-2",
        name: { gameName: "峡谷清道夫", tagLine: "10004" },
        iconId: 29,
        isSelf: false,
        leader: false,
        positions: ["top", "jungle"],
        stats: ready("demo-2"),
        score: 5.2,
      },
      {
        puuid: "demo-6",
        name: { gameName: "新来的队友", tagLine: "10009" },
        iconId: 29,
        isSelf: false,
        leader: false,
        positions: ["utility"],
        stats: { state: "loading" },
        score: null,
      },
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
  general: {
    closeToTray: true,
    language: "zh-CN",
    augmentDetails: true,
    titles: true,
    hotkey: "Alt+Backquote",
  },
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
      hotkey: null,
      inGame: false,
      watchTemplate: "",
      targetTemplate: "",
      allyTemplate: "",
      gameTeams: "enemies",
      style: "rich",
    },
    bench: { enabled: true, champions: [103, 99, 22] },
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
};

/** The tier names `rule` ranks with, best first, as the core resolves them (`callout::tier_names`):
 *  fewer than two names of the user's own stand in for none. */
function tierNames(rule: CalloutRule): string[] {
  if (rule.tiers !== "custom") return TIER_NAMES[rule.tiers]["zh-CN"];
  const own = rule.customTiers.map((name) => name.trim()).filter(Boolean);
  return own.length >= 2 ? own : TIER_NAMES.horses["zh-CN"];
}

export function demoBackend(): Backend {
  let settings = DEFAULT_SETTINGS;
  const listeners = new Set<(event: Event) => void>();
  const updateListeners = new Set<(status: UpdateStatus) => void>();
  const hotkeyListeners = new Set<(status: HotkeyStatus) => void>();
  const emit = (event: Event) => listeners.forEach((listener) => listener(event));
  let update: UpdateStatus = { state: "upToDate", version: "0.2.0", checkedAt: NOW };
  // The shell's shortcuts: each registered whenever it is named and not let go for the recorder.
  const held = (shortcut: string | null, suspended: boolean) => ({
    shortcut,
    active: shortcut !== null && !suspended,
    error: null,
  });
  let hotkey: HotkeyStatus = {
    ...held(settings.general.hotkey, false),
    suspended: false,
    callout: held(settings.automation.callout.hotkey, false),
  };
  const setHotkey = (
    shortcut: string | null,
    suspended: boolean,
    callout: string | null = hotkey.callout.shortcut,
  ) => {
    hotkey = { ...held(shortcut, suspended), suspended, callout: held(callout, suspended) };
    hotkeyListeners.forEach((listener) => listener(hotkey));
    return hotkey;
  };
  let plugin: PluginStatus = {
    loaderDir: "C:\\Users\\Player\\AppData\\Local\\app.winer.desktop\\pengu",
    active: true,
    managed: true,
    bundledLoader: "1.1.6",
    occupied: false,
    setupError: null,
    needsElevation: false,
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
    friends: friends(),
    // The client shows no lobby during champ select.
    lobby: null,
  };

  // The profile the demo player shows, and what they backed up before.
  let presence: Presence = { availability: "chat", statusMessage: "今晚上分" };
  /** The mobile state's message, put up or taken down as the core does. */
  const followMobileMessage = (on: boolean): Presence => {
    const message = mobileMessageFor(presence.availability, presence.statusMessage, on);
    if (message !== null) presence = { ...presence, statusMessage: message };
    return presence;
  };
  let background: number | null = 103003;
  let shown = { tokens: [101304, 505005], title: 1436 as number | null, banner: "24" };
  const challengeProfile = (): ChallengeProfile => ({
    tokens: shown.tokens.flatMap((id) => CHALLENGE_CHOICES.filter((token) => token.id === id)),
    title: TITLE_CHOICES.find((title) => title.id === shown.title) ?? null,
    challenges: CHALLENGE_CHOICES,
    titles: TITLE_CHOICES,
    banner: shown.banner,
    banners: BANNER_CHOICES,
  });
  let backups: BackupInfo[] = [
    {
      id: NOW - 86_400_000,
      takenAt: NOW - 86_400_000,
      size: 8_402,
      channels: ["general", "hotkeys"],
    },
    { id: NOW - 5 * 86_400_000, takenAt: NOW - 5 * 86_400_000, size: 5_877, channels: ["hotkeys"] },
  ];
  const keep = (backup: BackupInfo) => {
    backups = [backup, ...backups].sort((a, b) => b.id - a.id).slice(0, 10);
    return backup;
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
    get_snapshot: () => ({ ...snapshot, champSelect: champSelect(), friends: friends() }),
    get_settings: () => settings,
    set_settings: ({ settings: next }) => {
      settings = next;
      if (
        settings.general.hotkey !== hotkey.shortcut ||
        settings.automation.callout.hotkey !== hotkey.callout.shortcut
      )
        setHotkey(settings.general.hotkey, hotkey.suspended, settings.automation.callout.hotkey);
      return settings;
    },
    get_game_data: () => GAME_DATA,
    get_match_history: ({ puuid, begin, count }): MatchPage => {
      const all = history(
        puuid,
        SUMMARIES.get(puuid)?.name?.gameName ?? "对手",
        puuid.length,
        historyLength(puuid),
      );
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
      SUMMARIES.get(puuid) ?? summary(puuid, "对手", seedOf(puuid), rank("GOLD", "III", 40)),
    get_presence: () => presence,
    set_availability: ({ availability }) => {
      presence = { ...presence, availability };
      return settings.profile.presence.mobileMessage ? followMobileMessage(true) : presence;
    },
    apply_mobile_message: () => followMobileMessage(settings.profile.presence.mobileMessage),
    set_status_message: ({ message }) => {
      presence = { ...presence, statusMessage: message };
      return null;
    },
    restart_client_ui: () => null,
    send_callout: () => champSelect().callout.length,
    preview_callout: ({ rule, general }) => {
      const names = tierNames(rule);
      const graded = rule.tiers === "grades";
      // As the core does: the tiers take the seats in order, 1L for the best, each with a title of
      // its own leaning and, in the rich style, its emoji.
      const lines = names.map((name, index) => {
        const lean = leanOf(index, names.length, graded);
        const title = general.titles ? `【${DEMO_TITLES[lean]}】` : "";
        return rule.style === "compact"
          ? `${index + 1}L ${name}｜胜率60%｜KDA 4.1｜战力7.4｜【暗夜里的光】`
          : `${tierEmoji(index, names.length, graded)} ${name}：${index + 1}L【暗夜里的光】，近20场胜率60%，KDA 4.1，战力7.4${title}`;
      });
      // As the core does: the side and winer's name lead the first line, the opening line after
      // in 【】, unless it opens with a bracket of its own.
      const header = rule.header.trim();
      const opening = header === "" || header.startsWith("【") ? header : `【${header}】`;
      const first = `【蓝色方】winer 战绩鉴定${opening}`;
      return [rule.style === "compact" ? first : `📢${first}`, ...lines];
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
    // Storage: what winer keeps, and the cleanup.
    ...demoStorageHandlers(),
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
    get_skins: () => SKINS,
    get_profile_background: () => background,
    // As the server does with a skin the player does not own: the background stays.
    set_profile_background: ({ skinId }) => {
      if (SKINS.some((skin) => skin.id === skinId && skin.owned)) background = skinId;
      return background;
    },
    get_challenge_profile: challengeProfile,
    // As the server does with a banner the player does not own: the banner stays.
    set_challenge_profile: ({ challengeIds, titleId, bannerId }) => {
      const banner =
        bannerId !== null && BANNER_CHOICES.some((choice) => choice.id === bannerId)
          ? bannerId
          : shown.banner;
      shown = { tokens: challengeIds.slice(0, 3), title: titleId ?? shown.title, banner };
      return challengeProfile();
    },
    get_game_settings_backups: () => backups,
    create_game_settings_backup: () => {
      const id = Math.max(Date.now(), ...backups.map((backup) => backup.id + 1));
      return keep({ id, takenAt: Date.now(), size: 8_410, channels: ["general", "hotkeys"] });
    },
    restore_game_settings_backup: () => null,
    delete_game_settings_backup: ({ id }) => {
      backups = backups.filter((backup) => backup.id !== id);
      return null;
    },
    import_game_settings_backup: ({ text }) => {
      let file: {
        format?: unknown;
        takenAt?: unknown;
        gameSettings?: unknown;
        inputSettings?: unknown;
      };
      try {
        file = JSON.parse(text) as typeof file;
      } catch {
        throw { code: "invalid", message: "not JSON" };
      }
      if (file.format !== "winer-game-settings")
        throw { code: "invalid", message: "not a winer settings backup" };
      const id = Math.max(Date.now(), ...backups.map((backup) => backup.id + 1));
      return keep({
        id,
        takenAt: typeof file.takenAt === "number" ? file.takenAt : id,
        size: text.length,
        channels: [
          ...(file.gameSettings ? (["general"] as const) : []),
          ...(file.inputSettings ? (["hotkeys"] as const) : []),
        ],
      });
    },
    reveal_game_settings_backup: () => null,
    get_hotkey_status: () => hotkey,
    suspend_hotkey: ({ suspended }) => setHotkey(hotkey.shortcut, suspended),
    ...demoLoadoutHandlers(() => settings),
    // Callout: as the core does, the best tier to watch and the worst to go after on the red side,
    // the user in every tier of the team on the blue side, by champion, typed as one press would.
    preview_game_callout: ({ rule, general }) => {
      const names = tierNames(rule);
      const graded = rule.tiers === "grades";
      const title = (index: number) =>
        general.titles ? `【${DEMO_TITLES[leanOf(index, names.length, graded)]}】` : "";
      const enemies = [
        "【敌方·红色方】winer 战绩鉴定",
        `小心【阿狸】：${names[0]}，近20场胜率60%，KDA 4.1${title(0)}`,
        `对面【阿狸】：${names[names.length - 1]}，近20场胜率60%，可以多抓`,
      ];
      // No emoji in the game's chat.
      const allies = [
        "【我方·蓝色方】winer 战绩鉴定",
        ...names.map((name, index) =>
          rule.style === "compact"
            ? `${name}【阿狸】｜胜率60%｜KDA 4.1｜战力7.4`
            : `${name}【阿狸】，近20场胜率60%，KDA 4.1，战力7.4${title(index)}`,
        ),
      ];
      return typedLines(enemies, allies, rule.gameTeams);
    },
    // History.
    get_player_standing: ({ puuid }) => {
      const player =
        SUMMARIES.get(puuid) ?? summary(puuid, "对手", seedOf(puuid), rank("GOLD", "III", 40));
      const name = player.name?.gameName ?? "对手";
      return standingOf(player, scopeOf(puuid, name, seedOf(puuid)), settings);
    },
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
    onHotkey: (handler) => {
      hotkeyListeners.add(handler);
      return () => hotkeyListeners.delete(handler);
    },
  };
}
