# Live game and callout

The **Live game** page lists your party in a lobby, analyses both teams in champ select and while the game runs, with the [build](#builds) of your champion, and empties once the client is back on its home screen, leaving the champion lookup.

## Game analysis

One row per player:

- **Left**: champion, name, tier and title, rank, recent win rate, KDA and streak. Under the tier is
  its quip. A player whose history is hidden shows as "Hidden player" and gets no tier.
- **Right**: the latest 6 to 12 games (more on a wider window), one tile each: champion,
  kills/deaths/assists and mode; a win is green, a loss red, a remake grey. Hover a tile for its
  queue, result and age. When a player's history could not be read, **Retry** here reads it again.

Choose a name to open that player's history. When both teams are known, a switch at the top right
moves between them.

- **Sides**: on maps with a blue and a red side (Summoner's Rift, Howling Abyss), the heading says
  which one your team is on, and the callout's first line carries `[Blue side]` or `[Red side]`.
  Arena has no sides.
- **Parties**: once the game is running, players queued together carry the same number
  ("Party 1", "Party 2") and one colour per party; the number always shows, so the colour is never
  needed to tell them apart. In champ select, the teammates who came in from your own lobby are
  marked too. Other teammates who lately played at least two games on one team together, one of
  them among either's last ten, are marked **Likely party** with a dashed swatch: that is read from
  their games and can be wrong. Only the Tencent shards' history lists all ten players of a game,
  so elsewhere nothing is inferred.
- **Tiers**: from recent form, in the scheme chosen in **Settings › Rating**; the rules are in
  [How rating works](/en/rating).

## Lobby

In a lobby (and while it queues or a match is found), the **Live game** page lists every member of
your party: name, the positions asked for, rank, win rate and KDA over the last 20 games, recent
form score and the latest games; the party's leader carries **Leader**. Choose a member to open
their history. In a custom lobby, everyone in it is listed.

## Friends

The **Friends** panel on the **Overview** page lists the friends in champ select or in a game: the
mode, the state and how long it has been going, counted by the second; a game that can be watched
carries an eye. Friends in one game or one party share a colour and a number. Choose a friend to
open their history.

## Callout

The callout is ready once every teammate's history has loaded: the first line holds the side,
"winer rating" and the opening line if you wrote one, then one line per teammate in champ-select order from P1 through P5. Each
line names the teammate's place in your team's list in champ select (P1 to P5, from the top) and
their name; in Chinese the name sits in 【】, since the client's chat filter reads across spaces and
would join a tier and a name into one word (see [How rating works](/en/rating#callout)). The
default, rich style adds the tier's emoji, the title and the quip, for example:

```text
📢 [Blue side] winer rating
👑 Rift Demigod: P1 Light in the Dark, 60% in 20 games, KDA 4.1, form 7.4 [Patch Champion], the other team is filing a boosting report
```

With **Callout style** set to compact in **Automation › Callout**, each player gets one short line,
the same fields in the same order, to compare at a glance:

```text
[Blue side] winer rating
P1 Rift Demigod | 60% | KDA 4.1 | form 7.4 | Light in the Dark
```

- **Send to team**: posts it to the team chat of champ select, where your teammates see it.
- **Only me**: shows it in your own chat only, to look before sending.
- **Automatically**: turn on **Send in champ select automatically** in **Automation › Callout** and
  choose who gets it and whether your own line is in it. The opening line and the line template are
  edited there too, with a live preview.
- **Shortcut**: with a shortcut to send the callout set in **Automation › Callout**, pressing it in
  champ select does what **Send to team** does, without winer's window open. The panel's last line
  names the shortcut, or says where to set one.

Once the game starts, the panel has two columns: **Enemies** holds the enemy to watch and the one to
go after, chosen as [Automation](/en/guide/automation#lines-in-game) describes; **My team** holds one
line per teammate in the same team-list order as champ select. In the game the champions are settled and are how
players tell each other apart, so both columns name every player by champion (in 【】 in Chinese),
without name or seat; where the champion is not known, the name stands in. For example:

```text
[Enemy · Red side] winer rating
Watch Kha'Zix: Rift Demigod, 65% in 20 games, KDA 4.6
Go after Yasuo: Pure Workhorse, 35% in 20 games
```

The game's chat has no API, so the panel has no send button then: with **In-game sending** on,
pressing the callout's shortcut in the game types the chosen lines into the team chat. By default
that is the enemy column; **Automation › Callout** can switch it to your team's or both. The panel's
heading says whether in-game sending is on; its last line names the shortcut and what a press types.
Modes without sides, such as Arena, have no in-game lines and show no such panel.

## ARAM bench

In ARAM and Hextech ARAM the champions on the bench are listed at the top of the page: choose one
and you swap at once, without the client's cooldown. While rerolls remain, **Reroll** sits on the
right. With a wishlist set in **Automation › Bench**, a wishlist champion that ranks above the one
you hold is taken by itself; wishlist champions carry a star.

## Pick suggestions

In Summoner's Rift ranked and normal champ select, once you have a lane and until you lock in, three
champions are suggested above the teams, each with why:

- **Beats / Loses to**: an enemy already locked in appears in the champion's lane matchups, with its
  win rate against that enemy.
- **Tier 1–5**: the source's standing for it (OP.GG gives one, Tencent 101 does not).
- **Your games in the lane**: how many games you played it in this lane lately, and won; only the
  Tencent shards' history says which lane a game was.
- **On your pick list**: it is on the list in **Automation › Auto-pick a champion**.

The candidates are your pick list for the lane (then Any lane) and the five champions you play most
there; banned and locked champions, and those a teammate picked or showed, never appear. The
suggestions are shown only: winer hovers and locks nothing for them. **Pick suggestions** in
**Settings › General** turns them off.

## Builds

In champ select, the build of the champion you hover or lock in sits under the analysis; while the
game runs, switch to winer to see the one you are playing. Without a game, choose a champion and a
mode at the bottom of the page. The panel has sections:

- **Items**: starting items, boots, core items (three together) and late options, each with its
  pick rate and win rate.
- **Runes**: the most played pages with their pick rate, win rate and games. **Use these runes**
  writes the page to winer's own rune page and makes it current; the rules are in
  [Automation](/en/guide/automation#runes-and-summoner-spells).
- **Spells**: the usual pairs. In champ select, **Take these spells** takes them at once; a spell
  you already hold stays on its key.
- **Skills**: the max order and the points for the first 15 levels.
- **Matchups** (Summoner's Rift): the opponents the champion beats most and loses to most; the win
  rate is the champion's own against them.
- **Augments** (Arena, Hextech ARAM): grouped as prismatic, gold and silver, best first, with a
  filter by name. Hextech ARAM shows each augment's grade (S to C); Arena shows the average place and
  the share of first places, because wins do not count there. What an augment does comes from
  ARAM.GG (it can be turned off in **Settings › General**). Augments are picked in the game, so this
  section opens first while the game runs.

A section without numbers is left out; Hextech ARAM, for one, has no rune or spell statistics. On
Summoner's Rift a switch on the right changes the lane; with no lane assigned (blind pick, a
lookup), the lane the champion is played in most comes first.

### Where the numbers come from

The panel's heading names the source and the patch, for example "Data: Tencent 101 · 16.19", and
the games behind the numbers.

| Mode           | Source                                                                                   |
| -------------- | ---------------------------------------------------------------------------------------- |
| Ranked, Normal | Tencent's 101 statistics from the League app (the Chinese servers), or OP.GG (the world) |
| ARAM           | OP.GG, or off                                                                            |
| Hextech ARAM   | Tencent's League app, and ARAM.GG when that does not answer; the fallback can be off     |
| Arena          | OP.GG, or off                                                                            |
| Other modes    | no build numbers                                                                         |

All of these are switched in **Settings › General › Data sources**. OP.GG's terms do not allow
automated collection, and ARAM.GG asks programs not to read its data files; if that matters to you,
switch the row off. With ARAM's or Arena's source off, that mode's panel only says the source is
switched off, and remembered runes and spells are still set up; with the Hextech fallback off, only
Tencent's numbers are used.

These are public statistics from third parties and can lag a patch; in a patch's first days, when
there are none yet, the previous patch's are used. When a source does not answer, the panel says so
and offers **Retry**. Items and augments the client does not know (from another patch, say) are left
out. winer reads the numbers of the champion on screen only, keeps them for a few hours, and never
reads other champions ahead. With **Builds** off in **Settings › General**, the panel is hidden and
nothing is fetched.

### Writing the item set

**Write the item set** in **Items** writes the items as the champion's item set, named after the
champion ("winer · Jhin"), which the in-game shop lists. One written in champ select is there in
that game; one written after the game started, from the next game. Writing it by itself, and removing it, are in
[Automation](/en/guide/automation#item-sets).
