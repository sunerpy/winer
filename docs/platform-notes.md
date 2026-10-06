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
  `selectedChallengesString` (`"101304"` with one token; how several are joined was not seen), the
  same tokens in `topChallenges`, and the title in `title` (`itemId`, `name`). It offers no list of
  banner accents to choose from (`bannerId: ""`).
- `/lol-chat/v1/me` → `lol` of an unranked account has no `rankedLeagueQueue`, `rankedLeagueTier` or
  `rankedLeagueDivision` key at all, only `rankedPrevSeasonTier: ""` and
  `rankedPrevSeasonDivision: "NA"`; `challengeTokensSelected` repeats the token string.

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

## Pengu Loader (injected surface)

- Shipped host: Pengu Loader **v1.1.6**, activated by a `version.dll` symlink in the `LeagueClient`
  directory pointing at the loader's `core.dll`; plugins live in `<loader>\plugins\<name>\index.js`.
  winer ships that `core.dll` (`vendor/pengu-loader/`, Authenticode-signed by SignPath Foundation)
  and, when no loader is linked, writes it to `%LOCALAPPDATA%\app.winer.desktop\pengu` and creates
  the link itself. A symbolic link needs an elevated process (or Developer Mode); the client loads
  `version.dll` only when its interface process starts, so a new link waits for
  `/riotclient/kill-and-restart-ux` or the next launch. The loader writes its `config` and an
  encoded `datastore` beside `core.dll`.
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
- The page is `visibilityState: hidden` while the client window is minimized or hidden, and then
  `requestAnimationFrame` never fires: anything the plugin draws waits until the window is shown.
- `POST /lol-champ-select/v1/session/bench/swap/{id}` answered OK for a champion that was on the
  client's cooldown, sent through the bridge from a page-level mouse click in the client.
- For QA only, Pengu's `config` takes `RemoteDebuggingPort=<n>`: after a UI reload the client
  page is reachable over CDP (`scripts/windows/cdp.mjs` with `WINER_CDP_PORT` and
  `WINER_CDP_MATCH=/index.html`). Put it back to `0` and reload again afterwards: while it is
  open, any local process can drive the logged-in client.

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
  passes `--remote-debugging-port` through `additional_browser_args` instead, which **replaces**
  wry's own `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`, so it restates them.
- `PrintWindow(PW_RENDERFULLCONTENT)` captures a WebView2 window fully, but returned an all-black
  image of the client's CEF window after its GPU process had restarted with `--use-gl=disabled`.
- `decorations: false` keeps Aero Snap, resize borders, shadow and rounded corners; it loses only the
  Windows 11 Snap Layouts flyout (see `accepted-tradeoffs.md`).
