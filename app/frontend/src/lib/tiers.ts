// The rating schemes as the core names them (crates/core/src/callout.rs), best tier first, for the
// settings and the previews; and the roast titles a scoreboard line earns.
import type { Language, PlayerLine, TierSet } from "@winer/shared";

import type { MessageKey } from "./i18n";

/** In the order the settings offer them, the default first. */
export const SCHEMES: readonly TierSet[] = [
  "riftFive",
  "grades",
  "horseUniverse",
  "horsesFive",
  "horses",
  "rift",
  "custom",
];

export const TIER_NAMES: Record<Exclude<TierSet, "custom">, Record<Language, string[]>> = {
  riftFive: {
    "zh-CN": ["峡谷通天代", "人形防御塔", "峡谷公务员", "移动眼位", "纯正牛马"],
    en: ["Rift Demigod", "Human Turret", "Rift Civil Servant", "Walking Ward", "Pure Workhorse"],
  },
  grades: {
    "zh-CN": [
      "峡谷通天代",
      "人形防御塔",
      "峡谷公务员",
      "有用之人",
      "移动眼位",
      "峡谷提款机",
      "泉水观察员",
      "纯正牛马",
    ],
    en: [
      "Rift Demigod",
      "Human Turret",
      "Rift Civil Servant",
      "Useful Person",
      "Walking Ward",
      "Rift ATM",
      "Fountain Watcher",
      "Pure Workhorse",
    ],
  },
  horseUniverse: {
    "zh-CN": ["独角兽", "千里马", "汗血宝马", "峡谷骡子", "跛脚马", "纯牛马", "赛博牛马"],
    en: [
      "Unicorn",
      "Thousand-li Steed",
      "Blood-sweating Horse",
      "Rift Mule",
      "Lame Horse",
      "Pure Workhorse",
      "Cyber Workhorse",
    ],
  },
  horsesFive: {
    "zh-CN": ["独角马", "上等马", "中等马", "下等马", "纯牛马"],
    en: ["Unicorn", "Top horse", "Middle horse", "Bottom horse", "Pack mule"],
  },
  horses: {
    "zh-CN": ["上等马", "中等马", "下等马"],
    en: ["Top horse", "Middle horse", "Bottom horse"],
  },
  rift: {
    "zh-CN": ["峡谷之王", "大腿", "正常发挥", "混子", "提款机"],
    en: ["King of the Rift", "Carry", "Holding up", "Passenger", "Walking ATM"],
  },
};

/** What one game's numbers say about a line, beyond its score. */
export type GameTitle =
  | "bodhisattva"
  | "immortal"
  | "carryAlive"
  | "carryFeeding"
  | "dean"
  | "puzzle"
  | "trader"
  | "greyScreen"
  | "kSaver"
  | "turret"
  | "banker"
  | "underdog"
  | "helper"
  | "solo";

export const GAME_TITLE: Record<GameTitle, { name: MessageKey; why: MessageKey }> = {
  bodhisattva: { name: "title.bodhisattva", why: "title.bodhisattvaWhy" },
  immortal: { name: "title.immortal", why: "title.immortalWhy" },
  carryAlive: { name: "title.carryAlive", why: "title.carryAliveWhy" },
  carryFeeding: { name: "title.carryFeeding", why: "title.carryFeedingWhy" },
  dean: { name: "title.dean", why: "title.deanWhy" },
  puzzle: { name: "title.puzzle", why: "title.puzzleWhy" },
  trader: { name: "title.trader", why: "title.traderWhy" },
  greyScreen: { name: "title.greyScreen", why: "title.greyScreenWhy" },
  kSaver: { name: "title.kSaver", why: "title.kSaverWhy" },
  turret: { name: "title.turret", why: "title.turretWhy" },
  banker: { name: "title.banker", why: "title.bankerWhy" },
  underdog: { name: "title.underdog", why: "title.underdogWhy" },
  helper: { name: "title.helper", why: "title.helperWhy" },
  solo: { name: "title.solo", why: "title.soloWhy" },
};

/** The counts the per-game titles ask for. ARAM's games hold about twice the Rift's kills and
 *  deaths and over three times its assists, so its bars stand where as few of its players reach
 *  them as reach the Rift's: measured on the games WeGame scored (`fixtures/wegame/calibration.json`,
 *  1,260 Rift and 1,390 ARAM lines). Ten deaths are one Rift player in twelve but two ARAM players in
 *  three. Shares of the team need no such bars. */
const BARS = {
  rift: {
    feedKills: 1,
    feedDeaths: 8,
    aliveDeaths: 0,
    aliveTakedowns: 10,
    trade: 10,
    grey: 10,
    assists: 10,
  },
  aram: {
    feedKills: 4,
    feedDeaths: 15,
    aliveDeaths: 3,
    aliveTakedowns: 35,
    trade: 18,
    grey: 18,
    assists: 31,
  },
} as const;

/** The bars of a game of `mode` (the client's game mode): ARAM's for both ARAMs, the Rift's for
 *  every other. */
export function barsOf(mode: string): (typeof BARS)[keyof typeof BARS] {
  return mode === "ARAM" || mode === "KIWI" ? BARS.aram : BARS.rift;
}

/** The title one game earns a line on its team, the most telling first; `null` where nothing
 *  stands out, in a remake, or on a team of one. Counts are read against the game's `mode`
 *  (`barsOf`). */
export function gameTitle(
  line: PlayerLine,
  team: readonly PlayerLine[],
  mode = "CLASSIC",
): GameTitle | null {
  if (line.remake || team.length < 2) return null;
  const bars = barsOf(mode);
  const total = (pick: (player: PlayerLine) => number) =>
    team.reduce((sum, player) => sum + pick(player), 0);
  const share = line.damageShare ?? 0;
  const taken =
    line.damageTaken /
    Math.max(
      1,
      total((player) => player.damageTaken),
    );
  const gold =
    line.gold /
    Math.max(
      1,
      total((player) => player.gold),
    );
  const kda = (line.kills + line.assists) / Math.max(1, line.deaths);
  // The bottom lane's carry: a support shares the lane but not the damage. ARAM has no lanes.
  const carry = line.position === "bottom" && share >= 0.18;
  if (line.kills <= bars.feedKills && line.deaths >= bars.feedDeaths) return "bodhisattva";
  if (line.deaths <= bars.aliveDeaths && line.kills + line.assists >= bars.aliveTakedowns)
    return "immortal";
  if (carry && line.deaths <= 2 && share >= 0.25) return "carryAlive";
  if (carry && line.deaths >= 9) return "carryFeeding";
  if (!line.win && share >= 0.3) return "dean";
  if (line.win && share < 0.12) return "puzzle";
  if (line.kills >= bars.trade && line.deaths >= bars.trade) return "trader";
  if (line.deaths >= bars.grey) return "greyScreen";
  if (kda >= 5 && share < 0.15) return "kSaver";
  if (taken >= 0.3) return "turret";
  if (gold >= 0.24 && share < 0.17) return "banker";
  if (gold <= 0.17 && share >= 0.25) return "underdog";
  if (line.assists >= bars.assists && line.assists >= 3 * Math.max(1, line.kills)) return "helper";
  if (line.killParticipation !== null && line.killParticipation < 0.35) return "solo";
  return null;
}
