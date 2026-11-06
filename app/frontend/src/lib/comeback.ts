// The Comebacks (反击) and Phrases (快捷语) pages as the core keeps them (crates/core/src/comeback.rs
// and settings.rs): their limits, champ select's default quick phrases and how a phrase reads for one
// teammate.
import type { ComebackSettings, Language } from "@winer/shared";

/** At most this many lines, and quick phrases, of this many characters each
 *  (`ComebackSettings::LINE_LIMIT`, `PHRASE_LIMIT`, `LINE_LENGTH`); the core clips on save. */
export const LINE_LIMIT = 100;
export const PHRASE_LIMIT = 20;
export const LINE_LENGTH = 200;

/** The language's quick phrases while the user has none (`comeback::default_phrases`): the
 *  owner's own words, with the teammate's seat and champion. */
export const DEFAULT_PHRASES: Record<Language, readonly string[]> = {
  "zh-CN": ["{seat}大佬，小弟想玩下{champion}，给您磕一个了"],
  en: ["{seat}, may I play {champion}? Thank you!"],
};

/** The language's thank-you while the user has written none (`comeback::default_thanks`). */
export const DEFAULT_THANKS: Record<Language, string> = {
  "zh-CN": "谢谢{seat}大佬！",
  en: "Thanks, {seat}!",
};

/** The thank-you a trade that went through sends (`comeback::thanks`). */
export function thanksOf(rule: ComebackSettings, language: Language): string {
  return rule.trade.thanks === "" ? DEFAULT_THANKS[language] : rule.trade.thanks;
}

/** The quick phrases the Live page offers, in the order `send_phrase` numbers them
 *  (`comeback::phrases`): the user's, or the language's own while there are none. */
export function phrasesOf(rule: ComebackSettings, language: Language): readonly string[] {
  return rule.phrases.length > 0 ? rule.phrases : DEFAULT_PHRASES[language];
}

/** A seat as the callout names it, 1 for the first in the team's list (`callout::seat_label`). */
export function seatLabel(seat: number, language: Language): string {
  return language === "zh-CN" ? `${seat}L` : `P${seat}`;
}

/** The brackets a line may hold a value in, opening and closing (`callout::BRACKETS`). */
const BRACKETS: readonly (readonly [string, string])[] = [
  ["【", "】"],
  ["「", "」"],
  ["[", "]"],
  ["(", ")"],
  ["（", "）"],
];

/** Stands in for a value left blank until the brackets it emptied are taken away. */
const BLANK = "\uE000";

/** `text` without the pairs of brackets a blank value emptied (`callout::without_emptied_brackets`):
 *  a pair the template itself left empty holds no mark and stays. */
function withoutEmptiedBrackets(text: string): string {
  let kept = "";
  for (const character of text) {
    const pair = BRACKETS.find(([, close]) => close === character);
    if (!pair) {
      kept += character;
      continue;
    }
    const inside = kept.replace(/[ \uE000]+$/u, "");
    const emptied = kept.slice(inside.length).includes(BLANK);
    if (emptied && inside.endsWith(pair[0])) kept = `${inside.slice(0, -pair[0].length)}${BLANK}`;
    else kept += character;
  }
  return kept;
}

const replaceAll = (text: string, key: string, value: string) => text.split(key).join(value);

/** `template` with each placeholder replaced by its value, as the core fills a line
 *  (`callout::fill`): a blank value takes the brackets it sat in along, full-width punctuation
 *  takes no space on either side, and no separator is left at either end. */
export function fill(template: string, values: readonly (readonly [string, string])[]): string {
  let text = template;
  for (const [key, value] of values) text = replaceAll(text, key, value === "" ? BLANK : value);
  text = replaceAll(withoutEmptiedBrackets(text), BLANK, "")
    .split(/\s+/u)
    .filter(Boolean)
    .join(" ");
  for (const mark of ["：", "，", "；"])
    text = replaceAll(replaceAll(text, ` ${mark}`, mark), `${mark} `, mark);
  text = replaceAll(replaceAll(text, " ,", ","), " : ", ": ");
  return text.replace(/^[｜| ]+|[｜| ]+$/gu, "");
}

/** A quick phrase for the teammate in `seat` (`seatLabel`) on `champion` (blank while none is
 *  shown), called `name` (blank when hidden), as `send_phrase` sends it (`comeback::phrase`). */
export function fillPhrase(template: string, seat: string, champion: string, name: string): string {
  return fill(template, [
    ["{seat}", seat],
    ["{champion}", champion],
    ["{name}", name],
  ]);
}
