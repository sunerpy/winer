# FAQ

## winer keeps showing "Not connected"

winer can only connect to a client that is signed in. Make sure the client is signed in and in the
lobby, then look at the status bar at the bottom of the window:

- It says the client runs as administrator: the Tencent client is started that way by WeGame; choose
  **Restart as administrator**.
- Anything else: restart the client once, or choose **Restart the client UI** on the **Tools** page.
  If that does not help, **Settings › About** opens the log folder, and the log says why the
  connection failed.

## Why administrator rights

winer reads the client's local API address and credentials from the client's process. When the
client runs as administrator, Windows does not let a program without those rights read it, so winer
needs them too. When the client runs without them, so can winer.

## Can it get my account banned

winer uses only the API the client opens on your computer (the LCU), the same one the client's own
interface uses. It does not read the game's memory, change game files or act in a running game.
Even so, no third-party tool can promise zero risk; judge for yourself.

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
