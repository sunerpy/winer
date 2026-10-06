// Groups in the client: friends in one game or party, a premade party in champ select. Six colours
// in turn, the desktop's `--group-*` tokens by value (style.css), always beside the number.
import type { Language } from "@winer/shared";

import { h } from "./dom";
import { text } from "./i18n";

/** The colour slot, 1–6, of group `group` (1 up): the colours repeat past six, the number does not. */
export function groupSlot(group: number): number {
  return ((((Math.trunc(group) - 1) % 6) + 6) % 6) + 1;
}

/** `■ 开黑 1`, in the group's colour. */
export function premadeChip(group: number, language: Language): HTMLElement {
  return h(
    "span",
    { class: "winer-group", "data-winer-group": String(groupSlot(group)) },
    `${text(language, "premade")} ${group}`,
  );
}
