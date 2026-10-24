# Roadmap

The goal is the whole of the original game, rebuilt, plus online play, which
the original never had (its disc has no network code at all). It ships as
free, open-source code that reads the game's data from the player's own disc
image at runtime, so the repository never contains game data.

It comes as two programs: `StreetballRevival` (the launcher, which finds
the player's disc) and `street-game` (the game), plus the headless
`street-server` for anyone who wants to host matches.

Status marks: **done**, **partly**, or blank for not started.

Order of work: the whole game first (gameplay, then modes and menus, then
presentation and audio), online last. The simulation is kept deterministic
throughout so that online play can be added without changing it.

## 1. Disc data

| | Status |
|---|---|
| ISO9660, BIG/VIV archives, SSH textures | done |
| EAGL models (courts, hoops, players, ball), VU1 layouts | done |
| Player skeleton, animations (646 of 646 rotation tracks, root motion) | done |
| Database (players, teams, courts, moves, nicknames) | done (cards table unread) |
| Morph targets (league players' faces) | done |
| Locale text (`/LOCALE/*.EUS`): the `c0 fb` container, strings, `\xNN` punctuation | done |
| The 6 `nocomp`/`lowcomp` animation tracks (FnDeltaQuatChan type 11, raw pose type 0) | done |
| Accessory models (pads, braces, bands) | |
| Create-a-player kit assembly | done (the nine "Choose Baller" presets are outfits, `ballers::OUTFITS`) |
| Crowds (`*C.VIV`, `.e3b`) | |
| Hoop shadows (`ArbitraryShadow_Skin`), nets | |
| GS blend modes per render method (glows, flares) | |
| Audio: music, announcer, effects (`SCHl`/ABK, XA ADPCM) | |
| Movies (`.MPC`) | |
| Front-end art (logos `XATEAMS`, portraits `XAPORTS`, menus) | |
| Fonts | |

## 2. Gameplay (all in `street-sim`, so it is online automatically)

| | Status |
|---|---|
| 3 on 3, full court, to 21 win by 2, 1/2 points, shot clock, inbounds | done |
| Jump shots, contests, blocks, steals, passes and interceptions, rebounds | done |
| Dunks (normal and advanced) | partly |
| Full PS2 pad in the input (four turbo buttons, trick button, etc.) | done |
| Turbo meter | done |
| Trick moves, fakeouts, trick counters, advanced tricks | done (the values are the five-level ladder and repeat halving, not per-move constants) |
| Trick points, combos, the Gamebreaker meter | done (meter size: 50,000 trick points, traced) |
| Gamebreakers (level 1 and 2, pocket, cancel) | done (the staged Level 2 sequences play from the disc's scripts) |
| Alley-oops, give 'n go, kick pass, off the heezay, back 2 papa | done |
| Dunk variety by rating and approach, dunk to lay-up, putbacks, lay-ups | done (each move's own script's blend clips) |
| Pump fake, fadeaway, dive for a loose ball, picks, goaltending | done |
| Rule modes: league scoring, old school, dunks only, trick point challenge, gamebreaker wins, back to basics, spot points, shot clock settings | done |
| AI: difficulty levels, uses every move, plays to ratings | done (to be tuned against the original) |
| Tuning against the original (speeds, timings, odds) | |

## 3. Presentation

| | Status |
|---|---|
| Players skinned and animated, ball, courts, sideline camera | done |
| Animation for every move (the banks have them) | done (the moves' own script bodies and blend targets; the animation VM's callbacks not simulated) |
| Crowd, shadows, nets, glows | |
| Sound effects, announcer, music | |
| HUD: score, shot clock, turbo, gamebreaker meter, trick points, player indicator | partly |
| Intros, celebrations, replays | |

## 4. Modes and menus

| | Status |
|---|---|
| Front end: title, main menu, team and court select, My Rules, options, controller setup | partly (text menus; screens traced, the game's own art and controller setup to come) |
| Pick Up Game | done (choose teams, players, court, rules; pause; results and box score, the post-game rematch) |
| Saves (to disk, not memory cards) | partly (the story save: versioned binary format, written after every progression match) |
| Street School (tutorial lessons from the locale text) | done (the 26 lessons, pass detection, the practice pages) |
| League Challenge (5 regions + All Stars playable, legends unlock, the +1000 Reward Points finish) | done (region rosters traced; the All Star roster is the strongest league trio, `likely`) |
| Be A Legend (create a player, development points, rewards) | partly (the progression model plays end to end: events, PPs, reputation, DP charts, the Gym, rival recruits; the map, Update Team, careers and event intros in; the drawn map and the pre-game choose-player screen to come) |
| Rewards and unlocks, high scores, statistics | partly (Reward Points per game in Pick Up and the League Challenge, spent across the game's own seven-category card shop; courts/players unlock as they fall) |
| Local multiplayer (several pads, like the PS2 multitap) | |

## 5. Online

The simulation is deterministic: the same inputs give the same game, and
the computer players are a pure function of the game state. So only inputs
cross the network.

| | Status |
|---|---|
| Determinism that holds across machines (no platform maths in the simulation) | done |
| Compact input and state encoding; state checksums | done |
| `street-net`: protocol over UDP; rollback with input prediction; time sync; resync from the server's state on a mismatch | done |
| `street-server`: headless server (lobby, match, relay of inputs, its own copy of the game on its own disc as the referee: `--vol2`), self-hostable | done |
| In-game Host / Join by address, lobby, team pick, ready | done |
| Leaving and computer takeover, late joining | done |
| Spectators, rematch, created ballers' looks across machines | done |

Design:

- **Topology:** every player connects to one server (a player's own PC or a
  rented box). There is no peer-to-peer traffic, so only the host needs a
  reachable port.
- **Netcode:**
  1. Each client sends its pad state per tick, with redundancy.
  2. The server fixes every player's input for each tick (a late input
     counts as a repeat of the last one) and broadcasts the confirmed inputs.
  3. Clients run ahead with predicted inputs and roll back when a
     confirmation differs.
  4. The server runs the game too, and compares checksums with the clients.
     A client that drifts gets a full state from the server.
## 6. Release

- Publish code and docs only; no disc data, screenshots of game art, or
  binaries built with game data. Players point the game at their own ISO.
- Keep the repository name and description neutral, and say the project is
  unofficial and needs the original game.
- The license is MIT.
- Builds for Windows and Linux (the server at least); CI that runs the tests
  that need no disc.
