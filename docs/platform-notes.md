# Platform notes

Measured facts the code depends on. Each one was observed on real hardware; where a fact only holds
for one client build or one privilege level, it says so.

## LCU credentials

| fact                                                                                                                                                                                                                                           | measured on                   |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| The Tencent client writes `LeagueClient\lockfile` but leaves it **0 bytes**. A sibling `lockfile_` is also empty.                                                                                                                              | 16.16 and 16.19, two machines |
| `Riot Client Data\User Data\Config\lockfile` parses as `name:pid:port:token:protocol` but belongs to `Riot Client`; its token gets 401 from the LCU. Check the `name` field.                                                                   | 16.16                         |
| `LeagueClientUx.exe`'s command line carries `--app-port=<n>` and `--remoting-auth-token=<22 chars>`. It is the only source that always works.                                                                                                  | 16.16, 16.19                  |
| The Tencent `LeagueClientUx` runs **elevated** (launched through WeGame). From an elevated caller, `NtQueryInformationProcess(ProcessCommandLineInformation)` with a `PROCESS_QUERY_LIMITED_INFORMATION` handle returns the full command line. | 16.19, elevated               |
| Non-elevated, WMI's `Win32_Process.CommandLine` is null for it and `MainModule` throws, while `QueryFullProcessImageNameW` under `PROCESS_QUERY_LIMITED_INFORMATION` still succeeds.                                                           | 16.16, non-elevated           |
| `--install-directory` in the command line is **GBK mojibake** (`鑻遍泟鑱旂洘` for `英雄联盟`). Derive paths from `QueryFullProcessImageNameW` instead.                                                                                         | 16.19                         |
| 16.16 wrote the launch arguments into `Game\Logs\LeagueClient Logs\<ts>_<clientPid>_<uxPid>_LeagueClientUx.log`; 16.19 writes no Ux log and its `LeagueClient.log` has no token. Do not depend on logs.                                        | 16.16, 16.19                  |
| The LCU listens on the port named by `--app-port`, which belongs to neither process's own netstat listing. Port scanning by process does not work.                                                                                             | 16.16                         |

## LCU behaviour

- TLS: the leaf chains to `riotgames.pem` (Riot Games root, SHA-1 signed root). A name-checked
  rustls handshake against `127.0.0.1` succeeded on 16.16.
- `displayName` is the empty string on Tencent shards. Identity is `gameName#tagLine`; `tagLine` can
  also be empty on some accounts.
- `/lol-chat/v1/me` → `lol` holds string values only (`"level": "30"`).
- Availability (16.19, NJ100): `PUT /lol-chat/v1/me` answers 201 to every value, but only `chat`,
  `away` and `offline` take; the client's own identity then shows 在线, 离开 and 离线. `dnd` is
  ignored (the availability reads back unchanged; the client sets it itself during a game), and
  `mobile` is kept but shown as 在线分组, a group name, not a status. Read the value back to know.
  On GZ100 (16.19.821.7343, 2026-10-06) `online` and `spectating` were kept as well and shown as
  在线 and 正在观战中: whatever the client keeps, it names through the table below.
- zh_CN `champion-summary.json`: `name` is the title the client shows (`黑暗之女`), `description` is the
  short name (`安妮`), `alias` is the English key (`Annie`).
- Ranked: `/lol-ranked/v1/ranked-stats/{puuid}` → `queueMap.RANKED_SOLO_5x5` etc. Unranked is
  `tier: ""`, `division: "NA"`. `NA` means "no division" and is never displayed.
- Match history: `/lol-match-history/v1/products/lol/{puuid}/matches?begIndex=0&endIndex=19` works for
  any public puuid and returns one participant per game. `/lol-match-history/v1/games/{id}` returns all
  ten, also for a game 150 deep. For the **logged-in** player either form can answer
  `400 Error getting match list for summoner` while the other works: on HN10 (16.19) the `{puuid}`
  form failed in the morning and `current-summoner` in the afternoon of the same day. Try both.
- **The list ignores `begIndex` and `endIndex`** (NJ100, 2026-10-05): every range from `0–9` to
  `200–249`, on both path forms, answered with the same cached window, first 5 games and minutes
  later 20. It is one page of the newest games, never more; paging is SGP's (below).
- Sides: a champ-select player's `team` is the side, **1 blue and 2 red**, not "1 own team" (alone
  on a custom lobby's red team, 16.19 NJ100: `team: 2`). `cellId` is not a side: that player's cell
  was 0. During champ select `/lol-gameflow/v1/session` has empty `teamOne` and `teamTwo`; in the
  game the red player is in `teamTwo`.
- `/lol-game-data/assets/v1/champion-summary.json` (16.19, zh_CN) lists 173 champions and, beside
  them, 72 copies a mode uses: alias `Jade_<Champion>`, id 60000 + the champion's id, names equal
  to the champion's except for 13 still under an older title (`Jade_Galio` is 哨兵之殇). They are
  looked up like champions (their games carry those ids) but are not offered as choices.
- Augments (16.19): a Hextech ARAM game (`KIWI`, queue 2400) carries the picks in each
  participant's `playerAugment1`–`playerAugment6` (four used, zero for an empty slot), in
  `/lol-match-history/v1/games/{id}` and on list pages alike. The client's
  `/lol-game-data/assets/v1/cherry-augments.json` names Arena's and Hextech ARAM's augments
  together (554 rows) with `nameTRA`, `augmentSmallIconPath` and `rarity` (`kSilver`, `kGold`,
  `kPrismatic`); its ids are **strings**. It has no descriptions: those come from ARAM.GG's
  `aram-mayhem-augments.{zh_cn,en_us}.json` (247 entries, tooltip markup with `%i:...%` icon
  tokens and `?` where a value was not resolved).
- Champion mastery lives at `/lol-champion-mastery/v1/{puuid}/champion-mastery`; the
  `lol-collections` path 404s.
- Endpoints that 404 on an idle client: `gameflow/v1/session`, `lobby/v2/lobby`,
  `replays/v1/metadata/{id}`. Treat 404 as "absent", not as an error.
- Custom lobby for testing: `POST /lol-lobby/v2/lobby` with `queueId` set to the shard's own Custom
  queue, `isCustom: true` and a `customGameLobby` whose `mutators.id` is the queue's
  `gameTypeConfig.id` (draft 3110/18 and ARAM all-random 3220/21 on HN10, TJ100, NJ100; read
  `/lol-game-queues/v1/queues`, `category == "Custom"`). Then
  `POST /lol-lobby/v1/lobby/custom/start-champ-select`.
- Changing team in a custom lobby: `POST /lol-lobby/v2/lobby/team/{team}` refused every value tried
  (`200`, `two`, `TWO`, `red`, `CHAOS`, `2`, `team2`: 400 `INVALID_REQUEST`) and
  `/lol-lobby/v1/lobby/custom/switch-teams` is gone (404). The lobby's own 加入 button
  (`.custom-game-team-two .member-join-other-team-button`, clicked over the client page's debug
  port) moves the player.
- **There is no reliable way back out of a custom champ select.** On ARAM all-random (3220, 16.19,
  NJ100) `POST /lol-lobby/v1/lobby/custom/cancel-champ-select` answered 204 and changed nothing,
  `DELETE /lol-lobby/v2/lobby` answered 404, and when the timer ran out the game launched; the
  game server then refused `POST /lol-gameflow/v1/early-exit` (463 `REMOVAL_NOT_ALLOWED`) and
  killing the game left the client in `Reconnect`. Use a mode that ends by itself when nobody
  locks in (draft), keep pick automation off or hover-only, and let the timer run out.
- Custom games give **no rerolls**. An all-random custom lobby (3220) still puts the pool's extras
  on the bench for a lone player (two champions on NJ100), so bench swaps can be exercised there.
- The champ-select chat room is the conversation of type `championSelect`; a message posted with
  `type: "celebration"` is stored and shown to this client only.
- A client can lose its party registration: on machine B (2026-10-05, a new account,
  `isNewPlayer: true`) every lobby creation and `POST /lol-lobby/v2/eligibility/self` answered
  `400 CURRENT_PLAYER_PUUID_NOT_FOUND`, `/lol-lobby/v1/parties/player` had an empty registration, and
  the client's own `rcp-fe-lol-parties` logged the same 400 from 162 s after launch. No lobby, so no
  champ select, until the client is restarted; it is not something the helper caused or can fix.
- `POST /riotclient/kill-and-restart-ux` restarts only `LeagueClientUx` and its renderers: the API
  (`--app-port`, served by `LeagueClient`) and an open WAMP socket survive it, the plugin is back
  within seconds, and the window returns **un-minimized** even if it was minimized before.

## Profile and chat presence

Read-only, NJ100, 16.19, 2026-10-06.

- `/lol-champions/v1/inventories/{summonerId}/skins-minimal` lists **2635** skins, owned or not, each
  with `championId`, `isBase`, `ownership.owned`, `disabled`, and `tilePath` / `splashPath` under
  `/lol-game-data/assets/` (so the `lcu` protocol serves them). **515** of them belong to the mode's
  copies of champions (`championId` 60000 and up, skin ids such as 60001000) and repeat the real
  champions' skins name for name. A base skin is named after the champion's title (`九尾妖狐`).
- `/lol-summoner/v1/current-summoner/summoner-profile` → `backgroundSkinId` (0 when none is chosen),
  `backgroundSkinAugments`, and `regalia` as a JSON string.
- `/lol-challenges/v1/challenges/local-player` is a map of **399** challenges by id (about 1 MB); 349
  had `currentLevel: "NONE"`. Each carries `levelToIconPath`, one token picture per level, under
  `/lol-game-data/assets/`. `summary-player-data` names the token slots in
  `selectedChallengesString` (`"101304"` with one token, `"101101,101205,2023005"` with three on
  GZ100), the same tokens in `topChallenges`, the title in `title` (`itemId`, `name`) and the banner
  in `bannerId` (below).
- `/lol-chat/v1/me` → `lol` of an unranked account has no `rankedLeagueQueue`, `rankedLeagueTier` or
  `rankedLeagueDivision` key at all, only `rankedPrevSeasonTier: ""` and
  `rankedPrevSeasonDivision: "NA"`; `challengeTokensSelected` repeats the token string.

### What friends read for a status

GZ100, 16.19.821.7343, 2026-10-06, over the API and the client page (port 9223). Every write was put
back as it was found.

- The text comes from the client's own `lol-social-status` element, the same in the friends list and
  in the player's own identity block: a game state first, else the status message as
  `“{statusMessage}”`, else `availability_<value>` from `/fe/lol-social/trans.json`, plus
  ` - productName` or ` - platformId` only for a friend in another product or on another shard.
- zh_CN's `availability_mobile` is **在线分组**, the same string as the friends list's mobile group
  (`group_label_mobile`, `dropdown_hide_mobile`: 隐藏 在线分组). None of the 53 translation files the
  page loads has 手机在线, so no availability reads as that. The identity shows
  `availability-icon mobile` and `status-message mobile`: 在线分组.
- `PUT {"statusMessage": "手机在线"}` (201) read back at once and was still there 3 s later; with
  the availability still `mobile`, the identity block showed `“手机在线”` in a
  `status-message-quoted` span inside `status-message-wrapper mobile`. The quotation marks are
  characters of the template, not CSS (`::before` and `::after` are `none`). A post-game room's
  participant list, the chat server's copy of the presence, carried the message too.
- `/lol-platform-config/v1/namespaces/LcuSocial` has `StatusesDisabled: true`; the element showed
  the message anyway. Read from the client's code, not seen: the friend hover card prints the
  message in the same marks, the client's own status input is off under that flag, and its idle
  service clears a status message when the interface starts (winer's remembered presence puts it
  back).
- `lol.gameStatus: "mobile"` beside `availability: "mobile"` did not take (read back `outOfGame`).
  `productName`, `product` and `platformId` in the body were ignored (201, unchanged).

### Banner

GZ100, 16.19.821.7343, 2026-10-06.

- `/lol-regalia/v3/inventory/REGALIA_BANNER` maps every banner id (37) to `isOwned`, `purchaseDate`
  and `items` (`assetPath`, `idSecondary`, `isSelectable`, `isTencentOnly`, `localizedName`). `1`
  is the default (`default.png`, no name), `2` the banner of last season's rank, one item per tier
  (`idSecondary` `UNRANKED` … `CHALLENGER`); the rest are events' banners, 580 × 1480 pictures.
- The client's own customizer (`rcp-fe-lol-shared-components`) offers the default (`1`, type
  `blank`), the rank banner (`2`, type `lastSeasonHighestRank`, disabled without a last-season
  rank) and every event's banner above `2`, unowned ones greyed. Saving posts
  `update-player-preferences` with `bannerAccent: "<id>"` and `PUT`s
  `/lol-regalia/v2/current-summoner/regalia` with the type. An empty `bannerId` reads as the default,
  or as the rank banner while the type is `lastSeasonHighestRank`.
- `POST update-player-preferences` with the tokens, the title and `bannerAccent: "24"` answered 204:
  the summary's `bannerId` (also through `summary-player-data/player/{puuid}`) and the chat
  presence's `lol.bannerIdSelected` became `24` at once; tokens, title, regalia and the account
  loadout's `REGALIA_BANNER_SLOT` (item 1) stayed as they were. `bannerAccent: ""` put both back to
  empty. What the profile page and the hover card then draw was not looked at.
- `PUT …/regalia` with `preferredBannerType: "lastSeasonHighestRank"` (201) changed the preference
  and the presence's `regalia` (`bannerType` 1 → 2), while `bannerType` stayed `blank` for an
  account with no last-season rank; `blank` put both back.

## Match history from the shard's server (SGP)

Measured on NJ100, 16.19, 2026-10-05.

- `GET https://nj100-sgp.lol.qq.com:21019/match-history-query/v1/products/lol/player/{puuid}/SUMMARY?startIndex={n}&count={n}`
  with `Authorization: Bearer <accessToken of /entitlements/v1/token>` and the client's own user
  agent (`LeagueOfLegendsClient/16.19.8217343 (rcp-be-lol-match-history)`) answers 200 over a
  publicly trusted certificate. The entitlements `issuer` names the shard
  (`http://nj100-bcs-internal.lol.qq.com:28088`).
- Pages are real: the measured account had 161 entries (`startIndex=150&count=50` returned 11, 200 and
  beyond none). Fifty games are about 6.5 MB and took 0.4–0.75 s; each game carries all ten
  players.
- `tag=q_<queue>` keeps one queue; two tags, repeated or comma-joined, return nothing. Entries are
  also tagged `normal`, `custom` or `tutorial`.
- The newest entry can be a game left before it was recorded: `gameId` 0, no players. Custom and
  tutorial games are marked `private: true` but complete.
- Turned into the LCU's shape (`crates/core/src/sgp.rs`), a game reads exactly as
  `/lol-match-history/v1/games/{id}` does for the analysis (the test
  `a_page_from_the_server_describes_its_game_as_the_client_does`, on both captures of one game).
  So a scoreboard opened from a server page is drawn from that page's game, with no request.

## Kinds of game

Read-only, GZ100, 16.19.821.7343, 2026-10-06 (`fixtures/live/ranked/queues.json`,
`fixtures/live/history/client-list-gz100.json`).

- `/lol-game-queues/v1/queues` lists 141 queues: `category` `PvP` (106), `Custom` (20, each also
  `isCustom: true`) and `VersusAi` (15). Co-op vs AI is `VersusAi`, `type` `BOT`, `ARAM_BOT` or
  `RIOTSCRIPT_BOT`; the ones open now are 870, 880 and 890 (入门级, 新手级, 一般级, on `SWIFTPLAY`).
  Doom Bots (`NIGHTMARE_BOT`, 4210–4260) and Jade's co-op (`JADE_BOT`, 4320) are filed under
  `PvP`: only their `type` says the opponent is the computer.
- A custom game in a match list carries `gameType: CUSTOM_GAME` and the custom queue's own id
  (3220 for an all-random ARAM lobby, 3270 for a Hextech ARAM one), not 0. The tutorial's games
  are `TUTORIAL_GAME` by Riot's documented types; none was seen here.
- The client's own list on GZ100 answered 30 games to `begIndex=0&endIndex=19` (NJ100 had
  answered 20): what it holds, whatever the range. Two of those 30 were custom and three remakes
  (`gameEndedInEarlySurrender`, 130–168 s).

## Public build statistics

Read off the captures in `fixtures/builds/` (Jhin, 202, patch 16.19, 2026-10-06) and probed from the
Linux build host the same day; each answers a plain GET with no key, cookie or user agent.

- Tencent 101 (`mlol.qt.qq.com/go/battle_info/odp_proxy/lol_101strategy_{build,runeinfo,skill,skill_point,confront}`):
  the payload is the one field of `data._fieldValues`, under a name that changes (`R18087`,
  `R18119`), holding a JSON document as a string. A lane or patch with no numbers answers `code: 0`
  with that string empty.
- A `rune_top_details` row's last number is the row's own games, not the lane's: games divided by the
  row's pick share gives the same total for every row of a lane (1 241 522 for Jhin bottom), which
  is the lane's sample. Rows do not name their styles; they follow from the client's
  `perkstyles.json`.
- `skill` holds the summoner spells, with the shares last and the other way round from the item rows:
  `<spell>_<spell>_<win %>_<pick %>`, the picks summing to 99.7. `confront`'s `high_op_details` are
  the opponents the champion beats (Ezreal 54.09 %, which OP.GG's counters give as 54.3 %),
  `low_op_details` those it loses to; what their last column measures is not known.
- Tencent's Hextech ARAM numbers (`fuwen_hero_rank`) come in the same envelope with no patch (a
  `dtstatdate`). Their augment ids are the client's own (`cherry-augments.json`); their items include
  mode copies (`126697`) that the client's item catalog also lists. `itemone_json`, `itemcore_json`
  and `skill_json` are JSON objects keyed by rank, with rates in hundredths of a percent
  (`4874` = 48.74 %).
- OP.GG (`lol-api-champion.op.gg/api/global/champions/<mode>/<id>[/<position>]`): the `global`
  region answers. `ranked/<id>` without a position answers 404, so a lane is always asked for; every
  answer names the champion's lanes in `summary.positions`, whichever was asked. Arena's `total_place`
  counts finishes from 0: Jhin's 417 319 over 117 603 games is 3.55, a 4.55 average finish, beside a
  48.6 % share of top-four finishes.
- ARAM.GG (`aramgg.com/data/champion-augments/<id>.json`): `[[champion, "<document>", patch, date]]`.
  The document's tiers are Tencent's (`source: "tencent"`), 1 to 4 for S to C; its win rates are
  ARAM.GG's own.
- Arena hands out its summoner spells: the client's `summoner-spells.json` lists two for `CHERRY`
  (2201 and 2202), and Arena has no rune page, so the rune and spell memory does not act there.

## Pengu Loader (injected surface)

- Shipped host: Pengu Loader **v1.1.6**, activated by a `version.dll` symlink in the `LeagueClient`
  directory pointing at the loader's `core.dll`; plugins live in `<loader>\plugins\<name>\index.js`.
  winer ships that `core.dll` (`vendor/pengu-loader/`, Authenticode-signed by SignPath Foundation)
  and, when no loader is linked, writes it to `%LOCALAPPDATA%\app.winer.desktop\pengu` and creates
  the link itself. A symbolic link needs an elevated process (or Developer Mode); the client loads
  `version.dll` only when its interface process starts, so a new link waits for
  `/riotclient/kill-and-restart-ux` or the next launch. The loader writes its `config` and an
  encoded `datastore` beside `core.dll`.
- Without elevation the link fails with `os error 1314` (`ERROR_PRIVILEGE_NOT_HELD`), which a
  Chinese Windows words as 客户端没有所需的特权 ("a required privilege is not held by the client"):
  the client there is the calling process, winer, not League. Reported by the owner from a fresh,
  non-elevated Windows install running 0.0.2, where winer had connected to the client without
  elevation. winer now restarts elevated once for it (as for an elevated client); a link that
  exists needs no rights to be used.
- Pengu's own interface lives in `#pengu-root` (in `#lol-uikit-layer-manager`) behind an **open**
  shadow root, mounted on window load. It shows a welcome dialog while `DataStore` key
  `pengu-welcome` is not `false`, otherwise a "Pengu Loader is active!" toast (7 s) on every start,
  and an update notice when a newer Pengu is out. Its toaster's container holds a `<style>` with
  `.sldt-active`; one child per toast. The plugin sets the key at import, before window load, and
  hides that toaster's toasts that name Pengu (`plugin/src/pengu.ts`). Pengu also exposes
  `window.Toast` to plugins through the same toaster.
- Client CEF is `108.4.13+chromium-108.0.5359.125`. Build the plugin for `chrome108`. CSS nesting,
  `oklch()` and `color-mix()` are unsupported; `@layer`, `:has()`, `@container` work.
- `init(context)` receives `{ rcp, socket }` only. `window.DataStore` has synchronous
  `get/set/has/remove` and no `flush()`. `window.__native` is deleted before plugins load.
- The plugin runs in the page's main JS realm and is served from a plugin origin, so a sibling file
  is readable with `fetch(new URL("bootstrap.json", import.meta.url))`.
- `ws://127.0.0.1:<port>` and authenticated loopback HTTP both work under the client's real CSP.
- Every UI reload creates **two** JS contexts and one of them may never reach `load()`. A mount must
  tolerate a second instance of itself and never delete another context's element blindly.
- ARAM bench (16.19, NJ100): items are `.bench-container .champion-bench-item`; each shows its
  champion as `champion-icons/<id>.png` in an `<img>` or a background. A champion on the bench
  cooldown carries a class like `on-cooldown3` **on the item itself**, plus a `.cooldown-mask`
  inside it; champions already on the bench when champ select opens start on cooldown too.
- **Pengu's `#pengu-root` covers the bench** in champ select: `elementFromPoint` at a bench
  item's centre returns that empty div, so a click's target is `#pengu-root`, not the item. The
  item is still in `elementsFromPoint` at the same point; that is how the plugin finds it.
- **A champ select row clips its text** (16.19, custom draft, 2026-10-06): each
  `.summoner-wrapper.visible.left` holds a 78px `.summoner-object` row whose `.player-details`
  column (156px wide, `overflow: hidden`, 50px) stacks the status (`正在选用……`, 14px), the
  position (20px) and `.summoner-name`, one 16px line (`overflow: hidden`, ellipsis) around
  `.name-text` › `.player-name-wrapper`. A line put after the name stays inside that box and never
  shows; at the end of the column, with the column's overflow let go, it shows in the 14px the row
  leaves below it. The column's left edge moves with the row's state, 87–127px from the window's.
- **The Home tab is the activity centre** (16.19, 2026-10-06):
  `div.screen-root[data-screen-name="rcp-fe-lol-activity-center"]` › `section#activity-center` ›
  `main.activity-center__contents` (1055×718), filled by one iframe of Tencent's news and events hub
  (`lol.qq.com/client/v3/index.html`) in `lol-uikit-section-controller` › `div.managed-iframe` ›
  `div.managed-iframe-wrapper`, the iframe its only child; `div.persistent-control-panel` beside
  `main` holds a mute button. Hiding the screen root or the section leaves the whole tab black. A
  hidden iframe under `<body>`, `lol.qq.com/client/client_lcu_bg.html`, shares the hub's host and
  the start of its path, so the hub is found by where it sits. The esports pop-up is
  `iframe#contestPop` (`lol.qq.com/plugin/esports/pop.html`), a child of `<body>` at an inline
  `display: none` until it pops (not seen popping); there is no `iframe#tv-official-pop`. The page's
  `sessionStorage` is usable.
- The page is `visibilityState: hidden` while the client window is minimized or hidden, and then
  `requestAnimationFrame` never fires: anything the plugin draws waits until the window is shown.
- `POST /lol-champ-select/v1/session/bench/swap/{id}` answered OK for a champion that was on the
  client's cooldown, sent through the bridge from a page-level mouse click in the client.
- For QA only, Pengu's `config` takes `RemoteDebuggingPort=<n>`: after a UI reload the client
  page is reachable over CDP (`scripts/windows/cdp.mjs` with `WINER_CDP_PORT` and
  `WINER_CDP_MATCH=/index.html`). Put it back to `0` and reload again afterwards: while it is
  open, any local process can drive the logged-in client.

## What winer keeps

Read-only, on the owner's PC (winer 0.0.2, two days after it was installed), 2026-10-06.

- `%LOCALAPPDATA%\app.winer.desktop` held `EBWebView` (78,975,184 bytes in 361 files), `logs`
  (15,204 bytes, two days at the default filter) and `pengu` (508,204 bytes: `core.dll` 455,864, the
  plugin 52,228); `%APPDATA%\app.winer.desktop\settings.json` was 1,959 bytes. There was no
  `game-settings` folder and no updater folder in `%TEMP%`.
- In `EBWebView` the page cache (`Default\Cache`) was **empty**: the window's pages and pictures come
  through winer's own protocols (WebView2's `WebResourceRequested`), which it does not store on disk,
  and scripts served that way are left out of the code cache unless `msWebView2CodeCache` is on. The
  caches Chromium rebuilds came to 9,927,920 bytes: `GrShaderCache` 5.2 MB, `Default\GPUCache`
  1.6 MB, `Default\Code Cache` 0.9 MB, and `ShaderCache`, `GraphiteDawnCache`,
  `Default\DawnGraphiteCache` and `Default\DawnWebGPUCache` at 557,424 bytes each.
- The rest is what WebView2 downloads and updates itself: `component_crx_cache` 23.3 MB,
  `WidevineCdm` 22.7 MB, `Subresource Filter` 12.5 MB, `Speech Recognition` 2.7 MB, `hyphen-data`,
  `ZxcvbnData` and a dozen smaller ones. Microsoft's list of WebView2 browser flags has none that stops
  it. `--disk-cache-size` is on that list and bounds the page cache; its effect on this folder was not
  measured, since the page cache was empty anyway.
- `tauri-plugin-updater` writes each installer to
  `%TEMP%\winer-<version>-updater-<random>\winer-<version>-installer.exe`, starts it and ends winer
  with `std::process::exit`, so the file is never removed (read from its 2.10.1 source; no update had
  run in-app on that PC).
- The pictures the window holds in memory, as the client serves them (16.19, GET only): a champion
  icon 28,251 bytes, a profile icon 4,333, a skin's tile 39,905, its centred splash 105,313 and its
  loading-screen art 58,168. The background picker lists 2,635 skins.

## Windows host behaviour

- **GitHub through the system proxy.** On the QA host (in China) a direct connection to
  `github.com` is reset (`os error 10054`); PowerShell gets through because it uses the proxy set in
  Windows (`ProxyEnable`/`ProxyServer`, a local `127.0.0.1:7897`). A GUI app never sees
  `HTTPS_PROXY`, and reqwest reads those registry settings only with its `system-proxy` feature, which
  the workspace turns on for the updater: its background check failed without it and answered
  `upToDate` with it (2026-10-05). The LCU client and the game-data client (`crates/core/src/net.rs`)
  call `no_proxy()` and stay direct.
- `PrintWindow` captures the League client's window as solid black, and `CopyFromScreen` in the idle
  console session failed with "the handle is invalid" (2026-10-05); check the client page over CDP
  instead.
- `std::fs::rename` replaces an open destination; `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` returned
  `ERROR_ACCESS_DENIED` against an open destination. Write temp + `sync_all` + `rename`.
- Processes started over SSH run in session 0, where WebView2 cannot composite and session 1's windows
  are invisible. Launch GUI runs through a scheduled task with `/RU <console user> /IT`. The same
  holds for reading windows: `EnumWindows` from the SSH session lists none of session 1's.
- WebView2's DevTools port for QA: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, set by the scheduled
  task's cmd wrapper, never showed up on `msedgewebview2.exe`'s command line. The `qa` cargo feature
  passes `--remote-debugging-port` through `additional_browser_args` instead. Every build passes its
  `--disk-cache-size` the same way (`window.rs`), and that **replaces** wry's own
  `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`, so they are restated.
- `PrintWindow(PW_RENDERFULLCONTENT)` captures a WebView2 window fully, but returned an all-black
  image of the client's CEF window after its GPU process had restarted with `--use-gl=disabled`.
- `decorations: false` keeps Aero Snap, resize borders, shadow and rounded corners; it loses only the
  Windows 11 Snap Layouts flyout (see `accepted-tradeoffs.md`).
