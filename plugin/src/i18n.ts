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
  // In the client's own panel or in winer's window, as the history panel's option says.
  open: ["查看战绩", "Show their history"],
  lobby: ["房间成员", "Lobby"],
  premade: ["开黑", "Party"],
  // The home page while 隐藏首页推广 is on.
  homeHidden: [
    "首页推广已按 winer 的“隐藏首页推广”选项隐藏。",
    "Home-page promotions are hidden by winer's “Hide home-page promotions” option.",
  ],
  homeShow: ["暂时显示", "Show for now"],
  homeShowHint: ["客户端重启后恢复隐藏", "Hidden again when the client restarts"],
  // The history panel: a player's latest games over the client page.
  historyTitle: ["最近战绩", "Recent games"],
  historyLoading: ["正在读取最近的对局", "Reading recent games"],
  historyFailed: ["没有读到最近的对局", "Recent games unavailable"],
  historyOffline: ["winer 没有连上客户端，读不到战绩", "winer is not connected to the client"],
  historyInvalid: ["无法查询这名玩家的战绩", "This player's games cannot be looked up"],
  historyBusy: [
    "winer 正在读取其他玩家的战绩，请稍后再试",
    "winer is still reading other games; try again shortly",
  ],
  historyTimeout: ["winer 没有及时回应", "winer did not answer in time"],
  historyRetry: ["重试", "Retry"],
  historyClose: ["关闭", "Close"],
  historyInWiner: ["在 winer 中查看完整战绩", "Full history in winer"],
  victory: ["胜", "Win"],
  defeat: ["负", "Loss"],
  remake: ["重开", "Remake"],
  otherMode: ["其他模式", "Other mode"],
  /** Arena's finish; `{n}` is the place. */
  placement: ["第 {n} 名", "#{n}"],
} as const;

export function text(language: Language, key: keyof typeof STRINGS): string {
  return STRINGS[key][language === "en" ? 1 : 0];
}
