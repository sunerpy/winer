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
} as const;

export function text(language: Language, key: keyof typeof STRINGS): string {
  return STRINGS[key][language === "en" ? 1 : 0];
}
