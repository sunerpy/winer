import type { Language } from "@winer/shared";

const STRINGS = {
  title: ["队友战绩", "Teammates"],
  loading: ["正在读取战绩", "Loading stats"],
  hidden: ["隐藏的玩家", "Hidden player"],
  failed: ["战绩读取失败", "Stats unavailable"],
  unranked: ["未定级", "Unranked"],
  collapse: ["收起", "Collapse"],
  expand: ["展开", "Expand"],
  blue: ["蓝色方", "Blue side"],
  red: ["红色方", "Red side"],
  // Social: friends' games, the lobby, premade parties.
  inGame: ["游戏中", "In game"],
  winRate: ["胜率", "Win rate"],
  score: ["战力", "Form"],
  noGames: ["近期没有对局", "No recent games"],
  open: ["在 winer 中查看战绩", "Open their history in winer"],
  lobby: ["房间成员", "Lobby"],
  premade: ["开黑", "Party"],
} as const;

export function text(language: Language, key: keyof typeof STRINGS): string {
  return STRINGS[key][language === "en" ? 1 : 0];
}
