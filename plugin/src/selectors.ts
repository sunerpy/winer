// The client page's elements the friends list and lobby decorations look for. NOT MEASURED YET:
// every selector here is a guess, gathered in this one place for the integrator to check on the
// live client over the client page's debug port (docs/platform-notes.md, "Pengu Loader"), then to
// record in platform-notes and trim to what the client really uses. Each list is tried in order,
// and where a guess matches a group of entries as well as each entry, the entries are kept
// (`find.ts`). One more guess goes with these: the lobby panel's offset from the client's right
// edge (`style.css`, `[data-winer-panel="lobby"]`), which keeps it clear of the friends list.

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
