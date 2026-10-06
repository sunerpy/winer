// The client page's elements the plugin's decorations look for.
//
// The friends list and the lobby: NOT MEASURED YET. Every selector of theirs is a guess, gathered in
// this one place for the integrator to check on the live client over the client page's debug port
// (docs/platform-notes.md, "Pengu Loader"), then to record in platform-notes and trim to what the
// client really uses. Each list is tried in order, and where a guess matches a group of entries as
// well as each entry, the entries are kept (`find.ts`). One more guess goes with these: the lobby
// panel's offset from the client's right edge (`style.css`, `[data-winer-panel="lobby"]`), which
// keeps it clear of the friends list.
//
// The home page (`HOME`, `PROMOTION_POPUPS`): measured on the Tencent client, 16.19, 2026-10-06,
// over the client page's debug port.

/** The friends list, the social panel down the client's right edge. A miss draws nothing. */
export const FRIENDS_LIST = {
  /** The roster, the panel that lists the friends. */
  root: [
    "lol-social-roster",
    ".lol-social-roster",
    "[class*='social-roster']",
    "[class*='roster-container']",
  ],
  /** One friend's entry in it. */
  member: [
    "lol-social-roster-member",
    ".lol-social-roster-member",
    "[class*='roster-member']",
    "[class*='member-wrapper']",
  ],
  /** The friend's name inside an entry; the line goes after it. */
  name: [".member-name", "[class*='member-name']", "[class*='summoner-name']", "[class*='name']"],
  /** Attributes the client may write a friend's puuid (or chat id, `<puuid>@…`) into. */
  ids: ["data-puuid", "puuid", "data-pid", "pid", "data-id", "data-summoner-puuid"],
} as const;

/** The party in the lobby. A miss, or cards that name no member, falls back to the panel. */
export const LOBBY = {
  /** One member's card in the party. */
  member: [
    ".lobby-party-member",
    ".party-member",
    "lol-parties-lobby-member",
    "[class*='lobby-member']",
    "[class*='party-member']",
  ],
  /** The member's name on the card. */
  name: [".player-name", ".summoner-name", "[class*='player-name']", "[class*='summoner-name']"],
  /** The banner the line goes above. */
  banner: [".lobby-banner", "lol-regalia-banner-v2-element", "[class*='banner']"],
  /** The avatar, a click on which opens the member's history. Its controls (a crown to promote, an
   *  ✕ to kick) are left to the client. The card's lower half is not taken: the lane pickers sit
   *  there in draft queues. */
  avatar: [
    ".summoner-icon",
    "lol-regalia-crest-v2-element",
    "[class*='summoner-icon']",
    "[class*='avatar']",
  ],
  /** Attributes the client may write a member's puuid into. */
  ids: ["data-puuid", "puuid", "data-summoner-puuid"],
} as const;

/** The Home tab. On the Tencent client (16.19) it is the activity centre: the screen root
 *  `div.screen-root[data-screen-name="rcp-fe-lol-activity-center"]` holds `section#activity-center`,
 *  whose `main.activity-center__contents` (1055×718) is filled by one iframe of Tencent's news and
 *  events hub (`lol.qq.com/client/v3/index.html`), inside `lol-uikit-section-controller` ›
 *  `div.managed-iframe` › `div.managed-iframe-wrapper`, the iframe its only child. The screen root
 *  and the section are the whole tab: hiding either leaves it black, so winer never does. */
export const HOME = {
  /** The activity centre: winer draws only inside it. */
  centre: "section#activity-center",
  /** The hub: the iframe in the centre's contents. Found by where it sits, not by its address: a
   *  hidden background page of the client's under <body> (`lol.qq.com/client/client_lcu_bg.html`)
   *  shares the hub's host and the start of its path. */
  hub: "main.activity-center__contents iframe",
} as const;

/** The esports pop-ups. 16.19 has `iframe#contestPop` (`lol.qq.com/plugin/esports/pop.html`), a child
 *  of <body> at an inline `display: none` until it pops (it was not seen popping; while it waits,
 *  no overlay of its own sits beside it). `iframe#tv-official-pop`, the one the option first hid, is
 *  not on 16.19 and is kept for clients that have it. The page's other iframes (the chat, the
 *  store's config page, a login helper, a presentation page, the background page) are not
 *  promotions and are left alone. */
export const PROMOTION_POPUPS = ["iframe#contestPop", "iframe#tv-official-pop"] as const;
