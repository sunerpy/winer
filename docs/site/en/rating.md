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

- The newest 20 games played against other players among the recent games the client lists. Custom
  games (the practice tool included) and games against the computer are left out and take no place
  among the 20. Which games are against the computer is the client's own queue catalog's word: queues
  in its co-op vs AI category, and queues whose type names the computer as the opponent (Doom Bots,
  for one); the tutorial counts as well. The Tencent client lists its latest 20 to 30 games only, so
  with many of these the count is below 20.
- Remakes among the 20 are shown, and count toward no win, KDA, streak or form.
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

One line per teammate, the best tier first. **Callout style** in **Automation › Callout** picks the
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
- `{seat}` is the teammate's place in your team's list in champ select, counted from the top: P1 to
  P5 (1L to 5L in Chinese). A line names the seat and the player, not the champion: champions can
  still change during champ select, seats do not.
- `{title}` is the recent-form title, empty without one; `{quip}` is the tier's quip. Every tier of
  the built-in schemes has one (Rift five has three per tier in Chinese), picked per player and per
  game: the same throughout one champ select, likely another the next game. Custom names have none.
- The line template and the opening line are edited in **Automation › Callout**, with these
  placeholders: `{emoji}` `{standing}` `{seat}` `{name}` `{champion}` `{games}` `{winRate}` `{kda}`
  `{score}` `{title}` `{quip}`. A template of your own is used in either style. It can still use
  `{champion}`: the champion the teammate has picked or shown when the callout is sent.

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
style a teammate's line is `{standing} {champion} | {winRate} | KDA {kda} | form {score}`. The enemy lines open
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

The first rule of the tier's own that holds; failing those, the rules for any tier; failing those
too, the last row.

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
