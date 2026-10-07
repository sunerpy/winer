// ARAM's bench in the client's champ select: the client greys a bench champion out for a few
// seconds after it lands there and ignores clicks meanwhile. With the setting on, the cooldown
// class and mask come off, and a click on a bench champion is carried out by winer at once
// (`POST /lol-champ-select/v1/session/bench/swap/{id}`, which does not wait out the cooldown).
// Selectors are the client's own class names and change between patches: a miss leaves the
// client's own behaviour alone.
export const BENCH_SELECTOR = ".bench-container";
const COOLDOWN = "on-cooldown";
const HIJACKED = "data-winer-bench-hijacked";
const EMPTY = "empty-bench-item";
const LOCKED = "locked-out";

/** Hides the cooldown masks and lets disabled bench items take clicks; toggled with the setting. */
export const BENCH_STYLE = `${BENCH_SELECTOR} .cooldown-mask { display: none !important; }
${BENCH_SELECTOR} .champion-bench-item, ${BENCH_SELECTOR} .bench-champion-background { pointer-events: auto !important; filter: none !important; opacity: 1 !important; }`;

/** Takes every cooldown class off the bench. Returns how many elements it changed, so an
 *  unchanged bench writes nothing and the mutation observer stays quiet. */
export function liftBenchCooldown(root: ParentNode): number {
  let changed = 0;
  for (const container of root.querySelectorAll(BENCH_SELECTOR)) {
    for (const element of container.querySelectorAll<HTMLElement>(`[class*="${COOLDOWN}"]`)) {
      const names = [...element.classList].filter((name) => name.includes(COOLDOWN));
      if (names.length === 0) continue;
      element.classList.remove(...names);
      changed += 1;
    }
  }
  return changed;
}

const ITEM_SELECTOR = `${BENCH_SELECTOR} .champion-bench-item`;
const CHAMPION_ICON = /champion-icons\/(\d+)\.png/;

/** The champion a bench item shows, read from its icon: an <img> or a background image. */
export function benchChampion(item: Element): number | null {
  for (const element of [item, ...item.querySelectorAll("[src], [style]")]) {
    for (const source of [element.getAttribute("src"), element.getAttribute("style")]) {
      const match = source ? CHAMPION_ICON.exec(source) : null;
      if (match) return Number(match[1]);
    }
  }
  return null;
}

/** Gives each usable bench item its own capture handler. The client recreates these nodes during
 *  champ select, so the mutation-driven render calls this again for new items. */
export function hijackBenchItems(root: ParentNode, swap: (championId: number) => boolean): number {
  let hijacked = 0;
  for (const container of root.querySelectorAll(BENCH_SELECTOR)) {
    for (const mask of container.querySelectorAll<HTMLElement>(".cooldown-mask")) {
      mask.style.setProperty("display", "none", "important");
    }
    for (const item of container.querySelectorAll<HTMLElement>(ITEM_SELECTOR)) {
      if (
        item.hasAttribute(HIJACKED) ||
        item.classList.contains(EMPTY) ||
        item.classList.contains(LOCKED)
      )
        continue;
      item.setAttribute(HIJACKED, "true");
      item.addEventListener(
        "click",
        (event) => {
          if (item.classList.contains(EMPTY) || item.classList.contains(LOCKED)) return;
          const champion = benchChampion(item);
          if (champion === null || !swap(champion)) return;
          event.preventDefault();
          event.stopImmediatePropagation();
        },
        true,
      );
      hijacked += 1;
    }
  }
  return hijacked;
}

/** The bench item a click was meant for: the target's own, or, when something else is drawn on top
 *  of the bench (Pengu's empty `#pengu-root` was measured covering it in champ select, 16.19), the
 *  one under the click's point. */
function clickedItem(doc: Document, event: MouseEvent): Element | null {
  const direct = event.target instanceof Element ? event.target.closest(ITEM_SELECTOR) : null;
  if (direct || !doc.querySelector(BENCH_SELECTOR) || typeof doc.elementsFromPoint !== "function") {
    return direct;
  }
  for (const element of doc.elementsFromPoint(event.clientX, event.clientY)) {
    const item = element.closest(ITEM_SELECTOR);
    if (item) return item;
  }
  return null;
}

/** Hands clicks on bench champions to `swap` in the capture phase. When `swap` takes one (returns
 *  true) the client's own handler, and with it the cooldown, never sees the click. */
export function interceptBenchClicks(
  doc: Document,
  swap: (championId: number) => boolean,
): () => void {
  const onClick = (event: MouseEvent) => {
    const direct = event.target instanceof Element ? event.target.closest(ITEM_SELECTOR) : null;
    // A direct click reaches the item's own handler later in the capture path. This global handler
    // remains for overlays and for a newly drawn item before the next render.
    if (direct?.hasAttribute(HIJACKED)) return;
    const item = direct ?? clickedItem(doc, event);
    if (!item || item.classList.contains(EMPTY) || item.classList.contains(LOCKED)) return;
    const champion = benchChampion(item);
    if (champion === null || !swap(champion)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  doc.addEventListener("click", onClick, true);
  return () => doc.removeEventListener("click", onClick, true);
}
