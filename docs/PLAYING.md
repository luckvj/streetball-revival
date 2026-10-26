# Playing with friends

> **Work in progress.** Online play is new and still needs testing. If
> something goes wrong (a dropped connection, a desync, a lobby that never
> fills), please report it on the [Discord](https://discord.com/invite/htu5pdxPQz) or in an issue, saying
> whether you hosted or joined.

Everyone needs two things:

1. **The game.** Download the zip from the repository's Releases page and
   unzip it into a folder of its own. (Or build it: see the README.)
2. **A disc image of their own copy** of the original game: the USA
   PlayStation 2 release, serial SLUS-20651, as a `.iso` file.

Then run `StreetballRevival.exe`, press **Browse**, pick the `.iso` and press
**Play**. The launcher remembers the disc from then on.

## Starting a match

Main Menu, then **P** on the keyboard or **Select** on a controller → **Online**
(the original's Main Menu has six rows and no online entry, so it is a button).

**Hosting (one person does this):**

- Pick **Host 1 vs 1** to play one other person, or **Host 3 vs 3** for up to
  six people, three a side. The computer plays any seat nobody takes.
- Choose the teams, players and court as in a Pick Up game.
- The lobby shows a **code** at the top, like `7QK3-M2X`. Send it to your
  friends.

**Joining (everyone else):**

- **Same house or same Wi-Fi:** the host's match shows up by itself on the
  Online page as "Join *name*". Press it.
- **Anywhere else:** pick **Join With Code**, type the host's code and press
  Enter.

Everyone presses Enter in the lobby when ready, and the match starts.

## If Windows asks

The first time you host or join, Windows may ask whether to let
`street-game.exe` use the network. Tick both **Private** and **Public** and
press **Allow**.

## If friends can't join over the internet

The host's game asks the router to open the way in (UPnP), which most home
routers allow. The lobby says when it couldn't:

- *"Your router would not open the game"*: turn on UPnP in the router's
  settings, or forward **UDP port 7375** to the hosting PC. Or let someone
  else host.
- *"Only your own network can join"*: the internet provider shares one
  address among many homes, so nobody outside can reach a match hosted
  there. Let a friend on another connection host.

As a last resort, everyone can join the same free VPN (Tailscale or
ZeroTier) and use **Join With Code** with the host's VPN address
(e.g. `100.101.102.103`) instead of a code.

## Good to know

- Everyone uses their own disc image, of the same release (the USA release,
  SLUS-20651): the game plays the match on your own disc's data and only checks
  that everyone's agrees, so another region's disc can't join. The teams,
  court and rules (the Replays option too) come from the host.
- The game never pauses online, but instant replays play as in a match
  alone, everyone's game holding for them at once. Leaving hands your player
  to the computer.
- After a match, everyone can vote for a rematch.
