// Generated from crates/core by `UPDATE_BINDINGS=1 cargo test -p winer-core bindings`. Do not edit.

export type Snapshot = { 
/**
 * The revision of the last patch applied; updates at or below it are stale.
 */
rev: number, connection: Connection, me: Me | null, phase: Phase, champSelect: ChampSelectView | null, game: GameView | null, };

export type Update = { rev: number, patch: Patch, };

export type Patch = { "key": "connection", "value": Connection } | { "key": "me", "value": Me | null } | { "key": "phase", "value": Phase } | { "key": "champSelect", "value": ChampSelectView | null } | { "key": "game", "value": GameView | null };

export type Event = { "type": "update", "data": Update } | { "type": "notice", "data": Notice } | { "type": "settings", "data": Settings } | { "type": "gameData" };

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

export type NoticeKind = { "kind": "accepted" } | { "kind": "declared", championId: number, } | { "kind": "picked", championId: number, locked: boolean, } | { "kind": "banned", championId: number, } | { "kind": "playedAgain" } | { "kind": "swapped", championId: number, } | { "kind": "calledOut", lines: number, } | { "kind": "failed", action: string, message: string, };

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
elevated: boolean, logDir: string, settingsPath: string, };

export type UpdateStatus = { "state": "idle" } | { "state": "checking" } | { "state": "upToDate", version: string, checkedAt: number, } | { "state": "available", version: string, current: string, notes: string | null, date: string | null, } | { "state": "downloading", version: string, received: number, total: number | null, } | { "state": "installing", version: string, } | { "state": "failed", message: string, };

export type IpcError = { code: ErrorCode, message: string, };

export type ErrorCode = "notConnected" | "notFound" | "invalid" | "busy" | "client" | "internal";

export type Settings = { appearance: Appearance, general: General, automation: Automation, plugin: PluginSettings, };

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
titles: boolean, };

export type Language = "zh-CN" | "en";

export type Automation = { accept: AcceptRule, pick: PickRule, ban: BanRule, 
/**
 * Return to the lobby after the post-game screen.
 */
playAgain: boolean, callout: CalloutRule, bench: BenchRule, 
/**
 * The kinds of game each rule acts in; a switched-on rule does nothing elsewhere.
 */
scopes: Scopes, };

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
callout: Array<Mode>, bench: Array<Mode>, playAgain: Array<Mode>, };

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
loaderDir: string | null, };

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

export type PluginMessage = { "type": "hello", version: string, context: string, } | { "type": "log", level: LogLevel, message: string, } | { "type": "benchSwap", championId: number, };

export type LogLevel = "debug" | "info" | "warn" | "error";
