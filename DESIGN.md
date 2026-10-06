# winer design system

The desktop window is a task tool people keep open beside the client for hours: dense, calm,
legible at a glance. Its visual language is a neutral system (hairline cards, mono readouts, status
lamps, one accent) with one addition that gives winer its own identity — the **Hextech** theme,
navy and gold after the League client itself — and the game's own imagery (champion, item and
profile icons) as the only pictures on screen.

Every value below is a token in `app/frontend/src/styles/tokens.css`. A literal colour, radius or
shadow in a component is a defect. The in-client plugin reuses the palette by value (it renders
inside the client's Chromium 108, where `color-mix()` and `oklch()` do not exist), so a token change
here is mirrored in `plugin/src/style.css`.

## Mark

A gold hexagon with a W cut out of it: navy `#0f1a26`, gold `#e3bd66` (a `#f7dc92` → `#c08e34`
gradient at 64px and up). `scripts/brand/icons.py` draws it natively at every size it ships in, so no
size is a downscale of another: `icon.ico` (16–256px, the 32px layer first), the PNGs, the masters
`app-icon.svg` and `tray-icon.svg`, and one tray glyph per display scale (16–48px). From 32px it sits
on the navy plate; below, it has no plate, fills the canvas with its vertical edges on whole pixels
and carries a navy outline, which is what keeps it visible on a light taskbar (gold on `#f3f3f3` is
1.6:1). The window's `Logo` draws the same two shapes.

## Themes

`data-theme` on `<html>` selects one of four palettes; `data-mode` (`light` | `dark`) is derived
from it and selects the accent's text tone. `System` follows the OS between Light and Dark.

| token           | Light 明亮 | Dark 暗黑 | Graphite 石墨 | Hextech 海克斯 (default) |
| --------------- | ---------- | --------- | ------------- | ------------------------ |
| `canvas`        | #f2f3f6    | #0c0d11   | #22252b       | #0a1017                  |
| `surface`       | #ffffff    | #14161b   | #2a2e35       | #101923                  |
| `inset`         | #f7f8fa    | #1c1f26   | #333840       | #162230                  |
| `inset2`        | #eef0f3    | #22252d   | #3a4048       | #1b2939                  |
| `border`        | #e1e4ea    | #2a2e37   | #414750       | #223142                  |
| `border-strong` | #d2d7df    | #363b45   | #4c535d       | #2e4157                  |
| `fg`            | #1b1d22    | #f1f2f4   | #eceef1       | #ece6d6                  |
| `fg-muted`      | #5f6570    | #9aa1ac   | #a5acb6       | #9ba7b3                  |
| `fg-subtle`     | #7d838d    | #737a86   | #868e99       | #748190                  |
| `nav`           | #eceef2    | #111317   | #262a30       | #0d151f                  |
| `nav-active`    | #e1e5eb    | #22252d   | #363b43       | #19263a                  |
| default accent  | blue       | blue      | blue          | gold                     |

Surfaces are three levels apart (canvas recessed, surface raised, inset sunk), so a wall of cards
still separates without shadows. Borders are opaque: a translucent border vanishes exactly at a
card edge in dark themes.

Accents (`data-accent`, `default` = the theme's own): **gold** #c8aa6e, **blue** #339cff,
**teal** #0ac8b9, **green** #04b84c, **orange** #fb6a22, **pink** #ff66ad, **purple** #8a45ef.
Each has a fill, a text colour on the fill, and a text tone per mode, all ≥ 4.5:1 on canvas,
surface and inset (checked when the palette was chosen). `accent-soft` is the fill mixed 16% into
the surface.

Semantic colours, per mode: `ok`, `danger`, `warning`, `info`, and the two game results —
`win` / `loss` (green and red, whatever the accent: blue is a side, never a result) with `-soft`
tints for row backgrounds. The map's two sides have their own pair, `side-blue` / `side-red`, used
only as a dot or chip beside the side's name. Groups (friends in one game or party, a premade party)
take `group-1` … `group-6` in turn: colour-blind-safe hues after Okabe and Ito, set per theme and
each ≥ 3:1 on canvas, surface and both insets, drawn as a dot or a 3px stripe and always beside the
group's number, never as text. `best` (amber, with `best-soft`) marks a game's best
value. The game's line-art icons (augments, runes) sit on `glyph-plate`, the game's own dark ground,
in every theme: drawn for it, they vanish on a light surface. Rank tiers have fixed colours (`TIER_COLORS` in `@winer/shared`) used only as a dot or
an emblem stroke next to the tier's name, never as text.

## Type

- UI: Instrument Sans (self-hosted, variable) with the OS CJK face behind it (Microsoft YaHei UI on
  Windows, PingFang SC on macOS). Mono: JetBrains Mono, for numbers, ids and readouts.
- Base size 13px (setting: 12–16), line height 1.5. Steps: 11 eyebrow and meta · 12 secondary ·
  13 body · 14 labels and row titles · 16 section titles · 20 page figures · 28 hero figures.
- Every table cell and every figure uses tabular numerals (global rule on `td`, `th`, `.mono`).
- Eyebrows are mono 11px, tracking 0.08em, `fg-subtle`, in the locale's own words.

## Space, radius, elevation

- 4px grid. Page padding 24px; card padding 16px (12px compact); gaps 8 / 12 / 16.
- Radius: 4 (chips inside rows) · 6 (controls) · 10 (cards) · 14 (dialogs) · pill.
- Elevation is for floating things only: `shadow-pop` (menus, toasts), `shadow-win` (dialogs).
  Cards are flat: surface plus hairline.
- Density `compact` shrinks row height 32 → 28 and card padding 16 → 12; nothing else changes.

## Layout and scroll ownership

```
┌─────────┬──────────────────────────────────────────────┐
│ brand   │ title bar: page title · readout ····· actions │ 40px drag strip, one region
├─────────┼──────────────────────────────────────────────┤
│ nav     │ page content                         ← scrolls│
│ groups  │                                              │
│         │                                              │
│ footer  ├──────────────────────────────────────────────┤
│ entries │ status bar: connection · phase · shortcuts    │ 28px
└─────────┴──────────────────────────────────────────────┘
```

- Shell primitive: `fixed-sidenav-shell`. The sidebar is full height, 224px or a 56px icon rail;
  its 40px brand row and the title bar form one continuous `data-tauri-drag-region="deep"` strip,
  with the window controls (44×40) at its right end on Windows. On macOS the brand row clears the
  traffic lights (80px).
- **Only the page content scrolls.** The title bar, status bar and sidebar never do; the sidebar's
  nav list scrolls on its own only if it overflows. Every flex/grid child on the way to a scroller
  has `min-h-0`; grid tracks are `minmax(0, 1fr)`.
- Page body: `max-w-[1280px]` centred, cards in a 12-column grid that collapses to one column below
  1100px of content width (container queries, not viewport queries).
- Window minimum 1000×660, so there is no mobile layout to maintain.

## Navigation

Groups and entries (glyph + label; the rail keeps the label as name and tooltip):

- 工作台 Workbench: 概览 Overview · 对局 Live game · 战绩 History
- 助手 Assistant: 自动化 Automation · 工具 Tools · 客户端增强 In-client

Footer entries: the theme switch (opens a menu) and 设置 Settings (opens a dialog; carries
`aria-haspopup="dialog"`, never `aria-current`). Collapse lives in the brand row, not the footer.

The settings dialog's sections are 外观, 通用, 评级 and 关于, a vertical tab rail on the left. 通用
holds the global shortcut that summons the window: its keys as `Kbd` caps, 更改快捷键 (a recorder
that takes the next combination, Esc cancelling, while the shell lets the old one go) and a ✕ that
turns it off; a lamp says 已生效, or the row says in `danger` that the system refused it. 评级
holds the rating scheme as radio cards (name, one-line hint, the tiers best to worst; two columns
from 520px), the custom names as five inputs while 自定义 is chosen, the titles switch, and 评价依据
in an inset block whose 完整说明 opens the site's rating page (`docs/site/rating.md`). 自动化 › 喊话 shows the scheme by
name with 去设置, which opens this section.

## Components

Primitives (`app/frontend/src/ui`), each with its states:

| primitive    | anatomy                                                                          | states                                                                                                                                 |
| ------------ | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Button       | 32px (28 small), radius 6, label + optional glyph                                | primary · outline · ghost · danger · link; hover, focus-visible ring, active, disabled (`not-allowed`), loading (spinner, keeps width) |
| IconButton   | 28/24px square, glyph 16/14, required label                                      | as Button; hover background is `fg/8`, never a surface token                                                                           |
| Card / Panel | surface, hairline, radius 10; Panel adds an eyebrow header row with a right slot | interactive cards darken their border on hover                                                                                         |
| Badge        | 20px, radius 4, label + optional readout                                         | neutral · accent · ok · danger · warning · info · win · loss                                                                           |
| Lamp         | 6/8px dot                                                                        | ok · warn · danger · idle (ring) · off; `pulse` while something is in progress                                                         |
| Toggle       | 32×19 switch, accent when on                                                     | on, off, disabled; label click flips it                                                                                                |
| Segmented    | inset track, raised selection                                                    | selected, hover, disabled; none selected while the state is one no option stands for                                                   |
| Input        | 32px, radius 6, optional leading glyph                                           | focus ring, invalid (danger border + message), disabled                                                                                |
| CommitInput  | an Input that saves on blur or Enter, never per key                              | commits only when the text changed while focused; a saved value comes back normalized                                                  |
| Dialog       | sized frame `min(absolute, viewport)`, lifted surface, corner ✕                  | focus trapped (edges only), Esc and scrim close, focus restored by the shell                                                           |
| Popover      | a layer of its own (portal, fixed), anchored to its trigger                      | prefers a side, flips when that side has no room, never leaves the window; closes on an outside pointer or Esc                         |
| Toast        | bottom-right stack, auto-dismiss 4s                                              | ok · danger · info                                                                                                                     |
| Pager        | ‹ · page numbers · ›, mono 12px, the current page in `accent-soft`               | the first, the last known and two either side of the current; a trailing … while more pages may follow; ‹ and › disable at the ends    |
| EmptyState   | glyph, title, one sentence saying what to do, optional action                    |                                                                                                                                        |
| Skeleton     | pulsing inset block shaped like the content it replaces                          |                                                                                                                                        |

Game primitives (`app/frontend/src/game`): `ChampionIcon` (rounded square, hairline ring; initials
while the client is not connected), `ProfileIcon` (circle), `AssetIcon` (items, spells, runes),
`RankBadge` (tier dot + tier name + division + LP), `ResultStrip` (last games as W/L ticks),
`KdaValue`, `WinRate`, `StreakBadge`, `TeamBoard` (a team in champ select or in game), `MatchRow`,
`MatchDetailView` (the scoreboard) and `ChampionList` / `ChampionPoolEditor`.

### Game surfaces

- **Scoreboard.** One grid row per player: player (champion, spells, name, award) · KDA · score
  (the grade's letter under it, its name as the tooltip) · damage (number, team share, bar against
  the game's highest) · taken · KP · gold · CS · items.
  Taken and gold drop out below 860px of scoreboard width (container query). A header row names
  every column; the score header carries the formula as its tooltip. `MVP` is a solid accent chip,
  `SVP` a neutral `fg/15` chip; the MVP's score is drawn in `accent-text`, a score under 4.5 in
  `fg-subtle`. The row of the player whose history is open is `accent-soft`. The game's best
  values (most damage, most taken, most gold, fewest deaths, best score) sit in an amber
  `best-soft` pill that says what it is best at as its tooltip; a stat everyone ties on, a remake
  and a game of one pick out nobody. Dragons and barons are counted only where someone took one.
  While titles are on, the title a line earned sits under the name as a title chip.
- **History.** A row's champion carries its MVP or SVP in the top-left corner, both solid (accent
  and `raised` with a hairline): a translucent chip on artwork cannot be read. Pages of 10, 15, 25
  or 50 games (remembered), cut from what has arrived: a filter
  fills its page from as many requests as it takes. A Pager under the list, the range (第 11–20
  场, and 共 N 场 once the last page is known) beside the filters. Turning a page closes the open
  scoreboard and brings the list's head into view.
- **Team board.** Champ select and the running game show one team at a time in a Panel (对局分析),
  the local player's team first; a Segmented switches to the other where both are known. One row
  per player: champion, name, tier and party badges, rank, recent win rate, KDA and streak on the
  left (a button that opens the player's history; a premade party's badge is led by its group's
  colour), the latest games on the right as 64px tiles:
  champion, K/D/A in mono, the mode's short name, tinted and underlined `win` / `loss` (a remake
  neutral), the queue, result and age as the tooltip. Six tiles show, eight to twelve where the row
  is wider (container queries). The local player's row carries the accent's inset edge.
- **Lobby.** While the client shows the lobby, the live page lists the party as the team board's
  rows: profile icon, name, 你 / 房主 badges and the lanes asked for, rank and recent form, the
  form score in mono, and the latest games as tiles on the right. A row opens the member's history.
- **Friends.** The overview's 好友动态 lists the friends in champ select or in a game, an inset row
  each: profile icon, name, mode · state, an eye where the game can be spectated and the elapsed
  time in mono, ticking by the second. Friends in one game or party carry their group's stripe and
  a neutral badge led by its colour, with the number. A row opens the friend's history. In the
  client, a friend in a game gets the same `mode · time` line under their name in Hextech gold,
  and a group the same stripe; the lobby's members get one line of form above their banner.
- **Sides.** On a map of two sides a team carries its side: the board's switch says 我方 · 红色方,
  a lone team a `side-blue` or `side-red` dot and its name. The callout's first line names the side
  (【蓝色方】) before the opening line. In the client the local player's line starts with the same
  chip, in the map's colours.
- **Modes.** Every automation rule is scoped to kinds of game (排位, 匹配, 极地大乱斗, 海克斯大乱斗,
  斗魂竞技场, 其他模式), and only to those it can act in at all: no picks or bans in the two ARAMs,
  a bench only there. Under each switch a row of pill toggles (`aria-pressed`, `accent-soft` when
  on) holds its modes; a section only some modes have names them in its eyebrow. 按模式查看 at the
  top of 自动化 shows the sections one mode has and says in a sentence what acts there. The
  overview's switches carry their modes as a second line.
- **Tiers.** A seat's tier is a Badge before its other badges, coloured by where the tier sits
  between best and worst (`tierTone` in `@winer/shared`): best → `accent`, good → `win`, middle →
  `neutral`, weak → `warning`, worst → `loss`; three tiers use best, middle and worst. The label is
  the user's word for the tier, led by the grade's letter in mono where the scheme grades (峡谷八档);
  the score is its tooltip. The tier's quip is a third, `fg-subtle` line of the player's cell. The
  plugin draws the same chip, the same colours by value, at the start of each party row's line, the
  score and the quip as its tooltip.
- **Feats.** What a line did that a badge names, computed by the core from the whole game
  (`view::Feat`): its best multikill (双杀, 三杀, 四杀, 五杀), 超神 (eight kills without dying), 一血,
  逃兵 (the server marked the player away), and the game's leads (杀人最多, 输出最高, 推塔最多, 助攻最多,
  金币最多, 承伤最高, 补兵最多; a tie leads together, a lead everyone shares names nobody). Tones:
  multikills climb `feat-double` blue → `feat-triple` violet → `feat-quadra` orange → `feat-penta`
  gold, 超神 is `feat-legend` crimson, 一血 `feat-blood`, the leads the scoreboard's amber `best`, and
  逃兵 `danger`; 五杀 and 超神 alone carry a ring and a sheen of their colour. A history row shows the
  first three as 18px chips (glyph and name; two in the overview's short list) and `+N` for the
  rest; the scoreboard shows every one as an 18px mark beside the augments, a multikill by its count
  in mono, the rest by a Lucide glyph (swords, zap, castle, helping hand, coins, shield, wheat,
  droplet, crown, log-out). The name and what earned it are the tooltip and the screen reader's text.
- **Titles.** A roast title (版本答案, 院长) is a chip of its own after the tier: a dashed
  `border-strong` outline, `fg-muted` text and a `warning` flame, so it never reads as another
  tier; what earned it is the tooltip where it is known. The plugin draws it as a dashed outline in
  the client's amber. 设置 › 评级 turns titles off everywhere.
- **Callout.** A Panel beside the team in one-team modes (ARAM) and under both teams otherwise:
  the lines exactly as they will be sent, in an inset block, then 发送到队伍 (accent) and
  仅自己可见 (outline). An accent badge in the header says when automatic sending is on.
- **Augments.** Hextech ARAM and Arena lines show their augments instead of runes: the client's
  icon in a 1.5px ring of its rarity (`--rarity-silver`, `--rarity-gold`, `--rarity-prismatic`,
  the game's own colours in every theme), name, rarity and description as the tooltip. On the
  scoreboard they sit in a strip under the player's name.
- **Bench.** A strip under the champ-select header, shown only in modes with a bench: each
  champion is a 36px button that swaps at once; wishlist champions carry a small accent star;
  the reroll button with its count sits at the right end.

Icons: one family, Lucide (24-unit grid, stroke 2), rendered at 14 / 16 / 20.

## Copy

zh-CN is the source language; en follows key-for-key (a compile-time check). Labels are short and
specific (自动接受对局, not 开启功能). Empty and error states say what to do next. Numbers carry their
unit in `fg-subtle` (`56 LP`, `1,240 ms`).

## Motion

150ms ease-out for hover and colour; 200ms for panels and dialogs; no ambient motion. Everything
honours `prefers-reduced-motion` and the in-app 减少动态 setting.

## Accessibility

- Every interactive element shows `cursor: pointer`; disabled shows `not-allowed` (Tailwind v4's
  preflight dropped the former, so it is a base rule here).
- `:focus-visible` draws a 2px `accent-text` ring.
- Colour never carries meaning alone: results also say 胜/负, tiers carry their name, lamps their label.
- Text selection is off for chrome (buttons, labels, nav), on for data.

## Accepted debt

- The Windows 11 Snap Layouts flyout is lost to the self-drawn title bar
  (`docs/accepted-tradeoffs.md` T-001).
- Champion and rank imagery comes from the connected client; with no client the window shows
  initials and tier names. The one thing fetched from the internet is what augments do (ARAM.GG,
  a third-party host, switchable in 设置 › 通用); it can lag a patch, and some values arrive as `?`.
