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
- **Auto-ban a champion**: on your ban turn, bans the first champion on the ban list that can be
  banned; a champion a teammate picked or showed is never banned. A champion you chose to ban
  yourself is the one banned.

## Callout

- **Send in champ select automatically**: sends the callout from
  [the game analysis](/en/guide/live#callout) once every teammate's history has loaded.
- **Send to**: the team chat, or only you.
- **Include myself**: whether your own line is in it.
- **Rating scheme**: the tier names come from **Settings › Rating**; **Open settings** goes there.
- **Opening line** and **Line template**: the first line is always the side and "winer rating", and
  the opening line follows them (left blank, nothing does). The template takes these placeholders: `{standing}` the
  tier, `{champion}`, `{name}`, `{games}`, `{winRate}`, `{kda}`, `{score}` the recent-form score,
  `{title}` and `{quip}`. Blank uses the default; a live preview shows the result.

## After the game

- **Return to the lobby**: goes back to the lobby when the game ends, ready to queue again.

## Bench

- **Take wishlist champions**: in ARAM and Hextech ARAM, takes a bench champion that ranks above the
  one you hold on the wishlist, at once and without the cooldown. The **Wishlist** holds up to 20
  champions, the earlier the better; while you hold a champion that is not on it, any wishlist
  champion is better.
