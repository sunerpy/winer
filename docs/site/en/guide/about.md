# Acknowledgements and disclaimer

## Acknowledgements

- [Pengu Loader](https://github.com/PenguLoader/PenguLoader) (MIT): loads plugins into the client's
  interface. winer ships its `core.dll`; the licence text is published with the source in
  [THIRD_PARTY_NOTICES.md](https://github.com/sunerpy/winer/blob/main/THIRD_PARTY_NOTICES.md) and
  shown in **Settings › About**.
- [Tauri](https://tauri.app) and the Rust and React open-source ecosystems, which winer's window
  and core are built on.
- [ARAM.GG](https://aramgg.com), for what Hextech ARAM's augments do, and Hextech ARAM augment
  statistics when Tencent's cannot be read.
- Tencent's League of Legends companion app and its public game statistics: builds, runes, summoner
  spells, skill orders, matchups and augments for Summoner's Rift and Hextech ARAM, from games on
  the Chinese servers.
- [OP.GG](https://www.op.gg), for ARAM and Arena builds and augments, and as an optional source for
  Summoner's Rift.
- [WeGame](https://www.wegame.com.cn): the game score's weights were calibrated on WeGame's public MVP
  and SVP results; its formula is not used.

## Disclaimer

- winer is an unofficial third-party tool. It is not affiliated with, endorsed, sponsored or
  supported by Riot Games, Tencent or WeGame. League of Legends and its names, logos and images
  are trademarks or property of Riot Games, Inc.
- winer uses only the API the client opens on your computer. It never reads or changes the game's
  memory or files, and never plays for you. The one exception is **In-game sending**, off by
  default: with it on, pressing the callout's shortcut in a game makes winer type the callout into
  the game's chat with synthesized key presses, which is third-party input. The in-client features
  run a plugin in the client's interface through Pengu Loader, which changes what the client shows.
- Any third-party tool may break the game's terms of service, and using winer may get an account
  restricted. You use it at your own risk.
- Scores, tiers, titles and callouts are entertainment drawn from match data and say nothing about
  anyone's real skill. Do not use them to insult or harass other players.
- Rank disguise changes only the rank friends see in the friends list and on your hover card; your
  real rank, matchmaking and your own profile in the client do not change. What friends see then is
  not your real rank: do not use it to mislead anyone.
- Build, rune, summoner spell, skill order and augment recommendations come from the third-party
  public statistics listed above, and winer does not guarantee they are accurate or current. Those
  sources are not offered for third-party tools and may change or stop at any time; the panels then
  say the numbers cannot be read.
- Setting up runes and summoner spells and writing item sets change the client only once you switch
  the matching rule on; both are off by default.
- winer is provided "as is" under the MIT licence, without warranty of any kind; its authors are not
  liable for any loss from its use.

## Licence

winer's source is published under the
[MIT licence](https://github.com/sunerpy/winer/blob/main/LICENSE). The open-source components it is
built from each have their own licence, listed in
[THIRD_PARTY_NOTICES.md](https://github.com/sunerpy/winer/blob/main/THIRD_PARTY_NOTICES.md).
