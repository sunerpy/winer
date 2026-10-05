# Live game and callout

The **Live game** page analyses both teams in champ select and while the game runs, and empties
once the client is back in the lobby.

## Game analysis

One row per player:

- **Left**: champion, name, tier and title, rank, recent win rate, KDA and streak. Under the tier is
  its quip. A player whose history is hidden shows as "Hidden player" and gets no tier.
- **Right**: the latest 6 to 12 games (more on a wider window), one tile each: champion,
  kills/deaths/assists and mode; a win is green, a loss red, a remake grey. Hover a tile for its
  queue, result and age.

Choose a name to open that player's history. When both teams are known, a switch at the top right
moves between them.

- **Sides**: on maps with a blue and a red side (Summoner's Rift, Howling Abyss), the heading says
  which one your team is on, and the callout's first line carries `[Blue side]` or `[Red side]`.
  Arena has no sides.
- **Parties**: once the game is running, players queued together carry the same number
  ("Party 1", "Party 2").
- **Tiers**: from recent form, in the scheme chosen in **Settings › Rating**; the rules are in
  [How rating works](/en/rating).

## Callout

The callout is ready once every teammate's history has loaded: the first line holds the side,
"winer rating" and the opening line if you wrote one, then one line per teammate, best first, for
example:

```text
[Blue side] winer rating
Rift Demigod: Ahri Light in the Dark, 60% in 20 games, KDA 4.1, score 7.4 [Patch Champion], the other team is filing a boosting report
```

- **Send to team**: posts it to the team chat of champ select, where your teammates see it.
- **Only me**: shows it in your own chat only, to look before sending.
- **Automatically**: turn on **Send in champ select automatically** in **Automation › Callout** and
  choose who gets it and whether your own line is in it. The opening line and the line template are
  edited there too, with a live preview.

## ARAM bench

In ARAM and Hextech ARAM the champions on the bench are listed at the top of the page: choose one
and you swap at once, without the client's cooldown. While rerolls remain, **Reroll** sits on the
right. With a wishlist set in **Automation › Bench**, a wishlist champion that ranks above the one
you hold is taken by itself; wishlist champions carry a star.
