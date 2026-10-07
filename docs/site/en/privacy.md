# Data and privacy

winer has no account, collects no usage data and runs no server of its own. These are the only
places it connects to:

| Connection                         | What it sends                                                  | What for                                                                                                                                      |
| ---------------------------------- | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| The League client on your PC       | the local credentials the client generates                     | reading your summoner, games and history; carrying out the automation                                                                         |
| Your region's match-history server | the access token the client got at sign-in                     | paging through match history, at addresses like `nj100-sgp.lol.qq.com`                                                                        |
| ARAM.GG                            | a plain web request, with the champion when reading statistics | what Hextech ARAM's augments do (can be turned off in **Settings › General**); Hextech ARAM's augment statistics when Tencent's do not answer |
| The Tencent League app             | the champion, lane and patch                                   | the build panel's statistics from the Chinese servers, at `mlol.qt.qq.com`                                                                    |
| OP.GG                              | the champion, mode and lane                                    | the build panel's statistics from the world, at `lol-api-champion.op.gg`                                                                      |
| GitHub                             | a plain web request                                            | checking for and downloading updates                                                                                                          |

The build panel reads the statistics of the champion on the Live game page only (yours, in champ
select and in the game), and once more when a champion is locked in while item sets are written by
themselves. A request carries the champion, mode, lane and patch and nothing about your account or
any player; with **Builds** off in **Settings › General**, nothing is read.

The client's credentials and access token are used in memory only; they are never written to the
settings file or a log.

## What stays on your computer

| What                                | Where                                                                                 | How long, and how large at most                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Settings                            | `%APPDATA%\app.winer.desktop\settings.json`                                           | kept, a few KB                                                                                           |
| Remembered runes and spells         | `%APPDATA%\app.winer.desktop\loadouts.json`                                           | kept, one setup per champion and mode, under 1 MB with every one remembered; forgotten in **Automation** |
| Player notes                        | `%APPDATA%\app.winer.desktop\notes.json`                                              | kept, 5,000 notes at most, 200 characters each; deleted in **Tools › My notes**                          |
| The client version last seen        | `%APPDATA%\app.winer.desktop\diagnostics.json`                                        | kept, the version number alone, for the self-check after the client updates                              |
| Game settings backups               | `%LOCALAPPDATA%\app.winer.desktop\game-settings`                                      | 10 at most, a few KB each; a new one past that removes the oldest                                        |
| The bundled Pengu Loader and plugin | `%LOCALAPPDATA%\app.winer.desktop\pengu`                                              | about 0.5 MB, updated with winer                                                                         |
| Logs                                | `%LOCALAPPDATA%\app.winer.desktop\logs`                                               | the last 7 days, 50 MB in all and 10 MB a file at most; past that the oldest files go first              |
| The window's WebView data           | `%LOCALAPPDATA%\app.winer.desktop\EBWebView`                                          | its page cache 32 MB at most; the rest are components WebView2 downloads and updates itself              |
| Update installers                   | folders starting `winer-` and containing `-updater-` in the system's temporary folder | removed when winer next starts after the update                                                          |

Logs are kept by day (UTC dates). winer checks them when it starts and once a day while it runs, and
removes the files past those limits.

While it runs, winer also caches icons, match history, players and builds in memory: pictures 32 MB
at most, everything else a number of entries at most. What goes unused for a while is let go on a
schedule, and all of it goes when winer quits.

**Storage** in **Settings › About** lists how large each of these is and its limit. **Clear cache**
removes old logs, update installers and the caches in memory, and clears the WebView's cache the next
time winer starts; the settings, the remembered runes and spells and the game settings backups stay.

## The client plugin

The plugin runs in the client's interface pages and connects only to a port winer opens on your
computer, with a token winer hands it. It reaches no outside address.

## This site

firlab.app is a static site hosted on GitHub Pages. winer opens it in your browser and sends it
nothing itself.
