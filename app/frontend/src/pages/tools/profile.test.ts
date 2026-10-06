import type { ChampionInfo, SkinChoice } from "@winer/shared";
import { describe, expect, it } from "vitest";

import { isAvailability, namedStatus } from "../../lib/presence";
import {
  backupSize,
  backupTime,
  filterSkins,
  hasDivisions,
  matchesSkin,
  restoreChannels,
} from "./profile";

const AHRI: ChampionInfo = {
  id: 103,
  name: "九尾妖狐",
  shortName: "阿狸",
  alias: "Ahri",
  icon: "",
};
const skin = (id: number, name: string, owned: boolean, championId = 103): SkinChoice => ({
  id,
  championId,
  name,
  owned,
  base: id % 1000 === 0,
  tile: "",
  splash: "",
});
const SKINS = [
  skin(103000, "九尾妖狐", true),
  skin(103015, "K/DA 阿狸", true),
  skin(103016, "至臻 K/DA 阿狸", false),
  skin(1001, "哥特萝莉 安妮", false, 1),
];

describe("profile tools", () => {
  it("finds a skin by its own name or its champion's, in either language", () => {
    expect(matchesSkin(SKINS[0] as SkinChoice, AHRI, "阿狸"), "the base skin by short name").toBe(
      true,
    );
    expect(matchesSkin(SKINS[1] as SkinChoice, AHRI, "k/da")).toBe(true);
    expect(matchesSkin(SKINS[1] as SkinChoice, AHRI, " AHRI ")).toBe(true);
    expect(matchesSkin(SKINS[3] as SkinChoice, undefined, "阿狸")).toBe(false);
    expect(matchesSkin(SKINS[3] as SkinChoice, undefined, "")).toBe(true);
  });

  it("narrows the skins by search, champion and ownership together", () => {
    const champions = new Map([[103, AHRI]]);
    const ids = (query: string, champion: number | null, ownedOnly: boolean) =>
      filterSkins(SKINS, champions, { query, champion, ownedOnly }).map((found) => found.id);
    expect(ids("", null, false)).toEqual([103000, 103015, 103016, 1001]);
    expect(ids("", null, true)).toEqual([103000, 103015]);
    expect(ids("", 1, false)).toEqual([1001]);
    expect(ids("至臻", 103, false)).toEqual([103016]);
    expect(ids("至臻", 103, true)).toEqual([]);
  });

  it("drops the division from Master up", () => {
    expect(hasDivisions("DIAMOND")).toBe(true);
    expect(hasDivisions("IRON")).toBe(true);
    expect(hasDivisions("MASTER")).toBe(false);
    expect(hasDivisions("CHALLENGER")).toBe(false);
  });

  it("restores one half of the settings or both", () => {
    expect(restoreChannels("general")).toEqual(["general"]);
    expect(restoreChannels("hotkeys")).toEqual(["hotkeys"]);
    expect(restoreChannels("all")).toEqual(["general", "hotkeys"]);
  });

  it("describes a backup by its time and size", () => {
    expect(backupSize(8402)).toBe("8.2 KB");
    expect(backupSize(0)).toBe("0.0 KB");
    expect(backupTime(new Date(2026, 9, 6, 9, 5).getTime())).toBe("2026-10-06 09:05");
  });

  it("names the statuses the client takes and the one it sets itself", () => {
    expect(isAvailability("mobile")).toBe(true);
    expect(isAvailability("dnd"), "the client's own, never winer's").toBe(false);
    expect(isAvailability(null)).toBe(false);
    expect(namedStatus("dnd")).toBe("dnd");
    expect(namedStatus("spectating")).toBeNull();
  });
});
