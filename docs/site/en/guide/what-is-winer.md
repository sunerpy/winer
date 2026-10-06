# What is winer

winer is a companion for the League of Legends client, for Windows only, tested on the Tencent
client (the China servers). It reads the API the client itself opens on your computer (the LCU); it
does not change game files or read the game's memory.

It has two parts:

- **The desktop window**: overview, live game analysis, match history, automation and tools. It is
  there whenever the client runs, stays in the tray when you close it, and comes up over a game
  with a shortcut.
- **The client plugin**: loaded into the client's pages by
  [Pengu Loader](https://github.com/PenguLoader/PenguLoader), it shows teammates' form, tier and
  title right in champ select, friends' games in the friends list and the party's recent form in the
  lobby, and makes ARAM bench champions swap on a click. winer ships Pengu Loader and sets it up when
  it connects to the client; it can be turned off.

## What it does

- **Live analysis**: from champ select on, one row per player on both teams: rank, recent win rate,
  KDA and the latest games, the side of the map and premade groups, and a tier, a title and a quip
  for each player from their recent form. See [Live game and callout](/en/guide/live).
- **Friends and parties**: the overview lists the friends in champ select or in a game and for how
  long, friends playing together in one colour; in a lobby, the party's rank and recent form before
  the game starts. See [Live game and callout](/en/guide/live).
- **Match history**: anyone's history by name, page by page to the very first game, filtered by
  mode; every game marks its MVP, SVP and feats, and opens into a scoreboard with a score for each
  player. See [Match history and feats](/en/guide/history).
- **Automation**: accept matches, pick and ban by position, post the callout when champ select
  starts, go back to the lobby after a game and take wished-for ARAM bench champions, each limited to
  the modes you choose. See [Automation](/en/guide/automation).
- **Tools**: switch your status (online, away, invisible), edit your status message, restart the
  client's interface.

Scores and tiers are winer's own formulas, all written out in [How rating works](/en/rating).

## What it does not do

- No macOS or Linux version.
- It does not read the game's memory, change game files or inject into the game itself; the plugin
  runs only in the client's interface pages.
- It collects no usage data and needs no account. Where it connects is listed in
  [Data and privacy](/en/privacy).

winer is not a product of Riot Games or Tencent and is not endorsed by them.
