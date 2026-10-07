# Automation

Everything on the **Automation** page is off until you turn it on. Each item's **Modes** decide
where it acts: Ranked, Normal, ARAM, Hextech ARAM, Arena and other modes. Only the modes an item can
act in are offered: the two ARAMs have no pick or ban phase, and only they have a bench.

**By mode** at the top shows only what one mode can use and says in a sentence what will happen
there. The automation switches on the overview list their modes too.

## Match found

- **Auto-accept matches**: accepts the match for you. **Wait before accepting** leaves a few
  seconds in which you can still decline yourself.

## Champion pick and ban

- **Auto-pick a champion**: on your turn, picks the first champion in the **Champion order** that
  can be picked. The order has an "Any" list and one per position; with a position assigned, its own
  list comes first, then "Any". Banned champions, locked ones, the ones a teammate picked or showed,
  and champions you do not own are skipped. A champion you hovered yourself is the one used.
- **Pick mode**: **Lock in** locks the pick; **Hover only** puts the champion up once and leaves the
  rest to you.
- **Show an intent while planning**: shows your pick to the team during the planning phase.
- **When autofilled, use only that lane's list**: on by default. Sent to a lane you did not ask for
  in the lobby, the pick comes from that lane's own list only, never from "Any"; with no list for
  that lane, nothing is picked. FILL asked for, no lane asked for, or a mode without positions never
  counts as autofilled. A champion you hovered yourself is still locked. Your seat in champ select
  says **Autofilled**.
- **Auto-ban a champion**: on your ban turn, bans the first champion on the ban list that can be
  banned; a champion a teammate picked or showed is never banned. A champion you chose to ban
  yourself is the one banned.

## Runes and summoner spells

- **Set up runes and summoner spells**: once your champion is locked in, sets up the runes and
  summoner spells you last played it with in this mode. The two ARAMs have no lock-in, so it happens
  each time your champion changes (a bench swap, a reroll). Each champion is set up once per champ
  select, so a change you make afterwards stands; a champion swapped away and back is set up again.
  Arena has no rune page and gives everyone the same spells, so it is not one of the modes.
- **What is remembered**: when a game starts, the rune page and the two summoner spells you go in
  with, per champion and mode, Ranked and Normal apart, even if you changed nothing. They stay on
  this computer.
- **Use the client's recommendation** (on by default): with nothing remembered for the champion in
  this mode, uses the client's own recommended page and spells, for your assigned position on the
  Rift and for no position in the ARAMs.
- **The rune page**: winer uses one page, named after the champion ("winer · Jhin" for Jhin), and
  rewrites it each time. Without one it creates it while there is room; without room it only writes
  over the temporary page the client made for the champion in hand, never one of your own pages.
  Where neither can take it, the spells are still set and the activity feed says why the runes were
  not.
- **Spells**: a spell you already hold stays on its key: Flash on F stays on F.
- **N remembered** and **Forget them**: how many setups are remembered, and forgetting them all.

Each setup is noted in **Activity** on the overview, for example "Set up the remembered runes and
summoner spells for Jhin"; a failure says why. Nothing is posted to chat.

## Item sets

- **Write item sets** (experimental): once your champion is locked in, writes the items of the
  [build](/en/guide/live#builds) as the champion's item set: starting items, boots, core items and
  late options, each named with the source and patch. Off by default and not recommended yet, still an
  experiment: a set written in champ select is in that game's shop, one written after the game
  started only from the next game.
- A champion keeps one of winer's item sets per map, named after the champion: Ranked and Normal
  share Summoner's Rift and the two ARAMs the Howling Abyss. Your own item sets stay as they are.
- Needs **Builds** on in **Settings › General**.
- **Remove winer's item sets**: removes every item set winer wrote, and only those.

## Callout

- **Send in champ select automatically**: sends the callout from
  [the game analysis](/en/guide/live#callout) once every teammate's history has loaded.
- **Send to**: the team chat, or only you.
- **Include myself**: whether your own line is in it.
- **Callout style**: **Rich** (the default) puts the tier's emoji first (👑 for the best tier, 💀 for
  the worst) and adds the title and the quip; **Compact** gives each player one short line, seat,
  tier, win rate, KDA, form and name in the same order, to compare at a glance. Emoji show in the
  client's chat only; lines typed in a game have none.
- **Rating scheme**: the tier names come from **Settings › Rating**; **Open settings** goes there.
- **Opening line** and **Line template**: the first line is always the side and "winer rating", and
  the opening line follows them (left blank, nothing does). The template takes these placeholders:
  `{emoji}` the tier's emoji, `{standing}` the tier, `{seat}` the place in your team's list in champ
  select (P1 to P5, from the top), `{name}`, `{champion}`, `{games}`, `{winRate}`, `{kda}`, `{score}`
  the recent-form score, `{title}` and `{quip}`. Blank uses the chosen style's default, which names
  the seat and the player rather than a champion that can still change; a live preview shows the
  result. The Chinese default follows Sona's seat, tier and data columns and omits free-form names,
  titles and quips so the client filter cannot join them (see [How rating works](/en/rating#callout)).
  A custom template may still use every placeholder.
- **Shortcut to send the callout**: none by default. Once set, pressing it in champ select posts the
  callout to the team chat, as **Send to team** on the Live game page does; in a game, with
  **In-game sending** on, it types the in-game lines chosen below into the game's team chat. It
  works while winer's window is hidden, and what it did is noted in **Activity** on the overview. It
  is set the way the shortcut that brings up winer is (see [Settings](/en/guide/settings)), and the
  two cannot share a combination.

### Lines in game

The game's own chat has no API in the client, so the only way in is to type like a player. With
**In-game sending** on (it is off by default), pressing the callout's shortcut in a game makes winer
type on the keyboard's behalf: `Enter` opens the team chat, a line goes in, `Enter` sends it, line
by line, and a line too long for one message goes out as several.

- It types only while the game's window is already in front; winer never brings the game there.
  The moment the game's window leaves the foreground, typing stops, and a half-typed line stays in
  the game's chat box unsent.
- It waits for every key to come up first: `Enter` with `Shift` held opens the chat to everyone,
  and a key held down repeats into the line.
- Close the chat box before pressing the shortcut: with it open, the first `Enter` sends whatever it
  already holds.
- Typing takes a few seconds, and keys you press meanwhile end up in the line being typed.
- When the game runs as administrator, winer has to as well to type into it: Windows does not let a
  program send key presses to a window with more rights than its own. When the client runs as
  administrator, winer asks to restart that way as it connects anyway.
- Key presses another program sends to the game are third-party input and may break the game's
  terms of service; you use it at your own risk.

**Whose lines to type** decides what one press types:

- **Enemies** (the default): the enemy to watch and the one to go after, three lines at most. Your
  teammates heard about themselves in champ select, and every line typed is one more you wait
  through.
- **My team**: one line per teammate, best first, as in champ select; **Include myself** applies
  here too.
- **Both**: the enemy lines, then your team's.

Each line typed holds your keyboard for about a second, so one press types six lines at most: your
team's first line and five teammates fit exactly; with **Both**, the teammates that do not fit (the
lowest tiers) are left out.

The two enemies are chosen this way:

- **Enemy to watch**: the best rated of the enemies above the middle of the scheme.
- **Enemy to go after**: the worst rated of the enemies below it.

A scheme that ranks the team splits around its middle tier: in Rift five the first two tiers are
above the middle and the last two below; for Rift grades, B and C are the middle. Without such an
enemy the line is left out, and with neither the enemy lines say nothing. Players whose history is
hidden, did not load or holds no games take no part. Modes without sides, such as Arena, have no
in-game lines.

In the game the champions are settled and are how players tell each other apart, so the defaults
name the champion alone (in 【】 in Chinese), without name or seat; where the champion is not known,
the name stands in. All three lines can be rewritten (**Enemy to watch**, **Enemy to go after** and
**Each teammate**) with the template's placeholders; `{champion}` is the champion the player plays
and `{seat}` their place in their team's list. With **Both**, one press types, for example:

```text
[Enemy · Red side] winer rating
Watch Kha'Zix: Rift Demigod, 65% in 20 games, KDA 4.6
Go after Yasuo: Pure Workhorse, 35% in 20 games
[My team · Blue side] winer rating
Rift Demigod: Ahri, 60% in 20 games, KDA 4.1, form 7.4 [Patch Champion], the other team is filing a boosting report
Human Turret: Lee Sin, 55% in 20 games, KDA 3.6, form 6.8 [Reliable Teammate], absurdly steady: carries and survives
```

In the compact style a teammate's line reads "Human Turret [Lee Sin] | 55% | KDA 3.6 | form 6.8".

A live preview shows what one press types, with your own recent form.

## After the game

- **Return to the lobby**: goes back to the lobby when the game ends, ready to queue again.

## Bench

- **Take wishlist champions**: in ARAM and Hextech ARAM, takes a bench champion that ranks above the
  one you hold on the wishlist, at once and without the cooldown. The **Wishlist** holds up to 20
  champions, the earlier the better; while you hold a champion that is not on it, any wishlist
  champion is better.
