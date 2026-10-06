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

| What                        | Where                                       |
| --------------------------- | ------------------------------------------- |
| Settings                    | `%APPDATA%\app.winer.desktop\settings.json` |
| Remembered runes and spells | `%APPDATA%\app.winer.desktop\loadouts.json` |
| Logs                        | `%LOCALAPPDATA%\app.winer.desktop\logs`     |

## The client plugin

The plugin runs in the client's interface pages and connects only to a port winer opens on your
computer, with a token winer hands it. It reaches no outside address.

## This site

firlab.app is a static site hosted on GitHub Pages. winer opens it in your browser and sends it
nothing itself.
