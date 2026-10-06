// The global shortcut's combinations as the recorder in Settings › 通用 reads them off a keydown:
// written the way the core keeps them (`normalize_hotkey` in crates/core/src/settings.rs), which
// is also what the shell registers.

/** Keys a combination can end in besides letters, digits and F1–F24, by `KeyboardEvent.code`:
 *  the core's `HOTKEY_KEYS`, which the shell's shortcut parser reads. */
const NAMED: Record<string, string> = {
  Space: "Space",
  Insert: "Insert",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Backquote: "Backquote",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  NumpadAdd: "NumAdd",
  NumpadSubtract: "NumSubtract",
  NumpadMultiply: "NumMultiply",
  NumpadDivide: "NumDivide",
  NumpadDecimal: "NumDecimal",
  Pause: "Pause",
  ScrollLock: "ScrollLock",
  PrintScreen: "PrintScreen",
};

/** What the keycaps show for a key whose name is not what is printed on it. */
const CAPS: Record<string, string> = {
  Super: "Win",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  NumAdd: "Num +",
  NumSubtract: "Num -",
  NumMultiply: "Num *",
  NumDivide: "Num /",
  NumDecimal: "Num .",
};

const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

/** The key `code` names in the settings' spelling; `null` for a modifier or a key that cannot end
 *  a combination (Enter, Escape, Tab…). */
export function keyOfCode(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1] ?? null;
  const digit = /^(?:Digit|Numpad)(\d)$/.exec(code);
  if (digit) return code.startsWith("Numpad") ? `Num${digit[1]}` : (digit[1] ?? null);
  const fn = /^F(\d{1,2})$/.exec(code);
  if (fn && Number(fn[1]) >= 1 && Number(fn[1]) <= 24) return `F${Number(fn[1])}`;
  return NAMED[code] ?? null;
}

export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export type Recorded =
  /** Modifiers held, no key yet: `Ctrl+Shift+…` so far. */
  | { kind: "partial"; keys: string[] }
  | { kind: "done"; combo: string }
  /** A key the shortcut cannot use, or no Ctrl, Alt or Win with it. */
  | { kind: "invalid" };

/** What a keydown makes of the combination being recorded. */
export function record(press: KeyPress): Recorded {
  const modifiers = [
    press.ctrlKey && "Ctrl",
    press.altKey && "Alt",
    press.shiftKey && "Shift",
    press.metaKey && "Super",
  ].filter((name): name is string => Boolean(name));
  if (MODIFIER_CODES.test(press.code)) return { kind: "partial", keys: modifiers };
  const key = keyOfCode(press.code);
  // Shift alone does not count: Shift and a letter is typing.
  if (!key || !(press.ctrlKey || press.altKey || press.metaKey)) return { kind: "invalid" };
  return { kind: "done", combo: [...modifiers, key].join("+") };
}

/** A combination's keycaps, as printed on the keyboard: `Ctrl`, `Shift`, `W`. */
export function keycaps(combo: string): string[] {
  return combo
    .split("+")
    .filter(Boolean)
    .map((key) => CAPS[key] ?? key);
}
