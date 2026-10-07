import type { AugmentInfo, AugmentOption, Build, NoticeKind } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { demoBuild } from "../../lib/demoLoadout";
import { translate } from "../../lib/i18n";
import { noticeText } from "../../shell/notices";
import {
  augmentGroups,
  availableTabs,
  defaultTab,
  hasNumbers,
  rate,
  rateTone,
  sourceOff,
  tabsOf,
} from "./builds";

const BUILDS = {
  enabled: true,
  riftSource: "tencent",
  aramSource: "opGg",
  arenaSource: "opGg",
  hextechFallback: true,
  recommend: true,
} as const;

const SETTINGS = { builds: BUILDS } as Parameters<typeof demoBuild>[3];

const augment = (id: number, rarity: AugmentOption["rarity"]): AugmentOption => ({
  id,
  rarity,
  tier: null,
  rates: { pick: 0.1, win: 0.5, games: null, placement: null, first: null },
});

describe("build panel", () => {
  it("offers the sections a mode has and keeps only those its build has numbers for", () => {
    expect(tabsOf("ranked")).toEqual(["items", "runes", "spells", "skills", "matchups"]);
    expect(tabsOf("hextech")).toEqual(["items", "runes", "spells", "skills", "augments"]);
    expect(tabsOf("aram")).not.toContain("matchups");
    expect(hasNumbers("other")).toBe(false);

    // A switched-off source takes only its own mode's numbers away.
    expect(sourceOff("aram", BUILDS)).toBe(false);
    expect(sourceOff("aram", { ...BUILDS, aramSource: "off" })).toBe(true);
    expect(sourceOff("arena", { ...BUILDS, aramSource: "off" })).toBe(false);
    expect(sourceOff("arena", { ...BUILDS, arenaSource: "off" })).toBe(true);
    expect(sourceOff("hextech", { ...BUILDS, aramSource: "off", hextechFallback: false })).toBe(
      false,
    );
    expect(() =>
      demoBuild(103, "aram", null, { builds: { ...BUILDS, aramSource: "off" } } as typeof SETTINGS),
    ).toThrow();

    // Tencent's Hextech numbers have no runes or spells: no empty sections that look like data.
    const hextech = demoBuild(103, "hextech", null, SETTINGS);
    expect(availableTabs(hextech)).toEqual(["items", "skills", "augments"]);
    expect(defaultTab(availableTabs(hextech), "hextech", true)).toBe("augments");
    expect(defaultTab(availableTabs(hextech), "hextech", false)).toBe("items");
    const rift: Build = { ...demoBuild(103, "ranked", "middle", SETTINGS), runes: [] };
    expect(availableTabs(rift)).toEqual(["items", "spells", "skills", "matchups"]);
    expect(defaultTab([], "ranked", false)).toBe("items");
  });

  it("reads rates as percentages with one decimal and colours only the clear ones", () => {
    expect(rate(0.4718)).toBe("47.2%");
    expect(rate(null)).toBe("—");
    expect(rateTone(0.53)).toBe("text-win");
    expect(rateTone(0.5)).toBe("text-fg");
    expect(rateTone(0.47)).toBe("text-loss");
  });

  it("groups augments by the client's rarity, prismatic first, each in the build's order", () => {
    const catalog = new Map<number, AugmentInfo>([
      [1, { id: 1, name: "升级：无尽之刃", icon: "", rarity: "gold" }],
      [2, { id: 2, name: "连拨击锤", icon: "", rarity: "prismatic" }],
      [3, { id: 3, name: "大力", icon: "", rarity: "silver" }],
      [4, { id: 4, name: "易损", icon: "", rarity: "gold" }],
    ]);
    const augments = [augment(1, "gold"), augment(2, "prismatic"), augment(3, "silver")];
    augments.push(augment(4, "gold"));
    const groups = augmentGroups(augments, catalog, "");
    expect(groups.map((group) => group.rarity)).toEqual(["prismatic", "gold", "silver"]);
    expect(groups[1]?.augments.map((option) => option.id)).toEqual([1, 4]);

    expect(augmentGroups(augments, catalog, " 无尽 ").flatMap((group) => group.augments)).toEqual([
      augments[0],
    ]);
    // A description matches too, where ARAM.GG's are on.
    const details = new Map([[3, "获得 10% 攻击力"]]);
    expect(augmentGroups(augments, catalog, "攻击力", details)).toEqual([
      { rarity: "silver", augments: [augments[2]] },
    ]);
    expect(augmentGroups(augments, catalog, "nothing")).toEqual([]);
  });

  it("says in the activity feed what was set up, and why the runes were not", () => {
    const zh = (kind: NoticeKind) =>
      noticeText(kind, (key, params) => translate("zh-CN", key, params), null);
    const applied = (runes: "written" | "noPage" | null, spells: boolean, recommended = false) =>
      zh({ kind: "loadoutApplied", championId: 202, recommended, runes, spells });
    expect(applied("written", true)).toBe("为 #202 应用了记住的符文和召唤师技能");
    expect(applied("written", false, true)).toBe("为 #202 应用了客户端推荐的符文");
    expect(applied(null, true)).toBe("为 #202 应用了记住的召唤师技能");
    expect(applied("noPage", true)).toBe(
      "为 #202 应用了记住的召唤师技能；符文没有改动：没有空位放 winer 的符文页",
    );
    expect(applied("noPage", false)).toBe("#202 的符文没有改动：没有空位放 winer 的符文页");
    expect(zh({ kind: "itemSetWritten", championId: 202 })).toBe("已为 #202 写入装备方案");
  });
});
