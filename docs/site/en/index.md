---
layout: home
title: "winer: a League of Legends client companion"
titleTemplate: false
description: See every teammate's recent form and tier in champ select, page through anyone's match history, and leave accepting, picking and the opening callout to winer. Windows only, tested on the Tencent client.

hero:
  name: winer
  text: A League of Legends client companion
  tagline: See every teammate's recent form and tier in champ select, page through anyone's match history by name, and leave accepting, picking and the opening callout to winer. Windows only, tested on the Tencent client.
  actions:
    - theme: brand
      text: Install
      link: /en/guide/install
    - theme: alt
      text: Quick start
      link: /en/guide/quick-start
    - theme: alt
      text: GitHub
      link: https://github.com/sunerpy/winer

home:
  facts:
    - term: Runs on
      text: 64-bit Windows 10 or 11, tested on the Tencent client; installs for the current user without administrator rights.
    - term: How it works
      text: Uses only the API the client opens on your computer; never reads the game's memory or changes game files.

  visual:
    desktop:
      light: /screens/live-light.webp
      dark: /screens/live-dark.webp
      width: 1440
      height: 900
      alt: The game analysis in champ select, one row per teammate, tier, title, rank, win rate and KDA on the left and every recent game on the right.

  index:
    title: What winer does
    intro: Everything at a glance. Every automation is off until you turn it on, and can be limited to the modes you choose.
    groups:
      - name: In the game
        items:
          - title: Game analysis
            body: One row per player on both teams in champ select and in game, with rank, recent win rate, KDA and every recent game, sides and parties.
            status: available
            link: /en/guide/live
          - title: Tiers and roast titles
            body: By default the team of five is ranked into five Rift tiers, each with a quip; or graded S+ to F on fixed bands, or ranked by horses or names of your own.
            status: available
            link: /en/rating
          - title: Callout
            body: A chat message ranked by tier, sent to the team or to you alone in one click or every game by itself, its first line naming your side and winer.
            status: available
            link: /en/guide/live#callout
          - title: ARAM bench
            body: Swap to a bench champion in one click without the cooldown, or let a wishlist do it.
            status: available
            link: /en/guide/live#aram-bench
          - title: Builds
            body: A champion's items, runes, spells, skill order and matchups in champ select, in game and outside a game, and how strong each augment is in Arena and Hextech ARAM.
            status: available
            link: /en/guide/live#builds
      - name: History
        items:
          - title: The whole history
            body: Anyone's games by name, page by page to the first one, filtered by ranked, normal and ARAM.
            status: available
            link: /en/guide/history
          - title: MVP and feats
            body: Every game marks its MVP and SVP, multikills, Legendary, first blood, the game's leads and AFK players.
            status: available
            link: /en/guide/history#each-game
          - title: Scoreboard
            body: A score and an S+ to F grade for every player, damage share, damage taken, kill participation, gold and items, the game's best values picked out.
            status: available
            link: /en/guide/history#scoreboard
      - name: Automation
        items:
          - title: Accept matches
            body: Accepts the match for you, after a wait you choose.
            status: available
            link: /en/guide/automation#match-found
          - title: Pick and ban
            body: Picks and bans by position from your own lists, skipping what is banned, taken or shown by a teammate.
            status: available
            link: /en/guide/automation#champion-pick-and-ban
          - title: Back to the lobby
            body: Returns to the lobby when the game ends, ready for the next one.
            status: available
            link: /en/guide/automation#after-the-game
          - title: Runes and summoner spells
            body: Once your champion is locked in, sets up the runes and spells you last played it with in the mode, or the client's recommendation.
            status: available
            link: /en/guide/automation#runes-and-summoner-spells
          - title: Item sets
            body: Once your champion is locked in, writes its build as an item set, changing only winer's own.
            status: experimental
            link: /en/guide/automation#item-sets
      - name: Client and tools
        items:
          - title: Client plugin
            body: winer ships Pengu Loader and sets it up by itself; teammates' form, tiers and titles appear in the client's champ select, and bench champions swap on a click.
            status: available
            link: /en/guide/client
          - title: Status and message
            body: Switch between online, away and invisible, edit your status message, restart a stuck client interface.
            status: available
            link: /en/guide/settings#tools
          - title: Five themes
            body: Light, Dark, Graphite, Hextech and System, eight accents, the tray and start with Windows.
            status: available
            link: /en/guide/settings#settings

  steps:
    title: From install to the first callout
    items:
      - title: Install
        command: irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
        body: Run it in PowerShell; the installer runs only once it matches SHA256SUMS. Or download the installer and run it.
      - title: Connect
        body: Sign in to the client, then open winer. When the Tencent client runs as administrator, winer asks to restart the same way; choose Yes. The client plugin is set up next, by itself.
      - title: Enter champ select
        body: The Live game page lists every teammate's form and tier; "Send to team" posts the callout.

  shots:
    history:
      light: /screens/history-light.webp
      dark: /screens/history-dark.webp
      width: 1440
      height: 900
      alt: The history page, each game with its result, length, feats, KDA and items, MVP or SVP on the champion, one game open on its scoreboard.
    rating:
      light: /screens/rating-light.webp
      dark: /screens/rating-dark.webp
      width: 1440
      height: 900
      alt: The rating settings, seven schemes, the roast titles switch and the basis of every rating.

  privacy:
    title: Where data goes
    intro: winer has no account, collects no usage data and runs no server of its own. These are all the places it connects to.
    sendsLabel: Sends
    modes:
      - name: The client on your PC
        sends: the local credentials the client generates
        detail: Reads your summoner, games and history, and carries out the automation you turned on.
      - name: Your region's match-history server
        sends: the access token the client got at sign-in
        detail: Pages through match history; without it, the client's latest 20 games.
      - name: The Tencent League app, OP.GG and ARAM.GG
        sends: the champion, mode, lane and patch
        detail: Public statistics for the build panel, for the champion on the Live game page only; can be turned off.
      - name: ARAM.GG and GitHub
        sends: plain web requests
        detail: What Hextech ARAM's augments do (can be turned off), and checking for and downloading updates.
---

<HomeIndex />

<HomeSteps />

<SplitBlock proof="screen" shot="history">

## Anyone's history, page after page

The Tencent client's own API returns the latest 20 games only. winer asks the region's match-history
server page by page instead, so it reaches the first game, filtered by ranked, normal and ARAM.

Every game marks its MVP, SVP and feats: double to penta kills, Legendary, first blood, the game's
leads, and players the game marked AFK. Open one for the full scoreboard with every player's score
and grade.

[Match history and feats](/en/guide/history) · [How rating works](/en/rating)

</SplitBlock>

<SplitBlock proof="screen" shot="rating" flip>

## Tiers with a sense of humour, and every rule written down

By default the team of five is ranked from Rift Demigod to Pure Workhorse, each tier with a quip,
and a teammate on three wins in a row is called the Patch Champion. Rather not compare? Rift grades
rate each player S+ to F on fixed bands.

Recent form, the game score, every title's rule and every feat's rule are written out in the
settings and in these pages; nothing is guesswork.

[How rating works](/en/rating) · [Live game and callout](/en/guide/live)

</SplitBlock>

<HomePrivacy />

## Install

::: code-group

```powershell [One command]
irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
```

```powershell [A pinned version]
$env:WINER_VERSION = "0.0.1"
irm https://github.com/sunerpy/winer/releases/download/v0.0.1/install.ps1 | iex
```

:::

The script checks the installer against the same release's `SHA256SUMS` and installs silently only
when it matches. You can also download `winer_<version>_x64-setup.exe` from
[GitHub Releases](https://github.com/sunerpy/winer/releases) and run it.
[Install](/en/guide/install) covers verifying, updating and uninstalling.

## Feedback

Report a problem or suggest something: [GitHub Issues](https://github.com/sunerpy/winer/issues).
