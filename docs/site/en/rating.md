# How rating works

winer rates players in three places:

- **Game analysis**: in champ select and in game, each player gets a tier from recent form, with a
  title and a quip.
- **Callout**: the same tiers, titles and quips, written into the chat.
- **Scoreboard**: every player of every game gets a score and a grade, and the game's MVP, SVP and
  roast titles are marked.

The formulas are winer's own, written in
[`rating.rs`](https://github.com/sunerpy/winer/blob/main/crates/core/src/rating.rs) and
[`tiers.ts`](https://github.com/sunerpy/winer/blob/main/app/frontend/src/lib/tiers.ts); they do not
reproduce the client's, WeGame's or another tool's ratings. The scheme and the titles switch are in
**Settings › Rating**.

## Recent form (0–10)

- The latest 20 games the client returns, custom games left out, so a history with custom games
  counts fewer than 20.
- Half win rate, half KDA: `10 × (0.5 × win rate + 0.5 × (1 − e^(−KDA/3)))`. KDA 3.0 counts as
  0.63, 6.0 as 0.86.
- Few games pull it toward 5.0: `form = c × raw + (1 − c) × 5.0`, with `c = games / (games + 5)`, so
  one or two lucky games cannot rank first.

For reference: twenty games at half won and KDA 3 is about 5.5; half won at KDA 4.3 is about 6.0.

## Schemes

| Scheme              | Tiers, best first                                                                                                              | How                  |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------- |
| Rift five (default) | Rift Demigod · Human Turret · Rift Civil Servant · Walking Ward · Pure Workhorse                                               | ranked in the team   |
| Rift grades         | Rift Demigod · Human Turret · Rift Civil Servant · Useful Person · Walking Ward · Rift ATM · Fountain Watcher · Pure Workhorse | fixed bands, S+ to F |
| Horse universe      | Unicorn · Thousand-li Steed · Blood-sweating Horse · Rift Mule · Lame Horse · Pure Workhorse · Cyber Workhorse                 | ranked in the team   |
| Five horses         | Unicorn · Top horse · Middle horse · Bottom horse · Pack mule                                                                  | ranked in the team   |
| Three horses        | Top horse · Middle horse · Bottom horse                                                                                        | ranked in the team   |
| Rift food chain     | King of the Rift · Carry · Holding up · Passenger · Walking ATM                                                                | ranked in the team   |
| Custom              | two to five names of your own                                                                                                  | ranked in the team   |

With fewer than two custom names, the three horses are used.

**Ranked in the team**: players are sorted by recent form, best first, and the k-th (counting
from 0) lands in the tier that `(k + ½) / players × tiers` falls in. Five players in five tiers get
one each, five in three tiers split 2 / 1 / 2, four in five leave the middle tier empty. A player
whose history is hidden or failed to load gets no tier.

**Rift grades** compares nobody: each player's own recent form decides.

| Grade | S+    | S     | A     | B     | C     | D     | E     | F     |
| ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- |
| Form  | ≥ 7.6 | ≥ 6.8 | ≥ 5.9 | ≥ 5.3 | ≥ 4.8 | ≥ 4.3 | ≥ 3.8 | < 3.8 |

## Callout

By default one line per teammate:

```text
{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}
```

For example:

```text
[Blue side]
Rift Demigod: Ahri Light in the Dark, 60% in 20 games, KDA 4.1, score 7.4 [Patch Champion], the other team is filing a boosting report
Rift Civil Servant: Garen Rift Sweeper, 50% in 20 games, KDA 2.9, score 5.2, not flashy, but every job got done
```

- The first line names your side, followed by the opening line if you wrote one.
- `{title}` is the recent-form title, empty without one; `{quip}` is the tier's quip. Every tier of
  the built-in schemes has one (Rift five has three per tier in Chinese), picked per player and per
  game: the same throughout one champ select, likely another the next game. Custom names have none.
- The line template and the opening line are edited in **Automation › Callout**, with these
  placeholders: `{standing}` `{champion}` `{name}` `{games}` `{winRate}` `{kda}` `{score}`
  `{title}` `{quip}`.

## Roast titles

Turning off **Settings › Rating › Roast titles** hides both kinds below, and leaves `{title}` empty
in the callout.

### Recent-form titles (game analysis and callout)

Five games at least; the first rule that holds:

| Rule                                         | Title                 |
| -------------------------------------------- | --------------------- |
| three wins in a row or more                  | Patch Champion        |
| three losses in a row or more                | Ranked Philanthropist |
| at most 2 kills and at least 7 deaths a game | Esports Bodhisattva   |
| KDA 6 or more                                | Rift Immortal         |
| at least 8 kills and 7 deaths a game         | One-for-one Trader    |
| at least 8 deaths a game                     | Grey-screen Regular   |
| at least 12 assists a game, twice the kills  | Rift Philanthropist   |

### Game titles (scoreboard)

Damage share, damage taken and gold are counted within the team; the first rule that holds:

| Rule                                                                                      | Title               |
| ----------------------------------------------------------------------------------------- | ------------------- |
| at most 1 kill, at least 8 deaths                                                         | Esports Bodhisattva |
| no deaths, at least 10 kills plus assists                                                 | Rift Immortal       |
| bottom carry (bottom lane, damage share 18% or more), at most 2 deaths, 25% of the damage | Alive Means Damage  |
| bottom carry, at least 9 deaths                                                           | De-carry            |
| lost, with 30% of the damage or more                                                      | The Dean            |
| won, with under 12% of the damage                                                         | Missing Piece       |
| at least 10 kills and 10 deaths                                                           | One-for-one Trader  |
| at least 10 deaths                                                                        | Grey-screen Regular |
| KDA 5 or more, under 15% of the damage                                                    | KDA Keeper          |
| 30% of the team's damage taken or more                                                    | Human Turret        |
| 24% of the team's gold or more, under 17% of the damage                                   | Rift Banker         |
| 17% of the team's gold or less, 25% of the damage or more                                 | Self-made Carry     |
| at least 10 assists, three times the kills                                                | Rift Philanthropist |
| kill participation under 35%                                                              | Solo Player         |

Remakes and teams of one earn no title.

## Game score (0–10) and grade

Each part is compared with the game's average player and counts for at most three of them; a part
whose game average is below 1 is left out (vision in ARAM, for example).

| Part                                             | Weight |
| ------------------------------------------------ | ------ |
| takedowns (kills + 0.7 × assists)                | 0.30   |
| damage to champions                              | 0.25   |
| damage taken (mitigated included)                | 0.12   |
| gold                                             | 0.10   |
| damage to objectives                             | 0.08   |
| vision                                           | 0.05   |
| crowd control                                    | 0.05   |
| survival (`(average deaths + 1) / (deaths + 1)`) | 0.05   |

The weighted mean x (how many average players the line was worth) goes into
`10 / (1 + e^(−4 × (x − c)))` with `c = 1 − ln(1.5) / 4`, which puts exactly one average player at
6.0: 1.25 averages score 8.0, 1.5 score 9.2, 0.75 score 3.6, half of one 1.7; the most is 10.

- **MVP**: the best score on the winning side; **SVP**: the best on the losing side. It needs one
  winning and one losing side; a tie goes to the earlier line. In the history list they sit on the
  champion's top-left corner.
- **Game grade**:

  | Grade | S+    | S     | A     | B     | C     | D     | E     | F     |
  | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- |
  | Score | ≥ 9.0 | ≥ 8.0 | ≥ 7.0 | ≥ 6.0 | ≥ 5.0 | ≥ 4.0 | ≥ 3.0 | < 3.0 |

  The game's average player is a B.

- A remake gets no scores and no MVP or SVP.

## Feats

The history list and the scoreboard also mark each player's feats in a game:

| Feat                                                        | Rule                                                                                                       |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Double, triple, quadra, penta kill                          | the game's best multikill, the highest one only                                                            |
| Legendary                                                   | eight kills or more without dying                                                                          |
| First blood                                                 | took the game's first kill                                                                                 |
| Most kills, assists, gold, CS, damage, damage taken, towers | the most in the game; a tie leads together, and when everyone has the same (none at all, say), nobody does |
| AFK                                                         | the game marked the player away: left or idled                                                             |

A remake keeps AFK only. The AFK mark comes from the match-history server and the client's own 20
games do not carry it. The game's leads need all ten players, so they are marked only where the
whole game was read.
