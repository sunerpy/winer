// Friends at play, in the client's own friends list (the social panel on the right): after the name
// of a friend in a game, the mode and how long the game has run, ticking; on the entry of each
// friend playing with other friends, a stripe in the colour of their group. Entries are found by the
// friend's puuid where the client writes one into the page, else by name. Nothing of the client's is
// moved, hidden or restyled: one line is added after the name and one attribute on the entry.
import { duration, type FriendView, type FriendsView, type Language } from "@winer/shared";

import { h } from "./dom";
import { first, innermost } from "./find";
import { groupSlot } from "./groups";
import { text } from "./i18n";
import { FRIENDS_LIST } from "./selectors";

const LINE_CLASS = "winer-friend";
/** On an entry: the colour slot (1–6) of the friend's group. */
const GROUP_ATTRIBUTE = "data-winer-friend-group";

const ID_SELECTOR = FRIENDS_LIST.ids.map((id) => `[${id}]`).join(",");

/** What the line says: `极地大乱斗 · 12:34`, the mode alone while the start is unknown. */
export function friendLine(friend: FriendView, now: number, language: Language): string | null {
  const status = friend.status;
  if (status.state !== "inGame") return null;
  const mode = status.mode || text(language, "inGame");
  return status.startedAt > 0 ? `${mode} · ${duration((now - status.startedAt) / 1000)}` : mode;
}

/** One friend's entry in the client's list, and the name inside it when it was found. */
interface Entry {
  element: Element;
  name: Element | null;
}

/** Finds the friend an entry belongs to: by an id the client wrote into it, else by its name. */
function friendOf(
  entry: Entry,
  byPuuid: Map<string, FriendView>,
  byName: Map<string, FriendView>,
): FriendView | null {
  for (const holder of [entry.element, ...entry.element.querySelectorAll(ID_SELECTOR)]) {
    for (const attribute of FRIENDS_LIST.ids) {
      const value = holder.getAttribute(attribute)?.trim().toLowerCase();
      if (!value) continue;
      const friend = byPuuid.get(value) ?? byPuuid.get(value.split("@")[0] ?? "");
      if (friend) return friend;
    }
  }
  const name = entry.name?.textContent?.trim().toLowerCase();
  return name ? (byName.get(name) ?? null) : null;
}

/** Keys a friend by every way the list may write their name: `name#tag` and `name`. */
function names(friends: FriendView[]): Map<string, FriendView> {
  const byName = new Map<string, FriendView>();
  for (const friend of friends) {
    if (!friend.name) continue;
    const name = friend.name.gameName.trim().toLowerCase();
    byName.set(`${name}#${friend.name.tagLine.trim().toLowerCase()}`, friend);
    // Two friends may share a name; the first, the longest at play, keeps it.
    if (!byName.has(name)) byName.set(name, friend);
  }
  return byName;
}

/** An entry that can be told apart: it holds a name, or an id the client wrote. */
function identifiable(element: Element): boolean {
  return (
    first(element, FRIENDS_LIST.name) !== null ||
    element.matches(ID_SELECTOR) ||
    element.querySelector(ID_SELECTOR) !== null
  );
}

/** The entries of the client's list: by the entry selectors, or, where none matches, each element
 *  whose own text is a playing friend's name, with its parent taken as the entry. */
function entriesOf(list: Element, byName: Map<string, FriendView>): Entry[] {
  const listed = innermost(list, FRIENDS_LIST.member, identifiable);
  if (listed.length > 0) {
    return listed.map((element) => ({ element, name: first(element, FRIENDS_LIST.name) }));
  }
  const found: Entry[] = [];
  if (byName.size === 0) return found;
  const walker = list.ownerDocument.createTreeWalker(list, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const name = node.parentElement;
    const element = name?.parentElement;
    if (!name || !element || name.closest(`.${LINE_CLASS}`)) continue;
    if (byName.has(node.textContent?.trim().toLowerCase() ?? "")) found.push({ element, name });
  }
  return found;
}

export interface FriendsDrawn {
  /** Entries found in the client's list. */
  entries: number;
  /** Lines drawn: a game's time is on screen and ticks. */
  lines: number;
}

/** Decorates the client's friends list for `view`; with `view` null takes every decoration off. */
export function decorateFriends(
  root: ParentNode,
  view: FriendsView | null,
  now: number,
  language: Language,
): FriendsDrawn {
  const list = view ? first(root, FRIENDS_LIST.root) : null;
  if (!list || !view) {
    clearFriends(root);
    return { entries: 0, lines: 0 };
  }
  const playing = view.friends.filter(
    (friend) => friend.status.state === "inGame" || friend.group !== null,
  );
  // Nobody to draw: the entries are not even looked for, which is most of the time.
  if (playing.length === 0) {
    clearFriends(list);
    return { entries: 0, lines: 0 };
  }
  const byPuuid = new Map(playing.map((friend) => [friend.puuid.toLowerCase(), friend]));
  const byName = names(playing);
  const entries = entriesOf(list, byName);
  // What this pass drew; anything else of ours in the list is left over from a game now over.
  const kept = new Set<Element>();
  for (const entry of entries) {
    const friend = friendOf(entry, byPuuid, byName);
    if (friend?.group) {
      const slot = String(groupSlot(friend.group));
      if (entry.element.getAttribute(GROUP_ATTRIBUTE) !== slot)
        entry.element.setAttribute(GROUP_ATTRIBUTE, slot);
      kept.add(entry.element);
    }
    const said = friend ? friendLine(friend, now, language) : null;
    if (!said) continue;
    let line = entry.element.querySelector<HTMLElement>(`.${LINE_CLASS}`);
    if (!line) {
      line = h("span", { class: LINE_CLASS });
      if (entry.name) entry.name.after(line);
      else entry.element.append(line);
    }
    // Written only when it changed: every write wakes the observer that drives rendering.
    if (line.textContent !== said) line.textContent = said;
    kept.add(line);
  }
  for (const line of list.querySelectorAll(`.${LINE_CLASS}`)) {
    if (!kept.has(line)) line.remove();
  }
  for (const element of list.querySelectorAll(`[${GROUP_ATTRIBUTE}]`)) {
    if (!kept.has(element)) element.removeAttribute(GROUP_ATTRIBUTE);
  }
  const lines = [...kept].filter((element) => element.classList.contains(LINE_CLASS)).length;
  return { entries: entries.length, lines };
}

export function clearFriends(root: ParentNode): void {
  root.querySelectorAll(`.${LINE_CLASS}`).forEach((line) => line.remove());
  root
    .querySelectorAll(`[${GROUP_ATTRIBUTE}]`)
    .forEach((entry) => entry.removeAttribute(GROUP_ATTRIBUTE));
}
