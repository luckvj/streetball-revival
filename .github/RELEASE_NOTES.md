> **Public beta.** Most of the game plays like the original, but expect some bugs and missing pieces. Online play is new and needs testing: please try it with friends and tell us how it goes on the [Discord](https://discord.com/invite/htu5pdxPQz).

A PC rebuild of a 2003 PlayStation 2 three-on-three streetball game, with the online play it never had: up to six people, three a side. **No game data is included**: the game plays from a disc image of **your own copy** of the original.

## Download

Open **Assets** at the bottom of this page and pick your file:

| Your PC | Download | |
|---|---|---|
| **Windows 10 / 11** | **`StreetballRevival-…-windows.exe`** | **The whole game in one file.** Download it, open it, play. |
| Windows, as a folder | `streetball-revival-…-windows.zip` | The same game unzipped into a folder, if you prefer. |
| Linux (x86-64) | `streetball-revival-…-linux.zip` | |

> Ignore the two **Source code** files GitHub adds at the end of the list: they are for developers and hold no game to play.

## What you need

- A disc image (`.iso`) of the original game: the **USA PlayStation 2 release, serial SLUS-20651** (other regions won't work). To make one, put your disc in a PC DVD drive and save it as an `.iso` with any disc-imaging program.
- A graphics card that supports DirectX 12 or Vulkan (most made since about 2015).
- Optional: a controller. PlayStation, Xbox and most other pads work and can be plugged in at any time; the keyboard works too.

## Install and play

1. **Open `StreetballRevival-…-windows.exe`** (put it on your desktop or anywhere you like first).
   - If Windows says *"Windows protected your PC"*, click **More info**, then **Run anyway**. This free project's files aren't signed, so Windows doesn't know them yet.
   - Using a zip instead? Unzip it and open **`StreetballRevival.exe`** at the top of the `Streetball Revival` folder (Linux: `./StreetballRevival`).
2. Press **Browse** and pick your `.iso`, or drag it onto the window. The launcher checks it is the right release and remembers it.
3. Press **Play**.

The first Play takes a few seconds longer: the one-file download unpacks the game into `%LOCALAPPDATA%\Streetball Revival`, and the first launch can show only sky while your PC prepares the graphics. Later launches start faster.

## Controls

| Action | PlayStation | Xbox | Keyboard |
|---|---|---|---|
| Move | Left stick | Left stick | WASD / arrows |
| Shoot (near the rim: dunk) | Circle | B | J / Space |
| Pass (on defence: switch) | Cross | A | K |
| Trick move / steal | Square | X | L |
| Jump: block, rebound | Triangle | Y | I |
| Turbo (hold 1 to 4) | L1 R1 L2 R2 | LB RB LT RT | Q, Shift, R, E |
| Pocket / cancel a gamebreaker | Select | View | P |
| Pause | Start | Menu | Esc / P |

**In menus:** arrows or WASD to move, **Enter** to pick, **Esc** to go back.

**Tricks:** hold turbo buttons while pressing the trick button. The more turbo buttons you hold, the fancier the move. Tricks fill your gamebreaker meter; when it is full, shoot for a gamebreaker.

**Settings:** Main Menu → **Options** → **Adjust Screen** opens PC Settings: full screen or window, the wide picture, vibration, Online, Watch A Game and your created teams.

## Playing online (work in progress)

On the Main Menu press **Select** (**P** on the keyboard) for **Online**.

1. One person picks **Host 1 vs 1** or **Host 3 vs 3** and sends friends the code shown in the lobby (like `7QK3-M2X`). The computer plays any empty seat.
2. Friends on the same network see **Join *name***; anyone else picks **Join With Code**.
3. Everyone needs their own disc image of the same USA release.

If Windows asks to let `street-game.exe` use the network, tick **Private** and **Public** and press **Allow**. If friends can't connect, [HOW TO PLAY ONLINE](https://github.com/luckvj/streetball-revival/blob/main/docs/PLAYING.md) covers routers, UDP port 7375 and the VPN fallback.

## Your save

Progress and the launcher's settings are kept in **`Documents\Streetball Revival`** (`street_save.dat`, `launcher.json`). Back that folder up to keep your progress; unzipping a newer release doesn't touch it.

## Troubleshooting

- **"This disc isn't the USA PlayStation 2 release"** or **"That file isn't a disc image"**: the image must be the USA release, SLUS-20651, saved as a whole-disc `.iso` (not a folder of files, not a compressed `.chd` or `.cso`).
- **"The game's files weren't found or couldn't be unpacked"**: download the release again. From a zip, keep the whole `Streetball Revival` folder together (the launcher starts the game from its `bin` folder).
- **Controller not working**: unplug it, plug it back in and press a button on it while a menu is showing.
- **Linux**: if the files won't start, run `chmod +x StreetballRevival bin/*` in the folder. The game needs ALSA, udev, Wayland or X11 and a Vulkan driver, which most desktops already have.

The zips hold all of this in `START HERE.txt` too.

## Feedback

Found a bug, or something that plays differently from the original? Tell us on the [Discord](https://discord.com/invite/htu5pdxPQz) or [open an issue](https://github.com/luckvj/streetball-revival/issues) with what you did, what you expected and what happened. Please don't attach disc images or game files.

## Made with the help of AI

Much of this rebuild was written with the help of a large language model (LLM), so **some features may be missing** or behave differently from the original. Reports against the original are the best way they get fixed.

---

Unofficial fan project, not affiliated with or endorsed by any rights holder of the original game. Its own code is under the MIT licence; see LEGAL.md for the full notice.
