# In-client

The client plugin brings part of winer into the League client's own interface. It needs
[Pengu Loader](https://github.com/PenguLoader/PenguLoader), an open-source tool that loads plugins
into the client's interface, which you install yourself.

## Install the plugin

1. Install Pengu Loader and activate it for the client, as its instructions say.
2. Open winer's **In-client** page. winer finds Pengu Loader by itself; if it does not, choose
   **Detect again**, or enter its folder at the bottom of the page (for example `C:\Pengu Loader`).
3. Choose **Install plugin**, then **Reload the client UI**. Reloading restarts only the client's
   interface process; your sign-in and a running game are not affected.

The plugin ships with winer and has winer's version. After winer updates, this page offers
**Update plugin**.

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

## Remove it

Choose **Remove plugin** on the **In-client** page, then reload the client UI.
