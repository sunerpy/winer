// Generated from crates/core by `UPDATE_BINDINGS=1 cargo test -p winer-core bindings`. Do not edit.

export type Snapshot = { 
/**
 * The revision of the last patch applied; updates at or below it are stale.
 */
rev: number, connection: Connection, me: Me | null, phase: Phase, champSelect: ChampSelectView | null, game: GameView | null, 
/**
 * `None` until the client has listed the friends once.
 */
friends: FriendsView | null, 
/**
 * The party, while the client shows the lobby (in it, in queue, match found).
 */
lobby: LobbyView | null, };

export type Update = { rev: number, patch: Patch, };

export type Patch = { "key": "connection", "value": Connection } | { "key": "me", "value": Me | null } | { "key": "phase", "value": Phase } | { "key": "champSelect", "value": ChampSelectView | null } | { "key": "game", "value": GameView | null } | { "key": "friends", "value": FriendsView | null } | { "key": "lobby", "value": LobbyView | null };

export type Event = { "type": "update", "data": Update } | { "type": "notice", "data": Notice } | { "type": "settings", "data": Settings } | { "type": "gameData" } | { "type": "openHistory", "data": { puuid: string, } };

export type Connection = { "status": "searching" } | { "status": "accessDenied" } | { "status": "connecting", port: number, } | { "status": "connected", port: number, platformId: string, };

export type Phase = "None" | "Lobby" | "Matchmaking" | "CheckedIntoTournament" | "ReadyCheck" | "ChampSelect" | "GameStart" | "FailedToLaunch" | "InProgress" | "Reconnect" | "WaitingForStats" | "PreEndOfGame" | "EndOfGame" | "TerminatedInError" | "Unknown";

export type RiotId = { gameName: string, tagLine: string, };

export type Me = { puuid: string, name: RiotId | null, level: number, iconId: number, ranked: Ranked, };

export type Tier = "IRON" | "BRONZE" | "SILVER" | "GOLD" | "PLATINUM" | "EMERALD" | "DIAMOND" | "MASTER" | "GRANDMASTER" | "CHALLENGER";

export type Rank = { tier: Tier, 
/**
 * `I`–`IV`; absent from Master upwards.
 */
division: string | null, lp: number, wins: number, losses: number, };

export type Ranked = { solo: Rank | null, flex: Rank | null, };

export type Position = "top" | "jungle" | "middle" | "bottom" | "utility";

export type ChampSelectView = { gameId: number, queueId: number, timer: TimerView, myTeam: Array<Seat>, theirTeam: Array<Seat>, myBans: Array<number>, theirBans: Array<number>, 
/**
 * ARAM's shared bench, and whether this mode has one.
 */
benchEnabled: boolean, bench: Array<number>, rerollsRemaining: number, 
/**
 * The callout as it would be sent now, one chat line per rated teammate.
 */
callout: Array<string>, 
/**
 * The local team's side; `None` where the mode has none (Arena, Swarm).
 */
side: Side | null, };

export type TimerView = { phase: string, 
/**
 * Epoch milliseconds at which the phase ends; zero when it never does.
 */
endsAt: number, totalMs: number, };

export type GameView = { gameId: number, queueId: number, 
/**
 * In the client's order: on a map of two sides, blue first.
 */
teams: Array<Array<Seat>>, 
/**
 * The teams are the blue and the red side; Arena's and Swarm's are not.
 */
sides: boolean, };

export type Seat = { 
/**
 * Absent for hidden players (enemies before load, anonymised ranked lobbies) and bots.
 */
puuid: string | null, name: RiotId | null, championId: number, 
/**
 * The champion shown is a declared intent, not a pick.
 */
intent: boolean, position: Position | null, spells: [number, number], isSelf: boolean, 
/**
 * Players sharing a number came as one premade party.
 */
premade: number | null, stats: PlayerStats, 
/**
 * Recent form and the tier it earns within the team; absent until stats arrive.
 */
rating: SeatRating | null, };

export type SeatRating = { 
/**
 * 0–10, see `rating::form_score`.
 */
score: number, 
/**
 * 0 is the best of `tiers` tiers.
 */
tier: number, tiers: number, 
/**
 * The tier's name in the user's words (`峡谷公务员` by default).
 */
label: string, 
/**
 * The grade, 0 (S+) to 7 (F), when the scheme grades on fixed bands instead of ranking the
 * team (峡谷八档); it is then the same number as `tier`.
 */
grade: number | null, 
/**
 * What recent games say beyond the tier (`版本答案`), when titles are on and one applies.
 */
title: string | null, 
/**
 * A line about the tier for the chat and the tooltip (`稳得离谱，能C还能活`).
 */
quip: string | null, };

export type PlayerStats = { "state": "loading" } | { "state": "hidden" } | { "state": "failed", message: string, } | { "state": "ready" } & PlayerSummary;

export type PlayerSummary = { puuid: string, name: RiotId | null, level: number, iconId: number, 
/**
 * The profile is private: rank and history are not shown to others.
 */
private: boolean, ranked: Ranked, recent: RecentForm, };

export type RecentForm = { 
/**
 * Counted games: remakes are left out of every figure below.
 */
games: number, wins: number, kills: number, deaths: number, assists: number, 
/**
 * `+n` for n wins in a row, `-n` for n losses.
 */
streak: number, 
/**
 * Newest first.
 */
matches: Array<RecentMatch>, 
/**
 * Most played first.
 */
champions: Array<ChampionForm>, };

export type RecentMatch = { gameId: number, queueId: number, championId: number, win: boolean, remake: boolean, kills: number, deaths: number, assists: number, 
/**
 * Epoch milliseconds.
 */
startedAt: number, };

export type ChampionForm = { championId: number, games: number, wins: number, };

export type MatchPage = { puuid: string, begin: number, games: Array<MatchSummary>, 
/**
 * A full page came back, so there may be more.
 */
hasMore: boolean, source: HistorySource, };

export type HistorySource = "server" | "client";

export type Side = "blue" | "red";

export type MatchSummary = { gameId: number, queueId: number, gameMode: string, startedAt: number, 
/**
 * Seconds.
 */
duration: number, line: PlayerLine, };

export type MatchDetail = { gameId: number, queueId: number, gameMode: string, gameVersion: string, startedAt: number, duration: number, teams: Array<TeamDetail>, };

export type TeamDetail = { teamId: number, win: boolean, bans: Array<number>, kills: number, gold: number, towers: number, dragons: number, barons: number, players: Array<PlayerLine>, };

export type PlayerLine = { puuid: string, name: RiotId | null, iconId: number, championId: number, championLevel: number, position: Position | null, spells: [number, number], items: [number, number, number, number, number, number, number], 
/**
 * Arena and Hextech ARAM augments, in the order picked.
 */
augments: Array<number>, keystone: number, subStyle: number, kills: number, deaths: number, assists: number, cs: number, gold: number, damage: number, damageTaken: number, vision: number, largestMultiKill: number, win: boolean, remake: boolean, 
/**
 * Arena placement, 1–8.
 */
placement: number | null, 
/**
 * Share of the team's damage to champions, 0–1. Only a full scoreboard knows the team.
 */
damageShare: number | null, 
/**
 * Kills plus assists over the team's kills, 0–1.
 */
killParticipation: number | null, 
/**
 * 0–10, see `rating::game_scores`; absent for remakes and single-player pages.
 */
score: number | null, 
/**
 * The score's grade on `rating::GAME_GRADES`, 0 (S+) to 7 (F).
 */
grade: number | null, award: Award | null, 
/**
 * What the line did that a badge names, most telling first (see [`Feat`]).
 */
feats: Array<Feat>, };

export type Award = "mvp" | "svp";

export type Feat = "afk" | "penta" | "quadra" | "legendary" | "triple" | "mostKills" | "mostDamage" | "firstBlood" | "mostTowers" | "mostAssists" | "mostGold" | "mostTaken" | "mostCs" | "double";

export type GameData = { champions: Array<ChampionInfo>, items: Array<AssetInfo>, spells: Array<AssetInfo>, 
/**
 * Runes and rune styles together; their ids never collide.
 */
perks: Array<AssetInfo>, 
/**
 * Arena and Hextech ARAM augments.
 */
augments: Array<AugmentInfo>, queues: Array<QueueInfo>, };

export type ChampionInfo = { id: number, 
/**
 * As the client shows it (`黑暗之女` in zh_CN).
 */
name: string, 
/**
 * The short form (`安妮`); the same as `name` where the locale has none.
 */
shortName: string, alias: string, icon: string, };

export type AssetInfo = { id: number, name: string, 
/**
 * An LCU asset path, served to the window through the `lcu` protocol.
 */
icon: string, };

export type AugmentInfo = { id: number, name: string, icon: string, rarity: Rarity | null, };

export type Rarity = "silver" | "gold" | "prismatic";

export type AugmentDetail = { id: number, description: string, };

export type QueueInfo = { id: number, name: string, gameMode: string, ranked: boolean, };

export type Notice = { 
/**
 * Epoch milliseconds.
 */
at: number, kind: NoticeKind, };

export type NoticeKind = { "kind": "accepted" } | { "kind": "declared", championId: number, } | { "kind": "picked", championId: number, locked: boolean, } | { "kind": "banned", championId: number, } | { "kind": "playedAgain" } | { "kind": "swapped", championId: number, } | { "kind": "calledOut", lines: number, } | { "kind": "presenceRestored", availability: string, } | { "kind": "presenceRefused" } | { "kind": "failed", action: string, message: string, } | { "kind": "loadoutApplied", championId: number, 
/**
 * The client's own recommendation: nothing was remembered for the champion.
 */
recommended: boolean, 
/**
 * What became of the rune page; absent where there were no runes to set up.
 */
runes: PageOutcome | null, 
/**
 * The two summoner spells are the ones set up.
 */
spells: boolean, } | { "kind": "itemSetWritten", championId: number, };

export type PlayerProfile = { puuid: string, name: RiotId | null, level: number, iconId: number, private: boolean, ranked: Ranked, };

export type Presence = { 
/**
 * `chat`, `away`, `dnd`, `mobile` or `offline`.
 */
availability: string, statusMessage: string, };

export type AppInfo = { version: string, 
/**
 * The process runs elevated; required to read an elevated client's credentials.
 */
elevated: boolean, logDir: string, settingsPath: string, 
/**
 * The licences of the third-party components shipped inside winer (`THIRD_PARTY_NOTICES.md`).
 */
notices: string, };

export type UpdateStatus = { "state": "idle" } | { "state": "checking" } | { "state": "upToDate", version: string, checkedAt: number, } | { "state": "available", version: string, current: string, notes: string | null, date: string | null, } | { "state": "downloading", version: string, received: number, total: number | null, } | { "state": "installing", version: string, } | { "state": "failed", message: string, };

export type IpcError = { code: ErrorCode, message: string, };

export type ErrorCode = "notConnected" | "notFound" | "invalid" | "busy" | "client" | "internal";

export type FriendsView = { 
/**
 * In game first (the longest-running game first), then champ select, in queue, the rest.
 * Offline friends are left out: nothing shows them, and a long list would ride along with
 * every patch.
 */
friends: Array<FriendView>, };

export type FriendView = { puuid: string, name: RiotId | null, iconId: number, 
/**
 * `chat`, `away`, `dnd` or `mobile`, as the client shows it beside the name.
 */
availability: string, status: FriendStatus, 
/**
 * Friends in one game, or one party, share a number from 1, which picks the colour they are
 * drawn in; a friend playing without other friends has none.
 */
group: number | null, };

export type FriendStatus = { "state": "outOfGame" } | { "state": "inQueue", mode: string, queueId: number, since: number, } | { "state": "champSelect", mode: string, queueId: number, since: number, } | { "state": "inGame", mode: string, queueId: number, 
/**
 * When the game started, epoch milliseconds; zero when the presence does not say.
 */
startedAt: number, 
/**
 * The game can be spectated.
 */
observable: boolean, };

export type LobbyView = { queueId: number, 
/**
 * A custom game's lobby, where everyone in it plays, on both teams.
 */
custom: boolean, 
/**
 * In the lobby's order, the local player among them; bots are left out.
 */
members: Array<LobbyMember>, };

export type LobbyMember = { puuid: string, name: RiotId | null, iconId: number, isSelf: boolean, leader: boolean, 
/**
 * The lanes asked for, first choice first; empty in queues without positions.
 */
positions: Array<LanePreference>, stats: PlayerStats, 
/**
 * Recent form, 0–10 (`rating::form_score`), once the stats are in.
 */
score: number | null, };

export type LanePreference = "top" | "jungle" | "middle" | "bottom" | "utility" | "fill";

export type HotkeyStatus = { 
/**
 * The combination the settings name (`Alt+Backquote`); `None` while the shortcut is off.
 */
shortcut: string | null, 
/**
 * The system has it registered for winer right now.
 */
active: boolean, 
/**
 * Let go while the settings record a new combination.
 */
suspended: boolean, 
/**
 * Why the system refused it, in its own words; usually another program holds the combination.
 */
error: string | null, };

export type Settings = { appearance: Appearance, general: General, automation: Automation, plugin: PluginSettings, profile: ProfileSettings, 
/**
 * The build panel and where its numbers come from (`builds`).
 */
builds: BuildSettings, };

export type Appearance = { theme: Theme, accent: Accent, density: Density, 
/**
 * Base font size in px, 12–16.
 */
fontSize: number, reduceMotion: boolean, };

export type Theme = "system" | "light" | "dark" | "graphite" | "hextech";

export type Accent = "default" | "gold" | "blue" | "teal" | "green" | "orange" | "pink" | "purple";

export type Density = "comfortable" | "compact";

export type General = { 
/**
 * Closing the window keeps winer in the tray.
 */
closeToTray: boolean, language: Language, 
/**
 * Fetch what Hextech ARAM's augments do from ARAM.GG; without it they show name and icon only.
 */
augmentDetails: boolean, 
/**
 * The roast titles beside a grade (`rating::FormTitle` and the scoreboard's own).
 */
titles: boolean, 
/**
 * The global shortcut that shows and hides the window, in [`normalize_hotkey`]'s form; `None`
 * turns it off. A file without the field gets the default; `null` keeps it off.
 */
hotkey: string | null, };

export type Language = "zh-CN" | "en";

export type Automation = { accept: AcceptRule, pick: PickRule, ban: BanRule, 
/**
 * Return to the lobby after the post-game screen.
 */
playAgain: boolean, callout: CalloutRule, bench: BenchRule, 
/**
 * The kinds of game each rule acts in; a switched-on rule does nothing elsewhere.
 */
scopes: Scopes, 
/**
 * Runes and summoner spells, remembered per champion and mode and set up again (`loadout`).
 */
loadout: LoadoutRule, 
/**
 * Experimental: write winer's item set for a champion once it is locked in (`loadout`).
 */
itemSets: boolean, };

export type AcceptRule = { enabled: boolean, 
/**
 * Wait this long before accepting, so a match found by accident can still be declined.
 */
delayMs: number, };

export type PickRule = { enabled: boolean, 
/**
 * Lock the pick in; otherwise only hover it and leave the lock to the user.
 */
lockIn: boolean, 
/**
 * Show the first choice as an intent during the planning phase.
 */
declareIntent: boolean, champions: ChampionPool, };

export type BanRule = { enabled: boolean, champions: ChampionPool, };

export type CalloutRule = { 
/**
 * Send once per champ select, as soon as every teammate's stats are in.
 */
auto: boolean, audience: Audience, includeSelf: boolean, 
/**
 * Sent before the players' lines, as written; empty sends none.
 */
header: string, 
/**
 * One line per player, with `{standing}`, `{seat}` (the place in champ select's list: `1L`,
 * `P1`), `{name}`, `{champion}`, `{games}`, `{winRate}`, `{kda}`, `{score}`, `{title}` and
 * `{quip}`. Empty means the language's default (`callout::template`).
 */
template: string, 
/**
 * How the team is split, and what the tiers are called.
 */
tiers: TierSet, 
/**
 * The user's own tier names, best first, for `TierSet::Custom`: two to five, blanks skipped.
 */
customTiers: Array<string>, };

export type Audience = "team" | "me";

export type TierSet = "riftFive" | "grades" | "horseUniverse" | "horses" | "horsesFive" | "rift" | "custom";

export type BenchRule = { enabled: boolean, 
/**
 * Best first.
 */
champions: Array<number>, };

export type Scopes = { accept: Array<Mode>, pick: Array<Mode>, ban: Array<Mode>, 
/**
 * Where the callout goes out by itself; sending it by hand works everywhere.
 */
callout: Array<Mode>, bench: Array<Mode>, playAgain: Array<Mode>, loadout: Array<Mode>, itemSets: Array<Mode>, };

export type Mode = "ranked" | "normal" | "aram" | "hextech" | "arena" | "other";

export type ChampionPool = { any: Array<number>, top: Array<number>, jungle: Array<number>, middle: Array<number>, bottom: Array<number>, utility: Array<number>, };

export type PluginSettings = { 
/**
 * Whenever a client connects: link the loader winer ships unless one is already linked,
 * install the plugin, and keep both current. Off, winer only keeps an installed plugin current.
 */
auto: boolean, 
/**
 * The teammate panel in champ select.
 */
teamPanel: boolean, 
/**
 * Hide the activity centre and esports pop-ups on the client home page.
 */
hidePromotions: boolean, 
/**
 * In the client's own champ select, a click on an ARAM bench champion swaps at once: the
 * plugin lifts the cooldown and winer carries the swap out.
 */
benchNoCooldown: boolean, 
/**
 * Pengu Loader's directory, when it cannot be found from the client.
 */
loaderDir: string | null, 
/**
 * In the client's friends list: the mode and running time of a friend's game, and one colour
 * for the friends playing together.
 */
friendStatus: boolean, 
/**
 * In the client's lobby: each member's recent form above their banner, and a click that opens
 * their history in winer.
 */
lobbyPanel: boolean, };

export type ProfileSettings = { rankDisguise: RankDisguise, presence: PresenceRule, };

export type RankDisguise = { enabled: boolean, queue: DisguiseQueue, tier: Tier, 
/**
 * Not shown from Master up, which have no divisions.
 */
division: Division, };

export type DisguiseQueue = "solo" | "flex";

export type Division = "I" | "II" | "III" | "IV";

export type PresenceRule = { remember: boolean, 
/**
 * `chat`, `away`, `mobile` or `offline`: the states the client takes from winer.
 */
availability: string, 
/**
 * Put back as well when set; `None` leaves the client's own.
 */
statusMessage: string | null, };

export type SkinChoice = { id: number, championId: number, 
/**
 * As the client names it; a base skin carries the champion's title (`九尾妖狐`).
 */
name: string, owned: boolean, base: boolean, 
/**
 * LCU asset paths, served to the window through the `lcu` protocol.
 */
tile: string, splash: string, };

export type ChallengeProfile = { 
/**
 * The tokens in the profile's slots, left to right: three at most.
 */
tokens: Array<ChallengeToken>, title: TitleChoice | null, 
/**
 * Every challenge with a level, the highest first: what a slot can hold.
 */
challenges: Array<ChallengeToken>, titles: Array<TitleChoice>, };

export type ChallengeToken = { id: number, name: string, description: string, 
/**
 * `None` for a token the client describes without a level.
 */
level: Tier | null, 
/**
 * The token at its level, an LCU asset path; empty when the client names none.
 */
icon: string, };

export type TitleChoice = { 
/**
 * The title's `itemId`, which is what the client takes.
 */
id: number, name: string, };

export type BackupInfo = { 
/**
 * The name of its file, and the order it was made in.
 */
id: number, 
/**
 * Epoch milliseconds when the settings were read from the client.
 */
takenAt: number, 
/**
 * Bytes on disk.
 */
size: number, channels: Array<BackupChannel>, };

export type BackupChannel = "general" | "hotkeys";

export type PluginStatus = { loaderDir: string | null, 
/**
 * The loader is linked into the connected client, so plugins load with it.
 */
active: boolean, 
/**
 * The loader is the one winer ships, kept in winer's own data folder.
 */
managed: boolean, 
/**
 * The version of the loader winer ships.
 */
bundledLoader: string, 
/**
 * The client's `version.dll` is a file of something else, so winer links no loader there.
 */
occupied: boolean, 
/**
 * Why the last automatic setup did not finish, as the system put it.
 */
setupError: string | null, 
/**
 * That setup failed because only an administrator can link the loader into the client, and
 * winer runs without those rights: restarting it elevated once creates the link.
 */
needsElevation: boolean, installedVersion: string | null, bundledVersion: string, 
/**
 * The installed plugin is byte-for-byte this build's bundle. Two builds can share a version.
 */
current: boolean, 
/**
 * Plugin contexts connected to the bridge right now.
 */
connected: number, };

export type BridgeMessage = { "type": "hello", version: string, snapshot: Snapshot, settings: Settings, } | { "type": "event", event: Event, };

export type PluginMessage = { "type": "hello", version: string, context: string, } | { "type": "log", level: LogLevel, message: string, } | { "type": "benchSwap", championId: number, } | { "type": "openHistory", puuid: string, };

export type LogLevel = "debug" | "info" | "warn" | "error";

export type LoadoutRule = { enabled: boolean, 
/**
 * With nothing remembered for the champion, use the client's own recommended page.
 */
recommended: boolean, };

export type BuildSettings = { 
/**
 * Off, the panel is hidden and nothing is fetched.
 */
enabled: boolean, 
/**
 * Where Summoner's Rift numbers come from.
 */
riftSource: RiftSource, };

export type RiftSource = "tencent" | "opGg";

export type LoadoutSummary = { remembered: number, };

export type PageOutcome = "written" | "noPage";

export type BuildSource = "tencent" | "tencentHextech" | "opGg" | "aramGg";

export type Build = { source: BuildSource, 
/**
 * The patch the numbers are from, `16.19`; empty where the source does not say.
 */
patch: string, championId: number, mode: Mode, 
/**
 * The lane the numbers are for, on the Rift.
 */
lane: Position | null, 
/**
 * The champion's standing on the source's own scale, 1 the best (OP.GG: 1 to 5).
 */
tier: number | null, 
/**
 * The games behind the numbers.
 */
sample: number | null, spells: Array<SpellOption>, runes: Array<RuneOption>, starting: Array<ItemOption>, boots: Array<ItemOption>, core: Array<ItemOption>, 
/**
 * Single items for the slots after the core, most taken first.
 */
late: Array<ItemOption>, skillOrders: Array<SkillOrder>, matchups: Matchups, 
/**
 * Best first.
 */
augments: Array<AugmentOption>, 
/**
 * Ids the client cannot name (an item of another patch, an unknown augment, a rune of no
 * style), left out.
 */
dropped: number, };

export type Rates = { 
/**
 * The share of the champion's games that took it, 0–1.
 */
pick: number | null, 
/**
 * The share of those games won, 0–1; in Arena, finished in the top four.
 */
win: number | null, 
/**
 * The games behind the two.
 */
games: number | null, 
/**
 * Arena: the average finish, 1 (first) to 8.
 */
placement: number | null, 
/**
 * Arena: the share of games finished first, 0–1.
 */
first: number | null, };

export type SpellOption = { spells: [number, number], rates: Rates, };

export type RunePage = { primaryStyle: number, subStyle: number, perks: Array<number>, };

export type RuneOption = { page: RunePage, rates: Rates, };

export type ItemOption = { 
/**
 * One item, or several taken together; a repeated id is bought more than once.
 */
items: Array<number>, rates: Rates, };

export type Ability = "Q" | "W" | "E" | "R";

export type SkillOrder = { 
/**
 * The basic abilities, maxed first to last.
 */
priority: Array<Ability>, 
/**
 * The ability taken at each level, from the first.
 */
sequence: Array<Ability>, rates: Rates, };

export type Matchup = { championId: number, rates: Rates, };

export type Matchups = { 
/**
 * The opponents the champion beats most often, best first.
 */
good: Array<Matchup>, 
/**
 * The ones it loses to most often, worst first.
 */
bad: Array<Matchup>, };

export type AugmentTier = "S" | "A" | "B" | "C";

export type AugmentOption = { id: number, 
/**
 * From the client's own catalog.
 */
rarity: Rarity, tier: AugmentTier | null, rates: Rates, };
