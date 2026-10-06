// A small element builder: the plugin draws a handful of rows, which does not justify a framework
// inside someone else's page.
type Child = Node | string | number | null | undefined | false;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Record<string, string | undefined> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value !== undefined) element.setAttribute(name, value);
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    element.append(typeof child === "number" ? String(child) : child);
  }
  return element;
}

/** A player's profile icon, round, from the client's own game data; nothing for an unknown icon. */
export function profileIcon(id: number, size: number): HTMLElement | null {
  if (id <= 0) return null;
  return h("img", {
    class: "winer-icon winer-icon--round",
    src: `/lol-game-data/assets/v1/profile-icons/${id}.jpg`,
    width: String(size),
    height: String(size),
    alt: "",
    draggable: "false",
  });
}

/** Same-origin game-data image: the plugin runs inside the client, so no proxy is needed. */
export function championIcon(id: number, size: number): HTMLElement {
  if (id <= 0)
    return h("span", {
      class: "winer-icon winer-icon--empty",
      style: `width:${size}px;height:${size}px`,
    });
  return h("img", {
    class: "winer-icon",
    src: `/lol-game-data/assets/v1/champion-icons/${id}.png`,
    width: String(size),
    height: String(size),
    alt: "",
    draggable: "false",
  });
}
