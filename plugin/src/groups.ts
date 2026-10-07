// Groups in the client: friends in one game or party, a premade party in champ select. Six colours
// in turn, the desktop's `--group-*` tokens by value (style.css), always beside the number.
import type { Language } from "@winer/shared";

import { h } from "./dom";
import { text } from "./i18n";

/** The colour slot, 1–6, of group `group` (1 up): the colours repeat past six, the number does not. */
export function groupSlot(group: number): number {
  return ((((Math.trunc(group) - 1) % 6) + 6) % 6) + 1;
}

/** `■ 开黑 1`, in the group's colour; a party read from recent games reads `疑似开黑 1` with a
 *  dashed swatch. */
export function premadeChip(group: number, language: Language, inferred = false): HTMLElement {
  return h(
    "span",
    {
      class: inferred ? "winer-group winer-group--inferred" : "winer-group",
      "data-winer-group": String(groupSlot(group)),
    },
    `${text(language, inferred ? "premadeInferred" : "premade")} ${group}`,
  );
}
