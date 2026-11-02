# FAQ

## winer keeps showing "Not connected"

winer can only connect to a client that is signed in. Make sure the client is signed in and in the
lobby, then look at the status bar at the bottom of the window:

- It says the client runs as administrator: the Tencent client is started that way by WeGame, and
  winer asks to restart as administrator by itself; if you chose **No** in the User Account Control
  prompt, choose **Restart as administrator** to ask again.
- Anything else: restart the client once, or choose **Restart the client UI** on the **Tools** page.
  If that does not help, **Settings › About** opens the log folder, and the log says why the
  connection failed.

## Why administrator rights

winer reads the client's local API address and credentials from the client's process. When the
client runs as administrator, Windows does not let a program without those rights read it, so winer
needs them too, and asks for them when it finds out. When the client runs without them, winer
connects to it without them as well.

The first activation of the Pengu Loader winer ships needs administrator rights once too: it creates
a link in the client folder, and Windows lets only an administrator create one. When winer is not
running as administrator it asks to restart that way by itself; once the link exists, winer keeps
using it without administrator rights. See
[In-client](/en/guide/client#the-first-activation-needs-administrator-rights).

Typing a callout or a comeback into a game needs them too: a game started from WeGame runs as
administrator, and Windows does not let a program without those rights send it key presses. With
**In-game sending** on and a shortcut set, winer asks to restart as administrator when it connects
to such a client; see [Lines in game](/en/guide/automation#lines-in-game).

## Can it get my account banned

winer uses only the API the client opens on your computer (the LCU), the same one the client's own
interface uses. It does not read the game's memory, change game files or act in a running game.
Even so, no third-party tool can promise zero risk; judge for yourself.

The one exception is **In-game sending**, off by default, which the callout and the comebacks each
have: with it on, pressing their shortcut in a game makes winer send key presses to the game's window
to type into its chat. That is third-party input and may break the game's terms of service; leave it
off if you would rather not take that risk. Comeback lines, the downloaded online library above all,
may break the game's chat rules, and a report can get the account muted or banned.

## The history stops at 20 games

The Tencent client's own history API returns the latest 20 games only. winer asks the region's
match-history server instead; when that server cannot be reached it falls back to the client's 20
games and says so under the list. Try again later, or after restarting the client.

## Why some players have no tier

A player whose history is hidden or failed to load has no recent form, so they get no tier and are
left out of the callout.

## The AFK feat shows on some games only

The AFK mark comes from the match-history server; the client's own 20 games do not carry it, so it
does not show when the history falls back to them.

## SmartScreen or antivirus blocks the installer

The installer has no code-signing certificate, so SmartScreen shows "Windows protected your PC";
choose **More info › Run anyway**. You can check the download against the release's `SHA256SUMS`;
see [Install](/en/guide/install#verify-a-download).

## The sent callout differs from the preview

The preview shows every tier with your own recent form; in champ select each line uses that
teammate's numbers. The format, the opening line and the tier names are the same as in the preview.

## How to report a problem

Open an issue on [GitHub](https://github.com/sunerpy/winer/issues). Attaching the day's log from
**Settings › About › Open the log folder** makes it quicker to find; logs never contain the client's
credentials.

There is also a QQ group, winer交流群 (number 1109319256), for reports and questions: open the
[invitation link](https://qm.qq.com/q/DV1thxtEWe) or scan the code below in QQ. winer's
**Settings › About › Community** opens it as well.

<img src="/community/qq-group.png" width="180" alt="The QR code of the QQ group winer交流群, number 1109319256" />
