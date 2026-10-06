# In-client

The client plugin brings part of winer into the League client's own interface. It is loaded by
[Pengu Loader](https://github.com/PenguLoader/PenguLoader), an open-source tool that loads plugins
into the client's interface. winer ships it; there is nothing else to install.

## Set up by itself

The first time winer connects to the client, it:

1. puts the Pengu Loader it ships in winer's own data folder and activates it with a `version.dll`
   link in the client folder, the way Pengu Loader activates itself. Windows lets only an
   administrator create such a link, so the first activation needs administrator rights once (see
   the next section);
2. installs winer's plugin, which then updates with winer;
3. reloads the client UI once, after the client has finished signing in and while it is idle (no
   lobby, queue, champ select or game), so the plugin starts at once, and brings the client's window
   back up afterwards; if the client does not become idle within two minutes, the plugin starts with
   the client's next launch. Reloading restarts only the client's interface process; your sign-in is
   not affected.

Pengu Loader's own welcome window and start-up notices do not appear. If Pengu Loader is already
installed, winer keeps using yours and only installs the plugin. If the client folder already has a
different `version.dll`, winer leaves it alone and the **In-client** page says so.

## The first activation needs administrator rights

Activating Pengu Loader creates a link in the client folder, and Windows lets only a program running
as administrator create one. So the first time, if winer is not running as administrator, it asks
to restart as administrator by itself: choose **Yes** in the User Account Control prompt, and the
activation completes once winer has restarted and connected to the client again.

If you chose **No**, the **In-client** page says that administrator rights are needed, and
**Restart as administrator** asks again. Once the link exists, winer keeps using it when it runs
without administrator rights and does not ask again.

When the client itself runs as administrator (the Tencent client does when WeGame starts it), winer
has already asked for those rights to connect to it, so the activation asks nothing more.

## What you see in the client

- **Teammate panel in champ select**: a line under each teammate's name in champ select: tier and
  title, rank, recent win rate, KDA and streak; your own line also carries your side of the map.
  Where the client's team list cannot be found, a compact panel shows the same. A click on the line,
  or on a teammate in the panel, shows their history: see **Recent games inside the client** below.
- **Instant bench swaps in the client**: in ARAM, a click on a bench champion in the client swaps it
  in without the client's cooldown.
- **Friends' games in the friends list**: in the client's friends list on the right, a friend in a
  game gets a line under their name with the mode and how long the game has run, counted by the
  second; friends in one game or one party carry a stripe of the same colour on the left.
- **Lobby members' form**: in a lobby, recent win rate, KDA and form score above each member's
  banner. A click on that line or on the member's picture shows their history: see **Recent games
  inside the client** below. Where the client's member cards cannot be found, a compact panel shows
  the same, and its members open their history too.
- **Recent games inside the client**: a click on a player in the lobby or in champ select opens a
  card beside them in the client, with no need to switch to winer's window. At the top are their
  rank, recent win rate, KDA, form score and streak (in champ select also their tier and title);
  below are their last 10 games (without custom games while the History page hides them), each with
  the champion, the result, the mode, kills / deaths / assists and how long ago it was. When the
  games cannot be read, the card says why and offers **Retry**. **Full history in winer** at the
  bottom opens winer's window. `Esc`, the ✕ at the top right or a second click on the same player
  closes the card; it also closes when the client leaves the lobby or champ select, or shows the
  match-found prompt. With this option off, a click on a player opens their history in winer's
  window instead.
- **Hide home-page promotions**: hides the esports pop-up and puts a short note in place of the
  news and events on the client's home page. **Show for now** on the note brings them back until the
  client restarts; with the option off, the home page shows as usual.

Each has its own switch under **In-client › In-client features** and takes effect at once.

## The plugin needs winer running

The plugin's data comes from winer: it connects only to a port winer opens on your computer, with a
token winer hands it. While winer is not running the plugin shows nothing and the client works as
usual.

## Turn it off

Choose **Turn off the in-client features** on the **In-client** page: winer removes the plugin and
the `version.dll` link it created (a Pengu Loader of your own stays as it is), from the client's
next start. **Turn on the in-client features** brings it back.
