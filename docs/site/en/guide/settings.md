# Tools and settings

## Tools

The actions on the **Tools** page act on the client directly:

- **Status**: online, away, mobile or invisible, the same as switching it in the client; invisible
  shows as offline to friends. Mobile shows friends a phone icon, but the Tencent client words the
  state 在线分组 ("online group"). A status the client set itself (in game, say) shows as the
  current one.
- **Say 手机在线 in my status message**: a switch that appears while mobile is chosen. With it on
  and no status message of your own, winer sets the message to 手机在线 ("on mobile"), which friends
  see in quotation marks in their friends list and on your hover card; another status takes it
  away, and a message you wrote yourself is left alone. With **Remember my status** on, it comes
  back with the status. Off by default.
- **Status message**: the line friends see under your name; the client puts it in quotation marks.
- **Remember my status**: the client sets you back to online when it starts and after each game.
  With this on, winer puts the status and message chosen here back within a minute of connecting to
  the client and of each game ending. It does so once per change; if the client undoes it three
  times in a row straight away, winer stops and says so in the activity feed. Off by default.
- **Restart the client UI**: restarts only the client's interface process; your sign-in and a
  running game are not affected. Useful when the client's interface hangs, or after a plugin update.

### Profile background

The current background is at the top. Below it are all skins of all champions, owned or not: search
by champion name, title, English name or skin name, filter by champion, or turn on **Owned only**.
Pick one and press **Set as background**. winer then reads the client's profile again. The client
may refuse a skin you do not own; if the background did not change, the page says so, and what the
client shows is what counts.

### Challenge tokens, title and banner

The three challenge tokens on your profile, left to right: each can be any challenge you have
reached a level in, or empty. The title is one of those you have earned. The banner is the client's
default or one you own, and the banner of last season's rank when you had a ranked tier last
season. After **Apply** the page shows the tokens, title and banner the client actually took, and
says so when it did not take all of them.

### Rank disguise

With it on, friends see the rank you choose in the friends list and on your hover card: pick the
queue (solo/duo or flex), the tier and the division; Master and up have no divisions. Only the rank
friends see changes. Your real rank, matchmaking and your own profile in the client stay as they
are. When the client puts the rank back, winer changes it again; turning the switch off brings your
real rank back. Off by default.

### Game settings backup

**Back up** saves the client's general settings (interface, camera, sound, display and so on) and
key bindings as one backup. Ten are kept and the oldest goes first; winer never backs up on its
own. A backup restores the general settings, the key bindings or both, and the client saves them at
once. Restoring works only from the lobby or the home screen, never in champ select or a game.

To take a backup to another computer, use **Show in folder** to find its file, copy it over, and
choose it there with **Import**.

## Settings

Open them with `Ctrl` + `,` or **Settings** at the bottom of the sidebar.

### Appearance

- **Theme**: System, Light, Dark, Graphite and Hextech, the client's own navy and gold.
- **Accent**, **Density** (comfortable or compact), **Font size** and **Reduce motion**.

### General

- **Language**: 简体中文 or English.
- **Keep running in the tray when closed**: on, the close button only hides the window and winer
  keeps working in the tray; off, it quits. A left click on the tray icon opens the window; its right
  click menu toggles auto-accept and quits.
- **Shortcut to bring up winer**: `Alt` + `` ` `` (the key left of 1) unless you change it, from anywhere: with
  the window in front it hides it, otherwise it brings the window to the front, and during a game
  the window stays above the game until it is hidden again or the game ends. Choose **Change the
  shortcut** and press the new combination, which needs `Ctrl`, `Alt` or `Win`; `Esc` cancels. The
  ✕ beside it turns the shortcut off. When another program already holds the combination, the row
  says so: pick another. The game must run borderless or windowed: in exclusive fullscreen, the
  game minimizes as soon as another window takes the focus.
- **Augment descriptions from ARAM.GG**: fetches what Hextech ARAM's augments do from ARAM.GG; off,
  augments show their name and icon only.
- **Builds**: shows the [build panel](/en/guide/live#builds) on the Live game page, from public
  statistics by the Tencent League app, OP.GG and ARAM.GG; off, it is hidden and nothing is fetched.
- **Summoner's Rift data**: Tencent 101 (the Chinese servers) or OP.GG (the world).
- **Start with Windows**: starts in the tray when you sign in to Windows, without opening the window.

### Rating

The rating scheme, custom tiers, roast titles and the basis of every rating: see
[How rating works](/en/rating).

### About

The version, whether winer runs as administrator, the log folder, and **Updates**. winer checks for
updates by itself after it starts and while it runs; this checks again on demand.

**Storage** lists what winer keeps on your PC, how large each is and its limit: the logs, the
window's WebView data, the game settings backups, the in-client components, the settings and
remembered setups, update installers and the cache in memory. Where each lives is in
[Data and privacy](/en/privacy).

- **Clear cache**: removes the old logs (not the one being written), the installers an update left
  behind and the caches in memory; the WebView's cache is in use while the window is open, so it goes
  the next time winer starts. The game settings backups, the settings and the remembered runes and
  spells stay. Afterwards a line under the list says what went, and how large it was.
- Without a cleanup nothing grows past its limit either: logs keep the last 7 days and 50 MB in all,
  the caches in memory let go of what expires, the WebView's page cache holds 32 MB at most, and
  update installers go at the next start.

## Keyboard shortcuts

| Keys             | Does                             |
| ---------------- | -------------------------------- |
| `Ctrl` + `1`…`6` | switches pages                   |
| `Ctrl` + `,`     | opens settings                   |
| `Ctrl` + `B`     | collapses or expands the sidebar |

`Alt` + `` ` `` (changed in **Settings › General**) works everywhere, a game included: it
shows or hides winer. The others work inside winer's window only. The shortcut to send the callout
has no combination by default; it is set in **Automation › Callout**, works everywhere as well, and
is described in [Automation](/en/guide/automation#callout).
