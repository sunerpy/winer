// The build panel's arithmetic: which sections a mode and a build have, how rates read, and the
// augments in the client's rarities, best first.
import type {
  AugmentInfo,
  AugmentOption,
  Build,
  BuildSource,
  Mode,
  Position,
  Rarity,
} from "@winer/shared";

import type { MessageKey } from "../../lib/i18n";

export type BuildTab = "items" | "runes" | "spells" | "skills" | "matchups" | "augments";

export const LANES: readonly Position[] = ["top", "jungle", "middle", "bottom", "utility"];

export const SOURCE_LABEL: Record<BuildSource, MessageKey> = {
  tencent: "loadout.source.tencent",
  tencentHextech: "loadout.source.tencentHextech",
  opGg: "loadout.source.opGg",
  aramGg: "loadout.source.aramGg",
};

/** The modes the panel can look a champion up in, Summoner's Rift once. */
export const LOOKUP_MODES: readonly Mode[] = ["ranked", "aram", "hextech", "arena"];

/** Summoner's Rift, where lanes and matchups mean something. */
export function isRift(mode: Mode): boolean {
  return mode === "ranked" || mode === "normal";
}

/** Whether any source has numbers for the mode (`builds::sources` in the core). */
export function hasNumbers(mode: Mode): boolean {
  return mode !== "other";
}

/** Augments are picked in Arena and Hextech ARAM. */
export function hasAugments(mode: Mode): boolean {
  return mode === "arena" || mode === "hextech";
}

/** The sections a mode can have, in the panel's order. */
export function tabsOf(mode: Mode): BuildTab[] {
  const tabs: BuildTab[] = ["items", "runes", "spells", "skills"];
  if (isRift(mode)) tabs.push("matchups");
  if (hasAugments(mode)) tabs.push("augments");
  return tabs;
}

/** The sections this build has numbers for: an empty one would look like data that is not there. */
export function availableTabs(build: Build): BuildTab[] {
  const has: Record<BuildTab, boolean> = {
    items: build.starting.length + build.boots.length + build.core.length + build.late.length > 0,
    runes: build.runes.length > 0,
    spells: build.spells.length > 0,
    skills: build.skillOrders.length > 0,
    matchups: build.matchups.good.length + build.matchups.bad.length > 0,
    augments: build.augments.length > 0,
  };
  return tabsOf(build.mode).filter((tab) => has[tab]);
}

/** The section to open: augments during a game where they are picked (augments are chosen in the
 *  game), otherwise the first. */
export function defaultTab(tabs: readonly BuildTab[], mode: Mode, inGame: boolean): BuildTab {
  if (inGame && hasAugments(mode) && tabs.includes("augments")) return "augments";
  return tabs[0] ?? "items";
}

/** A share as a percentage with one decimal: `47.2%`; a dash where the source gave none. */
export function rate(value: number | null): string {
  return value === null ? "—" : `${(value * 100).toFixed(1)}%`;
}

/** The win colour for a clearly good rate, the loss colour for a clearly bad one. Build rates sit
 *  close to half, so two points either way already say something. */
export function rateTone(value: number | null): string {
  if (value === null) return "text-fg-subtle";
  if (value >= 0.52) return "text-win";
  if (value <= 0.48) return "text-loss";
  return "text-fg";
}

export interface AugmentGroup {
  rarity: Rarity;
  augments: AugmentOption[];
}

/** Prismatic first, as the game deals them. */
const RARITIES: readonly Rarity[] = ["prismatic", "gold", "silver"];

/** The augments by the client's rarity, each group in the build's order (best first), keeping
 *  only those whose name, or description, holds `filter`. */
export function augmentGroups(
  augments: readonly AugmentOption[],
  catalog: ReadonlyMap<number, AugmentInfo> | undefined,
  filter: string,
  descriptions?: ReadonlyMap<number, string> | null,
): AugmentGroup[] {
  const needle = filter.trim().toLowerCase();
  const matches = (augment: AugmentOption) =>
    !needle ||
    [catalog?.get(augment.id)?.name, descriptions?.get(augment.id)].some((text) =>
      text?.toLowerCase().includes(needle),
    );
  return RARITIES.map((rarity) => ({
    rarity,
    augments: augments.filter((augment) => augment.rarity === rarity && matches(augment)),
  })).filter((group) => group.augments.length > 0);
}
