// The kinds of game the automation tells apart, and where each rule can act at all: the same lists
// as the core's `Scoped::applicable` (crates/core/src/settings.rs).
import type { Mode, Scopes } from "@winer/shared";

import type { MessageKey } from "./i18n";

export type ScopedRule = keyof Scopes;

export const MODES: readonly Mode[] = ["ranked", "normal", "aram", "hextech", "arena", "other"];
/** Modes with a pick and a ban phase; ARAM and Hextech ARAM hand out champions instead. */
const PICKED: readonly Mode[] = ["ranked", "normal", "arena", "other"];

export const APPLICABLE: Record<ScopedRule, readonly Mode[]> = {
  accept: MODES,
  pick: PICKED,
  ban: PICKED,
  callout: MODES,
  bench: ["aram", "hextech"],
  playAgain: MODES,
  // Arena has no rune page and hands everyone the same two spells; item sets need a map's shop.
  loadout: ["ranked", "normal", "aram", "hextech", "other"],
  itemSets: ["ranked", "normal", "aram", "hextech", "arena"],
};

export const MODE_LABEL: Record<Mode, MessageKey> = {
  ranked: "mode.ranked",
  normal: "mode.normal",
  aram: "mode.aram",
  hextech: "mode.hextech",
  arena: "mode.arena",
  other: "mode.other",
};

/** A mode's name where room is short: a match tile. */
export const MODE_SHORT: Record<Mode, MessageKey> = {
  ranked: "mode.short.ranked",
  normal: "mode.short.normal",
  aram: "mode.short.aram",
  hextech: "mode.short.hextech",
  arena: "mode.short.arena",
  other: "mode.short.other",
};

/** The kind of a queue of `gameMode`, as the core's `Mode::of` tells them apart. */
export function modeOf(gameMode: string, ranked: boolean): Mode {
  switch (gameMode.toUpperCase()) {
    case "CLASSIC":
    case "SWIFTPLAY":
      return ranked ? "ranked" : "normal";
    case "ARAM":
      return "aram";
    case "KIWI":
      return "hextech";
    case "CHERRY":
      return "arena";
    default:
      return "other";
  }
}

/** Every rule everywhere it can act, as the core starts out. */
export function defaultScopes(): Scopes {
  return {
    accept: [...APPLICABLE.accept],
    pick: [...APPLICABLE.pick],
    ban: [...APPLICABLE.ban],
    callout: [...APPLICABLE.callout],
    bench: [...APPLICABLE.bench],
    playAgain: [...APPLICABLE.playAgain],
    loadout: [...APPLICABLE.loadout],
    itemSets: [...APPLICABLE.itemSets],
  };
}

/** Whether `rule`, scoped to `modes`, acts everywhere it can. */
export function everywhere(rule: ScopedRule, modes: readonly Mode[]): boolean {
  return APPLICABLE[rule].every((mode) => modes.includes(mode));
}

/** `modes` with `mode` switched, in the canonical order. */
export function toggled(rule: ScopedRule, modes: readonly Mode[], mode: Mode): Mode[] {
  const next = modes.includes(mode) ? modes.filter((other) => other !== mode) : [...modes, mode];
  return APPLICABLE[rule].filter((other) => next.includes(other));
}
