// The demo client's builds, runes, spells and item sets (`demo.ts`): believable numbers for any
// champion, from the source the core would ask, and the commands that write them answered.
import type {
  AssetInfo,
  AugmentInfo,
  Build,
  BuildSource,
  ItemOption,
  Mode,
  Position,
  Rates,
  Settings,
} from "@winer/shared";

import type { Commands } from "./backend";

const asset = ([id, name]: [number, string]): AssetInfo => ({ id, name, icon: "" });

export const DEMO_ITEMS: AssetInfo[] = (
  [
    [1055, "多兰之刃"],
    [1056, "多兰之戒"],
    [1082, "黑暗封印"],
    [2003, "生命药水"],
    [3006, "狂战士胫甲"],
    [3009, "轻灵之靴"],
    [3020, "法师之靴"],
    [3031, "无尽之刃"],
    [3036, "多米尼克领主的致意"],
    [3072, "饮血剑"],
    [3089, "灭世者的死亡之帽"],
    [3094, "疾射火炮"],
    [3135, "虚空之杖"],
    [3157, "中娅沙漏"],
    [3165, "莫雷洛秘典"],
    [3340, "监视图腾"],
    [4645, "影焰"],
    [6655, "卢登的伙伴"],
    [6676, "收集者"],
  ] as [number, string][]
).map(asset);

export const DEMO_SPELLS: AssetInfo[] = (
  [
    [4, "闪现"],
    [6, "幽灵疾步"],
    [7, "治疗术"],
    [12, "传送"],
    [14, "引燃"],
    [21, "屏障"],
    [32, "标记"],
  ] as [number, string][]
).map(asset);

export const DEMO_PERKS: AssetInfo[] = (
  [
    [8000, "精密"],
    [8100, "主宰"],
    [8200, "巫术"],
    [8112, "电刑"],
    [8126, "恶意中伤"],
    [8138, "眼球收集器"],
    [8135, "寻宝猎人"],
    [8139, "血之滋味"],
    [8229, "奥术彗星"],
    [8226, "法力流系带"],
    [8210, "超然"],
    [8237, "灼烧"],
    [8009, "气定神闲"],
    [9103, "传说：血统"],
    [5008, "适应之力"],
    [5001, "成长生命值"],
    [5007, "技能急速"],
  ] as [number, string][]
).map(asset);

/** Beside the demo's own four: Hextech ARAM's (1000 and up) and Arena's (below). */
export const DEMO_AUGMENTS: AugmentInfo[] = [
  { id: 1336, name: "升级：无尽之刃", icon: "", rarity: "gold" },
  { id: 1220, name: "连拨击锤", icon: "", rarity: "prismatic" },
  { id: 1320, name: "升级：收集者", icon: "", rarity: "silver" },
  { id: 220, name: "连拨击锤", icon: "", rarity: "prismatic" },
  { id: 195, name: "巨人杀手", icon: "", rarity: "prismatic" },
  { id: 84, name: "穿针引线", icon: "", rarity: "gold" },
  { id: 206, name: "魔法转物理", icon: "", rarity: "silver" },
];

const rates = (pick: number, win: number, games: number | null = null): Rates => ({
  pick,
  win,
  games,
  placement: null,
  first: null,
});

const arena = (pick: number, placement: number, first: number): Rates => ({
  pick,
  win: null,
  games: null,
  placement,
  first,
});

const items = (list: number[], pick: number, win: number): ItemOption => ({
  items: list,
  rates: rates(pick, win),
});

/** Where the core reads a mode's numbers from (`builds::sources`). */
function sourceOf(mode: Mode, settings: Settings): BuildSource {
  if (mode === "hextech") return "tencentHextech";
  if (mode === "ranked" || mode === "normal") {
    return settings.builds.riftSource === "opGg" ? "opGg" : "tencent";
  }
  return "opGg";
}

/** A believable build for any champion: the numbers move a little with the champion and lane. */
export function demoBuild(
  championId: number,
  mode: Mode,
  lane: Position | null,
  settings: Settings,
): Build {
  if (mode === "other") throw { code: "invalid", message: "no numbers for other games" };
  const rift = mode === "ranked" || mode === "normal";
  const source = sourceOf(mode, settings);
  const nudge = ((championId * 7 + (lane?.length ?? 0)) % 9) / 1000;
  const build: Build = {
    source,
    patch: source === "tencentHextech" ? "" : "16.19",
    championId,
    mode,
    lane: rift ? (lane ?? "middle") : null,
    tier: source === "opGg" ? 2 : null,
    sample: source === "tencent" ? 1_241_522 : source === "opGg" ? 384_427 : null,
    spells: [],
    runes: [],
    starting: [],
    boots: [],
    core: [],
    late: [],
    skillOrders: [
      {
        priority: ["Q", "W", "E"],
        sequence: ["Q", "W", "E", "Q", "Q", "R", "Q", "W", "Q", "W", "R", "W", "W", "E", "E"],
        rates: mode === "arena" ? arena(0.83, 4.21, 0.14) : rates(0.83 - nudge, 0.511 + nudge),
      },
      {
        priority: ["Q", "E", "W"],
        sequence: ["Q", "E", "W", "Q", "Q", "R", "Q", "E", "Q", "E", "R", "E", "E", "W", "W"],
        rates: mode === "arena" ? arena(0.15, 4.48, 0.11) : rates(0.15, 0.487),
      },
    ],
    matchups: { good: [], bad: [] },
    augments: [],
    dropped: 0,
  };
  if (mode !== "hextech" && mode !== "arena") {
    build.spells = [
      { spells: [4, mode === "aram" ? 32 : 14], rates: rates(0.712 - nudge, 0.503 + nudge) },
      { spells: [4, mode === "aram" ? 14 : 12], rates: rates(0.203, 0.496) },
    ];
    build.runes = [
      {
        page: {
          primaryStyle: 8100,
          subStyle: 8200,
          perks: [8112, 8126, 8138, 8135, 8210, 8237, 5008, 5008, 5001],
        },
        rates: rates(0.4718 - nudge, 0.4914 + nudge, 585_750),
      },
      {
        page: {
          primaryStyle: 8200,
          subStyle: 8100,
          perks: [8229, 8226, 8210, 8237, 8139, 8135, 5008, 5008, 5001],
        },
        rates: rates(0.2133, 0.5031, 264_782),
      },
    ];
  }
  if (mode !== "arena") {
    build.starting = [items([1056, 2003, 2003], 0.745, 0.497), items([1082, 2003], 0.113, 0.488)];
  }
  build.boots = [items([3020], 0.899, 0.506), items([3006], 0.061, 0.494)];
  build.core = [
    items([6655, 4645, 3089], 0.203 - nudge, 0.552 + nudge),
    items([6655, 3089, 3135], 0.072, 0.539),
  ];
  build.late = [
    items([3157], 0.366, 0.596),
    items([3135], 0.201, 0.573),
    items([3165], 0.161, 0.577),
  ];
  if (rift) {
    build.matchups = {
      good: [
        { championId: 81, rates: rates(0, 0.5409) },
        { championId: 99, rates: rates(0, 0.5316) },
      ],
      bad: [
        { championId: 238, rates: rates(0, 0.4398) },
        { championId: 157, rates: rates(0, 0.4553) },
      ],
    };
  }
  if (mode === "hextech") {
    build.augments = [
      { id: 1336, rarity: "gold", tier: "S", rates: rates(0.2073, 0.511) },
      { id: 1220, rarity: "prismatic", tier: "S", rates: rates(0.1468, 0.523) },
      { id: 1004, rarity: "prismatic", tier: "A", rates: rates(0.0501, 0.518) },
      { id: 2103, rarity: "gold", tier: "A", rates: rates(0.0412, 0.502) },
      { id: 1320, rarity: "silver", tier: "B", rates: rates(0.1081, 0.489) },
      { id: 2102, rarity: "silver", tier: "C", rates: rates(0.0233, 0.461) },
    ];
  }
  if (mode === "arena") {
    build.augments = [
      { id: 220, rarity: "prismatic", tier: null, rates: arena(0.1468, 4.14, 0.201) },
      { id: 195, rarity: "prismatic", tier: null, rates: arena(0.0811, 4.26, 0.176) },
      { id: 84, rarity: "gold", tier: null, rates: arena(0.0688, 4.33, 0.181) },
      { id: 206, rarity: "silver", tier: null, rates: arena(0.0954, 4.33, 0.181) },
    ];
  }
  return build;
}

type LoadoutCommand =
  | "get_build"
  | "apply_runes"
  | "apply_spells"
  | "write_item_set"
  | "clear_item_sets"
  | "get_loadout_summary"
  | "clear_loadouts";

/** The demo's answers to the build panel and the rule cards. */
export function demoLoadoutHandlers(settings: () => Settings): {
  [K in LoadoutCommand]: (args: Commands[K]["args"]) => Commands[K]["result"];
} {
  let remembered = 3;
  const sets = new Set<string>();
  return {
    get_build: ({ championId, mode, lane }) => demoBuild(championId, mode, lane, settings()),
    apply_runes: () => "written",
    apply_spells: () => null,
    write_item_set: ({ championId, mode }) => {
      sets.add(`${championId}-${mode}`);
      return null;
    },
    clear_item_sets: () => {
      const removed = sets.size;
      sets.clear();
      return removed;
    },
    get_loadout_summary: () => ({ remembered }),
    clear_loadouts: () => {
      remembered = 0;
      return { remembered };
    },
  };
}
