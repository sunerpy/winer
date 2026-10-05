# Data and privacy

winer has no account, collects no usage data and runs no server of its own. These are the only
places it connects to:

| Connection                         | What it sends                              | What for                                                                     |
| ---------------------------------- | ------------------------------------------ | ---------------------------------------------------------------------------- |
| The League client on your PC       | the local credentials the client generates | reading your summoner, games and history; carrying out the automation        |
| Your region's match-history server | the access token the client got at sign-in | paging through match history, at addresses like `nj100-sgp.lol.qq.com`       |
| ARAM.GG                            | a plain web request                        | what Hextech ARAM's augments do; can be turned off in **Settings › General** |
| GitHub                             | a plain web request                        | checking for and downloading updates                                         |

The client's credentials and access token are used in memory only; they are never written to the
settings file or a log.

## What stays on your computer

| What     | Where                                       |
| -------- | ------------------------------------------- |
| Settings | `%APPDATA%\app.winer.desktop\settings.json` |
| Logs     | `%LOCALAPPDATA%\app.winer.desktop\logs`     |

## The client plugin

The plugin runs in the client's interface pages and connects only to a port winer opens on your
computer, with a token winer hands it. It reaches no outside address.

## This site

firlab.app is a static site hosted on GitHub Pages. winer opens it in your browser and sends it
nothing itself.
