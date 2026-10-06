// Finding the client's own elements from lists of guessed selectors, tried in order: a miss finds
// nothing rather than throwing, and a guess that matches more than it should still picks the level
// of the page that was meant.

/** The first element under `root` that one of `selectors` matches, the selectors tried in order. */
export function first(root: ParentNode, selectors: readonly string[]): Element | null {
  for (const selector of selectors) {
    const found = root.querySelector(selector);
    if (found) return found;
  }
  return null;
}

/** The elements under `root` that the first selector with a usable match finds, keeping those
 *  `usable` accepts, and of two where one holds the other, the inner one. A guessed class such as
 *  `[class*='roster-member']` can match a group of entries as well as each entry in it; the group
 *  holds entries, so the entries are kept. */
export function innermost(
  root: ParentNode,
  selectors: readonly string[],
  usable: (element: Element) => boolean,
): Element[] {
  for (const selector of selectors) {
    const found = [...root.querySelectorAll(selector)].filter(usable);
    if (found.length === 0) continue;
    const kept = new Set(found);
    for (const element of found) {
      for (let up = element.parentElement; up; up = up.parentElement) kept.delete(up);
    }
    return found.filter((element) => kept.has(element));
  }
  return [];
}
