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

/** The title one game earns a line on its team, the most telling first; `null` where nothing
 *  stands out, in a remake, or on a team of one. */
export function gameTitle(line: PlayerLine, team: readonly PlayerLine[]): GameTitle | null {
  if (line.remake || team.length < 2) return null;
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
  // The bottom lane's carry: a support shares the lane but not the damage.
  const carry = line.position === "bottom" && share >= 0.18;
  if (line.kills <= 1 && line.deaths >= 8) return "bodhisattva";
  if (line.deaths === 0 && line.kills + line.assists >= 10) return "immortal";
  if (carry && line.deaths <= 2 && share >= 0.25) return "carryAlive";
  if (carry && line.deaths >= 9) return "carryFeeding";
  if (!line.win && share >= 0.3) return "dean";
  if (line.win && share < 0.12) return "puzzle";
  if (line.kills >= 10 && line.deaths >= 10) return "trader";
  if (line.deaths >= 10) return "greyScreen";
  if (kda >= 5 && share < 0.15) return "kSaver";
  if (taken >= 0.3) return "turret";
  if (gold >= 0.24 && share < 0.17) return "banker";
  if (gold <= 0.17 && share >= 0.25) return "underdog";
  if (line.assists >= 10 && line.assists >= 3 * Math.max(1, line.kills)) return "helper";
  if (line.killParticipation !== null && line.killParticipation < 0.35) return "solo";
  return null;
}
