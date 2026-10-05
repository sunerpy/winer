# In-client

The client plugin brings part of winer into the League client's own interface. It is loaded by
[Pengu Loader](https://github.com/PenguLoader/PenguLoader), an open-source tool that loads plugins
into the client's interface. winer ships it; there is nothing else to install.

## Set up by itself

The first time winer connects to the client, it:

1. puts the Pengu Loader it ships in winer's own data folder and activates it with a `version.dll`
   link in the client folder, the way Pengu Loader activates itself. This needs administrator
   rights, which the Tencent client asks of winer anyway;
2. installs winer's plugin, which then updates with winer;
3. reloads the client UI once, while the client is idle (no lobby, queue, champ select or game), so
   the plugin starts at once; otherwise it starts with the client's next launch. Reloading restarts
   only the client's interface process; your sign-in is not affected.

Pengu Loader's own welcome window and start-up notices do not appear. If Pengu Loader is already
installed, winer keeps using yours and only installs the plugin. If the client folder already has a
different `version.dll`, winer leaves it alone and the **In-client** page says so.

## What you see in the client

- **Teammate panel in champ select**: a line under each teammate's name in champ select: tier and
  title, rank, recent win rate, KDA and streak; your own line also carries your side of the map.
  Where the client's team list cannot be found, a compact panel shows the same.
- **Instant bench swaps in the client**: in ARAM, a click on a bench champion in the client swaps it
  in without the client's cooldown.
- **Hide home-page promotions**: hides the event hub and esports pop-ups on the client's home page.

Each has its own switch under **In-client › In-client features** and takes effect at once.

## The plugin needs winer running

The plugin's data comes from winer: it connects only to a port winer opens on your computer, with a
token winer hands it. While winer is not running the plugin shows nothing and the client works as
usual.

## Turn it off

Choose **Turn off the in-client features** on the **In-client** page: winer removes the plugin and
the `version.dll` link it created (a Pengu Loader of your own stays as it is), from the client's
next start. **Turn on the in-client features** brings it back.
