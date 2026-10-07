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

Recent form reads how each game was played, not only wins and KDA: the score of every one of the last
20 games (see one game's score, below), averaged with weights, then read as a place among players. 5.0 is the average player, 9.0 better than nine in ten.

### Which games

- The newest 20 games played against other players. Custom games (the practice tool included) and
  games against the computer are left out and take no place among the 20. Which games are against
  the computer is the client's own queue catalog's word: queues in its co-op vs AI category, and
  queues whose type names the computer as the opponent (Doom Bots, for one); the tutorial counts as
  well.
- Remakes among the 20 are shown, and count toward no win, KDA, streak or form.
- In champ select, in game and in a lobby only the mode being played counts, never mixed with
  another: Summoner's Rift (ranked, normal, quickplay) apart from ARAM (classic and Hextech), Arena
  and the rotating modes each on their own. With fewer than 20 games of it, the games there are
  count, and the fewer they are the harder they are pulled toward the average; with none, the
  player gets no tier there. The History page reads every mode.
- The games come from the shard's match-history server first, with all ten players of each: the
  newest 20, then 40 further back at a time, until there are 20 games that count as above, 100 games
  at most: older games are not read. Where the server cannot be read (a shard other than
  Tencent's, or the server failing), they come from the client's own list, which holds the
  player's own row of each game only. A Tencent client's list holds 5 games right after signing
  in, 20 a few minutes later.

### Each game's score

- With all ten players known, it is the scoreboard's score: the line against the game's other
  players.
- With the player's own row only, the line is set against the average player of the same mode over
  the same minutes, with the same weights. That average was measured on real games (4,590 Rift
  lines, 8,370 of the two ARAMs, October 2026). It cannot tell a bloody game from a quiet one, so it
  agrees less with WeGame's score: on 155 Hextech ARAM games the rank correlation is 0.83 against
  the game's players and 0.69 alone. Read that way, games are also pulled toward the average harder.
  In Arena and other modes without an average, such a game counts its result only.
- Each score is then set against its position or champion role. On the Rift a support's line scores
  5.24 on average and a jungler's 6.49, which says nothing about who plays better; in ARAM, where the
  champion is drawn, a support champion's line scores 6.44 and an assassin's 5.69. So each game first
  gets an offset that brings every position and role to the same average:

  | Rift position | Top   | Jungle | Middle | Bottom | Support |
  | ------------- | ----- | ------ | ------ | ------ | ------- |
  | Offset        | +0.16 | −0.46  | −0.07  | −0.41  | +0.79   |

  | ARAM role | Tank  | Support | Mage  | Assassin | Marksman | Fighter |
  | --------- | ----- | ------- | ----- | -------- | -------- | ------- |
  | Offset    | +0.08 | −0.61   | −0.15 | +0.14    | +0.13    | +0.05   |

  The position is the one the match-history server records; with the client's list only, it is read
  from the lane that list records, and with none found there is no offset. The role is the
  champion's first in the client's champion list.

### Putting it together

1. **Weighted average**: the newest game weighs 1 and each older one 0.93 times the next, so about
   ten games halve a game's weight. A game in which someone else left or idled (on either side; the
   player's own leaving does not count) weighs 0.4 times as much again.
2. **Pulled toward the average**: `performance = c × weighted average + (1 − c) × average line`,
   with `c = n / (n + k)`. `n` is how many whole games the weights add up to, `(Σw)² / Σw²`, about 17
   for twenty games; `k` is 25 on the Rift and 10 in ARAM, 1.4 times that with the player's own rows
   only. It is larger on the Rift because Rift scores swing more: one player's games scatter about
   1.81 around their own level (1.20 in ARAM), while players' levels differ by only 0.34 (0.41), so
   twenty Rift games say less than twenty ARAM games. The average line is the score of a game's
   average player once set against position or role: 6.04 on the Rift, 5.83 in ARAM.
3. **The win rate counts a twentieth**: `raw form = 0.95 × performance + 0.05 × 10 × (wins + 5) /
(games + 10)`. A game's score already holds what wins games (gold, kills, staying alive), so the
   result itself only corrects it, and only after five wins and five losses are added: two wins of
   two count as 58%.
4. **Read as a place among players**: the raw form is set on a normal curve centred on a player with
   an average line in every game and half of them won, as wide as the measured players' raw forms
   spread: 0.232 on the Rift and 0.300 in ARAM, 0.189 and 0.255 with the player's own rows only. The
   share of players below, times ten, is the form: 5.0 the average player, 7.0 better than seven in
   ten, 9.5 better than 95%.
5. **Recent trend**: with ten or more scored games, a tenth of how far the newest five average above
   or below the fifteen before them is added, at most ±0.2.

For reference: on the Rift, a 6.04 in every game with half of them won is 5.0; 7.0 in every game is
9.4, and 5.0 in every game is 0.4.

Every number above was measured. In October 2026, 70 real players on one Tencent shard were sampled
with their newest 20 games each: 40 met in Hextech ARAM and 30 met on Summoner's Rift. Only each
game's numbers were kept, no names, ids or times (`fixtures/sgp/players.json`), and the tests check
every constant above against them. The game score itself is unchanged: on 155 Hextech ARAM games
WeGame scored, it orders each game's ten players with a rank correlation of 0.85 to WeGame's, and its
MVP is WeGame's in 78% of them and its SVP in 72%. Adding experience, healing on teammates and the
like brought no improvement that would show.

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

**Rift grades** compares nobody: each player's own recent form decides. The form is already a place
among players, so each grade holds a fixed share of them:

| Grade            | S+    | S     | A     | B     | C     | D     | E     | F     |
| ---------------- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- |
| Form             | ≥ 9.5 | ≥ 8.5 | ≥ 7.0 | ≥ 5.0 | ≥ 3.0 | ≥ 1.5 | ≥ 0.5 | < 0.5 |
| Share of players | 5%    | 10%   | 15%   | 20%   | 20%   | 15%   | 10%   | 5%    |

### One player alone

The History page shows a player's tier, title and quip too, worked out as in champ select. Ranking
in the team needs a team, though: one player ranked against themselves always lands in the middle
tier. So with one player alone, the player first takes Rift grades' fixed band, and the eight
grades are then spread in order over the scheme's tiers the way a team is: grade g (S+ is 0) lands
in the tier that `(g + ½) / 8 × tiers` falls in. For the default Rift five:

| Rift grades | S+, S        | A            | B, C               | D            | E, F           |
| ----------- | ------------ | ------------ | ------------------ | ------------ | -------------- |
| Rift five   | Rift Demigod | Human Turret | Rift Civil Servant | Walking Ward | Pure Workhorse |

Under Rift grades the tier is the grade, as in champ select. Under the other schemes the same
player's tier in champ select also depends on the teammates, and can differ from the History
page's.

## Callout

One line per teammate, kept in champ-select order from P1 through P5; an unrated teammate is skipped, and a tier never rearranges the lines. **Callout style** in **Automation › Callout** picks the
default line:

- **Rich** (the default): an emoji before the tier, the title and the tier's quip as well.

  ```text
  {emoji}{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}
  ```

  ```text
  📢 [Blue side] winer rating
  👑 Rift Demigod: P1 Light in the Dark, 60% in 20 games, KDA 4.1, form 7.4 [Patch Champion], the other team is filing a boosting report
  👌 Rift Civil Servant: P2 Rift Sweeper, 50% in 20 games, KDA 2.9, form 5.2 [Business as Usual], not flashy, but every job got done
  ```

- **Compact**: one short line a player, the same fields in the same order and the name last, so
  the lines compare at a glance.

  ```text
  {seat} {standing} | {winRate} | KDA {kda} | form {score} | {name}
  ```

  ```text
  [Blue side] winer rating
  P1 Rift Demigod | 60% | KDA 4.1 | form 7.4 | Light in the Dark
  P2 Rift Civil Servant | 50% | KDA 2.9 | form 5.2 | Rift Sweeper
  ```

`{emoji}` is the tier's emoji: 👑 for the best tier, 🔥 for the others above the middle, 👌 for the
middle, 😅 below it and 💀 for the worst; in Rift grades S+ is 👑, S and A 🔥, B and C 👌, D and E 😅
and F 💀. Emoji show in the client's chat only; lines typed in a game have none.

- The first line names your side and "winer rating", followed by the opening line if you wrote one;
  the rich style puts 📢 before it.
- The Chinese default follows Sona's `seat: tier|win rate|KDA|strength` format and omits player
  names, titles and quips. Champ-select seats already identify the players, and removing free-form
  text prevents the client filter from joining 上等马 with a name such as 会跑路的防御塔. The window
  and in-client panel still show those details; a custom template may still include them.
- `{seat}` is the teammate's place in your team's list in champ select, counted from the top: P1 to
  P5 (1L to 5L in Chinese). A line names the seat and the player, not the champion: champions can
  still change during champ select, seats do not.
- `{title}` is the recent-form title in brackets, empty without one; `{quip}` is the tier's quip.
  Every tier of the built-in schemes has one (Rift five has three per tier in Chinese), picked per
  player and per game: the same throughout one champ select, likely another the next game. Custom
  names have none.
- The line template and the opening line are edited in **Automation › Callout**, with these
  placeholders: `{emoji}` `{standing}` `{seat}` `{name}` `{champion}` `{games}` `{winRate}` `{kda}`
  `{score}` `{title}` `{quip}`. A template of your own is used in either style. It can still use
  `{champion}`: the champion the teammate has picked or shown when the callout is sent. A
  placeholder in 【】, 「」, () or [] that has no value takes its brackets with it.

### Lines in game

In a game, the callout's shortcut (with **In-game sending** on) talks about two enemies by default,
rated as teammates are:

- **Enemy to watch**: the best rated of the enemies above the middle of the scheme; within a tier,
  the higher recent-form score.
- **Enemy to go after**: the worst rated of the enemies below the middle; within a tier, the lower
  score.

A scheme that ranks the team splits around its middle tier: of five tiers the first two are above
the middle and the last two below; of three, only the first and the last count. In Rift grades,
S+, S and A are above the middle, D, E and F below, and B and C are ordinary form. Without such an
enemy the line is left out. **Automation › Callout** can make it talk about your team or both
instead: one line per teammate, in the tier champ select gives them. The defaults:

```text
Watch {champion}: {standing}, {winRate} in {games} games, KDA {kda}{title}
Go after {champion}: {standing}, {winRate} in {games} games
{standing}: {champion}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}
```

The first two are about the enemies, the third is the line for each teammate; in the compact
style a teammate's line is `{standing} [{champion}] | {winRate} | KDA {kda} | form {score}`. The enemy lines open
with the enemy's side and "winer rating", for example "[Enemy · Red side] winer rating"; your
team's with "[My team · Blue side] winer rating". In a game the champions are settled and are how
players are told apart, so the defaults name the champion alone, or the name where the champion is
not known; `{name}` and `{seat}` are still there, `{seat}` being the player's place in their team's
list.

## Roast titles

Turning off **Settings › Rating › Roast titles** hides both kinds below, and leaves `{title}` empty
in the callout.

### Recent-form titles (game analysis and callout)

Five games at least. A title leans the way the tier beside it does: a tier above the middle only
ever gets praise, only one below it a roast, and the middle tier one about how the player plays. The
middle is the one the in-game callout uses, above.

Kills, deaths and assists a game are set against the same mode's average player: an ARAM game holds
about twice the Rift's kills and deaths and over three times its assists, so ten deaths are a lot on
the Rift and ordinary in ARAM. The averages come from the games WeGame scored that the game score was
fitted on: 5.1 / 5.2 / 7.5 a player a game on Summoner's Rift, 11.1 / 11.1 / 25.6 in both ARAMs.
"×1.3" below means 1.3 times that average. Modes without an average (Arena and others) take no part
in these, and they are read only from five such games or more.

The first rule of the tier's own that holds; failing those, the rules for any tier, then how steady
the games were; failing those too, the last row. "Swing" is how far the game scores scatter around
the player's own average, against an ordinary player's (the measured players' median), from 8 scored
games.

| Tier             | Rule                                              | Title                                                      |
| ---------------- | ------------------------------------------------- | ---------------------------------------------------------- |
| above the middle | three wins in a row or more                       | Patch Champion                                             |
| above the middle | deaths ×0.65 or fewer                             | Rift Immortal                                              |
| above the middle | kills ×1.35 or more                               | Kill Collector                                             |
| above the middle | assists ×1.3 or more                              | Teamfight Engine                                           |
| above the middle | two games in three won, over 8 or more            | Serial Winner                                              |
| the middle       | three wins in a row or more                       | Patch Champion                                             |
| the middle       | three losses in a row or more                     | Ranked Philanthropist                                      |
| below the middle | three losses in a row or more                     | Ranked Philanthropist                                      |
| below the middle | kills ×0.55 or fewer, deaths ×1.1 or more         | Esports Bodhisattva                                        |
| below the middle | deaths ×1.3 or more                               | Grey-screen Regular                                        |
| below the middle | kills and assists both ×0.65 or fewer             | Teamfight Spectator                                        |
| below the middle | a third of the games won or fewer, over 8 or more | Rift Tourist                                               |
| any tier         | kills and deaths both ×1.2 or more                | One-for-one Trader                                         |
| any tier         | assists ×1.2 or more, kills ×0.85 or fewer        | Rift Philanthropist                                        |
| above the middle | swing 0.8 times an ordinary player's or less      | Rock Solid                                                 |
| at or below it   | swing 1.15 times an ordinary player's or more     | Slot Machine                                               |
| none of those    | above / at / below the middle                     | Reliable Teammate / Business as Usual / Along for the Ride |

### Game titles (scoreboard)

Damage share, damage taken and gold are counted within the team. Kills, deaths and assists have bars
per mode, the ARAMs' in brackets, set where as few players reach them: ten deaths or more are one
Rift player in twelve but two ARAM players in three, and only eighteen are as rare there. The first
rule that holds:

| Rule                                                                                      | Title               |
| ----------------------------------------------------------------------------------------- | ------------------- |
| at most 1 kill (ARAM 4), at least 8 deaths (15)                                           | Esports Bodhisattva |
| no deaths (at most 3), at least 10 kills plus assists (35)                                | Rift Immortal       |
| bottom carry (bottom lane, damage share 18% or more), at most 2 deaths, 25% of the damage | Alive Means Damage  |
| bottom carry, at least 9 deaths                                                           | De-carry            |
| lost, with 30% of the damage or more                                                      | The Dean            |
| won, with under 12% of the damage                                                         | Missing Piece       |
| at least 10 kills and 10 deaths (18 and 18)                                               | One-for-one Trader  |
| at least 10 deaths (18)                                                                   | Grey-screen Regular |
| KDA 5 or more, under 15% of the damage                                                    | KDA Keeper          |
| 30% of the team's damage taken or more                                                    | Human Turret        |
| 24% of the team's gold or more, under 17% of the damage                                   | Rift Banker         |
| 17% of the team's gold or less, 25% of the damage or more                                 | Self-made Carry     |
| at least 10 assists (31), three times the kills                                           | Rift Philanthropist |
| kill participation under 35%                                                              | Solo Player         |

Remakes and teams of one earn no title.

## Game score (0–10) and grade

Each part is compared with the game's average player and counts for at most three of them; a part
whose game average is below 1 is left out (vision in ARAM, for example).

WeGame does not publish how it scores. winer's weights were fitted on games WeGame scored, so that
winer's MVP and SVP are WeGame's as often as possible: in 126 Summoner's Rift games (ranked and
normal) the MVP matches in 85% and the SVP in 83%; in 139 Hextech ARAM games, 80% and 73%. With the
earlier single set of weights for every mode these were 68% / 58% and 68% / 56%. WeGame is said to
compare a player with others on the same champion, which one game cannot show; most of the rest is
there.

The two kinds of game weigh differently: a map with lanes counts farming and vision; in ARAM nobody
farms, and gold says more.

| Part                                             | Summoner's Rift | ARAM and Hextech ARAM |
| ------------------------------------------------ | --------------- | --------------------- |
| gold                                             | 0.36            | 0.34                  |
| survival (`(average deaths + 1) / (deaths + 1)`) | 0.19            | 0.22                  |
| kills                                            | 0.12            | 0.09                  |
| assists                                          | 0.09            | 0.09                  |
| damage to champions                              | 0.09            | 0.10                  |
| damage taken                                     | 0.05            | 0.09                  |
| minions and monsters                             | 0.06            | —                     |
| vision                                           | 0.04            | —                     |
| crowd control                                    | —               | 0.01                  |

In ARAM the champion is random, and the same numbers mean different things for different roles: many
kills and much damage are rarer for a tank than for a marksman, while taking damage is its job. So
ARAM weighs each line with its champion's first role in the client, and divides by the ARAM column's
total above, which keeps an ordinary player of any role around 6.0:

| Role     | Kills | Assists | Damage | Taken | Gold | Crowd control | Survival |
| -------- | ----- | ------- | ------ | ----- | ---- | ------------- | -------- |
| Tank     | 0.12  | 0.09    | 0.11   | 0.06  | 0.35 | —             | 0.21     |
| Support  | 0.14  | 0.09    | 0.13   | 0.10  | 0.35 | —             | 0.20     |
| Mage     | 0.10  | 0.09    | 0.09   | 0.10  | 0.34 | —             | 0.21     |
| Assassin | 0.07  | 0.08    | 0.09   | 0.09  | 0.34 | 0.02          | 0.22     |
| Marksman | 0.07  | 0.08    | 0.09   | 0.11  | 0.33 | 0.02          | 0.22     |
| Fighter  | 0.08  | 0.08    | 0.10   | 0.08  | 0.34 | 0.01          | 0.22     |

A champion the client gives no role uses the ARAM column above. The same on Summoner's Rift brought
no gain that would show, so the Rift weighs no role. Arena, URF and the other modes use the
Summoner's Rift column.

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
