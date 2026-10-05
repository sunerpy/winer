# Match history and feats

## Whose history

The **History** page opens on your own games. Type `name#tag` (for example `Light in the Dark#10003`)
to look up anyone; choosing a name in the game analysis or on a scoreboard opens that player's
history too.

## Pages and filters

Games come in pages of 10, 15, 25 or 50, and the choice is remembered. Filter by **All, Ranked,
Normal, ARAM, Other** at the top; a filtered page is still full.

The Tencent client's own history API returns the latest 20 games only. winer asks the region's
match-history server instead, with the client's own sign-in, so it pages through the whole history;
when that server cannot be reached it falls back to the client's 20 games and says so under the
list.

## Each game

Each game in the list shows its result, queue, time, length, kills/deaths/assists, CS and damage,
summoner spells and runes (augments in Hextech ARAM and Arena), and items.

- **MVP / SVP**: on the champion's top-left corner. The MVP is the best score on the winning side,
  the SVP the best on the losing side.
- **Feats**: up to three of the game's feats after its time, the rest behind "+N", which lists them
  on hover.

| Feat                               | When                                                |
| ---------------------------------- | --------------------------------------------------- |
| Double, triple, quadra, penta kill | the game's best multikill                           |
| Legendary                          | eight kills or more without dying                   |
| First blood                        | took the game's first kill                          |
| Most kills, most assists           | the most kills or assists in the game               |
| Most damage, most damage taken     | the most damage to champions, or taken, in the game |
| Most gold, most CS                 | the most gold, or minions and monsters, in the game |
| Most towers                        | destroyed the most towers in the game               |
| AFK                                | the game marked the player away: left or idled      |

Multikills change colour as they climb, and a penta kill and Legendary carry a ring; the game's
leads use the scoreboard's amber; AFK is red. The full rules are in
[How rating works](/en/rating#feats).

## Scoreboard

Open a game for its scoreboard, one table per team:

- Each player's champion, summoner spells, name and MVP or SVP; under the name, augments, feat
  marks and the game's roast title.
- Kills/deaths/assists and KDA, the **score** (0–10) with the game's grade (S+ to F) under it,
  damage and damage share, damage taken, kill participation, gold, CS and items.
- The game's most damage, most damage taken, most gold, fewest deaths and best score are picked out
  in amber.
- The row of the player whose history you are viewing has the accent's tint.

Scores, grades and titles are explained in [How rating works](/en/rating). Roast titles can be
turned off in **Settings › Rating**.
