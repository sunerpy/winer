// The client's home page while 隐藏首页推广 is on. On the Tencent client the Home tab is the
// activity centre, and almost all of it is one iframe of Tencent's news and events hub: hiding the
// activity centre, as the option first did, left the whole tab black. The option now hides the
// esports pop-ups outright and puts a small note of winer's in the hub's own box, beside the hub.
// The hub is hidden by a rule that holds only while the note is there, so wherever the note cannot
// be drawn the page stays the client's own. The note's button brings the hub back until the client
// restarts.
import type { Language } from "@winer/shared";

import { h } from "./dom";
import { text } from "./i18n";
import { HOME, PROMOTION_POPUPS } from "./selectors";

const NOTE_CLASS = "winer-home";
/** On <html> once the hub was asked back, for every context of the page to read. */
const SHOWN_ATTRIBUTE = "data-winer-hub";
/** The same in the page's session storage, which outlives a reload of the page but not a restart
 *  of the client's interface. */
const SHOWN_KEY = "winer-hub-shown";

/** The option's stylesheet: the pop-ups, and the hub while winer's note sits beside it. */
export const PROMOTIONS = `${PROMOTION_POPUPS.join(", ")} { display: none !important; }
${HOME.centre} :has(> .${NOTE_CLASS}) > iframe { display: none !important; }`;

/** What the home page shows: no note (`off`), no hub to put one beside (`missing`), the hub asked
 *  back for this session (`shown`), or the note in the hub's place (`noted`). */
export type HomeState = "off" | "missing" | "shown" | "noted";

/** Whether the hub was asked back in this client session. */
export function hubShown(doc: Document): boolean {
  if (doc.documentElement.hasAttribute(SHOWN_ATTRIBUTE)) return true;
  try {
    return doc.defaultView?.sessionStorage.getItem(SHOWN_KEY) === "1";
  } catch {
    return false;
  }
}

function showHub(doc: Document): void {
  doc.documentElement.setAttribute(SHOWN_ATTRIBUTE, "shown");
  try {
    doc.defaultView?.sessionStorage.setItem(SHOWN_KEY, "1");
  } catch {
    // Storage the page may not use: the attribute holds it until the page reloads.
  }
}

/** Forgets that the hub was asked back: the option is off, and once on again it hides the hub. */
export function forgetHub(doc: Document): void {
  doc.documentElement.removeAttribute(SHOWN_ATTRIBUTE);
  try {
    doc.defaultView?.sessionStorage.removeItem(SHOWN_KEY);
  } catch {
    // As above: the attribute was all there was.
  }
}

/** The note: one line saying the option hides the promotions, and a button that brings the hub
 *  back until the client restarts. */
function note(doc: Document, language: Language, context: string): HTMLElement {
  const show = h(
    "button",
    { type: "button", class: "winer-home-show" },
    text(language, "homeShow"),
  );
  const element = h(
    "div",
    { class: NOTE_CLASS, "data-winer-context": context, "data-winer-language": language },
    h(
      "div",
      { class: "winer-home-card" },
      h("span", { class: "winer-home-mark" }, "winer"),
      h("p", { class: "winer-home-text" }, text(language, "homeHidden")),
      h(
        "div",
        { class: "winer-home-actions" },
        show,
        h("span", { class: "winer-home-hint" }, text(language, "homeShowHint")),
      ),
    ),
  );
  show.addEventListener("click", (event) => {
    // The client's own handlers around the hub have no business with this click.
    event.preventDefault();
    event.stopPropagation();
    showHub(doc);
    element.remove();
  });
  return element;
}

/** Puts the note in the hub's place while `on`, unless the hub was asked back; otherwise takes
 *  every note off. Only the context that draws calls it: a note another context left (it drew
 *  before this one took over) gives way to this context's own, whose button this context answers.
 *  Writes nothing while the page already shows what it should, so the observer stays quiet. */
export function decorateHome(
  doc: Document,
  on: boolean,
  language: Language,
  context: string,
): HomeState {
  const shown = on && hubShown(doc);
  const hub = on && !shown ? doc.querySelector(`${HOME.centre} ${HOME.hub}`) : null;
  const box = hub?.parentElement ?? null;
  let kept: Element | null = null;
  for (const found of doc.querySelectorAll(`.${NOTE_CLASS}`)) {
    const current =
      kept === null &&
      box !== null &&
      found.parentElement === box &&
      found.getAttribute("data-winer-context") === context &&
      found.getAttribute("data-winer-language") === language;
    if (current) kept = found;
    else found.remove();
  }
  if (!on) return "off";
  if (shown) return "shown";
  if (!hub || !box) return "missing";
  // After the hub, not before: the box's first child stays the client's own iframe.
  if (!kept) hub.after(note(doc, language, context));
  return "noted";
}
