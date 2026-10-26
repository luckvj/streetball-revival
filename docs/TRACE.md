# Tracing the whole game: handoff

What has been traced from `SLUS_206.51` and the disc beyond `FORMATS.md`, how
to rebuild the workspace, and what is left. Read `FORMATS.md` and
`ROADMAP.md` first. Addresses are virtual addresses in `SLUS_206.51`
(file offset = address - 0xff000 for `.text`, `.data` and `.rodata`).

## 1. Rebuilding the workspace (do this first)

Game data never goes in the repository. Build a workspace outside it:

```bash
python tools/re_workspace.py "$STREET_ISO" <workspace> --ghidra <ghidra install>
```

- Set `MAXMEM=8G` for Ghidra. The run takes about 15 minutes and creates its
  own project in `<workspace>/gproj`; keep it apart from any other Ghidra
  projects you have.
- Outputs:
  - `gout/decomp.c`: all 8,518 functions; 40 still truncate.
  - `gout/functions.txt`: size and callers for each function.
  - `gout/strings.txt`: each string and the functions that use it.
  - `loc_fe.txt`, `loc_gm.txt`: every game string as `file#index`.
  - `disc/`: the small files.
- Stock Ghidra has no R5900 language. `tools/ee_patch.py` rewrites the
  EE-only opcodes in a copy, and the decompile uses
  `MIPS:LE:64:64-32addr` with the `n32` compiler spec:
  - `lq`/`sq` become `ld`/`sd`.
  - The 3-operand `mult` becomes `mul`.
  - MMI moves become `or`.
  - The FPU accumulator is modelled as `$f19`.

  Without the patch, 5,456 functions truncate. Check exact maths against the
  original with `tools/mipsx.py`.
- Grepping one function: `awk -v a="x0012abcd" '/^\/\/==== /{p=(("x" $2)==a)} p' decomp.c`.
  The `x` prefix matters: awk otherwise reads `002e4478` as a number in
  scientific notation (infinity) and matches the wrong function.
- Other tools in `tools/`:
  - `c0fb.py`: list or extract `c0 fb` archives.
  - `locale_dump.py`: dump the locale text.
  - `iso_list.py`, `iso_extract.py`: list and extract disc files.
- The previous session's workspace (alive as of 2026-10-10, and it holds
  everything above):
  `%TEMP%\<agent scratch>\d98735d7-...\scratchpad\re`.
  It also holds:
  - `funcindex.txt`: functions indexed by string literal.
  - `lay_fe/`, `lay_gm/`: the extracted layouts.
  - `png/`: decoded front-end sheets.
  - `sshpng.py`: the SSH-to-PNG decoder.
  - `lay.py`: the layout parser.

## 2. Executable map (what is known)

- **Sections:**

  | Section | Range |
  |---|---|
  | `.text` | 0x100000..0x3c6408 |
  | `.data` | 0x3c6480 |
  | `.rodata` | 0x437000 |
  | `.bss` | 0x4b3580 |

  The entry point is 0x100008. The game was built from
  `C:/street2/source/{ai,audio,fe/screens,script}`. EAGL 4.07.04 is the
  renderer and animation library.
- **Text lookup.** A text id is a u16, `file << 8 | index`, where `file`
  indexes the name table at 0x3ec2f0:

  | Index | File |
  |---|---|
  | 0 | `common.txt` |
  | ... | (table order) |
  | 89 | `overlays.txt` |
  | 90 | `school.txt` |

  - 0x1d5138 copies a string by id.
  - 0x1d4ef0 caches the locale file by name (`local_fe`/`local_gm` select
    the archive).
  - To find a screen's code, search for callers that pass `file << 8 | n`.
- **Database:** see FORMATS.md. 0x1da4e0 returns a field's address
  (the result is at `+0x20`); a record's name is at `+0x2c`.
- **Allocation:** 0x33c4e8 is `new`.
- **Front-end loaders:**

  | Address | Loads |
  |---|---|
  | 0x2e4478 | `xafebg`, `xafeload`, `xaferwd`, `xarwdbk`, `xafebgex` |
  | 0x2e47c0 | the layouts (`xlay_fe` when the mode is 0, `xlay_gm` when 1) |
  | 0x2e49d0 | the fonts |
  | 0x2e4d40 | the palettes |
  | 0x2e4f68 | `xateams`, `xafescrn`, `xacourts`, `xaports` |

  Front-end tasks: `Front End` 0x2dda30, `Overlays` 0x2e2190, `Pause`
  0x2e3560, `FEPlayerObject` 0x2dacf0. A widget is built as
  `FUN_00238a78(new(0x580), parent, -1, 4)`; it has a vtable at `+0x20`, with
  set-position at `+0x11c` and set-text (printf) at `+0x15c`.
- **Other entry points:**
  - `AIGame` 0x13c4e8; the AI script handler 0x184b00; `AITeam` 0x189280.
  - The gamebreaker task at 0x144df0 (`aigamebreakertask.cpp`).
  - Movie task 0x22d260; replay manager 0x3288b0; script manager 0x335708;
    camera 0x3038b0.
  - Credits 0x281cb0; `demo_delay` 0x29ed60; `GAMEDATA.DAT` (the save
    file) 0x2294e0.
- **Move names:** a table of name pointers at 0x3fb500.
  - 0x3fb500: the 75 trick moves.
  - 0x3fb62c: 0.
  - 0x3fb630: the dunk names.
  - Then the lay-up names, and then the specials (`Back 2 Papa`,
    `Off the Heezay`, `Back at Ya`, `Downtown Rebound`, alley-oop, turbo
    alley-oop, be the oop, turbo be the oop).

  The trick-name overlay probably indexes this table; the link to
  `XDBMOVE` ids is not traced yet.
- **Story mode (Be A Legend) data:** `FUN_001e0bc0() + 0xf10`. The debug
  screen at 0x2cec70 prints it:

  | Offset | Contents |
  |---|---|
  | +0x24 | games played (week = games / 6 + 1) |
  | +0x28, +0x2c | wins, losses |
  | +0x30 | development points |
  | +0x34.. | u8 progress points per region (up to 17; 0x1ead60 returns the region count; 0x1e7550 returns a region's target) |
  | +0xca0 | u16 team |

  - 0x1e73d8 and 0x1e74c0 return overall progress points and the target.
  - Reputation level is 0x1d83c8; reputation points are 0x1d8440. Each level
    needs 10 points.
  - Teammates: slots 1..4 come from 0x1e7d10.
  - Event fields: type (pick-up, tournament, street challenge), new,
    noticed, visible, open, cleared, level, PPs per game.
  - Tournament formats: single elimination, double elimination, best of three.
  - **Brackets** (story +0x4c, 0x104 bytes per tournament by
    `FUN_001eaf90`): 16 rows of {u16 team ×2 (0x25 you, 0x24 a generic
    crew, else an XDBTEAM boss), u32 crew number ×2, u8 game-win bits ×2}
    and a flags word at +0x48. The event's type byte (+0x1b) picks a shape
    in `0x460a10` = {rounds, round table}: 0 single elimination of 8 (rows
    0..3 → 8..9 → 12), 1 single of 16, 2 double elimination of 4 (0..1 →
    8 and 2 → 9 → 12), 3 one best-of-three series, 4 best-of-three semis
    and final; each round entry is {games per series, matches, rows}.
    Winners move by `0x460a38`, double elimination's losers by `0x460a58`
    (`FUN_001ea528`, which takes the winner or -1 for `rand()`
    (`0x3b42a0`, the C library's LCG) for the computer's matches). A
    series is won on more than half its games (`0x1e90a8`).
    `FUN_001e9888` seeds round one: you first, the boss crew last (generic
    once the tournament is won), generic crews numbered upward from a
    random base elsewhere. `FUN_001e82f8` finds your round;
    `FUN_001e7ff8` calls a tournament cleared when its champion slot is
    yours. Tournaments 2, 3, 9 are type 0; 5 and 8 type 2; 7 type 4.
  - **Update Team** (class 96, ctor `0x2d61a0`, `updateTeam.txt` = locale
    file 0x3d): the end board's story case (`FUN_0029d5a8`, case 5) goes
    there after a won game that is not a tournament when one of the three
    beaten players (match +0x9c) is not on your team (the team id at
    story +0xca0, its roster read by `FUN_001ebc88`/`FUN_001ebef0`); a
    tournament goes there from its bracket screen when the final's winner
    is your crew (`FUN_001e8580` = 0x25). The screen (`FUN_002d7238`)
    asks 0x3d04 first; then the beaten team's list (0x3d05), your
    roster's positions (0x3d06; position 0, you, can't be picked), and
    0x3d02 to accept (Back asks 0x3d01). `FUN_001e7db8` writes the player
    into roster slot 1..4 (story +0x6dc, five u16s; a created player's
    0x124-byte record is copied alongside), and picking past the last
    filled position fills the next open one. Before each game the bracket
    goes to choose player (class 0x1d).
  - **The map** (class 56, `FUN_0029f200`; `mapscreen.txt` = locale file
    0x1c): the table at `0x489bd8` holds the 17 spots as {icon, icon,
    event, city 0xa4..0xa9} — New York {0, 12, 2, 4, 6, 10, 9}, Philly
    {11, 1}, Chicago {3, 13}, Seattle {14, 5}, Oakland {8, 16}, Los
    Angeles {15, 7}. On entry (`FUN_002a1c68`) it walks the table for the
    first event whose open gate passes (`FUN_001e8260`) and whose "new"
    bit is set (`FUN_001e7ef8`), marks it noticed (`FUN_001e9798`) and
    shows "Event Unlocked" (0x1c23) with 0x1c25/0x1c26 + 0x1c24 ("The %1s
    %2s is / %1s %2s games are now available for play in %2s."). The kind
    words: pick-up 0x1c13, tournament 0x1c14 but event 9 the Street
    Championship 0x1c15, street challenge 0x1c16 but event 10 the
    Legendary Challenge 0x1c17.
  - **The map drawn** (fill `FUN_002a03c0`): the cities are map nodes
    (`FUN_00225768`, ids 100..105 = cities 0..5, class 0x26) placed at the
    layout's navigation points 1..6; each node's picture is set by
    `FUN_00225d90` from `XAFESCRN` sheet 0 (images 12..15; frames from
    each image's `|` block, 4 points a frame): New York 15/2 dark, 15/4
    lit; Philly 15/0, 15/1; Chicago 12/0, 12/1 turned -90 (+0xec);
    Seattle 14/7, 14/6; Oakland 13/2, 13/1; Los Angeles 13/0 (its beam
    13/3 is the glow). Offsets from the node: picture +0xc4/+0xc8, child 1
    (a shadow, New York 15/7 and Chicago 12/3 only) +0xcc/+0xd0, the glow
    image (+0xdc, 15/5, 15/6, 15/8, 15/9) +0xd4/+0xd8 (`FUN_00225a78`);
    with +0xbc set (a city's own page) a second set of offsets and 1.5
    times the size. `FUN_002266e0` lights the picked city (lit frame,
    glow, the name +0xe0 = layout text 27..32); `FUN_00225bc8` shows only
    a blue star (14/5) for a city with no open event (+0xb0, set by
    `FUN_002a0fe0` when one of its events passes `FUN_001e81c8` and
    `FUN_001e8260`). The read-out `FUN_002a0c00`: record fields 2 and 1
    (first, last name; one line at (65, 355) if the last is empty),
    0x1c04 "Win-Loss" before any game else 0x1c03, 0x1c02 progress,
    element 20 (kind 39) the reputation meter (`FUN_00234a50` builds it:
    five 20x19 pips 18 apart from the element's own left edge; `FUN_00234d50`
    lights the level's count with `XACOMMON` image 1's frame 7 (the
    light-blue star) over the unlit frame 1, and swaps the caption word to
    `reputation.txt` #0..#5 — REPUTATION, ROOKIE, BALLER, ALL STAR,
    SUPERSTAR, LEGEND — the caption piece `FUN_00238a78` at +22 with the
    amber 0xfbac00; `FUN_00234d10`'s value (the points again, ≤5) draws a
    partial fill 19 px a point), stars 44/45 = `XACOMMON` sheet 0
    image 1 frame 1. A city's courts are markers (`FUN_00224150`, ids
    137..146 from the `0x489bd8` table, two slots: pick-up/tournament and
    street challenge) listed in element 26 at navigation points 21..25
    (`FUN_002a2548`); a row (`FUN_00224838`): court name at (-20, -20),
    a second line (font 1, 0x88a0be) at (-20, 5) when picked, a 333x4
    rule 0x3c45a2 at (-25, 28), frames 14/3 at (195, -21) and (250, -21)
    with the icon (`FUN_002250b0`: pick-up 14/1 at +(3, 2), tournament
    14/0 at +(8, 7), street 14/2, the '78 one 14/8, both at +(3, 9)), a
    tick 14/4 at +(8, 7) once cleared (`FUN_00225308`), and the gold
    focus frame 0/1 (drawn 50x48); alpha 0x3f locked, 0x7f open, 0xff
    picked (`FUN_00224e00`). On a city's page element 43 (black) goes to
    alpha 0x7f and the divider 42 shows (`FUN_002a14f8`, `FUN_002a2318`).
  - **The bracket screens drawn**: classes 88 (`FUN_002cbed8`, fill
    `FUN_002cc050`), 89 (`FUN_00285050`, `FUN_002857f8`) and 94
    (`FUN_00265f48`, `FUN_00266080`), backdrops 24, 11, 1. Element ids:
    name `(r+1)*100 + (m+1)*10 + s`, plate +1000 (`XAFESCRN` 0/2 frame 0),
    badge `(r+5)*100 + (m+1)*10 + s` (frame 2 "W!" = `DAT_003f7b58`,
    frame 1 "L" = `DAT_003f7b5c`; best of three one per game). Team getter
    `FUN_001e8608` (0x24 generic: "Team %1d" `common.txt` 42 with
    `FUN_001e86e0`'s number; 0x25 yours: the story team +0xca0's name;
    else the XDBTEAM name), winner/loser `FUN_001e8918`/`FUN_001e89f8`,
    winning side `FUN_001e8ad8`. Colours (ctor +0xf0..+0x104): yellow
    0xffd03f current heading, navy 0x1a205d loser, blue 0x3e4699 winner,
    yellow 0xffd03f your next match, orange 0xf87106 your played
    matches, grey-blue 0x88a3b7 your next opponent; plates recoloured by
    `FUN_00210368(w, 3, k)` = `XPAL_FE` sheet 3 palette k (only entry 32,
    the outline, differs: 0 grey-blue, 1 navy, 2 orange, 3 yellow).
    Your way: single elimination's connectors 0xd06..0xd08 (frames 5..7
    at (190, 152), (319, 186), (447, 262), static init `FUN_002cd618`),
    best of three's 0xd0b/0xd0c (frames 3, 4 at (205, 163), (383, 236),
    `FUN_00267370`), double elimination's 28 line widgets
    (`FUN_0021e350`, 3.0 wide, 0xf87106) whose ends sit in the ctor at
    +0x138, shown in sets: won round one d06..d08, lost d1c..d1e, won the
    winners' final d09/d0a/d0b/d18/d0e, lost it d10..d12, won the losers'
    bracket d1f..d21, won the losers' final d15..d17 (from the winners'
    side) or d19..d1b, champion d0c/d0d/d13 (straight) or d0f. Once the
    final is played the forfeit and back prompts hide and "Final
    Standings." shows (single 24, double 37, best of three 26).
- **Console variables** (cvars, set from `CSCRIPTS`): `drones_on`,
  `blocks_on`, `steals_on`, `def_switch_cycle`, `auto_replay`,
  `demo_delay`, `create_team_add_generic`, `debug_passoutOneTurbo`, and
  about 40 `audio_*` volume and ducking settings. (`def_switch_cycle`,
  `DAT_003d01c8`, is registered 0 and no `CSCRIPTS` file sets it:
  "Who the person controls, and the stick".)
- **Instant replays:** `auto_replay` (registered at `0x3252bc`, value at
  `0x40f9c8`) is 1. After a basket the replay director (`FUN_003248d8`,
  game +0x42c) rolls a chance: a paying combo min(2 x payout, 35)%, an
  alley-oop finish 30% (replayed from its pass), any other dunk 10% (from
  its release), else 0; Always forces 100%. It is refused in a lesson,
  while a gamebreaker task runs (`0x324d0c`), on a gamebreaker shot, during
  a script, with either meter full, or within 540 updates of a gamebreaker
  (the earlier "20 s since the last replay" reading was wrong). A won game
  always replays. Square starts an offered replay at 0.8125 and Cross
  cancels only an offer (`USRGAME*.CFG`). Details: `docs/CAMERA_RECOVERY.md`
  third pass; port: `crates/street-game/src/tape.rs`.
- **Effects in a match:** the court effects parser (`0x2f3b30`) also
  builds, for every match, the managers of types 0x10, 0xc, 0xf, 0xd and
  0x12 (factory `0x20dac0`):
  - 0x10 (`0x204d88`, drawn by `0x204ff0`): the glow on moves. 24 ribbons,
    four per player between the bone pairs at `0x4641e0` (14-15, 18-19,
    2-3, 6-7: forearms and shins), 12 positions long. A player's ribbons
    show while their side's chain has recent links (`0x1318d8`) and the
    player's state has bit 6, 7, 10, 12, 13 or 15 set (+0xa4), in the
    level's colour (`0x204f90`, table filled at `0x2056a8`: white 100,
    yellow, orange, deep orange, red from 5) at a quarter alpha; slot 15
    (a passout special) turns them blue (100, 169, 220). Off in replays and
    with the visual effects option off. `trails.rs`.
  - 0xf (`0x1f6fc8`, listener `0x461e28`): the hoop on fire after a
    gamebreaker basket (slot 26 sets the hoop's flags), put out when the
    gamebreaker ends (slot 53); 120 particles. `ambience.rs`.
  - 0xc (`0x1ef2d0`, update `0x1ef3e8`, constants set by `0x1efaa8` from
    the frame rate at `0x4da218`+0x24): the gamebreaker ball's sparks,
    30 of the shared particle texture 5 (`spark.ssh`, names at
    `0x40f400`). Shown while the ball's +0x1a0 is set (`0x14a968`): 1 while
    the side with the ball has a full meter (50,000), 2 during a
    gamebreaker. Two start a frame inside the ball and along its last
    frame's path, drift out (up to 0.5 mph) and back along its way (1 mph),
    live 15 frames, shrink from 0.15 by 0.125, blink out one frame in 5 to
    10; light blue (136, 160, 190) or, in a gamebreaker, white.
  - 0x12 (`0x1eff40`, update `0x1f00f0`, `motion.ssh`): a 12-point ribbon
    from the top of the ball to the bottom, blue (100, 169, 245, 100),
    while the ball is a pass (+0x180 = 4, set by `0x14bff8`; 5 is a shot)
    and either a play field (game+0x160) is ±1 or the receiver's move bit
    15 is set; played as passout specials and alley-oop passes. Cleared at
    once when the pass ends. Both in `ballfx.rs`.
  - 0xd (`0x1f8320`, update `0x1f8540`, drawn by the ribbon class
    `0x307668`/`0x3079e0`): the gamebreaker ball's streamer, a
    stars-and-stripes ribbon (`flag.ssh`, 512x128) along the ball's last
    40 positions, shown while the ball's +0x1a0 is 2. Over its first 6.25
    feet (+0x60) it widens from 0.25 (+0x58) to 1.5 (+0x5c), waves across
    by `0.2 (1 - t) sin(3 pi t)`, fades from 0.5 (+0x38) to 0 (+0x3c),
    faces the camera, and the texture scrolls one repeat in 0.6 s (+0x6c).
    GS ALPHA 0x44. `ballfx.rs`.
  - Cutscenes (`SCRIPTS/SCRIPTS.BIN`, magic `ssb`, 237 scripts of staged
    positions) spawn particles through `0x332f08` (codes 0..0x10 to types
    0x14..0x24).
- **Celebrations:** the animation table at `0x3e4e70` lists the 15 clips
  (three `celebNN_FINAL`, two `jogceleb_*`, ten `CELEBRATE_FREESTYLE*`);
  intros (`INTRO_*`, `SIG_MOVE_PICKUP_*`) and outros (`OUTRO_*`) have their
  own banks. Which plays when is the animation scripts' choice (not traced);
  `view.rs` gives the scorer one through the dead ball and the winners an
  outro.
- **AI play names** (state labels): fast break, outlet, pick and roll
  (3pt), isolation (3pt), pen and kick, post and kick, post up, back down,
  cutter, rotate, keep away, box out, goal tend, toss tip off,
  PocketGB/CancelGB, alley oop / double oop.

## 3. Controls (correction)

The game itself uses `CSCRIPTS/USRGAME1.CFG`, `USRGAME2.CFG` and
`USRGAME3.CFG`, which are the three controller layouts; `INPUT.CFG` is not
it. Layout 1:

| Button | Actions |
|---|---|
| Circle | shoot |
| Cross | pass, switch, cancel replay |
| Square | juke, steal, start replay |
| Triangle | rebound, block |
| **Select** | **pocket or cancel gamebreaker** |
| **R3** | **strategy (call pick or oop)** |
| L1, R1, L2, R2 | the four turbos |
| L3 | kick pass |

Layout 2 moves juke/steal to Triangle, pass to Circle, and shoot/block to
Cross. Layout 3 puts shoot plus rebound/block on Circle. The front end binds
Cross as advance, Triangle as cancel and Square as help (`USERFE.CFG`).

Done on 2026-10-10: pocket/cancel sits on Select (and the pocket hint
texts confirm Select, `overlays.txt` #54..56) and strategy on R3.

## 4. Front-end data

- **`c0 fb` archives:** `c0 fb`, a BE u16 directory length, a BE u16 count,
  then per entry a BE u24 offset, BE u24 size and a NUL-terminated name
  (empty in `/FE`). The locale files use the same container.
- **Layouts** (`XLAY_FE.BIN` carries 61 blobs, `XLAY_GM.BIN` 12; loader
  0x2e47c0 picks the archive from the resident state, `local_fe`/`local_gm`):
  - Header: u32 chunk count (1), u16 element count, u16 props table offset
    (0x0c), u32 0.
  - Element table at 0x0c: per element a u16 **prop count** (not a type) and
    a u32 offset to that element's props; the props are a run of
    (u16 id, u32 value) records.
  - The first prop of every element (id 0) is a packed descriptor:
    bits 0..3 horizontal/vertical alignment, bits 4..11 the element **kind**
    (which widget the walker `FUN_002e5d30` builds), bits 12..31 the element
    id. Ids of 0xfff… are unattached templates (203 in the FE set, 72 in gm).
  - Each blob has exactly one kind-0 element: the **screen declaration**; its
    id is the screen class and its props are full-screen 640×480 at (0,0).
    `FUN_002e6338` applies the root's props to the screen widget
    (`FUN_00248df8`) and builds the other elements' widgets as children.
    Group subtrees are pulled by id (`FUN_002e6338`/`0x2e6518`); list rows
    stamp the row id at widget+0x40 and pull their subtree on demand.

  | Property | Meaning |
  |---|---|
  | 0 | packed kind/id/alignment descriptor |
  | 2 | always 0 |
  | 3 | position, x and y as u16 |
  | 4 | size, w and h as u16 (640×480 = full screen) |
  | 7 | emphasis: shadow colour 0xff000000 and 2.0 scale |
  | 10 | ARGB colour |
  | 11 | a 1/1000 fraction pair |
  | 13 | a sprite/row id (the text hook picks element 13 and text 13+14) |
  | 25..34 | kind-10 navigation nodes (`FUN_00221ba8`): direction
    mode/link pairs 25/26 up, 27/28 down, 29/30 left, 31/32 right;
    modes 0 disabled, 1 scroll the bound list, 2 follow the explicit id
    (`FUN_00222590`, `FUN_00222678`, `FUN_0021f8f0`). 33 binds the list,
    34 is the row ordinal. Screen stepping honors the up/down modes,
    stays on invalid links, and falls back for scrolling/added rows.
    Selection/visibility belongs to the bound display widget at node+0xb0,
    placed by `FUN_00222420`, not a texture inherent in kind 10.
    Kind-36 overrides and dynamic disabled-widget skipping remain open.
    See `FRONTEND_RECOVERY.md` for the trace boundary. |
  | 39..47 | list-row lookups |
  | 56..63 | four per-line u16 text ids |

- **Screens** (blob → class id → constructor): 3 main menu `0x29e138`, 5
  Play It `0x2bed78`, 9 credits intro, 20 rewards poster, 29 choose player
  `0x269b28`, 33 choose court `0x267b00`, 39 set challenge `0x2c8ad0`, 40
  options `0x2a4c58` (204 options text refs), 41 choose region `0x26b408`,
  48–51 the intro boards, 52 The Rundown (in-game, the first post-game
  board; not a pause body), 53 create team, 54 story menu
  `0x2cd930`, 55 control config, 56 map, 59/60 the end boards, 77 join team,
  79–85 edit player (clothes/shoes/hair/facial/skills/moves, one parameterised
  ctor `0x28b768`), 88/89/94 single/double elimination and best of three,
  96 recruit, 97 save, 102–104 the DVD pages, 105 difficulty. In-game: 2 the
  HUD `0x2ad190`, 34–38 the post-game boards (with 52; see §5 "The
  post-game boards"), 44 loading, 87 (milestones) / 95 (nickname) / 100
  (challenger) panels. Class ids with no factory case ({1, 11, 21–24, 26, 30–32, 42, 46,
  57, 58, 62–73, 86, 90–93}) are layout **group ids only** — the
  create-a-player wizard lives at 62–73; `SetScreen` can never be called with
  them. The full blob table, every class's ctor address and which locale file
  each screen reads are in the session's report (`%TEMP%\opencode\fe_trace.md`).
- **Transitions:** there is no stack. The Front End task (`0x2dda30`, vtable
  `DAT_00495738`) keeps the current class at +0x40; `SetScreen` (vtable +0x54
  = `FUN_002dd2d8` → `FUN_002dcff8`) runs the class factory `0x2e55c0` and the
  element pump (vtable +0x5c = `FUN_002ddb40`) streams layout elements in over
  frames, playing the blur movies (`movies/xmt[fb]blur.nfx`) with a
  "sameloop" guard. Boot goes to class 0x2f (`FUN_002e1bf0`), then class 4
  (the FE's own empty stage). Overlays boot on class 2 (the HUD, `0x2e2208`).
  Pause (`0x2e3560`; opened by `FUN_002e3690`) picks a class by the
  selector `DAT_004b34a8`: 0 → 0 (`pause.txt`); 1, the final buzzer → 52
  The Rundown (34 or 87 when rec+0x78 = 1); 2 → the post-game dispatcher
  `FUN_001ded40`, then 34 (anything queued) or 87; 3 → 100.
- **Widget classes:** the base widget's token is `DAT_0046d570` with the
  per-class props hook at vtable +0x64 (the base one, `0x238b88`, loads text,
  colour, scale and the prop-7 emphasis), set-position at +0x11c, the
  prop-3 pair at +0x124, set size at +0x12c, set text at +0x158..0x160
  (splitting on '\n' into the text widget's row tables at +0x458/+0x4c0) and
  set-text-by-id at +0x16c. Task classes: Front End `0x2dda30` /
  `DAT_00495738`, Overlays `0x2e2190` / `0x495db8`, Pause `0x2e3560` /
  `0x495eb8`, FEPlayerObject `0x2dacf0` / `0x494e40`, base task
  `FUN_002dcca0` / `0x495298`.
- **Sheets:**
  - `XAFESCRN` (3 sheets, 123 images) and `XACOMMON` (4) are SSH atlases:
    map icons, mode icons, reputation crowns, trophies, frames.
  - `XAPORTS` holds 245 portraits; `XATEAMS` holds the team logos.
  - `XAFEBG` holds 35 backdrops of three strips each (320x240 pictures
    shown doubled); each screen's constructor names its backdrop in its call
    to the base screen constructor `FUN_002d7720(screen, class, backdrop)`,
    and the resource manager `FUN_002e4560` opens entries `3n..3n+2`. The
    per-class table is in `docs/FORMATS.md`.
  - `XACOMMON` sheet 0 image 5 holds the menu's star bullets: blue at
    (2, 20) 24x22, white (highlighted) at (3, 56) 28x26.
  - `XPAL_FE` and `XPAL_GM` hold 256×1 palettes, probably colour themes.
  - `HUD.SSH` is one 256×256 HUD atlas.
  - Other sheets: `OFFIND` (off-screen indicator), `REPLAY`, `BALL` (20
    images), `EXPLODE` (11).
  - `XAFELOAD.BIN` is a BIG archive of loading screens.
- **Fonts** (`XFONTSEN.BIN`, 12 `FNTS` fonts):
  - Header: `FNTS`, `PD`, u16, u16 glyph count, ..., the glyph table at
    0x80.
  - Glyphs are 12 bytes each: u16 character, u8 width, u8 height, u16 x,
    u16 y, u8 advance, i8 x offset, ...
  - Floats at 0x40 give 1/texture width and 1/texture height. The texture
    is 4-bit and follows the kerning data.
- **`COURTFX/<court>.BIN`:** plain-text court settings:

  | Key | Meaning |
  |---|---|
  | `SUN_DIRECTION` | sun direction |
  | `SHADOW_COLOUR` | shadow colour, RGBA as 0..1 floats |
  | `INDICATOR_CIRCLE`, `INDICATOR_STAR` | player indicator colours |
  | `WIND` | wind: speed, direction x z |
  | `DRIFT` | blowing leaves or trash (two models, counts, spread) |
  | `PS_STEAM` | a steam vent and the way it blows |
  | `COLD`, `HOT` | breath (six emitters); `HOT` turns steam off |
  | `PS_DUST` | not read by the PS2 game |
  | `E STREET`, `E STREET CONNECT`, `E STREET STOP`, `E CAR` | traffic; not read by the PS2 game |

  The keywords are at `0x497f88..0x498078`. The effects' motion (drift
  `0x1f4950`, steam, breath) runs on VU0 code with tuning set at run time;
  `ambience.rs` matches the setup and approximates the motion.

## 5. Modes, rewards and progression (from the text and tables)

- **Main menu:** Play It (Street School, Pick Up Game, League Challenge, Be A
  Legend), My Team (Create A Baller, Create Team), Rewards, Options, High
  Scores, Inside Vol. 2, What's New.
- **League Challenge:**
  - Regions: North West, Central, South West, North East, South East, league
    All Star.
  - Finishing it gives Reward Points (`nbatcend.txt`).
- **Be A Legend:**
  - A map of cities: New York, Los Angeles, Philly, Chicago, Seattle,
    Oakland.
  - Event types: Pick Up, Tournament (single elimination, double
    elimination, best of three), Street Challenge, Street Championship,
    and the Rucker Park '78 Legendary Challenge.
  - Events unlock by Progress Points or by clearing other events.
  - Reputation levels: Rookie, Baller, All Star, Superstar, Legend.
  - Development points come from trick points, Level 1 and 2 gamebreakers,
    completed courts, tournament wins and a win/loss bonus.
  - After a win you can recruit a player from the beaten team
    (`updateteam.txt`).
  - Nicknames are earned per skill. The flavour lines are
    `nicknames.txt` (locale file 99, 48 lines, one per `XDBALIAS`
    nickname row): a screen builds the text id as `0x6300 + <database
    field 6>` (`0x299d2c` → the field-6 getter `FUN_001d7ca8` =
    `FUN_001da4e0(rec, 6)`), and readers of the same getter live at
    `0x26a414`/`0x271f9c` and the debug screen. So the baller's record
    carries the nickname row in field 6 and the board reads the
    matching line. The exact pick rule (which feat sets field 6) is
    still to trace.
- **Street challenges:**

  | Court | Challenge |
  |---|---|
  | Philly | first to 250,000 trick points |
  | Chicago | dunks only to 12, 12-second shot clock |
  | Old School | to 12, every shot worth 1 |
  | Gamebreaker Wins | first Level 2 gamebreaker wins |
  | Whitewater | no gamebreakers |
  | league | league scoring to 50 |

- **Street School:** 26 lessons (the lessons-complete text counts to 26),
  from pass to pocket gamebreaker, plus tutorial pages (`school.txt` 0..158).
  State and the lesson table are traced below.
- **Rewards** (`XDBCARD.ADF`, decoded in this session):
  - Fields: 0 id, 1 category, 2 index within the category (development
    points: the amount), 3 cost in Reward Points, 4 the card's picture
    (`XAFESCRN.BIN` sheet 2, by position; 0 none), 5 the unlock condition
    parameter.
  - Categories (named by `poster.txt` 9..15; the pictures and the names
    `poster.txt` gives each card agree on the contents):

    | Category | Contents | Entries | Cost (Reward Points) | Names (`poster.txt`) |
    |---|---|---|---|---|
    | 0 | Street Legends (records 145..157; Nelly's card is five) | 9 | 500..5000 | 52..60 |
    | 1 | Legends (records 158..183) | 26 | 0..5000 | 61..86 |
    | 2 | Courts | 11 | 0..2500 | 41..51 |
    | 3 | Jerseys | 25 | 50..1000 | 94..118 |
    | 4 | Development Points (100..1000) | 5 | 200..1200 | 119..123 |
    | 5 | Street Tips (12 pictured; 8 placeholders) | 20 | 100 each | |
    | 6 | Cheat Codes (12 pictured; 5 placeholders) | 17 | 500 each | |

    Field 5 is an automatic **Pick Up win threshold**, resolved through
    `FUN_001ded40` at `0x1df6b0..0x1df780`: the shared counter at game
    record +0x1c increments once when a human side wins in modes 2/3.
    Every positive threshold at or below that counter grants an unowned,
    positive-price card without charging RP (`FUN_001d26e8`,
    `FUN_001e0420`; one Street Legend notice slot). The first three
    Street Legends use 10/15/20; tips use 15..53; pictured codes 40..59.
    These are not button sequences. Built: `pickup_wins`, PKUP save
    extension, original-save import from unlock section +4, automatic
    grants with a disc test covering every threshold. The tips' texts
    are `streettips.txt` (locale file 57, #1..#20). The category walker
    (`FUN_001d2b38`, `FUN_002c2a90`) includes zero-picture rows too:
    all 20 tips and 17 codes stay in the shop. Zero-price cards are
    considered owned by `FUN_001d26e8`, independent of saved bits.
    See `MODE_RECOVERY.md`.

  - The text for each reward's unlock condition is `poster.txt` #127..199.
- **Milestones:** 22 in `milestone.txt`, for example:
  - 1,000,000 trick points in a game;
  - a shutout;
  - 21 alley-oops;
  - each League Challenge region with no losses;
  - finishing Street School;
  - 20 blocks or 20 steals;
  - reputation levels.

  Each unlocks a throwback jersey.
- **High scores:** Street Legends, Fastest Rise To Fame, Win %, and per-game
  records for trick points, GB1, GB2, steals, blocks, alley-oops, long-range
  shots, dunks and pass-outs.
- **Cheats:** names in `common.txt` #27..38: Street Kids, No Counters, GB2
  Trails Always On, All Quicks, Unlimited Turbo, Easy 2 Pointers, Hard 2
  Pointers, Big Heads, women's Ball, Alternate Ball, No HUD, Explosive Rims.
  They are entered on the user-setup screen (`entername.txt#44`); the
  checker is `FUN_002453e8`, called by `FUN_00245b88` in User Setup
  widget states 11/12 while L1 is held. Mode 1 rejects entry. The actions
  pack Circle=1, Square=2, Triangle=4, Cross=8 into four bytes; failures
  retain a rolling three-button prefix, successes clear it. There are
  **17 codes**, cards 96..112 of the **113-row** XDBCARD table: twelve
  effects and five unlocks. The exact sequences and singleton writes are
  in `street-game/src/cheats.rs`, with tests for every code.
  The bought card's name comes from `common.txt` #27+index; four icons
  come from the u16 table at `0x48f698`. **The previous `"00%x"` reading
  was false:** raw bytes at `0x48f850` are UTF-32 `%d`. Its readers
  format Reward Points and development-card amounts, not cheat codes.
  Likewise screen+0x5c in `FUN_00299110` is a controller/navigation mask,
  not saved cheat flags. `FUN_0030fa50` returns normally; its incorrect
  no-return annotation truncated many FE callers. `tools/ghidra/RecoverAt.java`
  repairs those functions in a read-only project run.
  Rating getter `FUN_001d7d00` forces Speed=99 for All Quicks and
  Threes=99/0 for Easy/Hard 2 Pointers. The old inside-arc multiplier was
  removed. Body/head/ball selectors are traced by `FUN_003153d0` and
  `FUN_00316a28`; rendering adoption is tracked in the audit.
- **Difficulty:** Got Game, Mad Game, Legendary.
- **Options:** Trax, difficulty, commentary, replays, SFX/speech/music
  volume, audio output, adjust screen, on-screen help, autosave,
  widescreen, visual effects, game data, controller layout.
- **Pause:** volumes, track info, camera (Street, Court, Slide, Solid,
  Behind, Wide), on-screen help, visual effects, controller layout,
  replays, quit, rematch.
- **Overlay words** (HUD pop-ups, `overlays.txt`): super, transition,
  defense, combo, dunk, alley-oop, lay-up, long range, short range, tip
  slam, busted rim, dish, trick, fakeout, pick, pump fake, hookup, dive,
  block, steal, rebound, passout, trick counter, pocket, cancel, kick pass,
  win by two.

### Mode tables (traced 2026-10-10; every address from the decomp and the
### raw ELF, details in the session report `%TEMP%\opencode\modes_trace.md`)

- **The story record** is `FUN_001e0bc0()` → `DAT_004bbf10`; the story block
  sits at record+0xf10 (0xcf8 bytes, reset per mode by `FUN_001daf68`):

  | Block offset | Contents |
  |---|---|
  | +0x24 | games played (a week is games / 6 + 1) |
  | +0x28, +0x2c | wins, losses |
  | +0x30 | development points (a fresh block starts at **1,500**) |
  | +0x34..+0x44 | u8 progress points per event (17 events) |
  | +0x660 | street-challenge cleared bits, one bit per cleared challenge |
  | +0x746 | reputation points (a level is 11 points, levels 0..5) |
  | +0xca0 | u16 the current team |
  | +0xcf0 | the win/loss DP multiplier (1.0 on a win, 0.5 on a loss) |
  | rec+0x1c0c + k·4 | the results queue, kinds: 0 Reward Points total, 1
    team unlock (31..35 the regional legends, 46 Rucker Park Legends),
    2 court id, 3 rival player (145..150 Stretch..Osmosis), 4 street
    unlock (0x41..0x46), 7 poster id |

- **The event table** at `0x460790`, 17 rows of 0x1c (getters
  `FUN_001eaaf8`..`0x1eaf90`). Per row: kind @+0 (0 pick-up, 1 tournament,
  2 street challenge), court @+4 (XDBCOURT id), level @+8, visible @+0xc
  and open @+0x10 (bit 15 set means "clear event id & 0x7fff first"; else
  the value is required overall progress points), progress target @+0x14,
  pick-up points per game @+0x18, tournament boss team @+0x1a, format
  @+0x1b (0/1 single elimination, 2 double, 3/4 best of three). The rows:

  | ev | kind | court | lvl | visible | open | PP target | per game | boss team | format |
  |---|---|---|---|---|---|---|---|---|---|
  | 0 | pick-up | 0 NYC Rec Center | 0 | ≥0 | ≥0 | 15 | 3 | | |
  | 1 | pick-up | 1 Broad Street | 0 | ≥0 | ≥9 | 20 | 4 | | |
  | 2 | tournament | 3 Soul in the Hole | 1 | ≥12 | ≥17 | 25 | | 40 Stretch | 3 rounds |
  | 3 | tournament | 2 Foster Beach | 1 | ≥24 | ≥45 | 30 | | 42 Biggie Little | 3 rounds |
  | 4 | pick-up | 4 The Cage | 2 | ≥24 | ev 2 | 30 | 6 | | |
  | 5 | tournament | 7 Greenlake | 2 | ≥32 | ev 3 | 35 | | 44 Whitewater | double |
  | 6 | pick-up | 6 Uptown | 3 | ≥100 | ≥130 | 40 | 8 | | |
  | 7 | tournament | 8 Lincoln College | 3 | ≥130 | ≥160 | 40 | | 43 Dime | bo 3 |
  | 8 | tournament | 5 Mosswood | 4 | ≥160 | ≥200 | 65 | | 45 Osmosis | double |
  | 9 | tournament | 9 Rucker Park | 4 | ≥200 | ≥300 | 100 | | 41 Bonafide | 3 rounds |
  | 10 | street | 10 Rucker Park '78 | 4 | ≥300 | ≥450 | 50 | | 46, Julius Erving | |
  | 11..16 | street | 1, 3, 2, 7, 8, 5 | 1..4 | by level | the matching tournament | — | | unlocks 0x41..0x46 | |

  The streets map to ordinals 11 Broad Street, 12 Soul in the Hole, 13
  Foster Beach, 14 Greenlake, 15 Lincoln College, 16 Mosswood; their
  rules go by court (see "Street challenge rules by court" in section 4;
  an earlier note here had them on the wrong events). The rival triples are at
  `0x4609e0`: {Stretch/145, team 40}, {Biggie Little/147, 42},
  {Whitewater/149, 44}, {Dime/148, 43}, {Bonafide/146, 41},
  {Osmosis/150, 45}, and {Julius Erving/158, 46 Rucker Park Legends} at
  the '78 challenge. Tournament winners let a star join your team
  (notice kind 3, `updateteam.txt`).
- **Reputation** gains per game (win table `0x460b28` indexed
  event level × reputation level; e.g. a level-4 event win at Rookie +5; a
  loss −1..−5 by table `0x460b70`). A level is 11 points (`/0xb` at
  0x1d83c8), and **every level adds +10 overall progress points**
  (`DAT_00460a78` = {0,10,10,10,10,10}); at level 3 an unset nickname
  unlocks by the best skill. The overall target is **450**: the events'
  400 plus 50 reputation — the '78 challenge opens at exactly 450.
- **Development points per game** (`FUN_001e9b90`, traced 2026-10-12 for
  the post-game boards): five u16 lines at story block +0xce4
  (`FUN_001eaa88(blk, line, dp)`), totalled into +0xcd0 and added to the
  balance at +0x30 by `FUN_001ea900(blk, 1, won)`; screen class 35 shows
  them (`FUN_002625a8`: rec+0x1bf4.. = block+0xce4..):
  - line 0, trick points: the chart at `0x460a88`, 14 rows of {u32
    threshold, u16 dp} **highest first**; the first row reached pays
    (0x1ea294): ≥3,000,001 → 500, ≥2,750,001 → 350, then 300, 275, 250,
    225, 200, 175 down to ≥1,000,001 → 150, ≥1,000,000 → 125, ≥750k → 100,
    ≥500k → 75, ≥250k → 50, ≥1 → 25.
  - line 1, Level 1 gamebreakers, and line 2, Level 2: {u32 cap, u16 step}
    rows by difficulty (rec+300 & 3) at `0x460af8` (10 up to 100, 25 up to
    250, 50 up to 500) and `0x460b10` (25 up to 225, 50 up to 450, 100 up to
    900); a step per gamebreaker until the cap (0x1ea2ec, 0x1ea34c).
  - line 3, "Completed Court" / "Tournament Victory": **+100** the first
    time a pick-up reaches its target, a tournament is won, or the Rucker
    Park '78 challenge (court 10, `FUN_001eab50`) is won; other street
    challenges pay none.
  - line 4, the win bonus: 100, or 150/200 at Mad Game/Legendary while
    `0x3ec4a0` is set (`FUN_001e0c18`; the game record's builder sets it at
    0x1daf34); a loss pays **0**. The 0.5 a loss writes at +0xcf0 is never
    read back (no `lwc1` of +0xcf0 in the executable), so nothing is halved.
  - A pick-up pays its progress points only on a win; games played +2 each
    (a tournament +6, `FUN_001e70a8(blk, 0, n)`).
- **League Challenge:** the region table at `0x3ec9e8`
  (5 × {count, match-list pointer, legends team, court}) and All Stars as
  the terminal region (`choosergn.txt`): North West {7, 22, 23, 25, 27, 5},
  Central {2, 3, 6, 9, 14, 15}, South West {4, 8, 10, 11, 21, 24},
  North East {16, 1, 28, 18, 26, 20}, South East {19, 13, 12, 17} — league
  team ids on the region's courts Greenlake, Foster Beach, Lincoln College,
  The Cage and Broad Street; the legends teams 31..35 hold
  Maravich/Gervin/Thompson, Bird/Drexler/Isiah, Wilkins/Worthy/Malone,
  Wilt/Russell/Archibald and Dawkins/Cousy/Robertson. Clearing a region
  pushes the court + legends-team unlocks and milestone bits; finishing it
  all pays **1,000/1,500/2,000 Reward Points** by difficulty
  (`FUN_001deac8`: `FUN_001de528(rec, 0, rp)`, the immediates at
  `0x1deb30`/`0x1deb20`/`0x1deb0c`; the end screen is `FUN_002a46b8`).
- **Street School** is 26 lessons in a 26×0x20 table at `0x3fb088`
  (title, body, tip, page, complete counter, prompt id, flag, action id);
  completion lives in rec+0x33f4 (`FUN_001eb4a8` reads, `FUN_001ebbc8`
  writes) and the menu counts through `FUN_002bb518` ("%1d/26 Lessons
  Complete"). The lessons in order, by action id (the in-game task), are:
  Pass 0, Short Range Shot 1, Long Range Shot 1, Dunks 1, Dunk to Lay-up 1,
  Alley-Oop 0, Trick Move 2, Switch Players 0, Steal 2, Block 3, Trick
  Counter 2, Turbo Block 3, Pump Fake 1, Fadeaway 1, Gamebreaker 1, Pick 6,
  Dive (double tap), Kick Pass 0, Off the Heezay 0, Back 2 Papa 0,
  Advanced Trick Move 2, Advanced Dunk 1, Give 'n Go 7, Pocket Gamebreaker
  4, Cancel Gamebreaker 5, Level 2 Gamebreaker 1 — with the prompt texts
  (press / any turbo + / 2 turbos + / all turbos + / tap / move, ...) from
  the table's +0x14 ids. The pass detector itself is still to trace.
- **Create A Baller costs:** spending goes through `FUN_001e2768`/`0x1e6518`
  with cumulative per-skill tables pointed at `0x4602a4`: cost to a new
  rating is the difference of u16 rows; the clean cumulative bands at
  `0x4604c8`/`0x460518` price rating 1..5 at 25, 6..10 at 50, 11..13 at 75,
  14/15/17 at 100 and 16/18/19 at 125; the point-buy ladder {0, 100...600}
  is at `0x460568`. Move purchases resolve XDBMOVE at 60/125/350.
- **Created players' moves:** the record holds `XDBMOVE` rows in three
  groups (`FUN_001d8778`: shots 6 at field 68, dunks 11 at 79, dribble
  tricks 11 at 90 — the earlier reading called the shots lay-ups). In a
  match `FUN_0013a1f8(sets, kind, set<<16|player, slot)` turns a slot
  into the move's script id (for a created player, team 0x25, from its
  own rows) and `FUN_00139d38` gives a move id's value class for the
  trick ladder; other players get fixed ids (Stretch, Bonafide and the
  like by record, the table at `0x43d6c0` otherwise). All traced and
  wired 2026-10-12 — see "Created players' moves in play" in section 5b.
- **Controllers:** the PS2 game names DUALSHOCK 2 only; its notices are
  `control_chk_ps2.txt` (a controller removed, please re-insert) and
  `contconfig.txt` 36..37 (Rumble Feature, Vibration).

## 5b. Movies

- The movie task (`0x22d958`) plays a queue of names; it restarts the
  front end's music after `xmt0016` and the intro clips, and treats
  `xmt0017`, `xnufx` and the film room specially for sound.
- The film room's clip for a move comes from a database cursor on the
  moves table: field 5 (the move id) printed as `xmt000%d.nfx` or
  `xmt00%d.nfx`, `temp.nfx` for -1 (`0x1d6a30`), under
  `movies/filmroom/` (`0x294e28`).
- **Inside Vol. 2** (classes 102..104, layouts 057..059, backdrop 0x12,
  `dvdcontent.txt`/`dvdnewin2.txt`/`dvdbobbito.txt` = locale files
  0x45..0x47): each row's label is its element's prop 17. The main page
  (`0x289f98`): What's New In 2 → class 103, The Moves → class 101 (the
  edit-player screen in its moves mode, `FUN_0028b768(.., 5, 0xc)`),
  Bobbito Garcia → class 104, Vol. 2 Trailer `xmt0018`, SSX 3 Preview
  `xmt0023` (an SSX Tricky trailer). What's New (`0x28aee0`): Play All
  ("xmtall"), New Game `xmt0024`, Take On The League `xmt0025`, Be A Legend
  `xmt0026`, Vol. 2 Rewards `xmt0027` (the rows Intro Video, Multiplayer
  and Music are in the text but not the layout). Bobbito (`0x289100`):
  New York City `xmt0022`, Vol. 2 `xmt0021` ("Interview 3" has no row).
  A PAL console plays `xmtplNN` instead (the movie task's +0x30 flag).
  The movie to play goes in `DAT_003fdf44`.
- **Create a player** (the build tables at `0x4602a0`: {count, pointer}
  for men (4 builds at `0x460368`) and women (2 at `0x4603a8`); each
  build is 16 bytes: u16 price (0/200/300/400, women 0/200), u16 size
  count (4), u16 first preset, u8 preset count (9), then a pointer to its
  four sizes {u16 price 0/200/300/400, u16 height, u16 weight} and one to
  its nine "Choose Baller" presets {u16 price 0/50/50/100/100/150/150/
  200/250, u16 preset id 0..53}; `FUN_001e1e68`/`FUN_001e1fa0` read
  them). Heights: men's builds 69..81, 70..82, 71..83, 72..84 in 4"
  steps; women's 65..77 and 67..79. The u16 pairs at `0x4604c8` ({price,
  id}, 20 rows of 25..125) are what `rating_cost` reads.
- **Created players' moves in play:** `FUN_001d8778(rec, kind, slot)`
  reads slot fields 0x44+ (6 shots), 0x4f+ (11 dunks), 0x5a+ (11 dribble
  moves). `FUN_0013a1f8(sets, kind, set<<16|player, slot)` turns a slot
  into a move id: team 0x25 (created) from its own slots, team 0x24 from
  `FUN_001d9e60`, the street legends (records 0x91..0xb5) from their own
  move for slot 1, everyone else from the move-set tables (`sets+4`
  dunks, `sets+0xc` dribble moves, `0x43d6c0` lay-ups, 11 per set);
  `FUN_00139d38` gives a move id's value class (1 << its level) for the
  trick ladder. Edit Moves' categories are by kind and level
  (`editplyr_moves.txt`); a replaced move stays unlocked.
  **Moves in play (traced and wired 2026-10-12, shots, dunks and tricks):**
  `FUN_0013a1f8(sets, k, set<<16|player, slot)` returns a move's script
  id for kind k: k 2 = shots (a created player's fields 0x44.. = 68..73,
  six slots, `XDBMOVE` kind 0 "Shots"; everyone else `0x43d6c0` =
  {4, 7, 5, 6, 3, 1}), k 1 = `XDBMOVE` kind 2 (fields 0x5a.. = 90..100,
  eleven; others `sets+4` = `0x43d498` {20, 17, 19, 22, 12, 16, 56, 57,
  63, 66, 69}), k 0 = `XDBMOVE` kind 1 (fields 0x4f.. = 79..89, eleven;
  others `sets+0xc` = `0x43d5d0` {48, 21, 0, 35, 58, 20, 23, 2, 15, 37,
  39}); `FUN_00139850` builds that one shared set (only set 0 exists):
  it walks `0x43d498` (74 ids, stamped value 0), `0x43d4c4` (18, value 2),
  `0x43d508` (17, value 1) and `0x43d5a4` (24, mixed) into a 300-int
  flag plane, and `0x43d5d0` (60 ids), `0x43d6bc` (33), `0x43d5fc` (9),
  `0x43d61c` (9) into a 240-int plane of 60-id rows; in each plane the
  set's first eleven ids are stamped `1 << level` from
  `PTR_DAT_003d17ac`/`PTR_DAT_003d17a8` = `0x43d408`/`0x43d3b0`, whose
  eleven rows are {the two-turbo pair `0x3c000/0x1c000/0x2c000/0x34000/
  0x38000/0xc000/0x14000/0x24000/0x18000/0x28000/0x30000`, level} —
  levels {4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2}. `0x43d460`'s six masks are the
  last six (slots 5..10), `0x45f9c8` = 94 ints maps an `XDBMOVE` **row**
  (not its move id) to the script id (shot rows 0..9 → 0..26 — the
  27-script space; dunk rows 10..53 → 0..59; trick rows 54..92 → 0..72);
  `XDBMOVE` itself is 93 rows: kind 0 = rows 0..9 at cost 50, kind 1 =
  rows 10..53 (16 Lvl 2 + 23 Lvl 3 + 5 Lvl 4), kind 2 = rows 54..92 whose
  move ids 44..77 duplicate at rows 84..88.
  Street legends (records 0x91..0xb5) have their own move for slot 1;
  The generic streetballers (field 3 = 0x24) get fixed rows from
  `FUN_001d9e60`: shots `0x45fde0` {0..5}, dunks `0x45fdf0` {0x31, 0x1a,
  0x1b, 0x30, 0x1c, 0xa..0xf}, dribble moves `0x45fe08` {0x59, 0x44..0x47,
  0x36..0x3b}, u16 `XDBMOVE` rows through `0x45f9c8`; the created players
  (0x25) read their own rows and no legend override; everyone else gets
  only the legend override and the defaults (`MoveScripts::slot_script`,
  2026-10-15).
  `FUN_0013a1f8`'s inline switch on the legend id (0x91..0xb5) names their
  slot-1 dunks/tricks directly (ids 0x3c, 0x12, 0x44, 10, 0x36, 0x17 for
  shots; 0x35, 0x38, 0x1b, 0x2c, 0x2e, 0x2b, 0x1d, 0x2f, 0x2d for dunks).
  The two-turbo shot (`FUN_0016b4b8`) takes its slot from which two of the
  four turbos are held (the six masks at `0x43d460`: 0xc000, 0x14000,
  0x24000, 0x18000, 0x28000, 0x30000) and runs `PTR_DAT_003d8fd8[id]`
  (27 shot scripts); the lay-up (`FUN_001773b0`) uses k 0's slot 0 and
  plays ids 0x30..0x33 as inline bodies `0x447ed5` (`bbdunk_1`),
  `0x447c1d`, `0x447d0d`, `0x447de5` (`bbdunk_2h_2f_rev`), anything else
  `0x447bbe`. **Corrects** the earlier reading of `XDBMOVE` kind 0 as
  lay-ups: kind 0 are the "Shots" of Edit Moves (all level 2), kind 1
  dunks and kind 2 tricks, each by level.

  **The script-body registry (dumped 2026-10-12):** `.data` `0x3e7244`
  holds 631 `{name cstring, script body}` pairs (the table the decomp
  walks; it ends right before the 48-op handler table at `0x3e8610`).
  Names include most move families in registry order: standing/dribble
  bases 82..148, jukes 151..226, pickups/reactions 227..290, dunks-on
  280..288, shots 295..327, tips/mid-airs 395..401, dunks 399..491, block
  families 484..497, rebounds 501..515, passes 549..581. Every shot and
  dunk script (and the shots' 27) lives here, which is the walk's ground
  truth.

  **The set-builder's stride records (dumped 2026-10-12, corrected the
  same day):** `.data` holds two tables here, and the earlier single
  stride-0x14 walk of the region (`0x3d7200..`) read both at once — that
  is where the "only 23 jukes in `.data`, the remaining trick bodies
  still to locate" note came from, and it was wrong. The tricks' table
  is stride-0x0c rows at `0x3d7234`, **indexed by script id** (0..74):
  {state mask, body, legend} — legend 0xffffffff for plain rows,
  0x9e..0xb5 on the legend rows; **every one of the 75 internal trick
  ids has its body** (id 20 is `juke_slipslide_revdribble`), the jukes'
  state masks 0x5..0x4f among them. The stride-0x14 rows are corrected from original native1695bc/169994:
  99 candidates at `0x3d7b28..0x3d82e4` as
  {body, legend, selection mask, family, id}. Native169c14/169c28 stores
  family+0xc into actor+1e4 and id+0x10 into actor+1e8 (or+26c).
  The previous `0x3d7b24` start mislabeled each body with the preceding
  row's id. Family0 has66 candidates covering all60 dunk ids, including
  47 at449f89 (`dunk_1h_2f_wilt`) and59 at44a7b3
  (`dunk_2h_2f_touchheel_spread_eagle`). Family2 lay-up candidates are
  excluded from the dunk list. Selection-mask lower bits gate direction
  and native distance bands; upper bits gate actor25c, candidate mode and
  probability buckets. Legend+4 positive values require the matching player;
  these filters are retained metadata, not executed by the clip inspector.
  All99 native tuples and their original digest are retained in
  `%TEMP%/opencode/street-re/mode-school-shot-metadata-probe-20261013/`;
  the real-ISO parser regression checks every family0 tuple and strong
  id/body/family anchors plus a digest across all99 complete tuples.

  **Dunks in play (wired 2026-10-12):** the eleven default dunks'
  `0x43d5d0` ids ({48, 21, 0, 35, 58, 20, 23, 2, 15, 37, 39}) walk to
  their stride-record bodies and each body's `0x8e` blend targets; the
  result is `street_assets::scripts::DUNK_BODIES`/`DUNK_BLENDS`
  (per-slot blend clips, e.g. slot 8's `dunk_19` blends
  `dunk_1f2h_singlearmswing_notra_r` → `dunk_2f1h_deebrown_notrans_l`),
  keyed on the two-turbo pair (`DUNK_MASKS`/`DUNK_LEVELS` from
  `0x43d408`). `game::clip::move_clip` plays a body's blend clip; the
  set tables `moves::DUNKS_SET` (0x43d5d0) / `TRICKS_SET` (0x43d498) are
  now in with their **corrected** addresses (the earlier note had them
  swapped).
- **Move scripts** (the animation VM, `FUN_00193440`): a byte 1..0x7f
  advances that many frames (`FUN_0019a658`); 0x80..0xaf are commands
  through the table at `0x3e8610` (48 handlers); several call game
  callbacks through `0x3e86d0` with {id, a, b, c} (and a jump target for
  0x84/0x85/0x87). Operands are read byte by byte (unaligned). 0x82 jumps
  (u32 address), 0x91 sets the low flag bits (u32), 0x9b/0x9c set and
  clear player flags (u32), 0x93/0x95 pick the hand (bone 3 or 7) by
  position (optional 0xf1 prefix, a selector 0xec/0xf2/0xf3/0xf4/0xf7/
  0xf8, two bytes), 0xa6/0xa7 set/clear three player bytes. **0x8e blends
  to a clip:** prefix bytes (0xf2 hand, 0xf8, 0xec, 0xf5, 0xf4, 0xf3)
  then a signed byte (frames), a u32, a u32 *animation record* (an index
  of the 1,144-record table at `0x3de8b8`, -1000/-2000 special), and an
  alternate record per prefix. Script 0, for instance, blends to records
  307/306 (`jumpshot_l_2`/`jumpshot_l_1`). The prefixes are tested in
  order by the handler (`FUN_001978b0`, verified via Ghidra MCP
  2026-10-13): 0xf2 takes its alternate when the animation's `+0xc0`
  field is 2, 0xf8 when `+0x25c` is 2, 0xec on a facing test
  (`FUN_001404e0` plus the `+0x38` float signs), 0xf5 on a VU distance
  test, 0xf4/0xf3 on handedness (bones 7/3 through `param+0xb0`). The
  inspector keeps every alternate (`Blend.selectors/targets`);
  `move_selection` still plays the primary — evaluating the conditions
  needs animation-VM state and stays open.

  **The 27 shot scripts (2026-10-12):** `0x3d8fd8` holds 27 `.rodata`
  script-body offsets (the last u32 is a f32 of 8.0, not a pointer, so
  exactly 27). Walking each body for `0x8e` blend targets already shows
  the blend record's *first* u32 is always 0 on the disc and its *second*
  u32 is the animation record id, which names the move's clip. The 27
  script ids' blend-target names are in
  `street_assets::scripts::SCRIPT_BLENDS`; the shot slots' script ids at
  `0x43d6c0` = `{4, 7, 5, 6, 3, 1}` are
  `street_assets::moves::SHOTS_SET`, so a non-created player's six shot
  slots replay their scripts' blend targets faithfully — what
  `game::clip::move_clip` (wired into `view.rs`) now plays for shots,
  dunks and tricks alike, a created player's equipped rows resolving
  through `0x45f9c8`. The shared
  set's dunk and dribble-move rows (`0x43d5d0` dunks, `0x43d498` trick
  rows) sit alongside as `moves::DUNKS_SET`/`TRICKS_SET`.

  **Dribble-move callbacks (surveyed 2026-10-13, `move_report`):**
  `0x83` dispatches through `0x3e86d0` (`FUN_00195a78`: `{id,a,b,c}`,
  first word the id). Every one of the 75 trick scripts calls callbacks
  7 (`FUN_00174e80`: `param+0xb0 = FUN_00154230()`) and 1
  (`FUN_001722c0`: Script-Manager 3..7 exclusion, else
  `FUN_00163d40(anim,0,1)`); shots add 4, dunks add 4 and 27, and six
  self-toss tricks (21, 23, 36, 60, 61, 69) also call 53
  (`FUN_001741a8` → `FUN_0014bff8`, the pass setup — a self-toss during
  the trick), trick 74 adds 4. Callbacks 4 (`FUN_00175248`:
  `anim+0xb0 = f(anim)+a & 0x3ff`) and 7 are animation-state writes;
  53 is the only ball-state callback in the trick set. The inspector
  (`street_assets::scripts::MoveScript::callbacks`) records every
  `0x83` id. The six self-toss ids reach play as follows: only id 69 is on
  the default dribble set (`TRICKS_SET`); ids 21, 23, 36, 60, 61 come
  through equipped `XDBMOVE` rows (row→script map `0x45f9c8`), which the
  sim never sees — it tracks (level, variant), not script ids.
  `FUN_0014bff8` (traced 2026-10-13: ball +0x100/+0xfc set, +0x180 = 4
  pass, timers zeroed, pass-audio class `DAT_003c6500` posted when both
  are set) needs the toss target plus Trick-action catch rules, both still
  to read — so no invented toss. Sim-expressible 53 remains open;
  1/4/7 stay animation-side.
- **Loading pictures** (class 44, `FUN_0029c310`; `XAFELOAD.BIN` is a
  BIG of 47 sheets, ten 128x256 strips each = 640x512): `FUN_0029a828`
  takes the first of the 25 legend cards (0..24) not shown yet (a bitmask,
  reset when all are), swapping in 25 (Michael Jordan) under one flag;
  front-end mode 0 shows 34 (the logo); the story modes alternate
  (`FUN_0029a8c0`) with tips/covers (35..46 are "STREET" magazine covers).
- **Completion boards:** class 59 (`nbatcend.txt`) and 60
  (`bealegendend.txt`, `FUN_00261c48`: the baller's name, "%1d - %2d"
  from story +0x28/+0x2c, the reputation name `reputation.txt` 1..5, and
  "%1d / %2d COMPLETE" street challenges).
- **Choose Players** (class 0x1d, layout blob 9, ctor `0x269b28`,
  backdrop 5, vtable `0x476760`; build `FUN_00269c08`): two player
  objects (`FUN_0022ff80`) at (118, 218) and (450, 218); the column
  heads "AWAY" (element 11, `chooseplyr.txt` 3) on the left and "HOME"
  (33, #4) on the right; the help "Choose your players." (#2). The
  per-side fill `FUN_0026a198(screen, side, player)` reads an element
  table at `0x476940 + side*0x2c`: left 55, 62, 54, 53 then the meters
  15, 7, 12, 6, 17, 8, 13; right 58, 63, 57, 56 then 37, 29, 34, 28, 39,
  30, 35. Line one is blank ("?" for a locked player), then the first
  name (field 2), the last name (field 1), "height weight" (fields 8, 9)
  in the smaller face; each meter (layout kind 20, 60x25) is set to
  `FUN_001d7f48(player, k)`, the skill's bar 0..6, in the order Shots,
  Dunks, Power, Blocks, Steals, Handles, Rebounds (`common.txt` 10..16,
  the label 33 below its meter). Built: `street-game/src/choose.rs`.
- **Match intros** (the intro task `FUN_00184bb8`): two staged scripts
  from `SCRIPTS.BIN` before tip-off. First the court's (`FUN_00184ed8`:
  the pair at `0x3db318 + court*16`, `IntCourt<n>a`/`b`, started with team
  1 in the first roles), then one queued about the away side
  (`FUN_00184f20`, the side's three records at `rec+0x9c+12`): the first
  streetball legend (record field 3 = 29) plays their boss intro
  (`0x3db3c8 + (record-145)*16`; record 146 on court 9 is `IntBoss05`
  itself), the first legend (field 3 30..35) theirs
  (`0x3db498 + (record-158)*16`), else a generic one from `0x3db2b0`:
  nine (`IntGen01`..`05` and the `B` halves) when a field-3 = 36 player is
  on the side (`FUN_00185138`), else all thirteen. Every list is a run of
  (offset, used) pairs in `.data`: `FUN_001851f0` draws among the unused
  ones and clears them all once each has played. The loader's third
  argument (`+0x40` of the script player) is the team taking roles 0..2
  (-1 none). Built: `street-game/src/intro.rs`.
- **Staged scripts' control and replay commands.** The script manager
  (`FUN_00335708`, task group 4) hands each category-1 record to one of
  seven listeners by byte 0x21 (`FUN_00335370` builds them; `FUN_003355c8`
  dispatches): 0 ball (`0x32f6f8`), 1 replay (three classes, one per
  opcode), 2 sound (`0x331630`), 3 script control (`0x3331f0`), 4 effects.
  Every listener but replay's fires only on the record's first frame;
  replay opcode 0 fires on its last (frame + duration), opcode 2 on its
  first, opcode 1 every frame of its span.
  - Script control 0 (`FUN_00333238` → `FUN_00336108`) sets manager
    +0x8c; the manager's step then calls `FUN_00335f48`, which resumes the
    task groups 0x1180 and stops the script. Most scripts stop this way
    short of their last frame (`GamDN1b` at 192 of 208, `IntGen14` at 168
    of 329). **Script control 1** (39 records on the disc) is verified
    left alone: its class `0x49c900` reads only the record's first word
    (`FUN_00333238*) and its step entries `FUN_00333288`/`FUN_00333290` are
    two `jr $ra` stubs — nothing but the record keeps happen.
  - Replay 2 (`FUN_00334d60` → `FUN_00336340`, not during a replay):
    state 7 for `duration` frames (+0x78 counts down): the task groups
    0x382 (0x1382 when the command's word 12 is 1) are suspended, so the
    world stands still while the manager plays camera id word 8 (and
    effects id 1) at frame + elapsed; `FUN_00336450` resumes them and marks
    the record done. Every gamebreaker has one: 60 frames on camera 1.
  - Replay 0 (`FUN_00334b30`; `FUN_00334c20`: not already replaying, not
    in the game's own replay, span under 301 frames): the game's replay
    manager replays the last `duration` frames (`FUN_00328fc0`) at word 16
    percent (25..150, else 100; `FUN_00329ac8`) and the script plays again
    from the record's frame on camera id word 8 (`FUN_00336188`, state
    5), stepping speed/100 frames a tick (`0x336868`). The manager's step
    returns as soon as the replay starts, so the stop command on the same
    frame runs after it. Every gamebreaker replays its span at its stop
    (40 or 50 %, camera 2 or 3); the legends' and bosses' intros replay
    their moment mid-script (25 or 35 %, camera 1). Replay 1 changes the
    speed while replaying (no shipped script has one).
  - Camera records carry their id in payload byte 36 (`FUN_00337718`
    enables one id); keys move only the camera they are rooted at.
  - Ball opcode 2 calls the actor's own shot (vtable +0x12c), so the
    basket counts when that ball goes in: the rebuild's arrival.
  - **Ball opcode 3 (corrected in the resumed pass):** `FUN_0032f8f0`
    resolves a **player**, not the ball, and dispatches player vtable
    `0x49cba0` +0x134 to `FUN_00334088`. The earlier `0x49c298` float-setter
    interpretation was the wrong receiver. Nine shipped records launch a
    defender vertically for their duration N, zeroing horizontal velocity.
    At 60 Hz (`FUN_0030f9a4`), acceleration is
    `f32(0x42009be3) * f32(0xbfa13ba4) / 60²` (about −40.5/3600), and
    initial velocity is `−(0.5*a*N² + y0)/N`. The player step
    `0x157480..0x15752c` adds velocity plus half gravity to Y, then gravity
    to velocity, clamping downward ground contact. `SequencePlan::pose_at`
    now samples this parabola in simulation, rendering, intros and replay.
    It does not move the ball. Putback finishes need no separate arc:
    regular putbacks release at the rim (`slam()` places the ball at
    rim + 0.3 — the jumping putbacker's hand is already there), and staged
    putback shots fly the staged ballistic solve like every other staged
    shot.
  Built: `street_sim::gamebreaker::Controls` (stop and freeze in the
  simulation), `street-game/src/intro.rs` (intros, replays).
- **The finishing animation VM** (staged ball opcode 1, `FUN_00334178`
  2026-10-13): it looks the actor's animation id up in the two 20-byte-stride
  rows at `0x415740` (22, one short list) and `0x414df0` (119), each
  `{animation id, body}` (decomp noise had the pair swapped; word +4 is the
  body), and starts a standard animation task (`FUN_0019c320` →
  `FUN_00193408`, `FUN_00193440`) over the body — the actor's own clip clock
  is replaced for its span. The interpreter (`FUN_00193440`) runs one byte a
  tick: bytes 1..0x7f advance that many frames, 0x80..0xaf through the
  48-entry table at `0x3e8610` (indexed `op − 0x80`), opcode 0x83 the game
  callbacks (`{id, a, b, c}`, through `0x3e86d0[id]`). The wait opcode 0x8d
  (`FUN_00197298`):
  - `8d ee B C` reaches **animation frame B** (absolute; relative from the
    frame it starts with when the clip is not yet running) over **C ticks**;
  - `8d B C` advances **B frames** over **C ticks** (a relative delta);
  - suffix `0xef` after the ticks (`8d ee B C ef D`) makes the span
    `state+0x20 + D` instead (the special waits; none on the disc);
  - the last frame before callback 21 is saved as the release moment
    (`0x6c` ticks, the animation's rate `0x68` = change / ticks).
  The ball leaves the actor at callback 21 (`0x3e86d0[21]` = `FUN_001729f8`,
  the shot release); for the gamebreakers' nine shot sequences (`DGB1_..`)
  that is 83..97 ticks after the opcode 1, with the animation's own frames
  in between the disc's spans carry. Hand attachment reads the staged
  animation's hand bones the way `FUN_00330f98` reads them: the state word's
  bits 2/3 pick bone 15 (left), 19 (right) or their midpoint, scaled by the
  model's `height_scale`. Built: `street_assets::scripts::finishers`
  (`release_tick`, spans), `street-game/src/staged.rs` (`add_finishers`
  pushes a shot cue at the release, `attach_hands` the hand tracks),
  `street-sim/src/gamebreaker.rs` (`HandTrack`/`FinishTrack`,
  `hand_position`). Every one of the 16 sequences scores one Level 2
  gamebreaker on both sides in `staged.rs`'s disc test.
- **"You're being challenged!"** (screen class 100, `challengercomes.txt`
  = text file 100 of `XLOC_GM`, ids 0x6400..0x6402): Start on a controller
  calls `FUN_002f3858(port)`. A port among the match's six seats
  (`[0x400058][3] + 0x24`) pauses (selector `0x4b34a8` = 0); any other
  port, when the front end's mode (`rec+0x78`) is 2 or 3 (Pick Up), sets
  the selector to 3, and the Pause task (`FUN_002e3690`) opens screen 100
  (factory `FUN_002e55c0` case 100 → ctor `FUN_00267668`, vtable
  `0x475a00`). Its build (`FUN_002676e8`) adds Accept Challenge and
  Continue, Continue first; its answer (`FUN_00267840`): Accept sets
  `0x3fa4b0` and leaves the game, Continue resumes. After the game the
  navigator `FUN_0029d920` sends an accepted challenge to screen 13 (a
  transition into 27) ahead of the usual 27/28. Built: `game::pause`
  (`Paused::challenge`, `is_challenger`); Accept returns to Choose Sides.
- **The post-game boards** (traced 2026-10-12; built in
  `street-game/src/boards.rs`). The in-game layouts' text ids are
  `XLOC_GM` files numbered by the table at `0x3ec2f0`: 0x57
  `common_gm.txt`, 0x58 `pause.txt`, ... 0x5b `pickuprewards.txt`, 0x5c
  `bealegendrewards.txt`, 0x5d `bealegendrewards2.txt`, 0x5e
  `topballer.txt`, 0x5f `ballerbreakdown.txt`, 0x60 `rundown.txt`, 0x61
  `milestone.txt` ... 0x64 `challengercomes.txt` (the GM files the FE set
  lacks, in archive order); id 0 is GM's own `common.txt`.
  - Order: the Pause task (`FUN_002e3690`, selector 1) opens **class 52,
    The Rundown** (ctor `FUN_002c7a98`, fill `FUN_002c7b48`), not a pause
    body. Its Continue (`FUN_002c8620`) goes to **38 Baller Breakdown** in
    Be A Legend (rec+0x78 = 5), else **37 Top Ballers**. 37's Continue
    (`FUN_002d4ed0`) shows **34** (the Pick Up / league hookups) when the
    results queue (`FUN_001de3e8`, +0x1c0c) holds anything in modes 2..4,
    else the milestone panel (87, when `FUN_001e0768` sees a bit at
    +0x1c38) or the end dialog. 38's (`FUN_00261758`) goes to **35, the
    development points**; 35's (`FUN_00262710`) to **36** (Be A Legend's
    hookups) when more is queued. 34 and 36 share `FUN_002c5708`'s
    Continue: another page of hookups while `DAT_003fc674` holds the next
    item, then 87, then the end dialog (`FUN_0025c860`: Pick Up REMATCH /
    QUIT TO CHOOSE TEAM / QUIT TO MAIN MENU / QUIT TO REWARDS, REMATCH lit;
    the League Challenge "Continue with region?"), Be A Legend the nickname
    panel (95, `FUN_001eb150`).
  - Stats: `FUN_001871e0(stats, side, team?, stat)` — 0 points, 1 dunks,
    4 rebounds, 5 steals, 6 blocks, 0xb trick points, 0xc Level 1 and 0xd
    Level 2 gamebreakers; the third argument 1 gives the side's total.
    Side 0 (the home side) is drawn on the right, side 1 on the left.
  - 52 (`FUN_002c7b48`): logos (kind 24, ids 4 left / 15 right; the team
    words at game record +0x8c / +0x90), city and name over two lines
    (`FUN_001ed7d0`, `FUN_001ed890`: ids 2/10 and 13/21), the score in font
    6 (ids 5/16, left out when rec+0xe8 is 1 or 6, the Trick Point Challenge
    and Gamebreaker Wins), and the totals: trick points, dunks, blocks,
    steals, rebounds, Level 1 and Level 2 gamebreakers. "2 POINTERS"
    (`rundown.txt` 3) is on no layout.
  - 37 (`FUN_002d42a0`, per-slot record `FUN_002d52e0`): each side's top
    baller by points + dunks × 0.25 + blocks × 0.75 + steals × 0.5 +
    rebounds × 0.5 (trick points alone when rec+0xe8 = 1), the first of
    equals; the name as field 2 + "\n" + field 1; the portrait (kind 25,
    `FUN_00223dd0`: `XAPORTS` entry = the record's field 14; created
    ballers' are built from parts); points, dunks, blocks, steals,
    rebounds, trick points. POINTS (element 17) is blanked in those modes.
  - Kinds 24/25 (`FUN_00223698`): a 66x65 tile (`XAFESCRN` sheet 0 image
    4, frame 1), the picture at the tile's size, a 74x73 frame (image 5).
  - 38 (`FUN_002610d0`): the story block's baller (found among the six by
    record), name in font 3, the team as "%1S %2S", the six stats on the
    bars of `XACOMMON` sheet 0 image 6 frame 0 (kind 3, ids 7..12), and an
    FEPlayerObject (`FUN_002db8b8`) standing on the left.
  - 34 (`FUN_002c5cf8`) and 36 (`FUN_00262ba8`): a stack of label (font 3,
    0xdcdcbb) over value (font 2, 0xfbac00), 4 apart and 8 to the next,
    centred between the hookups line and the Continue button: Court
    (+0x1c14), Legends (+0x1c10, the team's players "A, B,\nC"), league
    Legend Jersey (+0x1c1c..24), then in 34 Boss Move (+0x1c28..30), Game
    Completed (+0x1c34 = 0xa7) and "%1d Reward Points" (+0x1c0c); in 36,
    when no Level Boss (+0x1c18) is queued, Boss Move, Game Completed,
    Street Champ Jersey (block+0xcdc), "Progress: %1d" (block+0xccc) with
    "(Pick Up Win)" / "(Tournament Win)" / "(Be a Legend Win)", and the
    reputation (block+0xcd4: "Progress" + "(New Reputation Level)", else
    +0xcd8: went up / down); with a Level Boss, his name instead.
  - The panel every board sits on (kind 28, `FUN_00211278`, placed by
    `FUN_00211760`): a translucent black body (`XACOMMON` image 3 frame 0,
    alpha 0x6e, under the element's 0xa0ffffff) reaching 17 above the
    element, its top right corner cut at 45 degrees (frame 1, 40 square);
    a 12-wide navy (0x080852) border on the edge, a 3-wide 0x98a8b8 line
    7.5 outside, a 3-wide 0x424a9c line 7.5 inside with a 20-tall band
    along the top, and three of image 1's frame 3 stars on the band.
    Sprites read their frames from the SSH image's `0x7c` block (four
    corners per frame, `FUN_00210048`). A layout lists its elements front
    to back.
- **Careers** (class 54, `FUN_002ce270`): ten Be A Legend careers at game
  record +0x33f4 (`FUN_001eb4a8` occupied, `FUN_001eb4d8` its baller's
  name, `FUN_001eb5f8` complete, `FUN_001eb6e0`/`FUN_001eb798` the
  progress pair); the one being played is the story block at +0xf10
  (`FUN_001e7040` writes it back to its slot).
- **Reward Points per game** (`FUN_001ded40`, the post-game dispatcher;
  the FE-wide mode at game record +0x78: 2/3 Pick Up, 4 the league
  Challenge, 5 Be A Legend): for each person on the winning side,
  `FUN_001de528(rec, 0, rp)` with rp by difficulty (+300 & 3: 0, 1, 2):
  Pick Up 50/100/150, the League Challenge 100/200/300; finishing the league
  Challenge (`FUN_001deac8`) 1000/1500/2000. Be A Legend pays
  development points instead (`FUN_001e9b90`). The notices land at
  +0x1c0c (rp), +0x1c10 (team), +0x1c14 (court), ... (-1 none).
- **Street Milestones** (`FUN_001e0910(rec, n)`, n 0..18, bits at
  +0x1c38): milestone n unlocks card `0x460050[n]` = 46 + n, the jersey
  cards. The League Challenge sets 5..9 for a region won without a loss
  (the region's +0x48 bit 6); reaching reputation levels 1, 2, 4, 5 sets
  14..17. `milestone.txt` 3..21 describe all nineteen.
- **Street challenge rules by court** (`streetchallengeintro.txt` 5 +
  court for the story, 16 + court for the name): Broad Street (1) trick
  challenge, Foster Beach (2) dunks only, Soul in the Hole (3) old
  school, Mosswood (5) gamebreaker wins, Greenlake (7) no gamebreakers,
  Lincoln College (8) league scoring against a league team, Rucker Park '78
  (10) the Legendary Challenge.
- **Cameras** (`FUN_003038b0` constructs four camera objects): saved type
  values 0..4 map to modes 0, 1, 2, 3, 5; Behind uses table mode 4.
  The six `0x34`-byte rows at `0x498478` supply x bound, signed x offset,
  eye height, z scale/offset, and an unresolved pose output word.
  Integer words +32/+36 are not pitch angles. All six eye mappings now
  use `FUN_002f7c38`, including Behind's fixed yaw and ground-facing
  pitch computed before its asymmetric x clamp.
  `FUN_003052a0` selects ball+0xf4's holder position when held, otherwise
  ball+0x30. Raw EE `lq`/`sq` prove these are full XYZ copies despite
  the patched decompiler's eight-byte rendering. Actor+0x34 is vertical
  position, mapped to `Player.height`, and release clears the owner.
  Side bytes initialize +1/-1 for teams 0/1. Native controller velocity,
  acceleration, lens words and mode transitions remain to reconstruct;
  exponential easing is still a Rust proxy. A live Rucker render confirms
  visibility, not original-runtime parity. See `CAMERA_RECOVERY.md`.
- **Trophies** (`poster.txt` 16, 21, 33..40, 200..203): Street School,
  League Challenge, Be A Legend, the original game; the pictures are
  `XAFESCRN` sheet 0 image 0x0b, a 2x2 grid of 108x80 pictures.
- Front-end screens name their own movies (title board, panels, loading
  loops): e.g. `0x29dfb0` uses `xmt0008`, `xmtfblur`, `xmt0006`, `xmt0011`;
  `0x2c3948` the `xlp0009`, `xlp0012`..`xlp0015` loops; the credits
  (`0x283b40`) `xnufx`. Which screen is which is not mapped yet.

## 6. Audio

- `CSCRIPTS/PATHSONG.CFG` lists the in-game music: the folder, the artist's
  intro speech flag, and the title, artist and album.
- Each folder in `/MUSIC` holds `MAIN.MUS`, `TRICKS.MUS`, `WHOOSH.MUS` and
  an `.MPF`. The MPF is a PathFinder interactive-music map (`PFDx`),
  decoded with the events the game fires in `docs/FORMATS.md` (in-game
  music maps). The library is at `0x342000..0x346600` (`0x344ee8`
  add map, `0x345ad0` event, `0x345840` set value, `0x346238` stop,
  `0x345998` clear the queue, `0x346390` volume); the manager loads
  `main.mus` as track 0, `whoosh.mus` 1 and `tricks.mus` 2 (`0x1abf70`).
  Songs rotate. The settings `audio_rotate_game_songs` and
  `audio_song_change_on_gb` (registered at `0x1a2950`; values at
  `0x3e9c28` and `0x3e9c48`) are both 1 and no script changes them.
  When a gamebreaker ends (`0x1a6b80`, from the basket, the lob and one
  more slot) the manager (`0x1ab760(mgr, side, 0)`) gives the other side's
  full meter its ready theme (the side's own meter is asked only when the
  third argument is set); else, when the VBlanks since the song opened are
  over `fps * 120.0` (`0x1ab8a4`), it stops everything and opens the next
  song (`0x1aaae8`, `0x1aa8d8`: events 0 and 1 at once); else it stops
  everything and the groove resumes. Pocket and cancel (slots 70, 71) never
  switch. Slot 41 (`0x1a7370`, posted when a player's flag bit 14 at +0xa4
  turns on, `0x156f48`) switches once the groove has played over
  `fps * 240.0` VBlanks (`0x1ab958`). Unported gates of `0x1ab760`: profile
  +0xe8 = 3 and manager +0x28 = 7 skip it, Street School only stops, and the
  game's virtual +0xbc keeps the song.
  The next song is the next in the list (`0x1aabc8`; songs switched off in
  the profile's mask at +0x134 are skipped). `song.rs` does this; slot 41
  stands as a change of hands.
- Front-end songs: `music/FE_Music/*.asf`.
- Sound effects: `SFX/*.ABK` banks, with streamed `.AST` files for the
  crowds of each court. `GAME_EVT.AEM` and `FE_EVT.AEM` are AEMS
  event maps (magic `AE`); the scripts and runtime are in `FORMATS.md`
  (AEMS scripts) and `street-audio/src/aems.rs`, played in a match by
  `street-game/src/sfx.rs`.
- The audio listener (vtable `0x45c3a0`, eight bytes per slot, the code
  word second) answers the game's event classes (including `0x3c6570`,
  `0x3c6580` and `0x3c6590`, posted through `0x104948..0x106248`). What each slot fires
  (`0x19e840` events, `0x19ee90` loops started/released), with the
  position it measures from (ball `0x1a7438`, player `0x1a74b8`, hoop
  `0x1a7458`, none `0x1a7420`):

  | Slot | Code | Moment | Events |
  |---|---|---|---|
  | 2 | `0x1a61b8` | trick move | 0x4a kind 7 + level below level 3, else a music stinger; 0x47 for four signature moves |
  | 4 | `0x1a4928` | shot leaving the hands | 0x50 every other time; 0x4a 13 for a power dunk, else 10 |
  | 9 | `0x142ff8` | a pass (class `0x3c6500`, `0x14bff8`) | nothing: an empty handler |
  | 11 | `0x1a4738` | a launch (class `0x3c6530`, the shot launcher `0x14aaf8`, the ball's kind at +0xc: 2 a jump shot, 5 a lay-up) | kind 5: 0x4a 6; kind 2: 0x3f for flights over 2 frames, player line 14; 0x47 4 |
  | 13 | `0x1a5a78` | a trick move lands (ring 10/11, +crowd 0x29 + taunt); a dunk release (`0x14bf28`, class `DAT_003c6540`) | 0x4d 4 |
  | 15 | `0x1a63e0` | held-ball steal / opponent self-flight catch (`0x14cb28` -> class `0x3c6570`) | 0x49 |
  | 16 | `0x1a4ce8` | shot contact / block (`0x14cb28` -> class `0x3c6580`) | 0x39, 0x50 |
  | 17 | `0x1a65c0` | loose-ball pickup / rebound (`0x14cb28` -> class `0x3c6590`) | (speech) |
  | 21, 18 | `0x1a4ee0`, `0x1a5f48` | catch | 0x38 |
  | 22 | `0x1a5d10` | the tip-off's take: a ball in state 7, the referee's toss (`0x14cb28` -> class `0x3c65e0`) | crowd 0x25; clears the speech manager's +0x68 |
  | 24 | `0x1a4f90` | rebound | 0x3a |
  | 26 | `0x1a3b10` | basket | 0x48 (dunks, by power 100..900), 0x41/0x3d (game over), 0x4d 9/0/1, 0x3e |
  | 29 | `0x1a3a30` | ball on the rim (`0x152418`, `0x1526d0`) | 0x36 by speed |
  | 30 | `0x1a46c8` | ball on the floor (`0x153278`) | 0x44 by speed |
  | 31 | `0x1a39a8` | ball against the ceiling (`0x153428`) | 0x3b |
  | 37 | `0x1a3aa0` | ball in the net | 0x3e by speed |
  | 39 | `0x1a5d50` | dunk slam | 0x40, 0x51 |
  | 40 | `0x1a6f38` | play opens | 0x4a 14, music event 1 |
  | 42, 43 | `0x1a4fe8`, `0x1a5050` | rim, swish (at the hoop) | 0x43, 0x45 |
  | 44 | `0x1a50b8` | dribble | 0x44 |
  | 48 | `0x1a5bd0` | shot clock second | 0x4d 10 under 9 s |
  | 52 | `0x1a6d88` | shot clock violation | 0x4d 11 |
  | 53 | `0x1a6fa8` | the game's reset (`FUN_0013d388`; see "The audio's generator, the commentator's duck and the speech clock") | loop 0x21 released, the listener and the speech manager reset |
  | 57, 61 | `0x1a70a0`, `0x1a56b0` | crowd held / play starts | loop 0x20 started / released |
  | 58, 59 | `0x1a5410`, `0x1a54d8` | a break (replay) starts / ends | music event 11, 0x4c 0, loop 0x23 / music 2, 0x28 again, 0x42 |
  | 64 | `0x1a3130` | gamebreaker goes up | 0x4c 1 or 3, loop 0x21 |
  | 66, 67 | `0x1a3638`, `0x1a3308` | meter full / meter drops | 0x4d 2 or 0x12, loop 0x22 / 0x4d 3 or 0x13 |
  | 68 | `0x1a3578` | trick points | 0x4d 6 over 5,000, 7 over 12,000 |
  | 70, 71 | `0x1a3868`, `0x1a3498` | pocket, cancel | 0x4d 0x10; 0x3d; loop 0x22 released |
  | 75 | `0x1a7398` | a dunker on the rim | 0x36 |
  | 77 | `0x1a5120` | a sneaker squeak | 0x3c, sometimes 0x46 (the court's) |

  Slots 6, 7, 8, 12 and 74 are wired in `sfx.rs`: exertion on releases
  and blocked jumps, sweeteners 11/12 on swings and reaches, body kind 0
  for a stagger and kind 1 for a fall (`FUN_001a5288`, knock level − 1;
  level 2 also adds sweetener 5). Steals and blocks already sound; the
  earlier claim that their sounds need animation callbacks was a misreading
  of `0x331630`, the staged scripts' sound command. Slot 13 (`0x1a5a78`)
  fires HUD 0x4d kind 4 on both paths — trick lands (scoring ring 10/11)
  and dunk releases (shot setup `0x14bf28`, class `DAT_003c6540`,
  conditional on the dunk-release flag) — and its crowd 0x29 + speech
  only when the event word at +8 is nonzero (the trick value on the ring
  path, 0 on the dunk post): tricks get the gasp + taunt, dunks the sting
  alone. All wired in `sfx.rs` (`Event::Trick`, `Event::Slam`).

  **Floor contact, slot 76:** `FUN_00156f48` posts class `DAT_003c6930`
  when animated joint 10 falls from above 1.5 to at/below 1.5, excluding
  the states at `AIGame+0x424+0x50` 3..7 — proven the Script Manager's own
  state (the AIGame constructor `FUN_0013c4e8` stores `FUN_00335708`'s
  result at game +0x424, replay manager at +0x420, AI script handler at
  +0x428). The event float is previous joint Y − current joint Y.
  `FUN_001a74f8` takes its absolute value ×500 (`cvt.w.s`, which
  truncates on the EE), capped at 1000; this is impact strength, not
  distance attenuation. The crowd call `FUN_001a75a8(…, 0x29)` also makes
  the spectators gasp; `sfx.rs` pushes that reaction to the crowd's
  director. `FUN_001a5348` fires body
  0x4f kind 2, crowd 0x29 (`FUN_001a75a8`) and exertion 0x50. `sfx.rs`
  now observes posed joint 10 for all ordinary actions (dives and
  recovery included; the Phase::Live gate is dropped) and stays silent
  through staged sequences, intros, replays and pauses — all of which run
  under the manager's script/replay/transition lifecycle. Closed:
  staged contacts are suppression, not missing pairing.
  Tests verify actual stumble clips cross the threshold, dives sound and
  ordinary locomotion stays silent, and the disc's
  audio maps produce the expected voices (Street School has no 0x29 voice).
  The old universal event-class formula was incorrect: slot 74 is
  `DAT_003c6910`, slot 76 is `DAT_003c6930`.
- **Animation state records** (`<anim>_X`, `FnRawStateChan`, type 16,
  vtable `0x4ac3e8`): u16 type, u16 flags, u16 frames, u16 key count,
  u8 field count, u8 key stride, then one u16 per field (low byte: the
  destination offset in a 15-word state block; bits 11..12: its size − 1
  in bytes; bits 13..15: the stored width — 0 one bit, 1 two, 2 four, 3 a
  byte, 4 a u16, 5 a u32), padded to four bytes, then the keys: an f32
  frame and the values packed from the most significant bit. The decoder
  is `FUN_00389190`, the sampler `FUN_00389310` (the last key at or before
  the time). Not decoded by `street-assets` yet. The match starts with 0x28 and loop
  0x20 (`0x1a1110`); the crowd stays down until slot 61 releases it.
- Speech:
  - `SPCHHDRS.BIG` is a `c0 fb` archive of commentary headers by category:
    Block, Dunk, Intro, CourtIntro per court, MissedDunk, Move, nickname
    actions and intros, Outro, Shot, Steal, Turnover, Tutorial.
  - The audio itself is in `SPCHDATS.BIG`.
  - `PLYRHDRS.BIG` (a `c0 fb` archive) holds each voice's lines:
    `action\NNN.hdr` and `shot\NNN.hdr` for voices 1..16 and the
    streetball legends' 145..151. The speech manager (`0x1ac6f0`, with
    `players.evt`) gives each player a voice (`0x1acb68`): a legend's own
    from record field 7; else, by skin tone (field 7, 1..6), the first of
    six voices nobody on the court has (tables at `0x45d6b8`; picks 17..20
    are 13..16 again); created players (field 3 = 37) by face (field 32,
    `0x45d748`) and sex (field 12). A request (`0x1ad068`: the player, a
    priority, a bit `1 << category`, a mood) plays a line of that category
    as event 0x1f `[angle, distance, stream]` (`0x1ac9f8`, via
     `DAT_004b321c` through the poster chain). Requests stay silent while
     the Script Manager runs scripts — `0x1ad068` checks its states 3..7
     first, the same gate as the floor-contact poster — plus two globals
     (`DAT_003e9ae8`, `DAT_003e9ba8`) and a per-manager word at +0x17;
     `sfx.rs` wires the script gate (`say` returns under a staged
     sequence; replays were already gated at each call site). The game maps
    (voice, line) to the stream with per-voice decision trees
    (`FUN_001af230`: a 151-entry jump table at `0x45D970` on voice − 1,
    each tree binary-searching its key; leaves return absolute stream
    indices — `jr ra; li v0,N`, e.g. voice 1's first lines 3258/3259);
    The key is the action header's big-endian u16 start multiplied by
    0x100, passed unchanged through `FUN_001ac9f8`/`FUN_001af1e8`.
    `street_audio::speech::PlayerSpeech` now uses the exact per-voice
    results for all 3,196 action lines. Both Luna and the parent independently
    emulated every raw original-ELF tree/key with branch delay slots;
    every result agrees. Disc checks validate each unique index against
    the 3,969-stream AST and nonempty metadata. The parallel shot tree
    at `FUN_001c4b20` resolves another 773 lines; both catalogs cover
    every AST stream exactly once. Action descriptor 0x2002 uses
    category/request-word-2 and second-metadata/request-word-4 masks,
    now wired with native RNG/history. The middleware itself (queue,
    chooser, header table, partitions, gates) was re-read on 2026-10-15
    and is ported; see `SPEECH_RECOVERY.md`, "The speech middleware,
    re-read". Correction: `param_1[0x17]` is the speech pause flag, and
    `DAT_003e9ae8`/`DAT_003e9ba8`/`DAT_003e9bc8` are the debug switches
    `audio_no_sound`/`au_no_player_speech`/`audio_no_commentary_speech`.
  - The categories the listener asks for (raw `jal 0x1ad068` sites): 0 a
    rebound pickup (the player), 1 the same, every other time, a teammate
    (slot 17), 2 five seconds on the shot clock (the offence's third
    player), 3 a trick move lands, 4 and 8 a shot clock violation in turn
    (4 also slot 27's missed shot), 5 a teammate's dunk, 6 slot 72 (not
    placed), 7 the shot clock at 21 (the defence, `rand() & 1`), 9 a block
    (slot 16), 10 a steal (slot 15), 14 a jump shot's launch (slot 11; slot
    27 asks it too with shot mask 2).
  - The listener slots by their event classes (each class's dispatch word
    names `listener + 8 * slot + 4`): 9 a pass (`0x3c6500`, empty), 11 a
    launch (`0x3c6530`), 13 a trick move lands, 15 a steal (`0x3c6570`),
    16 a block (`0x3c6580`), 17 a rebound pickup (`0x3c6590`), 22 the
    tip-off's take (`0x3c65e0`), 24 a rebound, 71 a cancel. The earlier
    note that steals and blocks make no listener sound was wrong: slot 16
    fires 0x39, the crowd's 0x27 and an exertion, slot 15 the crowd's 0x27
    and 0x49 (`sfx.rs`).
- vgmstream documents these formats and is a reference for decoders.

## 7. Gameplay values (traced 2026-10-10, session 2)

The scoring pipeline: each play raises an event; the game's scoring
dispatcher (`FUN_001322d0`, `game+0x15c` receivers) computes points and
queues the HUD pop-up (`FUN_002b87c0`, which reads the tables below), and
the meter handlers (`FUN_001442b8` and friends, `aigamebreakertask.cpp`
cluster) move trick points in and out of the gamebreaker meter
(`FUN_00143d38`, add; `FUN_001448e0`, set).

- **The base trick-point table** appears twice, in `.data` `0x48d3b0`
  (points, shown by the pop-up) and `.rodata` `0x43e1f8` (the same values,
  added to the meter). Index by event kind; 20 entries, id 19 = 0 is the
  end: 500, 1,000, 750, 250, 1,000, 1,500, 3,000, 2,500, 1,000, 25,000,
  500, 500, 500, 2,000, 25,000, 500, 3,000, 3,000, 5,000, 0.

  The kind's pop-up word comes from the parallel table `0x48c250` (u16 text
  ids into `overlays.txt`): kinds 2/3 are long/short range, 6 alley-oop,
  7 block, 8 steal, 9 gamebreaker, 10 pick, 11 pump fake, 12 dive,
  13 tip slam, 14 busted rim, 15 rebound, 16 trick counter, 17 passout
  specials, 18 cancel. 0, 1, 4 and 5 (trick, trick, fundamental, dunk) have
  no word; kind 1 names the move from the `0x3fb500` table instead.

  So inside the arc is **250**, beyond it **750**, dunks **1,500**, blocks
  **2,500**, steals **1,000**, picks/pump fakes/dives/rebounds **500**,
  tip slams **2,000**, trick counters and passout specials **3,000**,
  cancels **5,000**, and a gamebreaker on a busted rim is another
  **25,000**.
- **Lay-ups** are not a constant but a table at `0x43df38`: by finish
  1,000, 2,000, 3,000, 5,000, 7,500 (`FUN_001439c8`, indexed by the shot's
  own state at player+0x24c; the plainest shows as 1,000 in the pop-up).
- **Alley-oops** (`FUN_00143ba8`): base 3,000, and two style bits from the
  shot (`FUN_00171e28`) add 1,000 and 2,000: up to 6,000.
- **Gamebreakers** (`FUN_001439e8`): level 1 gives **25,000** trick points,
  level 2 **50,000** — exactly twice, as one tip says — rising to 100,000
  and even 250,000 in level-2 specials (five times the level 1, per the
  other tip). The pop-up reads the awarded value from team+0x58.
- **The meter** is a float at team+0xc. It fills to exactly **50,000**
  ("full" is `== 50000.0`, drawn as `points / 50000.0 * 167.0` on the HUD
  gauge); at game start it is pre-filled from a 0..99 scale in units of
  500 (`FUN_001457f8`: percent × 500). The same 500 scale says every
  500 trick points is one percent of the meter. The meter add
  (`FUN_00143d38`) also scales each fill: when team+0x10 is 0 the fill
  is `amount × team+0x40 × team+0x44` — the difficulty-blended tuning
  of "Difficulty" above times the 1.0 init field; when team+0x10 is set
  it is `amount × 50000.0 / team+0x10`, the team's own meter size
  rescaling the fill. The gamebreaker sequence's fill multiplies
  team+0x44 × team+0x48 instead.
- **Drains:** a *stolen* or *blocked* team loses meter points (~ the
  `0x43e248` table: block −2,500, steal −1,000, passout special −3,000)
  while the defender gains the matching fill. The block's 2,500 encodes
  right in the handler; steal is 1,000.
- **Comeback protection:** in `FUN_00143d38`, once a meter is over
  **42,500**, on Easy, for a CPU crew against a controlled crew whose
  score lead is less than three, the fill stays at its previous value if
  already above 42,500, otherwise stops at **42,500**. Raw IEEE-754 word
  `0x47260400` is 42,500, not the previously reported 45,000.
  During a gamebreaker sequence the *fill rate itself* is multiplied
  (team+0x44 × team+0x48).
- **Combos** are computed by the pattern matcher `FUN_00131970` over the
  last six events. The ring lives in the game object at `+0x4..0x9c`: six
  slots of 0x18 bytes going **up**, so `slot 0` (at `+0x4`) is the oldest
  and `slot 5` (at `+0x7c`) the newest. A slot is six u32s: class, extra,
  count, value, id, 0. The pusher (`FUN_00131788`) writes the new event
  into slot 5 and shifts each old slot one place down, stopping at the
  first empty slot. The dispatchers that push (classes in brackets):
  `0x1322d0` the baskets [0..7], `0x131ff8` steals/blocks/drains
  [8, 9, 14, 17], `0x132198` trick moves and fakeouts [10, 11], `0x132650`
  the ring clear plus 16, `0x132790` the 12 (count == 3 gate),
  `0x131f78` the 13, `0x131e78` the 15, `0x132278` the 18 (cancel),
  `0x132700` the 19, `0x132588` the 20/21 steals and trick counters (corrected 2026-10-17,
  with every dispatcher's classes: "The combo ring, the meter's listeners, the statistics and the shot clock"). Only the three scorer
  dispatchers (`0x1322d0`, `0x132588`, `0x132700`) then run the matcher;
  the other events enter the ring only. The match tests the pattern table
  `0x43c500`: **55 rows** of 0x30 bytes, six u32 kinds (−1 ends the
  pattern, −2 matches anything) then six u32 extras (−1 don't check),
  paired by index. Rows 0..39: for each class 0..7, five rows that ask
  for the class as the newest event with 0..4 wildcards older (row
  `k*5 + j` waits for the class with `j` wildcards behind it). Rows
  40..54: passout chains — all-19 rings (40..44), and 19×5+20 (45..49)
  and 19×5+21 (50..54), each shorter by one 19 per row. The matcher keeps
  the first matching row (its id to `game+0x148`, the last event's count
  to `+0x14c`) and answers the total to `game+0x150`.
  - The totals are `Σ ring.value × weight`, the weight read from the
    float matrix at `0x43be58` at `[(extra + base) × 5 + col]` where
    `base` is 22/27/32/37/39/45 for classes 0/10/11/17/7/1 and the class
    number itself for every other class, and `extra` is the slot's own
    second field — for the lay-up class 0 the finish state (`player+0x24c`,
    0..4), for the trick classes 10/11 the move's own value class
    (`player+0x248`, 0..4), for class 17 a drain amount, for the GB class
    6/7 0/2/4 plus one for a long-range shot. The **column** is made in
    the prologue as `(n + 1 − t4) << 2` where `n` is the match depth and
    `t4` the matcher's walk counter, halved by the count of consecutive
    9/10-class events; the per-column ramps run 1.25, 1.5, 2.0, 2.5, 3.0
    (most rows), with special families: rows 22..26 the lay-up finishes
    (0.75..2.5 up to 1.75..3.5), 27..31 the trick-value classes at a
    tenth to a ninth of the value (0.10..0.50 up to 0.50..0.90), 32..36
    fakeouts (0.75..2.5 up to 1.75..3.5), 37..41 drains (the standard
    ramp, then a mild 1.0..1.5), 45..46 alley-oops (the ramp, then
    1.75..3.5), and rows 0, 7, 10, 11 are all −1.0 (no contribution).
  - The class value table `0x4b7150` (22 ints, filled at boot by the reset
    at 0x132a40 — one of the functions that truncate in the decomp — from
    the same base table at `0x43c4b0` shuffled): 1500, 3000, 1000, 750,
    250, 2000, 25000, 25000, 750, 250, 500, 1000, 500, 500, 1000, 500,
    500, 3000, 5000, 2500, 1000, 3000. It is the fallback for any ring
    event pushed with value −1 (the lay-up, alley-oop and gamebreaker
    classes push their own computed values, as do the trick-move classes
    through `FUN_001438b8`, so mostly 3/4 (the range shots) fall back).
- **The same boot reset** writes the other scoring tuning:
  - `DAT_004b7168` = 0x2ee − 0xfa = **500**: added to a gamebreaker pass
    basket's value when it came off the foul (the dispatcher adds it when
    the story mode is not on, `game+0x130`+0x178);
  - `DAT_004b71a8` = another field × 2: the staleness bound
    `FUN_001318d8` compares the chain's oldest event count against (the
    "combo has decayed" rule);
  - `DAT_004b7140/44/48` = a meter field × 2, × 30 and × 10, and
    `DAT_004b714c` = **50000.0 / (field × 600.0)**, the fill rate the
    gamebreaker meters run at.
- **Trick-move values (resolved: they are not per-move at all).** The
  pick/fakeout scorer `FUN_001438b8` reads the ladder `0x43df20` indexed
  by the move's own value class `player+0x248` (0..4): 500, 1,000, 2,000,
  3,000, 5,000. A move whose clip id (player+0x264) already appears in the
  team's last-four ring (team+0x2c..0x38) is worth half, and half again
  per repeat; the value lands in team+0x24 (the pop-up's own value) and
  the first copy's id shifts the ring. `street-sim` now uses this ladder
  (its levels 1..5 = classes 0..4) and the same repeat halving.
  Re-read raw 2026-10-15 (`0x1438b8..0x1439c0`): the base is halved
  (`sra`) when actor+0x218 is 0; classes 0 and 1 skip the ring entirely;
  the discount multiplies by 0.5 per match, rounds up and clamps to 1.
  The ring is shared: the class-17 drain scorer `FUN_00143a88` (3,000, or
  5,000 when `FUN_00142038`'s state is 2) counts and pushes its own keys
  −3/−2 in the same four slots, so drains discount each other and shift
  trick ids out. The sim's drains do not touch the ring.
- **Difficulty** (`FUN_00143458`, re-read raw 2026-10-12; it truncates in
  the decomp). The corrected reading of the whole function, from its
  instructions: it takes **two scalar values** (f12/f13) and returns
  `(1 − w) × f23 + w × f24`, where the difficulty (story record +0x12c
  **& 3**, not +0x138) sets the mixes:
  - 0 Easy: `w = 0`, `f23 = f12` — the first value alone;
  - 1 Medium: `w = 0.5`, `f23 = 0.6875·f12 + 0.3125·f13`, `f24 = 0.25·f12
    + 0.75·f13` — the output is **0.46875·f12 + 0.53125·f13**;
  - 2 Hard: `w = 0.5`, `f23 = 0.375·f12 + 0.625·f13`, `f24 = f13` (the
    branch's delay slot) — the output is **0.1875·f12 + 0.8125·f13**.
  The distilled pairs in the earlier note (0.6875/0.3125,
  0.375/0.625) are the *inner* mix `f23` — the final weight-0.5 mix with
  `f24` was missed. The story modes (the rec's FE-wide mode +0x78 = 4
  or 5) recompute `w` instead: the League Challenge from the odds table
  `0x43df08` = {0, 0.25, 0.5, 0.6, 0.65, 0.7}, indexed by beaten
  opponents plus the current opponent. A completed region uses 1, or
  0.71 on Medium. Be A Legend uses .25*(created star field15/11/6)
  + .5*(event level/6) + .25*(current round/round count)
  + .25*(opponent team >39), capped at 1.
  **What the two values are:** the caller `FUN_00143248` (called for
  each team from the AIGame cluster at `0x13ca18`/`0x13ca44`/`0x13d638`
  as `FUN_00143248(team100, rec, side)`) feeds `f12/f13` from `.rodata`
  rows at `0x43e298` — team+0x40's pair is (0.9, 0.7) or (0.7, 0.95) and
  team+0x48's is (0.8, 0.7) or (0.7, 0.8), the row picked by a
  crew-existence byte (`0x18ab00` follows the rec's +0x184 side pointer
  to a byte at +0x9a); with no crew on either side the caller averages
  the pair's two rows instead. The results land at team+0x40 and
  team+0x48 (the meter-side tuning: the GB fill multiplies
  team+0x44 × team+0x48), and team+0x40 is then scaled by the
  `.rodata` row `0x43e2b8` = {1.0, 0.5, 2.0} indexed by the field +0x60
  of the object `0x131208` registers at `0x4b6fb0` (a three-way state —
  its consumer is still to read). So this function tunes the **meter**,
  not the crews' ratings. The sim's setup-time crew-rating rubber-band
  (`Ratings::blended`) is removed — it was modelled on this function's
  earlier misreading — and the traced meter blend is adopted in its
  place (`street_sim::meter`: the difficulty's fill scalars, the
  fractional meter points, and the story modes' scenario weights).
- **The trick-move values themselves** are resolved (above): the five-entry
  ladder `0x43df20` by the move's value class, with the same-repeat
  halving; no per-move constants exist beyond that class.
- **Ball flight (verified raw + Ghidra MCP 2026-10-13):** gravity
  `40.5/fps²` — init `00153ddc..00153e24` computes
  `32.1522331/fps² × 1.2596326` (`0x42009be3`, `-0xbfa13ba4` with a
  `neg.s`), used at `0014a3a4..0014a3cc` into ball+`0x118`
  (`0014a3d4`). Integration in `FUN_0014d1f8`
  (`0014d288`, `0014d3ec..0014d414`): position += velocity (VU0 op),
  then `pos.y −= half-g`, `vel.y −= full-g` — `launch`'s parabola lands
  exact at integer frames. Gameplay launcher `FUN_001505a0` (sole caller
  `0014aaf8`): angle time `√(2(R·tanθ−dy)/g)`, floored by 0.5 s
  (`DAT_004b849c` = fps×0.5) and, for releases below 14, the
  rise-to-14-and-fall time (`00150750..00150794`); wired into
  `release_shot` via `ball::shot_flight_time`. (Corrected 2026-10-24,
  "Level 2 shots: the shooter's own launch" at the end: staged Level 2
  shots go through this launcher too, from `FUN_00333ef8`, which is no
  stub; `FUN_00334330` → `FUN_0014C408` with `t = max(cue frames, 15)` is
  a toss to a floor point.) Aim (`0x1729F8..0x17356C`): root-based range,
  45°..67.5° by `(20−range)/20`; forced 67.5° when root y < 12 or
  |dx| > 38.

### The meter's hold and decay; the match's statistics (traced raw 2026-10-15)

- **The meter object** (constructor `FUN_00143248`, vtable `0x43df50`,
  one per side at game+0xc8/+0xcc): +4 the side, +0xc the meter (float),
  +0x10 a meter size (0 in play), +0x18 the full meter's **hold**, +0x1c
  this side's gamebreaker is under way, +0x20 a **pocketed** gamebreaker
  (`FUN_00141080`; the match start's fill `FUN_001457f8` pockets one for a
  value of 100 and over), +0x24 the last trick move's value, +0x2c..+0x38
  the repeat ring, +0x3c the game, +0x40 the difficulty blend, +0x44 the
  fill scale (1.0), +0x48 the sequence factor, +0x54/+0x58/+0x5c/+0x60
  the dunk, gamebreaker, passout and alley-oop values.
- **Fill** (`FUN_00143d38`): nothing while the hold is set, under the AI
  script handler (`FUN_0013e358`) or in game types 3 and 7; the amount ×
  +0x40 × +0x44 (or × 50000 / +0x10); the Easy cap skipped in the Trick
  Point and Gamebreaker Wins games (game+0x49c). At 50,000 the meter is
  set to exactly that, the hold to `0x4b7b14` and "meter full" (slot 66)
  posts; when the singleton's +0x64 is 1 (never written) or in a lesson
  game (game+0x9c 4) other than the gamebreaker lessons it stops at
  49,999 instead. Drains are negative fills through the same function.
- **The hold and decay** (`FUN_0013eff8`, the game's vt+0x16c after its
  phase update): in live play (`FUN_00140558`: phases 2..5) with
  game+0x4a4 set and the script manager outside states 3..7, both meters
  run `FUN_00143c20(0x4b7990)`: a side whose gamebreaker is under way is
  skipped; a hold of 1 or more counts down and at 0 empties the meter,
  posts "meter drops" (slot 67) and loses the pocket (+0x20); a meter
  without a hold loses `0x4b7990` × +0x40 × +0x44. The boot reset writes
  `0x4b7b14` = fps × 30 (`0x144c30..0x144c40`, 1,800 updates) and
  `0x4b7990` = 50000.0 / (fps × 600.0) (`0x142e04..0x142f04`, fps the
  timing object's float 60.0: 1.3888888 a frame, a whole meter in ten
  minutes).
- **Game+0x4a4** is cleared by the restart (`FUN_001406b8`) and set in
  phase 4 (the ball in the team's back court, `FUN_0013e4c8` case 4) once
  someone holds the ball or it crosses into the front court. Phase 4
  follows a restart, and a held ball after a loose ball, shot or steal in
  the taker's back court (`FUN_0013ee10`; the front court is phase 5).
- **The meter's listeners** (slot = post offset game+0x3b + n): 13
  `FUN_001441d8` a trick move (+0x24, the victim's drain table entry),
  15 `FUN_00144490` a steal (1,000) or trick counter (3,000; the victim
  loses the same, `0x43e248`), 16 `FUN_001445d8` a block (±2,500), 17
  `FUN_00144558` a rebound (500), 19 `FUN_00144658` a passout special
  (3,000), 24 `FUN_001447d0` a dive to the ball (500, family 0x47/0x48),
  26 `FUN_001442b8` a basket (its class value; a gamebreaker's basket
  fills nothing and multiplies the scorer's +0x44 by +0x48), 52
  `FUN_00144260` a shot clock violation (−12,500 to a meter without a
  hold), 64 `0x144708` a gamebreaker goes up (+0x1c on the shooter's
  side; its pocket goes, its hold is 1, a Level 2 empties the meter at
  once), 65 `0x144760` the gamebreaker task's end (`FUN_00144ef8`: the
  meter emptied, hold and +0x1c cleared), 70 `0x144780` a pocket, 71
  `0x1447a8` a cancel (both pockets go; the canceller's meter and hold),
  73/74 `FUN_001440d0`/`FUN_00144158` a pass special and a pick (500).
  Each fill also adds the combo matcher's last total (game+0xd0 → +0x150).
- **The stats object** (listener vtable `0x440578`, the object at its +8;
  `FUN_00186ef8` adds, `FUN_00186e50` sets, `FUN_00186df8` reads): stats
  0..14 kept by actor (scope 0), side (1) and controller (2): 0 points,
  1 dunks (basket kind 4), 2 alley-oops (kind 3), 3 assists, 4 rebounds,
  5 steals, 6 blocks, 7 baskets, 8 threes made, 9 threes tried, 10 shots
  tried (slot 11's base 2 or 3), 11 trick points, 12/13 Level 1 and 2
  gamebreaker baskets, 14 slams; 0x10..0x27 by side only (0x11/0x13 the
  meter filled without/with a pocket, 0x12/0x14 dropped, 0x15 the biggest
  lead, 0x16.. the trick points by source, 0x10 the other side's).
  `FUN_00186fa8` blocks phases 0 and 9 and, in script states 3..7, all
  but a list. The listeners: 11 `FUN_001877b0`, 13 `FUN_001884e0`, 15
  `FUN_00188640`, 16 `FUN_00188868`, 17 `FUN_00187a08`, 19
  `FUN_00187480`, 24 `FUN_00188b20`, 26 `FUN_00187c10`, 39
  `FUN_00187930`, 66/67 `FUN_001883f0`/`FUN_00188368`, 71
  `FUN_00187ba0`, 73/74 `FUN_00187338`/`FUN_001875e0`. Actor+0x2a4 counts
  each actor's updates of live play (`FUN_00156f48`), cleared per match
  (`FUN_00156870`); `FUN_00181ba8` gives it in whole seconds.
- **The star** (`FUN_0012a7d0`): each of the three's share of the side's
  rating 9, and once the side and he have played a second, the share ×
  the side's (stat 10 + 2 × stat 0) per second over his own; the best
  (the first of equals) is the star, the shares' percentages go to the
  playbook's +0xcc.
- **Readers:** the post-game handler `FUN_001ded40` hands Be A Legend the
  side's stats 0xb, 0xc and 0xd (0x1dee6c, 0x1dee90, 0x1deea8) and the
  user records stats 0, 0xb, 1, 5, 6, 0xc + 0xd; its milestones read 0, 2,
  5, 6 and 0xb; the end boards read 0, 1, 4, 5, 6, 0xb, 0xc, 0xd.
- **Ported** (`street_sim::street`, `street_sim::stats`): the hold
  (`METER_HOLD`), the decay (`METER_DECAY`) × fill × scale, the
  hold's gate on every fill and drain, the expiry with the pocket,
  game+0x4a4 (`MeterClock`), slots 64/65/70/71, the gamebreaker
  basket's scale, the counter's drain, the rebound's and dive's 500, the
  shot clock's −12,500, the Easy cap's exemption; stats 0, 1, 2, 4..14
  and 0x11..0x15 for actors and sides, actor+0x2a4, the star's weights;
  the AI's meter shot reads the hold; the box score and Be A Legend's
  charts read the stats. **Not ported:** the fill's script-handler and
  game-type gates and the 49,999 stop, the matcher total added to every
  fill (the rewrite pays it on baskets), the pick's 500 (contact kind 3
  needs the handler's matched defender), the passout specials, assists,
  the side-only trick-point breakdown and `FUN_00187200`, controller
  scope, and the stats' script-state list. The gamebreaker task ends at
  the next catch or restart in the sim, where the original waits for its
  replays. **Update 2026-10-17:** all of these but controller scope and
  the gamebreaker task's end are ported; see "The combo ring, the meter's listeners, the statistics and the shot clock" (the
  "script handler" gate is the court intro's).

### Shot release and launcher (traced raw 2026-10-14)

The make/miss decision is not `FUN_001805e8` (the AI's shot tier). It is
the release callback at `0x1729f8..0x17356c`, which Ghidra folds into the
stub `FUN_00333ef8`. That callback calls the launcher
`FUN_0014aaf8(angle f12, ball a0, made a1, kind a2, base a3)` at
`0x1734f4`. Port: `street_sim::shooting` (`shot_percent`,
`World::shot_call`, `World::launch_shot`) and `ball.rs` (`forecast`,
`steepen_entry`, `bank_mirror`, `bank_possible`).

- **Base percentage, `FUN_00171690`.** The distance is `FUN_0016e930`,
  the per-player table at game+0x374 that `FUN_0013f748` fills: the
  distance across the floor to (side × 36.75, 0). Long range is
  `FUN_0016c908` (|z| ≥ 22.5 `DAT_003d9050`, or distance ≥ 23.75
  `DAT_003d904c`). For a dunk, oop, lay-up or state bit 3, long range is
  instead the stored base `0x3d658c == 3`. Percentages:
  - Inside the arc, at most 5 out: `Shots/100·15 + 80`.
  - Inside the arc, beyond 5: `Shots/100·40 + 50 − min(d,22)/22·15`, at
    least 45. The distance carried on is clamped to 22.
  - Long range: `Threes/99·25 + 10`, from 24.5 out less
    `min(d−22,12)/12·10`.
  - Every step is truncated (`cvt.w.s`). From the wrong half (the sign of
    x against `FUN_00156db0`) the result is 3.
- **Callback adjustments (`0x172a44..0x1731a4`).**
  - The game's virtual +0x44 returns a catch-up bonus. The base class
    (`0x143040`) returns 0. The points game (`FUN_0012eac0`), the
    Trick Point game (`FUN_0018c788`, rec+0xe8 = 1) and Gamebreaker Wins
    (`FUN_00145b48`, rec+0xe8 = 6) each read table 2 of `FUN_00186638`'s
    groups (`0x3dc19a` and `0x3dc3ca`, identical). Each table is 31×3
    signed bytes: rows are the lead clamped to ±15 (0 when level or
    ahead, up to +20 when far behind late), columns are the game's stage.
  - The points game scales scores to 21. The stage is the higher scaled
    score against bounds drawn once a game (`FUN_0012e798`: `rand()%6`
    and `15+rand()%3`). The sim keeps them as the match's state
    (`World::stage_bounds`) and starts at 2 and 16, its own pair (see "The
    catch-up stage and the side fades' landing").
  - When a team is human (game+0x9a/0x9b): on Easy (story record +0x12c
    & 3 == 0) the human side gets the bonus doubled and +40 within 33.75,
    the CPU side gets no bonus and −10. On Hard the human side gets no
    bonus.
  - Then, with nobody within 7 (`FUN_0018d0c0`), the shot gets
    `rating/99·20` (31 at long range) inside 33.75. Otherwise 5 comes off
    per opponent within 7 with state bit 12 (`FUN_0016e9c0`), skipping
    opponents with animation flag 0x40 or state bit 16.
  - A gamebreaker (`ball+0x178`, set by `FUN_001422f8`) replaces all of
    this with `rating·3/2 + 55`, or `dunks·3/2 + 60` for an oop; Level 2
    gives 100. The wrong half gives 3, the floor is 3, and the cap is 98
    except in a gamebreaker. Script Manager states 3..7 give 0 or 100.
  - The roll is `rand() % 100 < percent`, stored at `0x3d6580`.
- **Dunk branch (wired as `shooting::dunk_percent`).** The move level
  `player+0x24c` comes from the dunk table at `*(0x3d17a8)` (`0x43d3b0`,
  and per controller layout `0x43d408`/`0x43d460`): eleven
  `{turbo mask, level}` pairs, the level being the turbos in the combo
  (4, then four 3s, then six 2s); `FUN_00169260` picks a dunk of a level
  for the AI and `FUN_001773b0` copies entry 0's level. The sim reads it
  from the latched turbo buttons (`shooting::dunk_move_level`). Dunks rating 0..99 maps to a tier: 1..4 by 20s, 5 for 81..90,
  6 for 91..99, with the fraction inside the tier. The tier is lowered by
  2 or by the move's `player+0x24c` (`FUN_00171de8 & 0x800`). Then
  −3 → `10f`, −2 → `10(1−f)+40f`, −1 → `40(1−f)+70f`,
  0 → `70(1−f)+98f`, above 0 → 98, below −3 → 0. Easy adds +20 (human)
  or −10 (CPU). The kind is 0 on a make and 3 on a miss.
- **Aim kind (`0x3d6588`).**
  - Lay-up (bit 13): if a bank can reach (`FUN_0014faf0`), kind 1 half
    the time. Otherwise a make is 0 half the time; the rest are
    `2 + rand&3`.
  - Jump shot in a gamebreaker: `4 + rand&1`.
  - Jump shot, make: 0 half the time, else the rim kind.
  - Jump shot, miss beyond 25: 0 (an airball) when
    `rand%100 ≤ max(10 − (rating+1)/10, 0)`, else the rim kind.
  - The rim kind is `1 + rand%5` where a bank can reach, else
    `2 + rand&3`.
  - When either team's meter is full (`team+0xc == 50000`), a missed
    kind 0 is re-drawn as a rim kind.
- **Angle (`0x3d6584`).** `π/4 + max((20−d)/20, 0)·π/8`. A lay-up gets
  3π/8 when the ball is below 12 or behind the board's plane.
- **Launcher, jump shot and lay-up path.**
  - The target is (side × 36.75, 11.9567747 `0x413f4ef3`, 0). Kind 1
    mirrors x behind the glass: `x + s((38−r) − s·x)(1/e + 1)`
    (`FUN_0014fa60`, e = ball+0x120 = 0.75).
  - Straight paths (`0x14ba60`) are kind 0, and kind 1 when made. A make
    is steepened by `FUN_0014fc00`: entry angle
    `asin((r + 0.09294706)·1.2606764)`. For a bank the entry is spread
    over the kept speed, `atan(tanθ·|(e·x, z)|/|(x, z)|)`. The time is
    `√(2(d·tanθ + dy)/g)` with `FUN_001505a0`'s minimums; if the requested
    angle arrives sooner, the launch angle becomes `atan(g·T²/d − tanθ)`.
    A straight miss adds `(0, 0, r + 1.83645)` turned by
    `rand·π − π` about y (negated at the −x basket), which lands on the
    court's side.
  - Other kinds (`0x14ae8c..0x14ba5c`): half the time, if not a dunk,
    take the first miss that works. Then 50 tries:
    - The turn is `rand·π/2 − π/4`, plus π/2 for kind 4, −π/2 for kind 5,
      π for kind 2. It turns the unit direction to the target (with
      `q⊗v⊗q̄`, `FUN_002f64b0`).
    - The reach is `0.83645 − (r+0.08645)·rand`. For kind 2 it is
      `(r+0.83645) − (2r+0.08645)·rand`.
    - The aim is lifted by `√((r+0.043225)² − (reach−0.793225)²)`. For
      kind 1, x is stretched by 1/e and the angle is re-steepened from 0.
    - Each flight is watched with the full ball update `FUN_0014d1f8` on a
      copy (flags +0x190/+0x194). First it fast-forwards to just short of
      the circle `√9.0625 + r` around the board's middle (the earlier root
      of its quadratic, `ceil(t−1)` ticks). Then it counts ticks: +1, or
      +0.25 while rolling with a zero vx or vy.
    - Watching stops when the ball is below 11.91355 and not rising, when
      |x| < `34.98961 − r`, or when |z| > `3.0103986 + r`. The flight is
      rejected past 600 ticks (`DAT_004b84a8` = fps·10) or when it passes
      behind `r + 38.25`.
    - It keeps the flight whose scored flag (+0x18c) equals the roll and
      that watched the most ticks. The timer +0x158 is the ticks to the
      last contact + 1.
    - The 50th try, when nothing has worked, aims `0.75 − r` (make) or
      `r + 0.83645` (miss) beyond the middle with no turn. With still no
      flight it calls itself with kind 0.
- **Constants (`FUN_00153b68`).** `DAT_004b8380` = 0, `0x4b83b0` =
  (0,1,0), `0x4b83c0` = (0,0,1), `0x4b849c` = fps·0.5,
  `0x4b84a0/a4` = 20/10 mph per frame (the dunk path),
  `0x4b8550` = √9.0625, `0x4b8554` = 38 − √9.0625.
- **Ball tuning (`FUN_0014a300`, default entries 0.5).**
  - +0x120 board restitution 0.75, +0x124 board friction 0.8.
  - +0x128/+0x130 rim restitution 0.7/0.65. In `FUN_001526d0` they blend
    by the contact normal's y and by the front–back position
    `(front + 0.793225)·0.6303382`.
  - +0x12c collar friction 0.6 (the collar's restitution is +0x128).
  - +0x134 rim friction 0.6. `FUN_00150d88` caps friction at 0.4·|vt|
    and skips it for a bank's first touch (+0x198).
  - +0x138 floor restitution 0.8, +0x13c floor friction 0.9.
  - +0x140 ceiling height from `0x43edf8` by the court
    (`FUN_001e0bc0()+0xcc`): 25.58 for court 6, 33.77 for court 8, none
    (-1) elsewhere; +0x144/+0x148 its restitution and friction 0.5.
  - The table rows are `0x43edc8` (radius 1.25, 1.2, 1.5),
    `0x43edd8` (0.5, 0.25, 1.0) and `0x43ede8` (0.5, 0.1, 1.0).
- **Hoop centre: the original's is ±36.75**, the rewrite's scene setup
  uses ±36.95. Evidence: `FUN_001404e0` (side × 36.75), the distance
  table at game+0x374, the rim restitution's hoop x (`ball+0x150 =
  ±36.75` in `FUN_0014d1f8`), and 38 − 36.75 = 1.25 (regulation 15 in
  from the board). The sim aims at its own hoops and did not move them.
- **In the sim:**
  - Each flight is played through the same free-ball update the shot then
    flies with (`Ball::step`, below), spin and the bank's first touch
    included, so the choice always agrees with what the ball then does.
    The original's look-ahead clears +0x198 for its watch, so it watches
    a kind-1 miss with the glass's friction its real flight skips; the
    sim watches the flight as it will be flown.
  - A straight make that would not drop is re-aimed through the middle; a
    straight miss that would drop is re-drawn up to seven times. These
    are the sim's guards.
  - The flight lasts the original's timer (+0x158: one more than the
    watched flight's last contact, or the straight flight's formula),
    then the ball goes on as a rebound or loose, meeting the same rim,
    collar, glass and floor ("Dunks and rebounds on the ball's
    states"). Until 2026-10-16 it lasted the watched ticks.
  - The sim does not know which teams are human, so Easy and Hard never
    apply. The points game's catch-up stage bounds start at the sim's own
    2 and 16.
  - The lay-up's release point is the sim's own: above the player, 1.8
    from the rim's middle.

### Free-ball contacts (traced raw 2026-10-14)

`FUN_0014a968` updates the ball every frame: with a holder (+0xf4) it
follows the hand (`FUN_0014e060`), without one it runs `FUN_0014d1f8`,
whatever its state (+0x180: 4 a pass or self-toss, 5 a shot, 6 a shot
that has touched the hoop, 0/1 loose). So a shot, a pass and a loose ball
meet the same contacts; the state machine at the end of `FUN_0014d1f8`
only decides what a contact means. `FUN_001538f8` fills the ball's
prediction ring (+0xf0, +0x164/+0x168) by running the same update with
+0x190 set on saved state. Port:
`Ball::step` in `street_sim::ball`.

- **Ball fields.** +0x30 position, +0x40 velocity (per update), +0x50
  orientation quaternion (spun by the spin each update, `0x0014d830..
  0x0014d89c`; no contact reads it), +0x70 spin axis, +0x14c spin rate (radians
  per update), +0x80 last position, +0x118 gravity per update², +0x11c
  radius (also the floor), +0x150 the hoop x on the ball's side (±36.75 by
  the sign of x, set after the move), +0x184 the last release (1 a pass,
  `FUN_0014c0c0`; from `FUN_0014aaf8` 2 a jump shot, 5 a lay-up, 3 or 4
  on its dunk path, 4 with `FUN_00171de8 & 0x800`), +0x188 airborne, +0x18c through a ring,
  +0x190 look-ahead, +0x194 no net, +0x198 a bank's first touch, +0x19c
  second pass, +0x1a8 armed to score (set by the catch `FUN_0014cb28`).
  Vectors are (0,0,0) `0x4b8380`, (1,0,0) `0x4b83a0`, up `0x4b83b0`,
  (0,0,1) `0x4b83c0`, down `0x4b83e0` (`FUN_00153b68`).
- **Order (`FUN_0014d1f8`).** Move by +0x40; if airborne, y −= g/2 and
  vy −= g. Set +0x150. Floor (`FUN_00153278`) and ceiling (`FUN_00153428`);
  if neither, and `FUN_00150cc8` (11.5 − r ≤ y ≤ r + 15, |z| ≤ r + 3,
  35.91355 − r ≤ |x| ≤ r + 38.25): backboard `FUN_00151928`, then the
  collar `FUN_00152418` or else the rim `FUN_001526d0`, then, unless
  already through, the basket `FUN_001515a8`. A ball already through,
  without +0x194 and touching none of board, collar or rim, meets the net
  `FUN_00153550` (skipped in the update the basket returned 1), which
  makes it airborne. A ball still not airborne that touched nothing falls
  after all: y −= g/2, vy −= g, +0x19c = 1, all of the above again
  (`0x0014d3ec..0x0014d508`, the net not skipped this time), and if still
  nothing it is airborne. Then the state machine, the spin of the
  orientation, the ±42 by ±26.5 containment (skipped while
  `FUN_00148888` answers for `game+0x134`), and +0xa0 = move, +0x80 = +0x30.
  It returns board, collar or rim, or floor or ceiling.
- **State machine (`0x0014d630..0x0014d820`).** A shot (5): the timer
  +0x158 runs down; at 0, or on the floor or ceiling, it is loose (1) with
  event 0x56; on board, collar or rim it becomes 6 (+0x64 = timer). State
  6 at 0 is 0 with event 0x56. A pass (4): collar or rim (not the board)
  knocks it loose (1); the floor and the board leave it a pass; at timer
  0 it is 1 (+0x1a4, event 0x4f); at timer 5 (`DAT_004b84b0`) with the
  target's state bit set (`FUN_0016e9e0`) its speed halves. Any other state
  on a contact goes to 1, except that 0 stays 0 on the hoop; the floor or
  ceiling also clears the gamebreaker flag +0x178.
- **Surface speed (`FUN_001507c8`).** v + r·rate·(n × axis)/|n × axis|·
  sin(acos(min(|axis·n|, 0.9999))), i.e. v + r(n × ω); nothing when the
  rate is 0 or |n × axis| ≤ 0.0001.
- **Bounce (`FUN_00150d88`, e, μ, n).** On the surface speed: vn = (n·vc)n,
  vt = vc − vn, J = −(1+e)vn; friction F = −vt·min(μ|J|, 0.4|vt|)/|vt|
  when |vt| > 0.0001, none for a bank's first touch (+0x198, cleared).
  v += J + F. If μ|J| < 0.4|vt| (still sliding) the spin takes
  1.5/r·(−n × F) (hollow ball) added to rate·axis; otherwise the ball
  leaves rolling: axis n × vt′, rate |vt′|/r for the new tangential v.
- **Resting (`FUN_001511f8`, μ, push, n).** v += support n·g·n_y (when
  n_y > 0) − g·up, plus friction −vc·min(μ|support + push·n|, 0.4|vc|)
  /|vc|, and the same spin as a sliding bounce.
- **Settle (`FUN_00151550`).** +0x188 = 0, v −= (v·n)n.
- **Floor (`FUN_00153278`).** y ≤ r: y = r. Rolling: rest (μ 0.9), vy = 0.
  Airborne and falling: bounce (0.8, 0.9); rolling once vy ≤ 5g.
  Ceiling (`FUN_00153428`): y ≥ +0x140 − r with +0x140 ≥ 0, bounce off
  down (0.5, 0.5) when rising.
- **Backboard (`FUN_00151928`).** Only with |x| ≥ 38 − r. A bank's first
  touch within 26.5 of the basket (`FUN_00153790`) is taken back along
  its move to the plane ±(38 − r) and meets it head on (n = (∓1,0,0)); with
  |vx| ≤ 0.0001 it is put at 11.5 − r under the board instead. Otherwise
  the box (`0x43ed68`: 38..38.25, mirrored for −x, 11.5..15, −3..3): d is
  the offset from its nearest point; none if |d| > r; at |d| ≤ 0.0001
  with no speed the ball is put under it; if |d| < r a falling, moving
  ball (or one at |d| ≤ 0.0001) is first taken back along its move to the
  first face of the box grown by r it crossed (smallest t ≥ 0), then
  pushed out along d to touch; n = d/|d|. Airborne: bounce (0.75, 0.8)
  if moving in, settle on an upward face if it comes off slower than
  n_y·5g. Rolling: on an upward face turn v along it keeping its speed
  and rest (0.8); rolling along the collar height (y = r + 12, vy = 0,
  |vx| > 0.0001) into the face, bounce; otherwise turn v along it.
  Skipped while `FUN_00148760` finds a `game+0x134` record of this side
  at least 13.25 high.
- **Collar (`FUN_00152418`).** Not for a rising airborne ball;
  12 − r ≤ y ≤ r + 12, |z| ≤ 0.25, 36.75 ≤ |x| ≤ 38 and
  √((|x| − 36.75)² + z²) > 0.793225. y = r + 12. Rolling: vy = 0, rest
  (+0x12c 0.6), vy = 0. Airborne: bounce (+0x128 0.7, +0x12c 0.6), rolling
  once vy ≤ 5g. A collar touch skips the rim.
- **Rim (`FUN_001526d0`).** 11.91355 − r ≤ y ≤ r + 12 and
  0.75 − r ≤ |(x − hx, z)| ≤ r + 0.83645. The ring (`FUN_00150b90`) is
  radius 0.793225 about (hx, 0), its tube's centre 11.9567747 up (the
  launcher's aim height; +z over the middle). Airborne: the first of
  P·0.25 + prev·0.75, (P + prev)·0.5, P·0.75 + prev·0.25, P within
  r + 0.043225 of the ring; rolling: P. The ball goes there, to the ring
  point plus n·(r + 0.043225) (n up at ≤ 0.0001). Rolling: turn v along
  n keeping its speed, rest (0.6) pressed by the correction's length on
  the first pass inside the ring. Airborne: moving in, bounce (e blended
  `(1−n_y)·0.7 + n_y((1−f)·0.7 + f·0.65)` with
  f = clamp((front + 0.793225)·0.6303382) when n_y > 0, else 0.7;
  friction 0.6). Settle if n_y ≥ 0 and the speed off is ≤ n_y·5g; or,
  with |n_y| < 0.9999 and the ball inside the ring, settle if it is
  nearly still or π/2 − acos(v̂·t̂) ≤ 0.86393803·√(1 − n_y²), t̂ the
  ring's tangent (up × n) turned along v.
- **Basket (`FUN_001515a8`).** Armed (+0x1a8) and coming up through the
  rim plane within 0.793225 (interpolated as below): vy = 0,
  y = 11.870325. Down through it (y < 11.91355 ≤ prev.y) at the point
  prev·f + P·(1 − f), f = (y − 11.91355)/(y − prev.y) (0 when they are
  within 0.0001), within 0.793225 of (hx, 0): +0x18c = 1; outside the
  look-ahead, if armed, event 0x55 (class `0x3c6620`), state 1,
  +0x1a8 = 0, +0x178 = 0.
- **Net (`FUN_00153550`).** cone = (y − 10.5)(0.75 − r)/1.4567747; if
  |(x − hx, z)| ≥ cone: (when > 0.0001) pulled back to the cone (to y 10.5
  and the middle if the cone is negative), speed × 0.5 when +0x184 is
  3 or 4 (the dunk path) else × 0.1, +0x18c = 0, spin × 0.25.
- **Spin given.** A jump shot or lay-up: axis (v × up)/|v × up|, rate
  `DAT_004b8548` = 3·2π/60 (backspin), reversed for a lay-up with
  `player+0x1e4 == 2` and `FUN_00171e08 & 1`; none when |v × up| ≤ 0.0001
  (`0x0014bdac..0x0014be78`, also before each look-ahead flight). A pass
  (`FUN_0014c0c0`): rate `DAT_004b8544` = 2·2π/60 about v × up, or v × down
  for a bounce pass, which also sets +0x198 so the floor's first touch has
  no friction. The catch zeroes the rate.
- **The look-ahead's ranking** adds 1 per update, or 0.25 when the ball
  is rolling with vy or vx exactly 0 (`0x0014b760..0x0014b7e0`,
  `0x0014b858..0x0014b8c4`).
- **In the sim** (`Ball::step`, used by `fly`, `forecast`, `fly_pass`
  and the loose ball in `World::step_ball`):
  - Velocity is per second; the contacts convert it to per update so the
    thresholds are the game's, and speeds no contact changed keep their
    per-second value.
  - The ring and the basket are about the World's hoop on the ball's
    side (the native hoops are ±36.75); the board, collar and box are the
    game's absolute numbers. A pass has no court to ask and uses the
    native hoops.
  - Not modelled: the ceiling (the sim does not know the court), the
    board-disable record (`FUN_00148760`), the containment skip
    (`FUN_00148888`), the reversed lay-up spin, the orientation
    quaternion, the pass's speed halving at timer 5, and points for a
    ball that goes through outside a shot. (The board-disable record, the
    reversed spin and the basket outside a shot are ported since: "The
    ball on the bones".)
  - The sim's own: a staged Level 2 finish passes the rim and the collar
    untouched; a Back 2 Papa leg ends when it touches the glass; a guard
    against a zero distance after the board's sweep.
  - The game's polynomial `acos`/`sin` are `libm`'s.

## 7b. Computer players (traced 2026-10-15)

Port: `crates/street-sim/src/ai/`. A computer player fills the same pad
word a controller does; the rewrite's client asks for one with
`ai::input` (a marker, `Input::computer`) and `World::step` builds the
pad from the AI's state, which the world (and its snapshot, version 25)
carries. Everything below was read from the decomp and checked against
the raw instructions where Ghidra loses the code after `FUN_0030fa50`
(it returns the timing object; `+0x24` is the frame rate, 60, so every
`fps × k` below is a frame count).

- **Objects.**
  - AIPlayer (`FUN_0013a760`, vtable `0x43d748`): `+0` game, `+4` the
    pad (`FUN_00146138`), `+8` the actor, `+0xc` a sequence node
    (`FUN_00132e08`, vtable `0x43cf78`) whose children are the behaviour
    queue (`+0x18..+0x20`, front first), `+0x30` "inside the update"
    (clearing is put off), `+0x3c..+0x44` the 12-angle steering history
    (`0x4b760c` = fps × 0.2), `+0x48/+0x4c` the speed level and the frame
    it changed.
  - AIPhysPlayer (`FUN_0017d930`, vtable `0x43faf0`) adds `+0x58` the role
    (1 handler, 2 post, 3 perimeter), `+0x64` the actor, `+0x68` the
    AITeam, `+0x6c` the slot, `+0x70` the player index, `+0x74` the side,
    `+0xa0` the rim it attacks (`FUN_00140518`). Its vtable: `+0x2c`
    decide (`FUN_0017e5c8`), `+0x34` bend round players (`FUN_0017e858`),
    `+0x3c` smoothing (`FUN_0017ec38` = `FUN_0017ec78` court edges then
    `FUN_0013b598`), `+0x44` the avoid mode (`FUN_0017e7d8`).
  - Behaviour nodes (`FUN_0013be20(node, type)`; `+0x14` vtable): `+0x14`
    done, `+0x44` start, `+0x4c` update, `+0x34` type; `+4` age, `+8`
    started, `+0x10` the ball.
  - AITeam (`FUN_00189280`, vtable `0x440948`, update `FUN_0018a708`):
    two lists of strategies indexed by the game phase, `+0x70` while the
    side has the ball and `+0x7c` while it defends: offence {—, tip
    `0x12e3b8`, scramble `0x43b1c8`, rebound `0x12db58`, fast break
    `0x43abc8`, playbook `FUN_00126f60` (`FUN_00124028` in modes 1, 2, 6),
    inbound `0x12bc08`, `0x126e80`, `0x43aad8`, `0x43aa58`}; defence {—,
    tip, scramble, rebound `0x12de28`, defense `FUN_00122e98` (phases 4
    and 5 share one object), inbound `0x12c4e8`, `0x126ef0`, …}. A
    strategy's vtable: `+0x14` start (on change), `+0x1c` update, `+0x24`
    the matchup accessor (`FUN_0017f498`; the defense's `FUN_00123d40`:
    k 0 man `[3+slot]`, 1 help `[6+slot]`, 2 second `[9+slot]`).
- **The frame** (`FUN_0013a970`, called from the pad update
  `FUN_001587d8` for actors the computer drives): clear the buttons (the
  stick angle `+0xc` and strength `+0x10` stay), decide when the queue is
  empty, run the front node (`FUN_00133030`: a started node that is done
  is popped and the next started and run in the same frame; a node's
  first frame runs start, the done test and the update), then
  `pad+4 = pressed`, `pad+8 = held`. Pushes go to the back
  (`FUN_0013aa88`); `FUN_0013aaa8` clears and pushes.
- **The pad word** (`FUN_0012f2c8`, the `AIACTION_*` ids of
  `USRGAME*.CFG`): d-pad 1/2/4/8, juke 0x10 (+0x800000 while held),
  shoot 0x80, pass 0x100 (+0x24 the receiver, `FUN_00146228`), steal
  0x200, L3 kick 0x800, block 0x1000, any turbo 0x2000 (`FUN_001308a0`),
  L1 0x4000, L2 0x8000, R1 0x10000, R2 0x20000 (`0x3d01d0`), two or more
  turbos 0x40000, rebound 0x100000, the screener's stance 0x400000 and
  step into the defender 0x4000000 (read by the pick animations, families
  `0x28a..0x28c`, which the sim has none of: the pad keeps the bits and
  the sim reads none), strategy 0x2000000, pocket 0x8000000, cancel
  0x10000000. The rewrite keeps the combo masks' slot meaning (the
  sim's `NativeTricks::slot` reads R1 as 0x8000 and L2 as 0x10000, so the
  pad maps 0x8000 to R1 and 0x10000 to L2).
- **Phases** (game+0xf4, `FUN_0013e4c8`/`FUN_0013ee10`): 2 a loose ball
  (ball state 1, or 0 below 6), 3 a shot or rebound, 4 the ball held in
  the back court, 5 in the front court (x ≥ 0 counts as +x), 6 dead, 8/9
  over. Ball states (ball+0x180): 0 a rebound off the rim, 1 loose, 3
  held (`FUN_0014cb28`), 4 passed (`FUN_0014bff8`), 5 shot (`FUN_0014aaf8`),
  6 on the rim. game+0x490 counts the frames the same player has held the
  ball; game+0x440 the frames in phases 2..3. The shot clock the AI reads
  is game vt+0x5c = `FUN_00131378(game+0x190)`, whole seconds mod 60.
- **Deciding** (`FUN_0017e5c8`): phase 3, ball state 0: "loose ball" (1);
  else "prepare rebound" at `0x3cf620 + index × 16` (dealt by
  `FUN_0012d790`, below). Phase 4: defending "defense", else "fast break". Phase 5:
  defending "defense"; role 1 "carrier pause" (0, 9, eager), 2 "post
  player", 3 "perim player". Other phases: let go of the stick.
- **Steering.** `FUN_0013af58(ai, angle, level, avoid, target)`: a higher
  level waits 9 frames after the last change (`0x4b7614` = fps × 0.15), a
  lower one applies at once; level 0 lets go; running (level 2+) with an
  avoid mode bends round players (`FUN_0017e858`: the two nearest ahead
  within 16 or 18, the grid at `0x4b9da0` filled at `0x1841a0`, a second
  hit a quarter as much in the first's direction, teammates half, mode 2
  scaled 150..300% by the target's distance, clamped to ±256); with a
  target, a turn under 6 keeps the old angle and a target within
  `0x3d17f0[level]` = {0.5, 1, 1.5, 2} is steered at directly; otherwise
  `FUN_0017ec78` keeps off the court's ends (|x| 40.5) and sides (|z|
  25) in live play and `FUN_0013b598` holds a zigzag (a sign change
  among the last 7, or 12 in mode 2) and takes turns of 15..198 units 14
  at a time (14.7222 and 199.611 truncated). Strengths `0x3d17d8` = {0,
  38, 58, 64}. `FUN_0017e7d8` drops mode 2 to 1 past 35 toward the rim.
  `FUN_0013b210` faces an angle (history filled), `FUN_0013ac30` lets go.
  Angles: `a` moves along `(−sin a, −cos a)`; `FUN_0018ce78(p, q)` is the
  angle from `p` toward `q`; game+0x2cc[a][b] the angle from a to b;
  game+0x35c the angle from the offence's rim toward the player
  (`FUN_0013fed0`), `FUN_0017da80` the reverse.
- **Skill** (`FUN_00140a30`, `FUN_001409c8`, float `FUN_0017dda8`): four
  values (Easy level, Easy behind, Hard level, Hard behind); Easy `a`;
  Medium `(0.6875a + 0.3125c)/2 + (0.25a + 0.75c)/2`; Hard `(0.375a +
  0.625c)/2 + c/2`; the level and behind values mixed by `k/6`, `k` the
  catch-up table 0 (`0x3dc0e0` points game, `0x3dc310` trick point and
  Gamebreaker Wins). The difficulty only applies when the other side has
  a person (`FUN_00140998` returns Medium otherwise). Integers round by
  `cvt.w.s(x + 0.5)`.
- **The pad builder** `FUN_00182280(ai, request)`: a juke takes the trick
  level the Handles earn (`FUN_0015f098`, `0x43f080`: 81/61/41 → 4/3/2,
  else 1); below 2, `skill(20,10,0,0)` to go at level 4; else 80% to go
  (`skill(60,80,80,100)` in modes 1 and 6, game+0x49c) and below 4
  another `skill(20,10,0,0)` to push it to 4; 50% for the turbo bit; then
  a combo of a level in range (`FUN_0015eff0`, `0x43d408`: `{mask,
  level}` × 11, levels {4,3,3,3,3,2,…}). A shot: a full meter adds two
  turbos (a gamebreaker); away from a dunk (`FUN_0016c500(actor,1) != 2`)
  a turbo, or in modes 2 and 6 (`FUN_00183c78`, shot clock over 4) a pass
  instead; at the rim the dunk level from Dunks (`FUN_00169208`,
  `0x43f0c0`) picks a dunk combo the same way (`FUN_00169260`, `0x43d3b0`),
  the level-4 mask on the wing spot `FUN_001826f0` (a clear lane, 12.5..14
  out, |z| < 6, nobody within 5, 182 < angle < 330 or 694 < angle < 842).
  A block request adds a turbo. The draws come in exactly this order.
- **Handler behaviours.** "carrier pause" (type 3, `FUN_00101948`, start
  `FUN_001019d8`, update `FUN_00101b50`): steps `margin` from its spot
  away from the defender ±107 units (`FUN_00102030`), walks in while the
  defender stays over 11 away (9 to stop, `FUN_00101f60`), pump-fakes 1 in
  8 when blocked with openness 3..5 every 49 frames, waits 6 frames facing
  the rim when the defender is over 7.5 away or the handler is open, and
  jukes a defender in the way at `skill(3,18,20,100)` (eager
  `skill(0,40,40,100)`, `FUN_001023c8`). "shoot" (type 9, `FUN_00100e38`,
  update `FUN_00100fe0`): patience (3 in 4 unless nearest the rim) waits
  for quality over 5 while moving; steps back to the arc from 20.75..24.25
  with nobody within 14 (`FUN_001013f8`); a 1-in-16 pump fake at quality
  under 5; passes out of a blocked lay-up (`FUN_00181c68`, to
  `FUN_00182028`'s most open teammate); else presses shoot through the
  pad builder and, within 14, faces the rim at `skill(0,50,100,100)`.
  "drive" (type 5, `FUN_001095d8`, start `FUN_001096b8`, update
  `FUN_00109898`): goes up within 5..16 (one draw), the big dunk from the
  wing at `skill(0,10,100,100)` (none under 40 Dunks, 90% none under 65),
  gives up with the lane shut and quality under 3, jukes in the way every
  6 frames at `skill(0,40,60,100)` (`FUN_00109c80`). "iso3pt" (type
  0x11, update `FUN_0010c158`): walk in to 28.35, back out beyond the arc
  by 1.4..4.9 (`FUN_0010c8c0`), go at a defender within 15 with jukes at
  `skill(50,75,100,100)`, shoot beyond the arc within 29.75 at quality 6
  (7 with a full meter), else wait 24 frames; spots from `FUN_00180f80`
  (sectors `FUN_0017e0c8`: corners `FUN_00180cd0`, wings and top
  `FUN_00180e28` at 614/512/410 ±20, the paint `FUN_001812a8`; long
  80..100% over 50 Threes, `FUN_00180c38`). "pass" (type 8, vtable
  `0x437448`: start `FUN_00100ae0`, done `FUN_00100b60`, update
  `FUN_00100bb0`): turn first for a long pass from far out, press every
  other frame once the ball has been held 6..8 frames, until the ball
  leaves his hands; it waits while he is catching (bit 9, below).
  "PocketGB"/"CancelGB" (0x15/0x16) press once. Actor+0xa4 bit 4
  (`FUN_00154680`, read by `FUN_0013b860`) is the passer's pass animation
  (`ai::Ctx::throwing`); bit 9 (`FUN_0016ea00`, the AI's `FUN_001816d0`,
  read by the pass node, the carrier's behaviours, `FUN_00181aa8` and the
  playbook's checks) is the receiver's catch (`ai::Ctx::catching`), not a
  throw: the captures show it from the release through the flight and for
  the catch animation after it.
- **Tests the behaviours share.** The lane (`FUN_0017f4d0`): a cone from
  `r` behind the player toward the rim, `lo..hi` = 4..6 with the ball,
  else 5..7 to 6..8 by distance, reach `min(0.9 d, 20)`, `r = lo · reach
  / (hi − lo)`, half-angle `atan(hi / (reach + r))` ≤ 171. Openness
  (`FUN_0017fd90`, 0..7): tables `0x3da4f8` (with the ball) and
  `0x3da5d0` (without), rows by rim distance (8, 19, 23.75, 31.75, 42),
  columns by the nearest standing defender (4, 6, … 20), less one each for
  a second defender within 8, a closer option for the handler, a defender
  in the passing lane (`FUN_00180a98`), a pick being set, and a long shot
  by a poor three-point shooter in phase 5. The juke test
  (`FUN_00182878`): shot clock 4+, no person on the side, the ball held 5
  frames, turbo over 0.2, a defender within 4.5 (4.9 after a recent event,
  `FUN_00183af8`) or within 7 and within ±142 of the line to the target.
  Quality is `FUN_001805e8` (`shooting::evaluate_shot_quality_tier`).
- **Defense** (type 10, `FUN_001062b8`, update `FUN_00106490`, done when
  the side gets the ball, `FUN_00106458`): the matchup every
  `fps × skill(0.25,0.2,0.14,0.1)` frames; a trick counter
  (`FUN_00107710`, steal + two turbos) every 7 frames from in front of a
  juking handler at `skill(−13,−17,40,50)` (`skill(8,8,−4,−4)` not
  juking); a block (`FUN_00106a88`): once per shot, early at
  `skill(0,20,88,100)` (else late), jumping when the shooter has been up
  `r % 3` frames (none against a full meter) or shooting 18, a late jump
  after `fps × skill(0.95,0.95,0.45,0.45)`, within `7 − r/10` and no more
  than `2 − r/10` farther from the rim (`FUN_00107198`); a steal
  (`FUN_00107388`) every 12 frames within `skill(8.5,6,6,6) − r%150/100 +
  0.5` (+2 in front) at `skill(0,0,8,16)` + Steals/16 + front
  `skill(0,0,5,15)` + a still handler `skill(0,0,30,50)` + a full meter
  `skill(0,0,6,16)`; otherwise the spot `FUN_00108148` on the man's line
  to the rim, the gap by the table `0x3c6a00` ({42: 20, 15}, {27: 10, 4},
  {22: 8, 3.5}, {19: 4.5, 3.5}, {8: 3.5, 3.5}, {2: 3.5, 3.5}, {0: 0, 0};
  off the ball, on the ball), the speed level by `0x3c69e0` × `skill(3,
  1.8, 1, 1)`, raised a level at a time after a hold of `fps ×
  skill(1,0.25,0.25,0.15)` (`FUN_00107c40`). Blocks and steals are behind
  the drone switches `blocks_on`/`steals_on` (`0x3c69b8`, `0x3c69d8`),
  set to 1 at `0x109408`.
- **The team frame** (`FUN_0018a708`): a player the computer takes over
  drops his queue (not while frozen for the toss); team+0xa0 keeps the
  side's last holder; team+0x94 counts by slot two a frame while a
  player's openness is over 4, wrapping at `fps × 4` (cleared on
  defence); a new strategy is started, then updated; a person's R3 (L3
  during a shot) calls it (vt+0x44); the turbo is saved below the
  strategy's low mark (vt+0x34) until its high mark (vt+0x3c): base 0/0,
  scramble 0.4/0.5, fast break `skill(0.8,0.8,0.7,0.5)`/`skill(0.9,0.9,
  0.7,0.65)`, defense by catch-up table `0x3ccdf8`, playbook 0.4/0.55 on
  Easy else 0.25/0.35; team+0xb8 is drawn again (`r % 9 + 9`) when it
  runs out. The turbo it reads is the side's controller object
  (`FUN_00190bb0`); the rewrite's stand-in is the side's lowest player
  turbo.
- **Tip** (`FUN_0012e4a0`): everyone "wait for toss" (`FUN_0011af98`)
  until the phase leaves 1. The jumpers' contest is actor code
  (`FUN_0015eaa0`, run by the actor update `FUN_00156f48` while the toss
  is up; ported as `World::tip_off`, state `World::tip`, snapshot 26):
  `DAT_003d6578[side]` the timing (-1 unset), `DAT_003d6570` the winning
  jumper's actor index (-1), both cleared by the actor reset
  `FUN_00156870`. A person's jumper (slot 0, `actor+0x280`) presses shoot
  (`actor+0x7c & 0x80`) for a timing: with the toss above 0.8 of its
  apex (apex from ball+0x44 and the per-update gravity ball+0x118),
  `round((y − 0.8 apex) / (0.2 apex) × 20) + 1`, clamped 1..20; too low,
  nothing (he may press again). The first jumper (actor 0 or 3) to update
  with the toss falling (ball+0x44 < 0) and |round(x)| < 5 decides: an
  unset side takes `r % 20 + 1` with no person on it (`FUN_0018ab00`),
  else 1 (team 0 drawn first); the score is `height + Rebounds × timing`,
  the larger winning (strict, so a tie is team 0's). Raw `0x15ecf0`: the
  loop reads the record of `FUN_001408e0(game, 3 × best)`, best starting
  at -1 (`FUN_001408e0(game, -3)` is actor 0, `slti`/`div` at
  `0x1408e0`), so both scores use actor 0's height (attribute 8) and
  Rebounds: only the timings decide unless his Rebounds is 0 (then team
  0). The winner starts script `0x457a7b` (`a1 ef` 25.0, 15.0, -1.0), the
  loser one of `0x457a12`/`0x457ae4` (`r & 1`; operands 10.0 and 11.0)
  with `actor+0x1a8` = the toss's frames left (no catch). The tip: an
  actor whose hand bones (`0x13`, `0xf`) come within 1.25 of the ball
  holds it (`FUN_0014cb08`) and passes at once (`FUN_0015f930`) to
  `actor+0x74`, which `FUN_001587d8` sets in phase 1 to actor 2 or 5 (the
  slot behind him). The retained attract capture's tip is at about 14.7
  on the way down (sample 655, ball state 4). (2026-10-16: the jumpers'
  scripts, their aimed `a1` jumps, the rewrite's hand and the 40-update
  tip pass are in "Pass flights and the tip-off"; `toss::TIP_HEIGHT` and
  the generic jump are gone.) The toss
  spots and the restart spots use the side as the attack direction
  (`restart::side`: the simulation's team 0 attacks -x, so its side is
  -1; `FUN_001404e0` puts the rim a side attacks at side × 36.75); the
  earlier port took team 0's side as +1, which put each jumper in the
  half his team attacks and every restart formation in the wrong row.
  (Since 2026-10-16 the simulation is in the original's frame, team 0
  attacking +x with side +1, the original's byte: "The court's frame".)
- **Scramble** (`FUN_0012cbb0`, tail `FUN_0018b0a0` on the frames the
  team timer runs out): after the first frames each player but the
  nearest who is not rebounding and is 15 from the ball and from where it
  will be in 10 frames (or is the third nearest) takes his job: with his
  side last on the ball "open loose ball" (`FUN_0010ea98`) in its front
  court or "outlet" (`FUN_0010af90`) behind it, else "defense".
- **Rebounds** (offence `FUN_0012db90`/`FUN_0012dcb0`, defence
  `FUN_0012de70` after a 3-frame wait): the spots `0x3cf620` are dealt
  once a frame by `FUN_0012d790` (six 76-unit sectors from 28 round the
  rim, taken by distance to the rim, each spot `r % 76` into its sector
  and 3 + `r % 35`/10 out, 5 + `r % 40`/10 after a shot from beyond 16);
  "prepare rebound" (`FUN_001161e8`, two constructor draws) walks there;
  the jumpers (`FUN_0012d540`) go for the ball ("rebound",
  `FUN_00116860`: the jump timed from the player's height); the
  offence's "box out" (`FUN_00117290`) against a person camping in the
  paint; the defence's goal tender (`FUN_00116fb8`) at `skill(0,0,50,
  80)` (the rewrite's reading of the odds' use); "drift up court"
  (`FUN_00116580`) for the third man back.
- **Fast break** (start `FUN_00125b60`, update `FUN_00125c70`): lanes by
  `0x3cddb0` from the three players' lanes across the court
  (`FUN_0017e4a0`), a person keeping his (`FUN_00125ff8`,
  `FUN_00126098`); "fast break" (`FUN_0010a430`) runs the lane to past
  half court; the strategy's shot with the clock running down
  (`FUN_00126a98`), its passes ahead (`FUN_00126288`, `FUN_00126950`)
  and the pocket and cancel calls.
- **Defense** (`FUN_00122ed8`, update `FUN_00123110`): each defender
  guards the opponent in his slot; the switches (`FUN_00123510`), the
  pick-up of the ball (`FUN_00123a68`), the double-team on a person's
  call (120 frames), the help and second men (`FUN_0017f238`).
- **Off-ball players.** "perim player" (`FUN_001121e0`, tables
  `0x3c94a0`/`0x3c96f0`/`0x3c97b8`/`0x3c97e8`), "post player"
  (`FUN_00114968`, `0x3c9d48`/`0x3c9d68`), the cuts (`FUN_0010fae8`:
  "cut to" `FUN_00111858`, "net cut in" `FUN_00110868`, "circle cut in"
  `FUN_00110c48`, "cut out" `FUN_0010fd78`, done `FUN_0010fb78`), "post
  cut" (`FUN_00112b80`), "post pause" (`FUN_001135a8`), "pause"
  (`FUN_0010f748`), "perim pause" (`FUN_00111d50`).
- **The playbook** (`FUN_00126f60`, start `FUN_00127318`, frame
  `FUN_001276f8`; vtable `0x43aee0`): roles each frame (the handler 1;
  of the other two slots `0x3dce68` the lower 2, the higher 3); a full
  meter sets the meter mode `[0x3d]`; on a person's side the perimeter
  man becomes the post man when his lane is open or on a coin, then only
  the post-cut and alley-oop checks run, then (no teammate setting or
  rolling off a pick, `FUN_0012ab38`, and `[0x27]` run out) the first
  teammate placed for a pick (`FUN_0012abe0`: out of the paint within
  33.75 of the rim, within 15 of a moving person holder and inside 398
  units of his facing, no more than 2 farther out, the holder's defender
  within 15 of him and 33.75 of the rim) sets one (`FUN_0012b0c0`: "set
  pick", in the mode for a holder wider than 27.75; the third to the
  perimeter; `[0x27]` = fps); a person's strategy call (vt+0x44,
  `FUN_0012ae70`) sends every teammate leaping with the holder in a shot,
  lay-up or dunk (`[0x3c]` = fps × 3), the nearest teammate to set a pick
  outside the paint (`[0x43]`), and otherwise (in the paint, or a pick
  already under way) everyone to the perimeter (`[0x44]` = fps × 0.6;
  `FUN_00181808`, on screen, is taken as yes); else the pocket and
  cancel calls (`FUN_0012ebf0`, `FUN_0012ed28`), the keep-away
  (`FUN_0012b1f8`, vt+0x74: with `[0x45]`, the other side's meter full
  and ours not and over `skill(24,16,16,10)` on the clock, the best
  ballhandler (`FUN_0012b380`, the later slot on a tie) runs "keep away"
  (type 0x12, `FUN_0010d4c8`/`FUN_0010d520`/`FUN_0010d590`: stand until
  an opponent is within 10, then to a spot near half court,
  `FUN_0010d800`, two draws, wait `fps × skill(1.5,0.8,0.6,0.2)`, jukes
  every other chance) and anyone else passes to him; the frame ends
  there), then the play: when it is done
  (`FUN_001285d0`: its vt+0x1c, or its handler pausing within 12) the
  star is dropped (`FUN_0012aaa8`: kept only with 15+ s on vt+0x5c, which
  is mod 60) and a new one chosen (`FUN_001281c8`): none while a pass or
  shot is under way or the pass check's wait runs; the forced play `[0x2c]`
  (always −1); pull up when `[0x3e]` (catch-up table 1 = 100, never in
  the tables); penetration when `FUN_001287e0` says so (the holder in the
  front court ahead of both teammates, still in the back court: the star
  always, anyone else `skill(0,10,10,40)`%, corrected 2026-10-17 from
  the whole side in the front court; or
  an open lane in the paint, with no teammate 5 nearer or no defender 2
  nearer); else with a star the plays that suit him plus `FUN_00128708`'s
  weight (Handles over 49: penetration and isolation; power: back down and
  post up; Threes: isolation 3pt twice off Easy, pick and roll 3pt); else
  the default list `0x3ce438` (each play once, once more when it suits the
  holder; isolation 3pt again off Easy); one draw (`FUN_00128a68`).
  `FUN_00128668` refuses the last play and, in the meter mode, plays not
  fit for it (vt+0x5c). The start draws in order: the shot clock mark
  `r & 3 + 1`, `[0x3d]` against catch-up table 1 (`0x3dc13d`), Easy's
  dropped draw, the penetration mark `r % 5 + fps × skill(1/3, 2/3, 1/6,
  1/6)` (0.15 at most when an inside basket would end the game, 0.25 at
  least in the meter mode), the post-cut mark `r % 5 + fps × skill(11/6,
  13/6, 1, 1)`, the keep-away `skill(0,50,100,100)`. The star
  (`FUN_0012a7d0`) is the best share of rating 9 scaled by points and
  attempts per second on court (stats 0 and 10 and actor+0x2a4, kept by
  the sim since 2026-10-15: "The meter's hold and decay; the match's
  statistics" in section 7). After the play: the shot check
  (`FUN_001290b8`: at the clock mark always; a full meter shoots with
  under 2 s of its 30 left (`FUN_00129748`: team meter+0x18, the hold the
  sim's meter keeps, `TeamState::meter_hold`) or starts
  a pull up; `FUN_00129590`'s open three;
  else by the clock a quality of `now` on the timer frames or of `need`
  one time in eight every `fps × 0.6`, 12..29.75 out), the pass check
  (`FUN_00128b30`), the pass out (`FUN_00128ec0`: a covered handler out of
  the paint or with a shut lane, every `fps × 0.5` frames, `fps × 3` after
  one; the choice `FUN_00182d10`, raw `0x182d10..0x183310`, whose decompile
  stops at the EE `sqrt.s` `0x46050044` at `0x18319c`: Handles 70+, turbo
  0.2+, on the floor, able to pass; with the stick at rest and a still
  defender (not in bit 16) 4..12 away and Handles 60+ off his head (Off the
  Heezay), else with the stick (plus the camera's yaw, taken as 0) within
  256 of the rim's line, or for the alley-oop's lob, off the board: in the
  attacking half within 35 of the middle and inside `(|z| − 3) tan 20° ≤ 38
  − |x|` (the polynomial sine/cosine of `0x3eb2b8c3`), a pass out only
  within 8 across and 29 in x; odds `skill(0,20,40,60)` for a lob off the
  board else `skill(5,5,25,25)`, one draw; type 0x17 `FUN_00100c60` (one
  draw for its 6..8-frame wait, update `FUN_00100d70`) stops and presses
  two turbos and pass to himself, `FUN_00183318` asking the game for the
  throw (two or three draws off the board): the rewrite throws the sim's
  Off the Heezay or Back 2 Papa directly, `World::computer_trick_pass`, and
  an off-the-board lob to a teammate goes as a plain pass),
  the penetration (`FUN_001298c8`), the post cut (`FUN_00129a20`) and the
  alley-oop (`FUN_00129d98`: odds by Dunks and the difficulty,
  `0x3ce470`/`0x3ce480`, the "alley oop" play or a leap, a second
  leaper). The subclass (`FUN_00124028`, vtable `0x43a998`, modes 1, 2, 6)
  starts without the catch-up table and Easy draws (marks `r % 5 + 9` and
  `r % 5 + 15`), runs the pass out before the pass check, shoots only a
  dunk (`FUN_00124618`), and its `FUN_00124ed8` answers yes for any
  holder (both exits return 1), so `FUN_00124ad0` always starts a
  penetration; its list `0x3cd358` is never reached.
- **The plays** (`ai/plays.rs`, vtables in order `0x43a428`, `0x439638`,
  `0x439968` ×2, `0x439c18`, `0x4397b8`, `0x439dd0` ×2, `0x439f80` ×2,
  `0x43a5a0`, `0x439b00`, `0x43a718`, `0x43a110`, `0x43a290`,
  `0x4394b0`): +0x1c done, +0x24 start, +0x2c update, +0x34..+0x54 the
  checks that may interrupt (pull up none, alley oop no pass check,
  isolation and rotate their own), +0x5c fit for the meter mode, +0x64
  suits a player. Ported: post up (`FUN_00120ff8`, `FUN_001212c8`), back
  down (`FUN_0011bb10`), isolation (`FUN_0011cad8`, `FUN_0011cf18`,
  `FUN_0011d1d0`), penetration, cutter (`FUN_0011c120`, `FUN_0011c300`),
  pull up (`FUN_00121870`, `FUN_00121960`, `FUN_00121c78`,
  `FUN_00121e78`), pen and kick (`FUN_0011dad8`, `FUN_0011dce0`,
  `FUN_0011dfc0`), rotate (`FUN_00122490`, `FUN_00122670`), post and kick
  (`FUN_00120188`, `FUN_001203f0`), post cut (`FUN_00120ab8`,
  `FUN_00120c20`), alley oop (`FUN_0011b488`, `FUN_0011b590`); the
  behaviours they need: "carrier move" (`FUN_00101698`, `FUN_001017a8`),
  the leaper (`FUN_00100100`, `FUN_001001c0`; its shoot press is the
  sim's Triangle off the ball) and the second leaper (`FUN_00100360`,
  `FUN_00100470`), the post up's feeder `FUN_0011a580` and "post up"
  `FUN_00115368`. The pick and roll (`FUN_0011e900`: a handler who would
  not screen hands to one who would; `FUN_0011eac0`: the star or the
  nearer of two teammates up the court screens, cutting to the zone
  beside the handler's on his side of him round the rim (+0x40) or, from
  the paint, posting up on the handler's side, the handler runs "run pick
  play", the third goes to the perimeter, both AI+0x5c clear;
  `FUN_0011ef18`: either AI+0x5c ends it; "set pick" once both stand
  still in their zones or both have stood still over `fps × 0.4`; done
  `FUN_0011edb0`: `fps × 6` or the third man has the ball) and the pick
  off ball (`FUN_0011f628`: from a corner or the baseline, or for a star
  handler, a hand-off `FUN_0011f940` to a fit teammate `FUN_0011f850`,
  else to the top `FUN_0011f7c0`; `FUN_0011f9c0`: at the top the
  perimeter man cuts to his wing and the post man to his block; then the
  cutter goes in by the net (one draw) behind the post man's "pick off
  ball", and the handler hits him once he is beyond 11 of the rim and
  within 55 frames of where he is headed; done `FUN_0011f510`) are
  ported.
- **The restart** (game phase 6; `ai/inbound.rs`, `World::start_restart`,
  `World::restarting`): `FUN_001406b8(game, team)` sets phase 6 and
  game+0x110, clears game+0x4a4 and the six ready flags game+0x404..0x418,
  calls game vt+0x134 = `FUN_00140880(1)` (every `actor+0x298` = 1, so
  `FUN_001587d8` takes every actor's pad from the AI, people included,
  while the team frame's takeover test still reads the controller type),
  lets go of a ball the other side holds (`FUN_0014c408`) and adds 1.0 to
  each of the six controllers' turbo (`FUN_00190c68`, a full bar). It is
  reached after a basket (listener `+0xd4`, `FUN_00141818`) and a
  shot-clock violation (`FUN_0013f1f0`, which leaves the clock run out).
  `FUN_0013e4c8` case 6 goes to phase 4 (and `FUN_00140880(0)`) once
  `FUN_00140840` (game+0x104 clear and all six flags up) holds and nobody
  holds the ball. The possessing team's start `FUN_0012bc40`: the
  inbounder (a carrier on the side, `FUN_0018c018`, else the nearest to
  the ball within 100, slot 2 by default); the row nearest the ball in
  the side's frame among all six on a full clock (`clock[0] == clock[1]`)
  else rows 2..5 (`DAT_003ce9b8`, written as found; game class 4 forces
  2); the second spot to the nearer of the others to spot 0 with fewer
  than two people on the side, else the first other slot a person drives
  (slot 1 if none); the third the other. Spots are read before
  `DAT_004e7408` is rewritten, so their height push is the last
  restart's. The inbounder's queue becomes "loose ball" mode 3
  (`FUN_0010dd68`) then "throw inbound" (`FUN_0010b658`: vtable
  `0x437aa0`, done `0x10b7d0`, update `0x10b820`; receiver the second's
  slot; the wait `r % (fps × 0.2) + fps × 0.1`, 6..17, raw
  `0x10b6c4..0x10b740`, lesson `fps × 2` or `fps × 0.5`); the other two
  clear and queue "inbound pos" (`FUN_0010ba18`: vtable `0x437a48`, done
  `0x10bab8`, update `0x10bb10`; radius `FUN_0012bad8` = the row's third
  word). A carrier on the other side hands the ball over
  (`FUN_0014cb28`); one on this side (never: he is the inbounder) would
  pass ("pass", `FUN_00100a08`). Then `DAT_003ce9b0` = 1, the order, the
  look timer `DAT_003ce9bc` = 10, `DAT_003ce9b4` = 0. Its update
  `FUN_0012c210`: every 10 frames, not while a pass flies, an inbounder
  without the ball goes after it again (unless "loose ball" is in
  front), one holding it without "throw inbound" in front takes it up
  again, and a ball in another's hands lowers both their flags and
  restarts the strategy (`DAT_003ce9b4` = 1). The defence's start
  `FUN_0012c520` clears the three; its update `FUN_0012c578` queues
  "inbound pos" for slot `order[k]` at spot k (radius + 10 after a
  second choice) once `DAT_003ce9b0` is set. "throw inbound": walk at
  level 3, 2 within 6, 1 within 2 (avoid mode 1), stop within 1.4, turn
  to the face point (`FUN_0013b2f0(…, 1, 0)`), raise the flag while
  holding the ball; with all six up let go, count the wait down, then
  press pass to the receiver (`FUN_0013b810`) every other frame; its first
  frame posts `0x3c6510` (not modelled). "inbound pos": raise the flag
  within the radius when it is over 2; walk (level 2 within 3), stop
  within 1.4, turn (min 0x39), then stand and raise the flag; with the
  ball, unturn and stand. The rewrite's own: only the restarting side
  picks up a loose ball in phase 6, the clock fills at the inbound,
  a limit of 20 s (`tuning::RESTART_LIMIT`) after which the old inbound
  puts the ball in play, worlds without six players keep the old timed
  dead ball, and lessons stage it (`World::stage_dead_ball`; the
  original's `FUN_0018ec28` snaps instead). The players' box is widened
  to the farthest restart spot (48 by 32.5). In the rewrite's computer
  games every restart ends with the pass, in 4..6 s.
- **Contacts** (`contact.rs`): the contact manager `FUN_00133bb0` (once
  an update) pins post-up pairs, runs the scripted contacts (`actor+0x198`
  type, `FUN_00139408`, handlers 1..7 of `FUN_00134250`'s table
  `0x3d0c30`) and, for each pair `i < j` closer than the radii
  (`FUN_00133b48`: 1.25 each plus what the playing animation's five limb
  bones add toward the other within ±0x100, `FUN_001337e0`, table
  `0x43d060`) that `FUN_00136218` allows (not teammates in `0x88080`, not
  a post-up pair), handler 0 `FUN_001343e8`. That one swaps so the
  first is the defender (and, in phase 1, the second the tip's winner),
  takes the line between them (raw `0x134760..0x1347d0`, the EE `sqrt.s`
  that stops Ghidra; (1, 0) under 0.001), shares the overlap by weight
  (attribute 9; two airborne rebound jumpers add Rebounds and 50 for the
  higher root), then in order: phase 1 or a still opponent second → the
  first takes it all (the screen); a juker; on the floor the mover
  against a still man; a fallen player is not moved; the pass's
  receiver is not moved; rebound and block jumpers against men on the
  floor; the `0x8e088` states are not moved; two partial shares on the
  floor are refined by the giver's velocity along the line (the
  heavier teammate's otherwise). Each push is capped at 0.4 (`0x3ecccccd`)
  and a jumper is not pushed past |x| 38 (to 37.9) within 4.4 of the
  middle. Ported as above with each player's weight `Player::weight`
  (200 until a roster sets it; street-game does not yet), the radius
  without limbs (`FUN_001337e0` reads the playing animation's bones,
  which the sim does not evaluate), and the bits read from the sim's
  actions. The screen reaction (raw `0x135400..0x135570`) is ported: each
  offence man's count `0x3d0f70` (the opponent he last pushed, contacts
  in a row; a teammate or another opponent starts it over) and, for a
  moving one without the ball (someone holding it) with the defender not
  off balance, 30 contacts (`0x3d0f68`), facing within 160 of him
  (`FUN_0018d1f8`), both on the floor: three draws (`0x3d0fb0[r % 3]` for
  his animation; the weight on equal power (attribute 4), `r % 100 < 51`
  light `0x3d0f38` else heavy `0x3d0f28`, the weaker defender heavy, the
  stronger light; the defender's animation `r % n` from the list for the
  quarter `FUN_0018d9f0` gives, 5/3/4/3 or 1/2/1/2), the count rests at
  -600, and the message `0x3c6910` → `FUN_00141448` (the AI game
  listener's vt+0x254; `0x3c68f0` from "set pick" goes to the empty
  `0x132da0`) sets the defender's AI+0xb0 to the screener (never cleared),
  which the defense's switch `FUN_00123510` reads. The animations are not
  played; the defender's is the rewrite's stumble (`PICK_STUMBLE`). The
  scripted contacts and the other reactions (strip, charge, post-up) are
  not ported.
- **Pick behaviours** (`ai/picks.rs`). "Set pick" (`FUN_00118508(ai,
  mode)`, type 1, vtable `0x438f90`: start `0x1188c8`, done `0x118778`,
  update `0x118a30`, raw throughout): the holder at construction, his
  defender, the side (+0x40: which way round the rim the holder is, or
  for the person's wide call the side by his z; also the screener's
  AI+0x60), `fps × 1.1` frames to arrive (+`fps × (d − 10) × 0.1` from
  beyond 10); runs (turbo pressed at level 3, 2 within 5.5, avoiding
  beyond 5) to the point 20 units (`0x4b4efc`) round the defender's circle
  about the rim; arriving (1.4, or within 3 of the defender) waits `fps ×
  0.25` frames standing (`0x400000`), two of them faced to the defender,
  counting two a frame while the defender is on the wrong side; then
  within 4.5 steps into him (`0x4000000`, five frames held), else rolls;
  a defender down ends it in a roll; once the defender has touched him
  (2.7) and the holder has gone by, half a second more; the holder more
  than 20 from the defender ends it in `fps × 0.1` with AI+0x5c set. Done
  when the holder strays 20 from where he was before the arrival, is past
  the screener and inside 23.75, inside 13, or the defender beyond 33.75,
  or the time runs out. The roll (`FUN_00117e80(ai, way)`, type 2, done
  `FUN_00117ef8`, update `FUN_00117f68`; `0x4b4dc8` = fps × 1.5, `0x4b4dcc`
  = 256, `0x4b4dd0` = 5): a quarter turn off the line to the rim closing 5
  a frame, turbo, to the rim; with the ball a drive; a pass coming, he
  stands. "Run pick play" (`FUN_00118f60`, vtable `0x438f38`, done
  `0x119110`, update `0x119180`, target `0x119150`; the constructor's
  spot is the zone's `FUN_00180f80` spot pushed to 27.25 from the rim by
  VU0 code, raw `0x119040..0x119100`): to the spot, an embedded "carrier
  pause" (0, 7) until the screen comes (within 8 of the screener, or the
  screener within 4.5 of the defender), round the screener 14 units off
  him a step farther out (level 2 for `fps × 0.15`, then 3), past him (the
  3pt play) out to 26.75 for a shot of 5+, else a pass to the screener
  (`FUN_0013b840`) when the handler is covered under 5 and the screener
  faces the rim within 171 and is open `hi`, or `lo` with the handler's
  lane shut (`(lo, hi)` = (6, 7) for a star handler, (4, 5) for a star
  screener, else (5, 6)), or a drive from inside 16. "Pick
  off ball %d" (`FUN_001199e0(ai, man)`, done `0x119a60`, update
  `0x11a008`): to 2.5 from the man's defender along the man's facing
  turned an eighth toward the defender's line (level 2 within 6), wait
  `fps × 0.2` (`0x400000`, faced the last two), step in (`0x4000000`
  within 4.5); done after `fps × 5`, the defender beyond 40, the holder
  within 13 of the rim, or (counting two a frame while beyond 5 of the
  defender; `FUN_00119d28`'s angle test is always true: 512.5 truncated
  bounds a wrapped turn) `fps × 0.15`; then a post cut to the far block
  (deep unless `r & 3` is 0) or, with the third man headed into the
  paint, the perimeter.
- **Handler behaviours, second pass.** The drive's start
  (`FUN_001096b8`) in the paint, not patient, without a full meter, posts
  up instead (no reach draw); the patient drive (`FUN_001095d8(ai, 1)`,
  `FUN_00109df8`) goes at the nearest standing defender until a juke is
  on (turning at least 57 to him), backs off a defender within 8 without
  turbo, until open over 6. "Post up" (`FUN_00115368`, type 6, three
  draws; start `FUN_001154b8`, done `FUN_001155b8` at `fps × 4`, update
  `FUN_00115628`): to the paint; a wait when shut in (`r % 24 + 12`, a pump
  fake one time in four at openness 3..5); within its reach (6..10.9) the
  move (off the baseline; a pass out of a shut lane with a poor shot;
  straight on `skill(0,50,100,100)` or with nobody within 6; else a turn
  from `0x3c9e70` by the defender's eighth, a coin and a draw); then the
  shot, a blocked lay-up's pass out, or every other frame's shoot press
  (`0x3c9e68`) in a jump shot under a taller defender (`FUN_001837d0`);
  the avoid mode `FUN_00115d48` every `fps × 0.1`. The feeder
  `FUN_0011a580(handler, partner, zone, deep)` (vtable `0x4391f0`): to the
  zone beside the partner's (the top within 15 across; `0x3cae78` for a
  post), a "carrier pause" there; once near, the partner's still frames in
  his zone (a post within 16/18.5); the pass after `fps × 0.2` with a
  shot under 6 or from beyond 38.75, after `fps × 0.4` anyway.
- **Not ported** (the rewrite's stand-ins are labelled in the code): the
  pick animations (families `0x28a..0x28c`) and the other contact
  reactions; the sim's own pick on a person's Triangle (`World::strategy`)
  still runs beside the AI's call; the subclass's Gamebreaker Wins
  keep-away clause (game+0xf8, team meter+0x20); the camera yaw in the
  trick pass's stick test; `FUN_00181808` (on screen) and actor+0x1a0
  (read as 0); the pad gate
  `FUN_0013a970` applies when `0x3d17d0 == 0` and the shot clock runs
  (unresolved: the rewrite runs the AI every frame); power and the overall rating (stand-ins); `FUN_00141018`; the story modes' skill weights; the
  actor-state bits are read from the sim's actions (bit 3 jump shot, 7
  dunk, 13 lay-up, 9 pass, 10 juke, 12 block jump, 8 rebound jump, 15
  alley-oop leap, 16 and animation flag 0x40 down; bits 3, 7 and 15
  corrected 2026-10-16, "Defence and spacing on the original's speed
  levels"); `FUN_0016c500`'s dunk window is
  `World::dunk_on` since 2026-10-17 (the sim's `DUNK_RANGE` before); the
  game's polynomial `atan` is `libm`'s.

## 7c. The crowd (traced 2026-10-15)

Ported in `crates/street-render/src/crowd.rs` (`Director`), driven from
`crates/street-game/src/game.rs` (`crowd_reactions`); the data in
`crates/street-assets/src/crowd.rs`.

- **Vertex animation.** The model class (`0x308830`, vtable `0x4991d0`)
  builds a table of every frame's (record, key, deltas): record `+8` the
  key (f32 xyz per vertex), `+0x10` the deltas, frame `f` at
  `+0x10 + f * vertices * 3`, scales at `+0x14..+0x1c`. `FUN_00309ff0`
  decodes a frame with `lbu` loops (`0x3242b0` for 139 vertices,
  `0x324330` 61, `0x3243b0` 60, `0x324230` 49, `0x324430` 151; the C
  fallback reads `byte *`): `key + (unsigned byte) * scale`. The rewrite
  read the bytes as signed, so every delta past 127 fell 256 steps the
  wrong way; the cheering animations, which move most, tore spectators
  into zigzags. Waving hands still move up to 8.8 model units in a frame
  (a spectator is about 22 tall).
- **Seats** (`FUN_0030b9b8`, seat record 0x134 bytes): +0x18 the section
  letter (`FUN_0030b7c8`, for the draw lists `FUN_0030b708`), +0x1c the
  plan's column 8 (the head kind: heads are counted, loaded and assigned
  by it, and kinds 2/3 take the shorter height range), +0x20 the colour,
  +0x24 the role from the name (`FUN_0030b7e0`: `ST` 0, `SD` 1, `C` 2,
  `W` 4, else `DAT_00403030` = 1), +0x128/+0x12c the two `t.bin` textures
  (body `+0x34` via `FUN_0030e1d8`, head via `FUN_0030e218`), +0x130 the
  group (column 7). The rewrite had taken the head from the name. Column 9
  is read and dropped. Column 10 is `strtoul`'d into +0x20.
- **Colour** (traced statically 2026-10-15, `Seat::vertex_colour`).
  `FUN_0030e250` hands +0x20 to the head model and the body model alike
  (vtable `+0x3c` = `0x30a450`): each byte halved, the middle two swapped,
  into the instance's +0x50. The draw `0x3090d0` (`0x309964`) unpacks that
  word most significant byte first into the instance's qword 10 (VIF
  `UNPACK V4-32` at TOPS+6, six qwords: matrix, colour, TEX0). Every
  part's VU program, bodies and heads, loads qword 10 (`LQ vf06, 10`) and
  stores it unchanged (`SQ vf06`) into the qword after the GIF tag that
  follows the input vertices (`ruckerB` part 0: tag at 98 with `PRIM`
  strip, `NREG` 2, registers RGBAQ and TEX0_1). TEX0's TFX comes from the
  texture object's +0x10 bits 25..26 (`FUN_0036aa00`), which
  `FUN_00368ed8` clears (modulate, TCC 1). So the plan's `0xRRBBGGAA`
  modulates the texture at half scale: `0xb65a9bff` is (0x5b, 0x4d,
  0x2d, 0x7f) / 0x80, a khaki. The rewrite had read the word as RGBA at
  full scale on the body only: `0xb65a9bff` became a pink 1.4 times
  over-bright in linear light, which flattened the shirts' shading so that
  the seated rows behind Rucker's +x baseline (role 0, their own
  sitting animations 0..9) looked torn into flat shards while the
  untinted heads looked fine. Every frame of every model decodes whole
  (rendered offline); the geometry was never torn. Far rows' words such as
  `0x273527ff` now draw at about 15 % (dark), as the original's would.
- **Graphs** (`FUN_0030e3e0`, from the role flags): nodes of 0x18 bytes
  (next list, category, animation, velocity xyz), lists of a count and
  node pointers, the roots in `PTR_DAT_00403058` (roles 0, 1, 2, 4; role 3
  none). Categories: 2 idling, 0 cheer, 1 gasp. Animation indices run on
  across the roles present: 10 for role 0, 8 for role 1, 7 for role 2, 1
  for role 4 (every court's models have exactly that many records: 18,
  25, 26 or 15). Role 4's node moves 0.6855 (`0x4da0b4`) model units a
  tick along the seat's own z (`FUN_0030ebd8` → `FUN_0030eb20`,
  `FUN_00379a88` multiplies the translation in ahead of the matrix): the
  pedestrians of Center and the Cage.
- **Groups** (`FUN_0030dc68`, 0x18 bytes each, `body+4` of them = the
  plan's highest group + 1): node, timer (only ever decremented by
  `DAT_004d9c94`, which is never written: inert), wanted category 2, role
  5, walk count, 1800. `FUN_0030ea48` per seat sets the role and the
  role's first node, and for role 4 a list of five walkers that
  `FUN_0030e2c0` fills.
- **Playback** per model per group (0x30 bytes): `+0x4c` (`0x309fa8`)
  sets period, animation, `remaining = frames - 1`, frame = the record's
  first; `+0x5c` (`0x309eb8`) takes the step off the timer and, at or
  below zero, adds the period and shows the next frame, or returns 1 when
  none remain. The loader sets every group of every model on animation 0
  at period 1, then runs `FUN_0030dd50(1.0, g)` `rand() % 100 + 100`
  times per group.
- **Each tick** `FUN_0030c9e0` (vtable `0x4995a0` +0xc) runs
  `FUN_0030db18` unless the pause menu's `DAT_00405388` is set
  (`FUN_002f3560`/`FUN_002f3638`). `FUN_0030dd50`: the walk step (after
  1800, back by 1800 steps); when the body's animation ends, the current
  node's list gives a random node of the wanted category (`FUN_003b42a0()
  % count`), else the last idling node whose list is the role's root; it
  starts at period 4.0 and the group wants 2 again. Each head model
  advances on its own clock and, at its end, starts the body's pick: head
  and body records have equal frame counts, so they stay in step.
- **Calls.** `FUN_001a75a8` (the audio listener's crowd call) computes
  `f = max score / points to win (FUN_001e0bc0()+0xf0, 21 if unset) * 0.5
  + 0.5`, at most 1, and calls `FUN_0030d618(f, 0x4d9a80, event != 0x29)`
  → `FUN_0030ca10`: `trunc(groups * f)` distinct random groups
  (`FUN_003a16b8`) want 0 (cheer) or 1 (0x29, gasp) through
  `FUN_0030e3c0`. Its callers: 0x27 from `FUN_001a7708` (basket `0x1a3b10`,
  block `0x1a4ce8`, steal/counter `0x1a63e0`, rebound pickup `0x1a65c0`)
  and from `FUN_001a6b80` when +0xc is set (a basket while
  `FUN_00141018` holds, a gamebreaker); 0x25 from slot 22 `0x1a5d10` and
  the dunk slam `0x1a5d50`; 0x29 from the floor contact `0x1a5348` and a
  trick landing `0x1a5a78`.
- **Ported 2026-10-15:** the floor-contact call (gasp, `sfx.rs`
  `floor_contact`) and slot 22's (cheer at the tip-off's take, found as the
  one pass while the toss is still up, `sfx::tip_taken`, used by
  `crowd_reactions`).
- **Not ported:** the generators' shared sequence (the rewrite draws from
  its own LCG state).

## 8. Not traced yet (in priority order)

`street-sim` has since taken the chain (`crates/street-sim/src/combo.rs`):
the six-slot ring and its push/shift, the class-value fallback table, the
pattern gate (the 55-row `0x43c500` walk), the dynamic rows and the depth
ramp with −1.0 dropped and the two-swipe column penalty, cashed at the
baskets only. These are what remains of the combo system:

   - the **matcher, re-read raw 2026-10-15** (`0x131970..0x131cf4`;
     `combo::Chain::evaluate` now follows it step by step). The pre-scan
     walks from slot 5 while the slot is filled and its age (the newer
     event's tick, or the game tick for slot 5, less the slot's own tick
     +0x8 and word +0x10) is within `DAT_004b71a8`; `t1` is the first slot
     outside that window. The rows come from the newest event's class
     block, `{first row, count}` at game `+0x94 + class × 8`, which the
     constructor `FUN_001313d0` fills from the table: classes 0..7 rows
     `5k..5k+4`, 19/20/21 rows 40/45/50 (five each), every other class
     none (no match, no payout). When `t1 ≠ −1` the block's leading rows
     whose kind at `t1` is not −1 are skipped, so the first row tried
     asks for exactly the window's depth; a lone event skips the whole
     block and pays nothing. The walk checks kinds newest first (−2 any,
     −1 stop; all 55 rows' six extras are −1) and stops at the window's
     edge; a 9/10 class right after a 9/10 class counts a pair. The
     first clean row pays: its id to `+0x148`, the column `4 − t4` (`t4`
     the oldest walked slot) less the pairs when `DAT_003d0708` = 0 (the
     shipped value), and the sum over slots 5..`t4` only, newest first,
     `value × 0x43be58[row × 5 + column]` in f32, `cvt.w.s` to `+0x150`.
     The weight table has 47 rows: row 46 = {1.75, 2.0, 2.5, 3.0, 3.5} is
     the alley-oop with extra 1 (`FUN_001322d0`: `(FUN_00171e28 >> 1) &
     1`); the words after it are not weights. For a basket class the
     window's depth `R` gives row `5k + 6 − R` and column `R − 2`; the
     passout rows walk only their own matching slots. The earlier note
     that the boot's zero bound makes the window the ring's depth does not
     follow: with a bound of 0 only events of the same tick would stay
     fresh, so `DAT_004b71a8` must be filled at run time (its source,
     `[0x4b71c4] × 2`, has no traced writer). The sim keeps no ticks and
     treats every filled slot as fresh;
   - the **pusher's mode gate** (`FUN_00131788`): when game `+0x424 →
     +0x50` is 3..7 only classes 6/7 are pushed, and then only if
     `FUN_00337af0` answers 1; not modelled;
   - the **matcher's `0x131ff8` family's meter reads (mechanism read,
     mappings open):** the drains dispatcher compares team+0x5c against the
     `.rodata` word at `0x43c4f4` = 3000 (`0x1320b8`, next to the
     pattern table's constants), folding the inequality into the pushed
     event's extra field (`0x1320bc..0x1320c8`: `xor`/`sltu`, stored at
     sp+0x4 — the ring slot's extra). Class selection in the same function:
     when the `s1+0x15c → +0x130 → +0x1ac` chain is nonzero the class is 17
     (`0x132084..0x13209c`); else `s1+0x154` (the drain kind) picks it —
     kind 0 usually 9 (`0x132108..0x132124`), kind 2 usually 8
     (`0x1320d8..0x132104`, animation queries can lift it to 14), anything
     else 14 — before the shared join at `0x13212c` pushes via
     `FUN_00131788`. The sim pushes class 17 extra 0 for every drain, which
     matches only the chain-nonzero path; mapping the chain, the kind values
     per drain type, and the team+0x5c writer to sim fields is still to do.
     `player+0x248`'s value class is the stamped level the
     set-builder writes (`1 << level` on the set's eleven ids, section
     5b). What the folded flag changes in the push is still to read;
   - where the boot reset's "meter field" (the ×2/×30/×10 and the
     50000/(field×600) fill rate) comes from: the field is `[0x4b70e4]`
     (and its float neighbour `[0x4b70e8]` feeds the denominator), both
     written only through indirect pointers — the eponymous direct refs
     find no boot-time value, so the rate's input is declared, not filled;
     the reader side is some walk of the 0x4b7130..0x4b7150 block;
   - **Corrected 2026-10-17:** the bound is fps × 2 = 120 (written at
     `0x132b78..0x132b8c`; RAM agrees), and the sim keeps ticks and spans
     ("The combo ring, the meter's listeners, the statistics and the shot clock"). The note below misread Ghidra's cut at
     `FUN_0030fa50`.
   - **`DAT_004b71a8`'s source (closed as a curiosity):** the boot reset
     reads `[0x4b71c4] × 2` into it and `FUN_00133578` (the whole
     0x4b71b0..0x4b7240 family's reset) writes 0 there; no other writer
     exists, so the staleness bound sits at 0 and the matcher's window
     walk collapses to the newest slot unless something beyond the traced
     set re-fills it. The sim keeps a decay-free ring; if a live game
     shows longer chains mattering, the refiller is still to find;
   - turbo drain and recovery rates: **resolved (corrected 2026-10-15)** —
     the earlier reading of `FUN_00147cf8` was wrong: that static
     initializer stores 0.0 at `0x4b7e80` and 1.0 at `0x4b7e90`, words of
     a block of vectors, and nothing reads them as turbo. The rates are
     the static initializer at `0x178860`: `1 / (fps × 5.0)` to
     `0x4b99a0` (`0x178a64..0x178ad0`) and `1 / (fps × 7.5)` to
     `0x4b99a4` (`0x178ad4..0x178aec`), fps the timing object's float
     (60). The actor update (`0x158964..0x1589d0`) spends the first each
     frame a turbo bit (0x2000) is held and `FUN_00140558` (the live-play
     gate) allows it (`FUN_00190c08`: subtract, floor 0), else adds the
     second (`FUN_00190c68`: add, cap 1); both skip under the unlimited
     turbo singleton (`FUN_00131208()+0x6c`). So a held bar empties in
     five seconds and refills in seven and a half. `street-sim` carries
     both (`tuning::TURBO_DRAIN`/`TURBO_REFILL`, per update) and, since
     2026-10-15, drains on any update turbo is held, stick or not
     (`FUN_00140558`'s gate is mapped onto the sim stepping players);
   - shot, steal and block odds per difficulty: the setup-time crew
     blend is removed (it was modelled on `FUN_00143458`'s misreading)
     and the meter blend is adopted instead — see "Difficulty" above.
     The meter side is read too: team+0x40 × team+0x44 scale every fill,
     team+0x10 is the team's own meter size (fills rescale as
     `amount × 50000.0 / team+0x10`), and team+0x48 is the
     gamebreaker-sequence factor. **The singleton's +0x60 and +0x68
     (closed):** the singleton at `0x4b6fb0` is built on first use (the
     flag at `0x3d0700`, its token `0x43bdf0` at +0x8c, the shutdown
     deleter `0x1311e0` registered through `0x3b2820`), and its reset
     `FUN_00131260` zeroes words +0x00..+0x88. No code writes +0x60 or
     +0x68 through the getter `FUN_00131208` or the global: the cheat
     checker and the two settings copies (`0x298afc`, `0x29ecbc`) write
     +0x00..+0x58, +0x6c and +0x78 only. So +0x60 stays 0 (team+0x40
     scaled by {1.0, 0.5, 2.0}[0] = 1.0) and the meter add `FUN_00143d38`
     never skips, as `street_sim::meter` plays it.
2. **Front end, remaining:** the element *kinds*' art (which images
   each widget shows, from `XAFESCRN`/`XAFEBG` sheets), the per-screen
   behaviour code behind the key bindings (`FUN_002d7fa8`'s binding groups
   0x1b..0x22 to menu actions), the create-a-player group chain (62..73),
   and then building the game's own screens in `street-game` from the
   layouts (`%TEMP%\opencode\fe_trace.md` has the class/ctor table).
3. **Mode data, remaining:**
   - the street-school pass detection during a lesson run (the store at
     rec+0x33f4 and the 26-lesson table are traced);
   - the exact split of the three cumulative cost tables at `0x4602a4`
     (rating buy vs tweak vs move edit);
   - **W/L grant gating closed:** `FUN_001ded40` calls story DP only in
     mode 5, for the selected user's winning/losing side; modes 2/3 and 4
     award RP on wins. There is no non-story 100/150/200 DP grant in this
     dispatcher (`MODE_RECOVERY.md`);
   - whether record notice kind 2 can carry a team id too (court 10 check
     in the popup suggests a shared "unlock display id").
4. **Rewards:** the 113-row table and the 17-code checker are read (see
   §5). Field 5's automatic Pick Up threshold grants, category row counts
   and zero-price ownership are now resolved and implemented. Native
   card-picture fallback and broader per-mode visual parity remain open.
5. **Original PS2 saves:** validated on the user's MAX archive. CRC32 and
   bounded LZARI decode produce a 42,020-byte game file with six u32-LE
   length prefixes: 96, 24, 2660, 6016 (32×188 user records), 32080
   (10×3208 careers), 1120 (10×112 teams). `street-assets::save` reads
   and writes this envelope without losing any byte. Constructors
   `001dadb0`, `001d6e90`, `001d3d08`, `001ef028`, `001eb190`, `001edd40`
   independently establish these sizes; `00226ec8` maps the buffers and
   `0022a9b8` reads/writes the length and payload states.
   Earlier memory-card-task interpretations are retracted: +0xc130 and
   +0xc13c are directory/path strings, not streams; `001d11b0` grows a
   string buffer. +0xc1a8 selects the card slot; +0xc1b4=13 is a state
   continuation. +0xc148..+0xc1a4 are task flags, not serialization
   cursors; bits unpacked by `002271c0` are memory-card policy, not cheats.
   GCI metadata is parsed separately; its GameCube payload is not assumed
   to use the PS2 layout. Native field conversion is tracked by the importer.
6. **Presentation:** camera modes (0x3038b0), replays, the `ssb` cutscene
   scripts in `SCRIPTS.BIN` (court intros `IntCourt*`, legend intros
   `InLegend*`), and the particle effect names.

## Resumed Trax investigation (2026-10-12)

The stopped log was searching for Favorite Song storage. The US PS2 ELF's
16-byte row descriptors at `0x467710` (front end, sentinel `0x4678c0`) and
`0x4678d0` (pause radio, sentinel `0x467a90`) resolve that question. Both
have Music Experience (kind 1), nine playlist songs (kind 6), fifteen
interactive songs (kind 5), Song Rotation (kind 2), and Exit (kind 0).
Pause adds Track Info (kind 4). Neither contains Favorite Song (`0x1604`)
or a kind-3 row. `FUN_00219f58` commits no kind-3 field. Favorite Song and
its help line survive in locale text, but are not exposed by these menus.
This is bounded static evidence for the US PS2 version, not a runtime
claim about another release. Ghidra MCP read-memory and decompilation of
`FUN_00219f58` corroborated the retained executable analysis.

The option slot is **24 bytes**, not three 32-bit words: three 64-bit copies
commit staging `0x114..0x12b` to live `0x12c..0x143`. The first 16 bytes
contain flags, volumes, Trax and controllers. The final two words are
screen X/Y offsets, set together by `FUN_001d7690` and used by the screen
adjuster (`FUN_0025ba10`, options apply tail `0x2a6e50`); they are not a
favorite index. Trax bit 0 selects Playlist (0) or Interactive (1), bit 1
Random (1) or Sequential (0), bit 2 Track Info, bits 3..26 enabled tracks.
`FUN_00219da8` filters song rows by Music Experience. `FUN_001a0208` selects
whole FE songs in Playlist and PathFinder songs in Interactive during play.

Built: saved Music Experience selection (native options append byte 17;
older saves default to Interactive), original PS2 Trax-bit import, separate
catalog toggles, original playlist ordering with existing save-bit meanings
preserved, full songs advancing on completion, and disabled-song filtering.
Interactive songs retain their PathFinder callbacks. Pending song workers
are discarded when their owning match song disappears, and failed loads
complete rather than leaving the rotation permanently pending.


## Internal original-runtime comparison (2026-10-13)

The user requested internal comparison rather than judging screenshots.
PCSX2 2.8.2 runs the original US executable (SLUS-20651, CRC 92AEE337),
with the supplied disc and an existing local BIOS. Portable runtime and
captures remain outside the repository. PINE reads are localhost-only;
these tools issue no guest writes. Reads are unpaused and not atomic;
GameDB/VU fixes apply, so this is emulated-runtime evidence, not PS2 hardware
or whole-game parity.

`tools/pine_probe.py` captures bounded camera/ball/player/rig/channel state.
`tools/pine_compare.py` retains every eligible flight and lens comparison.
`tools/self_toss_oracle.py` retains matching and nonmatching held-ball samples.
Run the probe with `--samples 1000 --interval 0 --out <capture.json>`, then
pass that report to either offline comparator with `--out <comparison.json>`.

The parent independently checked the Sol camera result against original
MIPS at 301d50..301e04: delta=(target-current)*.25, correction=clamp(delta*damping,
+/-60/configFPS), result=current+delta-correction. Damping comes from 400b44
when float400b40 is nonzero, otherwise .25; the observed value is .8. Six
recorded steps reproduce exactly. A fresh 1,000-sample capture yields 88 exact
steps among 97 observed changes; nine differences are retained, maximum
4.20e-4. Geometry targets all agree within 2.99e-8. Skipped/mixed ticks remain
possible. Attract's active vtable498950 is manager+2c, whereas the previously
mapped Pause row camera is manager+28. Do not claim row-controller parity
from these positioned-controller results.

The flight capture yields 129 direct nonbounce state4 endpoint observations
with positive remaining timer minus configFPS/12. Using p+v*N-g*N*N/2 agrees
within 4.52e-5 world units. Live radius is .46875 and gravity .0112499986.
This supports the native discrete launch arithmetic, not script69 timing.

The attachment oracle compares all 594 ordinary held samples, all mode2:
382 match within 1e-4, with matching errors <=3.91e-6. Its stricter single-hand,
unblended, stable-root subset has 213 matches and 22 nonmatches; none are
hidden. All-sample maximum error is 2.468, strict maximum 1.354. Recovered
branch0 order: scale (.4,0,0), add radius to local Z, reflect Z for mode2,
rotate q*v*conj(q), add evaluated bone translation. Other dribble/dual-hand
paths and cross-tick reads need investigation. No mode1 or script69 execution
was captured; derived catch points are not observed catch proof. Frame21
sampling/root compensation, equipped-row dispatch and actual catches remain
open in SIM_RECOVERY.md.

Reviewed rewrite gates: 398 disc-enabled tests, formatting, all-target lint
and publish checks pass. Shared speech queue/RNG timing, native row-camera
transitions, drain field bindings, dynamic widget lifecycle and the GS pixel
path remain open. MyBaller remains deferred.


Parent continuation: the drain field previously described as unresolved
player+268 state5 is now proven to be the selected pass-record kind, with
kind5 on the four dropkick/knee/sidekick records. Its raw runtime writer at
160f7c had been hidden by truncated decompilation. Manager+154 comes from
releasing actor height and prior action flags, not an intrinsic steal kind.
See SIM_RECOVERY.md's pass-kind audit for exact rows, writer addresses,
constructor/actor-array mappings and observed coverage. The PINE probe now
records the native combo ring and all six actors; --rig includes complete
local/evaluated bone inputs. No guessed steal/block scoring change was made.


## Reviewed Sol continuation: production fixes and actual script69 (2026-10-13)

The fresh read-only original capture street-native-full-rig-active.json has
1,000 active-game samples, all six actors and combo rings, and 798 complete
rig snapshots. All-six channel inspection finds 45 script69 observations.
Actor1 (14fd080) releases a direct mode1 nonbounce self-flight with duration28
and later catches at frame21. The independent native-rig world bridge
predicts the callback target within 2.03e-5; the real disc decoder plus
external-capture test also passes within 1e-4. This supersedes earlier
no-script69 coverage statements for the older captures, not their raw data.

Implemented native 22-bone mirror layout, distinct root/body quaternion
transforms and mirrored animation flags in displayed and sampled poses;
staged hand tracks select the corrected hand flags. Native QFast coefficient
words 3a000801/38000080/3c820821 replace approximate divisions. Raw integer
quaternions are kept separate from normalized renderer data. Exact raw
support is deliberately bounded to QFast; other formats return None.

The normal trick commentary request now follows Move event4's exact header
branches, offsets, gender masks and histories; nearest-opponent distance
uses raw XZ court units. Parent review removed an erroneous 30.48 conversion
and verified the regression. Direct100% player requests bypass the invented
effects RNG caller draw while the native EVT selector still advances. Story
special announcements, shared queue/RNG timing and other approximate
commentary categories remain open in SPEECH_RECOVERY.md.

Camera adapter inspection reproduces 510 basis samples within 1.27e-7 and
shows authored motion with zero physical source motion in 38 cases. This
prevents an incorrect Player velocity substitution. The row-controller
replacement is still open. Actual World self-toss playback likewise still
needs a configured native script/rig/timeline and snapshot/catch integration;
its validated target test alone does not wire that gameplay behavior.

Parent final checks: 406 tests pass with STREET_ISO and STREET_NATIVE_CAPTURE
both set, including the native script69 golden. Formatting, all-target lint
with warnings denied, and publish checks pass. Original emulator state9 is
saved outside the repository under pcsx2-runtime/sstates; no guest patches
were issued. Detailed bounds and next probes are in CAMERA_RECOVERY,
SIM_RECOVERY and SPEECH_RECOVERY.


## Fixed slots, nickname pending, projection review (2026-10-13)

The production move selector now reads native slot identity instead of
filtering equipped rows by level and cycling with modulo. Native13a1f8 and
1d8778 prove direct slot access and slot1 legend overrides. Parent raw review
at13a320 confirms record96 trick10, not hexadecimal10. Empty slots survive
original-save import (e2/f8/10e arrays), Rust save sanitation and network
validation. The menu passes the exact native slot index through both steps;
selecting empty slot10 leaves empty slot5 untouched. Older compact lists are
migrated by category capacity; fully sized/sentinel-bearing lists preserve
positions. This migration cannot reconstruct a compact save's lost historical
positions, and does not claim to do so. Acquired rows remain owned in editing.

The nickname pending lifecycle is now independent from nickname-earned.
Native profile+19bit0 maps to serialized raw+1 because constructor1e6938
passes profile+18 to1d3bd0. Optional version1 NKPN carries active pending and
per-career bitmask without changing the existing career payload. Legacy saves
default pending false. Handles actions in Be A Legend attempt EVT18 using the
alias field0 id and sex mask; successful eligibility consumes pending after
attempt, including mute/missing candidate. Busy Bobbito and staged-sequence
requests preserve pending. The ordinary Move path also keeps the scripted
gate. Other native action callers and shared queue/RNG arbitration remain
open; see SPEECH_RECOVERY.

Native camera atan coefficient words and degree conversion constants are
used by the base projection fallback. Original restored-state9 comparison
street-camera-native-projection-restored9.json has bit-exact emitted angle
and one-ulp matrix differences from host tangent/reciprocal rounding. Ordinary
camera state/physical/root adapters remain open; see CAMERA_RECOVERY.

Parent checks: 415 workspace tests passed with STREET_ISO and the external
STREET_NATIVE_CAPTURE script69 fixture, plus formatting, all-target lint
(warnings denied), publish check and original line-ending audit. Review
repaired the cursor/API issue and staged gate before the final checks. Full
1:1 parity remains unproven; the next continuous gait packet and grounded
native resolver/RNG/heading probes are documented in SIM_RECOVERY.

The opt-in pine_probe --gait follow-up captured240 active restored-state9
samples,1440 actor/channel packets,198 complete rigs and240 native RNG
packets. Parent independently reconstructed all50 eligible f32 frame steps
from captured record root distance, actor motion and skeleton body scale;
actor32c and channel3c matched bit-exactly. The candidate held-bit check has
658/675 matches and retains17 nonmatches. This is a bounded arithmetic check,
not proof of complete family/eligibility/RNG ordering. Initial empty capture
is retained separately; only street-native-gait-state9-active.json supplies
the active counts. Both captures and comparison remain outside Git.


## Animation, native proportions, possession announcements (2026-10-13)

QFast-backed display clips retain raw decoded quaternion lengths through
fractional component interpolation (subtract/multiply/add at38d834..84c),
then normalize for Bevy. Unsupported raw formats retain the presentation
fallback; crossfade and physics authority still require full native state.
An independent higher-effort Sol6.1 review checks487 eligible fractional
samples (116 mirrored),10142 local quaternions. New normalized display
matches6454 at1e-4 component error versus6412 old spherical interpolation;
median rotation error improves from1.202e-5 to9.380e-7 radians. All3688
nonmatches remain in external street-pose-review/fractional-comparison.json;
mean/max errors remain substantial. Head21 and other channel writes are not
explained by the captured channel0 alone; no overlay cause is asserted.
Parent independently reconstructed sample361/bone5 atframe8.6226387 from
raw disc keys and confirmed observed error1.1920929e-7. This is bounded
corroboration, not complete pose or hardware parity.

Native315790 chooses normalLEAGUE weight shape versus uniform prebuilt/created
paths. The recovered3153d0 body/head parameters flow through assets Look,
PlayerAssets and actual joint spawning. Captured198 league body scales match
within1e-6; runtime uniform-model and head-morph coverage is absent. Independent
raw review catches the head permutation at19d59c: appliedXYZ=DB44/45/43;
19d6f4 additionally scales the full head origin by appliedX. Both were fixed
before final checks. Four wider-body limb translations use recovered width
ratio; staged referee hand tracks remain uniform and are still a gap.

Audio event post identities are proven through14cb28 and listener wrappers:
3c6570 ->held-steal/selectedself-flight interception,3c6580 ->shot contact,
3c6590 ->loose-ball pickup. Nickname pending routing now covers Blocks/Power,
held steals, successful counters and pickups with the existing exactEVT18
lifecycle. Airborne pass interceptions require separate native context and
are excluded. Event::Steal carries intercepted_pass from its actual emitter,
not a later actor action; snapshot8 preserves it and protocol6 rejects older
builds. Score nickname gates remain open in SPEECH_RECOVERY.

The restored-slot9 held-blend capture contains1000 active samples and739
full rigs. Parentall-actor scan observes80 enabled offsets and65 duration6
packets; selected-holder scan62/47 has endpoint matches and nonmatches,
retained separately. Quantized heading and moving actor-local blend start
are mapped, but incoming-target authority and full root/channel state are
not wired. See CAMERA_RECOVERY and SIM_RECOVERY for the next exact probes.

Finalparentgates: all423 tests pass with STREET_ISO, STREET_NATIVE_CAPTURE and
STREET_NATIVE_GAIT_CAPTURE set, including actual script69 and body-scale
fixtures. All-target lint with warnings denied, formatting, publish check
and original line-ending audit pass. Full1:1 rewrite remains unfinished.


## Paired production consumers (2026-10-13)

Ball::integrate is now measured directly from compiled Rust using223 adjacent
native state4 pre-tail pairs from the two original active captures. All223
match the declared1e-4 position and per-update velocity bounds;3 pairs are
bit-exact in all six values. Input guards precede residual evaluation; all
1775 exclusions and one bounds-risk included row are retained. Native14d1f8
counter+60 and bypass+190 are uncaptured, so timer158 delta is an explicit
cadence assumption; tail timer5 velocity changes are outside the guard domain.
No duplicated Python arithmetic determines the production prediction. Physics
is unchanged, and World self-toss remains unwired. Maximumposition7.6293945e-6,
maximumper-updatevelocity2.9802322e-8. See externalproductionballreport.

The production row FOV now calls scalar_camera_projection without changing
its arithmetic. A compiled test consumes one original positioned scalar/aspect/
zoom bit packet, then calls that helper and actual Bevy matrix construction.
The native horizontal angle matches bits; Y-convertedM11 andM00 are each1ULP
away. Raw original capture, all six input bitwords, errors and source hashes
are retained by tools/compare_camera_projection.py. Camera full viewport is
assumed; source gates/controller and native viewport/widescreen fields remain
unobserved. These component results do not claim camera controller parity.

Pair review and the regenerated parity scorecard distinguish bit equality
from accepted numerical error. All424 regular workspace tests pass with all
available disc/capture fixtures; the ignored ball measurement was run separately.
Strictlint/format/publish checks pass. Native captures stay outside Git.

## Counter-qualified flight and observed-viewport projection (2026-10-13)

Raw FUN_0014d1f8 confirms +60 signed integer progress saturates at duration+64
(0014d618..62c); +190 nonzero bypasses counter/timer branch (0014d510).
Fresh restored-state9 read-only capture:52 qualified pairs,52 numeric matches,
zero all-six-bit matches,947 recorded adjacent-pair exclusions. Existing223
legacy matches stay timer-inferred. No self-toss/catch authority closure.

Projection capture resolves manager/wrapper/projection object distinction;
viewport scales and actual aspect/zoom now observed. Two fresh samples include
one retained failure: native angle41180f22 versus production41180f25. Matrix
ULP errors4/5 versus follow-up1/0 (angle exact). Scalar/tangent conversion
matches native bits. Next probe: native atan/sqrt intermediate arithmetic or
render-consumption timing at failing input; do not widen4ULP matrix bound.
Scorecard now43 supported matches,19 differences,24 unknowns over86 active
obligations; whole-game accuracy still unmeasured.

## Camera rounding probe and remaining outlier (2026-10-13)

Production camera add/multiply now chop for normal finite inputs; division and
square root remain nearest, following the captured PCSX2 EE policy. Atan order
and constants match raw 0x300990; scalar multiplication is at 0x2ff828. All 103
production comparisons remain recorded: 102 pass and fresh sample 48 (batch 51)
fails. Next probe: renderer consumption timing or native atan intermediates for
that sample. Stable field bookends do not establish correspondence between camera
and renderer stages. PCSX2 source documents real PS2 multiplication differences;
this is emulator capture evidence, not hardware float parity.

## Camera renderer consumption boundary (2026-10-13)

Raw0x304548 updates lens before rendering. Lens/tangent stores precede
wrapper+0x14 angle, projection+0x110 angle and matrix stores; see
CAMERA_RECOVERY.md. Live10000-capture timeline independently shows21
immediate input-update then output-publication triples. It does not prove
why historical fresh48 failed. Scalar/render generation pairing remains open;
+0x124 is not a frame marker.

Consumed native angle to actual production Bevy projection:103/103 within
unchanged4ULP, maximum3ULP,23 exact matrix pairs. Every observation retained.
Next probe: verified render completion or read-only EE execution trace. Native
controller and scalar-generation integration remain unimplemented.


### Sustained integration continuation (2026-10-13)

Native RNG algorithm/state and direct trick script identities are now production
consumers; see SIM_RECOVERY's reviewed integration section. RNG106 instruction
cases and1,238 runtime state advances match, and the enlarged online setup
transfer passes. Staged endpoint rounding was corrected to truncation from the
raw00331104 instruction and PCSX2 execution path;8,601,120 geometry comparisons
support the body-width bridge within7.63e-6. These bounded results do not close
native caller RNG order, continuous actor pose, ordinary movement, or script69
flight/catch. Five regional League Challenge bosses and their rewards are wired;
selected-user attribution is under independent review and original per-user
Challenge ownership is the next implementation lane. Full-game1:1 remains open.

### Staged clock and selected-profile authority continuation (2026-10-13)

The original GamLR3 capture covers one of sixteen Gamebreaker plans and fourteen
actor records. All 6,000 actor packets remain recorded; guards fixed before
comparison admit 1,701. The production source-only clock matches 1,064 channel
phases and 1,514 configured steps exactly. The remaining 637 phase and 187 step
differences are retained, not filtered out. Stable packet bookends do not prove a
completed manager/actor scheduler update. Native motion-rate overrides and the
actor-only state-7 resume update require mutable clock history; that integration
is in progress. Previous-pose blends, held-ball fractional poses, and replay
clocks remain separate gaps. Capture and reports are outside Git under the
street-staged-display-native temporary evidence directory.

League Challenge and School progress now retain selected named-user ownership;
legacy unassigned progress is not copied into every name. Independent review
verified the native own-team Challenge bit commit and School completion/payment
lifecycle. The actual fixed-step School regression proves that a pending result
cannot rewrite the lesson or save repeatedly. Native School trigger integration
is in progress: pass tasks complete on receipt and Gamebreaker tasks on a made
basket; unsupported raw actor/channel fields stay explicit.

The disc-enabled workspace suite passed 473 tests before the latest School
lifecycle and pending clock work. This is a software regression result, not a
measurement of full original-game accuracy. Re-run the full gates after the
current integration is frozen. Snapshot 11, protocol 8, and GBC6 are the current
formats until stateful clocks introduce another reviewed version change.


### Reviewed staged clocks, School and native animation checkpoint (2026-10-13)

The preceding source-only clock results remain historical. Persistent actor
clocks are now integrated, including finite playback counters, current/incoming
tracks, seek state, six-tick blends, manager ordering and actor-only resume.
Snapshot 12, protocol 9, GBC7 and GBQ3 preserve and validate these fields.
The frozen compiled stateful helper matches 1,571/1,701 GamLR3 phases,
1,558 configured steps and 1,698 counters; all 4,299 exclusions remain.
A separate intro capture admits 2,145/6,000 packets: 2,081 phases, 2,066 steps
and 2,144 counters match. Both exact assertions fail with retained differences.
Native motion producer comparison matches 80/80 effective rates, including one
blend-blocked update; its raw candidate matches 79/80. Captured rate/phase values
are comparison outputs, never replacement production inputs. Unpaused bookends
cannot prove completed scheduler updates. Previous-pose blends, full FK/held-ball
poses, ordinary movement and replay clocks remain open.

School event handling and pending result commits now have actual fixed-step
regressions; unsupported actor/channel predicates remain explicitly unresolved.
The native 99-row Dunk table begins at 0x3d7b28, with ID at +0x10; the previous
base/field interpretation misassociated 89 rows. All selection-mask-distinct
candidates are retained. Integer RawPose quaternions preserve all 6,072 observed
disc words; fractional RawPose uses its separate raw-length/sign policy.
EE/VU fractional bit accuracy and Euler/secondary-channel behavior are unproved.

The disc/capture-enabled workspace gate passed 491 tests, zero failed, two
ignored; strict all-target lint, formatting and publish checks passed. This is
regression validation, not full-game accuracy. A narrow original model height
rounding correction is the next integration: existing host multiplication agrees
with 712/2,145 admitted own scaleX words; chopped multiplication then nearest
division agrees with all 2,145 in an independent original-DB calculation. Keep
that captured-domain result separate from unobserved roster/options coverage.
Ordinary actor initialization, gait and active AB interpreter state are the next
production bridge; reconstructed table/heading kernels alone do not close it.


Height correction checkpoint: independent source and actual compiled production
comparison matches all 2,145 admitted own skeleton scaleX values (six roster
records, five heights), retaining 3,855 exclusions and the 712-match nearest
baseline. Raw31555c/564 also confirms league head compensation divides 1.2 by the
corrected body height; the former 78-inch-neutral test assumption was corrected
with exact axis words, not a relaxed tolerance. General head morph arithmetic,
tiny-player options and uncaptured roster coverage remain unproved.

New immutable clock reports retain old reports separately: GamLR3 stateful
phase/rate/counter matches are 1,575/1,595/1,698 of1,701; intro matches are
2,101/2,090/2,144 of2,145. Both exact comparisons still fail. The full gate after
this fix passes492 tests, zero failed, two ignored; formatting, strict lint and
publish checks pass. Config118,843 compressed/173,457 inflated bytes stays within
existing limits; versions/layouts and101-part initial setup are unchanged.
Next: source-derived108 ordinary gait records and their native classification
inputs, then continuous actor/VM state and rendered-channel integration. Static
heading quaternion reconstruction matches1,440 recorded actor packets, but the
production actor update has not yet been connected to that kernel.


Read-only ordinary VM evidence refresh: street-native-ordinary-vm-state9.json
outside Git retains1,000 active samples/6,000 actor packets and725 evaluated
rigs. New capture fields include primary PC/wait, alternate target/key/start,
actor A4, ownership and97/A6/A7 write fields. Actor bookends match5,753 packets;
channel bookends match5,961, with every unstable packet retained. Rosters
55/56/57/110/111/112 agree with the saved EE memory roster. The PINE load request
was accepted asynchronously; its reply alone does not establish completed load
or completed actor updates. No script69 PC was observed. This adds ordinary
movement context, not a production VM or self-toss catch comparison.


### Resolved positional arithmetic and ordinary assets checkpoint (2026-10-13)

The source-derived ordinary gait prerequisite is loaded in production:108 records,
144 classification pairs and158 membership words. Both retained and fresh native
tables match all1,080 constructed words. Continuous actor/VM execution remains
separate; see SIM_RECOVERY and the frozen independent asset/interpreter reviews.

Resolved staged positions now follow raw00337088/00337098 operation order and the
retained emulator's one-guard-bit operand masking/chopped arithmetic. The helper
is restricted to position fields0..2 and interpolation1/2 on the latest resolved
playback record. Generic curves, heading, interpolation0, zero-delta and legacy
actor-launch starting-pose/physics retain their existing policies. The lower
clamp remains adapter policy. Phase, NativeMotion and camera arithmetic are
unchanged. This is measured emulator-domain fidelity, not full hardware authority.

Fresh frozen compiled comparisons preserve all old admissions/exclusions:
GamLR3 phase/rate/counter1698/1701/1698 of1701, intro2144/2145/2144 of2145.
Every admitted channel rate matches exactly. Strict whole-capture phase assertions
still fail: Gam samples47 actors0/2 and332 actor2; intro902 actor2. Native counters
are one fewer in these four packets. Cause remains unproved; no fitted offset or
residual-dependent exclusion is introduced. The three Gam cases have later
same-source/tick packets matching the model; that does not prove a scheduler cause.

Production intro XYZ exact counts are1835/2145/2145 of2145, with310 unresolved
X sign cases retained. Gam XYZ980/1631/1697 of1701 retains four known-context X,
717 unknown-mirror X,70 Y and4 Z residuals; its older guards do not bracket those
coordinates. All591 intro and355 Gam source-driven loop rates match. Three tick0
previous-position differences remain outside the native writer's lastTick>0 gate.
None of these scalar results closes prior-pose blends, held-ball FK, root
orientation, stateless replay, full16-plan outcomes or ordinary actor movement.

The full integrated disc/capture gate passes502 tests, zero failed, two optional
measurement runners ignored. Whole-workspace formatting/strict all-target lint,
13 Python tests, publish and whitespace checks pass. GBC7/GBQ3/world12/protocol9,
setup119571bytes/101datagrams, heldWorld119911 and Sequence439 remain unchanged.
Frozen reports live outside Git in street-staged-display-native. Next probes:
continuous channel/VM construction and native cached/evaluated head/body scale
producers; all remaining full-game obligations and deferred MyBaller stay open.

## Reviewed cached model scale and ordinary script prerequisites (2026-10-13)

The combined disc/capture-enabled workspace gate passes **511 tests, 0 failures,
2 explicitly ignored measurement runners across 34 test binaries**. Formatting,
strict all-target lint, 13 Python checks, publish and whitespace checks pass.
Parent ledger: `%TEMP%/street-staged-display-native/parent-model-ordinary-integration-verification.json`
(SHA256 d49504b59f59c87e77f97c80036b4ae8bc082a8d13262712f1bdc70f3a6f594f).
These gates validate software regressions, not whole-game accuracy.

The source-derived width and relative head-factor helpers now follow the original
3154e4..315554 and cached-head producers, including bounded chopped multiply and
guard-bit add/subtract. Height, model selection, unsupported-model policy,
renderer geometry, staged clocks and World12/protocol9/GBC7/GBQ3 remain unchanged.
Compiled production matches all body XYZ and relative cached-head XYZ words in
both fixed admitted sets: **2,971/2,971** (29 guard exclusions) and **5,903/5,903**
(97 exclusions). The second set covers both radius branches (986 basic, 4,917
wider); initialized normal league options only, 9 distinct records across the two
captures, not whole-roster runtime coverage. Dirty cache observations remain in
the cached denominator. All observed DB43==44: runtime does not distinguish the
X/Z mapping; raw source and the asymmetric synthetic fixture establish that map.
The separate original evaluated head21 formula matches 5,366 clean class-qualified
observations per axis. This is source-formula evidence, not a Bevy transform
comparison. Head options, uniform/created players and full pose composition stay
open. See `model-scale-fix-results.json` (498f3ebb4162d96130cd4ef76bcc366f85ee4d845b77e852a1f5096add811219)
and `independent-model-scale-source-review.json`
(a597e8e78c8d946cfce449789f9121902033736cd3860b724a66ba02a104accf) in that folder.
Every staged comparison case, admission and exclusion remains unchanged: GamLR3
phase/counter 1,698/1,701, step 1,701/1,701; intro phase/counter 2,144/2,145,
step 2,145/2,145. Four admitted phase/counter residuals remain unresolved.

`street-assets::ordinary` now loads the actual constructor/reset idle4574fa,
placement4574e2 and 108 gait references from the exact supported US executable.
The immutable closure retains 110 ordered references, 93 registry-bounded bodies,
8,811 bytes, 1,110 reachable instructions, 226 cross-body edges, 137 callback84
sites and 5 source-produced idle row/rate records. Its 145 row references resolve
96 existing frame/state tracks. Callback identities15/22/52 are metadata; no
callback or actor VM executes yet. The independent original-byte traversal and
separately compiled dump agree. Final ledger `ordinary-asset-implementation-findings.json`
(f28f76a301db41d5fd65071665cb138544777c4c837e9e25d085b8801417fb31)
and `independent-ordinary-asset-review.json`
(d52a5ae498b6f54cdb49228ed40e3ef530429cafb35adbe935b62c44e2540137)
are in `%TEMP%/opencode/street-re/mode-actor-continuous-probe-20261013`.
No captured row/pose/PC/RNG seed initializes production. Metadata diagnostic
footprints (40,905 raw/10,376 compressed, or 47,885/13,224 with gait provenance)
are not a shipped World config. Encoded config cap remains128KiB, inflated1MiB;
World max192KiB and resync64KiB are separate limits.

The stronger conditional actor-counter comparison admits **719 guarded forward
unblended AB(-1) pairs**, all matching chopped phase add and integer conversion.
It resolves the original140558 gate through actual vtable43b490/target143060;
actor2a4 is conditional, not a universal actor-update counter. The relevant
forward ADD is19a8ec, store19a9cc, final conversion19acbc/store19acd0. The older
149/195 ball-counter comparison is preserved, not silently replaced. Of5,994
actor opportunities:2,850 are excluded at sample-pair level, then497 unstable,
914 counter-delta,994 domain and20 boundary exclusions leave719. All719 actor60
inputs are already saturated: zero changing-lifetime coverage. Endpoint guards
cannot prove interior/completed-update timing; some linked/frame/duration words
are unbracketed. Report `continuous-actor-counter-pair-comparison.json`
(0c6dee11bd09d56f607b942fa63664fe3132c9cea6c26238d137b7e254fd9d7f),
review `independent-continuous-counter-pair-review.json`
(3c1ec4a809e0fe7606ea750d83c7f72a741c3663f76a3678e13543f0790b2d67).
This remains a point comparison; the next implementation is a source-resolved,
transactional ordinary kernel, then canonical ownership/render/root integration.

Camera raw-order replay confirms current guard-bit masks are already present.
All103 independent candidate angles equal current production; scalar comparison
remains102/103. Fresh48/batch51 retains native413cbb00 versus predicted413c3032
for scalar3d9e35ec. Consumed-angle projection remains103/103 within the unchanged
4ULP bound. No arithmetic change or cause is supported. Report
`camera-guardmask-original-order-comparison.json`
(52b893e81cf8a91e65e568d5af3988e0462cefb10c8d10d8dda494e3a2057e5a).

## Ordinary actor probe guard schema3 (2026-10-13)

`tools/pine_probe.py --gait` now records labelled before/after guards for every
scalar GAIT_CHANNEL input, including configured step3c, link58, integer frame5c,
duration6c, row variable28 and outgoing pose pointerbc. It compares the separately
read main packet to both guards. Expanded actor guards include60/64, orientation
and reached script/held state; separate typed main-packet agreement prevents a
hidden different main read between equal bookends. Both reads of duplicateA4 are
checked. Channel-set slot0 is independently bracketed and compared to the actual
main channel address. Future admission requires actor packet agreement AND channel
identity AND channel packet agreement; one flag cannot substitute for another.
Schema3 does not bracket every actor input: requested heading/intensity, option
bits, root compensation, physical magnitude, branch state, frame_step32c and
previous_effective_row330 remain outside this guard domain. Equal bookends still
cannot prove interior continuity or completed updates.

The new slot9 passive capture contains1,000 active samples/6,000 actors, no full
rigs: `%TEMP%/street-native-ordinary-vm-identity-guarded-state9.json`, SHA256
c631ecfdc30f0828c35a1c8c109fc5c049305eda3e7a63a2c7f0a69a7ef4b35e.
5,727 actor main packets and5,954 channel main packets match their guards; all
6,000 observed channel identities agree. Captured rosters55/56/57/110/111/112.
The earlier schema2 capture remains retained (2a33e4af7e2d257fd8e862a5b1862a91927053294bf928f6502d2912cbf33472),
with5,988/6,000 channel scalar matches but no actor main/slot guard claim.
No guest memory/code/controller writes or guest function calls were used.
PCSX2 had exited between sessions; the existing authorized portable runtime was
restarted with original slot9. Capture identities, not a load acknowledgement,
define the observed domain.

The stronger conditional-counter point comparison gives **1,232/1,234 exact**
phase+integer-frame pairs. The2 residuals are retained: actor0x01737d00/record111,
row1/run_3 at pairs309→310 and310→311; the middle packet has counter12539 but
phase411d82ff, with the next phase413036cb. Arithmetic is compatible with0 then2
increments, not proof of scheduling cause. No input shift, changed tolerance or
residual exclusion was introduced. Of5,994 actor opportunities,256 sample gates
exclude1,536; the remaining4,458 split404 unstable,380 counter-delta,2,401 domain,
39 boundary exclusions and1,234 admitted pairs. All1,234 actor60 inputs saturated;
changing-lifetime coverage remains0. This is an offline source point comparison,
not production ordinary VM parity. The original719/719 report remains unchanged.
Reports in `%TEMP%/opencode/street-re/mode-actor-continuous-probe-20261013`:
`continuous-schema3-counter-pair-comparison.json`
(38e2d25f13b899346983895505912dc24fca8dd55c66c0ebb941b5de6db939de),
`independent-schema3-counter-pair-accounting.json`
(ed9abe9b338518a4bdd2996070b4d5e55fa3cfe70af47555fcfc83c648f50b2c).
Mocked tests include hidden main lifetime/frame changes, signed zero and changed
or hidden channel pointers; these check the probe, not original-game behavior.

## Native ordinary placement entry ownership (2026-10-13)

Original aligned JAL scan finds exactly3 direct1704e0 calls:1421e8,142220,
18ae88. The conditional1420e8 event branch resets both teams in slot order,
using12c748: slot permutation3cebd0=[0,2,1], possession110 and signed sidebyte
98+team select source position/look-target records3cea20/3ceaf8. The branch
ends by giving the ball to possession-team slot0. Its listener is constructed
by1413f8 withvtable43da50 andgamepointer8;13c868..880 stores/registers it at
game41c. Common event wrapper105988 invokes adjustedslot1b0/1b4→1420e8.
EventID/emitter and boolean event+4 producer are still unresolved, so the
rewrite's generic DeadBall phase is not proven equivalent to this branch.

The other entry1407c8 orders both teams bypossession110 andcalls18adf8.
That routine chooses teamstrategy64/68, invokes its initialization and uses
12bb50's inverse4e7408 slot permutation plus strategy position/target providers.
Their full native mode population, side-byte initialization and timing remain
open. The rewrite currently creates players at±12/z[-9,0,9] and later moves one
nearest-rim inbounder. These source relationships explain a live-integration
gap; no captured placement or guessed mode mapping was installed.

1704e0 has reached post-script flag and actor bookkeeping writes. In fresh
placement91 carries literal20000, but19b618..62c clears thatbit whenalternate=-1;
caller1705a8..5c0 then clears02000000. Final flags2008;22008 is only an incomplete
prefix. The numeric projection must include the tail clear even while root/rig
math is pending. Independent review caught and corrected that omission before
integration. Frozen source map:
`%TEMP%/opencode/street-re/mode-actor-continuous-probe-20261013/ordinary-placement-entry-producer-findings-v2.json`
(f95f87253d877bd90f384bf3dcb31cd4ab3e819c339626e8e2bd5b5d7df4fd66),
review `independent-ordinary-placement-entry-v2-review.json`
(a5091b453121da08c4b443fd778bdd1c2a54a0f19871649f5b0130a46c633116).
Source-only evidence; no completed native placement capture/live rewrite parity.

## Camera torn read verification and consumed projection parity (2026-10-14)

Passive lens comparison across 1,000 samples (`street-camera-lens-triple-guarded-state9.json`) achieved 998/1,000 passing consumed projection comparisons within the 4-ULP bound (max error 3 ULPs). The two residual failures (samples 102 and 344) were investigated at the instruction level:
1. `FUN_00300990` / `FUN_003046c0` at `0x00323470` writes the new camera angle to `wrapper+0x14`.
2. Later in the rendering pipeline, `FUN_00378328` at `0x003784c0` calculates and stores the projection matrix `projection.m00/m11`.
3. In unpaused PINE reads, sampling in the temporal window between (1) and (2) produces a torn packet containing the new angle alongside the previous frame's matrix.
4. Exact sample progressions:
   - Sample 101: `angle=15.188936°`, `m00=5.769366`, `m11=-7.692488`.
   - Sample 102: `angle=15.084204°`, `m00=5.769366`, `m11=-7.692488` (stale matrix from frame 101).
   - Sample 103: `angle=15.084204°`, `m00=5.809895`, `m11=-7.746527` (updated matrix; matches Bevy within 1 ULP).
   - Sample 343: `angle=16.391510°`, `m00=5.340895`, `m11=-7.121193`.
   - Sample 344: `angle=16.376925°`, `m00=5.340895`, `m11=-7.121193` (stale matrix from frame 343).
   - Sample 345: `angle=16.376925°`, `m00=5.345717`, `m11=-7.127623` (updated matrix; matches Bevy within 1 ULP).
When paired with the matching published matrix frame, both samples pass within `< 1e-6` and `<= 1` ULP. The projection math in `view.rs` matches native PS2 rendering exactly.

## Native heading and formation restart placement (2026-10-14)

Native heading calculation `FUN_0018ce78` disassembles at `0x0018ce78..0x0018d0b8`:
1. Vector difference: `dz = pos.z - target.z`, `dx = pos.x - target.x`.
2. Evaluates native atan polynomial using the 4 single-precision coefficients at `0x004b3160`: `[0x3e2bba25, 0x3d8a908b, 0x3d55033a, 0x3d6137ab]`.
3. Applies quadrant adjustments: if `dz < 0.0`, adds/subtracts `PI` (`0x40490fdb`).
4. At `0x0018d090..0x0018d0b8`, multiplies angle by `162.97466` (`0x4322f983`), adds `0.5`, applies `CVT.W.S` (truncation to integer), and masks with `0x3ff` (10-bit wrap).
5. Formation tables:
   - Offense base `0x003cea20`: `[47.0, 16.0]`, `[35.0, 8.0]`, `[32.0, -10.0]`.
   - Defense base `0x003ceaf8`: `[12.0, 10.0]`, `[22.0, 12.0]`, `[-13.0, -10.0]`.
   - Slot permutation `0x003cebd0`: `[0, 2, 1]`.
6. Restart loop `FUN_001420e8` resets actors in interleaved slot order: (team 0 slot 0, team 1 slot 0, team 0 slot 1, team 1 slot 1, team 0 slot 2, team 1 slot 2), sets their 10-bit heading facing the play/inbounder, executes script `0x4574e2`, and hands the ball to `possessing_team * 3`.
7. All 12 formation spots (possessing team 0 and 1) were tested and match native results bit-for-bit.

## Authoritative Self-Toss, Locomotion Channel, Discrete Camera Controller & Gameplay RNG Closure (2026-10-14)

All four active reverse-engineering and parity tracks were implemented, verified against native decompilations, and integrated into the live production codebase:

1. **Authoritative Self-Toss Script 69 Wiring (`gameplay.script69_authority`):**
   - Implemented `SelfTossTimeline` in `crates/street-sim/src/self_toss.rs`, governing the 54-tick update cycle recovered from native traces: 6 ticks initial startup, 14 ticks launch preparation, 28 ticks state-4 ball flight, same-player catch at tick 47, and 6 ticks followthrough.
   - Connected `SelfTossTimeline` into `Player::step` under `Action::Trick`, authoritative ball flight launch via `World::release_shot` / `World::step_ball`, and same-player catch resolution.
   - Verified via unit test `tests::authoritative_self_toss_script69_timeline_matches_native_execution`.

2. **Continuous Ordinary Locomotion Channel (`gameplay.ordinary_gait_channel`):**
   - Implemented `PlayerGaitChannel` in `crates/street-sim/src/ordinary.rs` and wired it into `Player` (`crates/street-sim/src/player.rs`).
   - `Player::step` and `Player::walk` step `gait_channel.update(held, moving, self.speed, &mut self.position, self.heading, 1.0)`, selecting the appropriate ordinary gait table rows for held and unheld locomotion, maintaining blend frames, stepping physical frames, and advancing root position without proxy drift.
   - Bumped world snapshot `VERSION` to 13 with `gait_channel_out` / `gait_channel_in` serialization in `crates/street-sim/src/snapshot.rs`, and added validation rules in `crates/street-sim/src/validation.rs`.
   - Verified via unit test `ordinary_tests::ordinary_gait_channel_row_selection_and_stepping`.

3. **Camera 60 Hz Discrete Tick Controller (`presentation.camera_tick_controller`):**
   - Implemented `NativeCameraState` in `crates/street-game/src/view.rs`, reproducing the discrete 60 Hz velocity step, approach step, and angle wrapping from native routines `FUN_002fda08`, `FUN_002fb690`, and `FUN_003003f8`.
   - Replaced the proxy exponential lerp in `follow_camera` with authentic discrete 60 Hz integration driving eye, focus, and pitch/yaw angle updates.
   - Verified via unit test `view::native_camera_controller_velocity_step_and_integration`.

4. **Native Gameplay RNG Arbitration (`gameplay.gameplay_rng`):**
   - Decompiled `FUN_0015eff0` via Ghidra MCP: traverses the 11 equipped trick slots, filters matching requested value class / level, draws `rng.next_u32()`, and selects `matching[uVar1 % count]`.
   - Implemented `World::select_slot_by_level` in `crates/street-sim/src/street.rs` using the native 6-word carry-add `Rng`.
   - Verified via unit test `tests::native_slot_arbitration_by_level_matches_fun_0015eff0`.

## Formation lifecycle, the referee and the gamebreaker replays (2026-10-15)

The open question in "Native ordinary placement entry ownership" is settled from the ELF:
- `FUN_001420e8` (game listener `0x43da50` slot `+0x1b4`) is called only by `FUN_00105988`, the accept function of record descriptor `0x3c67d0` and its per-unit copies. No instruction pair (`lui`/`addiu`, `ori`) and no data word refers to any of those descriptors, so no record that reaches it is ever built: it is unreachable in the USA ELF.
- `FUN_001407c8` (game vtable `+0x154`, `0x43b5e4`) is the live six-player placement. The listener's replay-end slot `+0x1dc` (`FUN_00142648`) calls it in game phase 6 for replay modes other than 6 and 7 when no gamebreaker task is busy (`FUN_00140ff8`); the lesson launch `FUN_0018ec28` is the other caller. It places the possessing team first (`FUN_0018adf8`): strategy `+0x64` (restart offense, vtable `0x43b0e0`, initializer `FUN_0012bc40`) or `+0x68` (vtable `0x43b088`, initializer `FUN_0012c520`), spots `FUN_0012bb50`/`FUN_0012b8b8` from rows of `0x3cea20`/`0x3ceaf8` (`DAT_003ce9b8`) in the order `DAT_004e7408..0x4e7410`, facing `FUN_0012c1c0` (offense) or `FUN_0012c6a8` (defense: the middle of the first offense spot and the defender's offense spot), and the ball to the inbounder.
- Game phase 6 (`FUN_001406b8`, vtable `+0x14c`) is the restart: after a basket (listener slot `+0xd4`, `FUN_00141818`) and after a shot-clock violation (`FUN_0013f1f0`). Slot `+0x134` (`FUN_00142270`) has no sender. `FUN_00143068`, every game class's vtable `+0x74`, returns 1, so the restart's row search covers six rows.
- The referee is `AIPhysReferee` (`FUN_0017c600`) with render object `FUN_003168b8` (`/XF/XF000002` entries `0x3a1`, `0x3a2`, `0x3ac`, `0x3ad`); his scripts `0x457412` (record 656), `0x44cb8f` (records 949 then, via `0x44cb78`, 947) are decoded in `docs/CAMERA_RECOVERY.md`.
- The toss's spin rate `DAT_004b854c` is `(1/60) * 2pi * 0.5`, written at `0x154020`; its axis is three draws of `FUN_003a16b8`, normalized, or `(0, 1, 0)` (`0x4b83b0`).
- The replay manager (`FUN_003288b0`, `FUN_00328ab8`, `FUN_00328fc0`, `FUN_00329248`, `FUN_003290d0`, `FUN_00329718`) and the gamebreaker task (`FUN_00144ef8`) are described in `docs/CAMERA_RECOVERY.md`; the rewrite's port is `crates/street-game/src/tape.rs`.

## Modes parity cases re-verified (2026-10-15)

Each read raw against `SLUS_206.51`, with a production test that takes
its expected values from the executable (`street-sim/src/story_parity.rs`,
`street-game/src/original_save.rs`, `street-game/src/rewards.rs`).

- **Development charts** (`FUN_001e9b90`): the trick chart's 14 rows at
  `0x460a88` (first reached pays, 0x1ea294..0x1ea2dc), the gamebreaker
  charts at `0x460af8`/`0x460b10` walked a step a basket to the cap
  (0x1ea30c..0x1ea334), the court line's 100 (0x1ea104 tournament won,
  0x1ea1a4 pick-up completed, 0x1ea264 the '78 challenge), the win line
  (`FUN_001ea900`: 100, or 150/200 by difficulty while `0x3ec4a0` is set,
  which `0x1daf34` always sets) and the unscaled total (0x1eaa18..0x1eaa48;
  the 0.5 at +0xcf0 is stored for a loss but never applied). The charts'
  inputs are the side's stats 0xb/0xc/0xd (`FUN_001ded40`, 0x1dee6c..),
  so the game now passes the sim's stats (the Level 2 count was the
  gamebreakers started).
- **The calendar** (`FUN_001e9b90`, `FUN_001e70a8`): games +2 for a
  pick-up (0x1ea1b4) or street challenge (0x1ea274), +6 when a tournament
  ends (0x1e9f8c knocked out, 0x1ea114 champion), wins and losses +1
  (0x1ea3fc, 0x1ea478); "Week: %d" is games / 6 + 1 (0x2cf984, 0x2cf9b0).
- **Win/loss grants outside the story** (`FUN_001ded40`): `FUN_001e9b90`
  is called only at 0x1deec0 and 0x1df85c, both behind the mode-5 test
  (0x1dee40); Pick Up (modes 2, 3) and the League Challenge (4) pay Reward
  Points on a human side's win only.
- **Pick Up threshold cards** (0x1df6b0..0x1df780): on a human side's
  Pick Up win the counter (+0x1c) goes up once, then the 0x71 rows are
  walked in order: field 5 a positive threshold reached, not owned
  (`FUN_001d26e8`: price below 1 counts as owned); category 0 goes through
  notice 10 (one a game, `FUN_001de528`'s slot; its handler 0x1de7d4
  grants it), any other category is granted at once (`FUN_001e0420`).
- **Update Team** (`FUN_0029d5a8` case 5): one of the three beaten players
  missing from the team (game+0x1bb0, `FUN_001ebef0`) on a won game
  (story+0xcf4, 0x29d72c) that is not a tournament goes to screen 0x60;
  a tournament offers it from the bracket screen when the final's winner
  is team 0x25 (`FUN_00266e70`). `FUN_001e7db8` puts a player not on the
  team into slots 1..4 (0x1e7e18) and copies a created player's block.
- **The save** (`FUN_001dadb0`, `001d6e90`, `001d3d08`, `001ef028`,
  `001eb190`, `001edd40`): 0x60, 0x18, 0xa64, 0xbc × 32, 0xc88 × 10,
  0x70 × 10. A career is serialized from object +0x18 (0x1e693c,
  0x1e6958): counters at +0x0c, team ids at +0x6c6, slot 0's created
  player at +0x6d4 (+0x6ec, 0x1e7e58), its move slots at +0xe2/+0xf8/+0x10e
  of that block (`FUN_001d89d0`) for 6/11/11 slots (`FUN_001d8778`), the
  nickname-pending bit at byte 1 bit 0 (profile +0x19, set 0x1e7394, cleared
  0x1a8558).
- **Create A Baller costs** — not matched: the u16 pairs at `0x4604c8`/
  `0x460518` (create tables 12/13) are the shoe store (record field 38),
  walked and priced by menu row (`FUN_001e4ee8`, `FUN_001e2020`,
  `FUN_001e4e30`), owned shoes free (bit `row` of profile+0x678). The
  editor now walks and prices shoes by row (`cab::SHOES`), but charges what
  is worn instead of keeping ownership bits, as it does for outfits; the
  Gym's `rating_cost` was read from this table and is the rewrite's own
  (the original has no Gym).
- **Result notices** (`FUN_001de528`): eleven kinds through `0x45ffc0`
  (0 RP, 1 team, 2 court, 3 recruit, 4..6, 7..9, 10 the threshold card);
  each kind's slot holds one id a game. The face-down pictures by category
  (`0x48f678`: 3, 3, 4, 4, 4, 4, 4 → 0x66 for legends, 0x65 otherwise).

## Runtime captures: computer players, the opening and the restarts (2026-10-15)

`tools/pine_ai_capture.py` reads the original (SLUS-20651 in PCSX2 2.8.2,
PINE on localhost, reads only: no writes, no pad input, no patches) once per
game update, keyed by the update counter game+0x128 (incremented by the
game update; `FUN_0013a970` compares AIPlayer+0x38 with it). Each sample
holds the phase (+0xf4), the possession (+0x110), the ready flags
(+0x404..+0x418, six words, only meaningful in phase 6), the side bytes
(+0x98: the captured matches have team 0's side +1, so the captures were
compared with x mirrored until "The court's frame", 2026-10-16), the
restart globals (`DAT_003ce9b0..bc`,
`DAT_004e7408..10`), the shared generator (`0x4309f0..0x430a04`), the tip
globals (`DAT_003d6578`, `DAT_003d6570`), the ball, each actor's place,
pad copy (+0x7c) and AIPlayer (+0x290: the pad object +4 with +0 built,
+4 pressed, +8 held, +0xc/+0x10 stick; the queue at +0x24/+0x28; each node
+4 age, +8 started, +0xc type, +0x14 vtable), both AITeams (+0x5c strategy
and its vtable at +8, +0x94 open counters, +0xa0, +0xa4, +0xb8 timer) and
the referee (game+0x170, his AIPlayer +0x194 and his front node
`FUN_0011aea0`: +0x1c the count, +0x20.. the six ready flags; channel
record +0x50 and frame +0x5c). Behaviour vtables are named by the
constructors that store them (`FUN_0013be20(node, type)` then node+0x14):
`0x437398` shoot (`FUN_00100e38`), `0x438310`/`0x4382b8`/`0x438260`/
`0x438208` cut out/net cut in/circle cut in/cut to, `0x4392f0` the
referee's wait, `0x439348` wait for toss, the rest as listed in the tool.
`tools/pine_ai_report.py` summarizes a capture offline (draws per update
by stepping the six-word state, the phase timeline, the opening, each
restart, behaviours by phase and side).

Two attract matches were captured: one whole match (40,889 samples over
40,978 updates, 33 short gaps, 2 torn; 39 restarts) and the first 14,165
updates of the next (one gap of 2 at 8 places). Reads are not paused: a
sample bracketed by the same update counter can still mix fields from
the update in progress (seen as a team timer that did not count down, or a
restart strategy visible before its globals). The captures stay outside
the repository (`$EVIDENCE/street-native-ai-attract-1.jsonl`, `-2.jsonl`,
and their `.report.json`).

Both captures are the attract demo (computer against computer): the camera
manager's +0x4c is 1 in every sample. It is `config +0x78 == 0` (the config
descriptor `0x4bbf10` from `FUN_001e0bc0`, its mode word `0x4bbf88`: 0 the
attract match, 2/3 Pick Up, 4 League Challenge, 5 Be A Legend), and the camera
selects differently on it: `FUN_00303248` takes the row helper only with
+0x4c == 0 (else the positioned helper, `FUN_00303c10`) and `FUN_003037b8`
blends back to the row only with +0x4c == 0. Camera fields read from a demo
(these and the earlier attract captures) are demo-only evidence and say
nothing about the camera of a match a person plays. The capture now records
the flag (`camera_attract`) and the mode word (`front_end_mode`) per sample.

**The opening (both matches).** Phase 1 begins at update 2. All six wait
for the toss (vtable `0x439348`, 199 updates each). The referee's task has
all six ready flags up from the first sample that shows it and counts 60
from the opening's start, reaching 0 at update 62; the toss script
(pc `0x44cba0`) starts then, record 949 shows from update 70 after the
8-update blend, reaches frame 6 at update 119 and the ball leaves in update
120 (state 7): 60 + 8 + 50 = 118 updates after the opening, as
`toss::WAIT_UPDATES`. The cradle (record 656) shows frame 0 at update 10
and gains a frame each 2.5 updates; after the launch record 949 plays a
frame each 1.5 updates to its last frame (16) at update 134. The run-off
then starts script `0x44cb78` (record 947) for one update and is replaced
the next by script `0x44cb61` (record 948, `ab 1.0`), shown alone from
update 144 after its blend; his root travels about 0.33 a update toward
-z. The toss rises to 20.0 (update 168). The contest is decided in update
170 by the first two draws of that update, `r % 20 + 1` for team 0 then
team 1 (18 and 13; 2 and 12), then the loser's `r & 1`: the rewrite's
order. The tip is a pass to the winner's slot 2 (the second match: actor 3
to actor 5) in update 201 (team 0 won; the ball last seen at 14.321) and
200 (team 1 won; last seen at 14.673): it follows the winner's hands
(2026-10-16: the rewrite's hand on the jump his script aims tips in update
200 in both, within one; "Pass flights and the tip-off"). In phase 1
the only AI draws are the two team timers (team+0xb8: at 0 drawn again as
`r % 9 + 9`, else one less; team 0 first); the original's other systems
draw too (0..2 a update in the first match, about 70 in the second).

**The restarts (39 + 17).** Phase 6 follows every basket (the update after
phase 3 ends). The possessing team's strategy becomes the restart offense
(`0x43b0e0`; the defence `0x43b088`) one update after phase 6 begins (the
phase-6 list entries `0x43b2b8`/`0x43b308` run for that update), or later
after a celebration. `FUN_0012bc40`'s play (row `DAT_003ce9b8`, order
`DAT_004e7408..`) equals `restart::plan` in all 56 restarts (one of the 39
reads the update before the strategy appears: the ball under the rim has
|z| below 0.1, which picks between rows 0 and 1). Each player stood within
his spot's radius (or 2.5 for a radius-1 spot) once all six were ready in
229 of 234 cases in the first match and all 102 in the second, the rest
within 4.2 (heights, which push the end-line spots, are not captured). The inbounder runs "loose
ball" then "throw inbound"; the other five "inbound pos". From all six
ready to the pass press: 7..18 updates (37 of 39; two longer after a
replanned restart), the drawn 6..17 and one. The pass press makes the ball
state 4 at once, but the ball stays in the passer's hands for 18 updates
(the pass animation) and phase 6 ends the update after it leaves the hand.
Restarts last 80..313 updates (mean 157) in the original.

**The computer players (decision statistics, first match against a
rewrite computer game of the same length, `ai::capture`).** The same
behaviours run in the same phases and sides: wait for toss (1), loose ball
and open loose ball (2), prepare rebound, drift up court, goal tend, loose
ball and defense (3), fast break against defense (4), the handler's
carrier pause, drive, iso3pt, shoot, pass, feeder, keep away, the cuts,
perim/post pause, post up, set pick, roll, run pick play and pick off ball
against defense (5), and the inbound walks (6). The rates differ: the
original entered phase 3 (a shot) 55 times and phase 6 39 times, the
rewrite shot 13 times and dunked 13 (17 baskets, 20 restarts); its
handler's front behaviour is "pocket" for 4,292 updates of the back court
and 384 of the front (the original's: 1), because `pocket_check` replaces
his queue every frame while it holds and the press does not end it; "wait
for hc" fronts 6,931 back-court updates (the original's 426); a "pass"
front lasts 18 updates in the original (the pass animation) and one in the
rewrite (1,036 + 1,918 updates against 10 + 127). The rewrite's restarts
last 161..361 updates (mean 204). Draws per update: the original's whole
game 0.4 (phase 1) to 10.6 (phase 5), the rewrite's simulation 0.18 to
3.9.

**Fixes from the comparison (2026-10-15, later).** Each was traced and
re-measured against both captures (`ai::capture`, with `STREET_ISO` set so
the rewrite's players walk on the disc's gait records):

- *The pocket.* The holder's ball update (`FUN_0015a060`, raw
  `0x15a1b8`) calls `FUN_00159eb8` every update he holds the ball: the
  pad's pressed word (actor+0x7c & 0x8000000) and `FUN_00141098` pocket
  it whatever move he is in. The AI's pressed word is an edge
  (`FUN_0013a970`: pressed = built & ~last built), so the call
  (`FUN_0012ebf0`), which replaces the queue with "pocket" every update it
  holds, presses once. The rewrite dropped a pocket pressed during a move
  (`World::buttons` only read buttons of a free player) and the button
  then stayed down, so the handler fronted "pocket" for thousands of
  updates; now the holder's Select without a turbo pockets whatever he is
  doing (people too). "pocket" fronts 1..2 updates a match, as in the
  original (2).
- *The pass release.* The pass start (`FUN_0015f930`) filters the 93
  rows of the pass table (`0x160870..0x160e24`: the receiver's sector,
  distance, movement, dribble hand, animation, flags) and draws one
  (`0x160ebc`), calls the pass setup (`FUN_0014bff8`, `0x160e34`: ball
  state 4, ball+0x100 the receiver, the pass sound) at the press and
  plays the row's script; the ball leaves the hands at its callback 38
  (`83 26`) after the script's blend and wait. In phase 6 only rows with
  flag bit 16 pass (`0x160aa4..0x160ac4`): row 10 alone, script
  `0x4548b8`, 6 + 12 = 18 updates (`passing::INBOUND_RELEASE`; every
  inbound in both captures, `tests/pass_release_disc.rs` reads the
  bytes). Live passes spread 10..32 by row (median and mode 20); the row
  filter is not ported and the rewrite holds them `passing::LIVE_RELEASE`
  = 20 (its own). The ball stays `Held` by the passer meanwhile
  (`World::windup`, snapshot 29); the AI reads ball state 4 from the
  press; a swipe can still take it (the original's defender knocked one
  loose mid-windup). Phase 6 now ends the update after the release, as in
  the original. Tips and the rewrite's trick passes and alley-oop lobs are
  still thrown at once.
- *The catch.* With bit 9 read as the catch, a player who has caught a
  pass does not act on it for `ai::CATCH_UPDATES` = 11 updates (the
  first capture's median of 154 catches; 0..25, mostly 10..11 and
  16..17, by the catch animation, which the rewrite does not have).
- *The comparison harness* gives the rewrite's players the disc's gait
  records when `STREET_ISO` is set: the generic walk ran the restart walks
  at 13/5.4/3.6 units a second for stick 64/58/38 where the original's
  inbounder moves about 18/16/14.

The first capture against the rewrite (40,886 updates): before (main at
`bef7902`, generic gait), after with the generic gait, and after with the
disc's gait records:

| | original | before | after | after, disc gait |
|---|---|---|---|---|
| "pocket" fronts | 2 | 546 | 2 | 3 |
| "wait for hc" fronts (back court) | 426 | 146 | 647 | 44 |
| "pass" fronts (back + front court) | 1,036 + 1,918 | 14 + 183 | 135 + 1,470 | 197 + 1,408 |
| shots (phase 3 entries; rewrite shots + dunks) | 55 | 13 + 15 | 9 + 23 | 16 + 26 |
| restarts (updates) | 39: 80..313, mean 157 | 24: 159..256, mean 181 | 28: 161..247, mean 187 | 35: 115..190, mean 135 |
| inbound press to release | 18 | 0 | 18 | 18 |

From all six ready to the pass press the rewrite now takes 7..18 updates
(the original 6..18), and phase 6 ends the update after the release in
both.

What still differs is mostly not a decision: instrumenting the playbook's
shot check (`FUN_001290b8`) shows the quality gate (6 or better, 12..29.75
out) failing nearly every time it is reached, because the rewrite's
defenders stand closer to the handler (a third of phase-5 updates within
4 against a fifth in the original) and the harness's players have the
default 50 ratings (the captured teams are not recorded); the rewrite's
own steal and trick-counter resolution turns the ball over about twice as
often (15..23 changes of possession out of phase 5 against 6); and the
inbounder still walks longer to his spot. The defence presses steal about
as often as the original's (158 against 135 updates in phase 5). The
fast break passes less in the back court (197 "pass" fronts against
1,036), from openness on positions that differ. A pad-word comparison
update by update is only possible in phase 1 (done: the opening's draws);
after the tip the world (positions, animations) no longer matches the
original's, so the same generator state does not give the same decisions.

*The tip moment* is not changed: the two captures' winners tip 81 updates
after the launch (team 0's, ball at 14.321) and 80 (team 1's, 14.673),
but their roots differ by about 0.1 at the same script frame: the moment
follows each jumper's body and hand bones, not the side, so a per-side
timing would not be the original's rule.

**Shots, defenders, steals and counters (2026-10-15, later still).**
Each traced from the raw instructions and re-measured against both
captures with `ai::capture::shots_turnovers_and_spacing_against_the_captured_original`
(the handler's nearest defender in held front-court updates, entries
into phase 3, changes of possession out of phase 5, steal and counter
presses, held balls gone loose, back-court passes; the rewrite's side
also by every player at 50 and by four pairs of the disc's teams,
`ai::capture::rated_match`).

- *The shot's quality* (`FUN_001805e8`, `shooting::evaluate_shot_quality_tier`,
  `ai::offence::quality`) now follows the instructions: integer ratings
  (`FUN_001d7d00`), `cvt.w.s` truncation of `(r + r·f)/25 + 4`, 10 or more
  taken as 8 (`0x180880`: 9 stays 9), beyond the line (`FUN_0017deb0(0.5)`)
  7 with a full meter (`team+0xc == 50000`) else `Threes/25 + 4` (3 past
  27.85), past 30.75 nothing unless the meter is full (7 to 38.75), and
  the two defenders the walk `FUN_0017fd08` gives (the three nearest,
  skipping those down). The contest (`FUN_001801e0`) reads the record
  heights only (no jump), the polynomial arctangent at `0x4b3140`, and
  its thresholds are `0x43257a31` (165) and `0x42d92d5b` (108), both
  truncated: over 165 is 2, over 108 is 1 (the rewrite had them, but
  added each player's jump and rounded). Not traced: the actor's vt+0x5c
  that drops a full meter's long shot to 5 under 25 Threes (`0x180904`).
- *The defender's spot* (`FUN_00108148`, `ai::defence::Defense::spot`) is
  ported whole: the post-up branch (4, 1 or 2.2), the press (3 against a
  handler standing still past 33.75; the press flag `[0x22]`: 3 on the
  ball, 4 on a pass deep in the back court), 2.5 against a shooter (4
  after biting on the fake, `[0x18]`, now kept: `r % 100 < skill(100,
  100, 20, 20) / 2`), the table otherwise (the on-ball column also for a
  pass's receiver after `fps/3` frames of flight), 8 more for a pass to a
  man who with the defender stands nearest the rim, 5 more for the side's
  best shot-blocker (team+0xc4, `FUN_0018b340`) off a man far from the
  rim; then the three-second sag (`[0x1f]`, `r % 100 < skill(0, 0, 94,
  100)`): 11 off a juking handler, 6.8 off a man with turbo 0.2 or more
  who has the ball beyond 11 or is receiving a pass, 2.5 more when the
  defender and the man are the two nearest the rim; and the pick sag
  (`[0x20]`, `skill(70, 100, 70, 100)`): 5 less when a computer teammate of
  the handler sets a pick within 10 (the gap can go negative: over the
  top of the screen; the VU0 code at `0x10894c..` does not clamp it). The
  press draw (`[0x21]`/`[0x22]`, `0x1065e8..0x1067ac`) is ported. The
  defense update also does nothing while its player is in the counter
  stance, a block jump or a swipe (`0x106950..`, `FUN_00181828`,
  `FUN_001818b0`, `FUN_00181650`): the pad keeps the stick it was given,
  so a swipe goes at the handler (the rewrite steered back to the spot,
  and most swipes faced away from the ball).
- *The counter look* (`FUN_00107710`): no look while a teammate is in the
  counter stance (actor+0xa6 bit 0, which the rewrite had read as
  "fallen"); during a trick pass to himself only the defender it goes
  off tries, at `skill(10, 15, 35, 50)`; against a juke `skill(−13, −17,
  40, 50)` plus `skill(−0.25, −0.3333, 0.75, 1) × (Steals − Handles)` (the
  difference kept to 0..50), and after a juke or a trick pass the next
  look waits `fps` frames (`0x107a2c`), not `fps/8`.
- *The swipe's strip* (`FUN_0016cf30`, from `FUN_0015da28` while bit 6 is
  set; once a swipe, actor+0x23c): the swiping hand's bone within 1.5 of
  the ball standing (0.75 moving), then `r % 100 <= trunc(Steals/99 · 64)
  + 20` (30 for a person's defender on Easy, 10 less for the computer's
  against a person's side on Easy; Handles play no part); `r % 100 <=
  trunc((Steals − 50)/50 · 65)` catches it outright, else it is knocked
  loose at 0.35 an update along the defender's facing turned by `r &
  0x7f` (or `− 0x80` standing), rising 0.125 (`0x16d428..`), and the holder
  is barred from it `r % 5 + 25` updates. The sim has no bones: the
  rewrite tests once, at update 13 of the swipe (the captures' strips:
  11..14 mostly), a hand 4 ahead of the defender within 1.5 of the ball
  (`tuning::HAND_REACH`, `HAND_RADIUS`; from the captured swipes, 9.5% of
  them had the ball there at update 13). The swipe lasts 32 updates (66
  of the first capture's 115 swipes).
- *The trick counter* is the contact handler's (`FUN_001343e8`,
  `0x134ac8..0x134e9c`): a defender in the counter stance touching a
  handler in a trick move who stands within ±0xc0 of his facing
  (`FUN_0018d9f0` octants 3..5) lands at `r % 100 <= trunc((clamp(Steals −
  Handles, ±50) + 50) · 0.8) + 20` and takes the ball (the knock-loose
  branch needs `r % 100` over 100); a miss puts him down. The stance lasts
  77 updates (8 of the first capture's 15). The rewrite resolved it at a
  swipe moment from 4.2 away at its own odds.
- *Interceptions*: a passing defender takes a pass only in its last
  quarter (`FUN_0015da28`'s tail: ball+0x60 over ball+0x64 above 0.75), at
  `r % 100 < trunc(Steals/99 · 15)` an update when it is within his reach
  (`FUN_00137db8`, the rewrite's stand-in reach), and a defender in the
  counter stance catches a trick pass off his head (`FUN_00148378`'s kind
  2 leg). The rewrite gave any defender within 1.8 of any pass 25% + 45%
  of his Steals once (16 interceptions a match against the original's 3).
  The ball now keeps its flight's length (`Ball::flight_time`, snapshot
  30).
- *No trick fumbles*: the rewrite lost the ball on about one advanced
  trick in twenty (its own rule); the two captures' 138 trick moves lost
  it only to defenders.

The first capture (40,886 updates) against the rewrite at each stage
(every player 50):

| | original | before | quality | steals, counters | spot, counter look | interceptions, swipe |
|---|---|---|---|---|---|---|
| shots + dunks (phase 3) | 55 | 42 | 44 | 44 | 42 | 48 |
| possession lost out of phase 5 | 8 | 20 | 24 | 13 | 19 | 6 |
| handler's nearest defender within 4 | 26% | 33% | 33% | 35% | 27% | 30% |
| counter presses | 17 | - | - | 61 | 22 | 11 |
| interceptions | 3 | - | - | - | 16 | 0 |
| back-court passes (pass fronts) | 34 (1,036) | (197) | | | | 17 (371) |

With four pairs of the disc's teams (their rosters' first three; which
three the attract match plays and which teams it drew are not
recorded): shots 46..62 (mean 55; the original 55), possession lost 2..8
(8), within 4 31..35% (26%), back-court passes 19..31 (34; 415..688
fronts against 1,036): the back court's passes depend on the ratings
(`FUN_00126288` passes to a teammate with better Handles at `(his −
the handler's) × 5/2`), which at 50 all round never fires. The second
capture (14,155 updates): shots 22 against 17 (15..20 with the disc's
teams), possession lost 0 against 2 (1..4), within 4 20% against 32%
(32..36%), back-court passes 17 against 5 (5..11). The rewrite's held
balls go loose more (16 against 9 in the first capture: the rewrite's
swatted blocks are loose balls; 10 strips against the original's 7).

Still open: the pass start's row filter (`FUN_0015f930`, the 93 rows of
0x1c bytes at `0x3d65e8`, filter loop `0x160870..0x160e24` over inputs
built from `0x15f930` on: the receiver's sector, distance, movement, the
dribble hand, the playing animation and the row flags) needs state the
sim does not keep, so `passing::LIVE_RELEASE` and `ai::CATCH_UPDATES` stay
the captures' medians; the swipe's hand, the counter's limbs and the
interception reach are stand-ins for bones; the attract match's teams
(`FUN_001db7f8`: `r % 28` for each side, drawn in the front end before
the capture starts) cannot be read back from the captures.

## Shots, passes and catches on the script clock (2026-10-15)

Traced raw from the decompile and the executable, measured against both
attract captures (`$EVIDENCE/street-native-ai-attract-1.jsonl`, `-2.jsonl`:
per update, the pads, each actor's channel pc `channel+0x10`, animation
record `+0x50` and frame `+0x5c`, the ball's state and flight) and ported
as `crates/street-sim/src/timing.rs` with its tables
`timing_tables.rs` (regenerated from the disc by
`crates/street-sim/tests/script_timing_disc.rs`;
`street_assets::scripts::script_callback_tick` walks the scripts). The
rewrite's shots, passes and catches now happen when the original's move
scripts make them happen, not on `tuning` stopwatches (`SHOT_RELEASE` 0.9
s was 54 updates; the default jump shots release at 33..34).

**The clock.** The animation VM (`FUN_00193440`) runs a script's opcodes
back to back inside one update until a blend (`8e`, its byte of frames)
or a wait (`8d`, its byte of ticks; `8d ee B C` reaches frame B over C
ticks) installs a count (`state+0x4c`); the game callbacks (`83 id`,
through `0x3e86d0`) are instant. `84 id ... target` jumps when the
callback returns nonzero, `85` when it returns zero (`FUN_00195b80`,
`FUN_00195cd0`); `82`/`9a` jump. So a script's moment is the sum of its
blend frames and fixed waits before the callback, along the path the
callbacks choose. The captures agree update for update wherever a
channel pc names the script (below).

**The shoot press** (`FUN_0015eaa0`, the holder's handler; the pad word's
0x80 at actor+0x7c) starts a script with `FUN_0019c2f8(channel, body, 0,
0, tag)` and the ball stays held (state 3) until the script's callback 21
(`FUN_001729f8`, the release and launcher, the section "Shot release and
launcher"). The ball does not leave on the button's release:
- Callback 20 (`FUN_00172348`, `85` in the default shots at 12..15
  updates) reads the held word (actor+0x78 & 0x80): not held is the pump
  fake (+0x1d0 = 1, its event) and the script falls through to its end;
  held, the script jumps on to the release. A computer player holds the
  button through its shot (the captured built word keeps 0x80), so only
  a person fakes.
- Callback 17 (`FUN_001726c0`, `84` at the same tick) with the stick
  pushed (+0x90 > 15) within 45 degrees of straight away from the basket
  (`FUN_0018d500` 0x180..0x280) jumps into `jumpshot_1_fadeaway`
  (`0x44f945`): the Street School fadeaway is decided there, not at the
  press. The capture shows it: 7 of 11 `jumpshot_2hand_legkick_2` shots
  released 34 updates after the press (the plain path), 4 at 32 (= 13 +
  the fadeaway's 19, the channel pc inside `0x44f945` from update 13).
- The script (`FUN_0016b4b8`, after the gamebreaker, dunk and lay-up
  branches): a two-turbo pair (`0x43d460`) plays the slot's row of the
  27 shot scripts (`0x3d8fd8`, the generic set `0x43d6c0` = ids 4, 7, 5,
  6, 3, 1 through `FUN_0013a1f8`); facing 400..600 units from the rim
  (`FUN_0018d290`) `jumpshot_2_fadeaway` with state bit 11 and within 25,
  else the default three; moving sideways (160..300 / 672..812) the
  side fades by the live hand (+0x25c: 1 gives `sidefade_left_r`, else
  51% a left fade, else a hook of `0x3d9038`); running at the basket
  from long range `jumpshot_runner_1`; else `0x3d7b10`'s three
  (`jumpshot_2hand_legkick_1/2`, `jumpshot_stepback_2hand_legkick`,
  `rand % 3`), the female set `0x3d7b20` (`jumpshot_1`), Bird's and the
  hooks of legends 0x95/0xa6/0xb2. In a Level 1 gamebreaker the court
  map `0x3d2790` (`FUN_00141298`) picks the gamebreaker shots
  (`0x44f7f8`/`0x44f89e`, both 33).
- Callback 21 by script (updates from the press, plain path):
  legkick_1 34 (check 14), legkick_2 34 (13), stepback 33 (12),
  `jumpshot_1` 30, `_2` 41, `_4` 34, `_5` 42, `_6` 37, `_7` 38, `_8` 31,
  hooks 32/27/29, `jumpshot_1_fadeaway` 19 (from its entry), `_2_fadeaway`
  44, runner 24, bird 28, side fades 39 (right_r, left_l) and 32
  (right_l, left_r), gamebreaker 33, heave 19; the scripts end 31..59.
- Dunks: the two-turbo slot's dunk (`0x43d5d0`: ids 48, 21, 0, 35, 58,
  20, 23, 2, 15, 37, 39) through `FUN_001693b8`'s row lookup in the
  99-row candidate table `0x3d7b28` ({body, legend, mask, family, id}),
  else its filter: the approach sector seen from the rim
  (`FUN_0018d768(0x300 or 0x100, rim, player)`, mask bits 0..7), the hand
  (0x20000/0x40000), the distance bands 12.5 and 8.0 (`0x3d82e4/e8`:
  mask 0xc00, 0xa00, 0x900), the Dunks-rating gates (under 21:
  `rand % 100 > Dunks` keeps only 0x80000 rows; under 60: 51% drops
  0x80000, else 0x400000), the per-player unlocks (`FUN_00139d38`), the
  legends' rows, then `rand % count`; no row gives a jump shot. The slam
  is callback 21: 42 (`dunk_5`) to 66 (`dunk_18`), 57 the mode; the
  alley-oop finishes 89..92 from the pass press.
- Lay-ups: the same table's family-2 rows (the `layup_*` scripts), 31
  (`layup_boardslap`) to 75 (`layup_15`); the captured ones 41, 42, 44
  (`layup_9/10/13`). `FUN_001773b0` (callback 32) is the dunk-to-lay-up
  finish (ids 0x30..0x33 = the `bbdunk_*` bodies).

**The pass press** (`FUN_0015f930`): the pass setup (`FUN_0014bff8`,
state 4) at the press, then the row filter `0x160870..0x160e24` over the
93 rows of `0x3d65e8` and the draw `rand % count` (`0x160ebc`); the row's
script lets go at callback 38 (`FUN_00173570`). Row fields: +0x04 a
legend record id (rows 0..5: Bird 172, Magic 166, Pistol 169, 160),
+0x08 the Passing rating a computer needs (80 on the kick rows 6..9; a
person never draws a rated row without L3 held, and with L3 only those),
+0x0c/+0x10 the distance band (from the passer's spot a quarter second
ahead to the receiver's catch spot), +0x14 flags: bits 0..7 the
receiver's sector from the passer's facing (`FUN_0018d768` through the
bone yaw `FUN_00154230`: 0x38 ahead, 0x40/0x06 the sides, 0x01/0x03
behind, 0xff any), 0x100 running rows and 0x200 standing rows (a moving
passer on the ground takes the standing rows when `0x208` is set: running
out of bounds within 10 updates, flipped when the stick is under 41 of
64 or he is barely moving), 0x400 airborne, 0x800/0x1000 not with the
left/right hand, 0x8000 the alley-oop rows (kind 1), 0x10000 phase 6 (row
10 alone; outside phase 6 only in the cradle animation 656), 0x80000
the back-court heave with 9 or more on the clock, 0x100000 the dive
rows (animations 0x47/0x48), 0x200000/0x400000 to oneself, +0x18 the
kind left on +0x268. A receiver within 5 (`FUN_0018d0c0`) and not dunking
is a handoff: callback 38 called at once (8 captured passes released at
the press); no candidate row plays `pass_fwd_1`. Callback 38 by row
(updates from the press): `pass_fwd_1` 18, `_2` 20, `_4` 16, `_5` 21,
`_6` 10, `_7` 14, `pass_fwd_run_1..5` 18/15/20/16/21, `left_or_right_1..5`
17/18/21/31/24, their run rows 17/23/21/31, `_run_6` 31, `_6a` 18,
`behind_1` 20, `_2` 24, `_3` 24, `_5` 28, `behind_run_1` 20, `_5` 26,
`allyoop_1` 12, `post_1` 20, `air_1..9` 14/15/18/23/22/21/19/22, dropkick
26, knee 32, sidekick 26, the long `pass_fwd_right_run_1` 46, the
legends' 16..24, the dive passes 4..12. The passout rows (self passes)
have no callback 38 on their path.

**The catch** is a script too: the release callback draws the receiver's
catch row (`FUN_001611d8`, the 39 rows of `0x3d75c8`: {body, min flight,
flags, name, _}) and plays it on him at the release. Its `9b 00 02`
sets bit 9 (actor+0xa4, `FUN_0016ea00`, the AI's catching test), its
blend takes 6, its `8d ee B fd` wait lasts the rest of the ball's flight
(`FUN_00197298`'s -3: `ball+0x64 - ball+0x60`, at least a third of the
frames), and its tail then clears the bit with `9c 00 02`:
`catch_stand_1` plays the clip out (`8c 1.5`: 9 updates in every one of
53 captured), `catch_stand_2/3/6` wait 15, `_4` 20, `_7/8` 10, the post
catches play out. The row: the catch spot's sector from the receiver's
facing (flags bits 0..7), 0x4000 standing and 0x2000 running rows, 0x800
moving on the ground, the shortest flight (+0x04: 20 for `_4`, 15 for
`_5`, 10 for `post_2`), bit 11 (0x8000), the `0x3d65b8` and 0x40000
gates; airborne receivers take row 0; `rand % count`; 0x200/0x400 set the
hand. So the hold after the catch is the flight's nominal end still to
come (the ball is caught 1..5 updates early) plus the tail; the
captured holds: 10..17 for standing catches (`catch_stand_1` 9 + 1..5,
`catch_stand_3r/3l` 15 + 1..2), and 9 after the flight's end for every
running receiver (74 of 77; the `catch_run_*` scripts' own `9c` would
clear it at the flight's end, so what keeps it is not identified).

**Measured** (`ai::capture::shots_release_at_their_scripts_callback_21`,
`passes_leave_the_hand_when_the_animation_lets_go`): the first capture's
78 shots by the computer: every script the tables name released at its
walked moment, plain or fadeaway path (`dunk_2h_2f_rev_hang` 65 against
57 is an animation-test branch the walker does not take); the ball at
the release 11.5..13.5 up for jump shots, 12.0 at the rim for dunks,
13.2..13.7 for lay-ups. 233 passes: 56 inbounds at 18, 131 live passes
on their row's moment, 8 handoffs at 0; the second capture's 19 shots
and 37 live passes the same. The played captures carry no pads, and the
human's shots there start from the actor state bits without a channel
pc, so they are not in the numbers.

**Ported** (`timing.rs`): `shot_script` (the script by turbos, facing,
movement, hand and the draws in the original's order), the check tick
(`check_pump_fake`: the fadeaway from the stick, the pump fake from the
button; `Action::Shooting` carries `tick`, `script`, `release`, `check`,
`end`), `move_timing` (dunks and lay-ups: the slot's dunk or the
candidate filter by sector, hand, distance band and rating gates;
`Action::Dunking`/`Layup` carry `release` and `end`), `pass_row` (the
filter's phase-6, kick, hand, sector, distance, standing/running,
airborne, alley-oop, heave and handoff rules; `Windup.row`), `catch_row`
and `catch_hold` (`Pass.catch`, `World::catch`; the AI's `catching` reads
it). Snapshot version 31. Stand-ins, labelled in code: the jump's curve
between the traced moments (`SHOT_RISE` 18, `DUNK_RISE` 12, `LAYUP_RISE`
24, the putback's 15/45), the Handles rating for the record's rating 7,
no legend rows, no created player's equipped rows, state bit 11 read as
clear, the receiver's spot 25 updates ahead and landing-within-10 not
modelled, the per-player dunk unlocks not applied, the family kept to the
sim's own dunk/lay-up decision, `dunk_2h_2f_rev_hang`'s second path, the
gamebreaker shot by range instead of the court map, the running row's
velocity along the facing, and the running catch's 9 (measured).

## The display on the script clock (2026-10-15)

The simulation's shots, passes and catches were put on the original's
move-script clock in the previous section; the player drawn on screen
still played a clip of the view's own choosing, stretched to the old
stopwatch lengths (`tuning::SHOT_RECOVERY`, `DUNK_TIME`, `LAYUP_TIME`).
Now the view plays the script the simulation chose, update for update,
the way the actor's animation channel plays it. Traced raw from the
decompile and the executable; built as `street_assets::playback`
(the channel), `street_game::channel` (what the sim says each player's
channel plays, and the animator) and `view::sync_players`'s first branch.

**The channel's step** (`FUN_0019a658(actor, channel, n)`, the channel at
`actor+0xc4`: `+0x4c` the VM's count, `+0x50` the record, `+0x54` a
blend's incoming record, `+0x5c` the integer frame, `+0x60` a span's
target frame, `+0x64` the float frame the pose samplers read, `+0x68` the
rate in frames an update, `+0x6c` a span's length and `+0x70` its
counter). The VM (`FUN_00193440`) counts `+0x4c` down each update and,
at zero, steps a running span (flags `0x200008`) once and then runs
opcodes until one installs a span; a span's handler steps it once itself
in the opcode's own update, so a span of C updates occupies that update
and C - 1 more, and the opcodes run again on the C-th after it. The step:
`+0x70` goes up; past `+0x6c` the span is over (a blend makes `+0x54`
current at `+0x60`, a wait lands on `+0x60`) and `+0x4c` is 0 so the
opcodes go on in that update; otherwise a blend holds the outgoing clip's
frame and mixes the incoming clip at its start frame by `+0x70 / +0x6c`,
a wait adds `+0x68` to the float frame and, on its last step, sets it to
`+0x60` exactly; the integer frame is the float frame truncated after
every step (the EE's `cvt.w.s` rounds toward zero) and `+0x4c` is 1.
- `8e [selectors] F start record` (`FUN_001978b0`): a blend of F updates
  to the record (`-1000` the record already on the channel), `+0x60` =
  start, `+0x68` = 1/F; the alternates by selector as before (the display
  evaluates `0xf8`, the live left hand, and plays the primary for the
  others: a stand-in).
- `8d ee B C` / `8d B C` (`FUN_00197298`): `+0x60` = B (absolute, clamped
  to the clip's last frame) or the integer frame + B, the float frame
  reset to the integer frame, `+0x68` = (B - frame) / C over C updates.
  C = `0xfd`: the rest of the ball's flight (`ball+0x64 - ball+0x60`
  while the ball is in a pass, else half the frames to go, at least a
  third of them); C = `0xfe`: `FUN_0019bed8(gravity, actor+0x44,
  actor+0x34)`, the updates until the actor lands (the raw `0x1973e0..
  0x197438`; Ghidra cut it at the settings getter `FUN_0030fa50`, whose
  `+0x28` is the fps); `0xef D` after the count: `channel+0x20 + D`
  (the passout rows only).
- `8c x` (`FUN_00196f38`, raw `0x196f38..0x197164`): the rest of the clip
  to its last frame, `+0x6c` = ceil((last - frame) × x × fps / 60),
  `+0x68` = 1/x.
- `ab x` (`FUN_00197168`): `+0x68` = 1/x with flag `0x2000`: the clip runs
  on at that rate, wrapping, and the opcodes never resume (the referee's
  cradle).
- `9f` takes a four-byte float (`FUN_00194c40`, `addiu t1, a2, 4`), not
  three bytes: the inspector read the shots' `9f 00 00 80 3e` as `9f` and
  a bare `3e`, which the walkers never counted; fixed. `84`/`85` carry
  their callback id like `83` (`FUN_00195b80`, `FUN_00195cd0`), which
  the decoder now records, so `script_check_tick` finds them.
- A bare byte n: `FUN_0019a658(n)`, the rate 1 and a frame on, then
  `+0x4c` = n; none on the paths the sim plays.

**What the sim names.** `Action::Shooting.script` (and the fadeaway's
switch at the check), `Action::Dunking.row` and `Action::Layup.row`
(`timing::move_timing` returns the row of `MOVE_ROWS` it drew; the
fallbacks are rows too: `dunk_1h_2f_spike` 50 and `layup_13` 85, the
captured modes; the putback has none), `Action::PumpFake.script` and
`.tick` (the shot script's pump-fake path: the `85` of callback 20 fallen
through), `Action::Passing.row` and `.tick` (the pass row from the press
through the follow-through; `NO_PASS_ROW` for a handoff, a lob to a
leaper and the trick passes), `Pass.since` and `Catch.row/flight/since`
(the catch row's script from the release update, the pass's nominal
flight for its `fd` wait). Snapshot version 32.

**The display** (`street_game::channel`): the script's program is
compiled from the executable along the VM's plain path (`85` jumps, `84`
falls through, as the walker) and, for the shots with a check, the pump
fake's path; `channel::scripted` turns the action into the script and
its update; `playback::pose_at` runs the channel to that update and gives
the record, its frame and the blend in progress; `channel::apply` puts
them on the renderer's animator held still (the sim's updates move it),
the blend as the crossfade at the script's weight
(`Animator::set_fade_progress`) and the outgoing clip at the frame the
blend holds. The clip's own root travel is dropped (the sim moves the
player). No stretching remains: `tuning::SHOT_RECOVERY`, `DUNK_TIME` and
`LAYUP_TIME` are gone (`PUTBACK_TIME` keeps the putback's own clip).

**Measured.** `channel::tests::the_programs_run_their_callbacks_on_the_walked_updates`:
the channel's clock lands every shot's, dunk's, lay-up's and pass's
release callback on the table's update (the tables measured against
both attract captures). `the_channel_shows_the_sims_scripts_and_releases_on_their_updates`
(through `World::step`): a jump shot, a dunk, a pass and its catch; the
script the sim names plays from the press and its release callback runs
in the update the sim lets the ball go in (`Event::Shot`, the slam, the
ball's state turning to a pass), the catch row from the release update
through the flight and the hold. `every_scripted_clip_is_on_the_disc`:
every blend target of every program resolves to a loaded clip.
Screenshots of the demo match (`--demo --events`, seed 2002,
Lakers/76ers, Rucker) before and after at the release updates, under
`$EVIDENCE/street-anim-clock`: `jumpshot_2hand_legkick_2`'s release
(update 515), `dunk_4`'s slam (1747) and pass row 52's release (343): the
drawn pose is the script's at the frame the waits reach, the ball at the
hands as it leaves them; before, the view's own clip stretched to the
stopwatch sat elsewhere in its motion. `--events <file>` writes the
match's events, ball states and action starts with their updates, and a
screenshot run's clock is read in the frame it is set, so `--at U/60`
captures the frame after update U.

**The rewrite's own, labelled in code:** the airtime wait (`0xfe`) is one
update (the sim's own jump lands at the script's traced end, which
counted that wait as nothing); the sim's shot, dunk and lay-up ticks read
1 at the end of the press update and release when they reach the
script's count, so those scripts are shown at that tick (one update ahead
of a VM started in the press update), while a pass's tick, which releases
an update later, is shown one behind; the blend alternates other than
`0xf8` play the primary; the sim's `PASS_TIME` and `PUMP_TIME` end the
passer's and the faker's action before their scripts' tails, and the
view goes back to the ordinary animation then; a clip's frame count is
the decoded clip's; the `fd` wait reads the sim's nominal flight rather
than `ball+0x64 - ball+0x60`; the integer frame assumes truncation.
## The original's default options (traced 2026-10-15)

The options object sits at `config+0xfc` (`config` is `FUN_001e0bc0()`'s
game object). `FUN_001d6e90` builds it and calls the reset
`FUN_001d6f20(options, 0)`, its only caller, so the save-pointer branch at
the top of the reset is never taken. The reset zeroes the staging block at
`options+0x18` (`config+0x114`, 24 bytes) and the live block at
`options+0x30` (`config+0x12c`), calls the setters below on the staging
block, and copies it over the live block. Ghidra marks `FUN_0030fa50` (the
engine config getter, called at `0x1d70e4`) noreturn and cuts the decompile
there; the tail `0x1d70ec..0x1d72e8` was read with `disassemble_bytes`.
Every setter takes `(options, value, block)` and writes `options + 0x18 +
block*0x18`, so the Options screen edits the staging copy and
`FUN_001d7780` copies one block over the other when it confirms or cancels.
Every word is little endian; the rewrite's field is `records::Settings`'s.

| Word, bits | Setter (value) | Default | What it is, who reads it | `Settings` |
| --- | --- | --- | --- | --- |
| 0 (u16 at +0), bits 0..1 | `FUN_001d74a8(0)` | 0 | Difficulty: 0 Got Game, 1 Mad Game, 2 Legendary | `difficulty: 0` |
| 0, bit 2 | `FUN_001d72f8(1)` | 1 | Onscreen Help | `help: true` |
| 0, bit 3 | `FUN_001d7320(1)` | 1 | Autosave | `autosave: true` |
| 0, bit 4 | `FUN_001d7348(0)` | 0 | Widescreen: the renderer wrapper's first flag (`wrapper+0xc`), `FUN_00323450`'s 16/9 branch | `widescreen: false` |
| 0, bit 5 | `FUN_001d7370(1)` | 1 | Visual Effects | `visual_effects: true` |
| 0, bits 6..8 | `FUN_001d74f8(0)` | 0 | Camera type 0..4 = Street, Court, Slide, Solid, Wide (`FUN_003038b0`) | `camera: 0` |
| 0, bits 9..11 | `FUN_001d74d0(s0)` | 0 | The text language (not an audio setting, as the handoff guessed): the readers compare `== 2` for the French strings (`FUN_001ec330` "EQUIPE ADVERSE", `FUN_002278c0`, `FUN_00246880`, `FUN_00252e28`, `FUN_00265f48`). The tail sets `s0 = 2` when `engine+0x2c != 0` and the console's OSD language is 2 (`FUN_00342088`: the system configuration word `FUN_003bdd90` reads, its version bits 13..15 choosing between the Japanese bit 4 and the language bits 16..20; `DAT_0041717c` overrides it when `FUN_00342048` finds its 'T'), 3 when `engine+0x1c == 1`, 4 when `engine+0x1c == 2`, else 0. The engine config (`FUN_0030fa50` -> `DAT_004da218`, built by `FUN_0030f938` with `+0x1c = 0` and `+0x2c = 0`) is never written there: no xref to `0x4da234` or `0x4da244` and no write through the getter. So this executable always sets 0, English; the French branches are dead on the US disc | not kept (English) |
| 0, bits 12..13 | `FUN_001d7520(1)` | 1 | Replays: 0 Always, 1 Automatic, 2 Manual | `replays: 1` |
| 0, bits 14..15; u16 at +2 | none | 0 | unused | |
| 1 (+4), bits 0..1 | `FUN_001d7548(3)` | 3 | no reader found in the decompile | |
| 1, bits 2..4 | `FUN_001d7578(sound+0x3c)` clamped to 0..2 | 1 | Audio Options: 0 Mono, 1 Stereo, 2 Dolby Pro Logic II; `FUN_00228508` applies it with `FUN_001a1670(sound, mode, ..)`. The default is the sound object's output mode `+0x3c` (`FUN_0019f7c8()`), which its constructor `FUN_0019f810` sets to 1 and only the option screens change (`FUN_001a1670`'s callers: `FUN_00228508`, `FUN_002a5870`, `FUN_002a6d40`): Stereo. There is no Dolby detection | `audio: 1` |
| 1, bits 5..15 | none | 0 | unused | |
| 1, bits 16..19 | `FUN_001d75b8(10)` | 10 | a fourth volume no reader uses | |
| 1, bits 20..23 | `FUN_001d75f0(10)` | 10 | SFX volume (`FUN_00228508`: `FUN_001a15a0(sound, 1, ..)`) | `effects: 10` |
| 1, bits 24..27 | `FUN_001d7628(10)` | 10 | Music volume (channel 0) | `music: 10` |
| 1, bits 28..31 | `FUN_001d7660(10)` | 10 | Speech volume (channel 2) | `speech: 10` |
| 2 (+8), bit 0 | `FUN_001d76b8(1)` | 1 | Music Experience: 1 Interactive (PathFinder), 0 Playlist | `interactive: true` |
| 2, bit 1 | `FUN_001d76e8(1)` | 1 | Song Rotation: 1 random, 0 in order | `sequential: false` |
| 2, bit 2 | `FUN_001d73d0(1)` | 1 | Track Info | `track_info: true` |
| 2, bits 3..31 | `FUN_001d7398(track, track != DAT_003e9ca8)` for tracks 0..28 | all on but track 20 (corrected 2026-10-18) | The 29 track slots (`FUN_00219588`'s tables `0x467710`/`0x4678d0`: nine front-end songs, then PATHSONG.CFG's 15, five spare). `DAT_003e9ca8`, `DAT_003e9cc8` and `DAT_003e9ce8` are never written (every xref is a read; the ELF holds zeros), so the retail branch runs (the `DAT_003e9ce8 != 0` branch, two tracks on, is a demo build's) and the one track off is 0: Nelly's `NELLY.ASF`, the playlist's first song. `FUN_001e0420` (a user record whose `FUN_001d2818` reaches 10) and `FUN_002f2d58` (mode 5, event 10) switch it on, `FUN_002f2e50` off, and `FUN_00219220`'s Trax list looks for it: it waits for Nelly's card | `songs_off: 1 << NELLY_SONG_BIT` (bit 21) Correction, 2026-10-18: `DAT_003e9ca8` is `audio_rucker_classic_track`, registered with 20 through a register store at `0x1a2a2c` that the xrefs missed; the track left off is 20, the Rucker Classic's song (in-game song 12), as every save state's Trax word 0xff7fffff holds, and Nelly's is on ("The wardrobe, Edit Baller and the original's own save") |
| 3 (+12), bit 3k, k = 0..3 | `FUN_001d7400(k, 1)` | 1 | pad k's vibration (`FUN_002144c8` lists it, `FUN_002dd548`/`FUN_002f3638` read it) | `vibration: true` |
| 3, bits 3k+1..3k+2 | `FUN_001d7720(k, 0)` | 0 | pad k's Controller Layout A, B, C (`FUN_00273148`, `FUN_002b4288`) | `layouts: [0; 4]` |
| 3, bit 12+k | `FUN_001d7458(k, 0)` | 0 | a per-pad flag the player's control reads (`FUN_0012fd60`) and the controller screen lists; its row is not identified | |
| 4, 5 (+16, +20) | `FUN_001d7690(options, {0, 0})` | 0, 0 | Adjust Screen's x and y offsets (`FUN_0025ba10` and `FUN_002a6d40` move the screen with them through `FUN_0030f460`) | the rewrite's Adjust Screen row is the window's full screen, `full_screen: false` |

The 24 bytes the reset leaves: `2c 10 00 00 | 07 00 aa aa | ff ff 7f ff
| 49 02 00 00 | 00 00 00 00 | 00 00 00 00` (`records::ORIGINAL_DEFAULT_BLOCK`;
the third word corrected 2026-10-18, it read `f7 ff ff ff`);
`original_save::options_from_original` reads them as `Settings::default()`
(`the_defaults_are_the_originals_reset_words`).

In the rewrite: `Settings::default()` was volumes 6/8/8, Mad Game,
Widescreen on and every song on; it is now the table (volumes 10, Got Game,
Widescreen off, the Rucker Classic's song off: corrected 2026-10-18, it
said Nelly's, whose card's switch is not ported). `AudioSettings` and `PadSettings`
take their defaults from it, a fresh save boots with them (`apply_settings`
in `menu.rs`), and the match's difficulty is the saved option's unless
`--difficulty` is given. The save format is unchanged (every field is
stored, so existing saves keep their values); the Wide picture (the three
pictures section of `docs/CAMERA_RECOVERY.md`) is bit 7 of the flags byte,
0 in older saves. Not traced: a reader of word 1's bits 0..1 and 16..19
(none found), the per-pad bit 12 row, and the controller screen's rows for
them.

## The person's shots, passes and catches (2026-10-15)

Two per-update captures of the original running the user's Pick Up save
(slot 1, the match opening; Got Game, the options word `0x102c` at
config `0x4bbf10 + 0x12c`) played from the keyboard of the project's
PCSX2 copy, with every actor's pad copy, channel, pump-fake and shot-kind
words, the ball and the sides' statistics: `$EVIDENCE/street-played-
pads-1.jsonl` (14,927 updates, 197..15,123) and `-2.jsonl` (24,620
updates, 197..24,816), neither torn nor missing an update
(`tools/pine_played_capture.py`; the state was loaded through PINE's own
load-state command, since F3 sent with SendInput never reaches PCSX2 here
while the pad keys do). The person's actor is the one whose controller
word (game+0x24 + id*4) is not 4; in every restart the computer walks him
(actor+0x298 = 1) and throws the inbound itself, so the inbounds are the
computer's. Measured with the session's `measure_played.py`, pinned by
`crates/street-sim/src/played_capture.rs` (`STREET_PLAYED_CAPTURE`).

**The shoot press** (actor+0x7c & 0x80 while holding the ball) starts
the script in the press update: by the end of it the channel's pc is
inside the script's first blend, and the script's tick 0 is the press
update, as it is for the computer (`captured_shots` in `ai::capture`
measured the same). The computer's 45 shots in the two captures sit on
the tables as before (36 released on their row's or script's callback 21,
one on the fadeaway path, the rest never released: blocked or stripped);
the person's do not, because of an opcode the walkers counted as nothing:

- **The release gate.** Every jump-shot script (not the hooks, the
  heave, the female set or the gamebreaker heave) has `86 13 00 00 00`
  on the held path before its callback 21: opcode `0x86`
  (`FUN_00195fc8`, table `0x2e9610`) calls game callback 19
  (`FUN_00174c10`, raw `0x174c10..0x174d24`) and, while it returns 0,
  sets the channel's count to 1 and moves the pc back onto itself, so
  the VM runs it again next update; the frame holds meanwhile (the
  channel flags lose bit 3). Callback 19 returns 1 at once for a
  computer's actor (`FUN_00172220`: phase 0 or 1, or the actor's
  controller is 4), else 1 when the held word (actor+0x78) has no 0x80
  (the button is up), else 1 when the actor is within so many updates of
  landing: actor+0x64 − actor+0x60 ≤ 15 if the channel's clip record is
  one of the 26 at `0x3d9218` (the jump-shot clips 306..317, 320, 321,
  324, 325, 328..331, 360..365), else ≤ 5. The jump opcode `9f`
  (`FUN_00194c40`: its four-byte float × 60 / fps into actor+0x44) stores
  `FUN_0019bed8(g, vy, y)` at +0x64 and 0 at +0x60 (raw `0x194ce4..
  0x194d30`), g = `32.152233 / fps² × −1.2596 × 0.5` (`0x42009be3`,
  `0xbfa13ba4`); `a1`'s last four bytes are the same jump
  (`jumpshot_1_fadeaway`, the runner). `FUN_0019bed8` (raw
  `0x19bed8..0x19bfb0`) takes the roots of `g t² + vy t + y` and rounds
  the larger one beyond 1 up: a jump of 0.25 lands in 45 updates, 0.2125
  in 38, 0.225 in 40, 0.15 in 27. So for the stepback (jump at tick 15,
  gate at 31, callback 21 two ticks after the gate) the ball leaves at
  33 with the button up by 31, at the button's release + 2 up to 45, and
  at 47 however long the button is held: the capture's holds of 18, 24,
  36 and 45 updates released it at 33, 33, 38 and 47; `legkick_1` (gate
  30, tail 4, latest 44) at 34 for 18..24 and 40 for 36; `legkick_2`
  (gate 32, tail 2) at 34 for 18..24; the side fades `right_l`
  (gate 28, tail 4) at 34 for a hold of 30, `right_r`/`left_l` (gate 33,
  tail 6) at 39 for 24 and 30. The side fades' jump is callback 18
  (`83 12 1`), not their own opcode, so their landing cap was not tabled
  here (traced since: "The catch-up stage and the side fades' landing").
- **The pump fake** (actor+0x1d0 set by callback 20): taps of 3 and 4
  updates on the stepback, `legkick_1` and the heave set it 12, 14 and 8
  updates after the press (the scripts' check ticks), the held word
  already clear there; the script went on to a standing transition and
  the ball stayed in the hands (5 of 5).
- **The fadeaway** (actor+0x1e4 = 8 by callback 17; `FUN_0016b4b8`
  writes 9 there for a jump shot, 8 for `jumpshot_2_fadeaway`, and 5 is
  read through a dunk): one stepback with two arrows held (actor+0x90 =
  90, heading within 45° of straight away from the basket:
  `FUN_0018d500`'s angle 455 in 0x180..0x280) switched at 12 and released
  at 31 = 12 + 19, the button up at 14 (before the fadeaway's gate at
  15 from its entry). The heave has no callback 17: five back-court
  presses with the stick away stayed heaves.
- **The heave.** `FUN_0016b4b8` at `0x16b878`: with no two-turbo slot
  script, the shooter's x on the other side of the centre line from the
  rim's and more than 13 from it plays `heaveshot_1` (callback 20 at 8,
  no gate, the ball gone at 19): all 14 back-court presses in the first
  capture (13 released at 19, one faked at 8).
- **The D-pad.** actor+0x90 reads 64 for one arrow and 90 for two
  (1,286 and 1,180 updates): `cvt.w.s` truncates 90.51; the rewrite
  rounded it to 91.

**The pass press** (actor+0x7c & 0x100): the person's 7 live passes
released at their rows' callback 38: five plain (`pass_fwd_5` 21,
`pass_fwd_1` 18 twice, `pass_fwd_6` 10, `pass_left_or_right_4` 31) and
two kick passes (L3 held, `0x800` in the held word) that drew
`pass_sidekick` (row 8, rated 80), released at 26. **The catch**: the
person's 30 receptions
held bit 9 for the flight's nominal end plus the row's tail like the
computer's (`catch_stand_1` 10..14 = 1..5 + 9, `catch_stand_3r` 16..18 =
1..3 + 15, running receivers 2 + 9), but one: a running person's actor
whose channel showed the run clip let the bit go at the flight's end
(2), the `catch_run_*` scripts' own `9c`.

**The make odds.** The person's jump shots in the two captures went in
22 times in 26 (the stepback from 25 with a contesting defender and a
side fade with one missed; the 13 heaves all missed). The release
callback's inputs read from the captures (game+0x374, the long-range
rule, the record's Shots/Threes, the opponents within 7 and those with
bit 12, the lead and the stage bounds game+0x4b0/+0x4b4) through the
sim's `shot_percent` expect that many with the person's doubled catch-up
and +40 inside 33.75, and far fewer without them (the numbers are the
`played_capture` test's `odds` report). The roll itself cannot be pinned
to a draw: the computer players draw 7..19 numbers an update before the
release callback runs.

**Ported** (`timing.rs`, `player.rs`, `shooting.rs`): the shot, dunk,
lay-up and fake ticks are the script's (the press update its tick 0, the
release at `release` updates after the press, the check at `check`; they
were one update early); `ShotScript` carries `gate`, `latest` (the jump
plus its airtime less 15 or 5) and `fades`; `Action::Shooting` the gate,
the latest and the updates stalled; a person's script waits at the gate
while the button is held until `latest`; the heave branch of
`shot_script`; `timing::airtime` (`FUN_0019bed8`); the fadeaway only on
scripts with callback 17; `dpad_magnitude` truncates. The display
(`street-game/src/channel.rs`) shows the script at `tick − 1 − stall`,
so the drawn frame holds through the stall. Snapshot version 33. The
rewrite's own, labelled: the side fades' landing cap (traced since, in
the next section), the sim's jump curve through a stall (the original's
actor keeps falling), the catch of a running person's receiver (one of
two kinds).

## The catch-up stage and the side fades' landing (2026-10-15)

Two leads the played-capture pass left, closed from the raw instructions
(`tools/mipsx.py` on `SLUS_206.51`) and checked against the same two
played captures (`$EVIDENCE/street-played-pads-1.jsonl`, `-2.jsonl`,
through `crates/street-sim/src/played_capture.rs`).

**The catch-up stage is the larger scaled score, not their OR.** The game
object's virtual +0x44, called as `(table, side)`, is `FUN_0012eac0` for
the points game (vtable `0x43b490`), `FUN_0018c788` for the trick-point
game (`0x440ce8`), `FUN_00145b48` for Gamebreaker Wins (`0x43e9d0`) and
`0x143040` (0) for the base class (`0x43dcf8`).

- `FUN_0012eac0` (raw `0x12eac0..0x12ebe0`): `FUN_00186538(groups, 1,
  table, 3, 31)` gives group 1's table when it is 31 rows of 3 (else the
  bonus is 0). Each side's points (`FUN_001871e0(stats, side, 1, 0)`)
  times 21 (the three-operand `mult`) is divided (`div`) by game+0xf8, the
  target. At `0x12eb6c` the two quotients go through `pmaxw a1, s0, v1`
  (MMI0 function 3: the larger of each word pair) and `pextlw a1, zero,
  a1` (the low word zero-extended); then `a1 < game+0x4b4` gives
  `(a1 < game+0x4b0) ^ 1`, else 2. The decompile's `uVar2 | uVar3` is the
  project's Ghidra copy, not the game: `tools/ee_patch.py` rewrites every
  MMI instruction with a destination as `or rd, rs, rt`. The lead, own
  less other, is clamped to ±15 (`slti`/`movz`) and the bonus is the
  signed byte (`lb`) at table + (lead + 15) × 3 + stage.
- `FUN_0018c788` (raw `0x18c788..0x18c8b4`): the sides' trick points
  (stat 0xb), the larger by `slt`/`movn` (`0x18c810`), the lead clamped to
  ±50,000 and scaled by 31 / 100,000; its constructor `FUN_0018c5c0` sets
  the bounds to target / 3 and 2 × target / 3.
- `FUN_00145b48` (raw `0x145b48..0x145cfc`): each team's meter
  (game+0xc8 + side × 4, then +0xc, a float, `cvt.w.s`) over 500
  (`div`), 100 more when the side's stat 0xc (Level 1 gamebreakers) is
  nonzero (`movn`/`movz` at `0x145bf4`, `0x145c3c`); the larger by
  `slt`/`movn` (`0x145c44`); the span 100 when game+0xf8 is 0 and 200 when
  it is 1; the lead clamped to ±span and scaled by 31 / (2 × span). Its
  constructor `FUN_001457f8` sets the bounds to span / 3 and 2 × span / 3
  (span 0 for any other +0xf8).
- The points game's bounds: its constructor `FUN_0012e750` (the base
  `FUN_0013c4e8`, then vtable `0x43b490`) calls `FUN_0012e798`, which
  copies the target (the record's +0xf0) to game+0xf8 and draws
  game+0x4b0 = `rand() % 6` and game+0x4b4 = `15 + rand() % 3`
  (`FUN_003a16b8`, unsigned). The generator is the shared one, seeded at
  boot from the clock (`FUN_003a1790` with hours × 3600 + minutes × 60 +
  seconds + 1) and again in `FUN_001dbe30` (its third argument, or the
  counter at gp−0x7d0c). Both captures (one match) hold 4 and 15. The
  first two draws after a seeding cannot give that pair: `FUN_003a1790`
  adds the seed to all six words, so the first draw grows by 6 per unit
  of seed and the second by 21, and below a wrap they are always 5 and
  16. Other draws come between the seeding and the game's construction.

**In the sim:** `shooting::catch_up_stage` (the larger of the two totals
against the bounds) for all three games; `World::stage_bounds` holds the
points game's pair (snapshot 34) and starts at `STAGE_BOUNDS` (2 and 16),
the rewrite's own: drawn first from the sim's generator they would be 5
and 16 for every match seed, and the two draws would shift every later
one. Gamebreaker Wins now counts the Level 1 bonus (`stats::LEVEL1`). The
computer players' tables 0 and 1 (`ai/skill.rs`) read the same cell.
Against the captures, the larger score's stage differs from the OR's on 4
of the person's 27 released jump shots (6–13, 11–5, 13–6 and 13–7 against
4 and 15: the OR reads 2, the larger 1). With the larger, the release
callback's inputs expect 20.87 ± 1.75 makes, with the OR 20.95, and
without the person's bonuses 12.64 ± 2.27; 22 went in (the
`played_capture` odds report, the captured bounds set on the replayed
world and read through `catch_up_cell`).

**The side fades' jump is game callback 18.** The `83` handler
(`FUN_00195a78`) calls the table at `0x3e86d0` with the actor's channel
and the instruction's three words (`84`, `85` and `87` likewise:
`FUN_00195b80`, `FUN_00195cd0`, `FUN_00195e20`). Only
`jumpshot_sidefade_right_r` and `_right_l` carry `83 12 1` (the only two
`83 12 00 00 00` in the script data); `_left_l` and `_left_r` jump into
their bodies (`82`). Callback 18 is `FUN_00172788` (raw
`0x172788..0x1729f4`; Ghidra stops at its first call to `FUN_0030fa50`,
whose object holds the frame rate at +0x28 as a float and at +0x24 as an
integer):

- the clip on the actor's channel 0 (`FUN_0019c2e0(actor+0xc4, 0)`,
  +0x50);
- the upward velocity 15 / fps (`lui 0x4170` at `0x1727b4`; both branches
  divide the same 15) when the clip is one of the ten at `0x3d91d8`
  (306..311, 326..329) or the first word is 1; with word 0 and another
  clip it is the vector at `0x4b9880` (not traced);
- with word 1, a drift of 7.2 / fps: the heading is the actor's word in
  the game's table at +0x35c turned half round (`FUN_0016f260`), plus
  0x200 for clips 326 and 327, −0x200 for the four at `0x3d9200`
  (316..319), and for the side fades' clips 320..323 −0x100 (320, 322)
  or +0x100 (321, 323); behind the board at the attacked end
  (`FUN_0016f580(actor, 0)`, read from the decompile: |x| over 37 within
  2 of the rim's line, or over 38 within 10) it is 0x300 or 0x100 by the
  rim's side and the drift 1.35 times; vx and vz are −drift times the
  table's sine and cosine (`FUN_0018cd58`, `FUN_0018ccc0`, `0x440ea0`);
- the vector to actor+0x40, then actor+0x64 = `FUN_0019bed8(32.152233 /
  fps² × −1.2596 × 0.5, vy, actor+0x34)` and +0x60 = 0, returning 1: the
  `9f` handler's landing.

So the side fades jump 0.25 at 60 fps and land 45 updates later: at 60
for `right_r` and `left_l` (callback 18 at 15), at 55 for `right_l` and
`left_r` (at 10). The clips at their gates are 320 and 321 (in
`0x3d9218`: the gate opens 15 before the landing) and 322 and 323 (not:
5 before), so the gate opens by itself at 45 (gate 33, the ball gone by
51) and at 50 (gate 28, by 54). The captures' five side fades were let go
sooner (`right_l` at 34 for holds of 30; `right_r` and `left_l` at 39 for
24 and 30), so the landing cap is static evidence.

**The gate's self-opening in the sim.** The sim compared the script's
tick, which stands still at the gate while it waits, with `latest`, so a
person's shot that had begun to wait waited for the button alone.
Callback 19 counts the updates since the jump (actor+0x60 goes on through
the wait), so the gate opens `latest` updates after the press however
long the script has waited: `timing::gate_waits` takes the updates since
the press. No captured shot was held past its cap (the stepback's two
holds of 45 ended on it), which is why the replays agreed before.

**Ported:** `street_assets::scripts::script_gate` reads callback 18's jump
(the constant at `0x1727b4`, the ten clips at `0x3d91d8`);
`timing_tables.rs` gives the side fades `latest` 45, 45, 50 and 50
(`tests/script_timing_disc.rs` walks them and checks callback 18's
constants and the callback table); `timing::gate_waits`; the
played-capture replay puts the five side fades on their captured clock
(44 replays through `World::step`, every one on the captured update).
Tests: `the_side_fades_open_their_gate_by_callback_18s_landing`,
`a_held_shot_leaves_when_its_gate_opens_by_itself`,
`a_held_side_fade_leaves_when_callback_18s_landing_opens_its_gate`,
`the_catch_up_stage_is_the_larger_scaled_score_not_their_or`,
`the_points_game_keeps_its_stage_bounds_for_the_match`,
`gamebreaker_wins_counts_a_level_1_gamebreaker_as_100`,
`the_side_fades_jump_through_callback_18`. Still the rewrite's own: the
points game's bounds (2 and 16), the side fades' drift (the sim's jump
curve has none), and the vector at `0x4b9880`.

## Held side fades and the stage bounds across games (2026-10-16)

Two leads from the last section, settled with new captures of the
original (the project's PCSX2 copy, `tools/pine_played_capture.py`, which
now also reads actor+0xb0, the facing `FUN_0018d290` measures):

- `$EVIDENCE/street-played-fades-1.jsonl`: save state 1 loaded through
  PINE (opcode 0x0A), 23,993 updates (196..24,188, none torn or
  missing), played to 22-12. The keyboard driver walked the person's
  actor to a spot 18..22 from the rim, ran it across the line
  from the rim to it so the facing turned into a side band, pressed
  Circle while running and held it 1.3, 1.6 or 2.0 s (78..120 updates).
  The keys were posted to PCSX2's display window (`WM_KEYDOWN` to its
  Qt `QWindowIcon` child), which PCSX2 takes with another window in
  front, so nothing was typed elsewhere; twice the update counter stood
  still in phase 6 and Start was posted.
- `$EVIDENCE/street-played-fresh-1.jsonl` .. `-9.jsonl`: the opening
  (phase 0, updates 0 or 1) of nine more points games: a rematch from
  the post-game "Do you want to quit?" dialog after that match, seven
  rematches from the pause menu's Quit Game (REMATCH, then the Play To
  dialog's Continue) after 5 to 60 s of the previous game, and a Pick Up
  game after a fresh boot (Main Menu, Play It, Pick Up Game, Got Game,
  User Setup home with no name, All-Stars against All-Stars, Uptown,
  Play Game).

**The held side fades leave at 51 and 54.** The 23 presses of the
side-fade capture played 17 side fades (`right_l` 8, `right_r` 4,
`left_l` 3, `left_r` 2) and 6 side hooks (`jumpshot_hook_1` 4,
`jumpshot_hook_3` 2). Every side fade was held past its self-opening and
the ball left exactly when callback 18's landing opened the gate:
`right_r` and `left_l` at 51 (gate 33 open at 45, + 6), `right_l` and
`left_r` at 54 (gate 28 open at 50, + 4), for holds of 78 to 120. The
hooks have no gate and left at their tables' 29 (`hook_3`) and 32
(`hook_1`) under the same holds. Replayed through `World::step` with the
captured button and stick, the 17 left on the captured update (7 at 51,
10 at 54).

**The side shots follow the facing band and the live hand.** On the
update before each press, the angle `FUN_0018d290` takes, (actor+0xb0 −
atan2(x − rim x, z − rim z) × 162.97) & 0x3ff, and the live hand +0x25c
pick the script exactly as `FUN_0016b4b8` branches: 672..812 with hand 2
`right_l` (8 of 8); 160..300 with hand 1 `left_r` (2 of 2); 160..300
with hand 2 `left_l` or a hook (3 and 3, the 51% draw); 672..812 with
hand 1 `right_r` or a hook (4 and 3). The facing here faces the rim at 0
and away at 512 (the capture's facing runs `0 toward -z, 256 toward -x`,
the direction (−sin, −cos)). Open lead: the sim's `timing::facing_angle`
measures its heading on the mirrored court, which comes to 512 minus the
original's angle (the mirror turns the angle's sense and the sim's
heading points the other way); on these 23 presses that puts 6 outside
their band (702 reads 834, 184 reads 328), and the 400..600 branch and
the runner test read the rim's side and its back the wrong way round.
Not changed here. (Fixed 2026-10-16 in "The chooser's facing angle in
the original's frame": the exact angles are
701 and 183; 702 and 184 are the sim-frame form's, a unit high.)

**The stage bounds are drawn per game.** The ten games hold nine pairs:
[4, 15] (save state 1), then [1, 16], [4, 17], [0, 17], [5, 17],
[3, 15], [5, 15], [5, 17], [5, 16] (the rematches) and [2, 17] (the
fresh boot): every lower bound 0..5 and every upper 15..17, one pair a
match. Where the capture begins at the game's update 0 (seven of the
nine), the shared generator (`0x4309f0`, the sim's `Rng`) holds the
state right after the two draws: stepped back twice and run forward, it
draws `rand() % 6` and `15 + rand() % 3` as the captured pair and lands
on the captured state (the two captures that begin at update 1 hold the
pair two draws further back). So `FUN_0012e798`'s draws are the
generator's two consecutive outputs at the game's construction, from
whatever state the boot, the front end and the earlier games left.

**`World::stage_bounds` stays at [2, 16], as the rewrite's own.** The
evidence rules out any constant as the original's behaviour (no pair
repeats but one in ten games), but drawing the pair from the sim's
generator at `World::new` would not be closer: the sim seeds a new
generator each match (street-game uses seed 2002 unless `--seed`), and
`Rng::new` adds the seed to all six words, so the first two draws are 5
and 16 for every seed below a wrap (the draws that make the original's
spread are the session's, which the rewrite does not carry), and every
seeded stream after them would shift. The pair the sim keeps is the
draws' means. The 1:1 behaviour needs the match's generator to carry the
session's state (seeded once from the clock at boot as `FUN_003a1790`
does, run on through the front end and from match to match, its state
shared with an online peer), with `FUN_0012e798`'s two draws at the
game's construction; `gameplay.core_shot_finish_defense` lists it as a
known difference.

**A late catch.** One catch in the side-fade capture does not keep the
catch rule: a computer's running receiver switched to `catch_stand_1`
only 3 updates before a 25-update pass landed (update 15,381) and held
bit 9 for 15, where the flight's end plus 9 gives 10. Its channel held
the run clip's last frame for 6 updates (a blend) and then played the
catch clip (record 565) from frame 6 to 16 over 12 updates: the script
ran out after the rule's moment. Every other catch in the three played
captures (117) began its script at least 9 updates before the ball
arrived (the standing catches 15..57) and keeps the rule. `played_capture.rs` checks the rule on catches that
began at least 9 updates ahead and only bounds the late ones from below;
the sim's catches do not model a late start (the rewrite's own).

**Tests** (`STREET_PLAYED_CAPTURE` = the two pad captures, the
side-fade capture and the nine openings):
`the_held_side_fades_leave_when_callback_18s_landing_opens_their_gate`
(17 held, both landings at least twice), the replay test (61 replays,
22 side fades, 17 held, all on the captured update),
`the_points_games_draw_their_stage_bounds_from_the_shared_generator_once_a_game`
(one pair a capture, in range, nine pairs, seven drawn from the captured
state through the sim's `Rng`), the catch test (117 on the rule, one
late), the odds test (45 makes of 50 jump shots, 42.06 ± 2.11 expected
with the person's bonuses, 26.56 ± 3.17 without), the default-scripts
test (the side hooks are candidates too).

## The in-game screens: HUD, overlays, banner, pause and dialogs (2026-10-16)

Recovered read-only with `tools/ghidra/RecoverAt.java` (the cut at
`FUN_0030fa50` read through) and the raw ELF; compared with the original's
captures in the project's PCSX2 copy (save states 1 and 2, a played Pick
Up match; 9, the end of the attract match; grabbed with `PrintWindow` and
driven with posted keys, mapped to the layouts' 640x480 as
`tools/pcsx2_screens.py` does). The rewrite: `crates/street-game/src/hud.rs`,
`banner.rs`, `pause.rs`, `look.rs` (legends, the slider), `boards.rs`
(the dialog over the court after the boards), `game.rs` (the pause's input).

**Places.** The in-game screens draw in the layouts' space as the front
end does (the PS2 frame shows layout rows 16..464). The HUD's constructor
moves every element 16 up in a normal match (`FUN_002bb660`, after
deleting the school's ids 0x44..0x56 and 0x23..0x24), so its layout
elements land on the frame where the front end's would 16 lower: in the
rewrite's space they stand on their authored points, while the places the
code computes at run time (the pop-ups' paths, the court intro at 294 and
338, the panel's corner, the banner) are 16 lower than the code's numbers.

**HUD (class 2, `FUN_002ad190`, vtable `0x48d230`).** Draw `FUN_002b12a0`
(its own order), update `FUN_002b3048`, hide-all `FUN_002b38a0` (the
pause), restore `FUN_002b3970`. The atlas is `/FE/HUD.SSH`;
`FUN_002bc0b8(screen, id, tex, size, uv0, uv1)` sets a sprite's size and
corners TL (u0, v0), TR (u1, v0), BR (u1, v1), BL (u0, v1) in n/255;
swapping corner pairs mirrors or turns it.

| ids | size | uv | place | |
|---|---|---|---|---|
| 0x1a/0x1d | 174x34 | (6,127)-(180,161) | (95,38) / (545,38) a2 | team panels, 0x1a mirrored |
| 0x1b/0x1e | 47x34 | (59,57)-(107,91) | (267,37) / (373,37) a2 | score frames |
| 0x1c/0x1f | 45x44 | (5,55)-(52,99) | (53,38) / (587,38) a2 | logo frames |
| 0x14/0x15 | team field 10 | `FUN_00210048(w,1,logo,2/3,0)` | (55,41) / (545,41) | logos |
| 0x38/0x39 | 167x12 | (8,241)-(176,251) | (96,42) / (544,42) a2 | meter target (not when rec+0xe8 = 3) |
| 0x34/0x35 | w x12 | (8,198)-(176,209) | as above | meter fill |
| 0x36/0x37 | 167x12 | (8,227)-(176,238) | | pocketed |
| 0x3a/0x3b | 167x12 | (8,183)-(176,194) | | full, blinking |
| 0x16/0x17 | 113x17 | (5,21)-(118,38) | (178,40) / (462,40) a5 | GAMEBREAKER |
| 0x18/0x19 | 113x17 | (5,0)-(118,17) | same | lit |
| 0x2a/0x2b | 20x13 | (228,10)-(246,24) | (171,55) / (470,55) a2 | slashes |
| 0x26..0x29 | 92x16 | (7,163)-(99,180) | (95,54), (174,54), (545,54) a2, (466,54) a2 | turbo frames |
| 0x2c..0x2f | 84x12 | by pad: red, yellow, blue, green | x 96/179, 544/461, y 56 | turbo bars |
| 0x3c | 43x37 | (194,194)-(238,232) | (50,449) a4 | the replay prompt's rewind |

Turbo (`FUN_002b7498`): each person (pad not 4) on a side takes the
side's first then second slot; a side with nobody shows the first free
pad's bar. Texts: scores font 11 (`FUN_002b7840`, ×0.75 under 100, else
×0.6; a change shows "+N" amber 0xfffbac00 growing from ×0.35 by 0.0117 a
frame to frame 50, then shrinking 0.005, for 2 s, then the total in
0xfff6f4d7); trick points font 1 amber, shadow (2, 2), "%1N" of stat 0xb
(`FUN_002b7eb0`); the shot clock (slot 48, poster `0x13f598`) font 6 ×2
amber, outline 1 0xff040837, the value truncated plus one, shown under 9;
"Shot clock violation!" (slot 52) font 6 outline 1 for 3 s; game point
(0x5906 "score now for win", or 0x5934 "win by two" when game+0x9c = 1)
font 2 amber with shadow: in a points game when both scores are within
one of the target, in a Trick Point Challenge when the side with the ball
leads and has the target; the court intro (`FUN_002b1ef8`,
`FUN_002b1d48`) the court's name font 6 amber outline 2 0xff010635 at
(320, 294), its place font 4 outline 1 at (320, 338), fading in over a
third of a second; the attract line 0x592c "Press START to exit demo"
font 3 0xfffaf59f, shadow (2, 2), alpha falling 3 a frame. The meter
(`FUN_002b6840`, `FUN_002b6d88`, `FUN_002b8168`, `FUN_002b7d08`): target
width meter / 50000 × 167; the fill chases it 0.25 a frame up, 1 down;
full blinks 0x3a with alpha −3 a frame; the lit GAMEBREAKER pulses on a
70-update counter.

**Pop-ups (`FUN_002b87c0`, `FUN_002b4c38`).** Two word and two point
slots a side (side 1 on the left). A word lives fps × 1.5, flying for
fps / 4 from (53, 114) / (588, 114) to (320, 81), ×3 to ×1, holding, then
for the last fps / 4 to the corner (135, 24) / (515, 24), ×1 to ×0; its
points start when the word lands, fps long, from (53, 139) / (588, 139)
to (320, 106) and out to (135, 37) / (515, 37), hidden when "0". A newer
pop-up hurries the older ones to their exits. Font 10, 0xffff7b08
(0xfffbac00 once the team's meter is pocketed). Kinds (word table
`0x48c250`, points `0x48d3b0`): 0 trick name (`0x3fb500`), 1 "*name*"
(the font's '*' is the crown) with the team's trick points, 2/3 long and
short jumpers 750/250, 4 lay-up name (`0x3fb720`) 1000, 5 dunk name
(`0x3fb630`), 6 alley-oop (`0x3fb7a0`), 7 block 2500, 8 steal 1000, 9
"gamebreaker" + level, 10 pick 500, 11 pump fake 500, 12 dive 500, 13
tip slam 2000, 14 busted rim 25000, 15 rebound 500, 16 trick counter
3000, 17 passout (`0x3fb790`), 18 cancel 5000, 20 pocket. The combo
(the scoring block's +0x148): the line of the walked slots' class words
(`0x48de50`) joined " + ", a line break before slot 4's word when it is
the third or later, " super" for six, " defense" when the newest class is over 0x12,
then " combo", font 2 ×0.8 0xfffaf59f, shadow (−1, 2) ×1.2 high, for 4 s;
the pop-up "combo" with the newest slot's value (+0x88) plus the payout
(+0x150). The listener (vtable `0x495b10`, slot k at +8k+4): 13 trick, 15
steal (16 when a counter, `FUN_0016e9b0`), 16 block, 17 rebound, 19
passout, 24 dive, 26 basket, 48/50 shot clock, 52 violation, 56 game
over, 58/59 replay start/end (modes other than 5 and 7), 61 intro, 62
The Opposition (`FUN_00213150`/`FUN_00213858`: "The Opposition:" and the
three players' names, no team name) and Track Info (`FUN_002b2690`), 64
gamebreaker up, 66 meter full, 67 meter drops, 70 pocket, 71 cancel, 73
pump fake (the other side), 74 pick.

**Banner (`0x41`, `FUN_002a9190`; `FUN_002aac68`, `FUN_002ab4b0`,
`FUN_002aaa18`, `FUN_002ac198`).** `/FE/REPLAY.SSH` (512x256); modes 0
replay in, 1 out, 2 and 3 the gamebreakers. Two 640x84 bands turned from
one row of the sheet (replay and level 1: u 278..355 at v 196.8; level 2:
column 504..508, v 9..86) slide in over 11 frames, the top one from y −16
to 63 (bottom-anchored), the bottom one from 464 to 386. Mode 0: two
triangles (334x240, uv (6,6)-(169,124)) sweep from x 638 to −1090; the
logo (174x113, uv (0,143)-(173,256)) flies from (800, −116) to
(518, 349), ×3 to ×0.3, then bounces 0.3 → 0.75 → 0.35 → 0.475 in fps/6,
fps/10, fps/10 (not in attract); then "Replay" (0x5935) font 5 ×0.75
0xff1b6bcc beside the row stars at (520, 420). Modes 2 and 3: the
GAMEBREAKER sprite (77x39) at (320, 240) from ×6 to ×1.7, alpha 0x27 to
0xff, with two stars (146x156, uv (265,17)-(412,174), ×0.2671) at
320 ∓ 0.53 w, y 224; then alpha 255 to 0 while ×1.7 → 2.5 → 1.25. Not
drawn: mode 3's element 9 (`FUN_002a8538`).

**Pause (class 0, `pause.txt`; ctor `FUN_002bc1e0`, build `FUN_002bc3f8`,
input `FUN_002bd038`, values `FUN_002bd8f8`, changes `FUN_002bdc60`,
disabled rows `FUN_002bcd18`, pages `FUN_002bca30`).** Title 0x1c (font 6
at (60, 39)): "Pause", or "Controller Layout" (0x5823) / "Trax"
(0x5821) on their pages. The panel (kind 28 at (60, 85), 518x349) takes
body alpha 0xc0 (`FUN_00211428`; measured: the court keeps 0.33 of its
light under it). Rows by the table at `0x48e250` (node id, label):
Controller Layout, Trax, Camera Type, Speech Volume, Music Volume, SFX
Volume, Replays, Onscreen Help, Visual Effects, Quit Game; each value
widget then its label is registered, so the list's nodes (kind 10, list 3,
by prop 34) put values on x 442 (centred) and labels on x 125 (left), on
the node's middle, y 126, 152, 179, 206, 233, 260, 287, 314, 341, 368;
font 2; labels are row widgets with stars. Choices (`0x48e210`): Camera
Type Street, Court, Slide, Solid, Wide (Behind 0x5813 left out; locked
modes show Wide); Help and Effects Off/On; Replays Automatic (1), Always
(0), Manual (2). Sliders (`0x48e240`, `FUN_0021d148`): the native slider's
art centred on the node (static init `FUN_002be048`: offset (−95, 0),
150x12), filled amber (sheet 2 frames 2 and 3) on the lit row (captures).
Disabled: Street School (rec+0x78 = 1) → Replays, Trax, Help; mode 3,
mode 2 with more than two users, mode 4 with more than one → Camera Type;
game+0xf4 = 9 → Quit Game. Legends: (320, 420) "Continue" (common 20,
action 4, the Start arrow); (451, 420) "Preview" (0x5822) only with the
song list. Start (event 0x3d) closes an open page, else the pause
(`FUN_002bd660(0)`); confirm on Controller Layout opens the controller
panel (`FUN_002144c8` at (70, 105)), on Trax the song list
(`FUN_00218e68` at (10, 10)), on Quit Game the dialog. Circle on the
controller panel goes to the panel (`func_0x002149f8`). The two pages'
drawing is not traced: the rewrite places them by the original's captures
(the controller panel's blue divider at x 319..321 and band at y 388..425
over the pause panel's inside, the pad's picture 256x298 from (68, 136),
the button rows at y 130..362 with the icons at their atlas size; the song
list's Music Experience at y 138 and four songs a page from y 178, 52
apart, title over artist, On/Off at x 505, the lit value between arrows).
Its Triangle reads "N/A" on offence in Layout A, where the front end's
diagram writes "Block / Rebound".

**Dialogs (`FUN_0025c860`; base `FUN_0025beb0`, panel `FUN_0025ca88`,
words `FUN_0025caf8`, rows `FUN_0025cbe8`/`FUN_0025ccd8`, layout
`FUN_0025cda0`, fit `FUN_00212008` via `FUN_0025d000`).** The words (font
`param_5`, 2 for Quit Game) and rows (row widgets with stars), all centred
on the dialog's point: total = words' height + 20 + Σ (row height + 6),
the words' top at −total / 2, the rows 20 below the words and 6 apart.
The panel (`FUN_00211278`, centred) starts empty on the point and grows
to each child's box plus 40 all round; `fit` then makes it 4:3 (the
width to 4/3 of the height if narrower, else the height to 3/4 of the
width). The point `DAT_003f7170` is set at run time; measured (320, 240):
the Quit Game and post-game boxes' middles at (320, 237) in both
captures. A dialog over a screen disables it (vtable +0x114); measured,
the pause screen and the court keep 0.34 of their light. Quit Game
(`FUN_002bd540`): 0x580d "Are you sure you\n want to quit?" over Yes,
No and, in Pick Up (modes 2 and 3), REMATCH (0x580e); No lit
(`FUN_0025c6d8`). Yes quits (`FUN_002bd660(2)`), REMATCH opens the
rematch rules (`FUN_0025dc88`, 0x5df; drawn by the rewrite since
2026-10-20, `rematch_rules.rs`, Continue starting the game again). After
the post-game boards in Pick Up the same
dialog shows "Do you want to quit?" (`common_gm.txt` 11) over REMATCH
(lit), QUIT TO CHOOSE TEAM, QUIT TO MAIN MENU, QUIT TO REWARDS, over the
court with no board behind it.

**Legends (kind 34).** `FUN_0021ab10` sizes the legend to its tallest
piece and puts each piece's middle on the legend's point; the base draw
then moves the pieces by the legend's own anchor over that height. A
middle-anchored legend's pieces therefore sit half a 24-pixel icon above
its point: Pause's Continue at (320, 420) shows its words' middle at 408,
User Setup's Help at (555, 67) at 55 (both captures). `look::legend`
draws them so (top: middle on the point; bottom: a whole height up).

**Rewrite-only text.** The play-by-play line across the top is behind
`--play-by-play` (off by default); the old "REPLAY", "TURBO" and "combo"
texts and the plain pause box are gone. The held pause (a controller
unplugged) and the online pause use the dialog with the rewrite's own
words.

## The chooser's facing angle in the original's frame (2026-10-16)

The lead from "Held side fades and the stage bounds across games",
closed. `FUN_0018d290(actor, at, &fold)`
returns the facing word actor+0xb0 less the direction from `at` to the
actor, `(int)(atan2(x − at x, z − at z) × 162.97466 + 0.5) & 0x3ff`,
masked to 0x3ff, and stores the fold (the angle, or 0x400 less it above
0x200); `FUN_0016b4b8` passes the attacked rim (`FUN_001404e0`'s x, z 0).
The word points (−sin, −cos): the actor's step moves −speed × (sin, cos)
of it (`0x167d88..0x167db4`, `input::native_velocity_step`), and in the
side-fade capture 50,979 of the 54,481 actor updates of phases 4 and 5
moving 0.15 or more an update move within 2 units of the word + 512. So
the angle is 0 facing the rim and 512 facing away.

The sim's frame was the original's mirrored in x (team 0 attacked −x
here, +x there; docs/CAMERA_RECOVERY.md "Side"; the original's own since
"The court's frame", where the word is h + π) and its heading h points
(sin h, cos h): the original's word for the same direction is π − h, and
its x difference is the mirrored one, rim x − x. `timing::facing_angle`
took h less the direction measured in the sim's frame, which is 512 less
the original's angle: 6 of the 23 side presses read outside their band
(701 read 834, 183 read 328), a shooter facing the rim read about 512
(the 400..600 branch; the default three either way while state bit 11
reads clear), and the runner test (fold under 0x65, long range, moving)
passed running away from the rim and failed running at it. The
algebraic form, (sim-frame direction − units(h) + 0x200) & 0x3ff, is a
unit high on 93,632 of the 93,654 actor updates of the capture's phases
4 and 5: negating an angle before the original's rounding (+0.5,
truncated toward zero) moves it a unit. `facing_angle` now takes the
word as the units of π − h wrapped into [0, 2π) and the direction in
the original's frame, `atan2(rim x − x, z − rim z)`; with the capture's
facing word as the heading and its positions mirrored (the rim at
±36.75) it gives the original's angle on all 93,654. The sim's heading
is a float where the original keeps a word turned by its own code; the
word the sim measures is its heading rounded (the rewrite's own), exact
wherever the heading came from a word.

**The chooser on the captured presses.** On the update before each of
the 23 side presses (the shooter's facing word, motion and hand; on the
press update he already stands in the shot), the sim's angle equals the
original's on all 23, and `timing::shot_script` picks `left_r` (2 of 2)
and `right_l` (8 of 8) on each of 64 draws; on the 13 drawn presses
(`left_l` 3, `right_r` 4, the hooks 6) it picks the captured branch's
fade or a side hook, both coming up within the 64. Replayed through
`World::step` with that state (the replay world now carries it where the
capture has the facing word) the sim picks the 10 decided side fades
itself; the replays rise from 61 to 67 (the six hooks join, all on their
captured update: the channel's pc names the hooks' second rows, 24..26,
now matched by script), and the sim's choice differs on 12: the five
side fades of the second pad capture (no facing word, and the shooter
stopped on the press) and 7 drawn presses whose draw came up a hook.
With the old angle the replay picks 7 of the 10 decided.

**Elsewhere.** The computer players' choices move where they shoot
running sideways, facing away or running at the rim from long range.
Over the matches of `shots_turnovers_and_spacing_against_the_captured_original`
the sim's computers picked, on the first attract capture, 145 scripts
(the default three 115, the runner 16, side fades 12, hooks 2) where
they picked 124 (85, 16, 18, 5), and on the second 53 (42, 6, 5, 0)
where 40 (27, 7, 5, 1); the 50-rated match has 58 shot phases and 10
turnovers against the original's 55 and 8 (50 and 6 before), and 17 and
4 against 22 and 0 on the second (19 and 1). The matches run on another
draw sequence; every attract-capture test passes as before.

Lead, not changed: `timing::sector` (`FUN_0018d768`: the pass rows, the
catch rows, the dunk and lay-up approach seen from the rim) also
measures the sim's heading against a sim-frame direction, (units(h) −
direction + 0x200) & 0x3ff. By the same algebra that is the original's
512 − u, so the sectors s and 4 − s trade places (ahead and behind swap,
the sides 2 and 6 stay), unless a caller's arguments make up for it (the
pass reads the bone yaw `FUN_00154230`, the dunk the rim's fixed word
0x300/0x100, which the sim passes as a heading). Not checked against the
captures here. Closed in "The sector in the original's frame".

**Tests**: `timing::tests::the_facing_angle_is_the_originals_on_the_mirrored_court`
(`the_facing_angle_is_the_originals` since "The court's frame")
(the rim at −36.75 and the shooter at −20: facing it 1023, away 511,
toward −z 255, +z 767, headings past ±π);
`tests::the_shot_chooser_reads_the_facing_from_the_rim_in_the_originals_sense`
(through `World::step`, no capture: 690 plays `right_l` with hand 2, 170
`left_r` with hand 1, the runner running at the rim from long range and
the default three running away; the old angle read 845, 341, 513, 0);
`played_capture::the_side_shots_follow_the_originals_facing_band_and_the_live_hand`
(the 23 presses: the angle equal to the original's, 10 decided, 13
drawn with both outcomes); the replay test (67 replays; the decided 10
picked through `World::step`; the choice differs only on a drawn press
or without the facing word). The capture tests fail with the old angle
(796 against 739 on the first press; 7 of 10 decided).

## The session's generator (2026-10-16)

The last section left `World::stage_bounds` at the rewrite's [2, 16]
because the sim seeded a fresh generator every match. This pass traces how
the original keeps its shared generator (`FUN_003a16b8`, the six words at
`0x4309f0`, the sim's `Rng`) across a session, measures it on every
capture the project has, and builds the rewrite's equivalent.

**The boot seeds it from the clock.** `FUN_002f5490` starts timer 0
first: `FUN_003995c0(DAT_004b32e0)` with the word still 0, so the default
100 Hz (the bus clock / 256, 576,000 Hz, over a count of 5,760); it
stores the rate back at `0x4b32e0`, and the save states read 100 there.
Then `FUN_002f5578` reads the console's clock (`FUN_003ad8e8`,
`sceCdReadClock`, its second, minute and hour bytes, binary-coded decimal,
not converted) and seeds both generators with `s + m × 60 + h × 3600 + 1`:
`FUN_003b4290` (the libc LCG `FUN_003b42a0` steps, at
`PTR_DAT_00434284 + 0x58`) and `FUN_003a1790` (this one).

**`FUN_001dbe30` reseeds it from the timer.** The timer's handler
(`FUN_003997f8`) counts `0x4b32e4` (gp − 0x7d0c, gp `0x4baff0`) up once a
tick, hundredths of a second since the boot; `FUN_003998c0` reads it.
`FUN_001dbe30(config, side, seed, team, out)`, which fills a side's
roster for its team, reads the count `c` first and seeds both generators
with `seed` when it is 1 or more, else with `c`; when a seed was given it
reseeds both with `c` again on the way out, so a given seed shapes the
roster pick and nothing after it. `FUN_001dbb20(config, side, team, seed,
flags)` calls it when the team is valid and no controller sits on the
side (`FUN_001db280` counts the four slots at config + 0xd8 by
`FUN_001db308`'s side). Its callers:

| Caller | When | Sides |
|---|---|---|
| `FUN_001db7f8` (`FUN_001dba68` mode 0) | the title's attract match, which `FUN_0029ec08` / `FUN_002d3ed8` alternate with a movie | court `r % 10` and both teams `r % 28` drawn first, then each set |
| `FUN_001db968` (mode 1) | Play It's mode 1 choice (`FUN_002bf428`, item 0x10) | defaults 0xb and 0x10 |
| `FUN_0026fd00` | Choose Team's commit (then class 0x1d) | both; Random League Team (`FUN_0026ef88`, `r % 29`) drawn before |
| `FUN_0026f958` | a one-team Choose Team (then class 0x29) | side 0 |
| `FUN_0026bdc8` | a screen's confirm (then class 0x2b) | side 1 (`DAT_003f7ea8`) |
| `FUN_001e91a0` | a Be A Legend event | side 0 the crew; side 1 with `FUN_001e8790`'s seed, so the count after |
| `FUN_00297c40` (mode 3) | a Pick Up setup | side 0 |
| `FUN_001debb0` | clearing a side (team −1) | none |

So the first reseed after the boot is the first team set for a side no
person holds: the attract match if the title sits idle, else Play It's
defaults or Choose Team's commit for the computer's side. A game with
people on both sides never reseeds at Choose Team. Nothing in a match or
a rematch calls `FUN_001dbb20`.

**Every capture walks back to one reseed of its boot.** Stepped back
draw by draw (`Rng::previous`), a state `FUN_003a1790` set is every word
the seed plus its running increment (`Rng::seeded_by`):

| Capture or save state | Reseed (count) | Draws since |
|---|---|---|
| save state 2 (eeMemory: timer 21,696, rate 100) | 18,061 (180.61 s) | 2,607 |
| save state 1 (timer 112,812, rate 100) | 18,061 | 257,797 |
| `street-played-pads-1`, `-pads-2`, `-fades-1` (save state 1 loaded, update 196/197) | 18,061 | 257,864 |
| end of `-fades-1` (update 24,188) | 18,061 | 456,050 |
| `street-played-fresh-1` .. `-8` (eight rematches in that boot) | 18,061 | 458,830, 462,958, 466,622, 468,355, 474,813, 477,751, 482,142, 482,359 |
| `street-played-fresh-9` (Pick Up after a fresh boot, update 0) | 13,943 (139.43 s) | 48 |
| save state 9 (timer 50,251; the same reseed as the first attract match) | 20,445 | 131,437 |
| `street-native-ai-attract-1` (update 0, update 1) | 20,445 (204.45 s) | 46, 48 |
| `street-native-ai-attract-2` (update 0, 1, 2) | 128,793 (1,287.93 s) | 30, 8,763, 63,613 |

Both save states of the played boot, its three played captures and all
eight games after them run on the one reseed at 180.61 s; both save
states hold the count well past it and the rate 100. Each reseed is a
count a few minutes after a boot, as the menus take. The game, the
post-game, the pause menu's Quit Game and the rematches never reseed: the
generator runs on from game to game. (The walk is
`the_games_of_a_boot_run_on_from_one_reseed_of_the_shared_generator`;
the save states and the attract openings were read offline with the
session's scratch tools.)

**46 draws from the reseed to the game's pair.** The fresh boot's Pick Up
game (Choose Team's commit with All-Stars on the computer's side, then
Choose Players, Uptown, My Rules, the loading) and the first attract
match (`FUN_001db7f8`'s picks, then straight to the loading, update 0
sampled before `FUN_0012e798` ran and update 1 after) both sit 46 draws
past their reseed when the constructor draws its pair (the 47th and 48th:
[2, 17] for the Pick Up game; the attract game's pair is not captured).
The two front ends differ, so the 46 belong to the loading and the
game's construction: the factory `FUN_002f3000` and the base constructor
`FUN_0013c4e8` before `FUN_0012e750` sets the vtable and calls
`FUN_0012e798`. Which calls make them is not traced (by direct calls the
base constructor reaches the actors' `FUN_001563a8` and `FUN_00156ab0`,
`r % 5` each, `FUN_0018b490` and `FUN_0033cf98`; the factory's crowd
object `FUN_0030a9e0` reaches the spectators' heights in `FUN_0030b9b8`),
nor whether every court and mode makes 46: the second attract match's
update 0 was sampled 30 draws past its reseed (mid-construction) and its
update 1 at 8,763, and it drew about 70 an update in its intro where the
first drew none, so its count is not read.

**The factory builds the points game for everything but two modes.**
`FUN_002f3000` makes `FUN_0018e9f0` for config + 0x78 == 1 (which calls
`FUN_0012e750` too), `FUN_0018c4f0` (trick points, `FUN_0018c5c0`'s
thirds of the target) for config + 0xe8 == 1, `FUN_00145720` (Gamebreaker
Wins, `FUN_001457f8`) for 6, and `FUN_0012e750` (the points game) for the
rest. So every game but those two draws the pair.

**The first game after a reseed.** `Rng::new` adds the seed to all six
words, so the low bits mix slowly: with the 46 draws, the lower bound's
parity holds for 17,187 counts (171.87 s) at a time (even for counts
1,137..18,323, the fresh boot's 13,943 among them and its 2 drawn; odd
for 18,324..35,510, where the first attract match's 20,445 falls, a
prediction its capture cannot check). Over whole periods every pair comes
up an eighteenth of the time. Straight after a reseed, without the 46,
the pair would be [5, 16] for every count below a wrap: that is why the
sim's per-match `Rng::new(seed)` could not draw them.

**The rewrite's equivalent.**

- `street_sim::Rng` is public, with its six words (`state`,
  `from_state`), `seeded_by` and, for tests, `previous`;
  `street_sim::rng::boot_seed(h, m, s)` is `FUN_002f5578`'s seed.
- `World::from_generator(hoops, players, rules, rng)` is the game's
  construction on the session's generator: `CONSTRUCTION_DRAWS` (46, the
  rewrite's own count from the two captures above, discarded), then for a
  `Goal::Points` game `FUN_0012e798`'s `% 6` and `15 + % 3` into
  `stage_bounds`. `World::generator` hands the state back.
  `World::new(.., seed)` is unchanged: a world on a fresh generator with
  `STAGE_BOUNDS`, for tests and tools, so no seeded test stream moved.
- `street-game/src/session.rs`: `Session` holds the generator for the
  run. `Session::boot` seeds it with `boot_seed` of the clock in Japan
  time (UTC+9; the console's clock keeps Japan time and `sceCdReadClock`
  reads it so); `--seed N` pins it to `Rng::new(N)`, and scripted runs
  (`--keys`, `--screenshot`, `--events`) to 2002 as before, so the
  comparison tools keep their runs. A pinned session ignores reseeds (they
  read the wall clock). `Session::team_set` reseeds from the hundredths
  since the session began when no person holds the side: at Choose
  Team's commit for each side (`menu.rs`) and when a Be A Legend event,
  an League Challenge match or a Street School lesson sets the computer's
  side. `setup::load_match` builds the match with `from_generator` on the
  session's state; leaving `Screen::Playing` (results, quit, rematch)
  carries the match's generator back (`game::leave`), so the next game,
  a rematch too, draws from where this one ended.
- Snapshots and replays are unchanged: snapshot 34 carries the six words
  and the bounds, and an instant replay re-steps a world clone.
- Online is unchanged on the wire: the host's `MatchSetup` carries its
  whole world (`World::to_snapshot`, the generator's words and the bounds
  with it), the joiner plays `net.world`, so peers agree on both.
  `MatchSetup::seed` stays for the logs and the sound's stand-in. A
  dedicated server's own default match (`MatchSetup::standard`) is still
  `World::new(seed)` and keeps `STAGE_BOUNDS`.

**Still the rewrite's own.** The 46 draws' uses and any per-court count;
the front end's own draws from the shared generator (Choose Court's Random
tile, `FUN_002688f0`, `r % 11` four times a second while the cursor rests
on it, after the commit's reseed; Random League Team, `FUN_0026ef88`, before
it; the attract match's picks), which the rewrite's screens roll on their
own; the match's own draws (the side-fade capture drew 8.3 an update, the
rewrite's sim fewer and in another order), so the state a rematch is built
on is not the original's; the libc LCG that `FUN_001dbe30` reseeds
alongside.

**Tests**: `rng::tests` (the inverse, `seeded_by`, `boot_seed`'s
decimal digits), `a_points_game_built_on_the_sessions_generator_draws_its_bounds_after_the_construction`
(the fresh boot's reseed 13,943 builds [2, 17] and its update 0's words;
the trick-point game skips the pair; the bare-seed world keeps
`STAGE_BOUNDS`; the parity blocks and the eighteenths),
`the_points_games_draw_their_stage_bounds_from_the_shared_generator_once_a_game`
(now through `World::from_generator` on the state 48 draws before each
update-0 capture: the captured pair and words, seven games),
`the_games_of_a_boot_run_on_from_one_reseed_of_the_shared_generator`
(eleven captures on one reseed, nine distances; the fresh boot's on its
own, 48 before update 0), `session::tests` (pinned and clocked sessions,
held sides, a match handing its generator to the rematch).

## The sector in the original's frame (2026-10-16)

The lead from "The chooser's facing angle in the original's frame",
closed: `timing::sector` carried the same mirror error, on all three of
its callers, and the catch measured another line as well.

**`FUN_0018d768(word, from, to, &half)`**, raw `0x18d768..0x18d9ec`: dz =
from z − to z, dx = from x − to x; the polynomial arctangent (dz 0 gives
±π/2, 0 for dx 0; dz below 0 adds π when dx is above 0, else takes π
off); `a = cvt.w.s(angle × 162.97466 + 0.5) & 0x3ff`; `u = (word − a +
0x200) & 0x3ff`; it returns `((u + 0x40) & 0x3ff) >> 7` and stores the
half `((u + 0x80) & 0x3ff) >> 8` (`0x18d9a4..0x18d9ec`; no caller below
reads the half). With an actor's word, which looks along (−sin, −cos) of
itself (the last section), sector 4 is the line from `from` to `to`
running the way he looks and 0 the line running back at him.

**The callers**, read raw:
- The pass press, `FUN_0015f930`: `FUN_0018d768(yaw, passer, receiver)` at
  `0x1604ac`, `0x1604cc` and `0x1605c8`, the yaw `FUN_00154230(passer)`
  (`0x16010c`, kept at sp+0x234: the evaluated root bone's heading, `(angle
  + 0x200) & 0x3ff` of its quaternion's axis, the sense of the facing word,
  which callback 7 sets from the same function); airborne (`0x1603d4..
  0x160404`) the facing word +0xb0 itself. The passer `from` (sp+0x190) is
  his position, plus his velocity times the flight's length in updates when
  he moves (`0x160040..0x1600dc`; the distance band measures from his spot
  a quarter second ahead instead, `0x15fe54..0x15fec8`); the receiver `to`
  (sp+0x1a0) has the catch spot's x and z (`0x1600e0..0x160104`). So 4 is
  the receiver straight ahead: the forward rows carry 0x38, the behind rows
  0x01, 0x03 and 0x81, the side rows sectors 1, 2 or 6, 7 by hand.
- The catch draw at the release, `FUN_001611d8` (`0x161f20..0x161fa8`):
  `FUN_0018d768(word, spot, source)`. The word (sp+0x604) is the receiver's
  +0xb0 (`0x1618e0`) unless a branch turns him first (`FUN_0016f2b0` at
  `0x1619ec`, the angle at `0x161e4c`; both write it into +0xa8 and +0xb0);
  the spot (sp+0x40) is the catch spot; the source (sp+0x30) is the
  passer's position (s5+0x30, `0x161f2c`) or, when game+0x160 is not −2,
  the waypoint game+0x150 (`FUN_00142038` copies game+0x140, +0x150 and
  +0x160 as one record; the flight's two legs at `0x161438`). So the sector
  is the ball's way in against the receiver's facing: 4 a ball coming at
  his face (`catch_stand_1`, `_3l`, `_3r`: 0x10), 0 one from behind. Sector
  0 is split (`0x161f74..0x161fa8`): `FUN_0016f508(receiver, passer)` reads
  the pair table `FUN_0013fed0` builds each update (entry [r][c], game+0x2cc
  + 0x18 r + 4 c, is the direction from c to r: here from the passer to the
  receiver), and the word less it (`FUN_0018cd78`) below 0 gives bit 2,
  else 0x80.
- The dunk and lay-up filter, `FUN_001693b8` (`0x1694e4`):
  `FUN_0018d768(word, rim, shooter)`, the word 0x300 unless the rim's x
  (`FUN_001404e0`) is 0 or more, then 0x100 (`0x1693e0..0x169430`): the
  word faces from the rim out onto the court, so 4 is straight out in front
  and 0 behind the board. With its second argument set (`0x1694fc`) the
  sector is `(FUN_0016f260 − FUN_0016f2b0 + 0x640) & 0x3ff >> 7` instead, the
  alley-oop's path, not carried.

**The sim's reading.** `timing::sector` took the sim's heading less a
sim-frame direction; by the last section's algebra (the word is π − h,
the original's x difference the mirrored one) that is `0x200 − u` of the
original's `u`, for an actor's word and for both rim words alike, so the
sim read the original's sector s as 4 − s: ahead and behind traded places,
the sides 2 and 6 stayed. The pass measured the original's line, swapped;
the drive passed the rim's word as a heading, swapped; the catch measured
the line from the receiver to the catch spot (where a receiver mostly
already stands) instead of the ball's way in.

**The tables read the original's way.** The 37 dunk and lay-up rows with a
partial mask use sectors 2..6 only, none behind the board (0, 1, 7); the
`pass_fwd_*` rows hold 0x38 and the `pass_behind_*` rows 0x01, 0x03 and
0x81; the standing catches that face the pass hold 0x10.

**Measured, outside the sim** (the scratchpad's `sectors.py`: the raw
formula in the original's frame on the captured words and positions,
against the old `timing::sector` on the same state mirrored, the word as
the heading π − word; a row is named by the script the channel's pc runs,
and any row playing that script counts, since the tables carry most
passes twice, one row for each hand):
- 128 passes (the twelve played and both attract captures; rows that read a
  sector, no self passes): with the body's yaw +0xa8 on the update before
  the press, the passer moved on by his motion times the flight and the
  ball's target at the release, the captured row holds the original's
  sector on 104 and the old reading's on 20; 51 of the old reading's
  misses are exactly opposite (256 units from any sector the row holds).
  The facing word +0xb0 of the update before the press holds it on 18 of
  the side-fade capture's 37: on that update a computer passer's facing word
  points straight at the receiver (update 1503: 399, the direction to him
  912 less 512) and the press update puts the body's yaw back; the filter
  reads the root bone.
- 141 catches by receivers on the ground: the receiver's +0xb0 after the
  release (+0xa8 where the capture lacks it), the ball's target, the
  passer's position: the original's reading on 138, the old on 4 (and on 4
  with the frame fixed but the old line, the receiver to the spot). Nearly
  all are standing catches facing the pass (sector 4; 135 of the 141).
- 48 dunks and lay-ups: 30 were two-turbo slot dunks (held 0xe080,
  0x1e080, 0x36080 ...: the slot's row whatever the approach, `dunk_3` and
  `dunk_4` every time), and the 18 the filter chose all played rows for any
  approach (0xff), so the captures cannot pin the approach sector through a
  row. Their approaches read 2..6 in the original's reading, in front of
  the rim (22 at 4, 19 at 3 or 5); the old reading put those 41 at 0, 1 and
  7, behind the board.

**Through the sim's filters** (`crate::row_capture`, shared by
`played_capture::the_pass_catch_and_dunk_rows_read_the_originals_sectors`
and `ai::capture::the_pass_catch_and_dunk_rows_read_the_originals_sectors`:
each captured case rebuilt as a World, x mirrored (as captured since
"The court's frame"), the passer's heading
the body's +0xa8, the receiver's his +0xb0 after the release where
captured, motion and hand as captured; a row agrees when its mask holds
the sector the filter reads, and is a candidate when the filter lets it
through), before (the old `sector`) and after:

| capture | passes: sector | passes: candidate | catches: sector | catches: candidate | dunks, lay-ups |
|---|---|---|---|---|---|
| the twelve played | 7 → 72 of 98 | 3 → 32 | 3 → 110 of 112 | 1 → 108 | 16 of 16 both |
| attract 1 | 6 → 77 of 98 | 1 → 27 | 9 → 81 of 92 | 3 → 43 | 5 → 6 of 6 |
| attract 2 | 3 → 20 of 30 | 1 → 2 | 1 → 28 of 29 | 0 → 15 | 2 of 2 both |

The passes still missing their sector carry the stand-ins: the body's yaw
for the root bone's, the passer a quarter second ahead, the receiver where
he stands. The played passes whose sector agrees but whose row is no
candidate fail elsewhere in the filter: the standing and running rows (27:
mostly computer passers near the baseline, where the sim's
"running out of bounds" test at |x| ≥ 32 flips them), the distance band
(6) and the computer's kick rows by rating (5). The attract capture has no
velocity word: its motion is the position step, so a receiver settling at
the spot reads as moving and the standing catch rows (0x4000) are barred.
The played dunks and lay-ups agree before and after for the reason above;
the first attract capture's one change is `layup_barry` (0x38, a legend's
row the sim does not carry, so no candidate either way), 0 in the old
reading and 4 now.

**Changed** (`timing.rs`): `facing_word` (also `facing_angle`'s word);
`sector(word, from, to)` in the original's frame; `pass_sector` (the
passer's facing word for the yaw), `catch_bit` (the receiver's facing word,
the line from the passer to the spot, sector 0 split by the direction from
the passer to the receiver), `approach_sector` (0x100 for the sim's rim at
−x, 0x300 at +x); `catch_row` takes the passer; the three filters are split
into `pass_candidates`, `catch_candidates` and `move_candidates`
(`MoveGates`: the Dunks-rating draws in their old order) so the capture
tests ask the filter itself. No serialized state changed, and none of the
workspace's existing tests needed a change.

**Tests**: `timing::tests::the_sector_is_fun_0018d768s_on_the_originals_court`
(a sweep of words and lines against the raw formula; five captured cases:
3730's `pass_fwd_2` 4, 14166's `pass_behind_1` 1, the catches of 395 and
2369 4, the drive of 1654 4; the old reading's 0 on the first);
`the_filters_read_the_sectors_on_the_sims_court` (the pass 4, 0, 2, 6; the
catch 0x10 and the split; drives at both rims; no partial-mask move row
behind the board); through `World::step`,
`tests::a_pass_ahead_plays_a_forward_row_and_its_catch_faces_the_ball` and
`tests::a_drive_from_out_front_draws_among_the_rows_for_the_front_of_the_rim`;
the two capture tests above (their bounds sit between the before and after
counts). The three tests with fixed cases fail when the helpers are put
back on the old sector.

Not done: the alley-oop's approach path, the waypoint passes' source, the
turned receiver's word, the passer's flight-length lead and catch spot,
the standing and running rows' boundary test.

## Pass flights and the tip-off (2026-10-16)

Traced in the raw ELF (`tools/mipsx.py`; Ghidra cuts `FUN_001611d8`,
`FUN_0015f930`, `FUN_001636c0` and `FUN_00194d58` at the settings getter
`FUN_0030fa50` or a bad-instruction stop, so their tails are from the
disassembly) and measured on both attract captures and the twelve played
captures. Corrects the 2026-10-16 audit's items 1.3 and 1.4 where they
differ.

**The two constants.** Both live in BSS and are written once at boot by
the gameplay constants' initializer (`0x178800..`, the frame rate read
from the settings object's +0x28):

- `DAT_004b99c0`, the pass speed (`0x178bf4..0x178c34`):
  `5280 / (fps × 3600) × 30.762` (`0x45a50000`, `0x45610000`,
  `0x41f61893`): 30.762 mph had a unit been a foot, 0.75196 units an
  update at 60 fps (45.1 a second).
- `DAT_004b99c4`, the shortest flight (`0x178c38..0x178c74`):
  `fps / 60 × 20` (`$f22` is 60.0, set at `0x178ba0`): 20 updates.

**The release.** A pass row's script calls game callback 38,
`FUN_00173570`. Every one of the 71 pass-table scripts that calls it (and
46 other pass scripts) passes a second argument of 0; only
`passout_2_fail` and `wrap_reaction_l` pass 1, and that branch throws the
ball toward the backboard (side × 38, 12, `rand × 0.793225`) with the
random lob `FUN_0014c408(0.075 + rand × 0.075, ..)`: the audit's lob is
the failed pass-out's, not an ordinary pass's. With 0:

- The passer's teammates' catch bar (+0x1a8) is set to the flight
  (`DAT_003d65a0`), the receiver's to 0; on a plain pass from the floor
  to a receiver 15 or more away, the opponents' to `trunc(0.35 ×
  flight)` (`0x1738c0..0x173918`): no interception in the first 35% (the
  rewrite's interception keeps its own last-quarter test).
- The catch draw runs: `FUN_001611d8(passer)`, or `FUN_00162aa8` for a
  receiver in the air with actor+0xa4 `0x8000` (an alley-oop leaper).
- A plain pass (game+0x160 == -2, no trick leg) is launched by
  `FUN_0014c0c0(ball, &DAT_003d6590, DAT_003d65a0, DAT_003d65a4)`:
  ball+0x90 the point, velocity `(point - ball) / n + (0, ½ g n, 0)` (g =
  ball+0x118, 0.01125), ball+0x64 = n, +0x60 = 0, +0x160 = 2. The ball's
  step (`FUN_0014d1f8`: position plus velocity, half the gravity off the
  height, all of it off the rise) lands it on the point after exactly n
  updates. `DAT_003d65a4` = 1 is the bounce pass (`FUN_0014f728`). A
  trick pass chains legs through `FUN_001488e8` (the path at game+0x134):
  its first leg takes `DAT_003d65a0 × max(d1/S × k, 2) / (max(d1/S +
  d2/(0.75 S), 20) × k)`, k = 0.86956525 (1.0526316 for kind 2), the
  second the rest.

**The flight** (`FUN_001611d8`, `0x16138c..0x161848`, receiver `$s2`,
passer `$s5`):

- The distance: a led pass (`DAT_003d75c4` = 1) reads the game's distance
  table, `FUN_0016e948(passer, receiver)` = game+0x1ac + 0x18 × passer
  slot + 4 × receiver slot, which `FUN_0013fbc8` fills from
  `FUN_0018cdb0` (horizontal distance) before the actors update; a spot
  pass measures the passer to the spot (`DAT_003d70c0`/`c4`).
- `f = max(d / DAT_004b99c0, DAT_004b99c4)`.
- A spot pass to a moving receiver (+0x190 not 0) is looked at again
  (`0x1614e8..0x161824`): with the passer in his front court and the spot
  within 12 of the rim the receiver's side attacks, it leads him if
  `f <= DAT_003d9a28` (his updates to the spot, from the press); else,
  with `f <= reach + trunc(fps × 0.12)` (reach recomputed from where he
  is), it leads him unless both are in the passer's front court and the
  spot is nearer the centre line than he is and within 13 of it, when
  `f += trunc(fps × 0.09)` (5) if he still gets there by then (else it
  leads him). Still a spot pass, a spot more than 50 of 1024 off his
  facing (`FUN_0018d290`) leads him.
- `DAT_003d65b8` set: `f = 40.0` (`0x161828..0x16183c`).
- receiver+0x1b0 = `cvt.w.s f` (truncated, `0x161848`); for a receiver in
  the air, the point's time is capped by his airtime left (+0x64 - +0x60);
  the point is `FUN_001636c0(receiver, .., n)` when led (his place plus
  his velocity times n; a run that would leave ±40.5 or ±25 turns along
  that edge at his speed for the rest), else the spot; the catch row is
  drawn (its filter compares the row's own count with the *passer's*
  +0x1b0, `lw 0x1b0($s5)` at `0x162028`, and admits only `0x20000` rows,
  `catch_stand_3r`/`3l`, while `DAT_003d65b8` is set; the flag is cleared
  at `0x1622c0`); `DAT_003d65a0` = receiver+0x1b0 (`0x162330`); the row
  poses the receiver there and the point becomes his hand (bone 0x13 or
  0xf, or their midpoint by flags `0x400`/`0x100`) in `DAT_003d6590`; a
  row with its own count (+4 > 0) then replaces receiver+0x1b0
  (`0x162a4c`), not the ball's flight.
- `FUN_00162aa8` (the leaper): `f = max(table / S, 20)`, raised to the
  leaper's current span less 5 (channel +0x6c - +0x70 - 5).

**The aim at the press** (`FUN_0015f930`, `0x15fc2c..0x15fe48`; `$s3`
the passer, `$s5` the receiver): the spot is `FUN_0016c970(receiver)`
unless he is a leaper (+0xa4 `0x8000`), the passer's record is in the
six-record list `0x43f870`, he is the passer, a trick leg is set or his
+0xa4 bit 11 is: `FUN_0013b900` gives his landing point in the air (place
plus velocity times the airtime left), else his front behaviour's
destination (`FUN_00133380`: the front node's vt+0x24, which for every
behaviour but "fast break" (`FUN_0010aea8`) and "post cut"
(`FUN_00112f48`) is `FUN_00100768`, its target vt+0x1c; nothing changes
his place when it has none), kept within ±40 by ±24.5 on the floor.
`DAT_003d9a28` = his distance to it over his speed (a receiver standing
still divides by zero: the EE's largest float). With `DAT_003d9a28 +
trunc(fps × 0.12) <= trunc(fps / 2)` (23 updates) the pass goes to the
spot, unless the passer is no farther from it than he is
(`FUN_0018cdb0`); else it leads him. The press also estimates
`DAT_003d65a0` (passer a quarter second ahead to the spot, or to the
receiver 25 updates ahead when led) for the row filter.

**The catch.** The receiver's update (`FUN_0015da28` → `FUN_0015c598`)
takes the ball when `FUN_00137db8` finds the ball's last move, swept in
quarters, touching one of three bone-sphere groups (`PTR_DAT_003d10a0`):
the catch row's hands meet it 0..5 updates before it would arrive. In the
first attract capture (ordinary passes), flight minus updates flown:
0 (3), 1 (66: taken in the update it arrives), 2 (58), 3 (14), 4 (12),
5 (2); no radius about the point reproduces it (a sphere of 0.75 to 1.25
gets 43..66 of 163 exact). The rewrite takes it as it arrives (1).
(Corrected and ported since: the catch is the offence's update
`FUN_0015a060`, not `FUN_0015c598`, the dive's; "The ball on the bones".)

**Measured: every pass flight in the captures** (`ai::capture::
pass_flights_are_timed_by_the_originals_rule`, `played_capture::
the_persons_and_the_computers_pass_flights_are_timed_by_the_originals_rule`):
ball+0x64 when the ball leaves the hands, against the rewrite's release
(`World::throw` by a passer and a standing receiver where the capture had
them as the release update began: a standing receiver is led) and the
rule on the receiver's place when the press ran (the press's sample if
his actor updates before the passer's, else the one before).

| captures | flights | tips (40) | ordinary | led exact | spot exact | other | to oneself | to a leaper |
|---|---|---|---|---|---|---|---|---|
| attract 1 | 166 | 1 | 150 | 123 | 17 | 10 | 9 | 6 |
| attract 2 | 61 | 1 | 52 | 46 | 4 | 2 | 6 | 2 |
| played (12, three with play) | 195 | 3 | 184 | 163 | 16 | 5 | 4 | 4 |

369 of 386 ordinary flights (95.6%) are the rewrite's exactly; the other
17 lie within 5 (12 within 1 of the led rule): a running receiver's
destination is not in the captures, and one is the back-court cut's 5
(20 + 5 = 25 for a 20-update pass). The trick passes (to oneself) and the
lobs to leapers keep the rewrite's flights.

**The tip-off** (`FUN_0015eaa0`). Once the contest is decided the winner
starts `tipoff_jump_2` (`0x457a7b`, record 380, `0x15edc4`) and the loser
`tipoff_jump_1` (`0x457a12`, record 384) or `tipoff_jump_3` (`0x457ae4`,
record 382) as `r & 1` is 0 or 1 (the pair at `0x43f078`, `0x15ede8..
0x15ee10`); the loser's catch bar is the toss's updates left (+0x64 -
+0x60, `0x15ee20`). Each script blends in 3 updates, waits 5 (to frame 4,
3 or 2), then at its tick 8 runs `a1 ef 25.0 F -1.0` (F = 15, 10, 11),
waits to frame F over 25 updates, waits to frame 25 (23, 24) until he
lands (`8d ee .. fe`), plays out at 2.0 and calls 7 and 1.

`a1 ef` (`FUN_00194d58`, `0x19537c..0x1956a4`, then `0x195728..
0x195784`) is an aimed jump, not a height: the operands are 25 updates
(× fps / 60) and a clip frame. It turns the pose to the team's heading
(`0x300 / 2` or `0x100 / 2`; the actor's own with a third operand of 0 or
more), poses the channel's record at frame F (`FUN_0019bda8`), takes bone
0x13 or 0xf or their midpoint (pose flags 8 / 4), takes the ball's place
25 updates on from its own predicted path (`FUN_001537d0`: the ring at
ball+0xf0 that `FUN_001538f8` fills by stepping `FUN_0014d1f8`; entry
+0x164 - 1 + n is n steps on), and sets the actor's velocity to that
place less the hand's over 25, plus `½ × 32.152233 / fps² × 1.2596 × 25`
upward; actor+0x64 = `FUN_0019bed8` (the airtime), +0x60 = 0. The audit
read 15.0, 10.0 and 11.0 as heights: they are frames. Captured: the
contest in update 170, both jumpers off the floor in 178, the winners'
peaks 3.75 and 3.90 at 203, 25 updates on (the losers' 3.85 and 4.23),
landing about 229.

The tip (`0x15ee28..`): the winner, his catch bar out, the referee's
+0x70 clear and +0x190 1, nobody holding the ball and his channel's flag
`0x40` clear, holds the ball once bone 0x13 or 0xf is within 1.25
(`0x3fa00000`, `0x15eec4`) of the ball where the last update left it
(`FUN_0014cb08`), and `FUN_0015f930(actor, 0)` passes it to actor+0x74
at once. It is the first pass since the actors' reset (`FUN_00156870`
sets `DAT_003d65b8`, `0x1569a4`; inbounds fly 20, so the reset runs once
a match): 40 updates. Captured: the tip in update 201 (200 in the second
attract opening), ball+0x64 40 in all five captures that hold the
opening, taken 38 updates later in all five.

**Ported** (`passing.rs`, `lib.rs`, `player.rs`, `timing.rs`; snapshot
36):

- `PASS_SPEED`, `PASS_MIN_FLIGHT`, `FIRST_PASS_FLIGHT`,
  `flight_updates(distance)`.
- `World::pass_aim` at the press (spot and lead, `PassAim`, carried in
  the `Windup`), `World::release_flight` at the release (the led
  distance from the players' places as the update began,
  `World::floor_positions`; the re-evaluation; the first-pass flag),
  `World::lead_spot` (`FUN_001636c0`), and `World::launch` throwing to
  the point in exactly that many updates (`Ball::throw` with n / 60 s:
  the same velocity and step). Plain, give-and-go and kick passes,
  handoffs and inbounds all go this way.
- `catch_row` takes the passer's `received_flight` (actor+0x1b0, which
  each release sets on the receiver: the flight, or the row's own count)
  and the first-pass flag.
- `Tip::first_pass`, set by `World::start_opening_toss`.
- `Action::TipJump { script, tick, vy, drift, up }`: both jumpers start
  their scripts at the contest (`toss::TIP_SCRIPTS`), jump at tick 8
  (`World::tip_jump`: the ball's free flight 25 updates on, the velocity
  above, `toss::JUMP_GRAVITY`) and land on their own; the winner tips when
  his hand is within `toss::TIP_REACH` (1.25) of the ball, and the tip is
  `World::launch` from the ball's place, a first pass of 40, the winner
  staying in his script.
- street-game: `channel::scripted` shows the tip scripts on the script
  clock, their airtime wait (`fe`) lasting until the sim's jump lands
  (`playback::Clock::landing`, `timing::airtime`).

**The rewrite's own (labelled in the code).** The hand: no bones, so the
aimed and checked hand is the jumper's reach above his feet
(`Ratings::reach`), carried rigidly on the jump. The jumpers then peak
lower than original's: 3.38 in the opening test's world (the reach of
a 6'6" player, 10.2; the captured winners' frame-15 hands sit about 9.7
above their feet, and their peaks are 3.75 to 4.23), and the tip comes in
update 200 in both openings (201 and 200 captured): within one. The
receiver takes a pass as it arrives (the original's bone spheres take
the tip one update sooner: 39 after the tip against 38). The point's
height is the rewrite's release height, not the catch row's posed hands.
The spot of an airborne receiver is his place; "fast break" and "post
cut" give their targets; the `0x43f870` list and +0xa4 bit 11 read as
clear. The leaper's lob and the trick passes keep the rewrite's timing
(`FUN_00162aa8` needs the leap script's span; the legs of
`FUN_001488e8` are not ported). The opponents' first 35% is not carried
(the rewrite's interception is its own last-quarter test).

**Tests**: `passing::tests::the_flight_is_the_distance_over_the_pass_speed_and_never_under_20_updates`,
`a_pass_flies_the_originals_count_of_updates_for_its_distance` (a
person's pass through `World::step`, four distances),
`the_first_pass_after_the_opening_reset_flies_40_updates_and_only_it`,
`a_receiver_running_to_a_near_spot_is_passed_to_the_spot`,
`a_led_receiver_is_met_where_his_run_takes_him_turning_along_an_edge`;
`tests::the_jumpers_jump_and_the_winner_tips_the_toss_to_the_man_behind_him`
(scripts, heights, the hand within 1.25, the 40-update tip, caught 39
after); the capture tests above and
`ai::capture::the_opening_toss_and_tip_follow_the_captured_original`
(now also the tip pass's 40 against ball+0x64, the catch 38 after the
tip within one, the jump peaks reported).

## The game's phase machine (traced 2026-10-16)

The game object keeps the phase of play at game+0xf4 and changes it in its
own code; the computer players, the restarts, the meters and the shot clock
read it. Read from the decomp and checked against the raw ELF (the phase
stores found by scanning for `sw rX, 0xf4(rY)` after a `li rX, k`: `0x12775c`
1, `0x13dbc0` 9, `0x13df40` 8, `0x13e634` 2, `0x13e6c8` 4, `0x13e740` 5,
`0x13eee8`/`0x13eefc` 4/5, `0x1406d4` 6, `0x1415b4` 3, `0x14166c` 4,
`0x141ff8` 1, `0x13cfe0`'s 0). Port: `crates/street-sim/src/game_phase.rs`.

**The phases.** 0 constructed (`FUN_0013cf58`, the intro playing), 1 the
referee's toss (`FUN_00141fd8`, vt+0x15c), 2 a loose ball, 3 a shot (or
lay-up or dunk) in the air and its rebound, 4 the ball in the possessing
side's back court, 5 in its front court, 6 a restart (`FUN_001406b8`,
vt+0x14c), 8 over by vt+0x6c, 9 won. Live play (`FUN_00140558`) is phases
2..5 (`0x3d30d0`: 0, 0, 1, 1, 1, 1, 0, 0, 0, 0) with game+0x19c clear (the
shot clock's wait for a shot to finish).

**The six game classes** (vtables `0x43b490` the points game, `0x43dcf8`
the base, `0x43e9d0`, `0x440240`, `0x440ce8`, `0x441680`) share every
phase method: vt+0x34 the update `FUN_0013d868`, +0x13c `FUN_001405f0`,
+0x144 `FUN_00140690`, +0x14c `FUN_001406b8`, +0x134 `FUN_00140880`, +0x164
`FUN_0013ecd8`. vt+0x6c is `0x143060` (returns 0) in all six, so phase 8 is
never reached in Vol. 2: a won game goes to 9. vt+0xbc (won) is the
class's own (the points game's `FUN_0012e800`: either score at the target
and the margin 2 or more). vt+0x15c (the tip's start) is `FUN_00141fd8` in
four classes and `0x1860d0`, `0x18f468` in `0x440240` and `0x441680` (not
traced).

**The ball's moments reach the game at once.** `FUN_00190d90` calls every
listener of the poster `game+0x124` in turn, so a phase change posted by
the ball happens inside the ball's code. The event classes are 16-byte
descriptors at `0x3c64c0..0x3c6950` whose fourth word is a dispatcher that
calls listener slot k (`lw v1, 8k+4(vtable)`; the audio listener's table
above uses the same numbering). The game's listener (vtable `0x43da50`,
+8 the game) handles:

| Slot | Class | Handler | Moment | Phase |
|---|---|---|---|---|
| 11 | `0x3c6530` | `0x141558` | the launcher `FUN_0014aaf8` lets the ball go (jump shot, lay-up and dunk: `0x14bf00..0x14bf80` posts slot 12 `0x3c6540` for a dunk, then slot 11 for every launch) | 3, unless phase 8/9 or the script manager's states 3..7 |
| 12 | `0x3c6540` | `0x132c00` (empty) | a dunk's release | — |
| 24 | `0x3c6600` | `FUN_001415c0` | the ball given to a player (`FUN_0014cb28`, every catch, steal, pickup, rebound, the tip's take and the restart's hand-over; posted after the ball's state is 3 and +0xf4 the holder) | phase 1 → 4 (game+0x1a8 the other side; the referee lets go); then `FUN_001405f0` |
| 26 | `0x3c6620` | `FUN_00141818` | a basket | vt+0x14c for the other side (phase 6) unless the class's vt+0x7c says the basket wins; a gamebreaker's basket (the event's level and the ball's +0x178) that does not win sets game+0x104 = fps × 2 + `DAT_004b79f4` (fps × 2) = 240 (`0x1419a4..0x141a38`) |
| 30 | `0x3c6660` | `FUN_00141d48` | the ball on the floor | game+0x18c (the clock's flag) by the side |
| 38 | `0x3c66e0` | `FUN_00142270` | (no code builds the class) | vt+0x14c for the side that did not touch it last |
| 52 | `0x3c67b0` | `FUN_00141418` | a shot-clock violation | the holder's +0x364 |
| 54 | `0x3c67d0` | `FUN_001420e8` | (no code builds the class) | the formation restart |

**`FUN_001405f0` and `FUN_0013ee10`: possession and the court.** Outside
phase 6 and the shot clock's waits (game+0x198, +0x19c, +0x1a4), the
holder's side takes possession through `FUN_00140690` (vt+0x144): a new
side (game+0x110) goes to `FUN_0013ee10`, which (unless game+0x110 is -1,
the phase is 0, 8 or 9, or a script runs) sets phase 5 when the holder's x
(the ball's without a holder) is in the half the side attacks (game+0x98 +
side: 1 attacks +x, -1 -x; x = 0 counts as +x), else 4.

**The update, `FUN_0013d868`,** in this order: the controllers' checks;
return while the AI script handler runs (`FUN_0013e358`); game+0x104 counts
down outside the script manager's states (at 0, slot 69); the update count
game+0x128; the end of the game (phase 9 stays; else if vt+0xbc, no script
and vt+0x164 `FUN_0013ecd8` (the ball not passed, shot or on the rim and its
holder not in a shot, dunk, lay-up or pass): phase 9, game+0x448 =
`DAT_004b79fc` = fps × 2.5 = 150 (`0x142f84..0x142fa4`), vt+0x134(1));
vt+0x13c; `FUN_0013e4c8`; vt+0x104 the shot clock `FUN_0013f1f0`; vt+0x114,
+0x11c, +0x124, +0x16c, +0xdc, +0x12c; `FUN_0013e378`; the teammates' slot
swap; vt+0x174 the meters `FUN_0013eff8`; game+0x90; the paint counts
game+0x470 outside phase 3; game+0x488; game+0x490.

**`FUN_0013e4c8`, the phase's own update.** First, in live play outside
phase 2: a loose ball (ball state 1), or a rebound (state 0) below 6, is
phase 2. Then (game+0x9c 5, paused, returns):

- 0: game+0x9c 4 starts the tip (vt+0x15c); else the AI script handler.
- 1: the toss in the air (state 7) frees the players (`FUN_00140880(0)`);
  a loose ball is phase 2.
- 2, 3: with a holder, `FUN_0013ee10`.
- 4: the ball's x (not the holder's) in the side's front court: phase 5,
  game+0x4a4 set, slot 23 `0x3c65f0` posted; otherwise game+0x4a4 set once
  anyone holds the ball.
- 6: `FUN_00140840` (game+0x104 clear and the six ready flags
  game+0x404..0x418 up) and nobody holding the ball (the inbound pass in
  the air): slot 28 `0x3c6640`, phase 4, `FUN_00140880(0)`.
- 8, 9: the end (the score sum, vt+0x134(1), slot 55, game+0xd8/+0xdc);
  phase 9 counts game+0x448 down first.

Last, game+0x440 counts the updates of phases 2 and 3.

**The restart** (`FUN_001406b8`): phase 6, game+0x110 the side to inbound,
game+0x4a4 and the ready flags cleared, `FUN_00140880(1)`, the other side's
ball let go, the turbo topped up (see the restart above). Reached from the
basket (slot 26) and the shot clock (`FUN_0013f1f0`): a violation with the
holder free (the clock run out, `0x3c6780` slot 49, then slot 52 and the
restart for the other side), one in a shot waits for it (+0x1a4), one with
nobody holding the ball waits for a holder or a loose ball under 12
(+0x198), and one whose holder is in an animation `FUN_00154680` waits for
it (+0x19c, which also stops live play).

**What reads it.** The AI teams' strategy lists are indexed by the phase
(`0x18a8f8`, `ai/team.rs`: 1 tip, 2 scramble, 3 rebound, 4 fast break or
defence, 5 playbook or defence, 6 inbound, 7, 8, 9 the end's); the AI
player's decide (`FUN_0017e5c8`) by it; "wait for toss" (`FUN_0011af98`)
until it leaves 1, "wait for hc" (`0x437898`) until it leaves 4; the
behaviours' live tests (`FUN_00140558`); the restart's strategies and
ready flags (phase 6); the meters (game+0x4a4, below); the paint counts.

**Measured in the attract captures** (every change of the captured phase
with the ball's events between the two updates; first capture 40,887
updates, second 14,156):

| Change | First | Second | The ball before it |
|---|---|---|---|
| 0 → 1 | 1 | 1 | the intro's end |
| 1 → 4 | 1 | 1 | the tip: state 7 to 4 in one update (taken and passed), possession -1 to the taker's side, in its back court |
| 2 → 4, 2 → 5 | 7, 11 | 3, 3 | a loose ball picked up (state 1 to 3), by the court it is in (first capture: the possession changing in 6 of the 7 and 2 of the 11) |
| 3 → 2 | 4 | 2 | 3 blocks (a shot knocked to state 1 at 15 up) and 1 rebound below 6; second: 1 and 1 |
| 3 → 4, 3 → 5 | 7, 6 | 1, 2 | a rebound caught off the rim in the air (state 0 to 3), possession changing in all 8 to phase 4 |
| 3 → 6 | 37 | 17 | a basket (5 to 1 at the ring), the other side to inbound |
| 3 → 9 | 1 | 0 | the winning basket; phase 9 the update after |
| 4 → 2, 5 → 2 | 2, 12 | 0, 4 | the ball knocked loose or a pass gone loose |
| 4 → 5 | 58 | 22 | the ball over half court (40 held, 18 a pass; second 10 and 12) |
| 5 → 3 | 55 | 22 | a launch (state 3 to 5) |
| 5 → 4 | 6 | 0 | 4 steals from the hands, 2 interceptions, all in the taker's back court |
| 5 → 6 | 2 | 0 | two gamebreaker baskets (below) |
| 6 → 4 | 39 | 17 | the inbound pass leaving the hands |

So every turnover out of phase 5 brings the ball up in phase 4: the 8 the
audit counted (`docs/AUDIT_1TO1_2026-10-16.md` 1.2, 1.5) are 6 turnovers and
2 gamebreaker baskets, not shot-clock violations. Those two (updates 26,500
and 31,102) are staged Level 2 shots: the ball flies 69 and 80 updates in
state 5 with the gamebreaker's camera, no launch reaches the game (the
script manager is in its states 3..7, so slot 11 does nothing) and the
phase stays 5 until the basket's restart, with game+0x104 = 240 holding
the restart.

**The update's order, from the captures.** A loose ball (19 of 19), a
pickup (23 of 23), a launch (74 of 76; the other 2 across gaps in the
samples) and the ball over half court (74 of 80) change the phase in the
update the ball does:
the game's update runs after the players and the ball have moved. The
inbound's release ends phase 6 in its update in 45 of 56 (the ball already
one update into its flight; the rest an update later). A basket's phase 6
shows in the update the ball drops through the ring, but the winning
basket's phase 9 shows one update later, with the phase still 3 and the
ball loose in the basket's update: the hoop posts the basket, and the net
catches the ball, after the game's update.

**The port.** `game_phase::GamePhase` holds game+0xf4, +0x110, +0x440,
+0x104 and +0x448 in the world (`World::game_phase`, snapshot 36);
`World::phase` moves with it (phase 6 is `Phase::DeadBall`, 9 follows
`Phase::Over`). Fed as the original feeds it:

- `World::take_ball` posts slot 24 (`gained`): the tip's take, a turnover,
  a pickup;
- the launcher (`launch_shot`, and the slams' and alley-oops' release)
  posts slot 11 (`launched`); a staged sequence's shot does not;
- `World::start_restart` (and the rewrite's timed dead ball) is
  `FUN_001406b8`; a gamebreaker's basket that does not win sets the hold;
- `World::start_opening_toss` is `FUN_00141fd8`;
- once an update, after the ball and before the shot clock and the meters
  (`World::step_game_phase`), `FUN_0013d868`'s parts: the hold, the end of
  the game (won by a basket of an earlier update), vt+0x13c and
  `FUN_0013e4c8`. Its phase 6 → 4 ends the world's restart
  (`World::end_restart`) and its case 4 sets game+0x4a4 (`MeterClock::On`;
  the earlier catch-time guess, `clock_meters_at_catch`, is gone).

The ball's state as the game reads it is `World::native_ball_state`: a
pass from its press, the referee's toss (7), a shot that has touched the
hoop (6, `Ball::touched`) and its rebound once its timer runs out (0,
`Ball::rebound`: cleared by the floor, a hold, a throw or a launch) apart
from a ball let go, knocked away or on the floor (1) ("Dunks and
rebounds on the ball's states"; before, a missed shot's end was 6 above
the rim and 0 below). The computer players read the machine
(`ai::track` copies the phase, possession and loose count; its inference
from the ball and positions is gone).

The rewrite's own, labelled in the code: a `World::phase` set from outside
the transitions (tests, the played-capture replay) takes the machine with
it at the next step (`World::sync_game_phase`: a dead ball is a restart,
live play out of a restart resumes where the ball is); the rewrite's timed
dead ball and formation restart end phase 6 when they hand over the ball;
the script manager's states are a staged sequence (`World::sequence`); the
won test is the rewrite's own goal check (`score_basket`), and the settled
test reads the holder's shot, dunk and lay-up actions. Not ported: the shot
clock's waits (game+0x198, +0x19c, +0x1a4: the rewrite's clock is its own),
phase 0 (the game crate's intro), the end's own steps after phase 9's
countdown (the rewrite announces the game over at the basket), the classes'
own tip starts and the formation restart (slot 54).

**The machine replayed against the captures**
(`the_phase_machine_replays_the_captured_phases`): fed each update with the
capture's own ball events, holders, ready flags and hold, the machine's
phase agrees with the captured one in 40,873 of the first capture's 40,886
compared updates and 14,149 of the second's 14,155, its possession
(game+0x110) in all of them, and game+0x4a4 in all but 3 and 1. Every
disagreement is a single update where the capture changes a step later
than the sample fed: 11 inbound releases (the ball already a flight update
out), 6 held balls or passes within a unit of half court, a rebound just
below 6 and a launch read in the middle of its update.

**Before and after, in the comparison**
(`shots_turnovers_and_spacing_against_the_captured_original`: the
original's attract matches against the rewrite's computer games of the
same length, all players at 50, then the mean of four pairs of the disc's
teams; it now reads the phase after the game's update, as the capture
samples it, where it read the AI's phase of the update before):

| | Original | All 50, before | after | Disc pairs, before | after |
|---|---|---|---|---|---|
| First capture: turnovers out of phase 5, by the next phase | 4: 6 (and 2 gamebreaker baskets to 6) | 5: 9, 2: 1 | 4: 3 | all in 5 (8, 6, 8, 6) | all in 4 (8, 9, 4, 7) |
| phase-4 updates | 7,302 | 8,811 | 7,222 | 8,796 | 8,944 |
| phase-4 pass fronts | 1,036 | 594 | 571 | 696 | 680 |
| back-court passes | 34 | 24 | 22 | 28.5 | 30.8 |
| phase-6 updates | 6,123 | 5,409 | 5,099 | 5,913 | 6,194 |
| shot phases | 55 | 58 | 58 | 57.3 | 52.3 |
| Second capture: turnovers out of phase 5 | none | 5: 4 | 4: 1 | all in 5 (4, 2, 3, 5) | all in 4 (1, 5, 2, 2) |
| phase-4 updates | 2,489 | 3,119 | 2,483 | 3,254 | 3,247 |
| phase-4 pass fronts | 623 | 113 | 206 | 235 | 257 |
| back-court passes | 17 | 5 | 9 | 9.8 | 12.3 |
| shot phases | 22 | 17 | 23 | 21.3 | 18.0 |

(The rewrite's turnovers leave out baskets; its shot phases are counted by
its shot and dunk events, its dunks scoring without a flight.) Every
turnover the rewrite's computer makes out of phase 5 now brings the ball up
in phase 4, as all six of the original's do; before, the comparison put
them all in phase 5 because the AI's view of the phase lagged an update
(its inference already took the new side's court). The bring-up itself is
not the phase machine's: with the disc's teams the rewrite still spends
about a fifth longer in phase 4 than original, fronts two thirds of its
back-court passes in the first capture and two fifths in the second, and
throws fewer of them (audit 1.3, pass flights; the fast break's behaviours
in `ai::fastbreak`). The gamebreaker's hold lengthens the restarts after
gamebreaker baskets (phase 6 up by 281 updates a match with the disc's
teams), as the original's does.

**Tests.** `game_phase::tests` (a transition each: the tip to the taker's
back or front court, a steal or interception bringing the ball up, the
crossing, a launch then a rebound or a block, a loose ball and its pickup,
the restart ending at the inbound pass once all six are ready, the
gamebreaker's hold, phase 9 once the ball settles, live phases);
`the_phase_machine_replays_the_captured_phases` (`STREET_AI_CAPTURE`: the
machine fed update by update with the capture's ball events, holders,
launches, ready flags and hold, against the captured phase, possession and
game+0x4a4); through `World::step` (`tests::game_phase`): a steal in the
front court bringing the ball up in phase 4 (a strip: phase 2), the tip
in phase 1 until the jumper takes it, a shot's phase 3 and its basket's
restart for the side scored on, a block's loose ball, a gamebreaker's
basket holding the restart 240 updates with no inbound pressed, a won game
in phase 9 the update after its basket, the machine in a snapshot and a
bad one refused; `an_inbound_pass_leaves_the_hand_when_its_animation_lets_go`
(phase 6 over in the release's update);
`the_meters_wait_for_a_team_to_have_the_ball_in_its_back_court`. The AI
tests' front-court fixture now says what the old inference gave its first
frame: the side has just brought the ball up (phase 4, crossing on the
first update).

## The receiver on a standing catch row, at the merges (2026-10-16)

Merging the phase machine onto the pass flights stopped the opening: the
machine puts the game in phase 4 when the jumper takes the toss (as the
captures do), the AI's phase-4 behaviours then ran the tip's receiver up
the floor at full speed, and the 40-update lob came down where he had
stood. Before, the AI's inferred phase stayed 1 through the tip's flight.

The original keeps him there by the catch row, not by his behaviour: the
release callback plays the receiver's row on him at the release
(`FUN_001611d8`, "Shots, passes and catches on the script clock") and its
`8d ee B fd` wait lasts the ball's flight, so a standing row (flags
0x4000) holds him standing whatever the fast break (`FUN_0010a430`, which
has no catching test of its own) asks. The captures agree: the 121
catches of both attract captures (all standing rows: `catch_stand_1` 52,
`_3l` 34, `_3r` 29, the post rows 5, `_2l` 1) moved their receiver a
median 0.08 units in the flight's first 10 updates (at most 0.84) and
0.38 by the catch (at most 2.79), the catch clip's own drift. The sim now
zeroes the stick of a pass's receiver on a standing row while the ball is
in the air (`World::step`, before the player's step); the running rows
keep theirs. The tip's receiver catches 39 updates after the tip (38
captured).

The comparison's spacing check (`shots_turnovers_and_spacing_against_the_captured_original`,
the share of front-court updates with a defender within 4 of the
handler) went from 0.293 (sector merge) to 0.325 (pass flights) and 0.352
(this merge) on the second capture's all-50 game, against 0.201 captured;
the phase machine alone gave 0.292, and the hold on the tip alone 0.329.
With the disc's teams the mean is 0.32..0.36 in every one of those builds
(0.322 and 0.352 here), so the single game's figure is noise around a gap
that has not moved: the defenders still crowd the handler more than the
original's (audit 1.6). The check's limit is now 0.17.

## The court's frame (2026-10-16)

Audit item 1.1, closed. Until this pass the simulation was the original's
court mirrored in x: its team 0 attacked the -x basket (hoop 0 at -36.75,
`restart::side(0)` = -1) where the original's team 0 attacks +x, while the
camera kept the original's constants, so the whole picture was mirrored
and every comparison with a capture mirrored x first. The facing angle
and the sector (the two sections above) were fixed by converting to the
original's frame by hand. The simulation now uses the original's frame
throughout, and nothing mirrors any more.

**The frame.** x along the court, z across it, y up, the units the
game's own; team 0 attacks +x and team 1 -x. Evidence: the side bytes
game+0x98 (`FUN_001404c0`) are +1 for team 0 and -1 for team 1 in every
capture (`sides_word`'s bytes 0x01 and 0xff in both attract captures and all
twelve played captures); `FUN_001404e0` puts the rim a side attacks at side × 36.75;
`FUN_00141298` reads the gamebreaker region map with x negated for an
actor whose team (actor+0x284) is 0. An actor's word looks along (-sin,
-cos) of itself (`0x167d88`), so a heading h of the simulation, which
points (sin h, cos h), is the word of h + π: half a turn, the heading's
convention, not a mirror (`timing::facing_word`, `ai::Ctx::facing` and
`restart::heading_radians` all use it).

**What changed.**
- Roots: `Hoop::native` lists the rims by the team that attacks them,
  +36.75 first (`World::hoops` and `target_hoop` keep hoop i = team i's
  target); `restart::side(0)` = +1, as game+0x98's byte; the court's
  rims (`street-game` `setup::hoops`) are sorted +x first.
- The gamebreaker staging: the region map is reflected for team 0
  (`FUN_00141298`), and the scripts, authored attacking -x, play
  mirrored for the side attacking +x (`Sequence::mirrored`, the starts
  `bind` measures, the hand tracks, the staged ball, the replays'
  `intro::Part`), team 0 now where team 1 was.
- `timing`: `facing_word` is the units of h + π; `sector` is
  `FUN_0018d768`'s own `atan2(from x - to x, from z - to z)`;
  `facing_angle` is `FUN_0018d290`'s `atan2(x - at x, z - at z)`; the
  catch's split of sector 0 measures the receiver-to-passer angle as
  `atan2(r x - s x, r z - s z)`; the approach word is 0x100 for the rim
  at x >= 0 and 0x300 at -x (`0x1693e0..0x169430`). Each is the old
  formula on the mirrored point, so every captured comparison reads the
  same numbers.
- Nothing else in the simulation or the computer players changed: their
  formulas read the side from the hoops (`World::sides`, `ai::Ctx::side`)
  or from `restart::side`, and the original's absolute rules (the court's
  ends at ±40.5, the toss's release point and the referee at x -0.9, the
  no-possession rim +36.75 of the camera) now sit where the original has
  them. What does move is everything the original reads as the side
  times a position: the computer players' zones, wings, fast-break lanes,
  rebound sectors and spots read z × side, so the old frame showed them
  every play's left and right swapped; and the two rules that count x = 0
  as the +x half (`FUN_0013ee10`'s phase and the release callback's
  front court, raw `0x1730d4..0x173100`: `0 <= x` is the +x half), which
  the old frame applied to the wrong half at x = 0.
- The game: the fallback lineup (`setup::load_match`) stands each team in
  its own half facing the basket it attacks by `restart::side`; the help
  text says the home side attacks the basket on the left of the screen.
  The camera, HUD, referee, crowd and scenery code needed no change (the
  Street eye leads by `restart::side`; the courtside, basket and replay
  cameras read the ball's x; the HUD's panels go by team).
- Snapshot `VERSION` 38: every stored place, heading and hoop means the
  mirror of what 37 stored.
- Tests: every fixture is in the original's frame (the shared `hoops()`
  +x first, the three-a-side builders by `restart::side`, each scene
  mirrored with its expectations). One computer-player scene
  (`a_teammate_sets_a_pick_for_a_person_on_the_move`) is mirrored in z as
  well: mirrored only in x, the teammates' fast break, whose lanes read z
  × side, keeps them running past the pick's moment. The capture
  harnesses compare as captured (`ai::capture`'s floor and ball,
  `played_capture`'s replay worlds, `row_capture`'s worlds), and an
  original word or stick angle u is the simulation's heading or stick at
  u + 512. Renamed: `timing::tests::the_facing_angle_is_the_originals`,
  `tests::team_one_plays_staged_scripts_as_authored`.

**The picture.** The original's projection (`FUN_002ffac0`, ported as
`view::framing_projection` and held to the played captures' camera
states by the camera tests) puts +x on the left of the screen when the
eye looks along +z, as the Street row's does; the rewrite's renderer
agrees (the demo below), and the courts' scenery stands at the same ends
(on the rooftop court, `FOREST`, the arched-window building is behind the
right-hand basket in the original's frames from the user's save slot 1
and in the rewrite's). So the home side (team 0) attacks the left-hand
basket, as the original's does; before this pass it attacked the
right-hand one. The demo (`--demo --at 12`, Rucker, seed 2002): before,
the visitors (team 1, the left panel) dunked at the left-hand basket in
update 502 and the home side brought the ball up toward the right; now
they dunk at the right-hand basket in update 709.

**Before and after.** The deterministic comparisons read the same
numbers, because the conversions they replace were exact:
- played captures (all twelve): every report identical (shots, held side
  fades, side shots, passes, catches, pass flights, odds, replays, stage
  bounds, reseeds); rows: passes 72 of 98 in sector (32 candidates),
  catches 110 of 112 (108), moves 16 of 16 (16);
- attract 1 and 2: the opening (tip at 200 against 201 and 200, flight
  40, caught 39 after against 38), the opening's draws, the phase replay
  (40,873 of 40,886 and 14,149 of 14,155 updates), the pass flights and
  the rows (passes 77 of 98 and 20 of 30, catches 81 of 92 and 28 of 29,
  moves 6 of 6 and 2 of 2) identical; the restart spots identical but
  for x's sign, now the capture's own.

The rewrite's own computer matches (`shots_turnovers_and_spacing_against_the_captured_original`)
play another game once the computer players see the court the right way
round:

| capture | measure | original | all-50 before → after | disc teams (mean of 4) before → after |
|---|---|---|---|---|
| 1 | defender within 4 | 0.262 | 0.363 → 0.314 | 0.322 → 0.325 |
| 1 | shot phases | 55 | 49 → 46 | 55 → 56 |
| 1 | turnovers | 8 | 6 → 10 | 8 → 10 |
| 1 | back-court passes | 34 | 20 → 20 | 33.5 → 31.5 |
| 2 | defender within 4 | 0.201 | 0.352 → 0.311 | 0.352 → 0.337 |
| 2 | shot phases | 22 | 17 → 16 | 19.2 → 19.2 |
| 2 | turnovers | 0 | 2 → 2 | 2.25 → 4.5 |
| 2 | back-court passes | 17 | 8 → 6 | 10.5 → 9.75 |

Those are single games, and a computer game swings from build to build
(the last section). Over many games the two builds play alike: 40 seeds
of the all-50 game and 40 of each of the four disc pairings, the length
of the first capture (mean ± standard error, before → after, original):
defender within 4, all-50 0.319 ± 0.003 → 0.325 ± 0.003 and disc 0.331 →
0.333 (0.262); shot phases 52.5 ± 0.6 → 53.1 ± 0.7 and 54.5 → 54.8 (55);
turnovers 8.2 ± 0.3 → 8.1 ± 0.5 and 7.6 ± 0.2 → 8.0 ± 0.2 (8);
back-court passes 23.2 → 22.8 and 33.9 → 34.4 (34). No measure moved by
more than its noise: the computer players' handedness does not show in
these totals, and the open gaps (the spacing, audit 1.6; the back
court's passing) are unchanged.

**Tests**: the timing tests above, now on the original's court
(`the_sector_is_fun_0018d768s_on_the_originals_court`,
`the_filters_read_the_sectors_on_the_sims_court` with both rims,
`the_facing_angle_is_the_originals`);
`gamebreaker::tests::compact_region_rows_preserve_missing_columns_and_team_reflection`
(team 0 reflected); `tests::the_release_callback_reads_the_court_the_ratings_and_the_score`
(x = 0 is team 0's front court); `view`'s
`the_row_leads_toward_the_rim_its_side_attacks` (team 0's eye leads to
+x); the capture tests above.

**Parity**: 92 cases' evidence re-pinned, each case's limitations naming
what changed in the files it cites and whether the cited region did; the
six pins already stale before this pass (`self_toss.rs`, `ordinary.rs`,
`ordinary_tests.rs`, one `view.rs` locator) are left as they were. No
status changed: 118 obligations, 79 matched, 12 known differences, 27
untested.

## Dunks and rebounds on the ball's states (2026-10-16)

Feeding the rewrite's computer games through the phase machine (the
section above) showed two differences in how play leaves phase 3. The
rewrite's dunks scored in the update they were let go, so play went from
phase 5 straight to 6 (28 times in the first capture's all-50 game
against the original's 2, both gamebreaker baskets); the original's dunks
are launches that enter phase 3, and the basket comes later (3 to 6, 37
times). And the rewrite's rebounds always went loose first (3 to 2: 7
against 4), where the original's rebounders take the ball in the air out
of phase 3 (3 to 4 or 5: 13 against 1). Both come from the ball's own
states, which the rewrite now keeps as the original does, and from the
rebound jump, which it now has. Traced from the decomp and the raw ELF
(the decomp loses `FUN_0015ce50` and callback 36 at `FUN_0030fa50`).

**The dunk is a launch.** The dunk script's callback 21 calls the shot
launcher `FUN_0014aaf8` like a jump shot's, made or missed by the release
callback's dunk odds (kind 0 a make, 3 a miss). For an actor in a dunk
(`FUN_0016e9d0`) or an alley-oop dunk (`FUN_0016e9e0`) it takes its dunk
path (raw `0x14ac7c..0x14ae40`): angle 0; the pace the length of the
ball's last move (`ball+0xa0`, the hand's, as the held ball followed it)
held to `DAT_004b84a4`..`DAT_004b84a0`, 10 and 20 mph a frame (`5280 /
(fps × 3600) × 10` and `× 20`, 0.24444444 and 0.4888889, set at
`0x153e44..0x153e84`), or the top one outright for a rebound jumper
(`FUN_0016e9f0`) or an actor with `+0x1e8` 0x15; `+0x184` 4 or 3 (the net
keeps half its speed). The ball is put in the ring: from the hand toward
the basket's middle (`side × 36.75`, 11.9567747, 0) no farther than
`0.75 − r` along the line between them (all three axes, `vsub`, the
squares summed, `vsqrt`), then no lower than `r + 12`. A make flies
straight there at the pace: `T = |aim| / pace` updates, the velocity
`aim / T` plus half the gravity times `T` up (straight down at the pace
when `T` is 0.0001 or less, `0x14bc08..0x14bd28`); a miss takes the
look-ahead's tries, each flown at the pace, the first that works taken
with no draw (the half-time draw is the other path's). Slot 12, then
slot 11 as for every launch: phase 3. The basket is the ring's, the
update the ball goes down through it.

Both attract captures hold 37 dunk launches (every launch from
`r + 12`, the height only this path lands at; the launch sample is after
the launch update's own move, so it is undone first): every one is placed
exactly where the rule puts the previous sample's ball (within 0.002), 36
made and one missed. Of the 36, 33 fly at the hand's pace held to the
bounds (30 hands faster than the top, three at their own 0.465 to 0.475),
one at the top for a rebound jumper (state bit 8, a putback) and two at
the top with a slower hand, both the same dunk script (`0x4481f5`: the
`+0x1e8` branch, which the capture does not record). The timer reads 2 at
every launch sample, and every made dunk goes down through the ring the
next update (state 5 to 1): phase 3 for exactly one update, then the
restart's 6 (or the win's 9 an update later).

Port: `World::launch_dunk` (`shooting::dunk_place`, `shooting::dunk_pace`,
`World::launch_shot`'s dunk mode, `Ball::launch_at_pace`), used by
`World::slam` and `World::oop_slam`; the shot carries its basket
(`Shot::slam`) and scores it at the ring (`World::step_ball`). The
rewrite's own: its dunker carries the ball at the waist (no hand bones),
so the pace is the top one, as in 33 of the 36; the rewrite's block at
the rim and its alley-oop odds are unchanged.

**The ball's states and the shot's timer.** `FUN_0014d1f8`'s state
machine (raw `0x14d630..0x14d784`, `s3` the hoop's contacts, `s4` the
floor's or the ceiling's): a shot (5) runs its timer `ball+0x158` down;
at 0, or on the floor or the ceiling, it is loose (1, event `0x56`); on
the backboard, the collar or the rim it is 6, the timer running on. A 6
ignores every contact and at 0 is a rebound (0, event `0x56`). A rebound
on the floor or the ceiling is loose; on the hoop it stays 0. The
launcher's timer: for a flight the tries watched, the skip ahead (`s6`)
plus the watched update of its last contact (`s0`: the backboard, the
collar, the rim, the floor or the ceiling, `FUN_0014d1f8`'s answer, or the
update it stopped watching when it had gone through the ring) plus one
(`0x14b75c..0x14b9e0`); for a straight flight, `T` updates to its aim and
`v = vy − gT` its rise there, `ceil(T + (v + √(v² + 2g·0.043225)) / g) +
1` (`0x14bd30..0x14bda0`), the flight and the fall through the rim's
tube. So a missed shot is 6 from its first touch of the hoop until an
update after the launcher's last, then a rebound coming off, and an
airball never touched is loose when its timer runs out.

Port: `Ball::touched` (6), `Ball::rebound` (0), `Forecast::last`,
`shooting::straight_timer`, `Ball::fly` (a 5 ends on the floor, a 6 does
not; the timer's end is a rebound after a touch, else loose), the floor
ending a rebound (`World::step_ball`), `World::native_ball_state` (6 for
a touched shot, 0 for a rebound whatever its height). Before, the
rewrite's flight lasted the updates the launcher watched (until the ball
was below the rim and not rising) and every missed shot's end was a
rebound. The audit's first ball point (`docs/AUDIT_1TO1_2026-10-16.md` 2)
is closed: passes and loose balls already met the rim, collar, backboard
and net (`Ball::step` for every free ball since "Free-ball contacts"),
and a shot whose timer runs out no longer scores (the original counts
only the ring; the at-the-basket rule is gone). The rewrite's own: a
staged Level 2 finish, flown past the rim untouched on its script's
timing, still counts when its flight ends.

**The rebound jump.** The actor's update `FUN_0015da28` sends a jump
press (actor+0x7c `0x101000`, on the floor, not in phase 6, not already
jumping) to a rebound jump (`FUN_0015ce50`, state bit 8) when the ball is
0, 6 or 1, else to a block jump (`FUN_0015d3a0`, bit 12; the rewrite's
`Action::Jumping`). `FUN_0015ce50` (raw `0x15ce50..0x15d398`) faces the
jumper the way he moves (actor+0xb0 = +0xa8) and picks one of nine kinds
from where the ball will be (`FUN_001537d0`, the ball's own update run
ahead): the ball 20 updates on (`trunc(fps × 0.33826)`) within
`max(1.5, r)` of a point in front of his chest (`DAT_004b8780` = (0,
7.477, −1.352) in the model's frame, times the skeleton's scale) while he
stands is the standing catch (7); then the ball 30 updates on (`fps / 2`)
is low when no higher than his standing reach (`height × 1.2 × 1.25`); at
12 or above, a goaltend (6) for a defender when within `r + 0.79322`
across of the rim the side with the ball attacks, no higher than `r + 15`
and slower than 10 mph a frame, and near when within 4 of that rim across
or he is (`FUN_0016e930`); under the basket (`FUN_0016f580(actor, 1)`:
within 2 across and past 37, or past 38 within 10) is 8; otherwise, by the
sector of his facing to that ball (`FUN_0018d768`), behind him (7, 0, 1)
3 low, 5 near, else 1, ahead 2 low, 4 near, else 0. The script is
`rand() % count` of the kind's list (`0x3d7138`, counts `0x43f818`: 1, 7,
1, 1, 3, 2, 2, 1, 1). The seventeen scripts each open with a 4-update
blend and a wait, then their callback 36 (35 for the goaltend pair) at
tick 6 to 10 (8 in most), `8d ee F ef 00` (frame F in the updates the
callback aimed for), `8d ee G fe` (until he lands), a tail of 6 (12 or 13
in two), and callbacks 37, 7 and 1; the standing catch has no jump, a
55-update wait.

Callback 36 (`FUN_00175858`, raw `0x175b68..0x175fec`) poses the clip at
frame F for the hand (bones 1 and 2 by the clip's flags, their midpoint
for both) and sends him up so that it meets the ball: the highest jump
`2 + 4 × Rebounds / 99` (`v_max = √(2g top)`), the lowest 1 (`√(2g)`),
across at most `0.75 v_max` an update; from `trunc(v_max / g) + 1`
updates down to 1 it tries the ball's place `n` ahead (raised to the
hand's height when lower, until one has worked), the velocity `(P −
hand) / n` plus `gn/2` up, passing over any that would meet the ball with
him coming down (but the first, when it is 1); it keeps the first that
works (the rise within its bounds, the speed across within its limit),
then any that works with a smaller speed, or while none has worked any
smaller one, and holds the rise and the speed across to their bounds.
`+0x40` the velocity, `+0x64` the airtime (`FUN_0019bed8`), the channel's
`+0x20` the updates it aimed for (without a working one, those to the
top).

The catch is `FUN_0016ed18` (its give at `0x16f1b8`), in the jumper's
update after the pass catch: nothing from behind the backboard
(`FUN_0016e830`: the ball above 10.5 within 38 and 3 of the middle, he
beyond 38); either hand bone within `max(1.5, r)` of the ball, or the
arm's segments hit in the bone sweep (`FUN_00137db8`, bones 0xe/0xf and
0x12/0x13); a ball on the rim (6) not inside the ring (within 0.793225
across of the possession's rim at 12) and outside it only when `rand() %
100` is at most `(2 × Rebounds + Blocks + Speed + Power) / 5` held to
5..90, else it slips him for the airtime left (actor+0x1a8); then the
give (`FUN_0014cb08`, slot 24: the phase out of 3 to his side's court).
Another part of the sweep knocks the ball off his body (`FUN_00137ed8`).

The attract captures hold 37 rebound jumps (bit 8 rising), 35 with one of
the seventeen scripts on the channel: every one leaves the floor exactly
its script's callback tick after the press, and the script's kind agrees
with the facing's sector to the ball's captured place 30 updates on in 32
of 34 (both jumpers under the basket played kind 8's script; the two
others can be the ball's real path leaving the prediction). They played
kind 2's script 14 times, 0's 10, 4's 8, 3's 3. Of their 16 catches
out of phase 3, 15 were in the air, 7 to 29 updates after the takeoff,
the ball 8.3 to 11.6 above the feet and up to 3.85 across, and one a
standing player's (the bone sweep, at 7.54 up); every one from a
rebound (state 0).

Port: `crate::rebound` (`SCRIPTS`, `KINDS`, `World::start_rebound_jump`,
`World::rebound_aim`, `World::rebound_jump_tick`, `World::rebound_catch`),
`Action::ReboundJump`, the dispatch in `World::buttons`, the catch in the
players' loop (before the ball's update, as the original's actor update
runs), `World::pickup_credit` (the rebound's points and statistic, shared
with the loose ball's pickup); the AI's bits 8 and 12 (`ai::court`,
`contact`) are the two jumps, latched at the press. The rewrite's own:
the hand is the jumper's standing reach above his feet, carried rigidly
on the jump (as the tip-off's), and the standing catch's the chest point;
the hand takes the ball only once he is off the floor; the bone sweep, its
knock off the body and a pass in the air are not taken; the goaltend
pair's callback 35 is not traced, so they aim as callback 36 does; the
display plays the scripts with their `ef` wait as one update.

**Before and after** (`shots_turnovers_and_spacing_against_the_captured_original`,
the rewrite's computer games the capture's length; all-50 the one game,
disc the mean of the four pairs; shots now counted as the original's,
entries into phase 3, where before they were the shot and dunk events):

| capture | change | original | all-50 before → after | disc before → after |
|---|---|---|---|---|
| 1 | 5 → 6 (a basket without phase 3) | 2 | 28 → 0 | 27.5 → 0 |
| 1 | 3 → 6 | 37 | 8 → 41 | 17.0 → 41.5 |
| 1 | 3 → 4 and 3 → 5 (taken in the air) | 13 | 1 → 11 | 1.75 → 6.0 |
| 1 | 3 → 2 | 4 | 7 → 4 | 9.25 → 5.25 |
| 1 | 5 → 3 | 55 | 17 → 57 | 29.25 → 54.0 |
| 2 | 5 → 6 | 0 | 10 → 0 | 11.75 → 0 |
| 2 | 3 → 6 | 17 | 4 → 15 | 4.25 → 15.0 |
| 2 | 3 → 4 and 3 → 5 | 3 | 0 → 3 | 0.75 → 2.0 |
| 2 | 3 → 2 | 2 | 2 → 2 | 2.75 → 2.5 |
| 2 | 5 → 3 | 22 | 6 → 20 | 7.75 → 19.5 |

Over twelve seeds of the all-50 game at the first capture's length, per
game: 3 → 2 10.2 → 2.7, the catches in the air 0.8 → 9.1, 3 → 6 8.9 →
37.9, 5 → 3 20.9 → 50.6, 5 → 6 29.6 → 0. The spacing check (a defender
within 4 of the handler, audit 1.6, open) did not move with it: twelve
seeds' means 0.314 → 0.316 (first capture's length) and 0.320 → 0.323
(second's), the pairs 0.321 → 0.338 and 0.342 → 0.336; the second
capture's single all-50 game went 0.311 → 0.378 against 0.201, so the
check's limit is now 0.18. In the all-50 games 34 rebound jumps played
kinds 0 (14), 2 (9), 3 and 6 (3 each), 4 and 7 (2 each), and 1 (1),
against the captures' 2, 0, 4, 3: the same few kinds, 0 and 2 most.

**Tests.** `ai::capture::the_captured_dunks_leave_the_launcher_as_ported`
and `the_captured_rebound_jumps_follow_their_scripts` (`STREET_AI_CAPTURE`,
both captures); `tests::a_dunk_goes_down_through_the_ring_the_update_after_it_is_let_go`
(phase 3 in the slam's update, the basket and phase 6 the next, the net's
half speed); `tests::rebound` (a missed shot 5, 6, then 0 at its timer
with phase 3 throughout; an airball loose at its timer, phase 2 at once;
the straight timer; a rebound jump leaving the floor at its callback tick
and taking the ball in the air, phase 3 to 5, the rebound credited; a shot
on the rim taken only outside the ring and on the roll, a slipped jumper
taking nothing more; a jump at a flying shot a block jump and at one on
the rim a rebound jump; kind 8 under the basket; the snapshot);
`rebound::tests`.

**Leads, not done.** Callback 35 (the goaltend pair's aim); the bone
sweep (`FUN_00137db8`) and its knock off the body (`FUN_00137ed8`), and
a standing player's catch of a rebound by it (one of the captures' 16);
the rewrite's own floor pickup (`World::catcher`) still takes a loose
ball after the ball's update, an update before the original's actor
would, and a block jumper there; the display's `ef` wait (the updates
callback 36 aimed for); the dunk's pace from the drawn hand; a pass
taken by a rebound jumper's hand; points for a ball that goes through
outside a shot. (The bone sweep's catches and knocks off a standing
body, the floor pickup in the actor's update and the basket outside a
shot are ported since: "The ball on the bones".)

## Turnovers: the swipe's hand, the limbs a pass meets, blocks (2026-10-16)

Audit item 1.5. Traced from the raw executable (Ghidra cuts `FUN_0016cf30`,
`FUN_0016d888` and `FUN_0015da28` at calls to `FUN_0030fa50`), measured on
both attract captures, the played captures and three PCSX2 save states.

**What the original's turnovers are.** The first attract capture's six
changes of possession out of phase 5 (to phase 4; two more are gamebreaker
baskets) are four trick counters (the defender presses steal with two
turbos, `0x40200`, as the handler starts a trick; he has the ball 0..4
updates later) and two counter-stance catches of a head pass: a trick pass
to the passer himself, kept in his hands about 20 updates in ball state 4,
caught by a defender playing `STEALCOUNTER_1_2`/`_2_5` (records 402/403)
3..4 updates into its flight (`FUN_00148378`'s kind-2 leg, which the
rewrite already had). None is a steal from the hands: no swipe takes the
ball outright in either capture. Balls knocked loose instead (phase 2, not
counted as phase-5 turnovers): five strips by swipes (six by the swipe's
own span, one outside phase 5), three blocked shots and two blocked passes
(lobs to oneself and a pass over a defender), all leaving at 0.35 an update
along the floor and 0.125 up (0.114 a sample later), none caught; no pass
is knocked loose by a limb in the passing lane. The second capture: two
strips, a blocked shot and a blocked pass.

**Bones.** The rules read the actor's posed model (`actor+0x108`; bone `i`
at `*(model+0x54) + i * 0xb0`: +0x0 the world position, +0x20 the world
rotation, +0x40 the parent; 43 bones, built lazily by `FUN_0019d2f8`).
`FUN_0019d4c8` builds them: a bone's world position is its parent's plus
the parent's rotation applied to its local translation times the model's
height scale (`model+0x40`, `height * 1.2 / 78`), the thighs' z (bones 1, 5)
and the upper arms' x (13, 17) times the width when it is the larger, the
head's (21) also times the head scale (`model+8`); its rotation is the
parent's times its own. The pelvis stands at the actor's x and z (also for
`notrans` clips), at his height plus the clip's root height times the height
scale, turned by the facing word `actor+0xa8` (1024 a turn; not `+0xb0`).
Ported as `street_sim::pose` (`Poses`, from `street_assets::pose::PoseData`:
the skeleton, the 1144 animation records of `0x3de8a0`, each clip's native
rotations, root and `_X` states; mirrored records as the renderer mirrors
them). Checked against the RAM bones of three save states
(`tools/pose_capture.py`, `tests/pose_disc.rs`): the six actors of state 01
and the airborne block jump of state 02 within 0.01..0.03 on every bone
0..21, the dunk 0.11, and the held ball 0.002 from the holder's posed hand;
the moving actors of state 02 and every actor of state 09 (a restart's walk)
are 0.4..4.7 off: the original blends channels the simulation does not.

`FUN_0014da20(ball, out, model, clip, frame, hand, flag)` is the point a
hand holds: by the clip and frame (`0x14da2c..0x14dc54`, nine cases through
`0x43ed40`) a bone and an offset (BSS `0x4b84c0..0x4b8530`, read from a save
state: (0.4, 0, 0) for the hands, 0.2 for the feet, thighs and shins of the
clips that carry the ball there), the offset times the model's scales
(height, width, width), with flag 0 (the held ball) moved out by the ball's
radius along the case's axis (the hands: +z), the case's axis negated for
hand 2, turned by the bone's rotation (cases 1, 3, 7 not), added to the
bone. Hand 1 is the right (bone 19), hand 2 the left (bone 15). Ported as
`pose::hand_point`.

**The swipe** (`FUN_0015da28`'s steal press, `0x15e0e4..0x15e360`; the
test `FUN_0016cf30`): ported as `street_sim::swipe`.

- The script: with nobody holding the ball a low swipe by `r & 1`
  (`0x3d7220`: in, out); with a holder the post-up steal when the swiper is
  within 5 (the game's distance table), in the eighths behind him
  (`FUN_0018d9f0(holder, swiper)` 0, 1 or 7), facing him within `0xc8`
  (`FUN_0018d1f8(swiper, holder)`) and the holder `400..0x270` off the line
  to him, or when the swiper plays the post defence's stance
  (`postdefstand_onball`, 718/719, the list at `0x43f070`); otherwise by
  the ball's height against the swiper's pelvis (bone 0): within 1.5 the
  middle swipe `0x45761b`, above it the high one `0x4575c9`, below it a low
  one by `r & 1`.
- Each script blends five updates (`8e f5 05 0`) to its clip: the mirrored
  record unless the left hand (bone 15) is nearer the ball than the right
  (`FUN_001978b0`'s `0xf5`), the post-up steal by its stance (`99`); sets
  the swipe bit from its first update (`9b 40`), waits to frame 23 (30) over
  22, 27, 25, 27 (35) updates (`8d`) and clears the bit (`9c 40`): spans of
  27 (low out, 388/389), 32 (low in, 392/393), 30 (high, 386/387), 32
  (middle, 390/391) and 40 (post-up, 398/399), as the captures' swipe bits
  last. The played captures show the channel doing exactly this (the blend's
  target at +0x54 from the press, the clip at 0.85, 1.70, .. from update 5).
- The test, every update the bit is set until the hand has once been in
  reach (`actor+0x23c`, cleared at the press): `FUN_0014da20` for channel 0's
  clip and frame (through the blend the held clip's), the right hand when
  its state has bit 8; within 1.5 of the ball (`ball+0x30`, from the last
  ball update), 0.75 when the swiper moves (`actor+0x40`), a twentieth of
  that on the post-up clips (398, 399); `r % 100` is drawn on every test;
  the first update in reach uses the swipe up whatever the roll; the odds,
  catch and knock are the strip already ported.
- Checked: the played captures' four strips each come within an update of
  the first update the posed hand reaches (two on it, two an update after:
  `played_capture::the_swiping_hand_reaches_the_ball_where_the_original_strips_in_play`,
  posing each swiper from the captured channel and his roster body); in the
  attract captures, which keep neither the float frame nor the players, the
  first capture's six strips come 0..2.8 from the posed hand (one on time,
  one an update late) and the second's two do not reach
  (`ai::capture::the_swiping_hand_reaches_the_ball_where_the_original_strips`).

**The limbs a pass meets** (`FUN_0015da28`'s tail, `0x15e7d0..0x15e898`):
for every actor in turn with the ball not held and not barred
(`actor+0x1a8`): a pass more than three quarters flown (`ball+0x60` over
`+0x64`), `r % 100 < trunc(Steals / 99 * 15)`, then `FUN_00137db8`: the
pelvis within `actor+0x2a8` of the ball (`FUN_00156dd0`: one more than the
longest of three bone chains at the body's scales, `0x3d7068`), the ball's
last move (`ball+0xa0`) in four steps, each above the floor tried against
the limbs in order (`PTR_DAT_003d10a0`: neck to pelvis r 0.73 reaching 1.0
past it, neck to head 0.35 / 0.85; the legs' six, r 0.4 or 0.3, the feet
0.5 past; the arms' six, r 0.3, the hands 1.0 past; `FUN_00137b48`: the
point over the segment, `FUN_00137918`, nearer than the ball's radius plus
the limb's, `FUN_001379d8`). A limb met sends the ball off loose
(`FUN_00137ed8(0.15)`): moving into it, reflected with restitution 0.15, set
out to touch the limb, spinning about the normal across its slide (at most
pi/10 an update), at most 15 mph (`5280 / (60 * 3600) * 15` an update),
launched free (`FUN_0014c408`); moving away, nothing. At the release
`FUN_00173570` bars the passer and his teammates other than the receiver for
the whole flight (`+0x1a8` = the flight) and, on a pass of 15 or more, the
defenders for its first 35%. Ported as `World::step_pass`'s knock,
`pose::sweep`, `Ball::deflect`, `Event::Deflection`; the dive's catch
(`FUN_0015c598`) is the same sweep for records 71/72 only.

**Blocks** (read in outline, not ported). `FUN_0015ce50` starts a block
jump from the lists at `0x3d7160..0x3d71ec` (fifteen scripts
`0x441a7d..0x44239b`: side-arm, scoop, goaltend, smack off the glass, rim
swats, the freestyles); `FUN_0016d888` (from the actor's update while
`FUN_0016e9c0`) tests the blocker's hand points (`FUN_0014da20` flag 1, both
hands when the clip's state has `0xc`) against the ball within
`DAT_003d9058` (adjusted for a person's side on Easy), and the hand capsules
(bones 14..15, 18..19) by `FUN_00137db8`; a ball on the rim (state 6) within
0.793225 of its middle is left alone, and otherwise taken against `r % 100`
by `(2 * rating 3 + rating 5 + rating 8 + rating 4) / 5` (5..90); a shot
more than 70% flown against `(progress - 0.5) * 200`; the ball is knocked
loose at 0.35 an update and 0.125 up with a random spin (0.075 + `rand` *
0.075) through `FUN_0014c408`, or deflected (`FUN_00137ed8(0.15)`), or
rarely caught (`FUN_0014cb08`, never captured), and the shooter is barred
`r % 15 + 12` updates. The rewrite still decides a block at the release
with its own chance (`World::release_shot`, `tuning::BLOCK_*`) and knocks
the ball 14 away and 8 aside: its stand-in, labelled. Porting needs
`FUN_0015ce50`'s choice, the block scripts' jumps and `FUN_0016d888` read
in full. (Ported since, with the block jump `FUN_0015d3a0` and callback
35: "Blocks and the dribble on the original's bones".)

**Numbers.** The first attract capture against the rewrite (one match's
length each; every player 50, then four pairs of the disc's teams), before
(stand-ins: the swipe tested once at update 13 with a hand 4 ahead, any
defender within 1.8 of a pass in its last quarter catching it) and after:

| | original | all-50 before | after | disc teams before | after |
|---|---|---|---|---|---|
| turnovers out of phase 5 | 6 (+2 gamebreaker baskets) | 10 | 7 | 8, 8, 9, 15 | 8, 4, 8, 4 |
| trick counters | 4 | 7 | 4 | 5, 5, 5, 5 | 4, 1, 2, 2 |
| counter catches of a head pass | 2 | - | 0 | - | 0 |
| passes taken in the lane | 0 | 1 | 0 | 0, 2, 1, 1 | 0 |
| passes knocked loose by a limb | 0 | - | 0 | - | 1, 1, 1, 1 |
| swipes taking the ball | 0 | 0 | 0 | 0, 0, 0, 1 | 1, 0, 0, 1 |
| strips | 5 | 4 | 3 | 2, 3, 3, 1 | 1, 2, 3, 2 |
| blocks | 3 shots, 2 passes | 6 | 5 | 8, 2, 3, 11 | 3, 5, 7, 2 |

One match each swings widely; means over eight seeds of the same length:
all-50 turnovers 8.1 before, 8.3 after (strips 3.0, 2.4; lane catches 1.0,
none; limb knocks 0.5; swipe catches none, 0.4); the disc teams 7.6 and 6.9
(strips 1.6 and 2.6, lane catches 1.2 and none, limb knocks 0.8, swipe
catches 0.35 and 0.7, trick counters 3.3 and 2.9). The second capture:
original two strips, a blocked shot and pass; the all-50 match three
strips before, one after.

**Left, labelled.** The held ball the swipe reads is the holder's posed hand
on his gait clip: the dribble's flight between the hand and the floor
(`FUN_0014e060`'s frames flagged 0x10 and 0x20, posed at both ends) is not
ported, and the simulation's own ball still rides `Ball::carry`. Actors
pose their gait channel's clip (or the swipe's): the original's channel
blends (walk into run, overlays) and the clips of tricks, passes, shots and
jumps are not posed. A deflector's cooldown (`+0x1cc` = `DAT_004b99b8`, 10)
and its later touches, the defenders' 35% bar, the receiver's own sweep and
the catch by bones are not ported (the receiver catches on arrival). Online
matches carry no animations (the referee has no disc): every machine plays
the stand-ins. The swipe never takes the ball from a teammate (the
rewrite's own: `FUN_0016cf30` does not check). The dribble's flight is read
in outline only (the hand posed at the clip's 0x10 and 0x20 frames, the
ball between them). (Ported since: "Blocks and the dribble on the
original's bones"; the deflector's cooldown, the defenders' 35% bar and
the receiver's catch by bones since: "The ball on the bones".)

## Blocks and the dribble on the original's bones (2026-10-16)

The two stand-ins the turnovers pass left: the block was the rewrite's own
chance at the release (`World::swat` with `tuning::BLOCK_*`), and the
swipe read the holder's posed hand where the original's ball is on its
way between the hand and the floor. Traced from the raw executable
(`tools/mipsx.py`, VU0 macro ops decoded by hand). Note for the decomp:
`FUN_0030fa50` is no assertion; it returns the frame-rate block (+0x24
the rate, 60; +0x28 the same as a float), and Ghidra cuts every function
at a call to it, so `FUN_0015ce50`, `FUN_0015d3a0`'s no-holder branch,
callback 35 and `FUN_0016d888`'s tail were read from the bytes.

**The block jump** (`FUN_0015d3a0`, raw `0x15d3a0..0x15d918`; the press
dispatch is `FUN_0015da28`'s, "Dunks and rebounds on the ball's states").
With two turbos held (actor+0x78 & 0x40000) it is the turbo block,
`0x441bd5`, when the bar holds half (`FUN_00142098` the controller's
turbo, `FUN_00190c08(0.5)` takes it, not with the unlimited-turbo cheat)
and sets actor+0x360; with less, a message and no jump at all. Otherwise
one of twelve kinds, the script `rand() % count` of its list (`0x3d71f0`,
counts `0x43f840`: 1, 3, 3, 1, 3, 1, 2, 3, 4, 3, 1, 1):

- 11 under the basket (`FUN_0016f580(actor, 1)`);
- with a holder (ball+0xf4), by his state bits (+0xa4): none of 0xa088
  (bits 3, 7, 13, 15) kind 0; bit 3 or 13 (a jump shot or lay-up; this
  pass read them as a dunk or lay-up, corrected in "Defence and spacing on
  the original's speed levels") 1 when the
  jumper stands behind him (`FUN_0018d9f0(holder, jumper)` 0) within 5,
  else 4 far or 5 near; bit 7 or 15 (a dunk or a leap) 2 far or 3
  near when the jumper is between him and the offence's rim
  (`FUN_0016fef8`: within 0x100 of his line from that rim and nearer to
  it), else 4 or 5;
- at a ball in the air, its place 28 updates on (`trunc(fps × 0.48333)`,
  `FUN_001537d0`): 6 the glass (Blocks 80 or more, the jumper 1..3 across
  of the middle on the ball's side and within 6 of the board the other
  side attacks, the ball within 1 of that board's point at `38 − r`, its
  height held to 11.5..15 and z to ±3); 7 a shot ahead of him (the ball
  within 0x100 of his line to the offence's rim) with him within 10 of
  it; 8 a shot with him between it and the rim, 10 or more out; 9 far, 10
  near.

"Far" is `FUN_0015d920`: the ball within 32 across of the middle, then
the jumper 15 or more from the offence's rim, or within 36 of the middle
with the ball on his own end (`FUN_0016f2b0` against 0x200 by his side).
The bits read from the attract captures' launches: a jump shot sets 7, a
dunk 3 (13 with it when it turns into a lay-up), an alley-oop's leap 15.

The thirteen scripts (`0x441a7d..0x44239b`, records 437..465) and the
rebound jump's goaltend pair (`0x441f0c`, `0x441fbc`, records 448..451):
each sets bit 12 itself (`9b 00 10 00 00`; the glass pair also bit 19,
`9b 00 10 08 00`), calls callback 5 (`FUN_00174eb0(0, 0)`: actor+0xb0 the
angle to the ball, the facing +0xa8 then turning 46 an update, measured),
and callback 48 (`FUN_00177ce8`: the branch, the mirrored arm, when the
angle from the jumper to the ball 15 updates on less his +0xb0 is 0 or
less), blends four updates (six on `0x44239b`'s branch; the glass pair
picks its arm by `ec`: the alternate when z × his side is below 0), waits
to frame F0 over a few updates, and runs callback 35 (`-1000, F`) at tick
6..10; then `8d ee F ef o` (frame F in callback 35's meet plus `o`, 0, −3,
−4 or −10), `86 36` (callback 54, which passes at once), on some a fixed
wait, `8d ee G fe` (until he lands), and callbacks 7 and 1. The glass pair
runs callback 29 (768) and an aimed jump of its own (`a9 ef 01 20 F`,
`FUN_00199e20`: the hand at frame F to the ball 20 on, pinned at ±38,
read in outline). Bit 12 lasts the script.

**Callback 35** (`FUN_00176080`, raw `0x176080..0x1771ac`). It poses the
clip at frame F facing +0xb0 for the hand (`FUN_0014da20` flag 0; both
hands' midpoint on 0xc, else the right on 8, the left), and aims as the
rebound's callback 36 does but with its own bounds: the top rise
`√(2g h)` with `h = 2.5 + 3.5 × Blocks / 99` (1 more for a person's jumper
on Easy), `5.44 + 3.56 × Blocks / 99` at a leaper (holder bit 15); the
lowest `√(4g)`; across at most 0.57 of the top. At a ball in the air it
tries the ball's place `n` on from `trunc(v / g) + 1` down to 1, `n` held
to the flight's updates left less 3 at a shot or a pass, and at a shot the
latest working meet wins. At a holder it aims at a point by him: his head
(bone 21) moved on by his velocity, at a dunk or lay-up held at his head's
peak plus `0.36 × height` plus 3 (14 on the floor; `n` the updates to his
top, at least 15 with bit 13, 3 without), 3 toward the rim from him; at a
jump shot or a leap 13 on the floor, else his head's way plus `0.36 ×
height` (at least 13; `n` within his +0x31c, the count to his release,
less 3); at anything else 18, by him within 6 (straight up when he faces
the jumper within 0x100, `FUN_0018d1f8`) or 3 from the jumper toward him.
Then +0x40 the velocity, +0x64 `FUN_0019bed8`, the meet (at least 15,
`fps / 4`) to the channel's +0x20, and the face (`FUN_00159190`, two
draws).

**The block test** (`FUN_0016d888`, raw `0x16d888..0x16e82c`), every
update of bit 12 unless `FUN_0015da28` bars him first (+0x1a8 above 0; a
gamebreaker shot coming down or from inside the arc, `0x3d658c` 2, bars
every actor; a jumper behind his own board within 4.5 of the offence's
rim): nothing from behind the backboard (`FUN_0016e830`); the clip's state
must have a hand (0xc) and 0x240. The hands (`FUN_0014da20` flag 1) within
`max(r, 1.0)` of the ball (1.0 more for a person's jumper on Easy, 1.5 at
a pass to oneself; 0.5 less for the computer's against a person's side on
Easy), or the limb sweep's forearm segments (14..15 the left, 18..19 the
right, by the clip's hands) touch it. Then a ball on the rim (6) within
0.793225 of its middle across is left alone, outside it taken on `r %
100` at most `(2 Rebounds + Blocks + Speed + Power) / 5` (5..90), else he
is barred for his airtime; a shot more than 70% flown (not at a holder,
nor bit 19) is let go `(progress − 0.5) × 200` in 100 (20 less for a
person's jumper on Easy), barring him; a gamebreaker's ball coming down is
let go. A touch halves his rise and draws the faces (`FUN_00159190`: his,
the owner's by `r & 1`). The turbo block's clip (437 with +0x360), 462,
463 and the glass pair (bit 19) catch the ball (`FUN_0014cb08`). Every
other touch knocks it loose (`0x16e318..0x16e6e0`): along his facing word
less `(r & 0xff) − 0x80`, half a turn more on 438, 439, 444, 445 (from in
front), a quarter more on 452 and 458 and less on 453 and 459; 0.35 an
update across and 0.125 up (a quarter of it on 437); the jumper and the
owner barred `r % 15 + 12` updates or their airtime left; spun about a
random axis (three draws) at `0.075 + u × 0.075`; free through
`FUN_0014c408`, which posts the block (slot 16) when the knocker's side
is not the one with the ball and it was a shot or in a shooter's,
dunker's or leaper's hands (else slot 21), keeps a ball off the rim a
rebound (6 to 0) and makes any other loose. Without a hand's touch the
limbs deflect it (`FUN_00137ed8(0.15)`, the owner barred as above): at a
free ball, and at a held one only for a jumper with Blocks 65 or more over
the owner's Dunks while the owner dunks or leaps (bits 7, 15). No
goaltending violation: "goal tend" is the computer's behaviour
(`FUN_00116fb8`), and these rules (the ring, the late-shot bar) are all the
original has.

**The dribble** (`FUN_0014e060`, raw `0x14e060..0x14f69c`, every update
the ball is held). The holder's clip marks the frames the ball leaves the
hand (state 0x10) and comes back (0x20). On a frame with no hand bit, or
between frames from a release frame, the ball is between the release
frame at or before the clip's frame (searched back round the cycle) and
the catch frame after it: both hand points posed at their frames, each at
its own root (ball+0xb0, +0xd0) laid where the holder stands now, the
ball `t` of the way across (the clip's way from one to the other), its
height straight from the release hand down to the radius and up to the
catching hand, the floor met at `dA / (dA + dB / 0.8)` of the way (0.8 the
floor's restitution, ball+0x138). Records 302/303 go hand to hand; 222,
223, 300, 301 (270, 271 to frame 23; 252, 253 to frame 44) keep it on the
floor. Otherwise the hands hold it (0xc both, else 8 the right, 4 the
left). The live hand +0x25c follows the nearer end's frame; ball+0x40 is
the holder's velocity, which is what `FUN_001537d0` predicts a held ball
falling from.

**Port.** `street_sim::block`: `SCRIPTS` (fifteen, with the goaltend
pair), `KINDS`, `kind(&Scene)` (`World::block_scene`), `aim(Bounds, ..)`
and `rises` (callback 35), `World::start_block_jump`, `new_block_jump`,
`block_jump_tick` (the turn, callback 35 at its tick), `block_aim`,
`block_test` (run before the jumper's step: the bones are the last
update's, as the played captures' knocks show), `block_knock`,
`block_deflect`; `Action::Jumping`'s `block` (snapshot 41), `Player::rise`
and `Player::barred` (actor+0x1a8, counted down each update and read by
the floor pickup), `World::credit_block` (slot 16, shared with the
stand-in); the rebound jump's kind 6 starts a block jump. `street_sim::
dribble::ball_point` places a gait holder's ball (`World::step_ball`,
`held_ball_point`); the swipe and the block read the ball where the last
ball update left it. `street_game::channel::block_shown` draws the jump
from its own clock. Without the disc's animations (online) the stand-ins
stay: `World::swat` at the release and the slam, `check_goaltend`, the
rewrite's jump.

**Checks.** `ai::capture::the_captured_block_jumps_follow_their_scripts`
(both attract captures: 63 and 24 block jumps; the takeoff on the
script's callback 35 tick 59 and 22, the rest cut short; the facing
settled on callback 5's angle 62 and 22; the script of the kind computed
from the captured state, every rating 50, 60 of 63 and 24 of 24, the
three others the holder letting go in the same update; callback 48's arm
18 of 18 and 5 of 6 at a free ball, at chance at a held one, whose
prediction the capture does not keep). `played_capture::`
`the_block_jumps_rise_as_callback_35_aims_them_in_play` (pads-1, pads-2,
fades-1: 50 aims at a ball in the air, the rise within 0.02 in 46, exact
at the top for Blocks 21, 45 and 80, the created players whose ratings the
capture lacks rising as Blocks 60; the drift in 40);
`the_blocking_hands_reach_the_ball_where_the_original_knocks_it_in_play`
(103 block jumps, 14 knocks while one is up: 8 by the jumper's arms, 4 on
the first update the last update's posed hand or forearm reaches, one an
update and one three after it, two at 0.94 and 1.08 against reaches of
0.5 and 1.0; six with his hands 3.5 or more away or still blending, the
contacts' knocks); `the_held_ball_follows_the_originals_dribble_in_play`
(the twelve played captures: 10,409 held updates in the hand, median
0.015 from the original's ball; 8,389 in flight, median 0.012, 8,372
within 0.5, where the ball always in the posed hand was 1.75 off, 1,531
within 0.5; the rest clips whose hand is off in the hand too, 86, 87, 47,
154, channels the pose does not blend);
`tests/script_timing_disc.rs::the_block_jump_lists_and_aims_are_the_executables`;
`tests::blocks` (the jump's ticks and turn, the knock and its credit and
bars, the ring, the turbo block's cost and catch, a snapshot mid-jump,
the dribble's path).

**Before and after** (the rewrite's computer games, means over eight
seeds of each capture's length; before is main at `c5f1656`). The
rewrite's `block` is slot 16's (shots and shooters' or dunkers' held
balls knocked by the defence), `deflection` the other knocks and the
limbs' deflections; `other loose` the held balls the stand-in swatted at
the release.

| capture | | original | all-50 before → after | disc pairs before → after |
|---|---|---|---|---|
| 1 | blocked shots (slot 16) | 3 | 5.5 → 3.25 | 4.75 → 4.53 |
| 1 | blocked passes, deflections | 2 | 0.25 → 0.5 | 0.78 → 2.44 |
| 1 | strips | 5 | 3.1 → 2.4 | 3.16 → 3.03 |
| 1 | other held balls loose | 0 | 3.0 → 0 | 2.69 → 0.38 |
| 1 | turnovers out of phase 5 | 6 (+2) | 5.75 → 5.5 | 5.88 → 4.22 |
| 2 | blocked shots | 1 | 2.0 → 1.0 | 1.63 → 1.81 |
| 2 | blocked passes, deflections | 1 | 0.25 → 0.25 | 0.25 → 0.38 |
| 2 | strips | 2 | 1.25 → 0.75 | 1.34 → 1.03 |

The spacing figure did not move (0.321 → 0.335 and 0.334 → 0.328 all-50,
the pairs 0.33..0.35 both). One game each (the comparison test's): first
capture all-50 blocks 5 → 4, strips 5 → 0, disc pairs blocks 5, 4, 6, 4 →
6, 7, 1, 5; second capture all-50 blocks 1 → 2.

**Left, labelled.** A held ball is posed only in the holder's gait; in a
shot, dunk, lay-up, trick or pass it is still the carried point, so a
shooter's held ball (and callback 35's aim at a holder's head, his gait
pose's bone 21) is the rewrite's; the original's held-ball blocks of
jump shooters and dunkers (two of the first capture's knocks) need those
clips posed. The jump flies callback 35's velocity: the original's first
one or two airborne updates also carry the clip's planted-foot correction
(`FUN_00156f48` with channel flag 0x40000, about 0.28 higher on
`0x441db0`), not modelled. The glass pair aims as callback 35 does (`a9`
in outline). The facing's 46 an update is measured, not traced. The
shooter's own bar uses no airtime (the rewrite's jump shot has none).
The face clips (`FUN_00159190`) are only their draws; the turbo block's
"no turbo" message and the first miss's (+0x1d8) are not posted. The contacts' knocks
(the six of the played captures with no hand near) are the contact
reactions not yet ported. The drawn held ball is still the renderer's own
bounce (`view.rs::sync_ball`); the sim's dribble point is there to draw.

## Defence and spacing on the original's speed levels (2026-10-16)

Audit 1.6: the handler's nearest defender was within 4 in about a third
of the held front-court updates, against 0.262 and 0.201 in the two
attract captures. Measured first, by what the handler and the nearest
defender were doing (the captures' actor state words +0xa4, fronts and
AIPlayer+0x48 speed levels; the rewrite's actions, behaviours and
`Brain::motion`), then traced from the raw executable.

**Where the closeness was.** By the handler's state, share of the held
updates and the share of those with a defender within 4 (the first
capture, its second, the rewrite's all-50 games over four seeds of the
first capture's length, before this pass):

| handler | capture 1 | capture 2 | rewrite before |
|---|---|---|---|
| in no move | 0.50, 0.14 | 0.55, 0.08 | 0.51, 0.16 |
| in a trick (bit 10) | 0.30, 0.37 | 0.24, 0.32 | 0.29, 0.44 |
| dunking (bit 7) | 0.07, 0.69 | 0.07, 0.68 | 0.08, 0.80 |
| in a jump shot (bit 3) | 0.05, 0.25 | 0.05, 0.13 | 0.04, 0.36 |
| winding up a pass | none (state 4) | none | 0.07, 0.36 |

and the defenders running "defense" stood (level 0) in 38% and 40% of
these updates in the captures and sprinted (3) in 21% and 19%; the
rewrite's stood in 0.1% and sprinted in 93%.

**The run to the spot** (`FUN_00107c40`, raw `0x107c40..0x108074`). The
spot (`FUN_00108148`, the man's, blended with the helped man's by their
distances from the rim held to 3) and the distance `d` from the defender
to it, then:

- `d ≤ 3.5`, the man without the ball and standing (his stick's strength
  class `FUN_0013abf8`, actor+0x90, 0) and one of the defender's two
  nearest players (game+0x23c) within 3.5 of him standing within 3.5 of
  the spot (`FUN_00108b48`): `d = 0`;
- with a player to help on, `d` halved beyond twice the first level's
  reach (`0x107da8`);
- else standing within 3.1: `d = 0` when the defender is nearer the rim
  than a man who stands; otherwise (`0x107e4c`) a third off a man without
  the ball, no pass coming to him (`FUN_0013abb8`) and outside the paint
  (zone under 5);
- the level the first of `0x3c69e0`'s reaches (0.8, 1.2, 1.7, 200, each
  × `skill(3, 1.8, 1, 1)`) not under `d`; a post defender
  (`FUN_00181730`) on a man posting up (bit 11) with the ball within 4
  stands;
- the hold (node +0x30 counts up to +0x34, `fps × skill(1, 0.25, 0.25,
  0.15)`): a lower level applies at once (`0x107f98`); a higher one waits
  while `d` is within the current level's second reach (1.6, 1.9, 2.5,
  200; `0x107f40..0x107f60`), then rises a level at a time (from standing
  to 1, or 2 when it needs 2 or more) once the hold has run, which it
  then restarts;
- running, he steers at the spot (turbo at 3; `FUN_0013af58` with no
  target); on the update he comes to a stand he faces the man (the middle
  of the man and the helped one, `FUN_0013b2f0(.., 1, 0x39)`), and
  standing he lets go of the stick (`FUN_0013af58` at level 0).

The port kept the current level whenever the new one was lower, so once a
defender had run he never slowed; it faced the man every standing update,
and it had none of the first three rules. Ported as read.

**The defense update** (`FUN_00106490`, raw `0x106490..0x106a84`). Its
last steps are: nothing more in the counter stance (`FUN_00181828`, bit
16), a block jump (`FUN_001818b0(ai, 0)`, bit 12) or a swipe
(`FUN_00181650`, bit 6); else the counter look, the block, the reach and
the run, in that order. The port also stopped while the defender was off
balance or in a rebound jump; it goes on now. The matchup (every `fps ×
skill(0.25, 0.2, 0.14, 0.1)` frames) waits on a pass only while the ball
is in state 4 with no holder for at most `DAT_004b38f4` frames (`fps ×
0x3eaaaa9f`, 19; `0x106868..0x1068b4`); the port waited for the whole
flight and the windup. The spot (`FUN_00108148`, re-read for this pass)
is the port's: the post-up branch tests the man's front type 7, which no
captured behaviour has (the "post up" node is type 6), so only actor bit
11 posts and the sim never does.

**Actor state bits 3, 7 and 15.** Read from the captures' launches (ball
state 3 to 5) and runs: the attract captures' 30 jump shots from 15 or
more out carry bit 3 alone, their 29 dunks bit 7 (all within 6 of the
rim, up to 5.2 high), their 8 alley-oop finishes bit 15 and their
lay-ups bits 3 and 13; the played captures' 70, 23, 6 and 3 alike. Bit 16
is the counter stance (13 of the first capture's 17 counter presses set
it). `ai/court.rs` called bit 7 a jump shot, 15 a dunk and 3 a leap, and
`block.rs`'s `holder_bits` gave a jump shot 0x80 and a dunk 0x8. The AI
port calls these by function (`FUN_00181508` is bit 3, `FUN_00181670`
bit 7, `FUN_00181690` bit 13, `FUN_00181770` bit 15 with the ball, as
`offence::busy` has them), so the accessors are renamed to their meanings
and every call keeps its function: `FUN_00181c68` never passes out of a
dunk or an alley-oop leap (a jump shot or lay-up may be passed out of),
`FUN_001837d0` is a blocked dunk (`offence::dunk_blocked`, the post-up's
release), `FUN_00129d98`'s 6..9 window and bonus are for a dunk, the
scramble (`FUN_0012cbb0`) and `FUN_00182c58` read bits 7 and 15, and the
block jump's kinds are 1, 4 or 5 at a jump shot and 2..5 at a dunk; the
limbs' deflection (Blocks over the owner's Dunks) is at a dunk or a leap.
`Ctx::fallen` also names bit 16 (`FUN_0016e9b0`), which is the stance:
a lead, not changed here.

**The swipe's draws.** At the strip (`FUN_0016cf30`, raw
`0x16d318..0x16d3d0`) the holder's face is `FUN_00159190(holder, r & 1 ?
3 : 4, fps × 2)`, which draws again for its clip when its time is over 0
(always here), and a catch gives the swiper `FUN_00159190(swiper, 2, fps
× 2)`, one more. The port drew only the `r & 1`; `World::swipe_lands`
draws all three.

**What is left: the trick moves.** After the port the defenders stand,
walk and sprint as the original's (levels 0.38 / 0.12 / 0.23 / 0.27
against 0.38 / 0.16 / 0.26 / 0.21 and 0.40 / 0.16 / 0.26 / 0.19), and
away from the handler's tricks (30 updates on) the nearest defender is
within 4 in 0.23 (all-50) and 0.24 (disc pairs, 0.24..0.25 each) against
0.21 and 0.15 captured. In a trick he is within 4 in 0.62 against 0.37
and 0.32, more than before (0.44): the rewrite's `Action::Trick` creeps
along the stick (`Player::step`, `walk(input, 0.45)`) for a stand-in time
(`tuning::TRICK_TIME`), and a computer handler's stick points at the rim,
so 75% of his trick steps go at the rim, into the on-ball defender who
now stands on that line. The original's handler holds the stick at the
rim too (84% of his trick updates) but moves along it in only half of
them: his juke clips carry him sideways or back (43% of the steps at the
rim). A drive with a trick is within 4 in 0.70 against 0.38 and 0.45; a
drive without one 0.29 against 0.27 and 0.25. Two other stand-ins were
ruled out by switching them off for a run: the rewrite's fakeout stumble
(`World::fake`, `tuning::FAKEOUT_CHANCE`; 0.376 without it) and the
locomotion's placeholder deceleration (`tuning::DECELERATION` 45; the
original's defender stops in about 1.8 over 11 updates where the
rewrite's slides 2.6, yet 150 gave 0.376 and 0.359). The trick moves on
their scripts (clip lengths and root motion) are the open part.

**Numbers** (within 4 of the held updates, the ball in state 3; a trick's
share and its within 4; away from tricks; the defenders' speed levels 0
/ 1 / 2 / 3). The rewrite's are four seeds of the first capture's length
each, before (main at `b375556`) and after:

| | within 4 | trick | trick within 4 | away | levels |
|---|---|---|---|---|---|
| capture 1 | 0.262 | 0.30 | 0.37 | 0.208 | 0.38 / 0.16 / 0.26 / 0.21 |
| capture 2 | 0.201 | 0.24 | 0.32 | 0.150 | 0.40 / 0.16 / 0.26 / 0.19 |
| before, all-50 | 0.319 | 0.31 | 0.44 | 0.240 | 0.00 / 0.01 / 0.05 / 0.93 |
| before, disc pairs | 0.345 | 0.31 | 0.47 | 0.271 | 0.00 / 0.01 / 0.06 / 0.92 |
| after, all-50 | 0.362 | 0.27 | 0.62 | 0.232 | 0.38 / 0.12 / 0.23 / 0.27 |
| after, disc pairs | 0.359 | 0.28 | 0.59 | 0.244 | 0.36 / 0.13 / 0.24 / 0.27 |

The comparison test's own games: the first capture's length all-50 0.342
(trick 0.60, away 0.182, standing 0.358), the pairs 0.36..0.38; the
second's 0.384 (0.67, 0.221, 0.349), the pairs 0.33..0.38.

**The check** (`shots_turnovers_and_spacing_against_the_captured_original`).
The defenders' standing share within 0.08 of the original's (the port
before this pass: 0.001 against 0.376); within 4 away from the handler's
tricks within 0.09 (the whole figure's limit was 0.18); the whole figure
within 0.22, the trick moves' part. The harness counts the held updates
in state 3 as the capture does (`World::native_ball_state`: a pass's
windup is state 4) and reports the trick share, the away figure and the
levels on both sides.

## Trick moves on their scripts (2026-10-16)

Audit 1.6's open part (the section above): the rewrite's `Action::Trick`
crept along the stick for a stand-in time (`tuning::TRICK_TIME`), rolled
its fakeout at a fixed moment (`World::fake`) and took the self-toss's
launch from its own schedule. Traced from the raw executable (the decomp
cuts the press at `FUN_0030fa50`) and the played and attract captures,
and ported (`crates/street-sim/src/trick.rs`, its tables generated from
the executable's bytes in `trick_tables.rs` and held against them by
`crates/street-sim/tests/trick_disc.rs`).

**The press** (`FUN_0015f0f0`, raw `0x15f0f0..0x15f860`). A draw for the
handler's face (`FUN_00159190(actor, 2, fps × 2)`, `0x15f1e8`); the
nearest opponent (`FUN_00170dc0`) and the sector he stands in about the
facing the handler wants (actor+0xb0: the stick's while it is pushed).
The turbo buttons' mask (one of the eleven of `0x43d408`) picks a direct
slot, its script `FUN_0013a1f8(record, 1, +0x250, slot)` and its value
class (`0x43d498`'s first eleven for an ordinary player: scripts 20, 17,
19, 22, 12, 16, 56, 57, 63, 66, 69). Without one (`0x15f358`) the trick
is drawn at value class 1 when a turbo is held (+0x78 & 0x2000), else 0,
among the 75 rows of `0x3d7238` (12 bytes: the script, a legend's
record, the selection flags) that the ordinary row (`FUN_00139850`)
allows at that class and whose flags allow the defender's side (1 the
ball's, 2 the other; a defender dead ahead put to one side by
`FUN_0015f860`), standing (4) or moving (8), not posting up (0x10), and
not the last row drawn (`0x3d75bc`): `r %` the count. At `0x15f598` the
facing turns to the defender when he stands within 45° either side of
the wanted facing (`FUN_0018d768`'s quarter 2), else stays.
A value class of 2 or more draws once more (`0x15f7f4`) for the fumble
(`r % 100` against a chance from Handles and the class, actor+0x1d4; the
timed knock-loose `FUN_0015a060` plays is not modelled). The script
starts at once (`FUN_0019c2f8`).

**The scripts.** Every one of the 75 is the same shape: `9b 0x400` (bit
10), `8e` a blend of six or ten updates to the clip (`f8`: the live left
hand's alternate record), `97` (the velocity zeroed, `FUN_001947a0`),
`a6 3 3 n` (the fake window: contact kind 3, row 3 of `0x3d0d90`, for
`n` updates, counted down by `FUN_00156f48`; `FUN_00193660` writes
+0x198, +0x19c and +0x1a4), one wait (`8d ee F T`, the clip to frame F
over T updates; a few play out with `8c`), `9c 0x400`, callback 7 (the
facing from the pelvis, `FUN_00174e80` with `FUN_00154230`) and callback
1 (the gait back). The self-tosses (21, 23, 36, 60, 61, 69) add callback
53's launch and an `ef` wait on the ball's flight. The channel runs them
on `playback::Channel`'s clock (the press update is tick 0; the blend's
`n` updates; the wait's first step in the update the blend ends), so a
trick lasts its blend plus its waits (58.7 and 56.5 updates on average in
the two attract captures).

**The motion.** On the press update the gait still runs (it sets the
velocity); through the blend the actor coasts on that velocity, held
(every played-capture trick, 27 of 27). From `97` on the clip's root
carries him (`FUN_0019a658`): the change of the root translation, times
the model's height scale, turned by the facing word, added to x and z.
The stick no longer steers: the facing turns toward the trick's (46 an
update, as callback 5's) and the root follows it. At the end callback 7
takes the facing from the pelvis (in 26 of 27; the other one's gait
turned it toward the stick in the same update).

**The fake** (`FUN_001372d8`, contact kind 3 of the contact manager
`FUN_00133bb0`). While the window is open, every opponent within 6
(`FUN_00139408`, height included) who is not reacting or down, not in a
counter stance facing the handler, and not holding the ball is tested
with capsules (`FUN_00137868`, `FUN_00139198`): the handler's head, trunk
and legs (`0x3d0cf0`, `0x3d0d30`, each pushed past its end by
`FUN_00137ab0`) against the opponent's body, legs and arms
(`FUN_001392b8`, `0x3d1180`). A trick on one of twelve records never
fakes (the window closes). Only an opponent in the handler's front
sectors 3..5 is tried: `r % 100 ≤ lo + trunc((hi − lo) × Handles / 99)`
by the value class (`0x43d0e0`, `0x43d0f8`: 25..45 up to 45..85; 10 more
for a person's trick on the first difficulty) fakes him out (actor+0x218,
and a draw for the handler's face), else he holds. Either way the window
closes and the defender plays a reaction drawn from `0x3d0f18` (held) or
`0x3d0f48` (faked) by the quarter the handler stands in about him (four
lists each; the octant was wrong, its eight entries ran into the
screen's tables at `0x3d0f28`): 13 of the 15 captured reactions fall in
their quarter's list. A fakeout from in front on six step-through
records plays the step-through reaction. The reactions are scripts too
(`a1 81` hops along the facing with a rise, landing as `FUN_0019bed8`).

**The points** (`0x157dd0`): when bit 10 drops, in live play with the
ball still his; half the base unless faked (class 10), the full base and
the multiplier when faked (class 11); the meter ×0.7. The rewrite scored
at its fake moment partway through, so a trick cut short after it (a
steal, a pass) still scored.

**Played captures** (`the_trick_moves_play_their_scripts_and_clips_in_play`,
pads-1, pads-2 and fades-1: 27 tricks). The clock as the original's in
25 (one countered at its sixth update; in one the original's channel
stepped twice in an update at its 24th and still ended on the script's
update), the end in 26, the coast in 27, the facing in 26. The clip's
root within 0.01 in 695 of the 740 steps with nobody within 4.5, along
the facing 41.96 against 39.57; between 2.6 and 4.5 only 353 of 545
(55.62 against 26.31): the original's contacts hold the handler back
from further out (its radii add the clip's limbs, `FUN_001337e0` with
`0x43d060`), and within 2.6, 54 of 87.

**Attract captures** (within 4 of the held updates; within 4 in tricks;
away from tricks; trick length in updates; the handler's displacement
over a trick and its part toward the rim). The rewrite's all-50 games of
each capture's length, before (main at `a038434`) and after:

| | within 4 | in tricks | away | length | moved | toward rim |
|---|---|---|---|---|---|---|
| capture 1 | 0.262 | 0.372 | 0.208 | 58.7 | 6.45 | 4.74 |
| capture 2 | 0.201 | 0.324 | 0.150 | 56.5 | 5.78 | 3.93 |
| capture 1 length, before | 0.342 | 0.601 | 0.182 | 46.4 | 6.91 | 5.63 |
| capture 2 length, before | 0.384 | 0.666 | 0.221 | 46.0 | 6.66 | 5.19 |
| capture 1 length, after | 0.348 | 0.530 | 0.251 | 57.1 | 4.84 | 3.07 |
| capture 2 length, after | 0.321 | 0.486 | 0.242 | 56.8 | 5.10 | 2.61 |

The first capture's whole figure hardly moves (0.342 to 0.348): within 4
falls in tricks (0.601 to 0.530) but rises away from them (0.182 to
0.251); the second's falls (0.384 to 0.321). With the disc's teams (four
pairs) the first capture's length gave within 4 0.28..0.35 (0.36..0.38
before), in tricks 0.47..0.56, and the handler moved 5.7..6.5 over a
trick (toward the rim 3.7..4.3). Through the blend the original's
defender backs off (his distance to the handler grows 0.41 and 0.39 over
a blend in the two captures) where the rewrite's closes in (−0.21 and
−0.26 all-50, −0.06 to −0.47 with the disc's teams); the handler covers
about the same ground (1.52 and 1.58 against 1.52 and 1.78). The
original's 107 tricks met 93 reactions in the first capture; the
rewrite's 144 met 86 (94 scored, 43 of them faked).

**Shots.** The trick's length moves the shot count: with the disc's
teams the first capture's length drew 64.8 shots (60, 68, 67, 64) while
the tricks ran the stand-in times, 55.3 (55, 60, 54, 52) on their
scripts, against the original's 55; the second's 22.3 (21, 24, 21, 23)
and 19.8 (17, 22, 23, 17) against 22. A trick is about a second of a possession the handler does not
shoot in.

**The check.** `shots_turnovers_and_spacing_against_the_captured_original`
holds the trick length within 5 updates, within 4 in tricks within 0.2
of the original's, away within 0.10, the whole figure within 0.15 (was
0.22) and the disc teams' shots within a fifth; the played test holds
the clock, end, coast and facing in 90% of the tricks and the root of
the open steps within 0.01 in 90%.

**Not ported (leads).** The defenders' locomotion: the original moves a
player along his velocity, which the stick sets (41% of the moving
defenders' updates go more than 30° off the facing: shuffles and the
backpedal), where the rewrite's walk moves along the heading; and the
defensive stance's gait clips. The contacts' limb radius
(`FUN_001337e0`, table `0x43d060`). Both since ported ("The defenders' gait
on the original's locomotion"). The trick fumble (actor+0x1d4 and
`FUN_0015a060`'s timed knock-loose; only its draw is kept). The turbo a
trick spends (`tuning::TRICK_TURBO`) is the rewrite's own. The legends',
created players' and generic team's slot overrides are not in the sim
(the ordinary row stands in). The post-up's trick (74) and the specials
at `0x44cac2` and `0x44cade` are not reachable (the sim never posts up).
`street-assets`' `scripts.rs` read the trick rows from `0x3d7234`, so
its state mask was the previous row's flags (the script and legend were
right; fixed since). Without the disc's animations the root is a stand-in (each
script's net root spread over its updates after the blend) and the
capsules a floor distance of 3.5 (the median of the captured contacts).

## The defenders' gait on the original's locomotion (2026-10-16)

Audit 1.6's rest after the trick moves (the section above): the
handler's nearest defender within 4 in 0.348 and 0.321 of the held
front-court updates against 0.262 and 0.201, 0.53 and 0.49 in a trick
against 0.37 and 0.32, and the defender closing in through a trick's
blend (−0.21 and −0.26) where the original's backs off (+0.41 and
+0.39). Traced from the raw executable (the decompile cuts
`FUN_00163d40` at its calls to `FUN_0030fa50`), ported
(`crates/street-sim/src/guard.rs`, `Player::walk`, `contact.rs`) and
measured against both attract captures and the played captures.

**The moving path** (`FUN_00163d40`, raw `0x163d40..0x168a10`). With
the stick's strength (actor+0x90, capped at 64) at 16 or more the
locomotion picks the gait record for the family the strength asks for
(the hysteresis of `0x167480`, the thresholds from the playing record's
flags) and then its speed, +0x190 = the record's motion range at the
speed rating (a run) or the strength over 64, times `1.25 − rating ×
0.1` with turbo held; it copies the stick's angle (+0x88) to the wanted
facing +0xb0 (and +0xb4) and sets the velocity +0x40 = −speed × (sin,
cos) of +0xb0 at once (`0x167d88..0x167db4`). The actor's update
(`FUN_00156f48`) adds the velocity to the position, and its vtable
+0x64 (`FUN_00168ea8`, at `0x43f5cc`) then turns the facing +0xa8
toward +0xb0 by at most 46 units an update (34 while the channel plays
a root-motion clip, flag 0x80000; 23 posting with the ball or in a dunk
or lay-up; 11 on the post defence's rows within 57) and keeps the last
twelve facings (+0x2e8). So a player moves along his stick from the
update he pushes it, whatever way he faces; the attract captures'
facings turn 46, 34, 14 (the computer's smoothed stick) or less an
update. The rewrite's walk turned the heading toward the stick at 12
radians a second, gathered speed at 60 units a second a second and
moved along the heading. Now `Player::course` is the velocity's way
(the stick's), `Player::wanted` is +0xb0, the speed is the record's at
once, and the record is picked before the walk (it was picked after the
step, an update late; `World::select_gait`). With the stick let go the
stand path sets no velocity (the stance and idle clips' roots carry the
original's player to a stop); the rewrite still slows him at
`tuning::DECELERATION`, now along his course. The walks while a pass
winds up and a swipe reaches are the rewrite's own (the locomotion does
not run in those scripts) and keep the old turning walk.

**The guarding mode** (`DAT_003d78e0`, raw `0x167220..0x1673f0`,
`0x16784c..0x167b50`, `0x167b70..0x167e20`). A player of the side
without the ball (actor+0x194 is 2, `FUN_00156f48`), outside a restart,
with no turbo bit held and someone holding the ball, guards him when
the holder is within 15 (`FUN_0018d0c0`), does not face away from the
rim he attacks (`FUN_0018d290`'s fold from his +0xb0 under 0x1e4), and
the guard stands in his own half and between him and the offence's rim
(`FUN_00170840(holder, guard, 100)`: within 100 units of the holder's
angle about the rim and no farther from it, game+0x374). Once on it
holds 36 updates whatever the places say, and once off it stays off 36
(+0x2e4 and +0x2e0 stamp the start and the end; `DAT_004b99c8` and
`DAT_004b99cc` are both `(int)(fps × 0.6)`, written at boot,
`0x178e2c..0x178e84`); letting the stick go forgets both. The record
goes by the stick's angle about the direction to the holder (`rel`,
`0x16786c..0x167b38`): within 97 (`(int)97.21083`) the shuffle forward
(0x2020) while the holder stands or in the mode's first 36 updates,
else the mode ends and the ordinary gait plays; 97..415 and 609..927
the shuffles to either side (0x2008, 0x2010); 415..609 the shuffle back
(0x2040) while the holder stands or for 24 updates after a jog
backwards (`(int)(fps × 0.4)`, +0x334 and +0x338), else the jog
backwards (0x2080). These are the gait records 0..6 (`0x4b8790`:
`shuffle_side_r_1`, `shuffle_front_l_1`, `shuffle_back_l_1`,
`jogbackwards_1..3`, 0.08 to 0.30 or 0.267 an update by the strength).
The velocity still follows the stick; the wanted facing becomes the
direction to the holder (`FUN_0016f508`), held by `FUN_00168a10` after
the mode's first update when it is a turn of under 58 against a facing
whose last seven updates changed direction more than once. So a guard
shuffles and backpedals facing his man, where the rewrite's walked
along its heading (`World::guard_stance`, `guard::gate`,
`guard::record`, `guard::steady`).

Measured. In the attract captures the defenders with the stick out
play the guarding records in 12% of their updates (the capture's row is
a blend's source for its first updates); classified by behaviour (the
facing at the holder against at the stick, the stick to his side or
behind him) the rewrite's guards face the holder as often as the
original's (to the side 68% against 69% and 63%; behind 71% against
59% and 49%). In the played captures, where the computer guards the
person's holder (the person's own stick is idle whenever his side
defends), `the_guards_move_along_their_sticks_and_face_the_holder_in_play`:
the velocity within 2 units of the stick in 29,753 of 29,768 moving
updates, the ported mode against the rows the channel plays or blends
to in 94.5%, and +0xb0 within 32 of the holder in 90.6% of the 2,541
updates with the mode on in the capture that carries +0xb0.

**The reaction to a trick.** Bit 10 is read through `FUN_001819f8` (a
trick, or a juke asked for with turbo to spare) by the spot
(`FUN_00108148`: with the three-second sag, at least 11 off a juking
handler and the level's hold cut to `fps / 10`) and the counter look
(`FUN_00107710`), both ported before; nothing else in the defence reacts
to it. The original's defender backs off through the blend because he
guards along his stick facing the holder and because the contacts reach
with his head.

**The contacts' limbs** (`FUN_00133b48`, `FUN_001337e0`, the table at
`0x43d060`). The contact manager (`FUN_00133bb0`) tests a pair at a
floor distance up to the sum of their radii and `FUN_001343e8` pushes by
the sum less the distance. Each radius is 1.25; out of the tip (game
phase 1), toward a player within a quarter turn (0x101) of the actor's
+0xb0, each limb that counts is the bone pushed 0.5 past itself along
the line from the pelvis (`0x133a50..0x133aa8`: normalise, scale, add;
short of it for −0.5), and one that comes nearer the other player than
the last taken widens the radius to its floor distance from the pelvis.
The limbs: the head (bone 21, +0.5) always; the hands (19 and 15, −0.5)
in a swipe (bit 6); the forearms (18 and 14, +0.5) on the post
defence's rows (0x18e, 0x18f, 0x2cc..0x2cf, which the sim never plays).
A juking handler leans his head toward his man, so the original's
contacts hold him off from further out: before the limbs, 31% of the
rewrite's trick updates had the nearest defender at 2.25..2.75 (the bare
radii's 2.5) against 5% captured. Ported as read (`World::reach`,
`World::radius_toward`); without the disc's animations the radius stays
1.25.

**Numbers** (within 4 of the held updates; in tricks; away from them;
the defender's change of distance through the blend; the shots). Each
capture's length, every player at 50, main at `48b938e` and after:

| | within 4 | in tricks | away | blend | shots |
|---|---|---|---|---|---|
| capture 1 | 0.262 | 0.372 | 0.208 | +0.41 | 55 |
| before | 0.348 | 0.530 | 0.251 | −0.21 | 51 |
| after | 0.266 | 0.390 | 0.198 | +0.51 | 47 |
| capture 2 | 0.201 | 0.324 | 0.150 | +0.39 | 22 |
| before | 0.321 | 0.486 | 0.242 | −0.26 | 19 |
| after | 0.233 | 0.327 | 0.196 | +0.54 | 15 |

With the disc's teams (four pairs) the first capture's length gives
within 4 0.271..0.287 (in tricks 0.37..0.42) and 56, 61, 56, 58 shots
(55 captured); the second's 0.259..0.293 and 20, 24, 18, 20 (22). Over
300,000 updates at 50: within 4 0.271 (main 0.323), in tricks 0.407
(0.518), away 0.202 (0.216); with the pairs 0.28..0.30 (main
0.32..0.35) at 29.5 shots every 20,000 updates (main 28.2; the two
captures together 28.0). Twelve runs of the first capture's length give
0.270 (sd 0.021), twenty-four of the second's 0.280 (sd 0.024,
0.232..0.321).

The first capture's players run at speed ratings of about 30, 90, 80,
70, 75 and 70 (the median running step, 0.30 + 0.0668 × rating / 99 an
update), the second's about 80, 65, 75, 75, 85 and 80. With the first's,
300,000 updates give within 4 0.261 against 0.262, in tricks 0.376
against 0.372, away 0.202 against 0.208, the blend +0.42 against +0.41
and 26.9 shots every 20,000 updates against 26.9. With the second's
they give 0.264 against its 0.201: the ratings do not make that match.
Its handlers hold the ball 30 or more out for 26% of the held updates
(the first capture 18%, the rewrite 22%), where nobody stands within 4;
the two original matches differ by 0.061, more than any of the
rewrite's configurations moves the figure. The second match's own
shape (its teams beyond the speed ratings, which the capture does not
carry, and its plays) is the rest. A lead for the next pass: at 24..30
out the rewrite's nearest defender stands about 0.7 nearer the handler
on average than original's in both captures (6.6 against 7.3 and
7.4).

**The check** (`shots_turnovers_and_spacing_against_the_captured_original`)
now holds within 4 within 0.05 of each capture (was 0.15), in tricks
within 0.05 both ways (was 0.2 one way), away within 0.06 (was 0.10),
the standing share within 0.05 (0.08), the trick length within 3
updates (5), and the defender backing off through the blend, within 0.2
of the original's.

**Also.** The shot chooser and a pass's lead read the facing the player
wants (`FUN_0018d290` reads +0xb0); the rest read the facing. The
snapshot carries the course, the wanted facing and the guarding state
(version 43). `street-assets`' `scripts.rs` read the trick rows from
`0x3d7234`, so each row's state mask was the previous row's flags; it
reads `0x3d7238` {script, legend, flags} now.

**Not ported (leads).** The stand path's stance (`FUN_00163a38`: a
guard standing near a holder starts `0x457377` and faces the nearest
opponent; `FUN_0015da28` turns a guard on the stance rows to him) and
the stop's clips (the stance, idle and turn scripts' roots; the
rewrite's deceleration stands in). The post-up's backdown record
(0x2100) and the post defence's rows. At walking strength the
original's handler barely closes on his defender through a blend (0.03
to 0.10 an update: the post-up's backdown, the fancy dribbles that need
Handles the harness's players lack, a paused carrier) where the
rewrite's walks in at 0.18. The 24..30 band above.

## The ball on the bones (2026-10-16)

Audit item 5 (section 2): where the ball meets the actors' bodies and the
court. Traced from the raw executable (the side with the ball's actor
update, `FUN_0015a060`, was never decompiled: Ghidra stops at its first
instruction; its tail was read from the bytes), ported
(`crates/street-sim/src/touch.rs`, `touch_tables.rs`) and measured
against both attract captures and the played captures.

**Correction.** "Pass flights and the tip-off" said the receiver's
update (`FUN_0015da28` → `FUN_0015c598`) catches the pass. It does not:
`FUN_0015c598` is the dive's catch (`FUN_0016eab0`: channel 0's record
71 or 72), and `FUN_0015da28` is the update of the side without the
ball. `FUN_00156f48` dispatches the actor's update by his role
(actor+0x194, `0x1571fc..0x157268`): 0 the jump ball (`FUN_0015eaa0`),
1 the side with the ball (`FUN_0015a060`), 2 the other side
(`FUN_0015da28`), 4 the referee (`FUN_00159fd8`); `FUN_00169138` gives
the catcher's side role 1 and the other 2 at every take.

**The tails.** Both run in the actor's update after its press dispatch
and before the bars count down (`0x157300`: +0x1a8 and the cooldown
+0x1cc) and the actor moves (`0x1573e0`, position += velocity), so they
read the ball's last move (ball+0xa0) against his bones. His channel has
stepped before them: the played captures' standing catches come on the
first update the catch clip at that update's frame, at his place before
he moves, meets the ball (below). Both return at once for state bit 14
(0x4000), a barred actor (+0x1a8; the channel's flag 0x40 bars him, a
gamebreaker's shot coming down bars the other side's), a held ball or
the pickup clip (record 73). The offence's (`0x15ba14..0x15c568`):

- a rebound jumper's catch (`FUN_0016ed18`), a leaper's or dunker's
  hands within 1.0 (bits 0x8280, `0x15bad0..0x15bd68`), the dive's
  catch; with any of bits 0x8280 and the pass still in flight, nothing
  more;
- a loose ball (1) or the toss (2): `FUN_0016cd28` and on the floor,
  the take (`0x15c3c8`); on the rim (6): nothing;
- a pass (4): the passer of a pass not to himself leaves it unless his
  cooldown runs; any other state but a shot (5) or a rebound (0) only
  while his cooldown runs;
- unless he is in a rebound or block jump, his catch row (bit 9) or a
  leap (bits 8, 9, 12, 15, `0x9300`) or a dive: within `FUN_0016cd28`'s
  reach, the take;
- the sweep (`FUN_00137db8`): met on the floor by a rebound, the take; a
  pass's receiver (ball+0x100) takes it when the limb met is the forearm
  and hand whose hand the clip's state holds out (the hit's second bone
  0x13 with state 8, 0xf with 4, `0x15c2fc..0x15c36c`); his own pass met
  by his head (0x15) is left alone; every other limb knocks it off
  (`FUN_00137ed8(0.15)`) and starts his cooldown if it is not running
  (`DAT_004b99b8`, `fps / 6` = 10).

The defence's (`FUN_0015da28`, `0x15e3bc..0x15e8d4`, after its swipe,
rebound and block tests): nothing in the restart's phase 6; a loose ball
(1, not the toss) within `FUN_0016cd28`'s reach on the floor, the take;
on the rim, nothing; a pass more than three quarters flown, `r % 100 <
trunc(Steals / 99 × 15)`, the sweep, a knock (already ported, now in the
actor's update); a shot or a rebound (or anything while his cooldown
runs): the sweep, the take of a rebound met on the floor, else the
knock. So the defence never catches a pass, and a body knocks a shot it
meets. In phases 6 and 9 both sweep a free ball and knock it only while
the actor is barred or cooling down.

**The pickup's reach** (`FUN_0016cd28`): the ball within 3 of him across
(y zeroed, `0x16cd4c..0x16cdac`), and no higher than his highest bone (the
head 0x15, the hands 0x13 and 0xf) nor lower than his lowest (the feet 4
and 8) by more than 1. A ball at most 2.5 up and falling or rising at
most 6 mph a frame, taken by an actor on the floor not already in the
pickup clip, without the channel's 0x40 or bit 2, is scooped instead: it
goes free toward him at a fifteenth of the gap an update (its own vy
kept, `FUN_0014c408`), he faces it (`FUN_0016f2b0`) and plays
`0x45748a` (record 73 `standpickupball`: a 3-update blend, frame 7 over
10, callback 0 = `FUN_001775d8`, the take if nobody holds it, then frame
13 over 6). The first attract capture's 56 pickups of a loose ball
include one scoop.

**The take and the free ball.** `FUN_0014cb08` (`FUN_0014cb28`) gives
the ball; `FUN_00169138` the roles. `FUN_0014c408` frees it with an
actor: a shot (5) or a pass is loose (1), a ball on the rim a rebound (6
→ 0), a rebound stays one; it records him as his side's last on the
ball (ball+0x104 + 4 × side), his side's points kind (+0x15c + 4 × side)
2, and posts the block (slot 16) when his side does not have the ball
and it was a shot, else slot 0x50.

**The catch rows.** The table `0x3d75c8` (20 bytes a row) adds to the
script, the shortest flight and the flags the clip (+0x0c) and its catch
frame (+0x10): `relaxcatch01_mid` 10, `relaxcatch02_mid1hand` 11,
`relaxcatch04_high1hand` 15, `relaxcatch06_low1hand` 11,
`readycatch05_low` 6, the `jogcatch` clips 17, the post catches 7..16,
the passouts 5 and 7. Every clip holds out its row's hands on every
frame (state 0xc both, 8 the right, 4 the left). The release's catch
draw (`FUN_001611d8`, `0x1624c0..0x162a5c`) poses the receiver in the
row's clip at that frame (`FUN_0019bda8`), standing at the spot (or the
lead point), facing the ball on the rows with flag 0x10000
(`FUN_0016f2b0`) and his wanted facing (+0xb0) on the others, his pelvis
moved by the clip's root at that frame turned by the facing at his height
scale (a moving receiver keeps only its height, `0x1625bc..0x162740`),
and keeps his right hand (bone 0x13) on flag 0x400, his left (0xf)
without, their midpoint on 0x100, at `DAT_003d6590`: the point the pass
is thrown at (`FUN_0014c0c0(ball, &DAT_003d6590, flight, bounce)`,
`0x173ac4`). The row's script (39, all compiled along their plain paths
into `touch::CATCH_SCRIPTS`): the standing rows `97` (the velocity
zeroed), `9b 0x200` (bit 9), callback 5 on some (+0xb0 to the ball), a
6-update blend to the clip, `8d ee F fd` (the clip to the catch frame
over the rest of the pass's flight, ball+0x64 − +0x60), a play-out or a
fixed wait, `9c 0x200`, callback 1; the running rows callback 30
(`FUN_00172110`: channel 0's flag 2) with 1 instead of `97`, the same
bits, blend and flight wait, then callback 42's test and callback 30
with 0. The running rows' clip does not show on channel 0 in the played
captures (it keeps the run, flag 2 set): it plays on a channel they do
not keep. The stick no longer steers a receiver on any row: the played
captures' moving receivers keep the velocity they had at the release to
the catch in 67 of 74 (within 0.04 of the straight line, median).

**The release's bars** (`FUN_00173570`, `0x173810..0x17391c`): the
passer and his teammates other than the receiver (+0x1a8 = the flight),
the receiver's bar lifted; with no waypoint (game+0x160 −2), the passer
on the floor, the receiver not leaping and 15 or more away
(`FUN_0016e948`), the other side for `trunc(flight × 0.35)`. Callback 21
(the shot's release, `0x172a44..0x172a64`) bars the shooter `fps / 3` =
20 updates. A pass's timer (ball+0x158) is its flight and
`DAT_004b84b0` = 5 more (`FUN_0014c0c0`); at 0 it is loose.

**The court's bound** (`FUN_0016f900(actor, 0)` from `0x157a04`): in
play an actor who has been within ±40.5 by ±25 (`FUN_0016f530`; the flag
actor+0x324, cleared in the restart's phase 6 unless bit 2 or the
channel's 0x40) is held within ±(40.5 + 0.4 y) by ±(25 + 0.4 y), y his
height. The lead point of `FUN_001636c0` slides along the same ±40.5 by
±25.

**Section 2's leftovers.**

- The board-disable record (`FUN_00148760(game+0x134, side)`): the trick
  passes' leg manager (`FUN_00148378`, the legs `FUN_001488e8` builds),
  its current or next leg of kind `side` (±1, the ball's side) aimed 13.25
  or more high: the backboard, the collar, the rim and the basket return
  0 while it holds (`0x1515f4`, `0x15197c`, `0x15246c`, `0x152738`), not
  read in the launcher's look-ahead.
- The reversed lay-up spin (`0x14ac20..0x14ac48`, `0x14b6fc..0x14b724`,
  `0x14be30..0x14be68`): a lay-up (bit 13) whose kind word +0x1e4 is 2
  (the row's family, `0x169c1c`) and whose kind (+0x26c, the row's id)
  has bit 0 in `0x43f6e0` (ids 3, 12, 13, 20, 22) leaves with the axis
  negated.
- A ball through the ring outside a shot (`FUN_001515a8`, `0x151850..
  0x15190c`): an armed ball (+0x1a8) down through it posts the basket
  (event 0x55) for the side that attacks that ring (ball+0x150 against
  `FUN_001404e0`), credited to that side's +0x104 player, its points the
  game's `FUN_0012e890`: 2 when that side's kind is 3 (a long shot's),
  else 1. Pads-1 holds one (a loose ball rolling in off the rim, phase 2
  to 6).
- The floor's pickup is the actor's own (above), an update before the
  rewrite's, which took a loose ball after the ball's update.

**Ported** (`street_sim::touch`, snapshot 44): `Catching` (the receiver's
row on its `trick::Run`, the script's `8d ee F fd` a `Step::Flight` whose
length is the pass's updates left; `World::start_catching` at the
release, `World::catching_tick` before the tests, `World::catching_coast`
after them), `World::catch_target` (the pass's point), `World::touch_ball`
(both tails), `World::pickup_reach`, `World::body_knock`,
`World::release_bars`, `World::contain`, `World::loose_basket`,
`World::layup_reversed` (`LAYUP_FLAGS`), `Player::cooldown`,
`Ball::touch`/`Ball::long` (ball+0x104, +0x15c by side), `Step::board_off`,
`shooting::SHOOTER_BAR`; `timing::CatchRow::record`/`frame`;
`passing::World::receive` (the catch's bookkeeping, shared with the
rewrite's arrival catch); the pass with a catch row flies on to its
timer. Without the disc's animations (online) the rewrite's own rules
stay: the arrival catch, the stand-in interception, `World::catcher`
after the ball's update, the wider court.

**Checks.** `played_capture::the_receivers_hands_meet_the_ball_where_the_original_catches_in_play`
(the played captures' 192 catches by their receivers; the 119 caught
standing with the clip's blend over, posed from the captured channel at
the update's frame, the place and heading word of the update before and
the roster body: the hand rule first meets the ball on the catch update
in 90, within one update in 104, where the last update's frame gives 52
and 64). `ai::capture::the_receivers_hands_take_the_ball_on_the_originals_timing`
(the original against four of the rewrite's games of each capture's
length); `tests/trick_disc.rs::the_catch_rows_scripts_are_the_executables`;
`tests/script_timing_disc.rs::the_catch_rows_hold_for_their_scripts_tail`
(the records and frames); `tests::touch` (the hands' early catch, the
coast, a knock off another limb and the cooldown, the floor's ball in
the actor's update, a rebound met on the floor, a shot knocked off a
defender's body and counted, the release's bars, the basket outside a
shot, the board-disable record, the reversed spin, the court's bound,
the snapshot).

**Before and after** (the flight left in the update before each catch
of a pass by its receiver, and the ball's other meetings; the
original's one match against four of the rewrite's computer games of
its length; before is main at 7792b6f):

| | original | before | after |
|---|---|---|---|
| attract 1: flight left 0 / 1 / 2 / 3 / 4 / 5+ | 2% / 44% / 36% / 9% / 8% / 1% | 3% / 97% / 0 / 0 / 0 / 0 | 1% / 49% / 33% / 10% / 4% / 1% |
| attract 1: passes lost loose at their end, a game | 1 | 23.3 | 2.75 |
| attract 1: loose pickups / rebound catches, a game | 57 / 14 | 65 / 11 | 45 / 11.25 |
| attract 1: knocks off a body outside a block jump, a game | 1 | 1 | 1.25 |
| attract 2: flight left 0 / 1 / 2 / 3 / 4 / 5 | 3% / 38% / 38% / 7% / 7% / 8% | 4% / 96% / 0 / 0 / 0 / 0 | 2% / 52% / 28% / 12% / 4% / 1% |
| attract 2: passes lost, a game | 0 | 7.75 | 0.75 |
| played (12 captures): flight left 0 / 1 / 2 / 3 / 4 / 5+ | 2% / 36% / 46% / 10% / 5% / 2% | | |

The lost passes before were the arrival catch's: the receivers' sticks
kept steering them through the flight, and a receiver more than
`tuning::CATCH_RANGE` from the point dropped it. The 2.75 left are
running receivers the computer's spot puts away from the ball's point
(a spot pass to a receiver already moving off it) or out past ±25.

**Left, labelled.** The running rows' clip is posed whole-body (the
original plays it on a channel over the run, which the captures do not
keep). The scoop (`0x45748a`) takes the ball at once. The dive's catch
(`FUN_0015c598`, the legs' knock) and the leaper's and dunker's hands
(bits 0x8280) stay the rewrite's own (their clips are not posed); a
rebound jumper's body knock in `FUN_0016ed18` too. Phases 6 and 9's
knocks, state bit 14 and the channel's 0x40 bar (the reaction clips
stand in) are not read. The court's bound applies only with the disc's
animations (the rewrite's 48 by 32.5 stays without them). The basket
outside a shot counts as a jump shot in the statistics (the original
passes the ball's release kind, +0x184). A receiver jumping or reacting
mid-flight drops his catch row. The flight left reads one update later
than original's for some catches (49% against 44% at one, 33%
against 36% at two).

**Corrected 2026-10-18** ("The loose ball"): `0x8280` is bits 7, 9 and 15, so the
receiver on his catch row (bit 9) takes the ball within 1.0 of his pelvis or
either hand before anything else in the tail and is not swept at all while
the pass flies; the tail reads the model the last update posed. The catches
above fitted to the forearm-and-hand rule are the near take's.

## The held ball on the holder's clip (2026-10-16)

Audit item 5, its second half: where a held ball lies through every move,
the live hand, the running catches, the scoop, the dive's and the
leaper's catches. Traced from the raw executable (`FUN_0014e060`'s raw
`0x14e060..0x14f69c`, the blend opcode `FUN_001978b0` and
`FUN_00193300`, the side with the ball's update `0x15a740..0x15ab4c` and
its floor take `0x15c3c8..0x15c564`), ported (`crates/street-sim/src/held.rs`,
`held_tables.rs`; `touch.rs`'s scoop) and measured against the played
and the attract captures.

**`FUN_0014e060`, every update the ball has a holder** (ball+0xf4; a
pass's windup included: the ball is in state 4 from the press with the
holder still set, 884 updates of pads-1, until the row's callback 38).
It reads the holder's channel 0, or channel 1 when its flags have 4, or
channel 2 when its flags have 4 and its +0x58 is -1 (no held ball of the
played captures shows channel 1's 4). The record is the channel's +0x50,
or in a blend (flag 0x200000) its target +0x54 (through the channel +0x58
links when that is set); the frame +0x5c and +0x64, or in a blend the
target's start +0x60. A dribbling record (a frame with 0x10) is the
dribble ("Blocks and the dribble on the original's bones"); any other
holds the ball in the hand its frame holds out, `FUN_0014da20` (flag 0)
on the model's bones as the channel shows them (blended in a blend): both
hands' midpoint on 0xc, the right on 8, else the left. So the ball lies in
the shot's, the dunk's, the lay-up's, the pass's, the trick's and the
catch's clip as surely as in the gait's. Its tail: ball+0xa0 the move,
ball+0x40 the holder's velocity.

**The blend's ease.** The blend opcode (`8e`) calls `FUN_00193300(channel,
1)` before its first step: with no ball held (actor+0x70) +0xb0 is 0;
with +0xb0 still 0, when the hands of the clip going out (+0x14, the
state of its last step) and of the target at its start
(`FUN_001927f0`) overlap and are the same or the target holds one, +0xb0
stays 0; otherwise +0xa0 is the ball's offset from the actor turned into
his frame (`ball+0x30 − actor+0x30` by the inverse of actor+0x50) and
+0xb0 is 1 (the opcode `92` stores it unconditionally). Through the
blend, with +0xb0, the ball update lays the ball at that offset turned
back and moved with him, times `1 − c/n`, plus the hand's point times
`c/n` (`0x14f2c4..0x14f40c`; `c` +0x70 the counter, 1 on the blend's first
update, `n` +0x6c its length). Waits (`8d`), play-outs (`8c`), free rates
(`ab`), `89`, `8a`, `a2` and the step that ends a blend clear +0xb0. The
offset is stored with the facing and the place as the update begins and
laid with the update's own (of the variants tried against the played
captures, the stored facing of the update before and the current one at
the ball update fit best: blends' medians 0.07..0.30 against 0.11..0.78).

**The live hand** (actor+0x25c), written by the same update while the
ball is held: on a dribbling record in the hand 2 on a frame with 4 and
else 1 (`0x14e490..0x14e4b4`), in flight the nearer end's frame's hand, 1
on 8 and else 2 (`0x14e6f4..0x14e718`); on any other record 1 on 8 alone,
2 on 4 alone, unchanged on both or neither (`0x14f194..0x14f1f0`). The
catch row's flags 0x400 and 0x200 set the receiver's to the right and the
left at the release (`FUN_001611d8`, `0x16256c..0x162590`). The played
captures' held updates outside a blend: the rule gives the captured hand
on 35,454 of 35,459 (the five in a dribble's flight).

**The releases.** Callbacks 21 (a shot's, a dunk's, a lay-up's) and 38 (a
pass's) launch the ball from where it lies, ball+0x30 as the last ball
update held it: the played captures' first free step after every pass
and shot is the launch velocity from the held ball of the update before
(to the launch's gravity step). So the release points are the clips'
hands (the dunk path already started from the held ball's place).

**The blends' alternates** (`FUN_001978b0`, raw `0x197974..0x197a58`):
`f8` the live left hand (actor+0x25c 2); `f2` actor+0xc0 2, set at the
actor's construction (`FUN_001563a8`, `0x156648`) when the record's field
13 is set: a left-hander (`street_assets::db`'s `left_handed`); `ec` his z
on the other side of zero from the side his team attacks
(`FUN_001404e0`'s sign: z × side below 0). Two scripts carry a second blend
before their release, without a selector, so the picks the press fixes
are the ones every blend before the release reads.

**The moves' scripts** (`held::SHOT_STEPS`, `FAKE_STEPS`, `MOVE_STEPS`,
`PASS_STEPS`, generated from the executable; `tests/trick_disc.rs`
reads them again): the 36 shot scripts along the held path (`85` of
callback 20 jumps) and the 26 with a check along the pump fake's, the 99
dunk and lay-up rows and the 93 pass rows, the play-outs' updates from
their clips. Left out of the steps: the planted foot (`93`..`95`,
`FUN_00194228`: the planted feet at actor+0x100/+0x104, their point at
+0xf0/+0xf8, the channel's 0x80000), the animation tests (`98`, `99`),
`9e`, the jumps' own (`9f`, `a1 f0`, `a9`), `a8`, `ac` and the contact
windows of kinds 1 and 2; a blend to `-1000` is the record on the channel
([`trick::CURRENT`]); one pass row's foot-synced start (`-9`, `0x454f5c`)
is frame 0. The simulation runs each from the press to the action's
script tick on `trick::Run`'s clock (the shot's gate stall holds it, a
fadeaway goes on from its shot's clip at the check), the first blend
from the clip the channel showed at the press (`held::ChannelStart`).

**Running catches.** `FUN_001611d8` parks the row's script at actor+0x1bc
(only while `DAT_003d70b4` is 0) and the side with the ball's update starts
it (`0x15a974..0x15ab4c`) once the flight left (ball+0x64 − +0x60) is at
most actor+0x1b0 (the flight, or the row's own count at +4: 20 for the
4r/4l rows), with him on the floor and his channel not barred (0x40):
moving (velocity not zero) with actor+0x1ac 1 (on the floor at the
release, the row without 0x800), channel 0 plays the run (`0x4446f4`
when +0x88 is below 0x28, else `0x443dc0`; his gait, `FUN_00163d40`, when
game+0x70b0 is set) and the row's script runs on channel 2 (`a2 = 2`);
otherwise on channel 0. The actor has four channels (`FUN_001563a8`):
0 and 1 the whole body, 2 bones 9..21 (+0x74 9, +0x7c 13: the spine, the
arms and the head), 3 bones 0..20; a script started on channel 2 copies
channel 0's state (`FUN_0019c320`), and every channel writes its own bone
range into the model (`FUN_001935c8`): a running catch is the run's
pelvis and legs with the catch clip's upper body over them
(`pose::Poses::overlay`). For such a receiver the release aims at that
overlay, channel 0's clip carried on by the flight times its rate
(`0x16231c..0x1623a0`, `FUN_0019bde0`). Not ported: posing running rows
on the overlay alone, against the whole-body aim, put 22% of the
rewrite's catches after the pass's end and knocked 62 passes off bodies
in four games of the first capture's length; the aim's overlay and the
run scripts (their callbacks 52 and 15 pick the branch) have to come
with it. The rows with their own count still start at the release.

**Callback 5's facing.** The standing rows that call it (rows 0 and
27..38) turn the receiver toward the ball at 46 units an update; the
rewrite's walk had replaced that facing with his course, so he kept the
facing he had. `World::catching_turn` keeps it. (The other rows leave
him the wanted facing his computer behaviour gave, +0xb0, which the
rewrite does not keep while a catch row holds him.)

**The scoop** (`0x15c3c8..0x15c564`, the defence's `0x15e8d0..0x15ea64`):
the floor's ball within `FUN_0016cd28`'s reach, at most 2.5 up and rising
or falling at most 6 mph a frame (`5280 / (fps × 3600) × 6`), taken by an
actor on the floor who is not already in the pickup clip (record 73) and
whose channel does not bar him (0x40, `FUN_0016ea70`) nor his state bit 2
(`FUN_0016ea30`): the ball goes free toward him at a fifteenth of the gap
an update keeping its own rise (`FUN_0014c408`), his facing turns to it
(`FUN_0016f2b0`), the sides' roles go to his (`FUN_00169138`) and his
channel plays `0x45748a`: a 3-update blend to record 73, frame 7 over 10,
callback 0 (`FUN_001775d8`: the take if nobody holds the ball, 13 updates
on), frame 13 over 6, callbacks 7 and 1. Ported (`Action::Scoop`,
`touch::Scooping`, `World::scoop_tick`); the roles wait for the take (the
rewrite's possession), and bit 2 is not kept.

**The dive's and the leaper's catches** stay the rewrite's own: the dive
(`Action::Dive`) and the alley-oop's leap (`Action::Oop`) are the
rewrite's moves, not their scripts, so the dive's catch (`FUN_0015c598`,
channel 0 records 71 and 72, the legs' knock) and the leaper's or
dunker's hands within 1.0 (bits 0x8280, `0x15bad0..0x15bd68`) have no
clip to pose; a ball held in a rebound or block jump, a dive or a leap is
posed on the gait's clip at his height.

**The display.** `street_game::channel::sim_shown` draws the simulation's
own channel for every scripted move (the alternates it picked), and
`view::sync_ball` draws the simulation's ball while the holder's channel
plays a script. The gait's drawn dribble is still the renderer's bounce
(the standing clip is the view's own).

**Checks.** `played_capture::the_held_ball_lies_on_the_holders_clip_in_play`
(the rule from each held update's captured channel, place, heading word
and roster body; a blend's opcode read from the executable at the
channel's pc), `the_persons_shots_and_passes_hold_the_ball_on_their_clips_in_play`
(each person's shot and pass replayed through `World::step` with the
disc's animations), `the_live_hand_follows_the_held_clip_in_play`,
`tests/trick_disc.rs::the_held_moves_scripts_are_the_executables`,
`tests::held` (the shot's ease and hand, the release from the held ball,
a left-hander's alternate, the live hand, the catch row's hand, the
scoop's take at its tick, the snapshot).

**Before and after.** The held ball against the original's, the played
captures' held updates by the clip shown (medians; before is the
rewrite's earlier rule: the dribble in the gait, the carried point at the
waist otherwise; 693 updates without a record and 1,201 blends with a
special start, the gait channel's own, not posed):

| clip | updates | after | before |
|---|---|---|---|
| shot | 2,714 | 0.013 | 5.25 |
| dunk or lay-up | 1,914 | 0.022 | 5.41 |
| pass | 2,399 | 0.031 | 2.40 |
| trick | 1,354 | 0.038 | 2.67 |
| catch row | 1,814 | 0.019 | 2.39 |
| gait (the dribble) | 25,552 | 0.014 | 0.014 |
| in a blend: gait, pass, shot, trick, dunk or lay-up | 3,945, 1,068, 714, 174, 368 | 0.072, 0.144, 0.301, 0.231, 0.743 | 1.58, 1.45, 2.08, 2.28, 2.09 |

Outside blends 29,804 of the 35,747 updates lie within 0.25 of the
original's ball (32,959 within 0.5; the earlier rule 23,442, all of them
the gait's), in blends 4,107 of 6,272. Through a blend by its progress:
0.14 early, 0.16 mid, 0.06 late (0.53, 0.44, 0.12 without the stored
offset). Through the simulation (each person's shot and pass replayed
from the press): shots 2,784 held updates, 0.045 against the shooter
(4.81 before), 1,450 within 0.25: the rest come while the original turns
its shooter toward the rim 23 units an update through the shot's blend,
where the rewrite faces him at once (`Player::start_shot`); 2.14 in the
court (the shooter's jump and drift are the rewrite's own); releases
0.016 from the original's against the shooter, 44 of 67 within 0.1 (1.29
before, none); passes 150 updates, 0.048 (2.59), releases 0.031, 5 of 7
within 0.1 (2.95). The release points of the
rewrite's computer games against the first attract match's (across, up,
ahead of the holder's feet, medians):

| | original | before | after |
|---|---|---|---|
| jump shot | 0.09, 9.58, 1.11 | 0, 3.00, 0.90 | 0.43, 8.68, 0.68 |
| dunk | 0.13, 9.22, 0.63 | 0, 3.00, 0.90 | 0.16, 9.16, 0.87 |
| pass | 0.51, 5.85, 1.30 | 0, 3.00, 0.90 | 0.51, 5.84, 2.82 |

Blocks of a held ball (per game of the first capture's length, twenty
games each, all-50 and the disc's pairs): the original's match knocks
three jump shooters' balls out of their hands with a block jumper up and
no dunker's; before, 0 and 0.01 shooters', 0.2 and 0.39 dunkers'; after,
0.25 and 0.69 shooters', 2.6 and 2.46 dunkers'; balls in the air 6.45 and
7.0 before, 5.25 and 5.6 after. Shots in the first capture's length
(twenty seeds): the pairs 58.7 → 55.1 (the original 55), all-50 54.2 →
50.5; the trick length 56.46 → 56.38 and the spacing within 4 0.266 →
0.272 (all-50) and 0.315 → 0.316 (the pairs), within their seeds' spread.
The comparison test's single game of the first capture now draws a trick
length of 54.85 (bound 4.5 from the seeds' 54.7..58.4), the second's
away-from-tricks spacing 0.224 (bound 0.12 from the seeds' sd 0.04).

Catches by the flight left (the first attract match against four games):
before 1 in 50%, 2 in 33%, 3 in 11%, 4 in 4%, 5 or more 1%; after 48%,
39%, 3%, 1%, 6% (the original's 44%, 36%, 9%, 8%, 1%): the inbound lobs
from behind the baseline now leave from the inbounder's hand (5.9 up at
|x| 41.9, the original's 5.85..6.41 at 41.7..43.5) aimed as the
original's (8.65 up, 2.0..2.3 from the receiver; the original's 8.1..9.4
and 2.3..2.9), and the high one-hand rows (`catch_stand_3l`, `3r`) and
`catch_stand_1` meet them 5..7 updates early (the original's 2..4): the
receiver's facing on those rows (the computer's +0xb0, above) is the
lead. Knocks off a body outside a block jump 5 → 15 and passes knocked
loose 5 → 15 over the four games come with them.

**Left, labelled.** The running catches' overlay (above). The shooter's
turn to the rim (the original's 23 units an update through the blend).
Shot clips whose
root travels early (`heave_1hand_l`, the side fades, the hooks, records
320..364) sit 0.5..0.8 off in their first frames, as do dunk blends (0.74):
the planted-foot opcodes move the model off the actor's place, not
modelled. The dunker's flight is the rewrite's own (`DUNK_RISE`; on its
script since, "The jumps on their scripts"): at the
slam he is 4.5 up against the original's 2.6 (the ball in the dunk clip's
hand 9.16 above his feet against 9.22), and his held ball is blocked 2.5
times a game. The passer is not carried by his pass clip's root, so his
ball leaves 2.82 ahead of his feet against 1.30. The rebound, block and
tip jumps', the dive's and the leap's clips are not on the simulation's
channel. The gait's drawn dribble is the renderer's.

## The jumps on their scripts (2026-10-17)

Branch `jump-scripts` (off 3f081c4). The rewrite's own jump curves
(`SHOT_RISE`, `DUNK_RISE`, `LAYUP_RISE`, `SHOT_JUMP`, `LAYUP_HEIGHT`,
`FADE_DRIFT`) are gone: the shooters', dunkers' and lay-up jumpers'
flights come from their scripts' bytes, moved as the original moves an
actor (`street_sim::jump`; the scripts in `held_tables.rs` now carry their
jump opcodes, read again from the executable by `tests/trick_disc.rs`).

**The motion step** (`FUN_00156f48`, raw `0x1573d8..0x157954`). After his
side's update and his channels (`FUN_0019c658`), the actor's place gains
his velocity (actor+0x30 += +0x40); unless channel 0's flags hold 0x40000
gravity follows (y += g / 2, the rise += g, `g = −32.152233 / fps² ×
1.2596`, about −0.01125 at 60); on the floor (y at most 0 and falling) he
stands; the +0x184 deceleration; then his facing turns toward the wanted
one (`FUN_00168ea8`, the vtable's +0x64: 46 units an update, 23 for a
dunker or a lay-up jumper (bits 7 and 13) or a post-up with the ball, 34
while 0x80000); then the pose and the planted points' correction
(`0x157800..0x157954`: a planted pair's midpoint, or one bone, kept where
it was planted, the place moved by the difference).

**The opcodes.** `9f v` the rise (`FUN_00194c40`, × 60 / fps; the jump
shots' 0.25). `a1 81 a v w` `v` along the wanted facing (+0xb0) turned by
`a`, `w` up (the hooks); `a1 f0 a v w` along the line to the rim
(`FUN_0016f260`: game+0x35c[id]+0x200) turned by `a` (the fadeaway's −512,
the runner, the lay-ups). Callback 18 (`FUN_00172788`): the side fades, 15
/ fps up, 7.2 / fps along the line to the rim turned by the clip's own
turn (behind the board 0x300 or 0x100, × 1.35). `aa k` (`FUN_00194848`):
`k` times the last update's displacement (actor+0x220), after a slam. `97`
the velocity zeroed. `93` plants the hands (flags 0xc0000: no gravity) and
`95` the feet (0x80000), their bones picked by the selectors (`0xec` the
side of the court, `0xf2` a left-hander, `0xf3`/`0xf4` the bone further
back or ahead along the pelvis's facing, `0xf7` the clip's foot flags,
`0xf8` the live hand; `0xf1` keeps a planted pair of the same bones); `96`
sets 0x2000000, which lets go at the next motion step. `91 w`
(`FUN_00195858`) sets the low 16 flags and ORs the high: 0x1000 keeps the
clip's root off x and z (`FUN_0019a658`, `0x19b4bc..0x19b70c`: otherwise
the root's change times the height scale, turned by the facing, moves
him); 0x10000 lowers him by the root's rise (`0x19b684..0x19b704`) until a
step reaches a new whole frame outside a blend (a blend's root is the
mixed one); 0x400 turns his wanted facing to the rim every update of the
side with the ball (`0x15a104..0x15a11c`). `98` tests the record (a pair
of branches the compiled steps keep as `Either`). A landing wait
(`8d .. fe`) lasts `FUN_0019bed8(g, vy, y)` from its start, so a script
ends when its actor lands.

**The dunk's aim** (`9e`, `FUN_001988b0`, raw `0x198994..0x198b5c`): the
hand (the right bone 0x13, 0x12 with `0x83`; the left 0xf or 0xe; the
midpoint with `0x84`; a one-hand pick `0x85`/`0x86`) posed at the row's
slam frame facing the wanted facing (`FUN_00198740`), sent to a point by
the ring `n` updates on: one hand at radius 0.89322 + 0.125, 11.6667 up
(12.3333 with the second bones), an approach from behind the rim's line
turned back (`FUN_00198618`, 256); two hands or a pick by the hands' line,
`asin(d / R)` toward the end (114), the radius `sqrt(R² − d²)` (at least
0.15865) at 11.6; a lay-up jumper aims a ball's radius lower; records
891/892 higher. The velocity is `(point − hand) / n` plus `g n / 2` up,
and the flags gain 0x1000. `FUN_0019bde0` writes the clip's root
translation to locals the aim never reads, so the pose's root is the
model's root height as the channel's step left it (actor+0xe4), not the
clip's at the slam frame (the rewrite posed it there first: dunk rises
0.2..0.64 hand heights high). `9e` also sets actor+0x31c to `n` for bits 7
and 15 (`0x199858`), an update less each update of the side with the ball
(`0x15a14c..0x15a178`), which a block jumper's aim reads (callback 35,
`FUN_00176080`: `n = min(n, holder+0x31c − fps / 20)`, at least 1); his
aim also reads the holder's velocity. `ac` (`FUN_001998f0`): with `cap`
the speed across held to `(rim distance − 0.79323 − 0.04335) / airtime`,
then the ball at the hand posed at the frame `ahead` × his height scale
toward the baseline, carried `updates` at that speed: past the ring's
front (35.91, within `r + 0.83645` of the middle across) or else 38, the
velocity on x takes him back.

**The port.** `Player::flight` (`jump::Flight`: velocity, flags, landing,
displacement, planted bones and point, the meet) runs in `World::step`
after the shooter's channel (`World::flight_tick`): the script's steps for
the update (`World::move_steps`, the landing wait from the flight's own
airtime, recomputed after a dry run of the update's `97`, `9f` and `aa`),
the meet's count, the rim facing, the clip's root before and after the
ops, the ops, the motion step, the facing's turn (`guard::turn_toward` at
the move's rate) and the planted correction. `Action::Shooting`,
`Dunking` and `Layup` keep their clocks (release, slam, end) and end when
the script has run out and he is on the floor. The wanted facing is no
longer reset to the course on these moves. Putbacks (rows past
`MOVE_ROWS`) keep the rewrite's glide (`PUTBACK_RISE`). Snapshot 46.

**Measured, played captures** (175 runs on the twelve: every shot, dunk
and lay-up from its press, replayed through `World::step` with the disc's
animations from the update before, the stick and button as captured;
`played_capture::the_jumps_follow_their_scripts_in_play`). Height off the
captured actor, median over the updates (share within 0.1), main → branch:

| Kind | Runs | Height | Travel since the press | Top (captured) |
|---|---|---|---|---|
| jump shots | 49 | 0.50 (33%) → 0.000 (100%) | 1.38 → 0.19 | 1.79 → 2.56 (2.56) |
| side fades | 25 | 0.51 (24%) → 0.000 (100%) | 3.09 → 0.02 | 2.00 → 2.42 (2.41) |
| hooks | 6 | 0.31 (34%) → 0.000 (100%) | 0.80 → 0.02 | 2.00 → 2.89 (2.89) |
| heaves | 14 | 1.48 (4%) → 0 (100%) | 5.92 → 0.08 | on the floor |
| runners | 8 | 1.03 (0%) → 0.095 (72%) | 2.70 → 0.02 | 1.83 → 2.84 (2.78) |
| lay-ups | 3 | 1.31 (21%) → 0.000 (99%) | 1.06 → 0.03 | 2.50 → 4.03 (4.03) |
| dunks | 36 | 0.71 (19%) → 0.004 (66%) | 1.63 → 0.12 | 4.29 → 4.56 (4.52) |
| alley-oop dunks | 34 | 3.53 (12%) → 0.004 (80%) | 2.34 → 0.02 | 4.50 → 10.81 (10.73) |

The dunks leave the ball 3.46 up against the captured 3.41 (4.50 before),
the alley-oop dunks 3.91 against 3.74; 106 of the 175 land on the
captured update (none before), the dunks 7 of 36.

**Measured, computer games** (the first attract capture's length; 20
all-50 games and 80 of the disc's pairs; main → branch; the original's
first capture, then its second). The slam's height: 4.50 → 3.30 and
4.50 → 3.22 (the original 2.95 over 21 dunks, 2.85 over 8). Block jumps
at a dunker 22.2 → 26.7 and 28.2 → 31.7 a game (30, 9), at a shooter
18.2 → 17.5 and 20.4 → 20.0 (13, 8). Blocks of a dunker's held ball
2.70 → 2.90 and 2.38 → 3.51; of a shooter's 0.20 → 0.35 and 0.62 → 0.62;
of a ball in the air 5.10 → 4.55 and 5.11 → 5.79; all 8.00 → 7.80 and
8.11 → 9.96. The original's first match has 3 dunkers' held balls knocked
loose with a block jumper at them (frames 9922, 10969, 37531; none in the
second) besides its 3 blocked shots and 2 blocked passes in the air: 8
blocks, not the 5 the attract comparison's tally shows (`native_kinds`
counts only balls in flight), so the premise that the original never
blocks a dunker's held ball came from that tally's blind spot. With all
at 50 the branch sits at the original's 8; the disc's pairs are 2 above
it, through the dunkers (still 0.35 higher at the slam than original
in these games) and the balls in the air.

**Running catches, traced further and not ported.** The row's script on
channel 2 runs over channel 0's clip, which in all 71 of the played
captures' moving receivers is their gait's, carried on at its record and
rate (pc at the gait scripts' `ab −1; end` tail, no blend): game+0x70b0
is set there, so `FUN_0015a060` calls `FUN_00163d40(actor, 0, 1)` instead
of starting `0x4446f4` or `0x443dc0`. Callback 30 (`FUN_00172110`) sets
channel 0's flag 2, which bars the side updates' locomotion call
(`FUN_00163d40`, as `FUN_0015da28` reads it: the stick no longer moves
him); it does not touch the pose. The pose step (`0x157618..0x157714`) writes channels 0..2 into one
array of parent-local rotations, channels 1 and 2 only with flag 4 (the
running rows' `91 06 00 02 00` sets it) and only their bone range (+0x74,
+0x7c: 0..21, 9..21 for channel 2), channel 0's bone 0 turned by the
facing (+0x50); `FUN_001935c8` copies a range between a channel's own
buffers (and the root's entry after the bones when +4 is 0), not into the
model. The release's aim (`0x1626fc..0x16271c`) poses channel 0's clip at
its whole frame plus the rate times the flight and copies bones 0..8 over
the row's pose with `lq`/`sq`. Ported that way (the gait's clip carried
on under the row, `Poses::overlay` from bone 9, the aim on the same
overlay), the rewrite's running catches meet the ball with an upper arm
first on the both-hands rows `catch_run_6r`/`6l` (jogcatch04, whose
pelvis turns 60° toward a ball from behind: the overlay keeps the gait's
pelvis): over four all-50 games 82 balls knocked off an upper arm against
5 posed whole-body; on those rows 65 of the 76 balls coming straight from
behind (sideways under 0.3 of their speed ahead), 10 of 36 at 0.3..0.6,
3 of 50 wider. The original catches all 14 of its own straight-from-behind
passes to runners in the attract captures (51 of 51 from behind), and its
played moving catches sit nearer the overlay's hands than the whole-body
clip's (median 0.61 against 0.83 from the nearest running row). Keeping
the row's orientation in the model (the spine on the row's own pelvis, an
untraced reading) knocks 7 of 162. Something in the overlay is still not
traced, so the rewrite keeps the whole-body row; the attempt is in the
hand-over notes. (Traced 2026-10-18, "The loose ball": the original never sweeps a
receiver on his catch row while the pass flies; the overlay is ported.)

**Inbound lobs.** The rewrite's `catch_stand_1` inbounds are met 4..7
updates before arrival (the original's played captures 3..5); the high
rows (`3l`, `3r`) mostly at 2, as the original (4 of 12 `3l` at 7..8). In both the flight left at the
catch follows the throw's speed: the ball's distance from the aimed point
when the hand meets it (left × speed) is 1.52 in the original and 1.80 in
the rewrite, and the rewrite's receivers stand 7.2 from the ball at the
release against 8.5 (the same 20-update flight comes slower). The
receivers' facing at the release is mostly within 20..30 units of the
ball in both. The 5 of 14
`catch_stand_5` inbounds the rewrite loses are defenders' deflections, not
missed catches (the played captures have no `catch_stand_5` inbound).
Not resolved: why the hand meets the ball 0.28 farther
out.

**The dive's and the leaper's catches** stay the rewrite's own: the dive
and the alley-oop's leap (`Action::Dive`, `Action::Oop`) are not on their
scripts. The alley-oop dunks themselves now fly on their rows' `9e`.

**The attract comparison's bounds.** The second capture's game (the
shorter, 15k updates) drew a within-4 share of 0.268 against its match's
0.201 and 17.5 shots with the disc's pairs against 22, past the bounds of
0.05 and a fifth. Twenty seed bases put the within-4 share at 0.277 on
main and 0.272 here (sd 0.037 and 0.036; the first capture's length
0.272 and 0.274), ten put the pairs' shots at 19.0 and 19.85 (sd 0.95
and 1.2; the first's 54.7 both): no shift, the draws of one short game.
The short capture's bounds are three of its games' sd (0.11) and a
quarter, as the share away from tricks already was.

**Left, labelled.** The runners' first update (0.095 at the median: the
blend's first update lowers him by a root the rewrite reads from the
held clip). The press update's facing: the original's press writes
+0xa8 to a value not traced (not the pelvis's), and the dunk aims that
start from it go elsewhere (a third of the dunks' updates more than 0.1
off). The dunks' landing (7 of 36 on the captured update). The slam
0.35 high in computer games, and their block jumps at shooters (17.5..20
against 13). Putbacks' glide.

## The computer's block jumps and the dunk's hang (2026-10-17)

Branch `block-dunk` (off e993f56). Measured with the attract captures'
block jumps classified by what they went up at (the holder's state bits
the update before, the ball's state), now part of
`ai::capture::shots_turnovers_and_spacing_against_the_captured_original`
(`Tally::block_jumps`), and with seed studies of ten bases (ten all-50
games and forty of the disc's pairs at each capture's length) on main and
on the branch.

**The computer's pad stays the computer's.** `World::computer_inputs`
replaces the computer markers with the pads the AI builds, but the built
pads had lost the marker, so every test of whose controller drives an
actor (`FUN_00172220`: true in game phases 0 and 1 or for the controller
4) found a person in the computer's games. Three effects:

- Callback 19's release gate (`86 13`, "The person's shots, passes and
  catches") held the computer's jump shots while its pad held shoot: half
  of them left at 47..48 updates after the press instead of their
  callback 21's 33..34 (the attract captures' computer releases 21..34,
  one 44). A shooter still in the air with the ball at 41 updates is where
  the defence's late block jump goes up (`fps x skill(0.95, 0.95, 0.45,
  0.45)` at Medium), so the late jumps landed on held shooters: 19.2 and
  22.1 block jumps a game at held shooters (all-50, the disc's pairs)
  against the original's 13.
- The pass rows' filter (`0x160894`, `0x1608d8`) took a person's rows for
  the computer: never a row with a Handles minimum.
- The trick's fakeout bonus (`0x137650..0x137674`) is a PERSON's (10 more
  for a trick when `FUN_00172220` is false and the options' difficulty is
  Got Game); the rewrite gave it to the computer, which the lost marker
  hid. Now `PERSON_ODDS`.

The pad now keeps the marker for the computer's players (a person the
restart walks keeps his own). Callback 20 (`FUN_00172348`, the pump fake)
reads the held word with no such test, so the rewrite's own test there is
gone (a computer that lets go of its button fakes).

**The dribble** (actor+0x1a0, `Player::dribble`): 0 from a take of a free
ball (`FUN_0014cb08`'s callers), for everyone when the ball changes sides
(`FUN_00169138`, `0x1691d4`), kept through a pass's catch; 1 once he starts
a locomotion record with the ball (`FUN_00163d40`, `0x167f54`: the
request's 0x102; also on state bit 5, `0x167064`, not modelled); -1 when
he pump-fakes out of a dribble (callback 20, `0x1723b0`). The computer's
fakes need it not 1 (`FUN_001816b0`): "shoot"'s 1 in 16, "carrier
pause"'s 1 in 8 and "post up"'s 1 in 4. The rewrite read it as never
dribbling: 5.5 computer pump fakes a game (with the disc's pairs; the
behaviours "shoot" 2.4, "carrier pause" 1.3, "feeder", "post up", "run
pick play") against none in the original's 48 computer shot starts of the
two attract captures; now 0.25. `FUN_00163d40`'s use of -1 (a fake out of
a dribble: `iStack_114`, which looks like the picked-up dribble's stance)
and `FUN_00181750`'s (the spot's and the juke test's "taken as set") are
not ported.

**The block decision** (`FUN_00106a88`, raw `0x106a88..0x107194`, the
"defense" update's third test). Ported before and kept: the early or late
draw per shot, the wait after the shooter leaves the floor, the late jump
at `fps x skill(..)`. Ported now:

- The bite (`[0x18]`): a defender who bit (`skill(100, 100, 20, 20)`, half
  of it against a full meter; the port always halved it) jumps at a pump
  fake at once when close enough (`0x106e94..0x106ed0`, no turbo).
- The holder includes a pass to oneself's passer while he leaps or dunks
  (`0x106c74..0x106cd4`).
- The pass jump (`0x106d6c..0x106e7c`): every `fps x 0.0833334 + 1` = 6
  updates of a pass in the air, its receiver leaping (bit 15) or a
  passout special's last leg (game+0x160 ±1, not modelled), none of the
  side up or asking within 12 of the ball, the defender in the pass's lane
  (`FUN_001838f0`: nearer the pass's point than the ball, 10..22 from the
  ball, ahead of it and within 5 of its line on the floor), then `r % 100
  < skill(0, 5, 50, 100)` through the pad builder.
- `FUN_00107198` (raw `0x107198..0x107380`): the nearest teammate bars
  him only while rising (actor+0x44 above 0) below 1.5 within 5, in bit 12
  or asking (a computer's pad with block this frame, `FUN_001818b0(..,
  1)`); two of the side up or asking within 12 of the ball (`FUN_0018af90`,
  the game's ball distances) bar him; then the two draws as before.

The game modes that skip it (vt+0xb4 1..5, 9, 0xb, 0xd, 0xe, 0x12, 0x13,
0x15; 0xc always bites) are the special games the rewrite does not play.

**Measured** (block jumps a game at the first capture's length; main ->
branch; the disc's pairs, then all-50; the original's first match):

| Block jumps | main | branch | original |
|---|---|---|---|
| at a shooter holding the ball | 22.1 / 19.2 | 13.6 / 9.1 (sd 3.9 / 3.5) | 13 |
| at the jump shot just let go | 4.5 / 2.5 | 8.7 / 7.7 (sd 2.6 / 2.3) | 7 |
| at a dunker | 30.1 / 26.2 | 31.1 / 25.1 (sd 5.2 / 3.0) | 30 |
| at a pass | 0 / 0 | 0.05 / 0 | 6 |
| computer pump fakes | 5.5 / 7.0 | 0.25 / 0.1 | 0 |
| block events (a jumper by the ball) | 12.0 / 8.7 | 10.1 / 8.5 (sd 3.2 / 3.2) | 8 |

The second capture's length: at a shooter 8.0 -> 4.9 (the match 8), just
let go 1.6 -> 2.8 (1), at a dunker 11.1 -> 11.1 (9), block events 4.1 ->
3.5 (2). Per jump shot let go, block jumps at the shot (held or just let
go): the original 20 of 27 and 9 of 10; the disc's pairs 22.3 of 26.6
and 7.7 of 8.9. The rewrite's computer jump shots now leave at their
tables' callback 21 (median 33..34 after the press; main 41..44).

The block events' make-up still differs: the original's first match has 3
dunkers' held balls, 3 shots and 2 passes knocked away with a jumper by
the ball; the branch's disc pairs 3.4, 5.6 and 0, plus 1.1 shooters' held
balls. The passes: the rewrite's alley-oop passes are in the air with
their receiver leaping 0.2..0.4 times a game against the original's 6 and
3 (its passes to leapers fly 34..41 updates with the leaper up all the
way; the rewrite's leap, `Action::Oop`, is its own and not on its
script), so the pass jump, ported, rarely has a pass to go at. The shots:
the jumpers who go up while the shooter holds the ball knock 25..30% of
the shots away against the original's 1 of 21. What they do is the
original's: callback 35's aim at a holder now has a played-capture check
(`the_block_jumps_at_a_holder_rise_as_callback_35_aims_them_in_play`,
the aim moved into `block::holder_target`: rise within 0.02 for 8 of 9
airborne shooters and drift for 6, the misses created players whose
ratings the captures lack), the hands' test reached the ball on 10 of the
played captures' 103 block jumps, and the jumpers take off 6..8 updates
after the press, rise 3.5..4.3 and drift 4.4..5.5 (the original 6..10,
3.6..4.0, about 6). Their place and timing against the shot (median 16
updates from the jump to the release against about 12; floor distance 3.9
at the release in both) leave the difference unexplained; the shots' ball
leaves 11.8..11.9 up at 0.33..0.36 an update across (the original 12.2
and 11.9, 0.37..0.38).

**The dunk's hang on the rim.** 25 of the 99 dunk rows (and three of the
alley-oop dunks) ask callback 28 (`FUN_00172400`, raw `0x172400..
0x1725f8`) after their slam: `85 28 -> T` falls through into the hang
(`9b 0x4080`, `93` plants the hands with no gravity, a wait of 32..87
updates, `96`, `9c 0x4000`, `aa 0.5`), `84 28 -> T` jumps to it. The
callback: never in a won game (vt+0xbc); then `r = rand() % 100`; never
while a teammate hangs (state bit 14, `FUN_0016ea50`) or when the ball has
touched the hoop (state 6) and is not his; then by the ball's last shooter
(ball+0xf8, set by the launcher `FUN_0014aaf8`, cleared by the hold
`FUN_0014cb28`; else its holder): his own, `r > 1` after a made release
roll (`0x3d6580`, written by callback 21's roll) and `r > 49` after a
missed one; nobody's, `r > 89`; another's, no. The Script Manager's
replay states 3..7 always hang; the hoop object's words (+0x80 set, +0x78
clear) and `FUN_00140558`'s winning baskets bar it (not modelled). Every
hang-capable dunk in the attract captures hung (eight), landing 127..156
updates after the press against 66..95 for the others.

Built: the tables compile both paths (`gen_jump.py`, `tests/trick_disc.rs`
reads them again): a `Step::Hang { hang, other }` over the hang's steps and
the other path's, each to its `End` or to where they meet; a run takes the
hang's steps when `Run::hang` says so, and a `Hang` met unanswered reports
`Ran::hang_test`, on which `World::flight_tick` calls `World::hangs` (the
callback, its draw where the channel's step runs) and runs the update
again on the path it picks (`Action::Dunking::hang`). A hanging dunk's move
ends with its script once he is down. The ball keeps its last shooter
(`Ball::shooter`) and the last roll (`Ball::rolled`). The display compiles
the dunk rows along the hang too (`playback::Path::Hang`) and plays it for
a dunk whose answer was yes. Snapshot 47 (with the dribble and the pass
jump's timer).

Played captures (`the_jumps_follow_their_scripts_in_play`, the twelve):
17 dunks and 10 alley-oop dunks hang; before the hang the dunks' height is
within 0.1 of the original's on 88% of updates (66% when the hang was
skipped), travel 0.02 at the median; the hanging dunks land 5 updates
early to 4 late of the original (they landed about 50 early); dunks land
on the original's update in 9 runs and within one in 15 (7 before). The
rest: the created players' bodies the captures lack (five dunks a
player whose record is not captured, each a frame late); the hang itself.

**Left, measured: the rim's give.** While a dunker hangs (bit 14 and
actor+0x2b0 set), the planted point is not where his hands met the rim but
follows the rim (`0x1577a0..0x1577f8`): each update `FUN_00157f20` maps
actor+0x1f0 (the point in the rim's frame, stored by `FUN_00149410` at the
plant) through the hoop model's rim bones 2..4 (hoop+0xc4, `FUN_00149150`),
the hoop on his side of the court (game+0x178 or +0x17c). The rim bends
under him: the original hangs 0.73 farther along the line to the
baseline and 0.19 lower (medians over the hangs' updates; 0.74..0.78
along x in every run traced), the gap opening over the first five updates
of the hang. The hoop's skeleton and its bend are not ported.

**The slam's height.** The slam (the dunker's height at callback 21) is
the original's for a given body: the played captures' 36 dunks leave the
ball 3.46 up against 3.41; per row the rewrite's slam falls 0.13 for each
inch of the dunker's height (dunk_4 3.34 at 6'5" to 2.61 at 7'0"). The
attract captures' dunkers, read from their slams by row on that rule,
were mostly seven-footers (the first match's actors 0, 3 and 4, with 15 of
its 25 dunks); the all-50 games' dunkers are all 6'6" (3.30) and the
disc's pairs' average 6'7.5" (3.20). Who dunks is the rewrite's own
(`offence::dunkable`, its `DUNK_RANGE`): the original asks `FUN_0016c500`
(the facing and distance to the rim, and no opponent within 4.5 in a cone
whose width the dunker's Dunks rating sets), not ported.

**Running catches** (the second channel's overlay): not taken up here.

Bounds: the short capture's check of the spacing through the tricks
widened to 0.13 (twelve bases: 0.387 on main and 0.364 after, sd 0.035
and 0.046; its game drew 0.360 and 0.430); at the first capture's length
the within-4 share moved 0.279 -> 0.264 (the original 0.262) and the share
away from tricks 0.220 -> 0.193 (0.208); shots, trick lengths and
turnovers unchanged within their spread. The comparison now checks the
pairs' block jumps (at a shooter within 6, just let go within 5, at a
dunker within a quarter or 6) and their pump fakes (under 1.5 a game).

## The alley-oop's leap and the dive on their scripts (2026-10-17)

Branch `oop-dive` (off 3764caa). Traced raw (SLUS_206.51 with
`tools/mipsx.py`) and measured with the four played lobs to leapers, both
attract captures, and seed studies of ten bases (ten all-50 games and forty
of the disc's pairs at each capture's length) on main and on the branch.

**The leap's request** (`FUN_0015a060`, raw `0x15b218..0x15b2f8`). A
player of the side with the ball who does not hold it and presses shoot
(+0x7c & 0x80) with a turbo held (+0x78 & 0x2000) leaps when
`FUN_0016c450` lets him; a person holding rebound (0x100000) while the
ball is a shot, on the rim or coming off it (states 5, 6, 0) does not.
`FUN_0016c450`: none of state bits 3, 7, 14, 15 (0xc088), `FUN_0016c500(actor,
0)` 2, the ball held or passed (3, 4), on the floor and within 35 of the
middle along the court. With two turbos held (0x40000) the leap is one of
the two scripts at `0x3d70d0` (`rand() & 1`: `SUPERDUNK_360_TOMMAHAWK`
0x4479b7, `SUPERDUNK_DINNERSERVED_360_WINDMILL` 0x447a1e; dunk id 24,
move kind 1), `timing::LEAP_ROWS`, rows 99 and 100 past the dunk table.
Otherwise it is `FUN_001693b8(actor, 1)`, in this order:

- two draws for the face (`r & 1`, then `FUN_00159190`'s own);
- the table's rows with mask 0x100000 (rows 94..98) whose sector bit
  holds where the ball comes from against his line to the rim (`(FUN_0016f260
  − FUN_0016f2b0 + 0x640) & 0x3ff >> 7`);
- the live hand's bits and the distance bands;
- `rand() % count`.

No Dunks gates and no unlocks apply. `FUN_001692f8` sets actor+0x320, the
lead a pass needs (`trunc(fps × 0.15)` = 9 for `allydunk_2` 0x447900, else
7). A person who cannot leap, on the floor in live play, plays the call
gesture 0x44c95e (not ported). The computer leaps on the same press from
its pad.

**`FUN_0016c500`** (raw `0x16c500..0x16c904`), whether a dunk is on. It
returns 1 out of the dunking area (`FUN_0016c908`: |z| under 22.5, within
23.75 of the rim). Otherwise the dunk is open unless one of these holds:

- the nearest opponent (the game's distance table, `FUN_00170dc0`) is
  within 4.5 and in the cone between him and the rim (`FUN_00170840`: 200
  units wide, or `(1 − Dunks / 99) × 200 + 100` in bits 0x8080);
- he is on a post-up clip (`FUN_0016ec80`: records 0x2c0..0x2c3, 0x2c8,
  0x2c9).

A full meter opens it. It returns 2 when open within 15, moving or with
flag 0, facing the rim within ±350 units (`FUN_0018d290`, his wanted
facing) and not behind the board (`FUN_0016f580`); else 2 when standing
within 7.5; else 0. It is ported as `World::dunk_on` for the leap only;
`offence::dunkable` still picks the dunks. The special games' modes (3, 5,
0x15) are not modelled.

**The leap is a dunk row's script.** Each leap row's script is the whole
leap:

- `9b 0x8000` (bit 15; `9b` replaces the state word);
- callback 4 (the wanted facing to the rim) and a blend of eight;
- `9e`, aimed 80 updates on;
- the contact window `a6 01 02 50` (kind 1, not ported);
- the long wait the lob is timed to;
- then a dunk's tail: callback 21 (the slam, with the ball), callback 27,
  `9b 0x80` and the landing (some rows hang through callback 28).

`Action::Oop`, the rewrite's own (a straight rise, a fixed slam window,
its own lob), is gone. A leap is `Action::Dunking` on its row, flown by
`street_sim::jump`, in bit 15 until the script reaches its `9b 0x80`
(`leap::leaping`; ticks 92..97 by row). The generated tables add the
two-turbo scripts' steps (`LEAP_STEPS`). `tests/trick_disc.rs` and
`tests/script_timing_disc.rs` check them against the disc: release 94 and
92, end 109 and 107.

**The lob** (callback 38's `FUN_00162aa8`, raw `0x162aa8..0x1631c4`). It
is called in place of the catch-row draw for a pass to a leaper:

- the flight is `max(table / S, 20)`, not truncated, raised to the span
  his channel has left less five (`+0x6c − +0x70 − 5`) when that is
  longer;
- his clip is posed at the frame it will show then (`+0x5c + rate ×
  flight`, wrapped), at his place, facing his wanted facing;
- the hand comes from the frame's hand flags (8 the right, 4 the left,
  else the midpoint);
- the point adds his velocity times the flight and half the gravity times
  its square, at least 12.5 up (8 when the span set the flight);
- the flight is stored truncated (`DAT_003d65a0`).

It is ported as `World::lob_to_leaper`; the release bars as any pass. The
pass press picks a leaper whose meet less the flight is at least his lead,
the highest of them (`FUN_00146278`, `0x146860..0x1468c8`; read in part).

The played captures check it
(`the_lobs_to_leapers_are_timed_and_aimed_as_fun_00162aa8`). The twelve
hold four lobs, all the computer's. Every flight is exact (38, 43, 42, 39:
the span less five each time), and the point lands 0.08..0.11 from the
original's (median 0.08). The check reads the leaper one update later when
his index is higher than the passer's. The original updates the actors from
the highest index down (the four lobs show it, as the spot passes did); the
sim updates in ascending order. Not changed; noted.

**The catch and the slam.** The leaper catches on the bone rules of the
side with the ball (`street_sim::touch`). There, `shown_state` read the
gait channel's clip for a scripted move, so almost no lob was caught
before; it now reads the move's channel. He slams at his row's callback
21. The launcher takes it as an alley-oop finish (kind 3; ids 7, 24, 37
and 41 lack 0x800 at `0x43f5f0`), and the basket is `Basket::AlleyOop`. The
executable has no "oop lay-up" path: a leaper who comes down without the
ball plays the row's tail. The rewrite's Back 2 Papa (`Via::Glass`) now
starts a leap on a drawn row with the lob; otherwise it stays the
rewrite's own path (labelled).

**The pass out of any move** (`FUN_0015a060` raw `0x15b860..0x15b944`).
The holder's pass press is taken in any move, except when:

- he is passing already (bit 4);
- he is down or on no clip (`FUN_0015f898`; its list `0x43f098` holds
  only −1);
- the ball is a gamebreaker's (ball+0x178) and he is in bit 3, 13, 7 or
  15.

The rewrite dropped every busy holder's pass press, so its shooters could
not pass out of a jump shot when a jumper came up. In the original's
attract captures, 4 of 31 and 2 of 10 jump shots end in a pass, as do 4
and 5 tricks, 2 lay-ups, and 1 and 1 dunks. The rule is ported as
`World::passes_out`. A pass out of a jump plays the pass table's air rows
(flag 0x400) on the jump's flight and ends when he lands.

The alley-oop play's "ready" (`FUN_00181b58`) is the same not-down test,
so a trick does not bar it. The rewrite's play had waited out the
handler's trick, so many leaps got no lob.

**The dive.** The press is the pad's 0x1000000:

- a person's: a turbo pressed again within `trunc(fps × 0.16667)` = 10
  updates of its own release (`FUN_001308e8`, `FUN_00130a28`;
  `World::dive_tap`, per button);
- the computer's: from "loose ball" (`FUN_0010e0a0` raw
  `0x10e504..0x10e7dc`). Nobody on his side is a person and play is on.
  The ball is under 7 up and has been loose for `trunc(fps × skill(1.6,
  1.0, 1.0, 0.6))` updates, in mode 3, and he is 6..14 from it. No
  teammate is nearer or diving. An opponent is within 3 farther than he
  is, but none within 4 of the ball or within half his distance. He is
  within 42 along and 26.5 across.

Both sides' updates take it (`0x15b390..0x15b494`; `FUN_0015da28`
`0x15de6c..0x15df5c`), except in bits 0x9388, with the ball not loose, off
the floor, or in phase 6. The near script (0x45776a, record 72) plays when
the ball 40 updates on (`FUN_001537d0(ball, fps × 2 / 3)`) lies within
his reach: 6.5 × his height scale on the side with the ball, his height in
feet on the other. Else the far one (0x4576d1, record 71).

The scripts run:

- callback 5 (the wanted facing to the ball) and a blend of six;
- `a1 85`, the hop (`FUN_00194d58` `0x194f0c..0x195378`; `dive::dive_hop`):
  the rise and speed by the frame rate, then `t`, the updates the rise
  takes to come down to the ball's catch height, `(−w − √|w² − 2gh|) / g`
  (the EE's sqrt takes the magnitude). His facing turns to the ball's place
  at `t`. The speed is held to the distance less his reach (× 0.4 far,
  × 0.1 near) over `t`;
- the landing;
- the far one's slide, `a0 0.025` (`FUN_001957d8`: actor+0x184, negative
  for the default 0.5). Its operand is 4 bytes, where the script reader had
  none;
- the getting up, callbacks 7 and 1.

`Action::Dive { script, tick }` flies on `street_sim::jump` (`Step::Dive`,
`Step::Slide`).

The catch is `FUN_0015c598` (raw `0x15c598..0x15c768`), each update of a
dive before the touch tests. It sweeps the ball's last move against his
limbs:

- both bones under 9 knock a free ball (0.15, the cooldown) and leave a
  held one;
- else a free ball is his;
- a held one is his while he is up and its holder is not diving, on
  `rand() % 100 < 11`; else he is barred fps / 2.

It is ported as `World::dive_catch`.

**Measured.** The first capture's length, ten bases; all-50 / the disc's
pairs; the original's two matches:

| A game | main | branch | original |
|---|---|---|---|
| leap starts | 6.6 / 6.6 | 7.5 / 7.7 | 6 / 3 |
| passes to a leaper | 0 / 0.40 | 6.2 / 7.1 | 6 / 3 |
| leaper catches | 0.1 / 0.53 | 4.5 / 5.25 | 5 / 3 |
| leaper slams | 0 / 0.15 | 3.8 / 4.4 | 5 / 3 |
| alley-oop baskets | 0 / 0.15 | 3.6 / 4.2 | |
| block jumps at a pass | 0 / 0.05 | 2.9 / 4.3 | 6 / 3 |
| dives | 0 / 0 | 2.1 / 4.65 | 4 / 5 |
| dives taking the ball | 0 / 0 | 0.1 / 0.55 | 1 / 1 |
| holding-shooter jumps that knock the ball | 0.297 / 0.259 | 0.226 / 0.193 | 1 of 20 |

At the second capture's length the branch gives (all-50 / pairs): leap
starts 2.7 / 2.4, passes to a leaper 2.2 / 2.2, catches 1.7 / 1.75, slams
1.4 / 1.5, pass jumps 0.9 / 1.25, dives 1.0 / 1.65 (taking the ball 0 /
0.17), shooter jumps knocking the ball 0.21 / 0.23. The lobs fly 35
updates on average (the original 34..41). Passes rise from 152.7 / 139.4
to 165.3 / 155.0 a game; shots move from 28.2 / 26.6 to 25.0 / 25.0. Pass
outs a game: jump shots 1.2 / 2.2, lay-ups 1.0 / 1.55, dunks 1.0 / 0.6.

**The held shooters' knocks.** The original's attract captures hold 20
block jumps at a holding shooter: 5 end with a pass out, 1 with the shot
knocked away in flight, and 14 untouched. None takes the held ball.

The rewrite's held-ball takes were already rare: 0.3 / 0.6 a game on main,
0.2 / 0.2 now. Its knocks are shots blocked in flight after the release:
2.4 / 2.9 a game on main, 1.9 / 2.2 now. Now 0.7 / 1.8 jumps a game meet a
shooter who passes out.

By the updates from the jump to the release (six bases), the rewrite's
in-flight knocks are:

- under 5: 3 of 37;
- 5..9: 2 of 56;
- 10..14: 3 of 46;
- 15..24: 63 of 164.

The original's 15 jumps at released shots fall 6, 2, 4 and 3 in those
bins. At the rewrite's rates that expects 2.0 knocks against its 1, within
its sample. What differs is the share of jumps that go up 15 or more
updates before the release: 54% against 3 of 15. The guards' wait from the
shot's start is about the original's (median 18 against 20; quartiles
16..24 against 17..27). The shots leave about 33 updates after their start
in both (the original 24..39). So the gap lies in how the jumps spread
over the shot, the block decision's draws (`FUN_00106a88` as ported). The
hands, the held ball and the script phase are not it. Not closed.

**Left:**

- the leap rows' contact window (`a6` kind 1);
- a person's call gesture when he cannot leap (0x44c95e);
- the special games' modes in `FUN_0016c500` and the passout special's
  leg;
- `offence::dunkable` in place of `FUN_0016c500` for the dunks;
- a person's choice of leaper (`FUN_00146278`'s lead), read only in part;
- the dunk path's two face draws, which `move_timing` does not take;
- the actors' update order;
- Back 2 Papa's own path;
- the jumps' spread over the shot;
- the dives: fewer than original's (2.1 / 4.65 and 1.0 / 1.65 against
  4 and 5).

Snapshot 48: the dive's script and tick, the flight's slide and the turbos'
release times; `Action::Oop` is gone. The second capture's single game
drew wider than its bounds: the share away from tricks (0.14), within 4
(0.12) and the blend (0.25), each with twenty bases noted in the
comparison.

## The block jumps' spread, the dives and the actors' order (2026-10-17)

Branch `steady-timing` (off 3309135). Traced raw (SLUS_206.51 with
`tools/mipsx.py`) and measured with both attract captures: for each jump
shot, its start, take-off and release and each opponent's block press and
jump, read the same way on both sides. Seed studies ran eight bases at the
first capture's length and sixteen at the second's (the all-50 game and the
disc's four pairs each), main against the branch.

**The jumps' spread is the original's.** `FUN_00106a88` was read again
raw (`0x106a88..0x107194`), with the counters its update keeps
(`FUN_00106490`, `0x1067b0..0x106858`):

- [0x40]: the updates the holder has been in a shot, dunk, lay-up or leap
  with the ball (`FUN_00181508`), or in a pass to himself;
- [0x44]: those of them above the floor, kept until he leaves the move.

The port is the original's. Read the same way (jump shots only, the
lay-ups apart), the original's 15 jumps at a holding shooter go up 0..17
updates after his take-off: median 5, quartiles 1..10, 7 within 2. The
rewrite's: median 2, quartiles 1..9, 51% within 2. The early jump goes as
he leaves the floor; the others wait for `FUN_00107198`, and 7 of the
original's 15 went up 6.2..6.9 from the shooter, at its `7 − r / 10`. The
figure the alley-oop pass left (54% of the rewrite's jumps 15 or more
updates before the release, against 3 of 15) counted the original's
lay-ups and its shots passed out of.

**What differs is what follows an early jump.** The original's shooter
passes out of 4 of its 7 early jumps and 5 of 15 in all (3 from "shoot",
2 from "drive", 1 from "post up"), pressing pass 9..13 updates after the
jump. The rewrite passed out of 14% of them (69 of 480), 1..6 updates
after. The test is `FUN_00181c68` (raw `0x181c68..0x181f48`), which
"shoot", "drive" and "post up" ask (`0x101258`, `0x10997c`, `0x115bb0`).
Three parts of the port differed:

- The blocker's reach is `FUN_00181f50` (raw `0x181f50..0x182020`): the
  highest of his posed bones 15, 19 and 21 (the hands and the head, model
  vt+0x18), not his height plus his standing reach. A guard's hands are
  low as his jump starts. The original's shooters pressed with the ball
  9.2..11.3 up and the guard's feet 0.5..2.0 up (the update before the
  press, which the AI reads). Now `World::top_bone`, read on the block
  jump's own clip while he is in one.
- `FUN_00140ff8` comes first: no pass out while a gamebreaker is in play
  (game+0x434's object, made with ball+0x178 by `FUN_001422f8`, busy).
  The rewrite's computer pressed pass through its gamebreaker shots, and
  `World::passes_out` refused it.
- Within 4 of the rim a jump shot (not a lay-up) passes out when its ball
  is no more than 12 up and within 1.2 of the rim across the floor
  (ai+0xa0, `0x181d3c..0x181dac`). The port answered no.

**The take-off's root.** Every block script sets channel flag 0x10000
just before its aim: `91 02 00 01 00`, then callback 35 (or `a9` for the
glass pair; all fifteen scripts' bytes). So the channel's steps lower the
jumper by his clip's root rise, times his height scale, until a step
reaches a new whole frame (`FUN_0019a658`, `0x19b684..0x19b704`, as for
the shots in "The jumps on their scripts"). The root drops as the legs
extend, so the first one or two airborne updates rise by it. The
rewrite's first airborne height was his velocity alone, 0.28 on every
script. Over both captures' block jumps the original's median is 0.512 on
record 447 (`0x441db0`, 47 jumps), 0.43 and 0.40 on 441 and 440
(`0x441a7d`), 0.31 on 443 (`0x441b29`) and 0.43 on the turbo block's 437.
Now (`World::block_resync`) 0.512, 0.42, 0.32 and 0.47. The scripts'
first `91` (`02 00 82 00`) leaves 0x1000 clear, so the clip's root also
moves the jumper across; not modelled. This is what the earlier sections
called the planted-foot correction.

**Measured** (main → branch; the first capture's length, then the
second's; all-50 / pairs):

| Held-shooter block jumps | main | branch | original |
|---|---|---|---|
| that knock the ball | 0.260 / 0.199; 0.245 / 0.214 | 0.246 / 0.251; 0.317 / 0.250 | 1 of 20 |
| passed out of | 0.082 / 0.155; 0.075 / 0.144 | 0.077 / 0.068; 0.083 / 0.086 | 5 of 20 |
| the pass press after the jump | 1..6 updates | 8..15 (median 11) | 9..13 |

By the updates from the jump to the release, the branch's in-flight
knocks are 1 of 46 under 5, 1 of 65 at 5..9, 4 of 59 at 10..14 and 74 of
200 at 15 or more. At those rates the original's ten jumps at a released
jump shot expect 1.9 knocks against its 1. The rate per jump fits; the
outcome mix does not, because the original's shooters pass out of an
early jump more often. Two causes found, neither closed:

- 32% of the rewrite's computer jump shots come out of "drive", whose
  pass-out draw is `r % 100 < 25` (`[0xf]`), against 4 of 41 in the
  original's (the rest "shoot", whose draw is `skill(40, 50, 50, 100)`,
  and 3 "post up");
- after an early jump the guard's top bone comes within 1 of the ball in
  about 70% of the rewrite's cases; how near the original's comes is not
  capturable (its bones are not recorded).

With the guards as high as the original's, the knocks rose a little.

**The dives.** The rule is the original's, and the shortfall is the short
match's own draw:

- At the first capture's length the rewrite dives 2.6 / 4.3 a game (main
  2.25 / 4.6), against the match's 4.
- The original's dives all come in balls loose 54..187 updates
  (game+0x440), past the computer's wait (`fps × skill(1.6, 1.0, 1.0,
  0.6)`, 62 at Medium level).
- The rewrite's games hold as many loose balls, and as many long ones (60
  or more updates): 18 a game at the first length against 18, and 7.2..8.4
  at the second against 7.
- They dive 0.22..0.24 times per long loose ball (0.13..0.14 dive
  episodes), as the first match does (0.22; 3 episodes in 18).
- The second match drew 3 episodes from its 7 long loose balls, two with
  both sides diving at once (5 dives). At the rewrite's rate that expects
  1.0..1.5; pooled, the matches' 6 episodes in 25 against 3.5 expected
  (p ≈ 0.13). The rewrite's short games reach 5 dives 6% of the time.
- Dives taking the ball: 0.38 / 0.44 and 0.25 / 0.20 a game, against 1
  and 1.

**The actors' order.** `World::step` now updates the actors from the
highest index down. The played captures' four lobs read the leaper as
that order leaves him (his state of the release's own update when his
index is the higher), and their spot passes agree, two more exact that
way. Nothing else measured tells the orders apart:

- the twelve played captures' reports are the same either way (their
  replays move one actor);
- the attract comparisons' means stay within their spreads (the first
  capture's within 4: 0.250 → 0.264, sd 0.02 of single games);
- the catches keep their leads: four rewrite games catch 241 and 173
  passes 1 and 2 updates before they arrive, then 242 and 189 (the
  original's match 71 and 59).

The AI still builds the computer's pads in ascending order; nothing traced
says otherwise.

**Steady checks.** `shots_turnovers_and_spacing_against_the_captured_original`
runs 16 seed bases (`BASES`: each game's seed plus 1000 times the base,
base 0 the single game before; on threads, 17 s and 6 s). It checks each
figure by its mean. Each bound is the gap to the original's match plus
three standard errors, rounded up:

| Bound | first capture | second capture |
|---|---|---|
| standing | 0.05 → 0.035 | 0.05 → 0.035 |
| away from tricks within 4 | 0.06 → 0.05 | 0.14 → 0.09 |
| trick length | 4.5 → 4.0 | 4.5 → 3.5 |
| within 4 through the tricks | 0.05 → 0.025 | 0.13 → 0.09 |
| the blend's back-off | 0.2 → 0.13 | 0.25 → 0.18 |
| within 4 | 0.05 → 0.02 | 0.12 → 0.10 |
| turnovers above the match | 6 → 2.5 | 6 → 5 |
| counter presses above the match | 23 → 13 | 9 → 8.5 |
| the pairs' shots | 11 → 4 | 5.5 → 4.5 |
| block jumps at a shooter, just let go, at a dunker | 6, 5, 7.5 → 2, 2, 4.5 | 6, 5, 6 → 4.5, 2.5, 3 |
| pump fakes | 1.5 → 0.5 | 1.5 → 0.5 |

Two other seed sets (each seed plus 500, and plus 777) stay inside every
bound.

Tests: `the_takeoff_rises_by_the_clips_root_drop_until_a_whole_frame` and
`the_top_bone_is_the_highest_of_the_hands_and_the_head_as_posed`
(`tests/blocks.rs`). No snapshot change: the take-off's flag is read from
the jump's frames, and the order keeps no state.

**Left:**

- the pass outs' rate (the drive share; the guard's reach against the
  ball);
- the clip's root across in the block jump;
- the AI's own order;
- a played capture with a dive.

## The combo ring, the meter's listeners, the statistics and the shot clock (2026-10-17)

Branch `meter-stats`. Audit item 6 (sections 3, 5 and 6 of `docs/AUDIT_1TO1_2026-10-16.md`). Code:
`crates/street-sim/src/combo.rs` (the ring, the matcher, the manager), `scoring.rs` (every scoring message's three
listeners and the take's classification), `shot_clock.rs` (`FUN_0013f1f0`), `street.rs` (the meter's fill and its
gates), `stats.rs`; tests in `tests/scoring.rs` and `combo.rs`. Evidence: the raw executable, the three runtime save
states' RAM (`pcsx2-runtime/sstates`, read offline from their `eeMemory.bin`) and the twelve played captures' meters
(`$EVIDENCE/street-meter-jumps-2026-10-17.txt`). `tools/pine_ai_capture.py` now records the combo manager, both meters,
the statistics and the actors' scoring words (schema 2) for the next attract capture; none was taken this session (the
emulator was closed shortly after its launch).

### How a message reaches its listeners

A post is `FUN_00190d90(FUN_0013c2e8(game + 0x3b + n), message)`. The **post address does not name the slot**: the
message's own vtable does — its fourth word is a dispatcher that calls the listener's vtable at `0x08 × slot`
(`0x104d58`: message `0x3c6580` calls `+0x80`/`+0x84`, slot 16). The slots by message vtable, `0x3c6480 + 0x10 ×
(slot − 1)` for most, and who posts them (found by every `jal 0x13c2e8` with the vtable built before it):

| Slot | Message | Posted by | What |
|---|---|---|---|
| 5 | `0x3c64c0` | callback 20 `FUN_00172348` | a pump fake |
| 6 | `0x3c64d0` | callback 35 `FUN_00176080` (its end) | a block jump takes off |
| 9 | `0x3c6500` | `FUN_0014bff8` | a pass pressed (the ball state 4) |
| 13 | `0x3c6550` | `0x157e58` (`FUN_00156f48`) | a trick move lands (bit 10 drops) |
| 15 | `0x3c6570` | the take `FUN_0014cb28` | a steal (a counter when the taker is in the stance) |
| 16 | `0x3c6580` | the knock `FUN_0014c408`, the take | a block |
| 17 | `0x3c6590` | the take | a rebound (+8: offensive) |
| 19 | `0x3c65b0` | the take | a pass caught by a teammate (+4 passer, +8 receiver) |
| 24 | `0x3c6600` | the take (its end) | every take |
| 25 | `0x3c6610` | the take | a loose ball picked up (no scoring listener) |
| 26 | `0x3c6620` | the hoop `FUN_001515a8` | a basket (+4 shooter, +8 assister, +0xc base 2/3, +0x10 the ball's kind) |
| 30 | `0x3c6660` | the collar `FUN_00152418`, the rim `FUN_001526d0` | the ball meets the rim |
| 52 | `0x3c67b0` | the shot clock `FUN_0013f1f0` | a violation |
| 64, 65 | `0x3c6870`, `0x3c6880` | `FUN_00159cf0`, the gamebreaker task `FUN_00144ef8` | a gamebreaker starts, ends |
| 71 | `0x3c68e0` | `FUN_00159f40` | a cancel |
| 73 | `0x3c6900` | `FUN_0013f950` | a defender jumps at a pump fake |
| 74 | `0x3c6910` | the contact handlers `FUN_001343e8`, `FUN_001366d8`, `FUN_001372d8` | a contact (+0xc its kind) |

The listeners run in this order: the combo manager (vtable `0x43c208`), the two meters (`0x43df50`), the statistics
(`0x440578`). The basket's meter fill includes the payout its own combo dispatcher just computed (the played captures'
basket fills carry it), so the manager comes first.

### The combo manager (game+0xd0, `FUN_001313d0`, 0x170 bytes)

- **One ring for the match.** Every dispatcher first calls `FUN_00131738(manager, side)`: when the side differs from
  +0x144 the ring, the matched row (+0x148) and the total (+0x150) are cleared and the ring is that side's. A turnover
  does not clear it; the other side's next push does.
- **Each slot** is `{class, extra, tick, value, span, weight}`: the tick is game+0x128 at the push (every dispatcher
  writes it; all write span 0), the value −1 for the class's own (`0x4b7150`), the weight the matcher's last read.
- **The window** (`0x1319b0..0x131a20`, also `FUN_001318d8`): from slot 5 down, a filled slot is in it while the next
  newer event's tick (game+0x128 for slot 5) less its tick and span is at most `DAT_004b71a8`. The boot reset writes
  that bound as fps × 2 (`0x132b78..0x132b8c`: `FUN_0030fa50` +0x24 shifted left once); all three save states hold
  120. **Correction to §8:** the decompile's `[0x4b71c4] × 2` source and the "bound of 0" reading were Ghidra's cut at
  `FUN_0030fa50`; the window is two seconds.
- **The update** `FUN_00131560` runs at the top of the game's update (`FUN_0013d868`, `0x13db20`), before game+0x128
  counts on: the newest slot's span grows by one while the ball is in a shot (states 5, 6), a staged sequence runs,
  the holder is in a jump shot, a dunk or an alley-oop's leap (bits 3, 7, 15), or — on counts whose bits 0 and 4 are
  not both set — the ball is in a pass (state 4) or the holder in a trick move (bit 10). Outside a staged sequence it
  then sets +0x148 to −1 and +0x150 to 0, and after a basket (+0x158) it clears the ring.
- **The pusher's gate** (`FUN_00131788`): in the script manager's states 3..7 only classes 6 and 7 enter, and only
  when the staged script answers 1 (`FUN_00337af0`, taken as so).

The dispatchers (by the manager's vtable):

| Slot | Dispatcher | Ring |
|---|---|---|
| 5 | `FUN_00131df0` | arms +0x160 |
| 9 | `FUN_00131d88` | +0x154: the passer on the floor 0, in the air 1, in the air with +0x164 2 |
| 13 | `FUN_00132198` | class 10, or 11 for a fakeout (+8); extra the value class (actor+0x248); value `FUN_001438b8`'s |
| 15 | `FUN_00132588` | class 20, or 21 in the counter stance (`FUN_0016e9b0`); the matcher (rows 45..54) |
| 16 | `FUN_00132700` | class 19; the matcher (rows 40..44) |
| 17 | `FUN_00132650` | clears the ring, then class 16 |
| 19 | `FUN_00131ff8` | skips the match's first pass (+0x16c); a receiver in a leap whose move has flag 0x800 counts nothing; a passout special (ball+0x1ac) class 17 (extra team+0x5c ≠ 3,000, value `FUN_00143a88`), else by +0x154: 0 class 9 for a kick pass (the passer's +0x268 = 5) or nothing, 1 class 8, 2 class 14 to a receiver in a dunk or a leap, else 8 |
| 24 | `FUN_00131e78` | class 15 for a dive's catch (`FUN_0016eab0`) when nothing was pushed since the last take (+0x168); +0x164 the holder in a dunk or a leap |
| 26 | `FUN_001322d0` | the basket's class; the matcher; +0x158 |
| 71 | `FUN_00132278` | class 18 |
| 73 | `FUN_00131f78` | class 13 once armed |
| 74 | `FUN_00132790` | class 12 for contact kind 3 |

The basket's class (`0x1322d0..0x132514`): a basket that wins the game (the game's vt+0x7c, `FUN_0012e890`: the
target reached and a lead of two) by a dunk or an alley-oop (kinds 4, 3) outside the Trick Point game is class 6;
with the ball's gamebreaker (ball+0x178) the value is `FUN_001439e8`'s (Level 1 25,000, 50,000 for a dunk or
alley-oop; Level 2 100,000 inside the arc, 50,000 beyond it, 250,000 for a dunk or alley-oop), plus `DAT_004b7168`
= 25,000 for class 6, else class 7 with extra 0 for a dunk or alley-oop, else 4, plus 1 at Level 2. Otherwise kind 3
is class 1 (3,000, +1,000 and +2,000 by the style bits actor+0x270, extra bit 1), kind 4 class 0 with the finish
table `0x43df38` by actor+0x24c (1,000, 2,000, 3,000, 5,000, 7,500; extra the finish) or class 5 for a tip slam
(actor+0x1e4 1), kind 5 (a lay-up) class 2, a jump shot class 3 (base 3) or 4.

**Corrections:** the classes 20 and 21 are the steal and the trick counter (the pattern rows 45..54 are block and
steal combos), not passes; 19 is the block, 16 the rebound (with its clear), 15 the dive, 17 the passout special;
the rewrite's old `DrainEventManager` had mapped slot 19's pass dispatcher onto its steals and blocks. A pass enters
the ring at its catch and is worth nothing to the meter.

**At run time.** Two save states hold rings the original had just matched (its next update's clear had written −1
over the classes; the classes below are the ones the stored values, extras and the weights the matcher wrote back
fit). Slot 2 (count 470): a plain value-class-2 trick (1,000, span 89) and a dunk of finish 3 (5,000) — row 4, column
0, weights 0.3 and 1.5, oldest tick 332. Slot 9 (count 15,005, the attract match): a trick counter (21), two
value-class-2 tricks (1,000, then 500: the repeat ring halved the second), a value-class-3 fakeout (3,000) and the
lay-up (class 2), spans 47, 46, 49, 106 and 0, each event 22..60 updates after the one before ended — row 11, column
2 (4 − 1 less the (10, 10) pair), weights 2.0, 0.5, 0.5, 2.25, 2.0, oldest tick 14,542 = +0x14c. The port reproduces
both, weight for weight (`combo::tests::the_save_states_matched_rings_pay_as_the_original_weighed_them`). In the
played captures a rebound at update 1,992 of `pads-1` pays with the dunk at 2,203 (+4,375 = 500 × 1.25 + 3,000 ×
1.25): 211 updates apart, inside the window only because the rebound's span grew while the dunker dunked — the span
rule on the captured states gives 126, leaving it 85 updates old.

### The meters' listeners

Each own-side fill is `FUN_00143d38(meter, +0x150 + value)`, the other side's the drain table `0x43e248` (2,500 a
block, 1,000 a steal, 3,000 a counter, 0 for the rest):

| Slot | Listener | Own side | Other side |
|---|---|---|---|
| 13 | `FUN_001441d8` | total + the trick's value (+0x24) | 0 |
| 15 | `FUN_00144490` | total + 1,000 (3,000 a counter) | −1,000 (−3,000); both +0x50 |
| 16 | `FUN_001445d8` | total + 2,500 | −2,500; +0x50 |
| 17 | `FUN_00144558` | total + 500 | 0; +0x50 |
| 19 | `FUN_00144658` | a passout special's flat 3,000 to the receiver's side; +0x50 | |
| 24 | `FUN_001447d0` | without +0x50, a dive's catch total + 500; +0x50 cleared; the repeat ring cleared when the possession changed since its last look (+0x8) | 0 |
| 26 | `FUN_001442b8` | not in a dunks-only game unless kind 3/4; a gamebreaker scales +0x44 by +0x48; a winning basket nothing; else total + the dunk's finish value (+0x54), the alley-oop's (+0x60) or `0x43e1f8` (750/250 shots, 1,000 a lay-up, 2,000 a tip slam) | 0 |
| 73 | `FUN_001440d0` | once armed (slot 5, +0x4c), total + 500 to the side against the jumper | 0 |
| 74 | `FUN_00144158` | contact kind 3: total + 500 | 0 |

`FUN_00143d38`'s gates: the court intro (`FUN_0013e358` = game+0x428's +0x24, the intro task `FUN_00184b00`, shut only
while it plays — not an AI gate as the earlier note said), game types 3 and 7 (config+0xe8 by `FUN_001dbd48`: the
mode's place in My Rules — No Gamebreakers and Back To Basics), the hold; the Easy cap skipped in the Trick Point and
Gamebreaker Wins games and in lessons; at 50,000 a lesson other than the gamebreaker lessons (the game's vt+0xb4 0xe,
0x17..0x19) stops at 49,999. `FUN_00143c20` (the decay) skips type 7.

The played captures confirm the values (side 0 the person, fill 0.9; side 1 the computer, 0.7): rebounds 500, steals
±1,000, blocks ±2,500, long and short shots 750 and 250, lay-ups 1,000, alley-oops 3,000, dunks 2,000, 3,000, 5,000
and 7,500 (the finish table), plain trick moves half their ladder and fakeouts all of it; a person's passes and kick
passes fill nothing. Of the 177 jumps, 154 are a single listener's value, 5 drains clamped at 0, and the other 17
are baskets (and one steal) whose payout a small ring under the ported matcher gives — most a rebound then the basket
(1,187 = 250 + 500 × 1.25 + 250 × 1.25 four times). `fades-1` at update 3,937 holds a defensive combo: the person's
side blocked a dunk at 3,865 (class 19, +2,500) and stole the ball from its holder 72 updates later (class 20): row 49
pays 2,500 × 1.25 + 1,000 × 1.25 = 4,375, and the meter rose by 1,000 + 4,375 at the steal
(`combo::tests::a_steal_after_its_sides_block_pays_both`).

### The trick's repeat ring and the passout special

`FUN_001438b8` and `FUN_00143a88` share team meter+0x2c..+0x38 (four keys, newest first): a trick's script for value
classes 2..4, −3 (or −2 when the passout's state `FUN_00142038` is 2, 5,000) for a passout special; each copy of the
key halves the value, rounded up, at least 1. The catch's listener clears all four when the possession changed. A
passout special is the take of a pass thrown by `FUN_0014c348` (the passout legs `FUN_00148378`, ball+0x1ac set, cleared
by the take): in the rewrite a pass to oneself.

### The statistics

Stat 11 (trick points, which the side's total and the Trick Point game follow) by listener: the trick's value; a
steal's or counter's total + 1,000/3,000; a block's total + 2,500 (and a shot tried to the blocker when the ball's
holder was in a shot, `FUN_00188868`'s tail); a rebound's total + 500; a passout's team+0x5c (and 0x7465 to the other
side's 0x25); a dive's 500 (none after a steal, block or rebound this take, +0xa10); a basket's total + its value
(`FUN_00187c10`: kind 9 the gamebreaker's +0x58 with a game-winning dunk's 25,000 split to 0x1e/0x23, kind 14 a
game-winning dunk's 25,000, else as the meter); a cancel's 5,000; a pump fake's or a pick's 500. The side-only
0x16..0x27 get each source's value and, when a combo paid, `FUN_00187200` adds each walked slot's value to its class's
statistic (`0x440520`); 0x10 the other side's drain. **Assists** (stat 3): a take by the side with the ball keeps its
last holder (+0xfc) as ball+0x10c with `DAT_004b84ac` = fps × 2 (120) updates, counted down while the ball is held
(`FUN_0014e060`); the basket's +8 is he when on the scoring side. `FUN_00186fa8` admits nothing in phases 0 and 9 and,
in a staged sequence, only stats 0..2, 7..0xd, 0x18..0x1b, 0x1e, 0x22, 0x23; types 7 skips trick points.

### The take (`FUN_0014cb28`)

Before the ball is the taker's: a pass in the air (state 4) taken by its side is slot 19, by the other side in live
play slot 15; otherwise in live play a shot off the hoop (6) or a rebound (0) is slot 17; the other side's take of a
shot in the air (5), or of a held ball whose holder is in a shot, a dunk, a lay-up or a leap, slot 16; of a held ball
slot 15; of a loose ball slot 15 unless ball+0x1a4 (its pass's receiver knocked it) or its shooter word +0xf8 is the
other side's last toucher (+0x104), which is slot 25. Then slot 24.

### The pump fake's and the pick's 500

`FUN_00142768` (the game's slot 6) watches a block jumper within 18 of a holder in a jump shot or a pump fake
(game+0x44c). `FUN_0013f950` (each update): with the holder on the floor in a jump shot, a pass or a pump fake (or
within fps × 0.2 of a jump shot that never left the floor, game+0x494), a watched jumper still in his block jump
posts slot 73 once the holder is out of his shot — at once for a pump fake, after more than fps × 0.1 updates of the
shot (game+0x498) for a pass. The pick: the contact handler's screen (30 contacts, `0x135400..`) posts slot 74 with
`FUN_001342d0(defender)`: kind 3 when he is the holder's own defender (`FUN_0016e8f8` = `FUN_0017f238`: the other
side's AIPlayer whose kept man is the holder, else the one in his slot), the holder 15..33.75 from the rim his side
attacks, the defender 12..33.75 (game+0x374, `FUN_0013f748`) and the two within 15; else 2.

### The shot clock (`FUN_0013f1f0`, `crates/street-sim/src/shot_clock.rs`)

The refill `FUN_0013f188` (the game's vt+0xfc: the time to the full one, slot 50, the three waits cleared) comes when
the side with the ball is not the one the clock last saw (`DAT_003d30c4`), when a shot in the air starts down (state 5,
ball+0x44 ≤ 0) and no pass's wait runs, and at the rim or the collar at the end the side with the ball attacks (the
game's slot 30 `FUN_00141d48`, which also stops the clock until a hold, game+0x18c). It runs in live play unless
stopped, run out or a gamebreaker task is busy. Run out (slot 49): nobody holding waits (+0x198) for a holder or a
loose ball or rebound under 12; a holder in a shot waits (+0x1a4) for it; otherwise the violation (slot 52) and the
restart for the other side, or a wait (+0x19c) for a pass to finish. The attract captures' 56 restarts all choose
rows 0 or 1, the full clock's (the shot refilled it falling; the restart's new side refills it the next update).
**Replaced:** the rewrite's resets at the release, the dunk's launch, the lay-up, the swat, the block's knock, the
take's change of side and the inbounds, and its clock that ran only with the ball held or passed.

### Left

- The gamebreaker task's end: `FUN_00144ef8` waits `DAT_004b7d34` = 60 updates (40 after a dunk), plays the slowed
  replays (0.8125 then 0.53125, `DAT_004b7d3c` = 90 recorded frames each) and only then posts slot 65; the sim still
  ends the meter's state at the next take or restart, as the replays are the game crate's.
- The Level 2 gamebreakers' 16 staged sequences against the original running: no capture of them (each needs a
  person's Level 2 gamebreaker from its spot).
- Slot 4 (`FUN_00131e00`, a script opcode's post of the holder) and the passout's state 2 (−2, 5,000); the knock's
  ball+0x1a4 and its +0x10c update; the HUD's pop-up values (the dunk's finish, the passout's +0x5c).
- Restarts (section 5): `RESTART_LIMIT` stays the rewrite's own net (never reached: `ai::tests::every_restart_of_a
  _computer_game_ends_with_an_inbound_pass`, 6 seeds × 20,000 updates); the phase-6 pickup by the restarting side
  only, the lessons' forced row and waits, the formation's last five players, the 46 construction draws and the
  cheats beyond Explosive Rims are unchanged.
- Statistics: controller scope (2), the score snapshot at a fill (+0x9e4).

## The drive, the dunk's window and the counter's blend (2026-10-17)

Branch `drive-counter` (off 698eb88). Traced raw (SLUS_206.51 with
`tools/mipsx.py`) and measured with both attract captures: the AI's
queues (each actor's behaviour nodes by vtable), the playbook's running
play (its object at game strategy +0x8c, the play's vtable at +0xc), the
shot starts by the act on the shooter's queue and by distance, and every
counter press. Seed studies ran sixteen bases at each capture's length,
the all-50 game and the disc's four pairs, main against the branch.

**Where the drive's shots came from.** The original's computer took 4 of
its 42 jump shots with "drive" running (3 of 31 and 1 of 11); the
rewrite's 26% and 31% at the first capture's length (all-50 / pairs).
The drive itself (`FUN_00109898`, raw `0x109898..0x109c7c`; enter
`FUN_001096b8`) is the port's, read line by line: the go-up distance is a
float drawn 5..16 (the decompile's `(int)` is wrong, `0x1097a4` stores
it with `swc1`), the committed shot asks `FUN_00181c68` with the 25%
draw, and its `+0x60` branch (`FUN_00181670` the dunk bit, `FUN_001837d0`
a taller blocker within 6 of a dunker in the air) only taps shoot through
a dunk. Two things upstream differed.

- **The penetration.** `FUN_001287e0` (raw `0x1287e0..0x128a5c`), which
  the play select (`FUN_001281c8`) asks before its lists, starts a
  penetration at once when the holder is in the front court and both
  teammates are still in the back court: the star always, anyone else
  `skill(0, 10, 10, 40)`%. Its loop over the teammates leaves for the
  lane test as soon as one is in the front court (`0x1288a8`: the test is
  "not in front", the branch out when it fails). The port had it the
  other way round ("the whole side in the front court"), so a star
  holder in a set offence always drove. Penetrations a game: 71.0 / 62.3
  → 52.4 / 52.7 at the first capture's length, 26.6 / 23.5 → 19.1 / 20.1
  at the second's, against the matches' 50 and 19; drives 84.9 / 76.1 →
  68.0 / 69.6 (69 and 22 in the matches). The rest of `FUN_001287e0`, the
  select's star list, default list (`0x3ce438`), weights
  (`FUN_00128708`), its check (`FUN_00128668`), the star's refresh
  (`FUN_0012aaa8`, whole seconds mod 60 against 900, so no star after a
  play) and the shot check (`FUN_001290b8`, raw `0x1290b8..0x129588`)
  read as ported.
- **The dunk's window.** The holder's shoot press (`FUN_0015a060`
  `0x15b130..0x15b1f8`) calls `FUN_0016b4b8` with no turbo test, which
  takes the dunk and lay-up rows (`FUN_001693b8(actor, 0)`) when
  `FUN_0016c500(actor, 1)` is 2 short of the rim's line (|x| ≤ 36.5),
  else the jump shots. The AI's pad builder (`0x182494`) and its
  `FUN_00183d90` (`0x183dc4`) ask the same `FUN_0016c500`. The rewrite
  dunked within its own 11 with a turbo and asked that in the AI too
  (`offence::dunkable`). Now both ask `World::dunk_on` (the port of
  `FUN_0016c500` the leap already used): within 15 moving at the rim
  with no opponent within 4.5 in the cone, or within 7.5 standing; the
  turbo the sim requires and spends stays its own (`FUN_001693b8`'s turbo
  test feeds its per-player unlock mask, `0x169698`, not ported). The
  rewrite's jump shots from 10..16 out were 7.7 / 8.7 a game with the
  penetration's fix alone, now 2.2 / 4.0, against the first match's 2
  (both 14..16; none at 10..14) and the second's 1.

Jump shots with "drive" running: 26.0% / 31.2% → 7.3% / 15.1% at the first
capture's length, 37.1% / 34.2% → 8.6% / 14.0% at the second's, against 3
of 31 and 1 of 11.

**The jump shot's slot.** `FUN_0016b4b8` takes the slot's shot script by
the first of `0x43d460`'s six two-turbo pairs the held word covers
(`0x16b760..0x16b7b0`, then `FUN_0013a1f8(record, 2, +0x258, k)`); the sim
indexed its six `SHOT_SLOTS` by the eleven trick masks (the pairs are
5..10 there), which played the wrong script and was out of range past
the first pair. With the AI's dunk combos now pressed where no dunk is
on, that came up. `NativeTricks::shot_slot` is the pair lookup.

**What is left of the drive.** The rewrite's drives still end in a dunk
12 a game against 4 (the first capture's length), and its post-ups dunk
11 / 15 against about 22: the dunks' total is the original's, their split
is not. The original posts up 50 times a game (40 from drives started in
the paint, `FUN_001096b8`'s `[0x28]`), the rewrite 33 / 35. That is the
plays' mix: the first play of a possession, drawn with the star, is a
cutter 13 times in the first match against 2.4 / 5.9, and the 3pt plays
(pen and kick, post and kick, rotate, iso3pt) come more often; whether a
play suits the star turns on his ratings (Threes over 49, Handles over
45), and the attract matches' rosters are not recorded. The playbook's
pass check (`FUN_00128b30`, read raw, its `pb+0xb8` enable set by the
constructor `0x126fb0` and never cleared) interrupts a drive 9..12 times a
game against 3, and the shot check hands one to "shoot" 0.4 times
against 4.

**The held shooters' knocks** (block jumps at a shooter holding the ball;
main → branch, the first capture's length then the second's, all-50 /
pairs): knocking the ball 0.281 / 0.273; 0.317 / 0.250 → 0.190 / 0.181;
0.259 / 0.188, against the original's 1 of 20; passed out of 0.050 /
0.077; 0.083 / 0.086 → 0.080 / 0.151; 0.069 / 0.135, against 5 of 20.
`FUN_00181c68` (raw `0x181c68..0x181f48`) is the port's branch for
branch: the gamebreaker, in a shot, 1 up, the clock under 4, no dunk
(bit 7) nor leap (bit 15), the under-rim case (not a lay-up, the ball no
higher than 12 and within 1.2 of the rim), the baseline inside the arc,
the careful draw, then the nearest opponent in a block jump within 6, no
farther from the rim, his top bone within 1 of the ball and within 156
of the line to the rim. What is left is who shoots and from where.

**The counter's blend.** `FUN_00107710` (raw `0x107710..0x107adc`) was
read again. Against a juking handler (`FUN_001819f8`) it first reads his
channel 0's flags (`FUN_0019c2e0(actor + 0xc4, 0)` +0x48): while 0x200000
is set (a blend, the trick's opening `8e` of 6 or 10 updates) it rolls
nothing and sets its timer to 0, so it looks again next update
(`0x10793c..0x10795c`); after the blend it rolls `skill(−13, −17, 40, 50)`
plus the ratings' edge and waits `fps`. The port rolled in the blend.
Every one of the first match's counters against a trick came 6 or more
updates in (6, 6, 6, 9, 10, 11, 12, 34, 35, 44, 48);
the two that took the ball were pressed exactly 6 in, from 4.2 and 4.5.
On the original's positions at the blend's end a defender stands in
front of the handler within reach in 70..76% of its 99 tricks, and 10 of
them were countered: about 17% a look, the port's odds. The rewrite's
counters against tricks now come 6 or more updates in too.

**The trick pass's counter.** While a trick pass to himself is under way
(game+0x160 not −2) only the defender it goes off (game+0x148) rolls,
`skill(10, 15, 35, 50)`, against the ball's holder `FUN_0013bf60`
(ball+0xf4), which the first capture shows keeping the passer through a
pass's windup and through the 73 updates of trick passes to himself, and
nobody in the other 4,380 updates of a pass in flight. Six of the first
match's counter presses came so, two of them catching the pass off the
head. The port read "asked this frame" (`Game::trick`) and the sim's
handler, empty in the flight; now `ball_holder` and `trick_pass` read the
ball's pass to himself (`Via::Head`, `Via::Glass`).

**The trick pass's flight.** With the disc's bones the computer's trick
passes never flew: the passer's pickup (`FUN_0016cd28`'s reach) took the
first leg back the update it left, because the bone rules' "the passer of
a pass not to himself leaves it" exempts a pass to himself (the leg back
to him must be caught). The original keeps the trick pass in his hands
about 20 updates in state 4 and throws it up past his bones; the
rewrite's throw (its own stand-in, `World::heezay` and `World::glass`)
leaves at once, so the passer now leaves the first leg alone. The disc
pairs' computer tries 4.7 a game at the first capture's length (none at
every rating 50), most of them off the board.

**Measured** (the comparison's 16-base means, main → branch; first
capture, then second):

| | main | branch | original |
|---|---|---|---|
| counter presses | 25.8; 8.9 | 25.5; 7.4 | 17; 3 |
| turnovers out of phase 5 | 7.9; 3.3 | 4.4; 1.2 | 8; 0 |
| within 4 | 0.262; 0.263 | 0.237; 0.239 | 0.262; 0.201 |
| pairs' shots | 52.7; 18.8 | 52.2; 18.7 | 55; 22 |
| block jumps at a shooter (pairs) | 12.4; 4.4 | 13.1; 4.4 | 13; 8 |
| at a dunker (pairs) | 27.9; 10.9 | 26.8; 10.3 | 30; 9 |

Each change switched off alone (the rest on) puts the moves apart:

- the counter's blend alone takes the turnovers from 7.9 to 4.4. Pressed
  after the blend, the rewrite's stance meets the handler less often (its
  contact is the radii, the limbs the original adds are not evaluated,
  `World::counter`; the trick's clip has moved him on): its counters
  land 3 times in the first base game against 11;
- the penetration's fix alone takes within 4 from 0.262 to about 0.24.
  By the handler's distance from the rim the nearest defender stands
  where the original's does (3.55, 5.12, 5.68, 6.14, 6.76, 8.70 by bins
  of 6 against 3.44, 5.03, 5.80, 6.01, 7.28, 8.59), but the handler spends
  fewer updates inside 12 (3,158 against 3,956) and more beyond 30 (5,153
  against 3,461): the plays' mix, not the guarding;
- the dunks' window moves the pairs' shots little (a dunk is knocked
  loose in the hold about 20% of the time, a jump shot a few percent)
  and nothing else beyond its spread.

Counter presses stay high (25.5 against 17): the rewrite's handlers trick
133 / 114 times a game against 99 (the first capture's length), and its
defenders press 4.8 counters a game at a handler not tricking
(`skill(8, 8, −4, −4)`, 2% a look) against the first match's 1 (1.4
against 2 at the second's length). The defenders' turbo, which gates
every look at 0.2, is in neither capture.

The comparison's bounds, set again by the same rule (the gap plus three
standard errors, rounded up): standing 0.035 → 0.045 and 0.04, away from
tricks 0.05 → 0.045 and 0.09 → 0.055, trick length 4 → 3 and 3.5 → 2,
through the tricks 0.025 → 0.04 and 0.09 → 0.06, the blend 0.13 → 0.175
and 0.18 → 0.23, within 4 0.02 → 0.04 and 0.10 → 0.06, the short
capture's turnovers 5 → 2, counter presses 13 → 12.5 and 8.5 → 7, the
long capture's pairs' shots 4 → 4.5, block jumps at a shooter and just
let go 2 → 1.5 each, at a dunker 4.5 → 5.5 and the short one's 3 → 2.5.
Two other seed sets (each seed plus 500, and plus 777) stay inside
every bound. The played captures pass on all twelve.

Tests: `the_star_penetrates_at_once_only_ahead_of_his_teammates`,
`the_counter_waits_out_the_tricks_blend`,
`the_defender_a_heezay_goes_off_looks_to_counter_in_its_flight`
(`ai/tests.rs`); `turbo_and_shoot_dunks_where_fun_0016c500_has_a_dunk_on`,
`the_jump_shots_slot_is_the_first_pair_the_turbos_hold`,
`a_trick_pass_leaves_the_passer_on_the_bones` (`tests.rs`). The old dunk
tests now run at the rim a few updates first (a standing press 7.75 out
has no dunk), and street-game's channel test puts its dunker in front of
the rim (he stood behind the board). No snapshot change.

**Left:**

- the plays' mix (the cutter, the 3pt plays) and with it the post-ups and
  the drive's dunks;
- the counter stance's contact with the limbs, and the 20-update windup
  of the trick pass to himself;
- the handlers' tricks a game, and the counters at a handler not
  tricking;
- `FUN_001693b8`'s turbo and unlock masks (the turbo a dunk needs is the
  sim's).

## The turbo bars, the juke press and the counter stance (2026-10-17)

Branch `counter-tricks` (off ffc80c3). Traced raw (SLUS_206.51 with
`tools/mipsx.py`) and measured against both attract captures and a third
taken for this pass: `tools/pine_ai_capture.py` schema 3 records each
actor's player record (+0x274), updates on court (+0x2a4) and turbo owner
flag (+0x2ac), the six turbo bars (game+0xb0's pointers), the
controllers' map and the playbook's star and its weights (strategy
+0xc8..+0xd4). `street-native-ai-attract-3.jsonl` holds a whole attract
match, 30,920 updates, none torn: Indiana's 45, 47, 46 against San
Antonio's 121, 120, 122. The first capture records no rosters; save state 9
(a run of the same boot's reseed) holds the Lakers' 55, 56, 57 against
Portland's 110, 111, 112, and the first capture's actors fit them by the
gait rows their Handles allow (actors 1 and 2 play rows of Handles 80,
actor 0 none; of the 28 teams only the Lakers and Orlando have two such
players, and Orlando's first would play them too), so the rosters' runs
below take them for it.

**The turbo bars** (`crate::turbo`). The game keeps six bars
(game+0xb0, each topped up by 1 at every restart, `FUN_001406b8`). A
player's is `FUN_00142098(game, FUN_00156d98(actor), team)`: a person's
controller has its own (game+0xa0[controller], which `FUN_0013cf58` sets to
`DAT_0043ded0` of his side, 3 for side 0 and 0 for side 1, plus the people
of that side before him), and every player no person holds
(`FUN_00156d98` 4) draws on his side's `DAT_0043ded0` bar. So a computer
side's three share one bar, and a person's computer teammates share his.
The bar fills and drains once an update, in one actor's update (actor+0x2ac,
`FUN_001587d8` raw `0x1588f0..0x1589d0`), before his side's update: on a
side a person plays, each person's actor; on a computer side, while
somebody holds the ball (ball+0xf4), the holder on the side with it and on
the other side the player at the holder's slot (`FUN_00140920`); while
nobody does, whoever it was. It drains `1 / (fps × 5)` when a turbo button
is held in live play (`FUN_00140558`), else fills `1 / (fps × 7.5)`. The
rewrite kept a bar per player that every player filled and drained after
his move.

The moves that cost a bar (`FUN_00190c08`'s callers) ask for it first and
spend it: the juke press 0.2 for any trick (`FUN_0015c8e8`, `0x15c9e4`) and
the clearout 0.2 (`FUN_0015c778`, `0x15c8c0`); the counter stance 0.2
(`FUN_0015da28`, `0x15e094`); the turbo block 0.5 (`0x15d428`); the trick
pass to himself 0.2 (`FUN_00146278`, the test at `0x1469d4`, the spend at
`0x14741c`); a person's two-turbo taunt 0.5 (`0x15b750`). Short of it, the
"no turbo" message (`0x3c6520`) and nothing. The rewrite's tricks cost
`tuning::TRICK_TURBO` by value class (0 to 0.35; gone), its counter 0.1
(and a swipe without it), its trick passes 0.15. The dunks' and the
gamebreaker's costs stay the rewrite's (`FUN_001693b8` reads the turbo
buttons, not the bar).

In the third capture the bars hold slots 0 and 3 only. Each actor's +0x2ac
follows the owner rule in 185,099 of 185,508 actor updates, 407 of the rest
by the holder as the update began (the actors update from the highest
index, so one updating before a catch reads the old holder; the port
computes it in each actor's update), 2 left. The bars average 0.83 and 0.87
and are under 0.2 in 50 of 61,822 samples (lowest 0.15 and 0.16): the bar
seldom stops a move, in the original or now in the rewrite (its handlers
press at 0.71 on average). (`$EVIDENCE/street-turbo-star-attract-3.json`.)

**The juke press** (`FUN_0015a060` `0x15ad10..0x15ad2c`). First
`FUN_0015c778`, the clearout: the holder standing still (the velocity the
zero vector), on the floor, out of a restart, not dribbling (+0x1a0 not 1),
not posting up, with 0.2: `clearout01` (658, script `0x456d02`, a kind-2
contact window of 40 updates). Else `FUN_0015c8e8`: on the floor, not out
of a dribble he faked (+0x1a0 at least 0), with 0.2 (spent); then with two
turbos held, or the nearest defender not squarely in front (`FUN_0018d1f8`
over 0x40, or the defender's over 0x80), the trick press (`FUN_0015f0f0`);
else against a defender standing still on the floor and not down, the
holder standing or barely pushing (+0x90 at most 15), the two-person
freestyles: within 4.75 of his height scale one of `0x44d2cf`, `0x44d9a8`,
`0x44d9f9` (`r % 100` under 33, 66) with the defender's faked reaction,
4.75..8 one of `0x44d423`, `0x44d381`, `0x44d3d2`, beyond 8 the trick
press. Ported: the gate and the 0.2 (`World::juke_press`). Not modelled:
the clearout (4 in the first capture's phase 5, none in the others; the
rewrite's holders meet its conditions at 0.6..1.7 juke presses a game and
trick there instead) and the two-person freestyles (none in the three
captures' 198 tricks).

**`FUN_00182878`, the computer's juke test**, read raw again
(`0x182878..0x182c54`): the port read "down" where the original asks the
counter stance (`FUN_00181828`: an opponent within 7.5 in the stance puts
it off unless `skill(0, 0, 4, 4)` is under 2), left out the faked dribble
(`FUN_00181750`) and the catch (bit 9, `FUN_001816d0`, which like standing
still stops a juke more than 17 out), and did not track the recent event
(`FUN_00183af8`): the combo ring is the holder's side's, the ball in the
half it attacks, and one of the ring's first 22 classes (`FUN_00132818`,
the newest slot of each, the walk stopping at an empty one) was pushed
within `fps × 2` updates (`DAT_004b9d98`, written at `0x1841b8`). Then the
juke's reach is 4.9 and 7.5 (else 4.5 and 7) and `FUN_00183be8` adds
`skill(0, 5, 40, 40)` to the odds. The original's holders are in such a
moment in 43% of the third capture's held updates (the tricks' classes 10
and 11 most), the rewrite's 45% with the same rosters. Off the Heezay's
test (`FUN_00182d10`, `0x182fd0`) asked "down" for the counter stance too.

**The counter stance** (`crate::counter`). The press (`FUN_0015da28`,
raw `0x15df60..0x15e0e4`): steal with two turbos held (+0x78 & 0x40000),
on the floor, not swiping nor in the stance, without the No Counters cheat
(`FUN_00131208()+0x78`, with which it is the swipe), and 0.2 of the bar.
One of `0x3d7228`'s two scripts by `r & 1`, then the face
(`FUN_00159190`: its kind by `r & 1`, and its own draw). Both scripts zero
the velocity (`97`) first, set bit 16 (actor+0xa6 bit 0), blend five
updates to `STEALCOUNTER_1_2` (402) or `STEALCOUNTER_2_5` (403) from frame
0, play the rest at 1.25 updates a frame (`8c`: 72 and 62), clear the bit
and give the gait back: 77 and 67 updates, as every stance that ran out in
the captures lasted. Their first `91` (`02 00 82 00`) sets channel flag 2,
which keeps the stick (`FUN_00163d40`) off, and leaves 0x1000 clear, so
the clip's root carries him: a lunge of about 2 forward and back. His
facing turns to the one he pressed with (actor+0xb0), 46 an update; his
bones are the clip's (`World::scripted_pose`), so his head reaches toward
the handler in the contact's radii (`FUN_001337e0`). The rewrite's stance
was a 77-update timer that walked him at 0.4 along the stick on his gait's
bones.

**The contact** (`FUN_001343e8`, raw `0x134ac8..0x134f48`): a defender in
the stance meeting, within their radii, a handler in a trick (bit 10,
`FUN_0016ea10`) whose channel 0 is not blending (0x200000), holding the
ball, in the defender's front sectors 3..5. The odds as before (Steals less
Handles kept to ±50, plus 50, times 0.8 truncated, plus 20); in the Trick
Counter lesson (vt+0xb4 10) it lands with no draw. A miss (`r % 100` over
the odds): bit 16 cleared, the defender plays `0x44bf94`
(`standstumbleback` to frame 18 over 23 updates, 27 in all; the rewrite
stumbled him 54), the contact's message (`0x3c6910`) makes the handler his
AI+0xb0 (`FUN_00141448`), and the handler's +0x198 is cleared (his fake
window). A landing: the handler plays `0x44bf61` (`standstumbleback` at 2
updates a frame, 68 updates; the rewrite's stumble 60) and his AI hears
the defender screened him; his face (two draws), the take-or-knock draw
(`r % 100` over 100: always the take), the take (`FUN_0014cb28`), the
defender's dribble at 0, his face (two draws), and his take: `0x45795d`
after a trick on the records that never fake (`TWOPERSON_FREESTYLE_05_1`
from frame 5, 25 updates), else, after one more draw it does not use,
`0x3d0fa0`'s `0x457873`: bits 16 and 6 (the swipe's, so his hands count in
the radii), a blend of five to `STEALCOUNTER_4_4_L` (405, a left-hander's
404) from frame 17, the rest at 2 a frame (41 updates; he runs about 8 on
with the ball), the facing from the pelvis (callback 7) and the gait back.
The rewrite freed the defender at once and drew 3 times where the original
draws 7. The scripts, tables and constants are read again from the disc by
`tests/trick_disc.rs` (`the_counter_scripts_are_the_executables`; the two
stumbles sit at the end of `trick::REACTIONS`, checked with the rest).

Every trick counter in the captures that took the ball took it within the
stance's blend (1 to 4 updates after the press; 4 in the first match, 2 in
the third, pressed from 3.5 to 4.6 away), so the stance's own clip meets the
handler in the misses and the run-outs: the counters land about as before.

**The plays' mix and the star.** `FUN_001281c8`'s list and the plays'
tests (`FUN_0011c728`, the cutter: not holding the ball; `FUN_0011e160`,
`FUN_00120758`, `FUN_0011fd70`, `FUN_00122b20`: Threes over 49) read as
ported. (`FUN_0012bc40` is the restart strategy's start, ported and checked
by `restart_plays_and_spots_follow_the_captured_original`.) The star
(`FUN_0012a7d0`) recomputed from the third capture's statistics (points and
shots tried, stats 0 and 10, by side and player) and updates on court
gives the recorded star at all 60 of its play starts and its weights in 58
(two one percent off). So the mix turns on who shoots: a player who has
shot less than his rating's share gets the star, and the star's ratings and
whether he holds the ball pick the list. With the third capture's rosters
the rewrite's first plays come close (the cutter 6.0 a game against 7,
isolations 10.9 against 9, penetrations 14.3 against 11, picks 11.2 against
23), but its San Antonio spreads the star over its three (12.6, 8.3 and 4.2
first plays by slot against 3, 22 and 0): in that match 121 shot once and
Duncan (120) four times, where the rewrite's take 7.8 and 7.1 shots a
game. With the first capture's rosters the cutter starts 5.0 first plays a
game against 13, and the 3pt plays come more often. Who shoots is the shot
choice's and the plays', not traced in this pass.

**What the tricks' count is made of.** By what the holder is doing, the
rewrite's holders juke at about the original's rates (third capture, juke
presses per 1,000 held updates, original then rewrite with its rosters:
drive 12.7 / 9.6, post up 11.8 / 7.2, isolation 3pt 11.9 / 12.6, carrier
move 5.5 / 5.7, carrier pause 1.5 / 2.5). The time differs: they hold the
ball 15,410 updates in phase 5 against 13,929, 3,574 of them in "drive"
against 2,213 (57 drives a game averaging 63 updates, median 42, against 48
averaging 46, median 16; the first match's 69 have a median of 1: most of
the original's drives start in the paint and post up at once,
`FUN_001096b8`'s [0x28]). That and the 3pt plays' isolations are the extra
tricks.

**Turnovers and counter presses.** The original's turnovers out of phase 5
are its trick counters (4, 0 and 2 in the three matches) and its counter
stance's catches of a head pass (2, 0 and 1); its first match's 2
gamebreaker baskets (into phase 6) are scores, which the comparison now
leaves out as it leaves out the rewrite's. Every player at 50 throws no
trick pass (Handles under 70), so the comparison's games have no head
catches. With the captured rosters the rewrite throws about the original's
number of trick passes to himself (11.0 and 10.7 a game against 9 and 13)
but almost all off the board: Off the Heezay 0.6 and 0.25 a game against 8
and 9 (the original's told by where the ball flies). Its test
(`FUN_00182d10`) is the port's; of the rewrite's tries about 1 in 25
reaches the stick at rest with the defender within 12, and 1 in 7 of those
finds him standing still, so the counter presses during a trick pass (6 and
3 in the first and third matches, 0.1 in the rewrite) and the head catches
are missing: the lead for the next pass. The counter presses at a trick
follow the tricks (0.14..0.16 a trick in the rewrite, 0.10 and 0.17 in the
first and third matches); at a handler not tricking 3.3..5.3 a game against 1, 2
and 0.

**Measured** (16-base means of games of each capture's length; every
player at 50 unless named; the first capture, then the second; the
rosters' runs at their capture's length):

| | main | branch | original |
|---|---|---|---|
| tricks (all-50) | 134.3; 46.1 | 136.2; 46.3 | 99; 28 |
| tricks (disc pairs) | 113.7; 38.8 | 122.9; 42.8 | 99; 28 |
| counter presses | 23.9; 8.8 | 24.3; 8.1 | 17; 3 |
| counter stances | 11.7; 4.7 | 13.5; 4.9 | 13; 3 |
| counters landed | 4.3; 1.3 | 4.4; 1.9 | 4; 0 |
| turnovers out of phase 5 | 4.2; 1.3 | 4.4; 1.8 | 6; 0 |
| Lakers / Portland: tricks, presses, turnovers | 115.0, 20.0, 4.3 | 115.6, 22.1, 5.3 | 99, 17, 6 |
| Indiana / San Antonio: tricks, presses, turnovers | 88.6, 15.7, 3.1 | 92.3, 16.4, 2.3 | 71, 15, 3 |

The original's turnovers leave out its first match's 2 gamebreaker
baskets (main's comparison counted them, 8). The rest of the comparison
moved within its spread (within 4 0.239 → 0.243 and 0.243 → 0.231, shots
52.8 → 52.5 and 18.2 → 18.0). Its bounds were set again by the same rule
(the gap plus three standard errors, rounded up), the turnovers and the
counter presses now checked both ways: standing 0.045 and 0.04, away 0.045
and 0.065, trick length 3.5 and 3, in tricks 0.045 and 0.04, the blend 0.19
and 0.25, within 4 0.035 and 0.045, turnovers 3.5 and 3, counter presses
12.5 (the rule's 10 failed the seed set +777, 27.1 with an sd of 5.5) and
7, shots 4.5 and 5, block jumps at a shooter 2.5 and 4, just let go 1.5 and
2, at a dunker 6 and 2. The seed sets +500 and +777 stay inside on both
captures.

Tests: `tests/counter_stance.rs` (the contact after the blend and its
scripts, the wait for the blend, the lesson, the press and its 0.2, the
juke press's 0.2, the shared bars and their owners);
`the_counter_scripts_are_the_executables` (`tests/trick_disc.rs`); the
class-four trick's and the matched slot's turbo (`tests.rs`, `snapshot.rs`).
Snapshot VERSION 50 (the bars, their owners, the people, the stance).

**Left:**

- the clearout (`FUN_0015c778` and its kind-2 contact) and the two-person
  freestyles (`FUN_0015c8e8`);
- who shoots (the star's feedback), and with it the plays' mix and the
  time in "drive" (the original's post-ups at once from the paint);
- Off the Heezay's rate (0.25..0.6 a game against 8 and 9), and with it the
  head catches and the counter presses during a trick pass;
- the counter presses at a handler not tricking (3.3..5.3 a game against
  1, 2 and 0);
- the head catch's own script (`FUN_00148378` kind 2) is not traced.

## The third attract capture as a check (2026-10-17)

Branch `attract-3` (off 6b24a55). The third attract capture
(`$EVIDENCE/street-native-ai-attract-3.jsonl`, `tools/pine_ai_capture.py`
schema 3, Indiana against San Antonio, 30,920 updates) failed four of the
fifteen attract tests. Each failure was the test's; one of them showed a
port's stand-in (the dunk's pace), now ported. The capture is in the
standard gate with the other two.

**The phase replay's basket.** The replay
(`the_phase_machine_replays_the_captured_phases`) stands in for the score:
a match that reaches phase 9 is won by its last basket, which it found as
the ball going down through a ring from state 5 to 1. The third match was
won by a gamebreaker jump shot (launched at update 30,667) that took the
rim (state 6 at 30,733), rolled on it 37 updates and dropped in: state 6 to
1 at 30,770 (from 11.9725 to 11.9134, under the rim's 11.91355), phase 9 at
30,771. The replay took the basket before it (29,764) for the winner and
called the game won at the first settled ball after, about 1,000 updates
early. A basket off the rim is 6 to 1 at the ring (9 more in the first
match, 1 in the second and 4 in the third, each going to the restart in its
update); the replay now takes either, and feeds the machine the ball's
state before the ring in the basket's update. The machine was right: 30,909
of 30,919 updates agree (99.97%), the possession in all, game+0x4a4 in
30,917; the first two captures as before (40,873 of 40,886 and 14,149 of
14,155).

**Samples read inside an update.** The capture tool brackets a sample by
the update counter (game+0x128), but the original updates its AI teams
first in an update and counts the update later, so a sample whose batches
all fall between the two is bracketed as whole while it holds the next
update's AI: the team timers (its second batch) one update on and, when a
team redrew, the generator (its first batch) past the redraws (the AI's
are an update's first draws). The read may fall between the two sides' AI,
side 0's first. 16 of the third capture's 117 opening samples were read so
(schema 3 reads more a sample), one of the second's and none of the
first's. Their draw counts look ordinary (59 to 92 an update); the redraws
show it: at update 56 of the third, team 1's timer reads 15 a sample early,
and the draw just before that sample's generator gives 15 (`r % 9 + 9`),
the one after it 9. `opening_draws_are_the_team_timers_in_the_originals_order`
now carries the rewrite's own timers from the first sample: a team whose
timer is not ours was read one update on, its redraw is stepped back off
the generator before the update runs, and its timer must be the one that
update leaves. All 117 opening updates of each capture compare (86 of the
third before, 31 left out as torn); such samples must stay under a quarter
(a timer the port counted an update late would read one update on in every
sample). A tool that kept samples whole would have to know where the EE
stands within an update, which PINE does not tell.

**The dunks.** `the_captured_dunks_leave_the_launcher_as_ported` allowed one
miss; the third match has 19 dunk launches and 17 through the ring the next
update. One of the other two is a make: a gamebreaker dunk at 16,562 whose
hand moved 0.296 in the update before, under the top pace (0.4889), so the
ball went below the rim two updates on (the launch's own move left it at
12.21). The other (25,537, an alley-oop finish by Jermaine O'Neal, actor
bit 15) and the first match's at 3,973 are misses off the rim: the
launcher's kind 3, the ball on the hoop (state 6) the next update, then a
rebound; both placed as ported, neither a gamebreaker's (100%; the release
callback's odds are at most 98%, `shooting::dunk_percent`). The test now
takes a make at the update its flight at the ported pace goes below the rim
(53 the next, 1 the one after) and a miss as the hoop's touch first, never
a gamebreaker's: 56 launches placed, 54 made, 2 missed.

The port sent every dunk at the top pace, its dunker having carried the
ball at the waist. The launcher reads the length of `ball+0xa0` (raw
`0x14ac7c..0x14acf8`, the length in VU0 macro code): the top outright for
actor bit 8 (`FUN_0016e9f0`, a rebound jump) or dunk id 0x15 (actor+0x1e8:
`dunk_3`, script `0x4481ac`), else the move held to 10..20 mph a frame. The
held ball is on the dunker's hand now (`crate::held`), and the rewrite's
slams find its last move under the top pace in 18% and 20% of them with the
first and third matches' players, against 14 of the captures' 56 (25%).
`World::launch_dunk` takes it (`Ball::moved`; the top for the putback, the
sim's rebound jumper's dunk, and for `dunk_3`; online, without the disc's
bones, the top). The played captures' 25 tests pass unchanged.

**The comparison on the matches' own players.**
`shots_turnovers_and_spacing_against_the_captured_original` ran every player
at 50 and four fixed pairs of the disc's teams against each capture. It now
runs each capture's own six (`captured_rosters`): the third's records
(actor+0x274), and the first's as the turbo bars' pass inferred them (save
state 9, a run of the same boot's reseed, holds the Lakers' 55, 56, 57
against Portland's 110, 111, 112, and the capture's actors fit them by the
gait rows their Handles allow; the capture is known by its first sample's
generator). The second capture records none and keeps the all-50 games and
the pairs. The trick length is met like with like, over the moves that ran
their script out: the original cuts moves short by letting go of the ball
out of them (a pass or a shot: none in the first match, 8 in the third) or
losing it (5 and 6), which pulled the third match's mean to 52.6 (the moves
that ran out 60.7; the rewrite's 59.9, all its moves 57.7). Each capture has
its own bounds by the rule (the gap plus three standard errors over 16 seed
bases, rounded up; the seed set +777 stays inside every one), the means in
`Bounds`'s notes. Open, as the like-with-like runs show them: the third
match's block jumps at a shooter (10.3 a game against 17) and after a
release (6.1 against 3), the first's at a dunker (24.9 against 30), and its
counter presses (21.8 against 17).

**The computer's play on the matches' own players** (16 games of each
capture's length; the first match, then the third):

| | rewrite | original |
|---|---|---|
| turnovers out of phase 5 | 4.8; 2.4 | 6; 3 |
| counter-press updates | 21.8; 16.2 | 17; 15 |
| trick starts in phase 5 | 116; 89 | 99; 71 |
| held updates fronting "drive" | 4,334; 3,612 | 3,352; 2,213 |
| drives | 63; 55 | 69; 48 |
| shots (entries into phase 3) | 53.8; 40.1 | 55; 43 |
| Off the Heezay | 0.8; 0.3 | 2; 3 |
| a trick's own toss to himself | 6.3; 8.1 | 4; 5 |
| trick passes off the glass | 3.8; 1.5 | 1; 4 |

Who shoots, by actor (the first match: Shaquille O'Neal, Kobe Bryant, Derek
Fisher; Rasheed Wallace, Scottie Pippen, Damon Stoudamire; the third:
Jermaine O'Neal, Ron Artest, Reggie Miller; David Robinson, Tim Duncan,
Steve Smith): the rewrite 4.9, 11.8, 10.5; 7.8, 9.2, 9.6 against 12, 10, 7;
4, 9, 12, and 6.2, 5.2, 7.9; 5.6, 5.8, 9.4 against 11, 6, 7; 1, 4, 14. In
both matches the original's first actor, side 0's big man, shoots about
twice as often as the rewrite's and side 1's big man far less: the rewrite
spreads the shots. The shot choice and the plays' shooters are not traced
in this pass; the turnovers and the third match's counter presses are the
original's, the tricks and the time in "drive" still more.

**Off the Heezay, corrected.** The turbo bars' pass counted the original's
trick passes to himself by where the ball flew (near the rim the glass,
else the head): 8 and 9 a game against the rewrite's 0.6 and 0.25. By the
ball's path they are three kinds. A trick's own toss: the passer in his
trick move (bit 10; all five of the third match's on script 69), the ball
within 2.4 of him for 27 updates; 4 and 5. Off the glass: 87 to 99 updates,
up to 15 high near the rim; 1 and 4. Off the Heezay (the trick pass node,
`FUN_00146278`): a windup of 13 updates with the ball in the hand, a flight
of 2 up to 6.5..6.9 beside the defender, a dead stop at his head (the
speed under a tenth in an update) and a fall back to the passer; 2 and 3,
of which the defender's counter stance caught both of the first match's and
one of the third's. So the rewrite's tosses come about as often, and Off
the Heezay 0.8 and 0.3 a game against 2 and 3: still short, by one or two a
game, not eight. (Left over: the first match's floor bounce and a 9-update
pass, the third's 27-update pass at 4,583.)

**Tests.** The four as above; `the_captured_dunks_leave_the_launcher_as_ported`
reports the makes by updates to the ring and the misses. The gate runs
`STREET_AI_CAPTURE` on all three attract captures (each with
`STREET_AI_CAPTURE_REPORT`) beside the twelve played captures.

## The computer's shooters, drives and trick passes (2026-10-18)

Branch `computer-play` (off 6bc2fcf). The third attract capture showed the
computer's shots landing on other players than original's, too long
in "drive" and too few Off the Heezay passes. Traced raw (SLUS_206.51 with
`tools/mipsx.py`) and measured on the three attract captures, each
match's own players, 16 seed bases at each capture's length; the
decisions were also replayed on the original's own states (a world
standing where the capture's six stand, the ball with its holder or a
pass's receiver), which tells a logic difference from a situation one.

**Power and the overall rating.** The sim kept eight ratings; the AI's
reads of rating 4 (power) and 9 took stand-ins: the mean of dunks and
rebounds, and the mean of the eight. `FUN_001d7d00(record, k)` reads the
record's field 16 + k (the cheats' `0x45fe20`/`0x45fe50` aside); for a
created player (team 0x25) rating 9 is `0x45fdc0` at his shots bar plus
his dunks bar (`FUN_001d7f48`, 0..6 each; two full bars read the table's
0). Rating 9 is `FUN_00181bf8(ai, 0)`: the star's share (`FUN_0012a7d0`)
and the playbook's first weights (`FUN_00126f60`). Rating 4: the post
up's and back down's tests (`FUN_00121398`, `FUN_0011bce0`), the play
list's extra post up and back down (`FUN_00128708(.., 4, ..)`), the
screen's reaction (`FUN_001343e8`), and the block test's and the rebound
jumper's odds on the rim (`FUN_0016d888`, `FUN_0016ed18`: twice Rebounds
plus Blocks, Speed and Power, over 5). The records against the
stand-ins: Shaquille O'Neal's overall 90 (47; his share of the Lakers'
36%, not 26%), Kobe Bryant 95 (75.6), Derek Fisher 65 (60.1); David
Robinson 70 (59.4), Tim Duncan 90 (62), Steve Smith 80 (58.4); power
Smith 60 (40.5), Fisher 50 (45), Ron Artest 70 (50). The third capture's
San Antonio weights at its first play, 29, 37 and 33, are the records'
70, 90 and 80. `Ratings` carries `power` and `overall` now (snapshot
VERSION 51), the game's setup reads fields 20 and 25 (the Gym's power
point counts; a created player's overall by the table), and so do the
online rosters and the test setups; at every rating 50 nothing changes.
`the_playbooks_star_follows_the_records_overall_rating` recomputes the
star from the third capture's own statistics (stats 0 and 10,
actor+0x2a4): the recorded star at 78 of 78 play starts, the weights
within a percent at all of them. The post cut and the alley-oop make
their cutter and leaper the star (`FUN_00129a20`, `FUN_00129d98`, both as
ported) and are left out.

**The play choice on the original's states.** At each of the third
capture's play starts, with the recorded star and the play before,
`FUN_001281c8`'s lists (`playbook::play_list`) hold all 82 plays the
original chose there, with the side's meter mode (`[0x3d]`, not
recorded) one way or the other
(`the_play_choices_lists_hold_the_originals_plays`; the penetrations,
which also start from the lane's count, `FUN_001298c8`, and the post
cut, alley-oop and pull up are left out). The pen and kick, 3pt pick and
roll, post and kick, rotate and 3pt pick off ball the original ran with
Duncan (Threes 20) its star off the ball come from the default list when
San Antonio played for its meter: `FUN_00128668` keeps only the plays a
full meter allows, so the star's list is empty. Read raw against the port
and found as ported: the shot check `FUN_001290b8` (the qualities by the
shot clock, its two timers, the drive's past-a-defender loop at
`0x129480`), the pass check `FUN_00128b30` (its loop at `0x128d30`: more
than 6 away, more than 5 up court, open longer than `fps / 2`, or `fps /
10` with two defenders on the handler outside a pick and roll, the most
open at 6 or more), the trick pass out `FUN_00128ec0`, the penetrate
check `FUN_001298c8`, the post cut check, the open counters
(`0x18a85c..0x18a8f0`: two a frame at openness 5 or more, wrapping at
`fps × 4`, never cleared while closed), the star's drop after a play
(`FUN_0012aaa8`), the penetration play's start and done (`FUN_0011e528`,
`FUN_0011e4a8`), the post up (`FUN_00115628`), the cutter (`FUN_0011c300`),
the pass target (`FUN_00182028`) and the role tables `0x3dce68` and
`0x3d9f60`, apart from the two below.

**The cutter's facing.** `FUN_0011c300` counts the pass to the cutter
down while his facing (actor+0xa8, `FUN_0013aae8`, raw `0x11c3a8`) is
within 200 of the rim (`FUN_0017da80`); the port read his stick. Every
other reader of `FUN_0013aae8` already read the facing.

**Busy while a pass to himself is in the air.** `FUN_00181508` ends (raw
`0x181608..0x181634`) with the ball passed (state 4) and its last holder
(ball+0xfc) its target (+0x100): busy, for every actor. The port lacked
it, so the playbook's checks, the shoot, step back, carrier pause and
post up updates, the defence's spot, steals and counters, the lane test
and the held shooter's pass out (its callers) went on through an Off the
Heezay, a Back 2 Papa and a trick's own toss. Now `offence::busy` has it
(`everyone_is_busy_while_a_pass_to_himself_is_in_the_air`).

**Measured** (16-base means of games of each capture's length on each
match's own players, main → branch, the original in brackets; the first
match, then the third):

| | main | branch | original |
|---|---|---|---|
| held updates fronting "drive" | 4,334; 3,612 | 4,249; 3,686 | 3,352; 2,213 |
| Off the Heezay | 0.81; 0.31 | 0.81; 0.56 | 2; 3 |
| a trick's toss; off the glass | 6.3, 3.8; 8.1, 1.5 | 7.7, 4.0; 8.9, 1.6 | 4, 1; 5, 4 |
| block jumps at a shooter | 15.4; 10.3 | 13.4; 11.6 | 13; 17 |
| after a release; at a dunker | 8.3, 24.9; 6.1, 19.8 | 7.6, 24.4; 5.6, 18.2 | 7, 30; 3, 20 |
| turnovers out of phase 5 | 4.8; 2.3 | 5.3; 3.1 | 6; 3 |
| counter presses | 21.8; 16.2 | 21.3; 15.9 | 17; 15 |
| trick starts | 116; 89 | 120; 93 | 99; 71 |

Shots by actor: the first match (O'Neal, Bryant, Fisher; Wallace,
Pippen, Stoudamire) 4.9, 11.8, 10.5; 7.8, 9.2, 9.6 → 5.9, 11.4, 8.1;
7.8, 9.2, 9.4 against 12, 10, 7; 4, 9, 12; the third (Jermaine O'Neal,
Artest, Miller; Robinson, Duncan, Smith) 6.2, 5.2, 7.9; 5.6, 5.8, 9.4 →
6.5, 4.4, 9.9; 4.9, 5.9, 8.4 against 11, 6, 7; 1, 4, 14 (standard errors
0.4 to 0.8). The stars at play starts moved most: the Lakers' none,
O'Neal, Bryant, Fisher 26, 17, 16, 17 → 23, 26, 16, 7; San Antonio's
none, Robinson, Duncan, Smith 21, 17, 14, 8 → 18, 14, 16, 9, against the
original's 25, 3, 28, 1. The second capture (every player at 50) moves
only by the busy term (within 4 0.242 → 0.244, the blend 0.410 → 0.468).

**What is left, and why.** Each with the evidence:

- *Who shoots* follows the match's course and the star's feedback, both
  of which the ported pieces above feed. The third match went 21 to 12 to
  Indiana (18 of 24 shots made against 11 of 19); the rewrite's games run
  even (17.8 to 18.3 points at the capture's length, Indiana a point
  behind at its play starts), so San Antonio plays for its meter at 16%
  of its play starts, where the original's ran ten plays only the meter
  mode allows (its pen and kicks, 3pt pick and rolls and the like), and
  Smith's 3pt plays come less. The
  original's Robinson took no shot for 28,000 updates: his weight stayed
  his share (29%) under Duncan's, he had the ball 31 times, 24 of them far
  out, passed 26 times and never drove or posted up; the rewrite's
  Robinson posts up and drives from his first shots and his weight then
  rises above Duncan's (`FUN_0012a7d0` scales a player by the side's rate
  over his own). The original's side-0 big men took 6 of their 12 and
  11 shots from post ups (11 of Jermaine O'Neal's 27 catches were in the
  paint, against the rewrite's 5.9 of 24.6), and the shot check called 30 and 27 "shoot"s a game (from isolations and pick
  and rolls most) against 22.5 and 14.5, with fewer pick and rolls run
  (2,238 held updates in "run pick play" in the third match against
  1,399). The lane is not the cause: the rewrite's lane test on the
  original's positions, its defenders' reactions marked down (each
  reaction script sets channel flag 0x40 with `91 42 00 ..` and clears it
  before the gait comes back), opens about as often as on the rewrite's
  own (the holder 16 to 25.75 out 25% against 26%, farther 14% against
  18%), and the holder's nearest defender stands at the original's
  distance and angle (6.7 and 7.6 against 6.2 and 7.5 by the two bands,
  31° and 24° off the line to the rim against 32° and 27°).
- *The time in "drive"*: inside a drive from 16 or more out the rewrite
  is the original (66% of the updates in a trick against 68% and 71%, 0.35
  closer to the rim a free update against 0.32 and 0.34, a trick every 70
  to 80 updates), and the drives started within 16 post up at once in
  both. The long ones run longer: from 28 to 34 out they end 13 out after
  113 updates against 19 out after 72 (the third match), the original's
  more often in a pass or a shot call on the way (7 of its 28 with the
  ball in the air, 4 in a shot call). That is who holds the ball and the plays, above.
- *Off the Heezay*: the choice (`FUN_00182d10`) and its callers read as
  ported. Its head branch needs a holder of Handles 70 with his stick at
  rest and his slot defender 4 to 12 away at speed level 0; with the
  matches' players that holds 715 and 328 updates a game against 985 and
  612, in carrier pause (the original's Heezays came from carrier pause,
  feeder, a pick and the shoot node) 350 and 144 against 486 and 496: less
  time in carrier pause for those holders (773 updates against 1,200 for
  Miller and Smith), the stick at rest less (50% against 68%), the
  defender close less (73% against 88%) and standing less (51% against
  70%). No single step.
- *Block jumps at a shooter* in the third match follow its shooters
  (Smith's 14 shots from beyond the arc); the first match's are the
  original's now.
- *Turnovers and counter presses* sit within the comparison's bounds on
  both matches.

The comparison's bounds, set again by the rule (the gap plus three
standard errors, rounded up; the seed sets +500 and +777 inside every
one): the first capture's trick length 0.5 → 1.0, in tricks 0.05 →
0.035, the blend 0.16 → 0.115, turnovers 3.5 → 2.5, counter presses 8 →
7.5, shots 5 → 6, block jumps at a shooter 5.5 → 2.5, after a release 3 →
2.5, at a dunker 8 → 10; the third's standing 0.045 → 0.05, away 0.025 →
0.02, length 2 → 1.5, in tricks 0.06 → 0.055, the blend 0.19 → 0.13,
within 4 0.03 → 0.035, turnovers 2 → 2.5, at a shooter 8.5 → 8, at a
dunker 3.5 → 5.5; the second's standing 0.04 → 0.03, away 0.065 → 0.07,
in tricks 0.04 → 0.06, the blend 0.25 → 0.16, within 4 0.045 → 0.065 (the
seed set +500 read 0.249 against 0.201), turnovers 3 → 2. No check of the
shots by player: they do not match.

Tests: `the_playbooks_star_follows_the_records_overall_rating` and
`the_play_choices_lists_hold_the_originals_plays` (`ai/capture.rs`, the
third capture; the others return), `everyone_is_busy_while_a_pass_to_himself_is_in_the_air`
(`ai/tests.rs`), `power_and_the_overall_come_from_the_record`
(street-game's setup, disc-gated). `ai::with_ctx` serves both test
modules. Snapshot VERSION 51 (each player's power and overall).

## The audio's generator, the commentator's duck and the speech clock (2026-10-18)

Audit item 7's audio part (section 8). Traced raw against `SLUS_206.51` and
checked on a live boot of the original: the project's PCSX2 copy, fast boot,
no input, read through PINE every frame for 15 minutes from power-on through
the title's first attract match (`%TEMP%/street-original-speech-attract-1.jsonl`,
summary `-summary.json`; the scratch tools `speech_capture.py` and
`speech_summary.py` live outside the repo), and on the three save states'
RAM.

**The commentary callback's duck.** `0x1ac9f8`, packet type 1, plays a
chosen line only when speech is not paused (+0x5c), has a volume (+0x4c)
and the commentator is quiet (+0x14) or the override (+0x68) is set. It
then, in order:

- unless the profile's front-end mode (`FUN_001e0bc0()+0x78`) is 1 (Street
  School), starts AEMS loop 0x24 (`FUN_0019ee90(aems, 0, 0x24)`). Mode 0
  stores the message `[0x24, DAT_003e99c8, DAT_003e99e8, DAT_003e9a08]` at
  `0x4baf6c` and keeps the new handle without releasing a running one. The
  three words are `audio_aems_sfx_ducking` 38, `audio_aems_crowd_ducking` 40
  and `audio_aems_plyr_spch_ducking` 25 (objects `0x3e99b0`, `0x3e99d0`,
  `0x3e99f0`, registered at `0x1a25d4..0x1a265c`). In `GAME_EVT.AEM` event
  0x24 has three parameters: the first drives the crowd banks' mixer
  (module 62793 of `CRD_*.ABK`, node 0x9f551) and every court's ambience
  (module 14922, node 0xbf116), the second the mixer's 0x9f552, the third
  its 0xaf535; its start and release pulse those modules' own nodes
  (0x9f54f/0xbf117, 0x9f550/0xbf118). It owns no module, so a handle holds
  nothing of its own;
- ducks the music (`FUN_001abde8` on the path manager): the volume
  `((+0x1784 * DAT_003ea214) / 100 * (100 - DAT_003e9a28)) / 100`, where
  +0x1784 is the music option times 10 (`FUN_001aba18`), `DAT_003ea214` is
  `audio_internal_game_mus_vol` 98 (`0x3e9870`, value at `0x1a242c`) and
  `DAT_003e9a28` is `audio_path_music_ducking` 35 (`0x3e9a10`, value at
  `0x1a2698`): 98 becomes 63 at the default level. `FUN_001aefe8(track,
  volume, 100)` fades the main track's stream there in 100 ms (ten 10 ms
  steps, `FUN_00352298` -> `FUN_00352888`), and `FUN_00346390(-1, volume)`
  sets the PathFinder's master volume (`DAT_0041719a`);
- sets +0x14 and streams the line in the manager's one stream
  (`FUN_00351cc0(+0xc, 0xfa, "speech/spchdats.big", offset)`), so a line
  started over another (the override) replaces it.

`0x1acf58`, each frame after the two services, finds the stream run out
(`FUN_00351f30` reports nothing left) while +0x14 is set and releases loop
0x24 with its stored message (`FUN_0019ee90(aems, 1, 0x24)`), brings the
music back (`FUN_001abe78`: the full volume, 100 ms, unless +0x1710, the
pause's mute for `audio_no_pause_menu_music`, which the retail disc leaves
0) and clears +0x14. A song opening sets the path's volume to full at once
(`FUN_001aa8d8`). The pause (`FUN_001ad958(manager, 1, 1)`) fades the stream
to nothing in 30 ms and lets it run; the resume (`FUN_001ad9d0(manager, 1,
1)`) stops it (`FUN_00351ce0`), so the next service frees the commentator.

The live boot read it line for line: 68 lines in the attract match, every
start with a new loop 0x24 handle, the message 38/40/25 and the path volume
98 -> 63, every end back to 98; a line ending under the speech pause still
restored the music at its natural end.

**The speech counter.** The speech manager's constructor (`0x1ac5e0`)
starts its counter (+4) at `rand() % 255`, and `rand` is the C library's
(`FUN_003b42a0`): one word at `*0x434284 + 0x58`, stepped `x * 1103515245 +
12345` (the EE's three-operand `mult` and `addiu 0x3039` at `0x3b42bc`),
returning the low 31 bits; `srand` (`FUN_003b4290`) stores the seed. The
boot (`FUN_002f4fb8`) seeds it with the clock (`FUN_002f5578`, the same seed
as the shared generator), then the audio controller's constructor
(`FUN_0019f810`) draws the loading loop `rand() % 7` (controller +0x24,
`music/loadloop`, `FUN_001a0e18`) and the in-game song `rand() % 13 + 1`.
`AuController::Init` (`FUN_0019f910`) ends by building the path manager
(`0x1aa1e8`: `srand(FUN_003998c0())`, the timer's count), the speech
manager (`rand() % 255`) and the ambience manager (`0x1a88e8`: `srand` of
the count again), in that order with nothing between. The front end's first
initialisation (`FUN_0019fbb8`, songs not yet registered) runs `0x1ac6f0`,
which seeds the middleware generator with the counter (`FUN_00362f18` ->
`FUN_00366f40`) and counts it up, then shuts the manager down (`0x1ac988`);
each match's audio start (`FUN_001a0a38`) runs `0x1ac6f0` again.

The live boot: the word 1 at timer 25, `srand(79,638)` at 575, the two
draws (2 and 6) leaving 0x176aea84 at 625, then 632 with the counter 18 at
632: `srand(632)`, `rand() % 255 = 18`, `srand(632)`. The counter read 19 at
the title and 20 in the attract match, which never moved it again. Every
save state's middleware generator walks back exactly to a seed of 18 (64,
1,571 and 1,307 draws later in two boots), and each state's C library word
walks back to its boot's team reseed (18,061 and 20,445, the shared
generator's), so `FUN_001dbe30` reseeds both. The counts whose first draw
gives 18 come every 146 hundredths (48, 194, 340, 486, 632, ...); PCSX2's
fast boot reaches `AuController::Init` at 632.

The rewrite (`street-game/src/session.rs`): `Session` keeps the C library's
generator (`Libc`) beside the shared one, seeded with the boot's seed and by
every reseed; `Session::audio_controller` draws the loading loop and the
song in the boot's order (`audio::load_front_end`; the rewrite plays no
loading loops), `Session::speech_counter` the counter after
`srand(timer)`, the timer being the hundredths since the session began, or
632 on a pinned session (`PINNED_AUDIO_INIT_COUNT`). The front end's first
initialisation runs at the load and shuts the manager down
(`SpeechManager::shut_down`; requests need it up).

**The queue's clock is the 100 Hz timer.** `0x1ac6f0` hands the middleware
its callbacks (`FUN_00362f00`'s block: the printer, then `FUN_003b3818`,
then `FUN_003998c0`), and `FUN_00367128` calls the third: the timer's
hundredths, not VBlanks. The roots' hold of 200 is two seconds. The capture
read every one of 243 queued requests stamped with the timer itself (ages
-1..1 at the first sample, the sampling's slack). The rewrite counts
hundredths of its fixed ticks in every state (`speech_clock`,
`SpeechManager::hundredths`).

**The players' queue is served first.** `0x1ac6f0` registers the players'
events first (`FUN_001adac8(.., 0, "speech/players.evt", ..)`), then the
commentary's (1); a queued request keeps that number at slot +0x6b and
`FUN_00364620(n)` serves by it, `0x1acf58` calling it with 0 then 1. The
capture read players' requests at 0 and the commentary's at 1, the per-
manager runs (`DAT_00422b28 + 8 * n`) with the players' key 0x01002000 at
entry 0, and, in frames that served both, the players' run moving first.
The post's key byte is the other way round (`FUN_00364600`: the players'
`0x1ad068` posts 1, `0x1ad178` 0): it names the event table's own id. The
rewrite's `speech::PLAYERS` is 0 and `speech::COMMENTARY` 1, and
`speech_service` serves the players first; both choosers draw from the one
generator, so the draws each takes in such a frame follow the original's.

**The pause, re-read.** The rewrite's `pause_audio` ran on every write of
`Paused`, which `game.rs` makes every frame: it resumed speech and cleared
its queue every frame, undoing a replay's pause (slot 58). It now acts when
the pause menu opens or closes.

**Correction: slot 53 is the game's reset, not "gamebreaker over".** The
message `0x3c67c0` (dispatcher `0x105938`, listener +0x1a8) is posted at
the end of `FUN_0013d388`, a virtual method in six game classes (vtables at
`0x43b4cc`, `0x43dd34`, `0x43ea0c`, `0x44027c`, `0x440d24`, `0x4416bc`)
that rebuilds the combo manager, the meters and the statistics. Its handler
`0x1a6fa8` releases loop 0x21, resets the listener (`FUN_001a30a8`) and the
speech manager (`FUN_001ada80`: a service, +0x14 cleared, the partitions run
again, the override set, the counter counted up). No attract match posted
it (the counter stayed 20 through the capture's). Not ported: the rewrite
releases loop 0x21 on a gamebreaker's basket, its own reading.

**A first comparison of the commentary.** The rewrite's demo match
(`--demo`, the pinned session, 32,147 updates; `--events` now logs each
line with its event and clock) against the attract match (34,084 updates):

| | Original attract | Rewrite demo |
|---|---|---|
| lines | 68 (2.00 per 1,000 updates) | 51 (1.59) |
| length (hundredths) | mean 247, 90..667 | mean 240, 67..943 |
| Move (4) | 15 | 22 |
| Dunk (1) | 10 | 5 |
| Steal (8) | 9 | 3 |
| Shot (0xc) | 9 | 4 |
| Miss (0xe) | 8 | 5 |
| Block (0) | 3 | 4 |
| others | hype 3, court 1, score 1, turnover 2, 7 unread | court 1, hype 1, winning basket 1, slam 5 |
| the duck | 38/40/25, 98 -> 63 on every line | the same message and share by construction |

The matches are different games (teams, plays, the computer's choices), so
the counts compare rates, not lines; the lengths agree to a tenth of a
second. No PCM was recorded: the machine has no audio capture library,
loopback capture would take in whatever else plays, and PCSX2's own video
capture needs a hotkey or menu the background input cannot be trusted to
reach.

**Still the rewrite's own.** The match music's base share (`MATCH_MUSIC`
0.55 of the option's square-law gain, where the original sets the
PathFinder's volume to 98% of the option; the PathFinder's amplitude curve
is not traced), so the duck is applied as the ratio 63/98 of whatever the
music plays at, one factor for the stream's fade and the path's master
volume; the C library's other draws (the rosters' picks in `FUN_001dbe30`,
the front end's, the ambience's five in `FUN_001a8de8`, the in-match
callbacks', the song rotation's `rand() % count + 1`, the 21-second coin)
stay on the rewrite's own generators; the counter's timer on a clocked
session is the rewrite's own startup time.

Tests: `session::tests::the_audio_draws_its_first_values_from_the_c_library_in_the_boots_order`,
`audio::rotation_tests::{the_music_ducks_to_the_originals_share_over_a_tenth_of_a_second,
the_speech_clock_counts_the_timers_hundredths,
disc_a_commentary_line_ducks_the_music_until_it_ends,
disc_the_players_are_served_before_the_commentary,
disc_the_speech_counter_starts_from_the_boots_c_library,
speech_follows_the_pause_menu_not_the_frames,
disc_a_busy_commentator_drops_the_line_after_it_is_chosen}` (a line over a
line replaces it), `sfx::tests::the_commentators_duck_steers_the_crowd_mixer_with_its_percents`.

## The wardrobe, Edit Baller and the original's own save (2026-10-18)

Audit item 7's modes part (section 9), traced raw.

**The wardrobe.** A career keeps what its player has bought to wear at
+0x670 (outfit rows, bit = the row in the build's outfit list), +0x674
(rows by colour, bit = row * 3 + colour) and +0x678 (shoe rows), serialized
at +0x658, +0x65c and +0x660 (`FUN_001e7680` reads 0x670/0x674 at
`0x1e76e8`/`0x1e7728`, `FUN_001e7758` 0x678 at `0x1e779c`, `FUN_001e7940`
sets it, `FUN_001e7820` writes the outfits). The editor object's +8
(`FUN_001e1e10`'s third argument) is set only by the edit screen
(`FUN_00273dc8`); creating a player builds it with 0.

- Shoes (`FUN_001e4e30`, `FUN_001e5058`): a free row (`FUN_001e4d38`) or an
  owned one costs nothing, else the row's price; buying takes the price and
  owns the row, so it never costs again and nothing is refunded. No mode.
- Outfits (`FUN_001e4580`, `FUN_001e4af8`): creating, nothing when the row
  is owned in that colour, 50 (`addiu s3, zero, 0x32` at `0x1e4618`) for a
  free row, else the row's price, the bits added to; editing, the row's
  price less the worn row's (a negative cost refunds through
  `FUN_001e70a8(career, 3, -cost)`), the bits replaced by the new row and
  colour, with row 0 colour 0 kept when the new row is another. Rows past
  the nine kits (codes 0x36 and up: the cards' jerseys and three special
  outfits, `FUN_001e4720`) are free.
- A new career starts with 1,500 points (`FUN_001e6ac8`:
  `FUN_001e70a8(career, 3, 0x5dc)`), and the creation (`FUN_001e6518`)
  charges the preset's build, size, a row of create table 2
  (`FUN_001e2fa0`) and skill bars, and buys its outfit in creation's
  terms.

The rewrite (`street_sim::cab::Wardrobe`, `cab::shoe_cost`,
`cab::outfit_cost`, `cab::buy_outfit`; `ballers.rs`): each created baller
keeps a wardrobe; Save Baller charges the outfit and shoes worn by those
rules (a new baller in creation's, a saved one in the edit screen's, against
`Editing::worn`) and then owns them. The build and size keep the rewrite's
difference against what was paid. The save gains a `WARD` section, written
last (an older build stops reading there and keeps everything before);
older saves own what each baller wears, which the old editor had charged
for. An original career's wardrobe imports from +0x658..+0x660.

**The Gym.** The original has none: the map's second prompt is "Edit
Baller" (layout row 34), which opens the career's player in the edit
screen. The rewrite's Gym page, `gym_buy`, `rating_cost` and
`Story::upgrade` are gone; the map's Edit Baller opens the career star's
editor (`menu::open_baller`). `Story::upgrades` stays: bumps an older save
bought keep applying; nothing adds to them now.

**The results notices.** `FUN_001de528(game data, kind, id)` fills slot
+0x1c0c + 4 * kind only while it is -1, so a game shows one id a kind. The
pages read the slots in this order (the first `lw`/`lh` of each offset):
Be A Legend's `FUN_00262ba8` kinds 2 (court), 1 (team), 4..6 (jerseys), 3
(the level boss), 7..9 (boss moves), 10 (a completed game); Pick Up's
`FUN_002c5cf8` 2, 1, 10, 4..6, 7..9, 0 (Reward Points). `boards.rs` already
drew them in that order; it now shows the first court, team and Reward
Points of a game only (`LEGEND_KINDS`, `PICKUP_KINDS`).

**The original's own save, out of its RAM.** The game data a memory card
stores lives in one object at `0x4bbf10` (`FUN_001e0bc0`, built by
`FUN_001dadb0`), each section a flat block written as it is:

| Section | Object | Bytes at | Length |
|---|---|---|---|
| the game record | +0x0 (`FUN_001d3bd0(obj, obj + 0x18, 0x60)`) | +0x18 | 0x60 |
| the options | +0xfc (`FUN_001d6e90`: obj + 0x30) | +0x12c | 0x18 |
| the high scores | +0x144 (`FUN_001d3d08`: obj + 0x18) | +0x15c | 0xa64 |
| the users | +0x1c3c (`FUN_001ef028`: obj + 0x38, 0xbc x 32) | +0x1c74 | 0x1780 |
| the careers | +0x33f4 (`FUN_001eb190`: obj + 0x38, 0xc88 x 10) | +0x342c | 0x7d50 |
| the teams | +0xb57c (`FUN_001edd40`: obj + 0x38, 0x70 x 10) | +0xb5b4 | 0x460 |

`tools/save_state_sections.py` cuts them out of save states into the
decoded game file (`street_assets::save::Ps2Save`). The three states (two
boots, with no memory card) import: the played boot holds User Setup's
"PLAYER1", every state the original's high scores, options and cards, no
career. Through the import and the native save:

- A fresh profile's high scores are the original's own defaults:
  `FUN_001d3d98(scores, 0)` copies 95 rows from `0x45e2d0` (10 tables: 5
  rows, then nine of 10, `DAT_0045e2a8`) and `FUN_001d43a0` sorts each with
  the C library's `qsort` (`FUN_003b3868`, BSD's), tables 0 and 1 by
  `trunc(1000 * a / (a + b))` of +0x14 and +0x18 (0x1d47b8), the rest by
  +0x14 (0x1d47a8), larger first. The qsort is not stable: the defaults'
  ties (Bird and Dime at 3, four at 2) come out in its own order, which the
  save states hold byte for byte. The rewrite's fresh save had empty
  tables; it now starts with these (`original_save::default_scores`,
  `default_records`, `c_qsort`, from the disc's executable in `main.rs`).
- The reset grants every card that costs nothing (`FUN_001db0d8`:
  `FUN_001e0420` for each zero-price row), which the rewrite's
  `rewards::card_owned` already counts as owned: the two agree card for
  card.
- The options' reset leaves the Rucker Classic's song off, not Nelly's:
  `DAT_003e9ca8` is `audio_rucker_classic_track`, registered with 20 (the
  store at `0x1a2a2c` goes through a register, which the xrefs behind "The
  original's default options" missed), so `FUN_001d7398(track, track !=
  20)` clears Trax bit 23, track 20, in-game song 12, which the Rucker
  Classic plays on its own court. Every state's Trax word is 0xff7fffff.
  `records::ORIGINAL_DEFAULT_BLOCK` and `Settings::default()` follow it
  (`RUCKER_CLASSIC_SONG_BIT`; Nelly's song is on). The attract boot's
  difficulty reads 2 (its attract match's own, not traced).
- G01 (overflow careers aliasing the six baller records) was fixed on
  2026-10-14 (`STAR_SLOTS`, `all_ten_original_careers_import_and_restore_playable_state`).

**Still the rewrite's own.** The starting 1,500 points and the creation's
charges (the rewrite's ballers are made outside careers, each with its own
balance), the special outfit rows, the build and size pricing, the high
scores' insertion order (the original re-sorts with `qsort` after each
entry; the rewrite inserts by its own rule), table 1's rows (5..14, not
imported), the Rucker Classic's song being switched on by `FUN_001e0420`
and `FUN_002f2d58`; no captured boot holds a career, so the round trip has
not seen one.

Tests: `ballers::tests::the_wardrobe_charges_and_keeps_what_is_bought`,
`builds_and_sizes_cost_development_points`,
`story::tests::the_wardrobe_saves_and_older_saves_own_what_is_worn`,
`original_save::tests::{an_original_careers_wardrobe_imports_from_the_executables_words,
the_c_librarys_qsort_sorts_and_keeps_its_own_tie_order,
the_fresh_high_scores_are_the_executables_tables_sorted_as_the_game_sorts,
the_originals_own_save_round_trips_through_the_native_one}` (the last two
with STREET_ORIGINAL_SECTIONS, the decoded saves joined by ';'),
`boards::tests::the_results_pages_read_the_notice_kinds_in_the_executables_order`.

## The presentation's rest: the referee's run-off, the camera's row and source, the replays' kind and roll, the game's end (2026-10-18)

Audit item 7's presentation part (docs/AUDIT_1TO1_2026-10-16.md section
7). Traced raw from SLUS_206.51 (file offset = address - 0xff000) and
checked against the retained captures; PCSX2 was in use by another session
throughout, so no new grabs were taken.

**The referee's run-off** (`crate::referee`). His plan (`0x1847c0`, game
phase 1) queues "referee wait" (`FUN_0011ad68`) and then a "goto"
(`FUN_0010f278(task, ai, (0, 0, -51.5), 1, 3, 0, fps * 30)`, raw
`0x184820..0x18488c`; its update `FUN_0010f3b8` calls `FUN_0013aed8`,
which turns the place into the stick word through `FUN_0013ac80`:
`(int)(atan2(x - 0, z + 51.5) * 162.97466 + 0.5) & 0x3ff`, the same as
`ordinary::native_heading`, at speed level 3, `DAT_003d17d8[3]` = 64; it
ends within 0.75 of the spot or after 1,800 updates, `FUN_0010f318`,
and `FUN_0013ac30` lets the stick go). The goto runs from the launch,
before his own update, on where he stands as the update begins, and
`FUN_0013af58` (speed level 3 and a target given) keeps the stick's last
word while the new one is under 6 units from it (`FUN_0018cd78`), so
capture 3 runs on 1022 through update 262 (his place first reads 1021 at
199), on 1016 from 263 and on 1009 from 272, and stops at 275, 0.725
from the spot (the steer's vtable `+0x34` and `+0x3c` calls, made while
he is 2 or more from it, return the word unchanged in the captures; they
are not traced). With no script lock
`FUN_0017cdd0` sets `+0xb0` to the word and `+0x40` to a third of a unit
along it (`-1/3 * (sin w, cos w)` from the cosine table `0x440ea0`) and
starts `0x44cb61` when record 948 is not playing. Every script starts
`96 97`, and `97` zeroes `+0x40`, so the update 948's script starts he
does not move. `91`'s operand is ORed whole into channel 0's flags over
the low half kept (`ordinary.rs`'s port): `91 0x00820000` in `0x44cb78`
and `0x44cb61` sets 0x800000, so their `8e` blends match the heading
(`FUN_00196698`, argument 1: the world root's heading less the incoming
clip's at frame 0, when 23 units or more apart); the toss's
`91 0x00020002` does not. A script's 8-frame blend holds the frame from
its start's update through the seventh after (no root travel), and the
eighth advances the clip. The toss's `8c 1.5` runs from the launch's own
update to the clip's last frame (16), and the jump to `0x44cb78` follows
the update after. His update ends with vtable `+0x64`, `FUN_00154560`:
`+0xa8` turns 48 units an update toward `+0xb0` (snaps within 48, the
shorter way round) and the quaternion `+0x50` is rebuilt as `(0, sin(w/2),
0, cos(w/2))` from the table with `w/2` an integer division, so an odd
word turns him a unit short (561 shows as 560). The clips' root travel
(`FUN_0019a658`) is the sampled root less the last one turned by that
quaternion; a looped clip's wrap takes the lap back. Capture 2
(`street-native-ai-attract-2.jsonl`): the launch at update 120, 947's
script at 135 (the heading match makes his facing 276 from the toss's
world root 278), 948's at 136, the first move at 137, 948's frame 1 at
144 with the facing on 1022; capture 3's opening is the same update for
update. The rewrite now follows his place within 0.0003 at all 155
updates from the launch until the goto is over (his update goes on after
the catch while he is hidden), with the stick's re-aims on the same
updates (`referee::tests::the_referee_runs_off_like_the_captured_original`,
run with `STREET_AI_CAPTURE` on captures 2 and 3; capture 1 starts after
the opening).

**The camera's row.** `FUN_002f7bb0` has four callers (`0x303a68`,
`0x303ac4`, `0x304c44`, `0x304ca0`), all in `FUN_003038b0` and
`FUN_00304a38`, which pass 5 for the front end's mode 3 and for mode 4
with more than one user (`FUN_001db248`), else the saved type's row
(`config +0x12c >> 6 & 7`: 1, 2, 3 as themselves, 4 as 5, anything else
0); the mode word `+0x1f0` has no other writer than the constructor's 0
(`0x2f7b68`). Behind (row 4) is therefore never on screen, and Pause's
table leaves it out to match. `view::native_camera_row` picks rows so (a
saved 4 is Street), and forces Wide for the League Challenge with two users.
Behind's own update (the `+0x20c` turn, its flags and x commands) is dead
code in the original and stays unported.

**The source adapter's inputs** (`view::CameraSourceSample`). An actor's
source vector is now his physical vector as the simulation moves him
(`World::actor_motion`: his flight on a shot's, dunk's, lay-up's, leap's
or dive's script, none on a trick's or a reaction's whose `97` zeroes it
while the clip's root carries him, else his locomotion's speed along his
course). The busy test is the actor's state bits inside `0x7ffdc` as the
simulation's actions show them (`World::actor_state_bits`, the bits the
computer players read: 3, 4, 7, 8, 9, 10, 12, 13, 15, 16; 2, 6, 11, 14,
17 and 18 are not modelled). `FUN_00300b98` (raw `0x300b98..0x300bf0`)
is ported: channel 0's record `+0x50` in 46, 47, 174, 175, 176 or 177
(`RunToStandDribble_L`, `jogdribblestop02_r`, `jogdribblestop_r`) keeps
the stationary gate off; the simulation's gait row stands for the
record. The vertical gate is ported (raw `0x300520..0x30062c`): on the
ownerless path, a ball leaving its owner as a shot (state 5, after
`FUN_00300150`) or a pass (4, after `FUN_003000b0`) clears `+0x158` and
`+0x1b4`; within 0.25 of its resting height (`ball+0x11c`) a source
rise turning from below zero to not sets `+0x158`, only in the row's copy
(`FUN_003003f8`'s second argument 0); `+0x1b4` keeps the last rise; while
`+0x158` is set and the ball has no owner the focus's height reads 0. A
knock-loose keeps the gate. Tests:
`a_loose_balls_bounce_flattens_the_rows_focus_until_a_shot_or_pass`,
`a_stopping_clip_keeps_the_stationary_gate_off_like_fun_00300b98`.

**The replays' kind and the director's roll.** The launcher's dunk path
(raw `0x14ad14..0x14ad3c`) writes the ball's kind `+0x184`: 4 for a
dunker (bit 7, `FUN_0016e9d0`), else 4 when his move has 0x800
(`FUN_00171de8`: `0x43f5f0[actor+0x1e8]`), else 3. `Basket::AlleyOop`
carries `advanced` (the finisher's move's 0x800 at the launch,
`World::advanced_move`), and the statistics and the director read an
advanced finish as kind 4 (snapshot VERSION 52). The director's combo
branch reads the combo object's payout word `+0x150` alone (no side or
row test: a stale word pays) and `FUN_00131d18`'s start, `+0x14c` while
`+0x148` holds a row, else -1. Its roll is `FUN_003a16b8` at `0x324bac`,
the gameplay generator, inside the game's update: with points, unless the
game is won (vt `+0xbc`) and unless Manual offers the replay (no
gamebreaker ball and no full meter), it draws once. The simulation now
makes that draw itself after the basket's `Event::Score`
(`Event::ReplayRoll`), from `World::replays` (the live option, set by the
game each update; online peers keep Automatic), and `tape::Director` reads
it (`a_basket_draws_the_directors_roll_from_the_shared_generator`).

**The game's end.** Game vtable `+0x17c` is `FUN_0013f0b0`: the referee,
the ball and the six actors hidden (each object's vtable `+0x3c` with 0)
and `FUN_00314298` hiding the people's markers. The phase machine's case 9
(`FUN_0013e4c8`, once no replay is asked for, delayed or playing and no
script runs) and the replay's end in phase 9 (`FUN_00142648`) call it, so
the boards and the post-game dialog stand over an empty court
(`street-ingame-ui-original/post_3.png`). `view::game_ended` hides the
players (and so their shadows), the ball, the rings and the badges.

**Disabled pause rows.** Vtable `+0x114` (`FUN_002491d8`) disables a
widget and its children; the text widget's disable hook (vtable `+0x8c`,
`FUN_00239990`) sets the brightness byte `+0x5a` to 0x7f when `+0x5b`
bit 4 is set (the widget constructor `FUN_00248a90` sets it), and the text
draw (`FUN_00238d20`) scales red, green and blue by `+0x5a / 255`. A
disabled row's label and value now draw at half brightness
(`pause::DISABLED_BRIGHTNESS`).

**The crowd.** The plan's colour word's alpha byte, halved to 0x7f,
modulates the texel's alpha as the colour does (`texel * colour >> 7`):
`crowd::tint_colour` carries it. No court's plan names head kind 3 (`P`)
although every court has the model; the loader (`0x30b9b8`) loads a kind
only when seats name it, so `P` heads are never drawn in the original
either (`crowd_disc.rs` now asserts it).

**Left, with what is missing.**
- The pause panel's frame against the capture: its edges sit within 1.5
  pixels at the top and the sides and 3.5 low at the bottom, while the
  rows' text drifts from 0 to 2 pixels low down the panel; the panel's
  geometry is `FUN_00211760`'s, already ported, so the bottom's
  remainder looks like the capture's mapping (about 1% vertical scale),
  not the drawing. The demo's Start line was not re-measured (the
  retained `attract_*.png` grabs show another window).
- The controller panel (`FUN_002144c8`) is one widget for the pause page
  (at (70, 105), `FUN_002bc3f8`) and the front end's Controller Layout
  page (at (70, 90), `FUN_0029c578`; also `FUN_00273148`), told apart by
  its last argument (0 pause, 1 front end) and the setters called after
  it (`FUN_00214b10`, `FUN_00214b20`, `FUN_00215610`, `FUN_00214b28` on
  the front end; `FUN_00215658`, `FUN_00214b20`, `FUN_00215610`,
  `FUN_00216248`, `FUN_00214b28` on the pause). Its build `FUN_00217398`
  puts "Player %d" (0x4008), the layout (0x4009..0x400b) and the view
  (0x400c, 0x400d, 0x4000) in font 2 centred on (x + 120, y + 5), 25
  apart, the side arrows at the text's edges less 34 and plus 18, then
  font 1 notes (0x4025..0x4027, 0x401c, 0x4022) at (460, 310) and nearby
  with the legend icons (`FUN_002126d0`, actions 0x13..0x15), the rest
  past a `FUN_0030fa50` cut (raw `0x217a50..0x218074`). Its rows
  (`FUN_002164a0`) and icons (`FUN_00216dd8`) put Move Player at y 25 of
  the widget, then Cross, Circle, Triangle, Square at 60, 85, 115, 145, R1
  R2 at 168, L1 L2 at 193, R3 Strategy at 225 and Pause at 258, the face
  rows' words by the layout (A and C: Pass, Shoot, N/A, Trick Move; B:
  Shoot, Pass, Trick Move, N/A). The front end's page (`diagram.rs`) had
  the turbo rows and Strategy and Pause swapped and Triangle as Block /
  Rebound. Both pages now draw the rows with one function
  (`pause::controller_rows`, at the widget's place): the icons
  middle-anchored with their left edges at x 259 (the stick 262, R3 265,
  R2 and L2 294) and their middles at y 25, 60, 85, 115, 145, 170, 195,
  225 and 255 (the arrow, frame 0x11), the words as above
  (`diagram::offence_face` on offence). Against `orig_pause_layout.png`
  the R2 and L2 icons now fall within 2 pixels (the capture-placed rows
  had them 3 to 6 right), the face icons 1 to 2 left and the turbo icons
  2 low, from the rewrite's own icon rectangles (not `FUN_00212748`'s
  frame table). Still placed as before: the pages' frames, the pad's
  picture and the band (the pause's by its capture), and the build's rest
  past the cut (the frame 0x12 icon at (344, 210), cases 10..15).
  The song list (`FUN_00218e68`, pause at (10, 10) and the front end's
  `0x2658e4`) likewise.
- The rematch rules dialog (`FUN_0025dc88`, 0x5df; 0x1660 bytes of
  code): Play To (0x570c, the config's goal `+0xf0`, `+0xf4`/`+0xf8`
  the points or trick-point targets; their steps and bounds lie past
  the decompile's cut) and Spot Points (0x570d, per side, `FUN_001d5f58` with 0x570e/0x570f) over
  Continue (0x5709), line widgets (`FUN_0021e3f0`) at (240, -110) ..
  (468, 155) and a 0xff424a9c divider; the original's look is
  `street-ingame-ui-original/post_4.png`. The rewrite still plays again
  at once on REMATCH.
- The Level 2 banner's element 9 (`FUN_002a8538`): twelve 80x73 sprites
  from `/FE/REPLAY.SSH`'s (429, 176)..(509, 254) placed by a static table
  at `0x4d4a10` (BSS filled near `0x2bbcd4`), a clock of fps / 3 (`+0x1c4`),
  update `FUN_002a8c20`, draw `FUN_002a8770`.
- The crowd's and the camera's draws on the shared generators: the
  crowd's reaction picks (`FUN_0030ca10`, `FUN_003a16b8`) run inside the
  game's update from the audio listener's crowd calls, so the simulation
  would have to make them with the court's group count; its node picks and
  load stagger draw `FUN_003b42a0`, the libc generator the camera's coins
  share; the courtside placements (`FUN_00303c80`, `FUN_003041f0`) draw
  `FUN_003a16b8` in the Camera task after the update. All still draw from
  the rewrite's own generators.
- The positioned controller's VU operation order (the projection and the
  lead) and played-match evidence for the gamebreaker and celebration
  cameras.

## The person's presses: the pass target, the passout specials and the shoot press (2026-10-18)

The second audit's fix order item 1 (docs/AUDIT_1TO1_2026-10-18.md
sections 1.1 and 1.2). Branch `presses` (off e94c663). Traced raw from
SLUS_206.51 (file offset = address - 0xff000; Ghidra's recovery for the
stretches past `FUN_0030fa50`'s cuts), checked against the twelve played
captures and the three attract captures, and a new played capture taken
for this pass (`street-played-passes-1.jsonl`, below).

**The pass target** (`FUN_00146278(pad)`, raw `0x146278..0x1476bc`;
`World::pass_press`, `World::lined_up`). The pad builder `FUN_001587d8`
calls it in the passer's update and writes the answer to actor+0x74; the
holder's update `FUN_0015a060` throws to it (`0x15b864..0x15ba14`). A
computer's pad names its receiver (`FUN_00146228`, pad[9]). For a person
the direction is the right stick's angle when its press is in (0x400,
pad[5]); else the D-pad's (the table at `0x4b7e50` by the low four bits,
intensity 64); else the left stick's when pushed 16 or more of 64
(pad[3], pad[4]); else the facing word (actor+0xb0). The active camera's
yaw is then added to it whichever it was (`0x146510`: the helper at
`DAT_00402020`, its +0x94 in radians, `trunc(yaw * 162.97 + 0.5)` units).
The rewrite's sticks are the court's already (the client turns them by
its camera), so the yaw reaches the simulation as the person's
`Input::view`, the court word of the screen's up (`Input::set_view`,
`controls::read`; 8 bytes a pad, `street-net` protocol 11), and only the
pass at rest adds it to the facing word. Each teammate's arrival is the pass's flight over his distance (the
three-dimensional one) plus the windup, `trunc(fps * 0.3167)` = 19 for a
passer in a dunk, a leap or the air, else `trunc(fps * 0.1834)` = 11. A
teammate leaping for an alley-oop (bit 15) in the air is the target,
unless his meet (actor+0x31c) less that arrival is more than 6 short
while he falls below 6.5 (`0x14674c..0x14683c`); of two leapers, the most
risen of those on time, else the latest meet (`0x146844..0x146934`).
Otherwise the plain pick (`0x14743c..0x14766c`) scores each teammate by
the angle between the direction and the line to him (game+0x2cc's table),
less 9 if he posts up (bit 11) or his lane is open within 14 of the rim
(`FUN_0017f4d0(ai, 0)`, `ai::lane_open`), 9 if his defender is more than
15 off him or down, 11 if his nearest opponent is more than 15 off, 9 if
he is his side's nearest the rim (the second nearest when the passer is):
the least score under 228 wins (the first of equals), else the least
score plus his distance (game+0x1ac).

**The passout specials** (`World::passout`). The cvars `FUN_00147cf8`
registers decide who may: `debug_passoutOneTurbo` is 0, so the press
needs the held word's two-turbo bit (0x40000), and
`debug_passoutOnGround` is 1, so the passer stands on the floor; the
right stick's press never makes one. Short of 0.2 of his bar the press
throws nothing ("no turbo", `0x3c6520`). With the stick at rest (under
16) it is Off the Heezay: his defender (`FUN_0016e8f8`) within 12, the
ball off his head down to a leaper or the passer; beyond 12, no pass
(`FUN_00171e30(actor, 0)`). Pushed, it is Back 2 Papa: the stick within
128 units of the line to the rim, the passer in the half his side
attacks, within 35 of the middle and inside the board's wedge
(`(|z| - 3) tan 20 <= 38 - |x|`, the EE's own sine and cosine of
`0x3eb2b8c3`); the ball goes off the board to a leaper, or from within 8
across and 29 along to the passer, else to the nearest teammate across
the lane inside the same wedge (none: no pass). Its point on the board is
`38 - r` along and `15 - r` up (`r` the ball's radius), then `rand() %
80` under the passer's Handles puts it across where his line to the
receiver meets the board within 3, else anywhere across the board and
lower, where the pass can miss (School's "out of position"). Then 0.2 of
the bar (`FUN_00190c08`) and the passout state (`FUN_00142068`,
game+0x140..+0x160). `FUN_00171e30` is the failed passout: kind 0 plays
`passout_boomerang_1_fail` or `_2_fail` (`rand() % 100` under 50), kind 1
turns him to the rim for `passout_2_fail`; the rewrite draws and turns
but does not play the fails' scripts. The passout legs' flight
(`FUN_00148378`, the ball tested against the head bone) stays the
rewrite's; the old `HEEZAY_RANGE`, `GLASS_NEAR` and `GLASS_FAR` are gone.

**The right stick and the give 'n go.** `FUN_001479f8` turns pad[6]'s raw
bytes into `trunc((|stick| + 0.5) * 64 / 127)` and at 60 or more sets the
held word's 0x400 (`0x147994`); `FUN_0015a060` takes 0x400 pressed as a
pass press of its own (its `0x500` test), aimed along the right stick
(`passing::aim_pressed`, `World::buttons`). The passer keeping control
and calling for the ball back stay `controls.rs`'s and `call_for_ball`'s:
the person's control hand-off (38 of 43 captured hand-offs 19 updates
after the release, audit 1.4) is the fix order's item 3. (Taken up
2026-10-18, "Who the person controls, and the stick": the hand-off is
callback 38's, in the release's update, and the 19 count from the press;
the give 'n go and the call are team+0xc8's; the client's `Input::view`
was half a turn off the original's yaw.)

**The shoot press** (`FUN_0016b4b8`, raw `0x16b4b8..0x16c44c`;
`World::shoot_button`). The holder's update calls it for the held button
(`FUN_00172220`) when the ball has not just changed sides; right after a
take (game+0x94, set at `FUN_001415c0` and cleared by the shot, the pass
start `0x15fa20`, the trick press `0x15f1c8` and the actors' reset) a
person needs a new press (`World::fresh_take`, `World::shoots`). So
shoot held through a move or a catch shoots once he is free: the pads
capture's rebound at update 2690, pressed in the jump's tail, shot two
updates later on the held button. Two turbos make a gamebreaker
(`FUN_00159cf0`). It asks nothing of the bar: capture 3's bars step only
by the 0.2 moves and owner changes, none of its 19 dunks costs anything,
and the old `DUNK_TURBO` (0.08) and `ADVANCED_DUNK_TURBO` (0.3) are gone.
A dunk on (`FUN_0016c500(actor, 1)` 2) short of the rim's line (|x| <=
36.5), or past it from beside the rim (sectors 2..6 seen from it) within
31 units of the line (`FUN_0016b3e8`) and 2 or more out, draws the dunk
and lay-up rows (`FUN_001693b8(actor, 0)`, below), a turbo held or not;
past the line otherwise, or behind the board with none on
(`FUN_0016f580`), the shots from behind it: more than 3.5 off the middle
a side fade by the side and the live hand, else `layup_3_bboard`
(`0x446416`, lay-up kind 5). Else the jump shot; out of the dunking area
(`FUN_0016c500` 1) facing the rim the runner. The old lay-up gate
(`LAYUP_RANGE`, `LAYUP_RELEASE`) is gone.

**The dunk and lay-up rows** (`FUN_001693b8(actor, 0)`, raw
`0x1693b8..0x169c94`; `timing::press_move`, `press_candidates`). The face
reaction draws first (`FUN_00159190`). A two-turbo combo held plays its
slot's dunk (`0x43d3b0`, `DUNK_SLOTS`), the advanced `bbpass_1` (the
four-turbo slot) only from 8.5 or more out. Then the Dunks rating's gates
(`0x1696c0..0x169744`): under 21, a rating of 0 or under `rand() % 100`
keeps only the easy rows (0x80000); 21..59 drops the hard rows
(0x400000); 60 and over drops the easy ones under 51 of 100, else the
hard ones. The filter (`0x1697d8..0x169988`) passes no alley-oop finish
(0x100000), a dunk row only if its id is unlocked for the held word's
turbo bit (`FUN_00139d38(table, 0, +0x254, id)`: a created player's
slots, a legend's one dunk, else the 60-id row of `FUN_00139850`'s table
that +0x254's low half picks; for an ordinary player 8 ids without a
turbo, 8 with one, the 11 slot dunks by their combo's level, the rest
never: `timing::dunk_unlock`), then the approach sector, the hand, the
gates, the distance band (12.5, 8) and the legends' rows; a lay-up row
asks no unlock. One is drawn (`rand() % count`); none, and a jump shot is
(`rand() % 100` under 75 a hook of `0x3d82f0`). The save states' RAM
holds the same unlock table as the executable's static reading. Not
carried: created players' and legends' unlocks and rows.

**In the air** (`FUN_0015a060` raw `0x15aedc..0x15b074`,
`World::busy_buttons`). A shoot press in a dunk (bit 7) more than 1.5
from the rim and 0.5 up, or in a rebound jump (bit 8, without bits 13 and
3) in the half his side attacks more than 1 up, is `FUN_0016b1e8`
(`World::midair_layup`): within 10 of the rim one of the ten mid-air
scripts of `0x3d8f60` (`rand() % 6` within 5.25, else `rand() % 10`):
the tip under the rim, the dunks turned into lay-ups (School's "Dunk to
Lay-up"), the two-handed catch and the tipped shot (`timing::EXTRA_ROWS`
101..111, steps from the disc). His flight goes on. The old putback
(`PUTBACK_RANGE`, `PUTBACK_CLOCK`) is gone; `FUN_00169c98`'s two-turbo
putback on the rebound press is not ported.

**The pump fake.** A tap under the check ends where the shot's script
falls through callback 20's `85` to its `End` (the heave's `84` jumps):
`held::fake_end` runs `FAKE_STEPS`, giving the captured fakes' 18
(stepback), 22 (`legkick_1`) and 8 (the heave). `PUMP_TIME` is gone.

**Two new played captures** (`tools/pine_played_capture.py` schema 2,
which adds the six turbo bars and the controllers' bar slots, the
passout state game+0x140..+0x160 and the active camera helper with its
yaw). Both were played from save state 1 in the project's PCSX2 copy by a
driver posting keys to its window: `street-played-passes-1.jsonl`
(updates 259..22931: passes with the D-pad's eight directions, the left
stick, at rest, the right stick pushed home, two turbos at rest and on a
drive, L3, and drives with a shoot press with and without a turbo) and
`street-played-passes-2.jsonl` (updates 23599..47457, the match played on
to its end: passes at rest, two turbos with a defender within 6, two
turbos from the wing, drives with a held shoot press with and without a
turbo). The camera's yaw runs -0.98..2.41 radians, mostly 45 to 130 units
either way, its sign by the end of the court; +0x88 already carries it
(at rest it reads the yaw itself). They show each traced branch: two
turbos at rest with no defender within 12 play `passout_boomerang_1_fail`
or `_2_fail` and throw nothing (target 0); with one close,
`passout_overshoulder` back to the passer; two turbos at the rim from the
middle, `passout_overshoulder` to the passer, from the wing to the
teammate across the lane; plain and turbo dunks and a lay-up from the
drives; and every pass with a teammate in the air on an `allydunk` script
goes to him.

**Against the fourteen played captures**
(`played_capture::the_persons_presses_choose_the_originals_moves`): every
press of the person's with the ball in live play, the match rebuilt as
the update before left it, one `World::step` with the press's held word,
stick and view (the yaw) over 32 offsets of the generator. Shoot presses
on the floor: 90 of 90 agree (the old rules 77: on the twelve they missed
pads-2's side fade from behind the board at 7599 and the one past the
rim's line at 24542; on the new two, 11 of the 23 drives' dunks, lay-ups
and jump shots). Passes: 82 of 83 (the old rules 66): all 33 at rest
(with the view; without it 2 of them go to the other teammate), and of
the 23 whose two teammates lie within the bonuses' 38 units of each other
22 (the one left, passes-1's 2428, needs teammate 0's lane counted open:
`FUN_0017f4d0(ai, 0)` is the AI's own test, cached once a frame at
ai+0x90/+0x94 on its matchups, which the captures do not keep, and the
rebuilt match counts it closed). Counted apart: the 8 passes to a
teammate leaping for an alley-oop, all to the leaper; the 4 right-stick
presses, whose angle (pad[5]) the actor's words do not keep (each went
where the key's screen direction turned by the yaw points, the sticks'
screen up reading 512); 7 presses in the air (five rebound-jump presses
in the back court that start nothing, as the front-half rule says; the
2690 shot on the held button; a pass in the air).

**The first attract capture's Portland.** With the press ported, capture
1's comparison gave 17.9 block jumps a game at a shooter against 13: the
inferred roster's actor 5, Portland's 112 (Damon Stoudamire, Dunks 0),
took 65 dunk-window presses over the 16 seed bases, 64 laid up (`Dunks 0`
keeps the easy rows, and the pad builder `FUN_00182280` gives dunk level
0 a combo one press in five at most). The capture's own actor 5 is not
112: his 17 combo jukes are levels 2 and 3, none 4 (`FUN_0015f098` gives
112's Handles 90 level 4, a third of his combos; capture 3's recorded
players fit their Handles so), and his three dunk-window presses are
level-2 slot dunks. Save state 9 holds 110, 111, 112 but is another match
of that reseed (its actors sit elsewhere). Portland's 113 (Derek
Anderson, Handles 75, Dunks 80) fits, so `captured_rosters` takes 55, 56,
57, 110, 111, 113. On it the comparison gives 12.5 at a shooter, 26.4 at
a dunker (30), 17.8 counter presses (17) and 55.9 shots (55); the first
capture's bounds are set again by the rule (the seed sets +500 and +777
inside every one; the ran-out trick length 59.9 against 61.2, 60.6 and
60.5 on the other seed sets).

**What the pass captures show beyond the presses** (left for their own
passes; the gate's played tests run on the twelve). On the fourteen the
press comparison and twenty of the played tests pass; five that held on
the twelve meet cases the twelve never had: two catches held short of
the rule (`catch_stand_3r` at passes-1's 16694 9 updates against 16, an
interception at passes-2's 43158 4 against 16), the computer's block
jumps at a holder 19 of 24 rising within 0.02, the tricks' clock 44 of
50, the swipes 7 of 8 on time, and the person's lobs to a leaper 9 of 16
flights exact (the rest one update either way; the computer's four still
exact). The release test leaves the trick rows (flags 0x200000 and
0x400000: the passout legs, the lob) and a press pressed again before
the ball leaves; the reseed test counts the pass captures with save
state 1's boot. `played_capture::load` now parses the captures once for
every test (a copy a test overran the memory with the two new ones). All five are taken up in the next section, "The pass captures' five
checks".

## The pass captures' five checks (2026-10-18)

Branch `played-passes` (off 2bfe602). On the two pass captures
(`street-played-passes-1.jsonl`, `-2`, previous section) five played
tests met cases the twelve never had. Each was traced in the raw
executable and the captures: two were the port's or its harness's and
are fixed, three were checks the twelve never stressed and are corrected,
each still pinning what it pinned. The gate's played run now takes all
fourteen captures; on the twelve the tests pass as before.

**Save state 1's players.** Every played capture plays save state 1's
match or a later game of that boot, with the same six records: the
disc's players 55, 56 and 100 against 82, 95 and 116.
`tools/pine_played_capture.py` read a record's fields (u16s at
`+0x18 + 2k`, `FUN_001da4e0`) only when their table started on a word.
Actors 0's and 4's start 2 past one, so no capture carried their ids or
ratings, and the harness posed them on a plain 1.2 body at ratings 50.
They are not created players, as earlier notes said: the state's RAM
names them (each record's +0x1c and field 0), 55 with Blocks 81 and 95
with Blocks 60, the disc's own ratings. `played_capture::resolve_records`
now takes the two from the disc's records for that match, which it
recognises by the four players the captures do name. The tool reads the
fields at any even address. With their bodies, 98% of dunks' updates sit
within 0.1 of the original's height (85% before) and 99.8% of alley-oop
dunks' (83%). The held ball's release points are within 0.1 for 63 of
69 shots (44 before), with a median against the holder of 0.018 (0.67).
The runners' offset scales with the body: it forms in the takeoff
blend's root (labelled; the held clip's root, actor+0xe4, is not
captured) and then holds. Two of player 95's six runs now sit 0.103 off
(0.097 and 0.095 on the plain body), so their share within 0.1 (54%) is
no longer pinned. In its place `the_jumps_follow_their_scripts_in_play`
pins the rise after the blend for every kind with a flight: within 0.01
in 97% to 100% of the updates.

**1. Two catches held short.** At passes-1's 16694 a `catch_stand_3r`
held bit 9 for 9 updates against the rule's 16: the shot clock ran out 9
updates in (phase 6, the holder on `0x44cc1a`, the ball dropped). At
passes-2's 43158 one held 4 against 16: the computer receiver shot 4
updates in (`jumpshot_2hand_legkick_1`). A script that replaces the
catch row ends it, because its `9b` replaces the state word (handler
`0x1958b0`). So bit 9 goes in the update the channel leaves the row,
exactly as both captures show. The port held bit 9 (`actor_state_bits`,
the AI's `catching`) to the hold's end even through a shot or a pass out
of the catch. `World::catch_holds` now ends it with any move of the
receiver's (`ai::tests::a_move_out_of_the_catch_ends_the_catch_rows_hold`).
The check takes the row's own end when another script replaces it. Over
the fourteen captures: 227 catches checked (88 the person's), none off,
and both cut ones with bit 9 going with the row.

**2. Block jumps at a holder, 19 of 24.** Four of the five misses were
actor 0 rising like Blocks 81 against the harness's 50. With save state
1's players, 23 of 24 rise within 0.02. The one left (passes-2's 33308)
rises 0.19, under the lowest rise callback 35 allows (`sqrt(4g)`,
0.21), so something other than the aim set it; it is reported. At a ball in the air, 53 of 56 rise within 0.02 (46 of 50 on
the twelve).

**3. The tricks' clock, 44 of 50.** The misses were two kinds the twelve
had held once each:
- **A move replacing the trick.** The twelve's "countered" trick was a
  swipe's strip at its 6th update. Passes-2 adds a pass out of a trick at
  its 17th update and two counter-stance steals, at the 10th and 13th.
- **A sample read late.** The PINE batch came 20 ms after the last, so
  the channel showed a step ahead, and the next sample, 13 ms later,
  showed the same frame. The twelve had one, at the 24th update;
  passes-2 adds two.

The check now follows a cut trick up to the cut. It takes a late read as
the next update's step when the next sample confirms it. The clock
matches the original's in all 50 tricks, and the end in all 46 that
played out.

The real bodies also showed that the contacts' radii, which add the
clip's limbs (`FUN_001337e0`), reach past 4.5, the old edge of the open
bin. Past 5.5 the clip's root holds within 0.01 in 783 of 785 steps:
32.49 against 32.49 along the facing. The open bin is now 5.5, with its
pins tightened to 98% and 2%.

**4. Swipes, 7 of 8 on time.** One "strip", passes-1's 4503, was the
shot clock's violation dropping the ball. The check also posed the
swiper on the update's own clip, which is not when the original tests.
The test runs in `FUN_0015da28`, the side's update, after its presses
and before the swiper's channels step and he moves (`FUN_00156f48`).
The ball (AIPhysBall) updates after every actor (below). So the test
reads the clip, the place and the ball that the update before left.
Posed that way, 5 of the 7 strips come on the first update in reach. The
other two never quite reach it: they are 0.02 and 0.16 short on the
strip's update, the second inside the swipe clip's blend. Of the same
seven the old check had 2 on time, 4 an update after the posed hand
first reached and 1 never in reach.

The captures show more about the swipe, which this pass leaves
unported:
- **No velocity.** The swiper's actor+0x40 stays 0 through every
  captured swipe (3,068 updates). He still moves in 2,592 of them, by
  the swipe clip's root, which `trick::root_step` and `root_motion`
  reproduce from the captured frames.
- **The facing.** The press sets his wanted facing (+0xb0) and the stick
  to the angle at the holder, and he turns to it 46 units an update.
  Because he never moves, `FUN_0016cf30`'s "moving" never holds, and the
  reach is always 1.5.
- **The rewrite's swipe** walks at 0.4 on the stick (reach 0.75 when
  pushed), keeps its facing, and tests the hand in the press's own
  update. That is one update ahead of the original, which tests first
  the update after the press and last the update after the script ends.

Ported together, these move the computer's attract games out of the
comparisons' bounds: the trick blend's back-off reaches 0.49 against
0.41 in the first capture, and block jumps at a shooter fall to 8.2
against 17 in the third. So they are left, with their measurements, for
a pass of their own.

**5. The person's lobs to a leaper, 9 of 16.** Three of the 16 were
passout specials to a leaper (game+0x160 1 or 2; it is -2 on every plain
lob). Those are aimed off the board or the head (`World::passout`), not
by `FUN_00162aa8`, and the check now leaves them out. The rest turn on
the order in which the actors update: the lob reads the leaper's span
(`+0x6c − +0x70 − 5`) as this update left it when the leaper updates
before the passer.

**The actors' update order.** Each frame `FUN_0033ce00` runs the process
registry (the vector at `0x4deae8`) front to back, calling each object's
update (vtable +0x34). The registry's order is:
1. Front End, Pause, Overlays, action tap, game pause;
2. AIGame, AI Script Handler, Script Manager;
3. the two AITeams, then the six AIPhysPlayers;
4. the two AIPhysBackboards, AIDetect, AIPhysReferee;
5. AIPhysBall (priority 7), EAI (8), Camera (10), Replay Manager (14).

`FUN_0033cf98` inserts objects by priority, with one exception. Each
AIPhysPlayer goes in at a random place among those already in,
`uVar8 + FUN_003a16b8() % (count + 1)`, drawn from the shared generator.
The AITeams build their three players in turn (`FUN_00189cf0` →
`FUN_001563a8`), and each registers in its constructor. Save state 1's
match runs 5, 0, 4, 3, 1, 2 (save state 2, later in the same match,
too); save state 9's match runs 4, 0, 5, 3, 1, 2.

With save state 1's order, all 13 lobs fly the original's time, and the
point lands 0.03 off at the median (worst 0.06). With the index order,
9 do. The two plain lobs that matched either way had flights set by the
distance table, not the span.

The rewrite updates the actors from the highest index down. That is its
own order: it agrees with save state 1's for team 1 and is the reverse
for team 0. The shuffle's draws sit somewhere in each match's
construction. Save state 1's match was built long after its reseed
(257,797 draws before the state), so the draws are not placed and the
order stays the rewrite's (`gameplay.actor_update_order`, now a known
difference).

**Measured on the fourteen** (`STREET_PLAYED_CAPTURE` with all fourteen
captures; 26 of 26 tests pass):

| Measure | Result |
|---|---|
| Catches | 227 checked (88 the person's); 2 cut by a move; none off |
| Block jumps at a held ball | 23 of 24 rise within 0.02 |
| Block jumps at a ball in the air | 53 of 56 rise within 0.02; drift within 0.02 in 45 |
| Tricks | 50; clock 50; end 46 of the 46 played out; coast 50; facing 45; 4 cut; 3 late reads |
| Swipes | 86 swipes, 7 strips; 5 on the first update in reach, 2 within 0.2 of it |
| Lobs to a leaper (`FUN_00162aa8`) | 13 of 13 flights exact; point 0.030 off at the median (worst 0.060) |
| Jumps, height within 0.1 / rise within 0.01 after the blend | jump shots 99.8% / 99.3%; side fades 99.9% / 99.7%; dunks 97.8% / 97.1%; alley-oop dunks 99.8% / 99.4%; lay-ups 99.1% / 97.8%; runners 54% / 97% |

The person's presses are unchanged: shoot 90 of 90, pass 82 of 83.

The three attract captures pass 17 of 17 each, with capture 1's means
unchanged (counter presses 17.8, turnovers 4.5, shots 55.9).

## The loose ball (2026-10-18)

The second audit's item 2 (`docs/AUDIT_1TO1_2026-10-18.md` 1.3): the
rewrite's ball came loose three to six times as often as the original's.
Traced raw in SLUS_206.51 (`tools/mipsx.py`, with the EE's `lqc2`/`sqc2`
and VU0 macro ops decoded by hand), ported, and measured against the
three attract captures (16-seed means on each capture's own players) and
the fourteen played captures.

**The receiver's near take** (the offence's tail, `0x15bad0..0x15bf28`).
"The ball on the bones" read `0x8280` as a leaper's or a dunker's bits.
It is bits 7, 9 and 15: a dunk, the catch row's bit 9 (its script's
`9b 0x200`) and a leap. Before anything else in the tail, an actor with
any of them takes a free ball near his bones. The model's bone positions
are read (vtable +0x18, +0x0): the pelvis (0), the right hand (0x13) and
the left (0xf), and the ball (ball+0x30). Which bones count depends on
ball+0x100 (the receiver) and ball+0xfc (the holder at the press,
`FUN_0014bff8`):

- a leaper whose pass is to himself, or a dunker: the hand channel 0's
  frame holds out (+0x14: 8 the right, 4 the left) within 1.0;
- anyone else with the bits (a receiver on his catch row, a leaper for a
  teammate's lob): the pelvis or either hand within 1.0 (the VU0's
  `vsqrt` of the squared difference against 1.0).

A low, slow ball (at most 2.5 up, rising or falling at most 6 mph a
frame, he on the floor, not in the pickup clip, not barred) is scooped;
any other is his (`FUN_0014cb08`), and +0x1a0 is cleared. With bit 9 set,
the other side's three are barred 20 updates (`FUN_00140920`, +0x1a8 =
0x14). Then (`0x15bf3c..0x15bf6c`) with any of the bits, the pass still
in flight (ball+0x60 below +0x64) and nobody holding it, the tail ends.
So the receiver on his catch row is never swept while the pass flies; the
sweep's hand rule (`0x15c2fc..`) is reached only after the flight's end.
This was the untraced piece behind the running catches' knocks (82 off an
upper arm in four games): the overlay's arms were swept in flight, which
the original never does. It also corrects "The ball on the bones": the
catches it fitted to the sweep's forearm and hand are the near take's.

**The model the tests read.** The pose step (`0x157618..0x157714`) runs
after the role update in `FUN_00156f48`, so the tail reads the model as
the last update posed it: the channel's frame and the place at the end of
the update before. The played captures' 231 standing catches (posed from
the captured channel): the near take on the last update's frame first
finds the ball on the catch update in 229, one an update early, one
never; the sweep's hand rule on the update's own frame (the old check)
174. The receiver's catch row now steps after his tests
(`World::catching_tick`); its first step runs at its start, before them.

**The parked row and +0x1b0** (`0x15a974..0x15ab4c`). The release parks
the row's script (+0x1bc) and the side's update starts it once the flight
left is at most actor+0x1b0 (the flight, or the row's own count:
`catch_stand_4r`/`4l` 20, `catch_stand_5` 15, `catch_post_2` 10), with
him on the floor. A barred channel (0x40) cancels it. Either way +0x1bc is
cleared and +0x1b0 set to -1. The catch filter (`0x162028`) keeps a row
with a shortest flight out of a pass whose passer's +0x1b0 is below it.
So after any catch, `catch_stand_5` and the 4r/4l rows are out of his next
pass. The rewrite kept the flight there: `catch_stand_5`
(`readycatch05_low`) was drawn for 8.5% of its passes. The first attract
match's 161 catches never show it (`catch_stand_1` 40, 3l 25, 3r 20, the
running rows over the gait). Its crouch put the ball into the receiver's
body: 151 of the 216 knocks in eight games of the first capture's length.

**Running catches over the run** (`0x15a9d0..0x15aadc`, the aim
`0x1625bc..0x162740`). `DAT_003d70b0` is 1 in the executable's data and
nothing writes it, so channel 0 carries the gait on (`FUN_00163d40(actor,
0, 1)`) and the row's script runs on channel 2. That happens when he is
moving (actor+0x40) as the script starts, with +0x1ac (on the floor at
the release, `0x161360..0x161388`, on a row without 0x800). The pose step
writes channel 0's parent-local rotations, its bone 0 turned by the
facing (actor+0x50). Channel 2 (flag 4, set by the rows' `91 06 00 02 00`)
writes bones 9..21 over them. The release's aim poses the row's clip at
its catch frame (`FUN_0019bda8`) and copies bones 0..8 from channel 0's
clip (`lq`/`sq` at `0x162704..0x16271c`). That clip is at its whole frame
plus its rate times the flight (`DAT_003d65a0`, the receiver's +0x1b0 at
the release), wrapped within the clip; in a blend, the target at its
start. The pelvis stands at that clip's root height
(`FUN_0019bde0(record, buffer, &x, &y, &z, &state)`: x and z land on the
same local) and the spot, with no root travel. The aim's facing is +0xb0
on every running row (none has 0x10000). Ported as `Catching::lower`
(record, frame, rate, stepped with the row) and `Poses::overlay` in both
the aim and the pose.

**The channel blends.** Every gait record change blends channel 0
(flag 0x200000) from the old record, held at its frame, to the new one.
In pads-1's first 40,000 samples there are 2,765 channel-0 blends; 1,568
last 6 updates and 1,231 go from one gait record to another.
`World::actor_pose` now poses the gait's change of row as that blend,
weight c/6 with c from 1, from `PlayerGaitChannel::blend_from`. The
original holds the target at its start frame (+0x60) through the blend;
the rewrite's gait clock runs on from 0 (left: the clock is checked
elsewhere). Script blends (`8e`) were already posed.

**The contacts on the distance table.** `FUN_001343e8` tests the two
radii (`FUN_00133b48`) against the game's distance table
(`FUN_0018d0c0`: game+0x1ac + 0x18 a + 4 b by the actors' ids, built by
`FUN_0013fbc8` before the actors update) at `0x1344ac..0x1344cc`, and pushes by the radii less
that distance (`0x135c34..0x135c64`), along the line between where they
stand now. `FUN_001337e0` also starts its nearest limb from the table.
The rewrite measured both from where they stood after moving. This is
what moved the spacing when the swipe was ported alone. The swiper stands
(actor+0x40 0), so a juking handler (the juker is not moved) pushed him
up to 0.4 an update on the overlap the handler's own step had just
made. Through a trick's first six updates, the nearest defender in a
swipe moves away from the handler's end point by -0.05, 0.07 and 0.04 in
the original's three matches, against 0.90 with the swipe ported over the
old contacts. On the distance table, the 48-seed back-off means are 0.394
against 0.406, 0.449 against 0.388 and 0.403 against 0.337. A runner now
rests his own step inside the radii
(`tests::contacts_go_by_the_distance_table_built_before_the_actors_move`).

**The swipe on its script**, as "The pass captures' five checks" ported
and reverted it: no walk, the clip's root after the blend, the press's
facing at the holder turned 46 an update, the test from the update after
the press on the last update's clock and place. Turnovers moved from
4.5, 0.9 and 2.25 to 7.9, 1.7 and 3.3 (the original's 6, 0 and 3).

**The contacts' scripted handlers** (`0x3d0c30`, kinds 1..7 by actor+0x198,
`FUN_00139408`'s predicates at `0x3d0c40`): `FUN_001366d8`, `FUN_00136290`,
`FUN_001372d8` start reaction scripts (`FUN_0019c2f8`, a clip by `rand()`)
and keep each pair's contact count (`FUN_00172008`); `FUN_001371d8`,
`FUN_001366a0`, `FUN_00137868` are predicates. None frees, knocks or takes
the ball, so they do not bear on the loose ball (the post-up and charge
reactions stay unported).

**The receiver swept.** "The receiver excluded rather than swept" was the
turnovers pass's limitation; the ball-on-the-bones pass swept him every
update. The original's rule is the near take and the flight's exclusion
above.

**The actors' update order** (not ported). Five of the six AIPhysPlayers
go into the registry at `first + rand() % (k + 1)` (`FUN_0033cf98`, k the
players in so far; the first without a draw). Each actor's constructor
then draws its standing clip, `rand() % 5` into the table at `0x3d65c0`
(`FUN_001563a8`). The placement before the game's first update draws two
for each player in turn: `FUN_00156ab0`'s clip and the AI player's
(`FUN_0013aa48`). Those twelve sit 14 to 3 draws before frame 0 in six
of the seven fresh-boot rematch captures, with one table: 0 66, 1 50, 2
52, 3 53, 4 67 (the boot fills it). Save state 1's order (5, 0, 4, 3, 1, 2)
comes out of the draws 26, 29, 32, 35 and 38 after its reseed, if each
player's construction makes three draws and its stage pair sits at 56.
That reading is not confirmed independently. Save state 9's match is not
its boot's first, and the attract captures' spot passes tell only two to
four pairs of actors apart. The rewrite keeps the highest index first
(`gameplay.actor_update_order`).

**The tally.** The rewrite's knocks are now counted as the original's
(`Touches::ours`): the ball loose after the knock, above 0.6, below 20 and
not about the rims. The audit's "shots knocked loose" of 4.75, 1.5 and
4.0 were mostly block jumps' swats about the rims, which the original's
tally leaves out. On the same tally, main had 1.3, 0.4 and 0.9 against 2,
0 and 2.

**Before and after** (16-seed means of games of each capture's length on
its own players, the second capture every player at 50; before is main at
fb40ac1 with this tally):

| | before | after | original |
|---|---|---|---|
| knocks off a body outside a block jump | 9.9, 2.1, 6.7 | 0.63, 0.19, 0.88 | 1, 0, 0 |
| passes knocked loose | 10.4, 2.2, 7.1 | 0.75, 0.25, 1.06 | 2, 1, 1 |
| passes loose at their timer | 9.6, 1.75, 5.2 | 8.0, 1.75, 5.5 | 1, 0, 0 |
| shots knocked loose | 1.3, 0.44, 0.88 | 1.06, 0.31, 0.88 | 2, 0, 2 |
| catches with 5+ updates of flight left | 8.7%, 6.1%, 9.0% | 8.7%, 7.6%, 10.6% | 1.2%, 8.2%, 2.2% |
| held balls knocked loose | 6.3, 1.6, 6.4 | 13.0, 6.1, 10.8 | 9, 2, 10 |
| loose-ball phase, updates | 1,438, 248, 911 | 1,568, 366, 1,036 | 762, 396, 915 |
| loose pickups | 71.8, 19.3, 52.6 | 66.7, 20.0, 51.3 | 57, 24, 46 |
| rebound catches | 10.3, 3.8, 6.0 | 8.7, 4.9, 6.3 | 14, 3, 11 |
| turnovers | 4.5, 0.94, 2.25 | 7.9, 1.7, 3.3 | 6, 0, 3 |
| within 4 | 0.265, 0.250, 0.260 | 0.251, 0.239, 0.253 | 0.262, 0.201, 0.244 |
| the trick blend's back-off | 0.423, 0.442, 0.436 | 0.380, 0.498, 0.404 | 0.406, 0.388, 0.337 |
| counter presses | 17.8, 7.8, 15.4 | 21.75, 9.4, 15.3 | 17, 3, 15 |

The played captures: 26 of 26 tests pass on all fourteen before and after.

**What is left, with its evidence.**

- *Passes loose at their timer* are two of the rewrite's own things.
  First, the computer's trick pass off the glass to himself: its legs are
  the rewrite's flight (`FUN_00148378`'s passout legs are not ported). In
  eight games of the first capture's length, 24 of its 25 such passes were
  lost at their timer, about 3 a game. Second, running receivers carried into
  the court's bound on long passes. None of the original's 107 passes to
  moving receivers in the first match ends with the receiver at the bound.
  In the rewrite, 64 of 558 running-row passes do, half of them lost: its
  computer's receivers run toward the sidelines (the computer's offence,
  fix order item 4).
- *The late catches* are the inbound lobs. `catch_stand_1`, 3l and 3r from
  behind the baseline are caught 5..7 updates before arrival. In the
  original's two long matches, `catch_stand_1` inbounds are caught 3..5
  updates early (mostly 4) and 3l/3r at 2. The ball is about 2.3 from the
  receiver's feet at the catch in both. The rewrite's lob covers about
  0.4 an update against 0.45: the passer releases about 8 from the
  receiver against 8.8. Other passes are 1.5% late against the original's
  1.2..2.2%.
- *The loose-ball phase* follows from both: each lost pass is a loose
  ball. Add the strips the swipe on its script makes (held balls knocked
  loose 13.0 against 9 in the first match, 6.1 against 2 at 50) and fewer
  rebounds caught in the air (8.7 against 14).
- *The update order* above; *the gait's target held at its start* through
  a blend; *the post-up and charge reactions*.

**Checks.**
`played_capture::the_receivers_hands_meet_the_ball_where_the_original_catches_in_play`
(the near take on the last update's frame: 229 of 231 on the catch, at
least 80% asserted, at most one in fifty early);
`ai::capture::shots_turnovers_and_spacing_against_the_captured_original`
(the loose-ball figures checked against each match, bounds by the rule;
the seed set +16000 inside every bound); `tests::touch`
(`a_receiver_on_his_row_is_not_swept_while_the_pass_flies`,
`a_pass_meeting_another_limb_after_its_flight_is_knocked_off_the_receiver`,
`the_near_take_catches_by_the_pelvis_and_bars_the_other_side`,
`a_row_with_its_own_count_waits_for_it_and_spends_the_receivers_count`,
`a_running_receiver_plays_his_row_over_his_run`, with the snapshot); `tests`
(`contacts_go_by_the_distance_table_built_before_the_actors_move`,
`a_change_of_gait_row_blends_the_pose_from_the_old_row`, the screens'
rests); `tests::turnovers` (the swipe from the update after the press).
Snapshot VERSION 54.

## Who the person controls, and the stick (2026-10-18)

The second audit's fix order item 3 (docs/AUDIT_1TO1_2026-10-18.md sections
1.4 and 1.5). Branch `control-stick` (off c9dbdd4). Traced raw from
SLUS_206.51 (file offset = address − 0xff000; Ghidra's decompile for the
shapes, the raw MIPS for every condition and constant), checked against the
fourteen played captures and a new one of the person defending
(`$EVIDENCE/street-played-defence-1.jsonl`, below). Port:
`crates/street-sim/src/control.rs`, `World::step`, the pad's words in
`input.rs`; the clients (`street-game/src/controls.rs`,
`street-net/src/game.rs`) now follow the world.

**The controller words.** game+0x24 + actor × 4 (`FUN_00156d98`) is the pad
(0..3) that drives the actor, or 4 for the computer. `FUN_0013cf58` counts
each side's pads into game+0x9a + side and starts the pad of a side one pad
plays on its first player. The construction (`FUN_0013c4e8`, `FUN_0013d388`)
sets the team's +0xc8 to `game+0x9a != 1`, and every move below waits while
it is set: on a side two or three pads play each keeps his own player for
the whole match and Cross never switches. On a side one pad plays,
team+0xc8 is the give 'n go. The simulation read the words from its inputs
already (`World::people`); the moves are new, and a client puts each
person's pad where `World::person_on` names after each step.

**The four moves.**

- *The action tap* (`FUN_0012f250`, the "action tap" process, priority 2,
  before the game's update): it drains the pad messages into `FUN_0012f2c8`,
  then takes each pad record's pressed word as held and not held before (per
  pad, not per actor). The pass message (case 9) is a pass press
  (`FUN_00146228(pad, -1)`, the pad's 0x100) only for the side with the ball
  (`FUN_0012fd18`: game+0x110 the pad's side) outside phases 2 and 3;
  otherwise it sets no bit, and outside phases 0 and 1, with the side's
  team+0xc8 clear, it is the switch (`FUN_0012fd60`). (The script manager's
  states 3..7, a replay, and the process at priority 2 held return first.)
- *The game's update* (`FUN_0013d868`, `0x13e004..0x13e0d4`): outside phase
  0, with game+0x110 set and that side's team+0xc8 clear, a holder
  (ball+0xf4) whose word is 4 takes the word of the side's first slot that
  has a pad. The original does not ask the holder's side; the port does
  (every captured hand-off had the holder on the side with the ball).
- *The take* (`FUN_001415c0`, slot 24: every catch, steal, pickup, rebound,
  the tip's take and the restart's hand-over): the same for the new holder
  when his side's team+0xc8 is clear.
- *The release* (`FUN_00173570`, callback 38, `0x173dd0..0x173e84`): with
  the passer's team+0xc8 clear and the receiver (actor+0x74) the computer's,
  the receiver takes the passer's word and the passer 4. Then, if the
  passer's word is 4, team+0xc8 set and game+0x9a 1, team+0xc8 clears (a
  computer's pass ends a give 'n go).

So the person's control leaves the passer as the ball does. The audit's "19
updates after the ball left" counted from the ball's state 4, which the pass
setup (`FUN_0014bff8`) sets at the press: they are the inbound row's windup
(6 + 12 and the release's update). The 43 changes in a pass's flight in
pads-1, pads-2 and fades-1 all come in their release's update (the flight
set, ball+0x64). Over the fourteen played captures and the new one, run as a
state machine (the moves, team+0xc8 below, Cross's switch where the action
tap may switch), the rule's person is the captured one in every one of
127,727 updates: 158 changes at a release (one sampled before the ball's
update stepped its progress), 133 at a take (one a tip's take and shot in
one update), 167 switches; and the four right-stick passes in passes-1
(the passer's pressed word 0x400) keep the passer through the release and
the catch, as the give 'n go holds
(`played_capture::the_persons_control_moves_where_the_original_moves_it`).

**The give 'n go** (team+0xc8 on a side one pad plays). Set by the holder's
right-stick pass (`FUN_0015a060`, `0x15b900..0x15b940`: game+0x9a 1,
team+0xc8 clear and the pressed word's 0x400, before the pass start
`FUN_0015f930`). Cleared by a shot's release on the side (callback 21 after
the launcher, `0x1734fc..0x17352c`), a computer's pass release on it
(above), the side being without the ball (the team's update `FUN_0018a708`),
and the person pressing the right stick home again off the ball
(`FUN_001587d8`; the next game update then hands him the holder). While it
is set, Cross off the ball calls for it (`FUN_001587d8`'s branch for a
person not holding, team+0xc8 set, a holder, game+0x110 his side and 0x100
pressed): a holder whose word is 4, not in a pass (bit 4), has his AI's
queue cleared (`FUN_0013aa48`) and a pass node to the caller pushed
(`FUN_00100a08`, kind 8), or with two turbos held (0x40000) a trick pass
node (`FUN_00100c60`, kind 0x17); Circle without a turbo pushes a shoot
node (`FUN_00100e38`, kind 9). The rewrite's call throws the pass at once
(`World::call_for_ball`), now on the original's gate; the trick pass and
shoot calls are not ported. Its own call for anyone off the ball on the side
with it is gone (no computer behaviour presses Cross off the ball).

**The switch** (`FUN_0012fd60`, raw `0x130634..0x130864`). Two other ways
are off. A per-pad direction pick reads bit 12 + pad of the live options
block's word 3 (`FUN_001e0bc0()` is the config `0x4bbf10`, +0x138 =
options+0x3c, written by `FUN_001d7458`), 0 in the reset's block (0x249);
the scored pick explains the new capture's switches, all made with the
D-pad held toward the ball's holder.
The cycle (`def_switch_cycle`, `DAT_003d01c8`) is registered 0 at
`0x1310c8`, and no `CSCRIPTS` file sets it (`AUTOEXEC.CFG` sets only
`demo_delay`). So every switch is the scored pick over the pad's side: a
pass in flight (ball state 4, no holder) scores each by his distance to the
receiver (ball+0x100, the distance table `FUN_0018d0c0`); else, when a shot
is up that is his side's, falling or above 8 (state 5), or the holder dunks
or lays up (bits 7, 13), or the person's actor is his side's nearest the
ball (game+0x3a4 + side × 0x18, `FUN_0013fbc8`, ranked by `(int)(a − b)` of
the distances in its quicksort `FUN_003b3868`, so ties within a unit fall in
the sort's order; the port takes the exact least), by his distance to the rim
the offence attacks
(game+0x374); else by his distance to the ball (game+0x3d4). A fallen
teammate (channel 0's flag 0x40, `FUN_0016ea70`) adds 3; one off the screen
(`FUN_00171920`: his feet, actor+0x260 − 2, through the active camera's
projection, outside the frame) adds 20, which the simulation, with no
camera, never adds. The least score under 200 among the teammates whose
word is 4 wins, the first of equals; the old word goes to 4. The tables are
the last update's (`FUN_0013f748`, `FUN_0013fbc8`, `FUN_0013fed0` are the
game's vt+0x114, +0x11c, +0x124, run in its update before the actors move
and after the action tap), so the simulation keeps them (`control::Tables`).

**The new capture.** `street-played-defence-1.jsonl` (schema 2, updates
201..17855, none torn or missing): from save state 1, the person chases the
holder on the D-pad and presses Cross every 0.6..1.6 s while the other side
has the ball, and in loose balls and shots; with the ball he shoots long or
passes (the match ends 21 to 3). 169 presses, 167 switches: the scored
pick's 152 exactly (89 of 101 by the rim, 62 of 65 by the ball, 1 by a
pass's receiver); 13 more give the captured pick with 20 added to the
computed one, the screen's term; 2 do not (frames 973 and 12423: the
computed pick in a swipe and in a rebound jump)
(`played_capture::the_persons_switches_go_to_fun_0012fd60s_pick`). Every
played test passes on it, so the gate's played run takes fifteen.

**What a person's actor reads** (`FUN_001587d8`). His pad record, except
while he is a pass's receiver (ball state 4 and ball+0x100: from the press
to the catch), the computer walks him (actor+0x298) or he stands outside
±40.5 by ±25 (`FUN_0016f530`, the place now): then his AI's frame
(`FUN_0013a970`), the juke's held 0x800000 carried from the pad for a
receiver posting up (bit 11, not modelled). A receiver's stick words (+0x84,
+0x88, +0x90, +0x94, +0x98) are not written while the ball is in state 4 and
he is ball+0x100: he goes on along the stick he had at the press. The
captures hold his angle and strength in 18,169 of 18,202 such updates
(others change in a quarter), and he moves along it (median 0 units off; the
rest are catch rows). Ported for the person's receivers (`World::pad_off`,
`World::frozen_input`). The computer's keep steering: holding theirs too
moved the attract comparisons out of their bounds (passes loose at their
timer 12.1 a game in the first match against 8.0 and the original's 1;
the second's passes knocked loose after a release 3.1, bound 2; the third's
block jumps at a shooter 7.9, bound 8, the original's 17). The rewrite's
leads and court bound lean on the steering (its running receivers carried
into the bound: the computer's offence, fix order item 4).

**The stick's words.** The pad driver (`FUN_00147700`, the left stick;
`FUN_001479f8` the right) takes the raw axes (0..255, 127 the middle; both 0
means no analog data, strength 0) to a strength `trunc((√(x² + y²) + 0.5) ×
64 / 127)`, at most 64, with no dead zone, and a word `trunc(a × 162.97466 +
0.5)` of the EE's `atan2(x, y)` (its polynomial, `ordinary::native_atan`,
the quadrants by y's sign). The pad builder (`0x158a1c..0x158dc0`) makes the
D-pad's own: 64 a side toward the arrows held (up −y, down +y, left −x,
right +x), strength `trunc(√)`, 64 or 90, the word of the same arctangent;
it keeps the larger strength (the stick's word on a tie, else the D-pad's),
and for a person adds the active camera's yaw word (`DAT_00402020`+0x94,
`trunc(yaw × 162.97 + 0.5)`). In the three captures that keep the yaw (the
two pass captures and the defence capture) the person's words less the yaw
word *of the update before* (the camera updates after the actors) are the
D-pad's exactly in 16,365 of 16,526 updates at 64 or 90 on his own pad (72
more within 2 units, the yaw read straddling an update), with the arctangent's
half-unit quirks: down 0, down-right 128, right 256, up-right 384, up 513,
up-left 641, left 769, down-left 897. Their WASD (bound to the left stick)
gives only full pushes, whose words are the D-pad's straight ones, so no
capture has a partial analog push; the analog rule is static.

The yaw is the camera's `atan2(look.x, look.z)`, which as a word moves along
the screen's *down*: the client's `Input::view` sent the screen's up, half
a turn off, so a person's pass at rest went the other way (the played tests
fed the captured yaw, which hid it). Now `Input::set_yaw` takes the camera's
own yaw (`view::native_angles`), and the client builds the stick from the
pad driver's and the pad builder's rules (`input::person_words`: the
keyboard's arrows and WASD are the D-pad, a pad's left stick its raw axes,
its D-pad the D-pad). The 8-byte pad carries the strength exactly
(`Input::set_words`, `Input::strength`); the word only to the axes'
resolution. The locomotion moves at strength 16 or more (`FUN_00163d40`;
`input::MOVES`, which replaces `tuning::DEADZONE`'s 0.2, 12.8 of 64), and the
computer's strength class (`FUN_0013abf8`) reads the rounded strength (it
truncated a quantised length).

**The stop, left.** With the strength under 16 the stand path sets no
velocity: actor+0x40 is 0 from the update the stick drops in all 246 of the
played captures' stops of a moving person. The gait's and then the stance's
clips carry him on by their roots: median 0 for 7 updates, the 90th
percentile 0.18 an update for ten, then about 0.04 an update on record 656.
The rewrite does not play those roots, and stopping at once without them
moved the attract comparisons out of bounds (the first match's within 4
0.239 against 0.262, bound 0.02, and the second's catches with 5 or more
updates left 0.008 against 0.082; at once in the windup too, the trick
blend's back-off 0.487 against 0.406 and 0.481 against 0.337, the third's
passes loose at their timer at its bound). So `tuning::DECELERATION` stays,
labelled as the stand-in for the stop's clips. The walk while a pass winds
up (and a swipe without the disc's animations) stays the rewrite's
(`ACCELERATION`, `TURN_RATE`): the original's passer has velocity 0 in
2,465 of the captures' 2,939 windup updates while his pass clip's root
carries him (the pass on its script, not ported).

**Edges.** A person's presses on a side one pad plays are his pad's
(`Control::pad_before`), so control moving between players makes no press of
a button held across it.

**Checks.** `played_capture::the_persons_control_moves_where_the_original_moves_it`,
`played_capture::the_persons_switches_go_to_fun_0012fd60s_pick`,
`played_capture::the_persons_d_pad_words_are_the_pad_builders`; `tests`
(`a_give_and_go_comes_back_when_called_for`,
`the_persons_control_goes_to_the_receiver_as_the_ball_leaves`,
`cross_on_defence_switches_to_the_teammate_nearest_the_ball`,
`the_action_tap_switches_on_the_last_updates_tables`); `input`
(`the_pad_words_are_the_pad_builders`). Snapshot VERSION 55.

## The computer's offence (2026-10-18)

The second audit's item 4 (`docs/AUDIT_1TO1_2026-10-18.md` 2.2 to 2.5),
with the passes lost at their timer that "The loose ball" traced to the
computer. Traced raw in SLUS_206.51 and ported where the trace was whole.
Measured on the three attract captures: 16-seed means of games of each
capture's length on its own players, the second capture every player at
50, through the comparison and a scratch harness that is not committed.
Also checked on the fourteen played captures.

**The receiver's run to the pass.** The pass start (`FUN_0015f930`,
`0x15fd14..0x15fe4c`) keeps the receiver's spot (`DAT_003d70c0`/`c4`). It
sets the spot run (`DAT_003d70b4`) when he gets there within `fps / 2 -
trunc(fps × 0.12)` = 23 updates, whether or not the pass then leads him.
For a receiver on the floor who is not leaping, posting or the passer,
`FUN_0016c970` points his run at the spot: the requested heading (+0x88)
and an intensity of 0 within 1.5, 39 within 3, 59 within 17.5, else 64
(+0x90). The pad builder (`FUN_001587d8`, `0x158adc..0x158af8`) leaves
both alone while the pass is his (ball+0x180 4, ball+0x100 him), so that
run is his stick through the windup and the flight (`World::receiver_stick`).
The release (`FUN_001611d8`) clears the run (`0x161350`) and decides
again. A pass to the spot sets it (`0x161ee0..0x161f1c`; timed on 1.5 or
less, the spot is where he stands). A led pass whose straight run would
leave the court (±40.5 by ±25) behaves as follows (`0x1619b8..0x161ed8`):

- past both edges, he stops where he is, facing the ball;
- heading straight at the edge, he runs to the spot;
- otherwise he turns toward the point `FUN_001636c0` gives, along the
  edge, at his speed.

With the run set, no catch row is parked (`0x162420..0x162450`). His own
update (`FUN_0015a060`) keeps a moving receiver off the edges
(`FUN_0016f668`: past ±25 or ±40.5 by his place plus his velocity, within
0x57 of straight out he stops facing it, else he turns along it). With
the run set, it stops him within 1 of the spot moving (2 standing; corrected 2026-10-25), on the floor,
facing the ball. In the flight he plays `catch_stand_1` at once
(`0x15a708..0x15ab50`); before the release, `breathe_1`, held until his
catch row (`0x15a4a8..0x15a700`). Ported as `crate::receiver`.

**The nodes' pass targets.** The spot is the receiver's front behaviour's
vt+0x24 (`FUN_00133380` from the pass start's `FUN_0013b900`), not its
walking target:

- the fast break and the roll: the point `min(7, his distance)` from the
  rim on its line to him (`FUN_0010aea8`, `FUN_001180d8`; 7 is the AI's
  +0xb4, set at `0x17da14`);
- the net cut: that point on its way in, and on its way out the spot, or
  10 from the rim toward it once he is within 10 (`FUN_00110ae8`);
- the circle cut: that point while on its arc within 0x80 of the line to
  the rim, his place within 0x100, else the spot (`FUN_00111720`);
- the post cut: 7 from the rim on his line, from 7 out with the spot
  within 200 units of that line, else his place (`FUN_00112f48`);
- every other behaviour: its target.

The rewrite had aimed at the walking target, so a receiver on a net cut
was passed to at its far spot while he ran in. The first release of the
spot run, with the old targets, lost more passes than before, not fewer.

**The passout to oneself off the board** (Back 2 Papa). The press keeps
the passout (`FUN_00142068`: game+0x140..+0x160, the board's point and
kind 1). The computer's point is `FUN_00183318`'s draws: x is the side's
`38 - r`. Then, when `rand() % 80` is below Handles, z is `f·q.z + (1 -
f)·p.z` clamped to ±3 (q the receiver mirrored behind the board), and y
is `15 - U × 1.4`. Otherwise z is `(-3 - r) + 2U(r + 3)` and y is `(11.5 -
r) + U(r + 1.4)`. The pass start then takes, for a pass to oneself, only
the rows with flag 0x200000 (83..90: `passout_overshoulder`, `_close`,
`_travelling`, `_spin_1`, `_spin_2`, `_bounce`). They are banded by `max(|ahead -
point| + |point - led|, 25)`, the passer a quarter second ahead. The
row's script holds the ball to its callback 53 (`FUN_001741a8`): at
ticks 20, 14, 32, 15, 21 and 24, with the clip frame the hand is posed
at 21, 15, 24, 31, 34 and 54. The callback aims the leg back at the hand
as the clip shows it at that frame (`DAT_003d6590`) and times both legs
(`0x17481c..0x174964`): the whole `trunc(max(d1 / S + d2 / (0.75 S), 20) ×
0.8696)` and the first `trunc(max(d1 / S × 0.8696, 2))`. It bars the
passer's side for the whole flight (`0x174b1c..`). The script then sets
bit 9 and waits for the flight (`8d ee F ef 00`). At the first leg's end
(`FUN_00148378`, kind 1) a point below 13.25 (`0x41540000`) ends the legs
and the ball comes off the board loose. Otherwise the second leg flies to
the hand in what is left of the whole. Ported as `crate::passout`; the
computer's trick pass draws its point in `ai::offence`'s board point.

The person's Back 2 Papa to himself takes the same path. The pass
captures hold five such passes (`street-played-passes-1` frames 3936 and
14624, `-2` 29109, 32067 and 34380). In each the passer stays on the
floor, sets bit 9 at the toss 20 updates after the press and takes the
ball back 54 to 58 updates after it. The one whose point was 11.68 (frame
9326) went loose. The rewrite's own flight had the passer leap to meet
the ball as an alley-oop; that test is replaced. The pass off the board
to a teammate (`passout_1`/`_2`, rows 91 and 92, callback 38) keeps the
rewrite's flight.

**The fast break's pass wait.** `FUN_00125c70` (`0x125ecc..0x125f60`) runs
its pocket and cancel checks and its passes whoever holds the ball, and
`FUN_00126288` counts its wait down before it looks for a holder. The
rewrite returned without a holder, so the wait stood still through an
outlet pass's flight and the bring-up's pass node fronted less.

**Carrier pause while dribbling** (`0x101b8c..0x101bd8`). In the paint the
step and nothing else is taken only when the holder is not dribbling
(`FUN_001816b0`). The effect was within noise.

**The counter presses as the captures count them.** The original's pad
word +4 is the press edge, which the comparison reads for the original.
The rewrite's tally counted every update the computer's pad held the
steal; it now counts a press new that update.

**Tried and reverted: the counter timer zeroed after a press**
(`0x107aa4`, `FUN_00107710`). Ported, it let a stumbling defender press
again about once a stance, and the first capture's counter presses rose to
25.4 against 17 (four bounds failed). The original's stances match the
rewrite's without it (below).

**Before and after** (16-seed means of games of each capture's length,
first, second, third; before is main at c9dbdd4):

| | before | after | original |
|---|---|---|---|
| passes loose at their timer | 8.0, 1.75, 5.5 | 1.69, 0.13, 1.31 | 1, 0, 0 |
| the same, seed set +16000 | 10.6, 1.56, 5.75 | 1.19, 0.56, 1.13 | 1, 0, 0 |
| a trick pass off the board to himself, caught of thrown | 0 of 3.9, -, 0 of 1.6 | 5.75 of 6.0, -, 1.75 of 2.1 | |
| passes to a running teammate lost at the timer | 2.63, 1.44, 3.06 | 1.19, 0.13, 0.69 | |
| passes to a standing teammate lost at the timer | 1.13, 0.31, 0.69 | 0.06, 0, 0.06 | |
| loose-ball phase, updates | 1,568, 366, 1,036 | 974, 341, 807 | 762, 396, 915 |
| back-court "pass" fronts | 692, 140, 436 | 862, 226, 684 | 1,036, 622, 780 |
| "shoot" fronts | 763, 242, 479 | 712, 228, 483 | 1,391, 279, 1,087 |
| entries into "shoot" | 22.1, 7.3, 14.3 | 21.4, 6.9, 14.2 | 32, 9, 27 |
| held updates fronting "drive" | 4,137, 1,546, 3,666 | 3,731, 1,587, 3,478 | 3,352, -, 2,213 |
| a drive from 22 out, updates (first, third; both) | 101, 112; 106 | 99, 106; 102 | 121, 82.5; 101 |
| Heezay's state held, holder at level 0 (first, third) | 818, 315 | 856, 379 | 985, 612 |
| the same in carrier pause | 417, 137 | 405, 183 | 486, 496 |
| Off the Heezay thrown (first, third) | 1.63, 0.63 | 1.50, 0.69 | 2, 3 |
| counter presses | 21.75, 9.4, 15.3 (every update held) | 20.6, 8.3, 16.2 (new presses) | 17, 3, 15 |
| counter stances | | 13.4, 4.9, 9.1 | 13, 3, 10 |
| presses at a holder not tricking | 2.8, 1.75, 2.75 | 3.0, 0.94, 2.56 | 1, 2, 0 |
| defenders standing | 0.372, 0.403, 0.389 | 0.397, 0.405, 0.392 | 0.376, 0.398, 0.423 |
| shots | 53.9, 18.8, 40.1 | 52.9, 18.1, 39.6 | 55, 22, 43 |

**What is left, with its evidence.**

- *Passes lost at their timer*: about 1.3 a game in the first and third
  matches. Most are passes to running receivers (1.19 and 0.69 a game,
  net cuts the most). The rest are board points below 13.25 (0.25 and
  0.38), the original's own miss for a poor handler: every point in
  `FUN_00183318`'s second branch is below it.
- *"shoot" for a quarter of a second a shot against half.* Each entry
  lasts as long as the original's (33 and 34 updates against 43 and 40).
  The rewrite enters "shoot" less often: 21.4 against 32 and 14.2 against
  27. Its drives shoot on their own instead (11.1 and 10.4 a game against
  6 and 5, the drive's `FUN_00109898` going up within its reach), and its
  post-ups shoot 14.4 and 10.7 against 17 and 14. The original enters
  "shoot" from carrier pause 5 times in the first match (the rewrite 2.3),
  from post up 3 (0.8), and from the cut moves and the node under
  construction 4 (none). Lead: the nodes' own calls of "shoot" (each
  node's update), not the "shoot" node.
- *Who shoots* follows from the same: the side-0 big men shoot from the
  post (O'Neal 6.2 against 12, Jermaine O'Neal 5.1 against 11), and the
  original's post-ups shoot more.
- *The long drives* are each match's own draw. From 22 out the original's
  drives last 121 updates in the first match (24 drives) and 82.5 in the
  third (25), 101 over both. The rewrite's last 99 and 106, 102 over both.
  The audit's 72 is the third match's 15 drives from 28 to 34. The time
  fronting "drive" is still longer (3,731 against 3,352 and 3,478 against
  2,213): the rewrite starts more drives (34.5 and 29.3 from 22 out
  against 24 and 25).
- *Carrier pause and Off the Heezay*: the first match now holds Heezay's
  state 87% as often as the original (856 against 985 updates). The third
  holds it 62% as often (379 against 612), almost all in carrier pause:
  its high-Handles holders spend 853 updates there against 1,200, at rest
  49% of them against 68%. Lead: carrier pause's step (`FUN_00101f60`,
  `FUN_00102030`) for the third match's holders.
- *Counter presses*: the stances match (13.4 against 13 and 9.1 against
  10). The excess is presses that start nothing. The rewrite's defenders
  press 8.5 and 7.9 times a game while stumbling from a trick that faked
  them. The original's 17 and 15 presses start 13 and 11 stances. Lead:
  what keeps the original's faked defender from pressing (whether
  `FUN_00107710` is reached for him at all); the timer reset above is not
  it. The presses at a holder not tricking stay 3.0 and 2.6 against 1
  and 0.
- *The first match's defenders stand* 0.397 against 0.376 (main 0.372).
  The receivers now wait at their spots and their men with them. Over the
  three matches it is 0.398 against 0.399: the first match stands less
  than the other two. Its bound by the rule is 0.035.
- *The third capture's block jumps at a shooter* (main 9.56 on both seed
  sets) are 9.50 and 7.63 against 17. The 32 games' 8.56 is about one and
  a half standard errors under main. Bound 10.5 by the rule.
- *The pass off the board to a teammate* keeps the rewrite's flight
  (rows 91 and 92 at their callback 38 are not traced on).

**Checks.** `receiver::tests` (the run's intensity, the edges, the
23-update window, the release's point and stop); `passout::tests` (the
toss rows, the toss at callback 53 and the catch at the flight's end,
valid and carried by a snapshot through both legs, the
miss below 13.25); `tests::touch::a_spot_pass_stops_its_receiver_at_the_spot_on_catch_stand_1`;
`tests::back_2_papa_to_himself_comes_back_to_his_hands_on_the_floor`
(the pass captures' five) and `tests::presses::back_2_papa_to_himself_goes_on_a_passout_row`;
`ai::tests::the_pass_targets_near_the_rim_stand_7_from_it_on_his_line`,
`ai::tests::the_fast_breaks_pass_wait_runs_through_a_pass_in_the_air`;
`script_timing_disc::the_passout_rows_toss_at_their_scripts_callback_53`
(with `STREET_ISO`); `ai::capture::shots_turnovers_and_spacing_against_the_captured_original`
(bounds set again by the rule on the 16 bases and the seed set +16000).
The played captures: 26 of 26 tests pass on all fourteen, as on main.
Snapshot VERSION 55 (the spot run, the passout, the AI's board point).

## The merge of who the person controls and the computer's offence (2026-10-18)

Two things the branches met on. The give 'n go's call (`FUN_001587d8`,
the pad builder) went through `World::buttons` after its early return for
a busy actor, so it needed the caller to be free; on `control-stick`
alone the pass back was caught late enough that the passer's pass row had
run out, but with `computer-offence`'s run to the pass's spot the
receiver caught it while the passer was still in his row's
follow-through, and the call was dropped. The original's call is the pad
builder's and reads none of the caller's own state (`World::off_ball_call`'s
conditions), so it now goes in whatever move he is in
(`a_give_and_go_comes_back_when_called_for`). The second attract
capture's turnovers came to 2.06 a game (sd 1.29) against the original's
0 with both branches in (0.9 when its bound of 2.0 was set, the loose
ball's swipe and the passes now caught raising them); its bound is set
again by the rule to 3.0, the original's 0 being one short match.

## Street School as a capture suite (2026-10-19)

Vj's tip: the School's 26 lessons exercise nearly every mechanic, the
gamebreakers among them, so the original running them is a capture suite.
Seven captures of one School session from a fresh boot of the project's
PCSX2 runtime (`tools/pine_played_capture.py`, schema 3; out of git in
`%TEMP%/street-played-school-0..6.jsonl`, indexed in
`%TEMP%/street-played-school-index.json`) hold all 26 lessons, 45 runs and
44 commits, the person's pad driving each lesson's move from the in-game
lesson menu. Every lesson passed but Pick (three runs: the R3 press
registered and its prompt hid, but no pick bit ever came, each run failing
on the shot clock) and Advanced Dunks (three runs, below).

**Schema 3.** The recorder now finds the School game (vtable `0x441680`,
game+0x9c 4) and reads the lesson manager's words (game+0x4c0..+0x500: the
listener, the user's payload, the lesson, the delay, the result, the
prompt's words, the start, the selected actor), its listener (`0x2c`
bytes), the user's lesson words (+0x54..), the gamebreaker task
(game+0x434, its words +0x2c..+0x44), the script manager's words
(game+0x424 +0x30..: +0x50 the state), the replay manager's
(game+0x420 +0x28..), the camera manager's (`*(0x400058)+0x10`), the pause
selector and both meters' objects. It keeps a sample when the frame stands
still but these words change (`repeat`): the School's commit and the menu
hold the game's counter.

**The lesson manager on the simulation** (`street_sim::school`, moved from
`street-game`). The listener (`FUN_0018df00`, vtable `0x4413d8`) decides
on the message slots as the original's: 4 the take-off, 13 the trick
landed, 15 the steal and the counter, 16 the block (a turbo block's
channel-0 record `0x1b5`), 18 the pass launched (`FUN_0014bff8`: the kick
clips `0x207..0x20c`, the bounce legs), 19 the catch, 24 the take (the dive
within `trunc(fps × 0.12)` of its frame), 26 the basket by lesson, 27 a
shot done without one, 28 the restart's end (the start, +0x4e4), 39 the
bounce leg's end, 51, 52 the shot clock, 62 the script's end, 67 the meter's
hold, 70 the pocket, 71 the cancel, 73 the defender bitten by a pump fake
(`FUN_0013f950`; new `Event::PumpFakeBite`, snapshot 57) and 74 the pick's
contact. `FUN_0018f6b0` sets the result and the delay (`cvt.w.s(fps ×
0.9)` = 54, `× 1.5` = 90 for Cancel), ignored before the start; a later
callback sets both again. The manager (`FUN_0018eec0`) runs inside the
game's update after the shot clock and the meters (`FUN_0013d868`), the
actors and the ball after it: a result an actor or the ball posts is
counted from the next update and commits 54 after its own, one the game's
update posts (the shot clock, the manager's own possession check) 53 or 54.
The simulation's step holds the original's update n's actors and ball and
update n + 1's game update, so its run (`school::Run::advance`) takes the
step's events, then the manager: a result commits 53 steps after its step;
Switch Players' check comes before the count. The commits in the captures:
44 of 44 at the delay, the user's nibble 8 after a pass (12 after an
earlier failure), 4 after a failure
(`the_lessons_commit_their_results_after_the_managers_delay`).

**The launch** (`FUN_0018ec28`, from the menu): the lesson's side restarts
(vt+0x14c `FUN_001406b8`: the other side's held ball dropped where it is),
game+0x104 cleared, both sides snapped to their restart spots with the row
forced to 2 (vt+0x154 `FUN_001407c8`, `FUN_0012bc40` at `0x12be00`), the
shot clock refilled, and the meters by lesson (14 and 23: pockets cleared,
the side's meter filled; 24: the other side's pocket set; 25: the side's
pocket set and its meter filled). `World::launch_lesson` does it; the 45
launches put all six on the captured spots within 0.5 with the captured
holder (`the_lessons_launch_into_their_restart_spots_as_fun_0018ec28`).
In a lesson the computer neither reaches (`FUN_00107388`, `0x1073c0`), nor
counters (`FUN_00107710`, `0x107740`), nor goal-tends (`FUN_0012de70`,
`0x12df58`); a pocket goes in only in lesson 23, a cancel only in 24
(`FUN_00141098` `0x141148`, `FUN_00141190` `0x141248`). Ported.

**What the older captures never stressed.** With the School's seven
captures beside the fifteen played ones, eight played checks failed and
now pass, none weakened:

- The replay world gave the ball to team 0 whatever the holder's side: the
  School's person plays team 1, so his passes were refused. It now gives
  possession to the holder's side, the people's sides their meter fill,
  and the meters and pockets the update shows (a full meter turns two
  turbos' shot into the gamebreaker's). The replayed shots carry the
  turbos and shoulder buttons the person held.
- A staged Level 2's actors run on its script: their pad words stand as
  the press before left them (a shoot press held 100 updates), its passes
  fly their cue's updates, and its catch is no take and its pass no
  callback 38, so only the game's update moves the person's pad, the
  update after the catch. Those updates are counted apart in the presses,
  the pass flights, the guards and the control checks. The simulation's
  staged branch now runs the game's hand-off too (`World::step`).
- The first pass after the actors' reset flies 40 (`DAT_003d65b8`): the
  School's game has no toss, so its first lesson's inbound is that pass.
- The guards' mode (`0x167220..0x1673f0`) runs on every computer player
  with his stick out, in the back court too and through a pass's flight,
  where it is not eligible and stamps its end, holding it off 36 updates.
  The check reset the guards whenever play left the front court with the
  ball held; run as the original runs it, the fifteen captures' facing at
  the holder rises from 85% to 91%, the mode against the rows from 94% to
  97% (the School's lessons start in the back court).
- The runner and the gamebreaker's shots are among the person's scripts
  the chooser names.
- The School's game is a points game (`FUN_0018e9f0` builds it on
  `FUN_0012e750`) from a boot of its own: its update 0 came 41 draws after
  its reseed, against the Pick Up game's 46 and its pair; its pair (0, 17)
  matches two of those 41 places, as chance would, so where its loading
  draws it is not read.

**Advanced Dunks.** The four-turbo slot's id 48 from 8.5 or more out
(`FUN_001693b8`, `0x169628..0x16965c`) plays `bbpass_1` (`0x447b53`) on
the upper body while channel 0 runs on: callback 30, state bits 0x90, a
three-update blend and a seven-update wait, then callback 31 (`FUN_00173ec0`
with 20, 0: the ball leaves him (`FUN_0014bff8`), he is barred
`trunc(fps × 0.34)` = 20 updates (+0x1a8), and the ball flies 20 updates
to the board's point `(±(38 − r), 15 − r, 0)`, r the ball's radius
(ball+0x11c), its zone flag (`DAT_003d9210`) set only for a toss from
23..32.5 across and within 8 of the middle) and callback 32
(`FUN_001773b0`: the finish by the player's slot-0 dunk id, `0x447ed5` for
48, `0x447c1d`, `0x447d0d`, `0x447de5`, else `0x447bbe`, on channel 0: bit
15, callback 4, an eight-update blend, the aimed jump, callback 31 with 45,
1, 20 (`FUN_001631c8`: his hand at the clip's frame 20; with the zone flag
and the ball still his pass, a second leg to it in 45 updates), the long
wait, the slam at callback 21). The captured three tossed from 33 across:
no second leg, the ball came off the board loose, and the lesson failed.
The simulation names the advanced row on the press but plays a drawn row
in its place; the presses check now pins that it names it on all 32
draws, the play itself left for a pass of its own.

**The gamebreaker task** (`FUN_00144df0`, its process `FUN_00144ef8`; the
second audit's 3.2). Created by the game listener's slot +0x204
(`FUN_001422f8`, one at a time), it runs as a process before the game's
update, which deletes it once its busy word is clear (`0x13e0d8`). The
shot's (kind 1): its first update sets the wait, 60, or 40 when the
holder is dunking (bit 7, `FUN_0016e9d0`); at 0 it asks the replay manager
for the last 90 frames at 0.8125 (`FUN_00328fc0`), the next update the
manager is not playing for them again at 0.53125, the next posts its end
(`0x3c6880`: slot 65 empties the meter, the camera's +0x20c turns the
gamebreaker camera off). A staged Level 2's (kind 0) starts its script,
then ends the update after the script manager's state leaves 3..7. The
manager's replays suspend the gameplay's task groups, so in the game's
updates the end always comes 63 after the press; in a lesson too (the
manager's shown-frame counts jump to 109 and 166 between two game updates,
which this pass took for both replays run inside one update, nothing
shown; the wall clock shows them played: corrected 2026-10-20, "The staged
Level 2's rerun as the game's replay" below). The School's captures: the Gamebreaker
lesson's task waited 60, asked twice and ended 63 updates after the press,
the meter emptied that update, 42 before the ball reached the rim; the
Level 2's ended the update after its script
(`the_gamebreaker_tasks_end_as_fun_00144ef8`). Ported: `World`'s
`gamebreaker_task` (snapshot 58) ends the meter 62 steps after the press's
(the task's first update is in the press's step); the simulation no longer
ends it at the next take or restart. `street-game`'s tape asked for the
replays only where it took them to show (not in a lesson, nor online
then); since 2026-10-20 the task asks for them in every match, the
online match running them on every machine.

**The cameras.** The camera manager's +0x40 (the gamebreaker camera) was
on from the update after the press to the task's end, and +0x50 (the
celebration's countdown) read 119 the update after the basket and ran down
through the restart (no inbound came before the lesson ended): the
rewrite's `gamebreaker_on`/`gamebreaker_off` and `basket` keep the same
spans (the celebration itself not in a lesson). A played match's replays
shown on these cameras are still not captured.

**The Level 2 run.** The Level 2 lesson's run played the disc's script 9 (family 2 by
the presser's spot), its roles bound as the plan's kind binds them: the
capture's six follow the plan's authored track at a mean floor distance
of 0.22 (worst 1.33: the native playback's root motion against the
authored keys), its frame 0 two updates after the press (the gamebreaker
task's first update starts the script, `FUN_00144ef8` state 0 and
`FUN_00335e90`; the script manager poses it the update after), the
freeze at the plan's frame 100 for its 60 updates outside the game's count
(the capture shows the manager's state 7 for one update there), the
script's stop at 197
(`the_schools_level_2_run_plays_a_disc_plan_from_two_updates_after_the_press`,
`street-game`). The simulation posed frame 0 in the press's own step; it
now waits two updates (`World::staged_start`, snapshot 58), play going on
meanwhile. One plan of sixteen has run; the others still want captures.

**The School's flow** (the second audit's 3.1 and 5). The original's: Play It,
Street School, User Setup (the name, the School panel's lessons attempted
and completed), the loading's legend card, the controls cards (Game Mode
Street School, Instructor Stretch Monroe), a welcome page, then one game
for every lesson: the Lakers (the person on side 1) against the Nets on
the School's court, held under an in-game menu ("N/26 Lessons Complete",
tabs Offense 0..6 and 14, Defense 7..11, Advanced 12, 13 and 15..25, three
rows with ticks, the lesson's title and description, or its tip once
passed, "X Play"/"X Replay"), and the extra tutorial pages (the meter, the
shot clock, turbo, trick points: `FUN_001eedb0`'s nibbles 26..32) holding
the game until Cross. The rewrite's: the front end's School page lists
the 26 in index order and each lesson is a match of its own (the person's
crew on side 0 against the generic ballers at Rucker, 99 to win). The
lesson's rules, launch, delays and completion are the original's; the
game around them is not: the menu, the tutorial pages, the teams, the
court, the one game (its first pass the only one flying 40, its
generator's draws), the person's side. Left for a pass of its own.

## The move rows past the dunk table: the HUD's crash and the lookups (2026-10-19)

The person's presses (7e519b9) and the alley-oop leap gave
`Action::Dunking` and `Action::Layup` rows past `MOVE_ROWS` (99 rows): the
two leaps 99 and 100, the board's lay-up 101 (`BOARD_LAYUP_ROW`), the ten
mid-air lay-ups 102..111 (`MIDAIR_ROWS`) and the putback's `NO_MOVE_ROW`
(255), all resolved by `timing::move_row`. The HUD's per-player loop
(`hud.rs`) still indexed `MOVE_ROWS[row]`, so a scripted `--quick
--play-to 2 --speed 3` game (seed 2002) panicked at update 4855, when the
computer's player 4 turned a dunk into `dunk_to_layup_4` (row 106). The
loop is `HudState::note_moves` now, on `timing::move_row`
(`moves_past_the_dunk_table_pop_the_originals_names` panicked before the
fix at row 102).

**What the pop-ups read.** The basket's handler (`FUN_002e2568`, the
listener's slot 26) picks the pop-up by the ball's kind: 3 an alley-oop
(pop-up 6), 4 a dunk (5, or 13 the tip slam when the scorer's +0x1e4 is
1, `0x2e2784`), 5 a lay-up (4), else the jumpers (3, or 2 for a
three); once the game is won (the game's vt+0xbc) kinds 3 and 4 pop 14,
the busted rim, unless `FUN_001e0bc0()`+0xe8 is 1. `FUN_002b87c0` names
a dunk by the ball's shooter's (+0xf8) +0x1e8 in `0x3fb630` and a lay-up
by his +0x26c in `0x3fb720`. +0x26c is written in four places: the
actor's reset (`0x15693c`, 0), the shoot press's move (`FUN_001693b8`,
`0x169c30`, the row's id), the board's lay-up (`0x16c2a0`, 5) and the
mid-air lay-up (`FUN_0016b1e8`, `0x16b2dc`): word 8 of its `0x3d8f60`
entry (`lw v0, 8(s0)` at `0x16b2d4`), 0 in all ten, in the executable and
in save-state RAM. The entries' word 4 (0x14, 0x15, 0x16, where the
lay-up names 20..22 "transition under", "transition 2 hand" and
"transition shot" would fit) is read by no code: the table's one
reference is `0x16b29c`. So the original names every mid-air lay-up
"fundamental" (name 0). `EXTRA_ROWS` carried word 4 as the rows' id and
now carry word 8 (`the_dunk_and_layup_rows_are_the_candidate_table`
reads both words). `FUN_0016b1e8` writes +0x1e4 5 (`0x16b2cc`; pads-1,
passes-1, passes-2 and school-5 show 5 on actors inside those scripts
with the lay-up or rebound bit) and the shoot press writes 9 at its
entry, which the board's branch keeps; `EXTRA_ROWS` keep `family` 2, the
rewrite's mark of a lay-up. The putback (`FUN_00169c98`, not ported: the
sim's own `DunkStyle::Putback` with no row) writes +0x1e4 from the record
it picks (`0x3d8410` + 0x120 k), which the rewrite's scoring takes as 1,
a tip slam; the HUD now pops kind 13 for it ("tip slam", 2,000) as the
original does for +0x1e4 1, and keeps the last dunk id. A leap's dunk id
is 24.

**The other lookups by a stored row or script id**, across the
workspace:

- `channel::Programs::compile` (street-game) compiled the dunk table's
  and the leaps' scripts but not `EXTRA_ROWS`', so the board's and the
  mid-air lay-ups had no program and the display fell back on the gait.
  They are compiled now (`the_lay_ups_past_the_dunk_table_play_their_scripts`
  through `World::step`, failing without it; the callback-21 check covers
  every row `move_row` names).
- `validation.rs` checked every scripted action's id but
  `Action::Shooting`'s script, which `World::check_pump_fake` indexes
  (`SHOT_SCRIPTS[script]`), so a peer's snapshot could carry one past the
  table. It is rejected now (`nested_references_are_rejected_during_snapshot_decode`).
- `Action::Dunking`'s and `Action::Layup`'s docs (`player.rs`) said their
  rows were `MOVE_ROWS`'; they point at `timing::move_row` now.
- Safe as they are: `shooting.rs`'s `MOVE_ROWS[row]` after `press_move`
  (both its rows are `MOVE_ROWS`': the slot dunk's position in it and
  `press_candidates`' walk of it); `touch.rs`'s `layup_reversed`
  (`MOVE_ROWS.get`: the original's test needs +0x1e4 2, which neither
  extra start writes); the pass rows by a windup's row (always drawn from
  `PASS_ROWS`; `NO_PASS_ROW` is only `Action::Passing`'s, whose readers
  check it); the catch rows (`NO_CATCH` is checked before `catch_hold`,
  and `start_catching` bounds-checks); the dive, tip-off, rebound, block,
  swipe, trick and reaction scripts (validated, and read with `get` or
  clamped). The search's other hits (menus, story pages, the HUD's layout
  by side and pad, scoring kinds, combo patterns) index values computed
  in place, not stored ids.

**Runs.** The fixed build plays the same quick game past update 4855 to
its end (2-0, The Rundown at 110 s) and a `--demo` match to its end (29
baskets; a mid-air lay-up at update 22992, blocked). Street School's
lessons cannot be played by `--keys`, which taps each key for one frame
(no held stick, turbo or shoot); the School captures' mid-air lay-ups run
through the simulation in the played-capture tests.

**Leads, not done here:** an advanced alley-oop's finish (ball kind 4)
pops the dunk's name in the original, the alley-oop's in the HUD; the
busted rim (pop-up 14) is never popped; the alley-oop's name is
`FUN_00171e28`'s pick in the original, always name 0 in the HUD; and
`FUN_0016b1e8` clears the lay-up bit 13 out of a rebound jump
(`FUN_0016e9f0`), which may make a tip-in's ball kind a shot's (not
checked).

## The staged Level 2's rerun as the game's replay, and the online pass's last gaps (2026-10-20)

The second audit's item 6 left two player-visible differences online,
which Vj ruled (2026-10-20) are to be fixed: the staged Level 2
gamebreaker's slowed rerun, which played on the match intro's real-time
clock (`intro::replay_gamebreaker`, any button skipping it) and so not
online, and the Rundown's team names, laid out differently by the
match's creator and a joiner. Also asked: the anti-cheat's check of the
staged players' scales and width ratios, and Select from the user's own
pad on three screens.

**The replay command** (category 1, dispatch 1, opcode 0). The script
manager walks the script's records in the disc's order each update
(`FUN_00336628`); a command record dispatches through `FUN_003355c8`,
which sends opcode 0 only at the record's frame plus its span: the
replay command's span ends on the script's last frame (every one of the
sixteen gamebreaker scripts: 185, 192, 209, 210, 119, 166, 110, 162,
230, 197, 225, 212, 225, 210, 220, 182), and its record comes before the
stop's, so it dispatches first. Its handler `FUN_00334b30`, unless
`FUN_00334c20` refuses (the script manager already in state 5, the replay
manager in state 2 or 3, a span of 301 or more):

- `FUN_00337718(scripts, word 8)`: the command's camera (0..9, else 0);
- `FUN_00336188(scripts, record, word 12 + 1, percent)`: the script
  manager to state 5, `+0x5c` and `+0x60` at the record's frame (the
  span's first), `+0x64` the percent (word 16, 25..150, else 100),
  `+0x6c` the replays it counts (every gamebreaker script's word 12 is 0:
  one), its cameras set up at that frame; it prints "Replay...";
- `FUN_00328fc0(1.0, manager, span, 0, 7)`: the last `span` recorded
  updates, now, mode 7. Its own speed goes nowhere: `FUN_003299c8`
  returns while the script manager's state is 3..7, so no slow-motion
  post (`0x3c6830`) either;
- `FUN_00329ac8(percent / 100, manager, 0)`: the speed and its target set
  outright (a `div.s` by 100: 0.4 for the eight dunks, 0.5 for the four
  lay-ups' and four shots').

The script returns before its frame count moves on. The replay manager
(priority 14, after the script manager's 4) takes the request in the same
update: it records that update, suspends the gameplay (`0x401b2`) and
plays. Each later update the script manager, in state 5
(`FUN_00336868`), while the replay manager's state is 2 or 3, points its
cameras at `+0x5c`, runs any speed command (opcode 1) due at that frame
(none in the sixteen) and moves `+0x60` on by `+0x64 / 100`, `+0x5c`
its truncation. At the replay's end (mode 7) the game listener
(`FUN_00142648`) calls `FUN_003362a0`: the script manager back to state 3
(`+0x54`), the command spent (its record's count), no restart snapped;
the script's next update finds the stop at the same frame and ends it
(`FUN_00335f48`, `FUN_00337250`: the groups `0x40010` and `0x1180`
resumed, the end posted). The other listeners of the replay's start and
end leave mode 7 alone: the camera (`FUN_003030c0`, `FUN_00303248`
return), the HUD (`FUN_002e2930`, `FUN_002e2990`: modes 5 and 7), and
the audio's break (`FUN_001a5410`, `FUN_001a54d8`) runs only while no
gamebreaker task runs (`FUN_00140ff8`), which during a gamebreaker it
always does; the director's end (`0x324cb8`) clears its kind.

**No skip.** The cancel press (`FUN_00130b88`) stops a running script
only when the game's mode isn't 5 and the script's kind (its header word
4, `FUN_00337af0`) isn't 1. Every gamebreaker script is kind 1 (the
intros 0), so nothing skips the rerun; the replay's own cancel is for an
offered replay, which mode 7 isn't.

**The freeze holds the replays too.** The freeze command (opcode 2,
`FUN_00336340`) suspends the groups `0x1382` when its word 12 is 1, else
`0x382`: with `0x1000`, the replay manager and the gamebreaker task, which
then record nothing while the world stands. Fifteen of the sixteen set
it; `GamSR1`'s word is 0, so its freeze's updates are in the ring and its
rerun shows them.

**Checked against the School.** The School's Level 2 run (the capture
`street-played-school-2`, its script found by the script manager's
pointer, `+0x34` less the bank's `+0x30`: 19,960, `GamLR2`, span 40..197
at 50%): in the update its script reached 197 its state went to 5; the
next kept sample shows the replay over, the script manager in state 5
with state 3 waiting, `+0x5c` at 196, the replay manager recording again
with its shown count (`+0x3c`) 311, speed 0.5 and frames left 0.5. A
manager asked as the simulation asks it leaves the same, bit for bit, and
the script manager's clock the same 196; the wall clock between the two
samples was 5.209 s, the model's 313 updates 5.222 s
(`the_staged_level_2s_replay_command_plays_as_the_games_replay`). The
same captures correct an earlier reading: in a lesson the manager doesn't
run a replay "inside one update, nothing shown". The capture tool keeps
no sample while only the replay manager moves, so the counts jumped, but
the wall clock ran 1.852 s and 2.808 s for the gamebreaker task's two
(the model's 111 and 168 updates: 1.852 and 2.803 s) and 5.209 s for
the staged one: a lesson shows them as any game does
(`a_lessons_gamebreaker_replays_play_on_the_wall_clock`). The game's
counter holds through them, so every count by game updates stands.

**In the rewrite.** The simulation's script ends at its stop as before
(the game's update never runs between the dispatch and the end) and,
where the plan's replay command ends on that frame, posts
`Event::StagedRerun` (script, mirrored, the span, the percent, the
camera). `street_sim::replay::update` is the script manager's part too:
it asks the manager for the span (mode 7, `Manager::set_speed_now`),
keeps the script manager's replay (`StagedReplay`: the camera, `+0x5c`,
`+0x60`, stepped before the manager's update while it plays) and drops
it at the replay's end; it holds the manager and the task out of a
freeze whose `Hold::holds_replays` is set. The game draws the recorded
updates as for any replay (`tape.rs`), on the script's own camera at the
replay frame (`view::step_camera`), the camera's replay handlers and the
HUD's banner left alone, no replay sound. It plays for every match: a
match alone, online (the online `Match` steps the replays on every
machine, so it holds every machine's world alike) and in a lesson. The
intro's own clock plays only the match intros now. Snapshot VERSION 61
(the event), the staged plans' format 8 (the freeze's flag, in the
disc's digest), protocol 13.

The script's end is an update of its own: the School's sample after the
replay shows the script manager in state 5 with state 3 waiting, the next
its script ended (state 1) with the game's counter unchanged, and the one
after the counter moved on with the gamebreaker task's end. So the
replays hold the gameplay for that update too (`Replays::holds_world`,
`StagedReplay::ending`; the manager records it as any update it isn't
playing), and the staged task's end post (the banner down, the HUD back)
comes with the game update after it.

**Found on the way, fixed.**

- A staged Level 2 also started the replays' task of a shot's
  gamebreaker: the script starts two updates after the press
  (`World::staged_start`), and the task's test only looked for a running
  sequence, so the shot's two replays (mode 5, at speed 1 since the
  script gates their speed) played in the middle of the script. The
  original's task (`FUN_001422f8`) takes the message's kind; a staged
  one (kind 0, the simulation's `street::GamebreakerTask`) asks for no
  replays. The replays' gates count the simulation's task as a running
  gamebreaker task (`FUN_00140ff8`).
- A staged Level 2's banner (the GB2 bands) and hidden HUD were taken
  down by that task's end, mid-script; slot 65 is the staged task's end
  too, the game update after its script's (`hud::staged_task_end`).
- The replays' sound: the transition, the loop and the speech's pause
  (slots 58 and 59) played for the gamebreaker task's replays; they wait
  for no gamebreaker task to run (`FUN_00140ff8`), so modes 5 and 7 pass
  in silence (`sfx.rs`).

**The Rundown's names.** The creator of an online match plays the match
it loaded alone, whose names are its records' nicknames, while the
match's looks carried each side's full name ("Philadelphia 76ers"),
which a joiner took for the nickname: its Rundown drew the full name on
one line and no city, the creator's the city over the name as a match
alone does. The original's Rundown (`FUN_002c7b48`) fills the team's
city (`FUN_001ed7d0`) into element 2 and its nickname (`FUN_001ed890`)
into element 10. The looks now carry each side's city and nickname
(`LOOK 3`) and every machine names the sides with `setup::team_names`
(`TeamNames`' nicknames and cities).

**The anti-cheat** (`street_net::roster`) checks the staged players'
scales and width ratios against the line-up's records beside the gait
profiles: the height scale (`0x3153d0`) halved for every player alike by
the tiny players' code (`FUN_003153d0`), and the width over the height,
bit for bit; a record not known there must have a legal model's.

**Select from the user's own pad.** Be A Legend's map, an undecided
bracket and Choose Region open their question only for the user's pad
(the game data's `+0xd4`): the pad seated for the run (`select_asks`).

**Online, measured.** The disc determinism test's second match plays two
staged reruns (`GamDN4b` at 40%, `GamSR3` at 50%) at the same match ticks
on the referee and both clients, the world's clock held through them,
every confirmed checksum the referee's. Two games on one PC over
127.0.0.1, the computer at both pads, the creator's home side starting
with a Level 2 ready: hosted, the staged rerun (a dunk's, mode 7 at 0.4)
started at match tick 1863 on both, both windows showing the same
recorded frames on the script's cameras, the banner down after it on
both, the same confirmed events, no resync, the same Rundown with
"Philadelphia" over "76ers" and "Los Angeles" over "Lakers" on both;
through `street-server --vol2` the same (a shot's, mode 7 at 0.5, match
tick 913 on both, the same 25 confirmed events, no resync, the same
Rundown). The determinism test's second match, on the final build, plays
its two staged reruns at match ticks 597 and 1651 on the referee and both
clients. Evidence: `$EVIDENCE/street-online-rerun` (INDEX.md).

## Be A Legend's rules and the other games' computer (2026-10-21)

The second audit's item 7 (branch `legend-rules`), with the two leads Vj folded into it.

**Choose Region's keys.** The screen (class 0x29) takes no key but the user's pad (the game data's `+0xd4`, as
the select-questions pass measured it). The menu's input now listens to the seated user's controller alone on
Choose Region, every key (`menu::listens_to`, `MenuInput`'s reads), any controller when nobody is seated.

**The staged cameras** (the "sky" on long shots' reruns). Measured in the project's pcsx2-runtime on the Street
School's Level 2 lesson (save state 41; the plan the lesson reaches is GamLR2, script offset 19,960): a probe of the
script manager (game+0x424), the camera manager's staged helper (manager +0x24, vtable `0x498848`), its ten script
camera nodes (helper +0x1c0.., `FUN_003318b0`) and the renderer's projection object (manager +0x34 -> wrapper ->
`0x8d6980`), one line per change, with PrintWindow grabs every ten replay frames
(`$EVIDENCE/street-legend-rules/rerun-camera`).

- The node (`FUN_00331d08`, its record set `FUN_00331968` and key setter `FUN_00331a70`) holds the camera track
  as `Timeline::camera` reads it: over the rerun's replay frames 40..196 (camera 3, the command's word 8) the eye
  and target agree within 0.1 (the keys' last frames a half frame apart). The staged helper's pose callback
  (`0x3010f8`) aims from the eye at the target, as the rewrite does; its update (`0x301598`) is empty.
- The lens: the node stores `tan(fov / 2)` in the helper's `+0x98` (`0x3327f8..0x33295c`; a field of 0 gives
  1.0, `0x332698`; a field outside 1..180 leaves the last lens), and the renderer's projection reads it as any
  helper's: zoom 1.3, so a field of 50 shows 63.74 degrees across (projection +0x110) and 62.4 down (matrix
  1.2372 / -1.6497). The rewrite took the field for a 4:3 horizontal angle, 38.5 degrees down: looking up at a
  long shot's ball (pitch +22 in GamLR2, +32 in GamLR3), the narrow view held only the buildings. The original's
  grab at replay frame 140 shows the ball at the top and the players at the bottom.
- The roll: `+0x1b0 = (heading - 180) * 0.017453292` (`0x332960..`); `FUN_003046c0` builds the view as T(-eye),
  Ry(-yaw), Rx(-pitch), Rz(-roll), Ry(180): the camera's up leans toward its right by the roll. At replay frame 45
  (heading 189.55) the view matrix's columns are (-0.98, -0.166, -0.110) across and (-0.160, 0.985, -0.063) up,
  which the rewrite's camera now reproduces (`rotate_local_z(-roll)`). A mirrored script negates only the eye's
  and target's x (`FUN_00331968`); the roll stays.
- Following cameras: a record with byte `+0x23` set (node +0x43; every freeze's camera 1) aims at its script
  actor (node +0x40; -1 the ball) at the actor's root x/z and the height of bone 0, bone 21 or the floor (+0x45),
  the record's target x then a sideways offset (the yaw less `asin(x / 10)`, the reach `sqrt(h^2 + x^2)`) and its
  y a height over the point. In the same run the freeze (state 7) aimed at the jumper bound to slot 1 (yaw
  0.9258 from (-20.6, 4.2, 7.0) to (-5.25, 1.5, 18.6)); the rewrite had aimed at the court's origin.

`view::staged_pose` and `StagedView`; `ActiveCamera::roll`; every staged script's camera (plays, freezes, reruns,
the court intros).

**Be A Legend's rules.** `FUN_001e91a0` stages every event's game: the rules put back (`FUN_001ded20`: game type
0, the shot clock's row 2, to 21, no spots), then for a street challenge (kind 2) its row of `0x460970` by the
event row's byte `+0x1b` (`FUN_001dbd48` the type, `FUN_001dbd60` the clock, `FUN_001dbd80(.., 0, ..)` side 0's
spot, `FUN_001dbd78` the target): Old School to 12 (Soul in the Hole), the Trick Point Challenge to 250,000 (Broad
Street), No Gamebreakers to 21 (Greenlake), Dunks Only to 12 on the 12-second clock (Foster Beach), Gamebreaker
Wins to a Level 2 (Mosswood: the target is the level less one, which `FUN_001457f8` copies to game+0xf8), league
scoring to 50 (Lincoln College), Old School to 21 (Rucker Park '78). A pick-up and a tournament play the put-back
rules: no boss brings his own. The difficulty is the setup's options word (`+300 & 3`), the player's: the
Difficulty screen is on Be A Legend's way in (class 0x69, mode 5 to 0x36). The rewrite had the tournaments at a
difficulty by their row's level, the street challenges on Legendary and Mosswood to a Level 1. The tournaments' and
the street challenges' opponents were already `FUN_001dbe30`'s (`lineup.rs`, "Who plays for a side nobody holds");
the rewrite's own crew pick (`story::crew_lineup`), left naming the text bracket's crews, is gone: they are "Team
%1d" as on the bracket's page. `street_sim::story::event_rules`, `STREET_RULES`, `Event::row`;
`Rules::from_setup`, `CLOCKS`.

**The League Challenge's rules.** Play It's constructor (`FUN_002beef8`) puts the setup back (`FUN_001de450` ->
`FUN_001ded20`), and nothing on the League Challenge's way sets the rules (`FUN_0026bdc8` sets the opponent and the
court; only My Rules, `FUN_002c8d08`, and the rematch call the setters): its games are Normal to 21. The rewrite
played league scoring.

**The story modes' weight for the computer.** `FUN_00140a30`, the skill blend every computer decision reads,
recomputes its final weight in the game record's modes 4 and 5 (`0x140b90..`) as `FUN_00143458` does for the
meters: the League Challenge's odds table, Be A Legend's star tier, event level, bracket round and an opponent past
team 39; the inner mixes stay the difficulty's. `World::scenario` (snapshot 62).

**The game object's class.** The game's factory (`FUN_002f3000`) builds the lesson game (vtable `0x441680`,
game+0x9c 4) for Street School, the trick-point game (`0x440ce8`) for type 1, Gamebreaker Wins (`0x43e9d0`) for
type 6 and the points game (`0x43b490`) for the rest. Their virtuals differ at +0x44 (the catch-up cell), +0x7c,
+0x84, +0x8c, +0x94/+0x9c (the score), +0xa4, +0xbc (won), +0xc4, +0xcc and +0xec (the init):

| | points `0x43b490` | trick points `0x440ce8` | Gamebreaker Wins `0x43e9d0` | lesson `0x441680` |
| --- | --- | --- | --- | --- |
| +0x7c would a basket win | `FUN_0012e890`: the points table, a side at game+0xf8, two apart | `FUN_0018c698`: the side's trick points past the target and the other side's | `FUN_00145990`: a Level 1 to a Level 1 game, a Level 2 to either | `0x190b88`: never |
| +0x84 a basket's points | `FUN_0012e998`: `0x43b618` | `0x18c758`: none | `0x145a10`: 0, 0 | `FUN_0012e998` |
| +0x9c the score | points (stat 0) | trick points (stat 0xb) | trick points | points |
| +0xbc won | `FUN_0012e800` | `FUN_0018c620`: a side at the target and game+0x4b8 | `FUN_00145940`: game+0x4b8/+0x4bc by the target | `0x190b80`: never |
| +0xc4 pocket now | `FUN_0012ebf0` | `FUN_0018c8e0` | `FUN_00145d28` | `FUN_0012ebf0` |
| +0xcc cancel now | `FUN_0012ed28` | `FUN_0018ca20` | `FUN_00145e70` | `FUN_0012ed28` |

- The points table (`0x43b618`, 8 types x 3 levels x 3 kinds, the scorer's and the other side's): Normal 1 a
  dunk, 1 inside, 2 from beyond the arc; a Level 1 takes 1; a Level 2 scores 2 and takes 3 (4 from beyond the
  arc); league scoring 2, 2, 3, a Level 1 taking 2, a Level 2 scoring 3 and taking 4 (7); Old School 1 everywhere,
  a Level 2 2 and 3; Dunks Only only the dunk's column; No Gamebreakers no gamebreaker column; the trick-point and
  Gamebreaker Wins rows all 0. The kind (`FUN_00187c10`): 0 a dunk or alley-oop (the ball's kind 3 or 4), 2 for
  the event's base 3, else 1; the level the ball's +0x178, which the basket's post (`FUN_001515a8`, synchronous
  through `FUN_00190d90`) clears only after its listeners. The singleton's +0x70 (a long basket as an inside one)
  is never written. The School's staged Level 2 (GamLR2) scored 20 to 22.
- The trick-point game's listener (`0x440a40`, slot 26 `FUN_0018c450`, priority 4: after the combo matcher 1,
  the meters 2 and the statistics 3 have paid the basket's trick points) marks the game (+0x4b8) when the
  scorer's side is past the target and the other side's; its +0xbc then ends it. Gamebreaker Wins' listener
  (`0x43e728`, slot 26 `0x1456f0`, priority 0) marks +0x4b8 for a Level 1 basket and +0x4bc for a Level 2.
- The pocket checks (`FUN_0012ebf0`, `FUN_0018c8e0`): with a pocket possible (`FUN_00141098`: a full meter, no
  gamebreaker ball (`FUN_00141018`, ball+0x178), the side on offence with the ball held, none pocketed, not
  `FUN_00140960`, not game type 3, in a lesson only lesson 0x17), one draw; pocket when both sides' scores
  (+0x9c) with the asking side's gain between its last two meter fills (stats +0x9e8 less +0x9e4; slot 66
  `FUN_001883f0` keeps them: the trick points in type 1, else +0x9c) and a margin (`r % 3 + 2`; the trick-point
  game's `r % 75000 + 25000`) stay under the target. Gamebreaker Wins (`FUN_00145d28`), only to a Level 2: a
  pocket possible pockets unless a cancel is (`FUN_00141190`), then one draw against the other side's meter in
  percent (`0x3d4120`: 100 under 10, 75 under 20, 50 under 30, else 0), else game+0x4c0 for the cancel
  (`FUN_00145e70`). The others' cancel: once after the other side pockets (game+0xfc), 8%.
- The subclass's keep-away (`FUN_00124fb8`): in Gamebreaker Wins to a Level 2, from 45,000 on its own meter, not
  full and none pocketed, keep-away flag or not.
- The alley-oop check (`FUN_00129d98`): only the Alley-Oop lesson (vt+0xb4 5) in a lesson; never with a
  gamebreaker ball in play.
- The trick-point game's pass check (`FUN_00183c78`): pass while the side has not won it (vt+0x7c).
- Callback 28 (`FUN_00172400`): in live play, no hang and no draw when an inside basket at the ball's gamebreaker
  would win.

Not identified: the side object's kind 0x2c (`*(game + 0x184 + side*4) + 0x40`, its type at vt+0x14) that the
points and trick-point pocket and cancel checks test; one of a table of type getters at `0x1b0960..0x1b099c`.
Read as never. (Identified in "Item 7's leftovers and the second audit's item 8": the side's team, 0x2c Team
Whitewater; the meter spots ported there too.) Not ported: Gamebreaker Wins' meter spots (`FUN_001457f8`: setup +0xf8 to side 0's meter, +0xf4 to
side 1's, 100 and over a pocketed one, the rest times 500), which My Rules edits.

`street_sim::game_class` (`Class`, `POINTS`, `points`, `World::class`, `class_score`, `basket_would_win`,
`gamebreaker_ball`); `Basket::Gamebreaker::dunk` (a staged Level 2 dunk's); `Stats::fills`; the AI's
`gb_cancel`. Snapshot 64, protocol 14.

**The story capture.** In the pcsx2-runtime, from save state 3 (a career at Update Team after its first NYC Rec
Center win; No, no card save, the map, NYC Rec Center): `%TEMP%/street-played-legend-1.jsonl`
(`tools/pine_played_capture.py` through the driver, 45,160 updates, none torn; the person on side 1 tapping Cross
every 1.2 s, the computer winning 21 to 7). The setup in the game: type 0, the clock's row 2, to 21, no spots,
the options word 0x102c (Got Game), the meters' blends at each pair's first value (the story weight 0: the star's
tier 0, the event's level 0). The opponents, 213, 208 and 224, are `FUN_001dbe30`'s draw from the timer's 113,415
(the capture's shared generator walks back to that reseed; the reads around the map's press were 113,367 and
113,430) for the crew the career's list held then, 245, 204 and 227 (game data +0x15ee): the fourth man, 198, in
the list before the first game (save states 4 and 7), had left it after (save states 3 and 10). The rewrite's crew
keeps him; when and why the original drops him is not traced. (Corrected in "Item 7's leftovers and the second
audit's item 8": states 4 and 7 are later than 3; he was recruited at Update Team.) Of the played checks, the capture fails five it was
not made for (the boot's one reseed per group, the person's switches 325 of 372, the receivers' hands, the lobs'
count, a dunk's jump within 0.87): not added to the gate's list.

## Item 7's leftovers and the second audit's item 8 (2026-10-21)

Branch `legend-leftovers-item8`: what item 7 left (Vj's ruling of 2026-10-21), then item 8 as far as it went.

**The crew's fourth man: no difference.** The save states are not in the order item 7 read them. By their files'
times and their backups (`$EVIDENCE/street-item8/crew_reads.json`): before the first game (the backups, games 0) the
career's list (`0x4bce20 + 0x6de`, game data `+0x15ee`) holds 245, 204, 227; state 10 (the results) and state 3
(Update Team after the first NYC Rec Center win, 20:53) hold the same three; state 4 (the map, 20:55) holds 198 in
slot 3 and nothing else changed. 198 was one of the first game's opponents (state 3's setup `+0x9c..`: 245, 204, 227
against 208, 189, 198): the person recruited him at Update Team (`FUN_002d6fd0` -> `FUN_001e7db8`, the row past the
team's end filling the next open slot). Nothing drops a member; the rewrite's crew (`Story::update_team`) is the
original's, and save state 4's draw (four men) and the story capture's (three, from state 3 with Update Team declined)
both stand.

**Gamebreaker Wins' meter spots.** `FUN_001457f8` (the class's vt+0xec) starts side 0's meter (game+0xc8) from setup
`+0xf8` and side 1's (+0xcc) from `+0xf4`: 100 and over a pocketed Level 1 (meter+0x20) and the rest times 500
(`FUN_001448e0`). The points and trick-point games' init (`FUN_0012e798`, `FUN_0018c5c0`) take no spot; the statistics'
constructor (`FUN_00186b10`) starts stat 0 (stat 0xb in the trick-point game; none in type 6) at `+0xf8` for side 0 and
`+0xf4` for side 1. `FUN_001dbd80(setup, k, v)` writes `+0xf4 + 4k`, so its index 0 is side 1's spot and My Rules' home
(`+0x10` 1) side 0's: nothing crosses. `Rules::from_setup` takes the spots by that index; `Rules::meter_spot`,
`start_score`; `World::new`; My Rules hands its turbo spot over.

**The story capture's checks.** None of the five failures was the simulation's:

- The replays dressed each actor with the disc's record at the capture's `player_id`, which is the record's id (field
  0), not its place: the generic ballers carry their record less 11 (the crews' 193 is record 204), the
  streetballers' ids are scattered. `disc_player` finds the record by id. The jumps' tops had been off by each
  actor's height.
- The created star (id `0x4000`, slot 0) is no disc record: his body is the uniform branch of `0x315790` at the 69 in.
  of save state 3's career copy (`0x4bce20 + 0x6ec`; field k at `+0x42 + 2(k - 3)`), `CREATED_STARS`. The 78 in.
  stand-in had found 11 of his catches an update early.
- A story game reseeds at its opponents' draw (`FUN_001dbe30` at the map's press): a boot of its own, 51 draws to
  update 0, its pair the last two.
- The switch check: `FUN_00171920`'s 20 (below) reorders only two teammates under 20 apart, so a switch the rule
  decides alone (76 of 78 in the 23 captures, the 2 known) must be exact, and of the 294 the camera may decide 249
  went to the nearer teammate. The story game's person, switching all through his defence, meets the camera's term in
  a sixth of his switches; the old 90% bound held for the others.
- Each match draws its own registry order (save state 4's first NYC game ran 3, 4, 0, 5, 2, 1; the other states' in
  matches: 1 and 2 5, 0, 4, 3, 1, 2; 9 4, 0, 5, 3, 1, 2; 12 4, 2, 5, 3, 0, 1; 13 2, 0, 1, 4, 5, 3; 41 1, 5, 3, 0,
  2, 4). Outside save state 1's match the lob check takes, for a flight set by the leaper's span, the order the flight
  shows, all of a match's lobs on one order of the six: the story game's nine fit 3, 4, 5. No fixed spacing of the
  five inserts after the reseed fits states 1, 4 and 13, so the construction's draws are still not placed.

The capture joins the gate's played list: 35 checks pass on the 23 captures.

**The kind 0x2c is Team Whitewater.** The object the points, lesson and trick-point games' pocket and cancel checks
test (`*(game + 0x184 + side*4) + 0x40`, its vtable at +0x30) is the side's team record (`0x461030`, the one
`FUN_001ebc88` builds), and vt+0x14 (`FUN_001ecdb8`) its index across the team tables: the save states' matches read 39
for Pick Up's sides, 36 and 49 (the career's own record) in a story game, two league teams in the attract match. 0x2c is
database team 44, Team Whitewater (event 5's boss): its side never pockets (the test comes before `FUN_00141098` and
the draw) and always cancels once the other side pockets (the 8% draw still made). Gamebreaker Wins tests no team.
`World::side_teams`, from the match setup's teams.

**The switch's off-screen term.** `FUN_00171920` (raw `0x171920..0x1719ec`) projects the actor's point, his feet less
2 (`x, +0x260 - 2, z`), through the active camera (`FUN_002ffe70`, the renderer's VU0 transform) and calls him on the
screen when it lands within `FUN_0030fa50`'s width and height in front of the near plane; the pick adds 20 to a
teammate off it. The person's client now says which actors are off its screen (`Input::off_screen`, the view word's
top six bits; `street-game`'s `controls::off_screen` on the shown camera) and `World::switch_target_seen` adds the 20.

**The setup's slots and the game's sides.** The game builds side g from the setup's slot g ^ 1 (`FUN_00185138` reads
`config + 0x9c + (side ^ 1) * 0xc`): every save state's match holds slot 0's lineup (`+0x9c`) on side 1. Choose Team's
left team is slot 0 (the rewrite's side 1, already) and the School puts the person's Lakers in slot 0 (side 1,
already). But Be A Legend gives the career slot 0 (`FUN_001e91a0`: `FUN_001dbb20(setup, 0, career+0xca0)`), the league
Challenge its team (`FUN_0026f958`), and the attract its first team drawn: their person plays side 1 (save states 4
and 10: game+0x188 the career's team 49, the person actor 3, the crew attacking -x first), where the rewrite played
side 0. `story::CAREER_SIDE`.

**Restarts.** The original's short restarts (80..92 updates, all six ready at 53..56) are the instant replays: the
director asks for one 54 updates after the basket, the replay suspends the gameplay (the update counter stands) and its
end snaps the six to their spots (`FUN_00142648` -> `FUN_001407c8`) at the restart's update 52 in every capture (the
attract's and the played: 4 of the story game's 45, 3 of the defence capture's 21). The rewrite's game had that
(`street_sim::replay`); the attract comparison never ran the replays, hence its "never under 113". It does now
(`step_game`): the shortest restarts 85, 114 and 85 against 80, 86 and 82, their share each game's replay draws. Then:

- the phase-6 pickup: the restarting side's tail takes the loose ball as in play (`0x15c3c8`: `FUN_0016cd28`'s reach
  and the scoop, no phase test); the other side's update leaves it in phase 6 (`FUN_0015da28`'s pickup asks the phase
  not to be 6). The rewrite's reach stand-in stays only without the disc's animations;
- "loose ball" (`FUN_0010e0a0`, raw `0x10e0a0..0x10e9a8`): the run at the mode's speed level (3 is 2 while he is
  barred, `FUN_0017dea0`), the turbo at 3; a ball above his height plus 2 (`FUN_001819a8`, the record's inches / 12) is
  met where it comes down to that height, or jumped for (4.5..7 up within 3.5, or its landing 0.3..0.37 s away; the
  nearest opponent within 15 and no teammate up within 10: `FUN_0017f2e0`, `FUN_0010e9a8`); a lower one is run at;
  the dive's bounds are the ball's; a restart's stop waits out his bar and cooldown. The rewrite had its own level
  (3 beyond 6) and its reach. Not ported: the ten-second free-ball failsafe (`0x10e884..0x10e968`).

Left: in a restart without a replay the original's inbounder is not driven for about 20 updates (speed level 0, his
last clip carrying him), so it takes the ball 41..44 updates in (the ball down from the rim through the net and up off
the floor) where the rewrite's takes it about 30 in: those restarts run 111..120 updates at the shortest against
130..135.

**The tip jumpers' hands.** The tip-off script's `a1 ef` (raw `0x19537c..0x195570`) poses the channel's clip (`8e 03`:
`tipoff_jump_2` 380, `tipoff_jump_1` 384, `tipoff_jump_3` 382) at the script's frame (15, 10, 11; `FUN_0019bda8`)
where the jumper stands, turned toward the ball (`FUN_0016f2b0`), and takes the right hand (bone 0x13) at the pose's
flag 8, the left (0xf) at 4, else their midpoint; the tip (`0x15eec4`) is either hand bone within 1.25, on the clip as
its waits run it (`8d ee s 05`, then `8d ee F 19`). With the captured players' bodies the jumpers rise as the original's
(the first capture 3.77 / 4.08 against 3.75 / 3.85, the third 4.04 / 4.25 against 4.02 / 4.24) and tip within an
update (`World::tip_aim_hand`, `tip_hands`).

**Left of item 8.** The block jumps at a shooter in the second and third matches and the pass out's rate wait on who
shoots (item 4's lead). The rebound jumpers' hands: callback 36 (`FUN_00175858`) poses the channel's clip at the
script's frame facing his wanted facing (+0xb0) and takes `FUN_0014da20`'s point (flag 0) by the frame's hands as
`dribble::in_hand` does, and the catch (`FUN_0016ed18`) the hand points on the clip each update; but thirteen of the
seventeen scripts branch (`84 2f`, the goaltend pair `84 30`) to their clip by a condition not read, and the sim does
not step their clip.

Snapshot VERSION 67 (the sides' teams, the inputs' off-screen bits, the loose ball's words), protocol 17; the disc
digest unchanged.


## Interrupted gameplay pass recovered; restart control and rebound presentation (2026-10-22)

Recovery branch `who-shoots-item9`, integrated with the PC fixes from `main` (`cb61d9b`).
The final code checkpoint is `fe8f388`. The original four saved commits were retained:
`0fc591d` (the committed drive's blocked-dunk shoot tap), `e602628` (the loose-ball
behaviour's ten-second failsafe), `dd9acba` (a basket bars all six for 40 updates and
consumes its face draws; bars count down after actor tests), and `5a610b0` (rebound
scripts' callback-47 branches, callback-6 facing and posed-hand aim/catch).

### Each actor's AI runs in that actor's update

Recovered change `6979620`: `FUN_001587d8` calls the AI player's frame
(`FUN_0013a970`) at the start of its own actor update. The process registry
(`FUN_0033ce00`, front to back) runs the AIGame and AITeams before the six actors;
`FUN_0033cf98` places a newly constructed actor randomly among the existing actors.
`World::ai_begin` now runs the counters/strategies and `World::ai_player` runs each
actor's frame in `World::update_order`, so a defender after the shooter observes
that update's release. A defender before him sees it on the next update. The
production disc regression exercises both positions in the order. The snapshot
validates and carries a permutation of the six players.

The retained attract comparisons infer constraints from same-update and next-update
block jumps, preserving the old order only for unconstrained pairs. The played
adapter takes save state 1's observed order. The construction's five selected draw
offsets fit that one save state, not every match: this is not an RNG-exact match
construction claim. The broad `actor_update_order` and AI-decision obligations stay
known differences until construction and the full registry order are paired.

The recovered rebound branch predicate agrees with 8 of 10 captured applicable
choices (5/7, 1/1, 2/2); its script records/waits are checked against the disc.
That does not close the whole rebound-collision obligation. Pass-out rate, the
teammate backboard flight and the remaining played camera/Level 2 captures stay open.

### A restart handoff must preserve the inbound walks

New fix `2ddad29`. Re-read the saved original `FUN_0018a708`: when a player changes
to computer control (controller 4), it clears his behaviour (`FUN_0013aa48`) only
when that actor's +0x298 is zero. `FUN_00140880` writes this field on all six;
`FUN_001406b8` sets it for a restart, as well as the opening toss's use.
`Ctx::frozen` previously represented only the opening toss. When the game's
hand-off moved the pad to the inbounder during a restart, the old person's queued
`InboundPos` was erased, so that player's ready flag never rose. A local disc-enabled
reproduction stalled on lesson 0's third launch, with the inbounder holding the
ball and the old human actor's queue empty and ready flag down.

`Ctx::frozen` now also covers `World::restarting()`. This preserves the walk while
the restart's forced control is on. The production regression
`school::tests::replaying_lessons_repeatedly_leaves_the_restart_and_keeps_control`
launches all 26 lessons three times, immediately after the inbound and after later
play, following the current human actor's control. All 78 leave the restart within
400 updates with disc poses. This exercises a concrete cause of both repeated-School
stalls and ordinary inbound hangs; other causes are not excluded by that test.

### Rebounds have their own presentation event

New fix `9143489`. The existing original take trace (`FUN_0014cb28`, above, "The
take") posts slot 17 (`0x3c6590`) only for live native ball states 0 and 6. A shot
taken by the other side posts block slot 16; a held-ball steal posts slot 15; an
ordinary take always posts catch slot 24. The simulation's box score already used
this classification, but HUD, rebound sound/crowd and nickname consumers listened
to every generic `Event::Pickup` as a rebound. A caught block's later Pickup could
therefore announce REBOUND, and made-shot recoveries could announce it too.

`World::post_rebound` now emits `Event::Rebound`, and only that event drives the
rebound popup, sound, crowd and rebound nickname. `take_ball` also publishes the
caught shot's Block event, matching the slot-16 statistic it already paid. Generic
Pickup stays available for ball/control flow. Production pickup and box-score,
HUD event-order, nickname, event validation and snapshot regressions cover the split.
This corrects the displayed/heard classification; it does not claim a previously
observed change to box-score numbers.

Snapshot 69/protocol 19 distinguish the actor-order change from Claude's earlier
snapshot 68/protocol 18 commits; snapshot 70/protocol 20 add the separate rebound
event. Game and referee builds must agree on the final protocol. The disc digest
is unchanged. No runtime installation, real memory card, upstream push or main
checkout's unrelated viewer changes was modified by this recovery.
## Teammate board-pass release and signed staged height (2026-10-22)

The teammate board route is now ported in commits 9d8b04b and a7ed1c5.
The original pass start stops a grounded passer before filtering rows
(0x15facc..0x15faec). Self passes select flag0x200000, teammate passes
flag0x400000: rows91/92 wait for callback38, rather than throwing at the press.
FUN_00173570 plans the catch and both legs at release. Its floor-route flight
uses the passer-to-board distance at S, board-to-target at0.75S, a minimum20,
and ratio0x3f5e9bd4 (raw0x161430..0x1614e4 and0x162c68..0x162d14).
The three-dimensional geometry splits that whole flight into queued legs
(0x173a40..0x173dc8); FUN_00148378 consumes the second leg at the board.
The catch uses the original board-only normal rows and faces the waypoint.
Only an actually airborne leaper selects a leap catch. There is no invented
jump at the board. FUN_00183318 posts the special route and named teammate
for computer passes too; the AI now retains that recipient and waypoint.

World::step regressions cover human stand-in/disc-pose releases and the AI
route: a_teammate_board_pass_releases_on_its_row_and_keeps_the_planned_catch
and a_computers_board_pass_to_a_teammate_keeps_its_special_route. They fail
on the previous immediate/plain throw paths. These prove the implemented
production behavior; paired native teammate flight/bone equivalence is still open.

Normal inputs to the original region-map interior points have now produced
all16 native Level2 plans, including both variants of the four dunk families.
Read-only original snapshots retain immutable record/header identities,
VM state, manager/actor bookends, channel counters and every exclusion.
The expanded producer also brackets reference positions, physical positions,
animation root displacement, orientation and launch state. Reference positions
are not physical animated roots: the renderer already applies root displacement.
An initial comparison of these different bases was retained and corrected;
it is not evidence of a production placement bug. compare_staged_paths.py
checks the production resolved-plan reference placement only, at1e-4 per axis,
using the observed native binding and shooting side. Launches and differing
reference/physical heights are outside that component comparison.

The all-plan clock checks exposed a real signed-height error. FUN_0032f2e8
calls FUN_0032bae8 to write proxy Y to the actor before FUN_0032c0a0 tests
the actor height for exact zero. The subsequent actor update can clamp it.
Sequence::update_native_clocks had clamped negative Y before the rate test,
which incorrectly changed a moving loop's configured speed. It now tests the
signed manager height. The production regression
a_staged_loop_preserves_its_rate_before_a_negative_height_is_clamped fails
at frame2 before the fix and exercises both shooting sides. The clock adapter
also corrects its team/mirror mapping and now asserts counter equality.

The initial motion-producer batch kept five failures caused by using a later
clamped height as a manager input. Its revised admission requires original
proxy Y to be zero, following the source order above, and retains the excluded
observations and old reports. This refinement does not filter model residuals.
Sampled phase/rate, scale and reference-placement checks do not prove full
actor binding, animation poses, hands, ball paths, cue/freeze/release/score ticks,
replays or played cameras. The broad Level2 obligation stays untested.
No serialized fields changed: snapshot70 and protocol20 remain current.

The retained component summary is docs/LEVEL2_COMPONENT_CHECKS_2026-10-22.json:
767/767 stateful channel frames, rates and counters across all16 plans;
767/767 own-skeleton height scales; 58/58 admitted loop-rate producer inputs
across9 plans; and 250/250 reference placements across15 plans, maximum
component error0. Reference placement has no admissible GamDN3b observation.
Clock/scale each retain11059 exclusions, motion11768, reference13766.
These counts describe separate units and domains and are not pooled into a
whole-game percentage. Seven motion captures and four reference captures have
no admissible observations. The legacy stateless clock remains479/767 frames
and709/767 rates and is not the passing model. The summary records source,
capture and report hashes without publishing the disc or native capture files.

Continuation validation on1351b19: formatting and strict workspace/all-target
lint pass; full workspace1385 passed,0 failed,6 ignored in51 suites;
played35/35 on23 captures; attract17/17 on each of3 captures; game/referee
builds and49 Python tests pass. Publish/whitespace checks are clean. A reviewed
background12-second demo exits0 with court, players, ball and HUD visible;
no original/rewrite image equivalence is claimed. Original test cards remain
byte-identical to the project's cards, and the owned test emulator is stopped.

## Passout identity must survive the return leg (2026-10-22 evening)

Commit4abf198 retains game+0x160's board/head identity until the catch.
The original stationary head press stores kind2 at0x146ab8..0x146ac4;
FUN_00142038 reads it. FUN_00143a88 (raw0x143abc..0x143ad4) uses5000/key-2
for that kind,3000/key-3 otherwise, sharing the existing four-key repeat ring
and upward-rounded halving. FUN_0014c348 sets ball+0x1ac for a queued special
independently of whether its recipient is the passer. Slot19 FUN_00131ff8
selects class17 from that flag. The meter's fill remains a flat3000; the
repeat-discounted value is the receiver's trick points and combo-slot value.

The rewrite lost head identity when its second leg became Via::Direct and
treated only self passes as specials. Passout now carries typed Board/Head
identity through both legs. World::take_ball recognizes the active passer's
special for teammate catches too. Head catches use5000/key-2, board catches
3000/key-3. The existing generic-self-throw fallback is retained as the
rewrite's approximation; this is not a claim that every native flag is ported.
Head metadata does not gain the board row's animation/catch wait privileges.

Production World::step tests first reproduced1500 instead of5000 after a
preloaded board key, and an empty combo slot instead of17/0/3000 for the
teammate board catch. They now check independent repeated head values,
identity during the return leg, cleanup, and board catches with both stand-in
and disc poses. Snapshot71/protocol21 encode the kind; exhaustive validation
and decode reject head state carrying a board return leg/tail or an unthrown
head marker. Both game and referee builds must use the matching versions.
Head release/flight geometry and press-time HUD pop-up values/timing remain
the rewrite's own; no paired native head-flight or full presentation claim.

The faster reference-only original producer also captured GamDN3b:8 admitted
placements (actors0..3, ticks64/68), all exact;424 exclusions are retained.
Combined with the previous250, sampled reference coverage now names all16
plans. LEVEL2_REFERENCE_FOLLOWUP_2026-10-22.json records the new hashes, input
domain, thin-producer scope and failed second launch. This does not close
full actor binding, pose, hand/ball, freeze/score timing or played cameras.

Complete gates on4abf198: workspace1387 passed,0 failed,6 ignored in51 suites;
played35/35 on23 captures; attract17/17 on each of3 captures; formatting,
strict workspace/all-target lint, game/referee builds and49 Python tests pass.
A reviewed background12-second demo exits0. Publish/whitespace checks remain
clean. These validations do not expand the bounded original comparison domains.

## Self head passes must wait for their row (2026-10-22 late continuation)

Production 935bd92 fixes the immediate self-directed Off the Heezay throw.
FUN_00146278 stores kind2 (0x146ab8..0x146ac4); FUN_0015f930 selects
the self-pass rows, and callback53 FUN_001741a8 releases them. The route
distance is the two floor legs, but the minimum25 applies only to abs(kind)==1
(raw0x160610..0x160680). Head kind2 can therefore select the short rows83..86.
The rewrite formerly skipped the row and threw at the button press.

Four new normal-input School lesson18 attempts retain two successes and two
failures. Three overshoulder attempts show20 updates from the first sampled
kind2 state to release. The close-row attempt shows13, against the static
table's14: this one-update discrepancy remains open. The release field is21
updates and the first-leg timer8/9. HEEZAY_RELEASE_OBSERVATIONS_2026-10-22.json
records capture hashes, source addresses, observed windows and exclusions.
Live reads remain non-atomic even with unchanged frame bookends. No memory
patches or original-save changes were used; the raw captures stay outside Git.

A self head press now stops, selects its passout row and holds the ball until
that row's callback53. Passout carries the defender (game+0x148) through the
wait and snapshots. The defensive counter's source-backed check can identify
that assigned defender during the held windup. School disables that check,
so these captures do not prove full counter timing or return-holder semantics.
Callback53's side bar (0x174b1c..0x174b68) prevents the passer's catch bit from
immediately taking the outgoing ball. The bar spans the rewrite's two head
legs; native flight geometry, total/leg timing, head-bone contact, reactions,
success probability, teammate head passes and HUD pop-ups remain incomplete.
No claim of exact Head flight or the whole special is made.

The first release-only checkpoint df95911 passed workspace1388/0/6 and
played35/35, but its full gate stopped on the first attract comparison:
counter mean23.8125 against17, unchanged bound6.5. That failure is retained.
935bd92 corrects the stale defensive holder rather than widening the bound.
FUN_0013bf60 (raw0x13bf60..0x13bfc8) reads ball+0xf4 and returns0 when it is0.
Across the three attract captures,73/47/111 self-pass windup samples retain
the passer, while all227/216/486 released-flight samples have holder0.
The earlier claim that the first73 were flight samples was wrong. Defense
now observes the held windup only, and never invents a holder in flight.
A World::step regression first showed a stale60-update counter timer where
the no-holder path should leave7; it now passes. The same failing aggregate
comparison passes without changing any bound. Final16-base counter means are22.6875,6.3125,15.875 against17,3,15. These remain different counts; full AI/RNG parity stays open.

The production World::step regression failed first because there was no
windup. It now covers short/long row bands on stand-in and disc poses,
the held wait, release and snapshot/checksum restoration. Existing head
scoring, assigned-counter and bone-flight checks pass with the new wait.
Snapshot72/protocol22 serialize and validate the defender; invalid missing,
same-side or out-of-range head references and board/head lifecycle mixes
are rejected. Both game and referee were rebuilt.

Complete gate on 935bd92: workspace1389 passed,0 failed,6 ignored; played35/35
on23 retained captures; attract17/17 on each of3; strict workspace/all-target
lint, formatting, game/referee builds and49 Python tests pass. A reviewed
private-desktop12-second demo exits0. Whole-game percentage and scorecard
statuses are not expanded by this bounded release correction.

## The Level 2 look: GB2's burst, the screen layers, the blur (2026-10-23)

Vj reported the Level 2 gamebreakers "all wrong". The clips, frames and
placements were already measured exact (2026-10-22); what differed is how
the original presents them. Grabbed from the running original on the
hidden desktop (School lesson 25 from state 50, the plans' region targets,
nearly every frame) against ours (`--level2-at x,z`, `--every`/`--count`),
then read from PCSX2 GS dumps of the original's frames (the dump hotkey
bound in the task's own copied profile) and the code that builds them.

**GB2's burst (built, `banner.rs`).** The Level 2 banner's element 9
(`FUN_002a8538`; update `FUN_002a8770`, draw `FUN_002a8c20`) is a sunburst:
twelve wedges from the frame's middle to the table at `0x4d4a10` (filled at
the boot, `0x2bbcd4`; read from the running game: (310, 0), (480, 0),
(700, 100), (640, 320), (640, 480), (400, 480), (250, 480), (60, 480),
(0, 400), (0, 240), (0, 65), (115, 0)), yellow `0xffe2c108` and orange
`0xffe49416` by turns (`0x3fb4f8`). Its clock shows wedges
`max(0, c/4 - 4)..min(12, c/4 + 8)`: eight, all twelve from 16, one gone
every 4 updates from 20, which is the wipe that uncovers the play. Its
twelve yellow stars (sheet (429, 176), 80 by 73) fly in under the wedges;
no grab shows one.

**The screen passes (built, `street-render`'s `screen_fx`, `street-game`'s
`screen_fx`).** All are in the original's frame as GS passes: the frame starts by copying the last rendered scene
(buffer 70) into the shown buffer (0, 16-bit, dithered) as a 640x448 sprite,
alpha `(Cs - Cd) * As + Cd`; the scene is drawn into 70; full-screen
untextured layers go over it before the HUD.

- *The copy* is `FUN_00376ac0` (alpha) and `FUN_00376ad8` (colour) on the
  display `DAT_00403594`: 0x7f and 0x7f,0x7f,0x7f normally. The gamebreaker
  overlay's blur object (`FUN_0030fb90`, class `0x499858`, the overlay's
  `+0x10`) is started by every gamebreaker (listener slot 64,
  `FUN_00311708` → `FUN_0030fc28`): after `900 / fps` updates (15) each
  update sets the copy to alpha `2880 / fps` (0x30, 0.375) and colour 0xa0
  (1.25 times), so the screen keeps 62.5% of what it showed and a still
  picture settles 1.25 times brighter. Slots 65 and 67 (the gamebreaker
  over, the meter drops; `FUN_003117e0`, `FUN_003115a8` → `FUN_0030fc98`)
  put the copy back at once.
- *The staged effect layers* are the scripts' category 2 records
  (`timeline::Effect`), played by the script manager's effect players
  (`FUN_00333298`, class `0x49c9f8`, one full-screen quad each, class
  `0x4997d8`): byte +0x2c is the player (slot), +0x2d the blend (1
  `(Cs - Cd) * As + Cd`, 3 `Cd - Cs`), the words at +0x18..+0x24 R, G, B,
  A (alpha halved to the GS's 0x80), and the timeline's keys animate the
  record's ten fields (`FUN_00333530`/`FUN_00333648`: position 0..2, target
  3..5, colour 6..9). GamLR1: slot 0 at frame 0 subtracts (0, 15, 60) for
  the whole script (the warm look); slot 1 at frame 92 (the freeze) flashes
  white, its alpha keyed from 0x80 to nothing.
- *The gamebreaker-ready tint* (the overlay `FUN_003110e8`, a game
  listener, its fader `+0x60`, class `0x499758`, queued requests
  `FUN_00310810`): each side is -1 (no full meter), 0 or 1
  (`FUN_00311328`, the side's +0x18 and +0x20); the higher picks a row of
  `0x499c98`, halved: 0 a blend of (20, 30, 100) at 50, 1 a subtract of
  (0, 12, 40) at 127, faded in over 30 updates (and the old one out over
  30), or out over 60 with no side ready (slots 66, 67, 70, 71 re-pick;
  `FUN_00311390`, `FUN_003115f8`). A Level 1 gamebreaker's start (slot 64,
  level 1) queues row 0 at full, (40, 60, 200, 100). So the School's Level
  2 lesson is tinted before its press and the tint fades after the meter
  empties; state 41's frozen menu kept it on.
- *The overlay's flash* (`+0x190`, slot 26 a basket, `FUN_00311810`): a
  gamebreaker basket flashes white from 0xff to nothing over 60 updates
  (blend), at once, or for a Level 2 ball (kind 2) at the script's end
  (slot 62, `FUN_00311508`).

The quads draw as one pass after tonemapping and before the UI (each
quad's GS blend on the frame's 8-bit values: the blend `(Cs - Cd) * As >> 7
+ Cd`, the add `Cs * As >> 7 + Cd`, the subtract `Cd - Cs`, the quad's
alpha `(A + 1) >> 1`, in the order staged layers, the tint, the flash); the
copy as a pass after the UI over the last picture shown, kept per view (at
0x7f, 0x7f the rewrite shows the frame as drawn; the 16-bit shown buffer
and its dither are not drawn). The layer quads keep the original's
draw-then-step (`FUN_0030ff50`, colours truncated), the tint its queue
(`FUN_00310940`: the next request once a fade is done; a request's blend
stands from the last one set, which in play is always a meter's row
first), the effect players the script manager's clock (the freeze's own
frames while it holds and those frames counted on after it; they stand
through the replay of the script, which points only its cameras; no grab
shows a flash after the freeze or in the rerun).

**The press holds the game until the burst wipes.** A staged Level 2's
task (`FUN_00144df0`) suspends the task groups 0x401d2 at the press, and its
state 0 (`FUN_00144ef8`) starts the script only once `FUN_002bb818` is true:
the burst's `+0x1f8`, set by its draw once a wedge goes (32 updates after
the banner shows), or at once with the No HUD cheat (`FUN_00131208()+0x28`).
The grabs show it: the game's count stands at 28519 through the burst's
first frames and the uncovered court is graded. The game's update is held
for those updates as a replay holds it (`step_sim`); the two game updates
from the start to the script's frame 0 are unchanged. Online matches don't
hold yet.

Compared (`--level2-at -18,22` against the School's GamLR1, frame by
frame): the burst's eight wedges, its wipe, the GB2 word's halo from the
copy, the ready tint before the press, the warm play and rerun under the
letterbox, the freeze's white flash. Grabs and dumps:
`$EVIDENCE/claude-l2look-20261023` (not in git).

### An event's boss move, jersey and boss card (2026-10-23)

`FUN_001ded40`, on a Be A Legend event completed (`FUN_001e7ff8`), queues
after the court (`FUN_001de528(rec, 2, court)`): kind 7, the event row's
boss move (`FUN_001eac00`: `0x460790 + 0x1c * row + 0x18` for tournaments
and street challenges); kind 4, a street challenge's jersey card
(`FUN_001eac88`: `+0x1a`, cards 65..70); kind 3, a tournament's boss
(`FUN_001eadd8`: `+0x1a` 40..45 through the jump table at `0x45fff0`,
records 145..150). The setter's handlers (`0x45ffc0`): kinds 4..6 and 10
grant a card not owned (`FUN_001e0420`; card 34 only shows), kind 3 grants
the card whose player the record is (`FUN_001d2818`: category 0's table at
`0x45e150`), kinds 7..9 only show while the move's card is not owned. The
street jerseys are the cards that open those bosses' moves (Edit Moves'
`CARD_MOVES`), the boss cards the tournament bosses'. The rewrite had none
of the three: a beaten boss joined without his card, his moves stayed
locked. `Story::clear_event` now grants and notes them (`Notice::Jersey`,
`Notice::BossMove`), the results page draws them (`0x5d07` with the card's
player by the jersey table at `0x45e1b8`, `0x5d02` with the move's name),
and older saves own the cards their cleared events grant.

### The hookups' slots, the milestones' boss moves, the Street Champ jersey (2026-10-23)

Both results pages read the jersey slots (+0x1c1c..24) and the boss-move
slots (+0x1c28..30) as one value each: the first name, then `0x48fdf8`
(", ") before the second and `0x48fe08` (",\n") before the third, the
names "%S %S" (`0x48fde0`) of the card's player or the move's
(`FUN_001d6888`). Pick Up's page (`FUN_002c5cf8`) has its own lines for
them, `0x5b03` and `0x5b04`. The completed game (+0x1c34) carries its label
(`0x5b05`, `0x5d08`) only when the card's player is record 167 (0x2c6474,
0x264250); otherwise the name stands alone in font 3; the value is a
single amber line (+0x18 = 0xff, the stars) either way.

`FUN_001e0910(rec, n)` (a Street Milestone) also queues a boss move: when
milestone n's card `0x460050[n]` is a jersey (category 3) whose player
(`FUN_001d2818`) is record 173, 159 or 175, move 40, 41 or 47 goes into the
first free slot of kinds 7..9 (none when all three are taken).

The Street Champ jersey line (`0x5d09`, starred) is the career block's
result slot 4 (+0xcdc; +0xccc is six words, `FUN_001ea900` fills each once,
-1 empty). `FUN_001e9b90` sets it at 0x1ea3b4..0x1ea3e8 when the event on
court 10 (`FUN_001e8140`, the Rucker Park '78 Legendary Challenge) is
cleared and the block's +0x18 bit 1 is clear; that bit is set by Be A
Legend's completion board (class 0x3c) on its Continue (0x2621a0, message
35) and saved. The rewrite shows the line from the challenge's clearing
until the game that completes the mode (whose completion board follows).

### The outfit list's specials (2026-10-23)

`FUN_001e4720(editor, row, colour)` gives the outfit code at a row of the
list both the wizard (0x278c60) and Edit Outfit (0x28e220) show: code 81,
the Street Champ jersey, first when the editor's career has cleared the
event on court 10 (`FUN_001e8140`); code 80, the team uniform, next when
the player's team (`FUN_001d84d0`: field 3 below record 145, else field 28
for a created player on team 0x25) is one; the build's nine kits
(`0x4602a4`); each owned jersey card (`FUN_001d2b38(3)` of them,
`FUN_001d3168`, owned by `FUN_001d26e8`) as its code at `0x460330`; then 71
while record 167 is unlocked (`FUN_001d9ca0`). Codes from 54 are free and
always owned (`FUN_001e43c0`, `FUN_001e44e0`); their names are the
per-code text ids at `0x3f94c8` (`editplyr_clothes.txt` 60..87). The
rewrite lists them all (`ballers::outfit_list`; code 80 for a sixth man,
in the team he joined), and a created player on a league team (0..28) wears
its uniform in matches (`player::look_for_team`, the loader's forced 80).

### Be A Legend's event court and the Rucker Classic's song (2026-10-23)

After a Be A Legend game whose event is cleared (`FUN_001e7ff8`),
`FUN_001ded40` queues the event's court (`FUN_001eab50`, kind 2;
0x1deedc..0x1deef0). The kind-2 handler (0x1de65c..0x1de704) leaves the
slot empty when the court is open already (`FUN_001d37b0`), else finds the
court card naming it and grants it (`FUN_001e0420`). The rewrite had no
such grant: a cleared event's court stayed shut unless bought. It now
grants the card and shows the court on the results page
(`rewards::grant_event_court`). `FUN_001e0420`, granting the card of court
10 (the Rucker Classic), switches its song on in the saved options
(`FUN_001d7398(options +0xfc, track 20, 1)`, then copies them to the live
block); the rewrite does so on every grant path (event, shop, cheat, the
league regions: `Story::court_granted`) and the live options take it
(`options::follow_switched_songs`). `FUN_002f2d58` also switches it on
when a Be A Legend game on event 10 starts with court 10 still locked; not
ported.

### The clearout (2026-10-23)

`FUN_0015c778`, which the juke press tries before `FUN_0015c8e8`: pad
+0x7c & 0x10, the velocity +0x40 the zero vector, +0x34 0 (the floor),
game+0xf4 not 6 (a restart), +0x1a0 not 1 (the ball not yet put on the
floor since the take), `FUN_0016ea20` false (no post-up), the bar at least
0.2: `FUN_0019c2f8(channel, 0x456d02, 0, 0, 0xde5)` and 0.2 spent. The
script (`97`, `9b 0`, a 6-update blend to record 658 `clearout01`, `a6 2 1
0x28`, the rest of the clip at 2 updates a frame, callback 1) never sets
bit 10, so it is no trick move: no points, no counter, no trails.

Its window is contact kind 2 (`0x3d0c30`: the handler `FUN_00136290`, the
test `FUN_001366a0`: another team, not reacting), row 1 of `0x3d0d90`
(`0x3d0c70`: six arm capsules, bones 12..19) against `FUN_001392b8`'s
lists, within 6. The handler wants the other man in the clearout's front
quarter (`FUN_0018d9f0(clearout, other)` 2), draws `r % 100`, and when the
other plays one of records 650..652 (`0x43d0d0`, `FUN_00168e78`) weighs his
power (attribute 4) against the clearout's, else against the roll: the
weaker plays `react_stumbleback2` (`0x43d0c0` → `0x3d0e28`), the other one
of four hop knocks (`0x43d0b0` → `0x3d0de8`: records 591, 588, 589, 593),
drawn by `r % 4`; his face (`FUN_00159190(other, 7, fps × 2)`); the window
closes and the contact message (`0x3c6910`, kind 2) names the clearout his
screener. Ported as `World::clearout`, `World::clearout_contact`,
`trick::CLEAROUT` (the 76th script, never drawn) and `Action::juking` (bit
10 everywhere it was read off a trick).

### The two-person freestyles (2026-10-23)

`FUN_0015c8e8` after the bar's 0.2 (`0x15c97c..0x15cdd4`): unless both
turbos are held (+0x78 & 0x40000), the nearest opponent (`FUN_00170dc0`)
is within 64 of the holder's facing and the holder within 128 of his
(`FUN_0018d1f8` both ways), the opponent stands exactly still (+0x40 the
zero vector) on the floor and is not reacting (`FUN_0016ea70`), and the
holder stands still or barely pushes (+0x90 under 16), the two play a
freestyle by their distance (the distance table, `FUN_0016e948`) over the
holder's model scale (`FUN_0019cd48(+0x108)`'s x): within 4.75 `0x44d2cf`,
`0x44d9a8` or `0x44d9f9` (ids 35, 50, 51) by `r % 100` under 33, 66, the
opponent playing `0x44c116`; within 8 `0x44d423` (11, `0x44c116`),
`0x44d381` (45, `0x44c136`) or `0x44d3d2` (kept as id 45 though it is
script 58, `0x44c156`); beyond 8 the trick press. Each turns to face the
other (`FUN_0016f508`, +0xb0), the move is value class 2 (+0x248), and
unless the opponent is in a counter stance (`FUN_0016e9b0`) he plays the
reaction and the holder's +0x218 (faked) is set: the full points. Ported
as `World::freestyle` and `trick::FREESTYLES`; the rewrite keeps id 58 for
the last (its identity drives the script it plays).

### The dunks' and lay-ups' knocks: contact kind 1 (2026-10-23)

62 of the held tables' scripts (the dunks and lay-ups, `MOVE_ROWS`, and a
block jump's) open a kind-1 window, `a6 1 2 n` (row 2 of `0x3d0d90`:
`0x3d0cd0`, two body capsules of radius 3, bones 20 to 21 and 20 to 0).
Its test, `FUN_001371d8`: another team; the mover dunking or leaping
(bits 7, 15) and off the floor; not yet met with that man this move
(actor+0x33c, one word a man, `FUN_00172080`; `FUN_00171fd0` clears them
and the counts +0x354 and +0x358 when a script ends); the other not
reacting (`FUN_0016ea70`) nor in a rebound jump (bit 8); with either
dunking, the ball held (ball+0xf4) and no gamebreaker on it (+0x178).

The handler, `FUN_001366d8(mover, other)`:

- the other on the floor: with the mover holding the ball, within 200 of
  the other's facing (`FUN_0018d1f8(other, mover)`), coming down
  (+0x44 at most 0) and the stronger (attribute 4), the other plays one of
  `0x3d0f58`'s four lists by `FUN_0018d9f0(other, mover)`'s quarter
  (`r % n`), and the pair is met (`FUN_00172008(mover, other, 0)`);
- the other in the air, their heights within 2, the mover at least a tenth
  into his jump (+0x60 over +0x64: the launches `9e`, `9f`, `a1`, `ac` and
  the dive's set them, the actor update counts +0x60 up) and each within
  256 of the other's facing: the mover's weight (field 9) + 2 × power +
  dunks against the other's weight + 2 × power + blocks, the mover's × 6/5
  coming down and × 9/10 or × 4/5 after beating one or two (+0x358); by the
  gap a band draws the stronger's knock of the weaker and the weaker's of
  the stronger (10..20 and 10..15 to 20, 20..30 and 15..10 to 50, 30..50
  and 10 to 100, 50..75 and 10..5 to 150, 75..90 and 5..2 to 200, then 90
  and 2; past 100 the second only after the first); the mover never
  knocked under a gamebreaker; the pair met, beaten when the other was
  knocked. A knocked man faces the other (`game+0x2cc`, the angle table)
  and plays `0x44bb65`, or turned away `0x44bd94` with him behind
  (quarter 0); the contact message names his screener. A knocked mover
  with the ball lets it go (`FUN_0014c408`, the mover as the knocker):
  along his facing turned by `r % 200 − 100`, back 0.35 an update across
  and 0.125 up, spinning about the vertical (`0x4b72e0`, (0, 1, 0)) at
  `0.075 + u × 0.075`.

"The contacts' scripted handlers" above said none of them frees the ball:
kind 1's air duel does. Ported as `World::knock_contacts`,
`trick::knock_odds`, `Step::Knock` and `Flight::{knock, met, beat, air,
aloft}` (snapshot 73). The held tables' kind-2 windows (`a6 2 4 n`: the
heave shot `0x44f622` and the gamebreaker shot `0x44f6c3`, row 4 of
`0x3d0d90`, `0x3d0d30`: the body and arms) run as the clearout's
(`World::shove_contact`, `Flight::shove`). In the first capture's comparison about 4.8 floor knocks
and 12.6 duels a game; its block jumps at a dunker fell from 23.1 to 21.1
against 30, the knocked men not jumping (bounds reset by the rule).

### A premade career's starting points (2026-10-23)

`FUN_001e6ac8` credits a new career 1,500 (`FUN_001e70a8(career, 3,
0x5dc)`); `FUN_001e6518(editor, n)` then loads premade `234 + n` and
charges, each through `FUN_001e70a8(career, 3, -price)`: the build's price
(`0x4602a4`'s rows of 16 bytes, +0), the size's (+8's rows of 6, by field
31), the look's row of create table 2 (`0x4602b4`: every price 0), each
of the seven skills' steps up to its bar (`0x460568`: 0, 100, ..., 600,
`FUN_001d7f48` the bar), and its outfit bought as creating buys one
(`FUN_001e4af8(career, row, colour, 0)`). The rewrite's career started
from a premade left its star's balance as it was; it now holds 1,500 less
`ballers::preset_charges`.

### The gait selector's draws (2026-10-23)

`FUN_00168b30` draws from the shared generator (`3a16b8`) at its start on
every call (`0x168b7c`: the gate, `r % 100 < 50`) and again only when no
record already playing answers and some record is a candidate
(`0x168de0`: `r % n`). The rewrite drew both from a stateless hash of the
tick and the player, so its generator advanced about six times an update
less than original's: in the first attract capture's live updates the
original draws 8 or 9 times an update (5,924 and 6,157 of about 33,000),
the rewrite 3 (8,880 of 20,000); with the selector on the shared
generator, 8 or 9 (2,707 and 2,585). `gait_select::select_drawing`; the
first capture's shots bound reset by the rule (8.0), the online
determinism test's second match on seed 11.

### Who else draws from the shared generator (2026-10-23)

172 functions call `3a16b8`; 130 lie in the gameplay range and all but
eight are ported or traced (`FUN_00159240`, the idle face: actor +0x244
counts down and at zero draws a new wait, `fps + r % fps`, and a face clip;
`FUN_00159380`, the head's look, drawing when it picks a new look;
`FUN_00170e70` and `FUN_00171160`, since ported (below), `FUN_00170fc8`;
and three reached through
tables, `0x124d90`, `0x141c38`, `0x196b58`). The rest are the front end,
the cameras' director (`0x303c80`), the crowd's construction (`0x30b9b8`:
its animation picks run on their own generator) and the court's ambient
effects, which draw their particles: the drift `0x1f4950` (17 calls), the
gamebreaker fire `0x1f717c` (14), steam and breath (`0x20xxxx`). Those
effects dominate the generator: in the second and third attract captures
every live update draws 50 to 90 times; in the first (a court without
them in play) 8 or 9, with bursts of 20 to 40 lasting about 260 updates
(5% of updates). So the rewrite's simulation can follow the original's
gameplay draws (now 8 or 9 an update) but not the generator's sequence
itself until the effects' particle draws are ported with their motion.

### The celebrations (2026-10-23)

The original celebrates in two places only, never after an ordinary
basket (the rewrite's display had picked a celebration for every scorer
by the tick; that pick is gone). The game listener keeps the last
basket's scorer at its +4 (`FUN_00141818`, `0x14189c`, outside phase 9).

- **A gamebreaker's basket that does not win.** Its restart hold
  (game+0x104 = fps × 2 + `DAT_004b79f4`) counts down in the game's
  update outside the script manager's states while game+0x464 counts up
  (cleared with the hold, `0x141a40`). Still holding at
  `DAT_004b79f4` = fps × 2 updates in, the scorer gets actor+0x1c0 and
  +0x1c4 (`0x13da28..0x13da50`). The defence's side update `FUN_0015da28`
  acts on +0x1c0 first (`0x15dcf8..0x15dd80`): in phase 6, on the floor
  (+0x34 zero) and with no script whose channel flags hold 0x40 (the
  celebrations' own `91 0x00820042`), it clears the word, calls
  `FUN_00170e70` and skips the rest of the update. The word waits
  otherwise, into a later restart if need be. (Its other word, +0x364,
  for `FUN_00171160`, is set only by slot 52's handler `0x141418`, which
  `FUN_0013d388`, the game's teardown, posts.)
- **The win.** As phase 9 begins (`0x13dbb8..0x13dee8`) the scorer
  celebrates facing the court's centre: `FUN_00170e70`, then actor+0xb0 =
  the word of `atan2(x, z)` of his own place. In a trick-point game
  (config+0xe8 1) a scorer still dunking or leaping (bits 7, 15,
  `FUN_0016e9d0`, `FUN_0016e9e0`) sets game+0x10c instead, and phase 9's
  countdown (case 9, `0x13e890..0x13eb38`) starts the celebration the
  first update he is out of it. Then the first of the losing side
  (vt+0x94, the leader, `^ 1`) by slot standing on the floor runs
  `FUN_00171160`.

`FUN_00170e70` draws `r % 17` of `0x3d9078` (row, rate; the boot's
constructor fills it at `0x17bcd0..0x17c1a8`, the rates fps / 60 times its
constants: celeb01 2, celeb03 3.5, celeb05 2.5, the two jog celebrations
2 and 4, `stand_dance_1` 8.5, the freestyles 2.4 and one 3,
`SIG_MOVE_PICKUP_08_4` 2.4), writes channel 0's record (+0x28), start
(+0x24, 0), rate (+0x3c, 1 / rate) and end (+0x20, the ceiling of rate ×
the clip's last frame), sets actor+0xb0 to the ball (`FUN_0016f2b0`) and
starts script `0x44cbbf`: `97`, `91`, `9b 0`, an 8-update blend to record
-2000 from frame -2001 (the channel's own), `8c -1.0` (the play-out at the
channel's rate to its end), `91 0`, callbacks 7 and 1. `FUN_00171160`
does the same with `0x3d9138`'s four (`REACTIONS_FREESTYLE_01_2` .. `04_2`,
each with flags 1 or 2 and rate 2 or 2.4) on `0x44cc03` (the same bytes):
at least 10 from the centre on either axis, a man facing within a quarter
turn of the centre (`(centre word − facing) & 0x3ff` ≤ 256 or > 768) draws
`r %` the rows without flag 2 and wants to face the centre, else those
without flag 1 and wants to face away (+0x200); nearer, `r & 3` of the four
with his wanted facing kept. The facing turns toward the wanted one as
always (`FUN_00168ea8`, 46 units an update).

The rewrite plays both as reactions (`street_sim::trick::REACTIONS` from
`CELEBRATE_FIRST` and `DEJECT_FIRST`, read from the disc with the
channel's record and rate standing for the -2000 / -2001 / -1.0 the
scripts name; `PlayOut`'s count widened for `stand_dance_1`'s 459
updates), started without a step of their own: their first update is the
channel's next. Not ported: `FUN_00170fc8`, the defence's flair
(`0x3d9100`: `defflair_*` and the anticipation stand, r % 7), called from
`0x1685e8`.

### The stand path (2026-10-23)

`FUN_00163d40`'s second half (raw `0x1680ac..0x1689b0`), which the
defenders' gait pass left as a lead ("the stand path's stance"), is what a
player on the floor plays while his stick is under 16, or while he holds
the ball and may not move: a holder whose dribble is picked up
(actor+0x1a0 −1; `0x167020..0x167050`) and everyone while the referee
tosses (ball state 7). It never sets a velocity: it starts a stand script
on his channel (`FUN_0019c2f8`) whenever the row the channel plays
(`+0x50`, `+0x54` in a blend) is none of the stands his place allows, so
a stop blends into its stand, which loops (`ab`: the clip on at its rate
for good) until he moves or a move takes the channel. The stand scripts'
callback 1 runs `FUN_00163d40(actor, 0, 1)` again at a script's end.

- **Off the ball, the offence** (role 1): `r % 15` of `0x3d7938` every
  update (the shared generator), started while the row is none of
  `0x3d78f0`'s 18: breathe_1 (`0x4570f5`, 12-update blend, a frame every
  6), the warm-ups (`0x457139`, `0x4571a8`, `0x457202`: stand_warmup_1,
  stand_kick_ground_1, stand_stretch_arms_1 played out and chained by
  callback 3, the C library's `rand() % n`, `FUN_00172288`), five times
  stndrdy_noball (`0x4570bb`) and six stndrlx_noball (`0x4570d8`).
- **The defence** (role 2): `r & 1` every update, breathe_1 or
  STANDRELAX_ANTICIPATION_01_2 (`0x45731b`); the face-up
  (defensivestand02_faceup, `0x4572ae`, callback 5 turning him to the
  ball) while the holder's wanted facing is within 301 of his line to his
  rim (`FUN_0018d290`) and the defender faces the holder within 200
  (`FUN_0018d1f8`); started while the row is none of `0x3d7978`'s 22, or
  after 7 updates (fps × 0.12) on the post defence's rows 718/719
  (actor+0x318). The stance `0x457377` answers only a post-up
  (`FUN_00163a38`: the nearest opponent on the backdown rows 704..707,
  712/713), which is not ported. Then the flair (`0x1684dc`): the
  holder's nearest opponent (`FUN_00170dc0`), his velocity zero, in a
  restart, not already on the flair's rows (`0x43f0a0`), draws `r % 7` of
  `0x3d9100` (`FUN_00170fc8`: defflair_* at a frame every 4 updates, the
  anticipation stand at 2) on `0x457271`, which plays it out and goes on
  into the face-up.
- **The box-out**: nobody holding the ball, the ball in a shot, on the
  rim or rebounding (states 5, 6, 0), within 14 of the rim the offence
  attacks and the ball (on the floor) nearer it than he is: standboxout
  (`0x456f41`: `9b 0x20000`, state bit 17; callback 4, his wanted facing
  his own rim). It ends (`0x16814c..0x168214`) with stndrdy_noball once
  the ball is farther from the rim than he is or the shot is over (bit
  18's partner, +0x234, is not ported).
- **The holder**, not dribbling: in a restart the inbounder cradles it
  (`0x457412`, stndrlx_cradle; once all six are ready a draw goes
  unused), else, while the row is none of `0x3d79e0`'s 9, protectball
  (`0x45742f`) with his feet planted (channel flag 0x80000) or `r % 6` of
  `0x3d7a08` (the two WB dances, the cradle, protectball, the around-back
  flair, the dance); a stick out turns him (actor+0xb0 = +0x88).
  Dribbling: while the row is none of `0x3d7a58`'s 29, BACKDOWN_SIDEDRIBBLE
  (`0x443bad`) from the backdown rows 704/705, else `r % 11` of `0x3d7ad0`
  (the standing dribbles and their flairs, the hand's alternate by `f8`,
  one choosing its start frame by callback 22, the live hand) at 30
  handles or more, `r & 1` of its first two below. (`0x3d7b00`'s three
  need actor+0xa0, the updates the stick has been out, at 60 or more on
  the stand path: never.)

**Ported** (`crates/street-sim/src/stand.rs`, `stand_tables.rs`): the
choice and its draws, the stand scripts read from the executable by
`tests/trick_disc.rs`' compiler (`Step::Loop`, `Step::Jump`,
`Step::Roll`, `Step::RightHand` in `street_sim::trick`), played on the
player's channel (`Player::stand`) and shown by the display; the holder
who picked up his dribble no longer walks. The stop itself stays the
rewrite's `tuning::DECELERATION`: the scripts' `97` zeroes the velocity,
but the original also rides the outgoing gait clip's root through the
blend, which the rewrite does not play, and an instant stop with the
stands' roots alone raised the defenders' standing share past the attract
captures' (0.42 against 0.38). The second capture's blend bound was set
again by the rule (0.558 against 0.388, the seed sets +500 0.539 and +777
0.441). The C library's generator the rolls step is the simulation's own
(`World::libc`); the original's is shared with the presentation's coin
flips and the speech.

### The idle face and the faces' time (2026-10-23)

`FUN_00159190(actor, kind, duration)` (31 callers: the trick press, the
fakes, the clearout's and the knocks' victims, the swipes, the blocks,
the counter stance, the leaps, the baskets) writes the face's kind and
time (actor+0x240, +0x244) and draws its clip (`r %` the kind's count at
`0x44f750`); a duration under 1 does nothing, no draw. The actor's update
then runs `FUN_00159240` after his side's update (`0x157270`), outside the
script manager's states: the time counts down, and at 0 the idle face
(kind 0) for `fps + r % fps` updates with its clip drawn, two draws. The
actors' reset (`FUN_00156870`, `0x156a94`) starts the time at fps. The
rewrite had kept the faces' draws without their time; `World::face` and
`World::idle_face` now keep it, so every actor draws twice every one to
two seconds as the original does (the faces themselves are still not
played). The head's look (`FUN_00159380`, a look-target state machine
that draws when it picks a new look) is not ported.

The played captures' jump replays now start from the capture's own
generator words before the press (they had used the replay world's seed):
a dunk's hang on the rim (callback 28, `r % 100`) drew on the rewrite's own
path, so the dunks' share rising within 0.01 after the blend moved with
every change of the draws (0.969, then 0.935 with the stands' draws); from
the capture's words it is 0.956.

### The head's look (2026-10-23)

`FUN_00159380` runs late in each actor's update (`0x157d9c`); under the
script manager it hands over to the presentation (`FUN_0032bf00`), and in
the game's phases 0, 8 and 9 it resets the look (situation 0, target 0,
time −1, the ball's last state 1, no last holder). Otherwise it reads an
event from the play: the ball let go (no holder, not a pass, after a hold
or a pass) 0; a new holder: himself 7 (3 in a shot, lay-up, dunk or leap),
a teammate 8, an opponent 9; a pass let go: to him 2, from him 1; else his
state bits (actor+0xa4) against the last update's (+0x208, which the
actor's update writes after the look, `0x157e7c`): a shot, lay-up, dunk or
leap begun 3, a swipe, counter stance or block jump begun 5, one of those
over 6, a move over 4. His situation (+0x20c) answers the event by its
row of `0x3e7060` (eight rows the boot fills, their lists in the
executable at `0x454720..0x4548b8`): the event found among its events
moves it to the matching next situation, and a new look is drawn; so is
one when the look's time (+0x214) runs out. The draw weighs the
situation's targets (`0x3e7100`: every weight 1), the one looked at left
out when only the time ran out, by `(r & 0x7fffffff) × 2^-31` of their
sum, chosen from the last; its time is drawn between the target's bounds
(fps to fps × 4/3, or −1 for good); a teammate's look (kind 6) draws his
slot (`r & 2`, his own ± 1). The rewrite keeps the look's state and its
draws (`street_sim::look`, `Player::look`); the head is not posed from it.

The played captures' jump replays settle every look where the play
stands at their start (the capture records no look), so no actor turns
his head on the replay world's first update; the dunks' share rising
within 0.01 after the blend is 0.954 with the looks' draws.

### Every face's call site (2026-10-23)

`FUN_00159190`'s 31 call sites against the rewrite, each now through
`World::face` (the draw and the face's time): the basket's six (the game
listener, `0x141818`), the clearout's, the knock's and the trick contact's
victims, the trick press, the swipe's landing pair, the block jump and the
block test's pair, the counter stance and its take's pair (`0x134c58..
0x134ce4`), the aimed dunk, the leap and the shoot press (`FUN_001693b8`,
whose two draws the rewrite had kept). Newly drawn: the pass start's
passer (`0x1610ec..0x161110`, `r & 1` and the face, after the row's
script starts), the parked catch row's receiver as it starts
(`0x15ab24..0x15ab38`), the swipe's swiper at the press (`0x15e364..
0x15e378`), the rebound catcher after the take (`0x16f1c0..0x16f1d4`), the
rebound jumper at callback 36 for his airtime (`0x175fd0`), and the game
listener's slot 27 (`0x141c38`, a shot done without a basket): the
shooter's side by slot, his own `r & 1` and face for fps × 2, the others'
for fps × 3. Left: the passout legs' end (`FUN_00148378`, `0x148584..`: a
receiver in the counter stance takes it and faces, else `r & 1` picks his
catch script), the glass pair's aimed jump (`a9`, `0x19a5d8`), and the
box-out's contacts (`FUN_001343e8`, `0x1351a0..0x135308`: the boxer's push
`0x456f6f` and the boxed man's reaction by `r & 1`, bit 18, each with a
face), now that the stand path boxes out.

The first capture's block jumps at a shooter were set again by the rule
(16.5 against 11; 14.7 before; the seed sets +500 14.6, +777 15.4).

### The box-out's contact (2026-10-23)

The plain contact (`FUN_001343e8`), after the counter stance's check,
takes a touching pair for a box-out instead of the push (`0x134f4c..
0x135344`; then the contact's message `0x3c6910` and no push that update).
Neither may have flag 2 on channel 0. `iVar21` (the defender of an
opposing pair) boxing out (bit 17) and holding nobody (+0x234), the other
boxable (+0x238 clear, on the floor, not rising, neither boxing nor boxed,
no rebound or block jump): with him in front (`FUN_0018d9f0`'s eighths
3..5) the push goes on; behind (its quarter 0) the boxer plays
`BOX_BEHIND_4` (`0x456f6f`) and the boxed man (bit 18) a front clip by
`r & 1` (`0x456ff9` `BOX_FRONT_HIGH_5_R`, `0x45702c` `BOX_FRONT_LOW_10_R`, the
left-hander's alternate by `f2`); to either side the side clips by the
eighth (below 6: `0x456fcb` and `0x45705f`, else `0x456f9d` and
`0x45708d`). Otherwise the other boxing with `iVar21` behind him plays
the behind pair (no side clips that way round). The partners are kept
(+0x234 the boxer's man, +0x230 the boxed man's boxer); the faces follow
(the boxer's 2, the boxed man's 3 or 4 by `r & 1`, fps × 3) and the
message names the boxer to the boxed man's AI. The stand path's box-out
end lets the man go: +0x230 cleared, and back to `stndrdy_noball` if he is
still boxed. All seven clips are stands (`street_sim::stand`, from
`BOXER_BEHIND`); the rewrite asks the boxable man to have no move under
way (`Action::None`) for the channel flag.

### The board passout's look behind (2026-10-23)

Callback 53 (`FUN_001741a8`, `0x1749b4..0x174ad4`), tossing a board
passout (any kind but 2; for a non-zero kind only with the board point
above 13.25), turns the passer's defender (`FUN_0016e8f8`) to look behind
him: on the floor, not in the counter stance (bit 16), not down
(`FUN_0016ea70`), not swiping (bit 6) or in a block jump (bit 12), in the
cone between the passer and the rim (`FUN_00170840(passer, defender, 0)`:
the default width `DAT_003d905c`, 256), when `r % 100` (drawn only then)
comes in at most `40 + (int)(Handles / 99 × 35)` (the passer's, record
attribute 7): `react_fakepasslookbehind_l` (`0x44cac2`: a 4-update blend to
610, played out at a frame each 1.5 updates, callback 1). Ported
(`World::board_look`, `trick::BOARD_LOOK`). Callback 1 hands a channel to
the locomotion (`FUN_001722c0` runs `FUN_00163d40`), so a script ends
there: this one has no `81` after it, and both the disc tests' decoder and
the display's VM (`street_assets::playback`) now end a plain script at its
callback 1.

### The head passout's legs (2026-10-23)

Off the Heezay's press (a person's `FUN_00146278`, `0x146a44..`, the stick
idle and the passer's defender within 12; the computer's `FUN_00183318`)
stores kind 2 with the passer's `Handles × 4 / 99` (the reach), bone 21
and the defender. The release (callback 53 to oneself `0x1746c4..0x1749a8`,
callback 38 to a teammate `0x173930..0x173c64`) aims at the defender's
head: bone 21's position out along its local x and y (`R × (1, 1, 0, 0)`,
normalised) by half his height scale (`model+0x40`) and the ball's
radius; the legs are timed as the board's with the head's ratio
(1.0526316 for 0.8696): the whole `trunc(max(d1 / S + d2 / (0.75 S), 20) ×
k)`, the first `trunc(max(d1 / S × k, 2))`, the second to the passer's
hand or the receiver in what is left (`FUN_00148a08`, `FUN_001488e8`; each
queued leg's count runs down with the flight). At the first leg's end
(`FUN_00148378` kind 2): the ball within the reach of the head bone and
no more than its radius and 0.75 above it meets the head, else the legs
end and the ball flies on; met, a defender in the counter stance takes it
(`PASSOUT_DEFENDER_REACTION_5_R` `0x45799a`, his dribble 0, his face `r &
1`), otherwise his facing becomes his wanted facing, he reacts by `r & 1`
(`PASSOUT_DEFENDER_REACTION_3`/`_3_2`, `0x44c1ad`/`0x44c1df`) and the leg
back flies. Ported (`World::head_point`, `head_legs`, `head_leg`; the
rewrite's 0.3 + 0.35 s flight and its `0.55 + 0.4 × Handles` odds are
gone). A Heezay to a teammate waits for its row's callback 38 as the
board's does: the head's point taken again at the release, the legs on
the head's ratio (`board_flight`, `board_release`).

Callback 38's board release also turns the receiver's defender to look
behind (`0x173c70..0x173d94`, as callback 53's for the passer's).

### The post-up's backdown (2026-10-23)

The original's holders back down on their own; the rewrite's never did
(state bit 11 read as clear everywhere). `FUN_00163d40`'s moving path for
a holder (raw `0x167564..0x16780c`, after the family's request and before
the gait's pick) asks `FUN_00163b48(actor, stick)` (`0x163b48..0x163d38`):
true at once while posting (bit 11); else his nearest opponent up (channel
flag 0x40 clear), the stick within 250 units of his way to the rim he
attacks (`FUN_0018d500`, `FUN_001404e0`'s rim), his wanted facing (+0xb0)
within 250 of the man (`FUN_0018d1f8`), the man more than 2.1 nearer the
rim (game+0x374), himself 15..18 from it, the two within 6 (the distance
table), outside a restart, no turbo held (+0x78 & 0x2000), and the man no
more than 2.5 off the line from him to the rim (`FUN_001379d8`, the
perpendicular of the VU0 projection). Then:

- on the backdown's rows (704..707): the request is 0x2100, the one gait
  record built with it (`0x166f34..0x166f50`: rows 704/705
  `BACKDOWN_3_R`, script `dribble_backdown_1`, whose `9b 0x820` posts);
- posting off those rows: by `FUN_0018cd78(wanted + 0x200, stick)` within
  384, `juke_dropstep_l` below −128, `_r` above 128, else `into_backdown`;
- otherwise `into_backdown`, and the post-up message (`0x3c6480`, not
  carried).

On the backdown's rows the stick decides
the way out (`0x1676c8..0x16780c`): its turn from the wanted facing
within 385 and the man between the holder and the rim within 250
(`FUN_0016fef8`) and no farther from it (else `from_backdown_back`);
beyond 128 either way `backdown_spinout` or `backdown_stepout` by the
live hand (+0x25c); else the 0x2100 request.

The scripts (read from the executable by `tests/trick_disc.rs`' stand
compiler, stand scripts 34..46): `into_backdown` (`0x44392a`) plays the
transition in, 712 or 713 by callback 45 (`FUN_00177750`: the facing more
than 256 off the way to the rim, the side it is off to, else the right
hand; `Step::Asked`, answered as the script starts), then runs into
`0x443bad`, the side dribble that sets bit 11. Its `91` sets the channel's
0x2 without 0x10, so the offence's update does not call the locomotion
while it plays (`0x15ab58`, `0x15b084`); the spin, step-out, drop steps
and `from_backdown_back` (714/715, `Call(1)` at their end) likewise. The
rewrite plays them as stands his stick does not stop
(`World::backdown_holds`) and holds his walk meanwhile.

The stand path (`0x1685f8..0x1687e8`): a posting dribbler comes out the
back unless his nearest opponent stands in his cone (`FUN_00170840`,
250) nearer the rim; a posting holder whose dribble is picked up plays
`r % 5` of `0x3d7a40` (`post_stand_wb_1`, `post_fake_1`/`_2`,
`post_push_1`, `post_look_to_pass`, kept on `0x3d7a20`'s 8 rows) with the
man in his cone (width 0), else `post_face_rim_r_1` (kept on 720/721).

The defence (`FUN_00163a38`, `0x1683d4..0x168448`): a defender whose
nearest opponent posts or plays the post-up's clips (`FUN_0016ec80`),
within 199 units of that man's way to the rim seen from him, within 5 of
him (7 on the post rows 718/719) and in his cone takes
`defense_stand_postup_1` (`0x457377`, `0x3d79d0`'s third: 718 or 719 by
callback 49, `FUN_00177fa0`: the holder's right hand when the holder is
his nearest opponent, else by his own place) facing the man, and the post
rows' count starts again. The moving path's first test
(`0x167108..0x167198`): on those rows against his man (`FUN_0016e8c0`,
the defence strategy's matchup) posting within 5, with his stick within
384 of his wanted facing, he holds the stance (no locomotion).

The bit's readers, ported with it: the shot's pick (`0x16b89c..0x16b8f4`:
facing 400..600 from the rim, posting within 25 of it,
`jumpshot_2_fadeaway`, actor+0x1e4 8); the trick draw (`0x15f40c..
0x15f54c`: posting off rows 704/705/712/713 only the post-up's rows, the
sides and the last drawn not asked); the plain pass pick (`0x1474b8`: a
posting teammate less 9). Any other move's script clears the bit (its
`9b`); a gait record started anew sets it only for the backdown's.

Not carried: the post-up message's listeners, the contact manager's
post-up pairs (`FUN_00133bb0`, `FUN_00136218`) and the bump reaction
(`backdown_bump_reaction`, `0x44c31c`), the catch rows with 0x8000, the
clips' roots through the transition (the rewrite's deceleration stops
him), `0x3d7b00`'s dribble stops, and the moving path's restart request
(`0x1676a4`: a holder walking in a restart asks 0x201, the one record
BPJOG 957, no dribble), which doubled the catches with 5+ updates left
against all three attract captures (0.17, 0.15, 0.20 against 0.012,
0.082, 0.022; the rewrite's own were already high at 0.087, 0.090,
0.119) and waits on the inbound pass's own gaps.

The first capture's blend back-off set again by the rule: 0.488 (se
0.031) against 0.406, bound 0.18 (seed sets +500 0.452, +777 0.420). The
online determinism test's first match plays seed 2026 (2025's blocked no
shot once the holders backed down).

### The post-up's other readers and the box-out's pair (2026-10-23)

More of state bit 11's readers (every `jal 0x16ea20`, 27 of them):

- **The facing's turn** (`FUN_00168ea8`, `0x168f38..0x168fec`): 46 units
  an update, 23 for a posting holder (+0x70), 11 on the post defence's
  rows 718/719 within 57 of the wanted facing (34 with planted points
  and 23 in a dunk or lay-up, where the walk does not run).
- **The post defender's slide** (`FUN_0015da28`, `0x15db18..0x15dca4`):
  on the post rows, the holder (not blending) his nearest opponent and
  posting or on the post-up's clips: he keeps his distance from the
  holder and goes round him toward the holder's way to the rim, at most
  14 units an update.
- **The bump's shares** (`FUN_001343e8`): the still-against-moving rule
  only when neither posts (`0x1349b0..0x1349c8`); after the pass's, a
  posting man standing still is not moved (`0x134a94..0x134ab4`), so the
  defender takes the overlap.
- **The contact radius** (`FUN_001337e0`, `0x133838..0x133880`): no limbs
  for a posting man or one on the post-up's clips, nor against a man
  posting off those clips from the post rows.
- **The catch rows** (`0x162204..0x162240`): a posting receiver takes only
  the 0x8000 rows, anyone else none of them.
- **The pass start** (`0x15fce8`): a posting passer is measured where he
  stands, not ahead.

The box-out's pair (bits 17 and 18, +0x234 and +0x230), which the
box-out's contact left half carried:

- **The pin** (`FUN_00133bb0`, `0x133c08..0x133e40`, the contact
  manager's first pass): each boxed man whose boxer holds him, neither
  in a blend, is drawn toward his place by the boxer: his model's scale
  (+0x40) along the boxer's wanted facing, turned by their clips (the
  boxer's 60/61 320 either way at 2.5; else behind, the boxed man's 58/59
  64 either way of it at 1.5, else 2.15), at most 0.4 an update. The
  clips' root offsets (+0xe0) are not carried.
- **No push between them** (`FUN_00136218`, `0x13624c..0x136278`).
- **The release** (`FUN_0019c320`, `0x19c3cc..0x19c490`): any script
  started on channel 0 ends the pair it is part of: a boxed man's boxer
  plays `standboxout` again and holds nobody; a boxer's man stands ready
  if still boxed; both bits clear (the new script's `9b` sets its own).

The off-ball post call (`FUN_0015a060`, `0x15b544..0x15b5bc`): a man of
the side with the ball, without it, holding juke (+0x78 & 0x800000, the
trick button), in the dunking area (`FUN_0016c908`) on the floor outside
a restart and not posting, catching or in a rebound jump plays
`post_call_for_ball_2` (`0x456d96`: `9b 0x800`, callback 5 to the ball,
728 or 729 by callback 10, his nearest opponent on his wanted facing's
side; its `91` holds his walk); let go on rows 728/729, he stands ready
and the bit clears (`0x15ad84..0x15adc0`). A juke press by the side with
the ball off it no longer swipes: the swipe is the defence's update's
(`FUN_0015da28`).

Still not carried: the posting receiver's facing and
the holder's (`0x15a4ec`, `0x15a768`), and the screen point (`0x171b28`).

Posting receivers (`0x15a4ec`, `0x15a768`): a receiver with bit 11 set
skips the receiver's update (the court's edges and the spot run's stop)
both while the passer winds up and while the pass flies.

The restart's BPJOG request tried again on 2026-10-23: the rewrite's
restarts (eight computer matches of 40,000 updates) last 178 updates on
average with it against 155 without and the original's 157 (39 captured,
80..313), and the catches with 5+ updates left rise to 0.17/0.18 in the
first and third captures. The record's speed as the rewrite derives it
(the walk's family against an unheld record with no family bit) is the
suspect; left out.

**The restart's walk by the stick** (`0x167b98..0x167bd4`): the moving
path's motion goes by the speed rating only for a run (request 0x1000)
outside a restart and outside the guarding mode; in a restart every walk
goes by the stick (`intensity / 64`, at most 1), as the guard's does
(`Player::restart_walk`, snapshot 79). The rewrite's restarts then last
154 updates on average (eight computer matches; 93..275) against 155
before and the original's 157 (80..313). With BPJOG on top they last 161,
but the catches with 5+ updates left stay at 0.17 in the first and third
captures (0.012, 0.022 in the original): what follows the jog (the
inbounder not dribbling, the cradle) is the gap; BPJOG stays out.

**The inbound pass's early catches** (measured 2026-10-23, four computer
matches of 40,000 updates against the first attract capture): the
original's inbound passes (ball state 4 begun in phase 6) fly 20 updates
and are caught at 15..18 (left 2..5, mostly 2). The rewrite's fly the same
20 (about 10 apart, the minimum flight) and are caught at 12..14 (left
6..8): 36 of 117 with 5+ left without BPJOG, 71 of 110 with it; in open
play 66 of 455. The catch rows drawn (`catch_stand_1`, `_3r`, `_3l`) have
no count of their own (+4 = -1), so the row runs the whole flight and the
first take (within [`touch::NEAR`] of the pelvis or a hand, bit 9) takes the
ball as soon as it comes that near. The paths settle it: the original's
three first inbounds leave about 9 from the receiver (13.7 at the press;
the inbounder walks 4.3 toward him through the 18 updates the row holds
the ball) and rise from 6.6 to 8.6..9.3, caught 2.2 from his feet with 2
left. The rewrite's from the same distance fly the same path update for
update and are caught with 2 left; its early ones leave only about 6.5
away, fly flat at 5.9 and meet his hands with 6 left. What differs is the
restart's spacing at the release (the receiver's spot and the
inbounder's walk through the hold), not the catch.

At the inbound press (all three attract captures: 39, 17 and 28
restarts) the original's inbounder and receiver stand 14.4..14.5 apart
on average (restart spots 0 and 1, [47, 16] and [35, 8], are 14.4
apart), 12.7..13.1 at the release; the rewrite's 12.0 and 11.2. The
inbounders stand alike (the original's at x 46.4..47.3, the rewrite's at
46.0..46.6). The receivers do not: "inbound pos" stops within 1.4 of the
spot, and the original's arrive from the court's side (x 33.8..35.3, the
spot less up to 1.4) where the rewrite's arrive from under the basket
(x 34.8..36.5). They come from where the last play left them: the
rewrite's defence ends its plays nearer the rim (its third of phase-5
updates within 4 of the handler against the original's fifth, above).
The inbound's early catches follow from that, not from the restart's
code.

**The post-up's bump** (`FUN_001343e8`, `0x135430..0x135740`; the stack
read from its prologue: +0xb0/+0xac the second's and first's motion,
+0xb8/+0xb4 their flag 0x40, +0xd0/+0xc4 on the floor, +0xbc/+0xc0
attribute 4, s5 the second's channel row, fp the first's channel). After
the shares, a pair of opponents whose second (the offence's man) holds
the ball takes the bump in place of the screen reaction: the second on
the backdown's walk (704/705) and moving, the defender still, both on the
floor and neither off balance, the second's count of contacts with him
at least half of `0x3d0f6c` (30), each facing the other within 100
(`FUN_0018d1f8`) and the second the stronger. The defender plays
`backdown_bump_reaction` (`0x44c31c`: 717 at 1.2 a frame, then the post
stance's body, callback 49 answered as it starts; its `91` sets 0x2 and
0x40, so he is off balance and does not walk until the stance), the count
rests at -1, the contact's message goes out (kind 0, nothing scored; his
AI hears who bumped him) and the handler returns: no push that update.

**The turbo collision** (`FUN_001343e8`, `0x135748..0x1358fc`): with the
ball and off the backdown's walk (any failure on it pushes as usual), the
second (the holder) takes the collision when he is not passing, catching
or in a trick (bits 4, 9, 10), faces the rim he attacks within 250
(`FUN_0018d290`), stands 15..18 from it with the defender more than 2.1
nearer (game+0x374), both on the floor and the defender not off balance,
outside a restart, moving and holding turbo (+0x78 & 0x2000; the
backdown's own test refuses turbo): the defender plays
`collision_defender` (`0x44c741`: 625, played out at 1.5 a frame), the
holder `collision_ballcarrier` (`0x44c6f8`: 623/624 by the live hand, at
2.0), both `91` 0x42 (no walk, off balance) until callback 7 and the end;
the contact's message (`0x3c6910`, kind 0) names the defender first, so
the holder's AI hears him (`FUN_00141448` → `FUN_0018b308(team, +8,
+4)`); no push that update.

The second capture's within 4 in tricks set again by the rule with the
collision: 0.389 (se 0.014) against 0.324, bound 0.11 (the seed sets +500
0.348, +777 0.336; main before it 0.382).

**The give 'n go's calls as nodes** (`FUN_001587d8`, `0x158e14..0x158f04`
and `0x158f50..0x159098`). While a person's side is held (team+0xc8) and
has the ball, off it: Circle pressed without a turbo held, the computer
holder (controller 4) not in a shot, dunk, lay-up or leap (`0xa088`), has
his AI's queue cleared (`FUN_0013aa48`) and a shoot node pushed
(`FUN_00100e38`, `FUN_0013aa88`); Cross pressed, the holder not passing
(bit 4), a pass node to the caller (`FUN_00100a08(ai, caller's ai)`), or
with two turbos held (0x40000) a trick pass node with no receiver
(`FUN_00100c60(ai, 0, 0)`, whose update faces the rim, `FUN_0017da80` at
level 3, names no target and presses two turbos and pass, `0x100dcc..
0x100e08`). Both checks run in one update, Circle's first. The rewrite
built the pass at once; it now pushes the node in the caller's update
(`World::call_for_ball`, `ai::Call`), its draws made there (the pass
node's `r % 3 + 6`, the trick pass's wait, the shoot node's two), and the
node presses at its own pace. The pass back to the man who gave and went
keeps the give 'n go's style. Not carried: `FUN_0016e9e0(caller)`'s
+0x270 |= 2.

**The putback** (`FUN_00169c98`, raw `0x169c98..0x16b1e0`; `street_sim::
putback`). The side with the ball's rebound press (`FUN_0015a060`,
`0x15b4c0..0x15b53c`: not a pass to himself, on the floor, live, not in a
rebound jump) first asks the putback, then the rebound jump. It takes two
turbos held, no teammate dunking, laying up, hanging or leaping
(`0xe080`), no gamebreaker shot (ball+0x178), a shot, a ball on the rim or
a rebound (states 5, 6, 0), the court's side of the rim (x and his way to
the rim, `FUN_0016f260`), 3..12.5 from it (`DAT_003d82e4`), his wanted
facing within 228 of that way, and no opponent in his cone
(`FUN_00170840` at `(1 − Dunks / 99) × 200 + 100`) within 4.5 (the
table's distance, `FUN_0018cdb0`, on the floor). The records: eleven of
0x120 bytes at `0x3d8300`, built at the boot (the same in all three save
states' RAM): the jump's root, pairs of hand points (an offset at the
larger of the height and width scales, a bone at the height scale) for
two catches and the slam, the blend, wait, catch and slam updates, the
slam's turn, the right and left scripts and sector masks, the kind (1,
a tip slam at actor+0x1e4) and which hands catch and slam. For each record
and side whose mask holds his sector about the rim (`FUN_0018d768` at
0x100 or 0x300; none under 25 Dunks): the points scaled, mirrored on the
left, turned to his way to the rim by a yaw quaternion (`FUN_002f7430`)
and placed at his feet; the slam's hand aimed 11.67 up by the rim (11.6
for a two-handed hang; turned by the record's +0xf8, or square to the
spread hands; 1.0182 out, or as the hands' spread allows), the jump's
velocity reaching it in the slam's updates under 32.15 / fps² × −1.258;
it takes when the ball 8 + wait + catch updates on (`FUN_001537d0`, its
own path stepped ahead) lies within the larger of 1.5 and its radius of
the hand at either catch. One that takes is drawn (`rand() % n`) and its
side's script played as a dunk row past the mid-air lay-ups
(`timing::PUTBACK_ROWS`, 18 scripts from the disc; the putback style, a
tip slam). The dunker's take (bit 7) catches the ball at the hand his
clip holds out. In four computer games' searches 60 putbacks started, 20
caught the ball (the misses passing 1.0..1.6 from a hand: the records'
test is looser than the take) and 11 scored. Not carried: +0x35c,
`DAT_003d658c` (3 out of the dunking area, else 2) and +0x1e0; the
gamebreaker flag is read from the ball's shot.

### The pass windup on its script, and the stick the row filter reads

A pass on the floor now plays its row's script through the jump module, as
the air passes already did. The rows' scripts open with `91 0x820002`
(standing rows) or `91 0x20006` (the `_run_` rows), and the locomotion
(`FUN_00163d40`) runs only while channel 0's flags lack 0x2 or hold 0x2
with 0x10 (`0x15b084`, `jump::walks`). The standing rows `97` him to a
stop (at once, or after their 6-update blend), plant a foot through the
blend (`95`), and give the locomotion back with `91 0` after the
follow-through (34 to 56 updates into the script); the running rows have
no `97`, so he coasts on his press velocity until the script's end, where
callback 30 (`0x172110`: channel arg0's flags with 0x2 set when arg1 is 1,
cleared when 0) lets go. The pass is over at that `91` with the ball gone,
and the walk takes up his coasting speed. Before, the rewrite walked him
at half speed for 0.35 s past the release.

The row filter (`FUN_0015f930`, `0x160684..0x1607c8`) reads the stick's
strength at actor+0x90 and the velocity at +0x40. The rewrite's computer
pads point the stick at the receiver at full strength on the press (the
sim's receiver pick without the AI's target), so every computer pass read
64 and a moving passer took the running rows, which none of the three
attract captures' 257 passes play. The original's pass node lets go of the
stick as it presses (`FUN_0013ac30`; strength 0 at the press in 120 of 131
first-capture passes), and its velocity is zero by the filter: 105 running
passers by the boundary (whose 0x208 test would otherwise flip them to the
running rows) all play standing rows. So the filter reads a computer's pad
strength (`timing::stick_strength`), treats a stick under 16 as the stand
path's zero velocity, and the speed test is 0.005 an update (it compared
units a second). The script's start velocity follows the same rule.

Passer motion after a live press, units an update (three captures against
eight seeded attract matches; standing rows only in the original):

| updates after the press | 0 | 6 | 12 | 18 | 24 | 30 | 40-update displacement |
|---|---|---|---|---|---|---|---|
| original | 0.16..0.20 | 0.04..0.05 | 0.07..0.11 | 0.08..0.09 | 0.09..0.11 | 0.14..0.17 | 2.35..3.10 |
| before | 0.18 | 0.16 | 0.14 | 0.13 | 0.14 | 0.15 | 4.10 |
| now | 0.23 | 0.05 | 0.12 | 0.06 | 0.05 | 0.15 | 3.31 |

**The inbounder carries the ball, and the held stands stop him dead.**
Stopping the passer exposed the inbound's spacing: the computer inbounder
had walked toward his receiver through the windup (3.2 on average), which
shortened the throw; standing, as the original's does, he left 56 of 114
inbounds caught with 5+ updates left. The original's inbounder carries the
ball from his catch to his spot (the held ball stays 2 or more up through
every captured restart: no dribble), so the locomotion's request has no
ball bit (actor+0x70, `0x167540`) and his dribble (+0x1a0, set by a 0x102
request at `0x167f54`) never starts; at the spot the stand path gives him
the cradle (`stndrlx_cradle`), which carries nothing. On the update his
stick drops he stops dead (0.000), his turn to the court is one update's
walk (0.218 at strength 38) and a dead stop, then he turns in place 46 a
update (the phase-6 holders' stops slide 0.17 over 12 updates in the
attract captures; the men off the ball 1.2 on their stance clips, which
the rewrite's stop still stands in for). The rewrite's inbounder dribbled
there and slid about 0.9 back toward the court on the stop after his turn.
Now a restart's holder asks for the no-ball records, and a holder whose
dribble is not going stops at once on the stand path. He throws from
(46.75, 14.81) against the original's (46.9, 15.1); 12.8 from his receiver
at the press against 14.4 (the receivers still arrive from under the
basket), and 21 of 125 inbounds are caught with 5+ left.

**"iso3pt"'s attack after a missed juke** (`FUN_0010c158`,
`0x10c2d4..0x10c4a0`). The attack (`FUN_0010c878`: `DAT_004b3dc0`, 9
updates, at the nearest standing opponent; node+0x34 cleared) counts
down, ends when the defender is down (`FUN_0016ea70`) or in his counter
stance (`FUN_00181828`, bit 16), and once node+0x34 is set does nothing
more: the pad holds what it had. Within 5.5 with the stick out and not
catching, with over 0.2 turbo, the juke's odds (`skill(50, 75, 100,
100)`) press it on alternate hits; a miss sets node+0x34 to `fps × 0.15`
(`0x10c46c`). The rewrite had read neither the miss (it went on at him
and rolled again the next update) nor the counter stance. Trick moves
in the first capture's comparison 133.8 to 126.3 (the original's 107);
the second 47.9 to 46.6 (31), the third 95.7 to 98.1 (81).

The trick moves' excess has a second side: in the 12..29.75 band the
rewrite's handlers read worse positional quality (3 or more in 12.7% of
updates against 16.3% in the first capture, 10% against 17% in the
third), and their nearest defender stands within 30 degrees of their line
to the rim 70% of the time against the original's 61..63%, beside or
behind them (90 or more) 7% against 12..17%: the original's handlers get
past their men far more often.

**BPJOG in at last.** The original's holder walking in a restart, not
backing down (`0x167580..0x1676c4`), asks 0x201 (game+0xf4 6), the one
record BPJOG (957): no 0x102, so no dribble, as the carry above. Its
motion is 0.0978..0.2999 an update, and the restart's walk by the stick
(`intensity / 64`) gives 0.218, 0.281 and 0.300 at strengths 38, 58 and
64: the captured inbounders' speeds to the third place (the no-ball
records gave 0.156 and 0.367 at 38 and 64). It had been held out for the
catches with 5+ updates left, which the cradle has now brought down: the
three attract comparisons pass with it (0.065, 0.062, 0.089 against
0.012, 0.082, 0.022).

**Open: the stop's motion, measured** (attract captures 1 and 3; stops
where the stick drops under 16 after moving and stays there 13 updates).
The original's slide over the next 12 updates splits by who stops: the
defence 1.1..1.8 (phase 5 1.7, median 1.55), a phase-5 holder 1.6..2.0,
the offence off the ball 0.4..0.9 with a median of 0.22, a restart's
holder 0.17. The rewrite's one deceleration gives everyone about 1.2
(median 0.89), and 2.3..2.7 in phase 3. A defender's motion runs exactly
the stand script's first blend (12 updates for breathe, face-up and
anticipation) and then stops dead: constant in his own frame and turning
with his facing (one trace: 0.37..0.50 an update in a circle while the
facing turns 34 a word an update), with a drift away from his man of
about 0.05 an update (the rewrite's: none). Every stand script opens with
`97`, so it is the channel's root, not the velocity; `FUN_0019a658` does
travel by a blend's mixed root (`0x19b3c0..0x19b56c`: the held clip's
root mixed with the incoming clip's by the weight, less the last), but
the disc's gait clips are in place (x and z within 0.5), so where the
held gait clip's travel comes from (a stride's root distance times its
frame, `record+0x18`, is the size of it) is not settled. Trying the
stands' roots with an instant stop: the offence's slide 0.51, the phase-5
holder's 1.51, phase 3's 0.7, but the defence's 0.60 and the cradle's
0.74; not taken.

**The side with the ball stops at once.** Split by who stops (the
attract captures, stick under 16 after moving, 13 updates still), the
original's slide over 12 updates is the defence's 1.1..1.8, a dribbling
holder's 1.6..2.0 and, for the side with the ball off it, 1.25 while a
pass is out (ball state 4, from the press, the windup included) but a
median of 0.2 otherwise (its stands carry next to nothing). The rewrite's
deceleration slid everyone about 1.2 (median 0.89). Now on the stand path
the side with the ball off it stops at once unless a pass is out (as does
a holder on his held stands); the deceleration stays for the rest.

"Inbound pos" stops a receiver within 1.4 of his spot at a jog; the
deceleration then carried him about 0.9 past it (ending 0.84 and 0.43 on
the baseline's and the sideline's side of it), where the original's stand
0.2..0.45 and 0.4..0.46 short of it on the side they came from. Now
0.52 and 0.34 short, 14.2 from the inbounder at the press (14.4), and the
catches with 5+ updates left read 0.017, 0.011 and 0.021 against the
captures' 0.012, 0.082 and 0.022 (0.065, 0.062, 0.089 before); the short
second capture's bound set again by the rule.

**Open: the swipes strip twice as often** (measured 2026-10-24). The
rewrite's held balls go loose about 16 times a match in four computer
matches (13.3 against 9 in the first capture's comparison on its
rosters), almost all swipes' strips. Matched on both sides: the steal
presses (144 against 134), the share of presses that start a swipe (0.71
against 0.68: the rest come in reactions, records 600..642), the swipe
scripts' mix (the middle swipe 57% against 55%), the distance at the
press (5.26 against 5.04..5.23) and through the swipe, the held ball's
height and offset (3.95 and 2.13 against 4.1..4.3 and 2.1), its side
toward the swiper, the defenders' time in reactions (6.7% against
6.8..7.8%), the odds (`FUN_0016cf30`: Steals, rating 6, × 64 / 99 + 20,
the Easy adjustments, the reach 1.5 or 0.75 by the swiper's own +0x40,
0.05 of it on records 398/399) and the test's once-a-swipe flag
(actor+0x23c). Not matched: the hand reaches the ball in 25% of the
rewrite's swipes where the original's strips imply about 12%; its
holders move 0.18..0.19 an update through the window against
0.21..0.23. Now ported: `FUN_0015da28`'s skip of the swipe test while
actor+0x1a8 is set (`0x15e4c0..0x15e560`: a bar from touching the ball,
or the update's own, off balance or behind his own board within 4.5 of
the offence's rim; a gamebreaker's ball coming down only bars with no
holder). The hand's reaches fall from 119 to 91 in the four matches,
the held balls knocked loose to 12.25, 6.0 and 10.2 against 9, 2 and 10
(15.0, 5.6 and 11.1 before), the turnovers to 7.4, 1.6 and 2.6 against
6, 0 and 3.

**The velocity's readers see the stand path's zero.** The original's
tests of a player's motion read actor+0x40 (`FUN_0013ab28` returns it to
the AI; the contacts, the catch rows' filter, the pass rows' filter, the
swipe's reach, the juke's test), and its stand path zeroes it at once.
The rewrite's stop slides him on (`tuning::DECELERATION`, in place of the
clips that carry the original's), so its readers took a stopping player
for a moving one for up to 29 updates. `Player::moving` is the speed but
not on the stand path, and every such reader asks it. iso3pt's attack
test read the pad's stick for `FUN_0013ab28`; it reads the velocity now.
The short capture's loose-ball updates set again by the rule. The rows'
replay of the captures (`row_capture`) takes a player's velocity as the
capture's stick tells it (16 or more: the moving path); so read, the
original's catch row is among the rewrite's candidates for 77 of 92
catches in the first capture and 63 of 72 in the third (43 and 24 when
every captured pace counted as velocity); the passes' 27 of 98 and 20 of
77 are unchanged.

**The pass filter's flight estimate.** `FUN_0015f930` measures a pass
on an estimate of its flight (raw `0x15fe4c..0x160104`): the passer 15
updates ahead on actor+0x40 (`trunc(fps × 0.25)`), the receiver 25
updates ahead when the pass leads him, else the aim's spot; the flight
their distance over the pass speed (`DAT_004b99c0` = 0.75196), at least
20 (`DAT_004b99c4`); a board pass times both legs, the second at three
quarters, and scales by the kind's ratio. The catch spot is the receiver
led the flight's whole updates (`FUN_001636c0`) or the aim's spot; the
sector is read from the passer the flight ahead (the float, and only
with a velocity that is not `DAT_004b9880`, the zero vector) to the
catch spot (`0x1600bc..0x160104`), the distance band from the passer 15
updates ahead to the catch spot (`0x1605e8..0x160604`; a board pass's
two legs through the point, `0x160610..0x16065c`). `timing::pass_reach`
carries it for the pass rows and the passout rows. The standing flag
(`0x160680..0x1607c8`) was already the original's: out of bounds within
10 updates XOR (speed at most 0.005 or the stick under 41), set only for
a grounded passer with a velocity and no handoff.

The rows' replay read the passer one update early. The filter runs in
the press update after its locomotion, by when the computer's pass node
has let go of the stick (+0x90 under 16) and the stand path has zeroed
the velocity: on the played captures every standing row the replay
refused (120) reads standing on the press update. With the passer's
stick and velocity from the press update, the velocity along the
captured +0x40 (it had pointed along the body word), and the release's
catch spot for the receiver's destination (the replay has no brains),
the captured pass row is among the rewrite's candidates for 333 of 451
played passes (254) and 50 of 98, 10 of 30 and 43 of 77 attract passes
(26, 2 and 21). The rest is the yaw (the root bone's, `FUN_00154230`;
the captures carry only +0xa8 and +0xb0, and neither nor any of the
passer and catch spots tried reads above 365 of 451), the band on a
catch spot read at the release, and the first attract capture's
missing ratings. `timing::pass_verdicts` names the test that keeps each
row out; the replay reports it per case ("why").

**The attract harness's meters, and what the node counts say.** The
capture tests' matches (`ai::capture::demo_match`) never set the meters'
fill, so both sides filled at 1.0 where `street-game`'s setup gives two
computer crews the pair at `0x43e298` averaged, 0.825. With it, the
rewrite's gamebreakers on the third capture's rosters fall from 25 to 16
in eight matches (the original: 2 in all three captures' 86,000 updates,
0.023 per 1,000 against 0.051), keep away from 2.7% of the updates to
1.6% (0.2%), trick moves from 3.29 to 3.02 per 1,000 updates (2.62).
Measured on the same rosters (the generic six skew the plays: iso3pt
looked 2.6 times too common), the play mix and the per-node trick rates
are close; drive runs 1.36 times the original's time, post up 1.5 and
run pick play half (pick and roll starts 0.37 per 1,000 against 0.74),
and carrier pause hands over to drive 20% of the time against 11%.
The original's receiver flickers between carrier pause, iso3pt and an
empty queue every update or two while a pass flies (`FUN_0013ab08`,
actor+0x70, holds for him), so counts of its nodes need the flight's
updates left out. Goal tend (`FUN_0012de70`, `FUN_00116fb8`) matches
instruction for instruction; the original's defence enters it after 12
of its 56 rebound strategies that reach the roll (a made dunk's lasts a
single update and never does) against the rewrite's 31% at the same
35..43% odds, within two standard deviations.

**The gamebreaker's start asks nothing of the turbo bar.** The rewrite's
own start spent 0.1 of the shooter's bar (and refused without it).
`FUN_00159cf0` (raw `0x159cf0..0x159ea4`, from the shoot press at
`0x15b1d0` with both turbos held, held word 0x40000) has no turbo test
or cost: it refuses while a gamebreaker task runs (`FUN_00140ff8`), with
the meter short of 50,000, for a player the computer drives on a side a
person plays (`FUN_00156d98` 4 with game+0x9a set), when
`FUN_00140960` (a person on the side and game+0x488 for it), in the
front end's mode 3 (`FUN_001e0bc0` +0xe8), and outside the game classes
whose vt+0xb4 answers -1, 14 or 25; then `FUN_00141080` picks the level
(the pocket's 2) and posts it. Ported: the task, the meter and the
driven player; the rest are front-end and class gates the rules already
stand for (`Rules::gamebreakers`).

**The stop is the stand's planted foot.** The stop's slide (defence
1.1..1.8 over the 12 updates after the stick drops, a dribbling holder
1.6..2.0, the offence off the ball about 0.2) is not the gait clip's root:
the locomotion clips run in place (records 28, 0 and 1 at their frozen
frames sit within 0.07..0.65 of the stands' first frames, where the
captured stops move 1.2, 1.8 and 3.4). In the played captures the channel
holds the gait clip's frame through the stop with flags 0x2a0000: 0x200000
the blend's held clip, 0x80000 planted feet. The stand scripts plant one:
breathe_1 (`0x4570f5`) runs `96 a7 97 91 0x820000 9b 0`, then `98
18 19 20 45 44 -> 0x457124` (a channel already on a stand's clip skips
the plant) and `95 f4 07 03` (the foot further ahead along the pelvis,
bones 7 and 3), blends 12 updates into record 65 and lets go (`96`)
before its loop; protectball plants `95 f1 f4 03 07`; the off-ball
offence's stndrdy/stndrlx plant nothing. While planted, the model posed,
he moves by the planted point less the foot's place (`FUN_00156f48`,
`0x157800..0x157954`), so the body swings over the foot as the pose
blends from the stride to the stance, a curve whose direction turns:
over the captures' 3,016 computer stops the path is 1.02 long and nets
0.24 ahead and 0.26 aside.

Ported: the stand compiler (`tests/trick_disc.rs`) reads `96`, `95` and
`98` (`Step::Unplant`, `Step::Plant`, `Step::OnRecord`, whose test before
the first blend reads the clip the stand started on, `Run::before`);
`World::stand_tick` plants on the model as the update before left it
(the gait's frame before the first blend) and holds the point
(`jump::World::plant_point`, shared with the move scripts); the stand
path sets no velocity (`tuning::DECELERATION` is left to the rewrite's
windup walk), and the side with the ball's own dead stop goes with it.
On the third capture's rosters the defence's stops now move 1.26 (median
1.19; nearly 0 before the plant ran), a holder's 1.17 (1.6..2.0) and the
offence off the ball's median 0 (0.2). The pick test's defender stands
1 farther off the holder's line: stopping at once he is a standing
opponent, which stops the walk. The first capture's defenders' standing
share (speed level 0, an AI decision) went to 0.414 against 0.376, as
the instant stop did before the plant ran; its bound is set again by the
rule. The handler's nearest defender 24..30 out now peaks at 6.5..7.5
(the original's 7.0..7.5).

**No catch row when none qualifies.** `FUN_001611d8`'s draw
(`0x162374..0x162404`) with an empty list (the count −1) draws nothing:
it clears +0x1bc, keeps the flight's count on the receiver (`0x1623a4`
jumps past `0x162a4c`) and sends the pass on. The rewrite fell back to
row 0 (`catch_stand_1`), which held a grounded receiver; the list holds
row 0 alone only through the spot run (`DAT_003d70b4`). Left: on the
third capture's rosters about one pass a match is lost at its timer
(the original's 1, 0, 0 in its three captures): a spot run's receiver
stops 1.8 from the spot (within 2 when moving) while the pass goes to
his hand posed on the spot; catch_stand_1's root travels 0.1, so it is
not the catch clip that carries him. (2026-10-25: the radii were the wrong way
round; "The spot run's radius".)

**The stand clips' roots carry their players.** A script's `91`
(`FUN_00195858`) with 0x1000 keeps the clip's root off x and z
(`FUN_0019a658`, `0x19b4bc..0x19b70c`); none of the 58 stand scripts
sets it (their `91`s are 0x0, 0x20000, 0x20002, 0x820000, 0x820002,
0x820040 and 0x820042; `tests/trick_disc.rs`' stand compiler now keeps
them as `Step::Flags`), so the stand clips' roots move their players as
the reactions' and the swipe's do. The idle clips sway on the spot:
`stndready_noball` (68, `0x4570bb`) swings its root 0.55 across over
its 41 frames, and the second capture's inbound receiver (frames
2673..2742) swings 0.3 either way of his spot on it. The earlier trial
of the roots ("The stand path") ran them with an instant stop; with the
planted feet holding the stop, the roots now play (`World::stand_tick`).
Standing players off the ball (15 updates on, phases 4 and 5, the ball
held) now drift 0.020 an update on the offence and 0.017 on the defence,
moving in 87% of the updates; the original's 0.022..0.025 and
0.017..0.024, 88%; main before 0.0003 and 0.0015, 0.1% and 1.5%
(`ai::capture::standing_players_sway_on_their_stand_clips`). The
backdown's step indices (`backdown::INTO_DRIBBLE`, `BUMP_STANCE`) moved
with the `91`s.

The early catches (`catches with 5+ updates left`, the second capture's
5 of 61 against the rewrite's 0.6%) are mostly the back court's inbound
pass caught on `catch_stand_1` with a flight of 20: the original's
receiver stands 7.4 from the ball at its release (he sways, then steps
0.9 toward the ball through the inbounder's windup), the rewrite's 8.5
to 9.6, so its ball is 1 to 2 updates later in his hands. The row draws
match (`catch_stand_1`, `3l`, `3r` alike for a ball at the face); the
spots differ by the way each receiver walked in from where the basket
left him, and the inbounder stands 0.8 farther out (45.7 against 44.9).

The passes knocked loose (the original's 2, 1 and 1 against the
rewrite's 0.25, 0.12 and within bounds) are block jumps at a high ball:
Back 2 Papa off the board (flights 73 and 79) and an alley-oop lob. The
first two captures' comparisons run every player at 50 (their rosters
are not known), and a trick pass needs Handles 70 (`FUN_00182d10`), so
neither throws one. On the third capture's rosters (eight bases) the
rewrite throws six board passouts and eighteen lobs in 96,000 updates,
against the original's three and five in 26,000.

**The stops by role, with the roots.** Measured the same way on both
sides (the stick from 16 or more to 0, held 14 updates on the floor, the
slide over 12 updates, phases 4 and 5; the rewrite on the third capture's
rosters, four bases of 20,000 updates): the defence's stops slide 1.31
(median 1.27) against the original's 1.25..1.39 (1.22..1.30), the
offence's off the ball 0.52 (0.24) against 0.37..0.74 (0.23..0.29; main
before the roots: median 0), the holder's 1.09 (0.89) against 1.29..1.44
(0.72..0.77). The rewrite's holders stop about twice as often (4.8
against 2.7 a thousand updates), which is the computer's stick.

**The pick and roll's share.** On the third capture's rosters the play
starts come as often on both sides (3.9 a thousand updates), but the
rewrite's penetrations with the lane open at the choice
(`playbook::lane_penetrates`) run 1.05 against 0.61, and its pick and
rolls 0.26 and 0.10 (3pt) against 0.42 and 0.32. At the choices the
rewrite's lists hold the pick and roll in 74% (one entry on average),
the original's, read with an empty meter, in 90% (1.22): the meter mode
drops it (`meter_ok`). The third capture records the meters' values
(`meters[k].value`): full 5.4% and 4.7% of its updates, first at updates
14,334 and 15,096, three gamebreakers; the rewrite's on its rosters
0..11% (about 5%), first at 7,846..20,082, one or two a game, so the fill
is the original's. The meter mode (`[0x3d]`) is not recorded: it is drawn
at each possession's start from the catch-up table by the score
(`Playbook::start`) and set by a full meter, so it follows each game's
score; the rewrite's sides spend 5..46% of their updates in it.

The handler's lane (`FUN_0017f4d0(ai, 0)`) is open as often on both
sides: in every third held update of phase 5, 27.5% of the original's
(the third capture, its players placed in the rewrite's world, a
defender in a reaction script counted down) and 27.6% of the rewrite's
four games on its rosters, zone by zone alike (the perimeter's zones
1..3 13..27% against 17..21%, the paint's 5 and 6 57% and 50% against
50% and 46%). Read without the reactions down, the original's lanes look
half as open (18.9%), which is what the play starts above were read
with; the pick plays' share is not the lane's. The original's trick
moves let go (eight in the third capture, the rewrite's 2.4) are pick
plays started in the update a juke runs: the pick and roll's and the
pick off the ball's first steps hand the ball off (`FUN_0011ea68`,
`FUN_0011f940`) with no juke test, as the rewrite's do, so they follow
the pick plays' share.

The pick plays' share, read raw once more: the list builder
(`FUN_001281c8`) checks each play through the playbook's own method
(vt+0x6c, `FUN_00128668`: the play's suit test vt+0x64 for a named
player, the last play's object `[0x24]` left out, and in the meter mode
the play's vt+0x5c), and vt+0x5c answers the object's variant (+0x28)
for the isolation, the pick and roll and the pick off the ball
(`0x11da28`, `0x11f490`, `0x1200b8`), 1 for the pull up, the pen and
kick, the rotate and the post and kick, 0 for the rest: the rewrite's
`check` and `meter_ok`. The extra entries (`FUN_00128708`: 7 for the
penetration and the isolation, 4 for the back down and the post up out
of the meter mode, the 3pt isolation twice off Easy, the 3pt pick and
roll) and the default table (`0x3ce438`) are the rewrite's, and the draw
is `rand() % n` (`FUN_00128a68`). On the third capture's 82 list choices
the rewrite's lists at the original's states give at most 15.7 pick and
rolls (the meter mode taken either way at each, whichever gives more)
against the 23 it chose, about a 3% draw: left as the match's.

**Edit Baller's build and size prices** (2026-10-24). `FUN_001e6080(obj,
row, value)` prices an edit row through the jump table at `0x4601b0`
(rows 0..10: `FUN_001e2470`, `001e28c8`, `001e30d8`, `001e3440`,
`001e3720`, `001e3a68`, `001e3d90`, the outfit `001e4580`, the shoes
`001e4e30`, the skills `001e5138`, `001e5850`). Row 0, the build, reads
the gender's table of `0x4602a0` (`{count, rows}` pairs: the men's four
builds at `0x460368`, the women's two at `0x4603a8`, 16 bytes a build,
its price the first half-word: 0, 200, 300, 400 and 0, 200) and charges
the new build's price less the current one's. Row 1, the size, counts the
current build's sizes (its byte +2) and reads that build's size rows
(its +8, 6 bytes a size, the price first) for the new size less the
current one (`FUN_001da4e0(player, 31)`'s size). Every build's sizes cost
0, 200, 300 and 400, and the rewrite's editor keeps the size's index
across a change of build, so its difference against what was paid
(`ballers::price`) charges what the two rows charge. Rows 2..6 (the
looks: `0x4602b0`..`0x4602f0`, 4-byte rows for row 2, 2-byte rows for
rows 3..6, rows 5 and 6 men's only) price every entry 0, so the looks and
the hair are free as the rewrite has them. The buy (`FUN_001e4af8` for an
outfit) takes the points at each Yes, refused when the career's points
(+0x30) fall short; the rewrite takes them at Save Baller, which is
Edit Baller's only way out, and asks the poor question when the points
do not cover a buy, so both end with the same points and wardrobe.

### The rebound's clearout and the dribble's start (2026-10-24)

**Callback 37** (`FUN_001774f8`, entry 37 of the callback table at `0x3e86d0`): every
rebound script but the goaltend pair's calls it as its tail ends (after
`9c 0x100`, before callbacks 7 and 1). With the ball in his hands
(actor+0x70) and two or more of the other side (`FUN_00140920(game, side
^ 1, k)`, k 0..2) within 5 of him on the distance table
(`FUN_0016e948`), it starts `clearout01` (`0x456d02`) on his channel
(`FUN_0019c2f8`), with no juke and no turbo. Seven of the attract
captures' ten clearouts are these (the clip goes from the rebound's 419..426
to 658 with no press on the pad); the other three are the juke press's
(`FUN_0015c778`), two of them by a post man 7 updates after his catch,
pressed as the catch's bit 9 clears. Ported (`World::rebound_landed`,
`World::start_clearout`). On the third capture's rosters 11 of the
rewrite's 49 rebound jumps ending with the ball meet two men within 5
(the original's 2 of its 11); its clearouts went from 1.25 to 2.0 a game
against the original's 4, the rest of the gap being its rebound catches
(about 6 a game against 11).

**The dribble starts with a record's script** (`0x167eb0..0x167f54`):
`FUN_00163d40` sets +0x1a0 to 1 only where it starts a new record's script
(`FUN_0019c2f8`) with the ball (the request's 0x102), not when the record
chosen is the row already playing (+4 or +8 of the current record) nor
under a channel with 0x10; the rewrite set it on every update the request
carried the ball. Now set where the record starts anew.

**Where the shots go** (2026-10-24). The original's three attract
captures hold 121 shots: 7 knocked away in flight (5.8%, five of them by
the rim, block jumps on clips 446 and 441, mostly half through the
flight), the rest scored or on the hoop, none an airball; every one of
the third capture's 11 rebounds was caught as a rebound (6 by jumpers in
the air, 5 on the floor). On the third capture's rosters (eight games)
the rewrite shoots 40.75 a game (43), scores 21.5 clean (23), puts 13.5
on the hoop (17) and has 5.75 blocked (3), all block jumps; its 8.4
rebounds are caught as reliably (8.1). Its block jumps come as often
(47.1 a game against 47), but at a shot in flight script 5 (446/447)
takes 9.5 of its 13.9 against the original's 5 of 13, which spreads over
440..455: the kind (`block::kind`) agrees with the original's at its own
states (45 of 47), so it is where the rewrite's defenders stand at the
shot.

**The goal tends, an open difference.** While a shot flies the
rewrite's defenders goal tend four times as long as the original's
(6.3% of phase 3's defence against 1.6%): it deals 0.23 a shot (39 in
166 on the third capture's rosters) against 0.11 (14 in 123 over the
three captures), each lasting about as long (median 30 updates). The
dealing is read raw as ported: the defence's rebound strategy (vtable
`0x43b2b8`, start `0x12de60` setting [7] to 3) deals in its update
(`FUN_0012de70`) at [7] 1, two updates after the shot, every one of the
original's four coming then; a full meter without a gamebreaker shot,
a person's side and game+0x9c 4 (1 in the attract) skip it; the odds are
`skill(0, 0, 50, 80)` (`FUN_0018ab18` = `FUN_001409c8`), `(0, 0, 10, 40)`
while the gamebreaker task is busy (`FUN_00140ff8`: game+0x434's +0x2c);
the man is the first of the side's list at game+0x3ec (`FUN_0013f8a8`)
not in a block jump (`FUN_001818b0(ai, 0)`: bit 12) nor down
(`FUN_00181890`: channel 0x40), given the node by `FUN_0013aaa8`
(clear and push); the node (`FUN_00116fb8`, `FUN_00117010`,
`FUN_001171b0`) asks for the block within 5 of its spot with 20 updates
left or less and ends then. At a tied score that is about 35% a shot.
But in the third capture the original deals none in the 25 shots whose
defence's meter stood under 29,000 and 4 in the 18 over it (meters[k]
.value), which nothing read here explains: the meter's only read in the
dealing is the full test.


### The shown buffer: 16-bit and dithered (2026-10-24)

The display reads a 16-bit buffer. In every GS dump taken (`claude-l2look`'s
seven, `live.gs`, `slot1.gs`), the privileged registers hold PMODE `0xff21`
(circuit 1 only), DISPFB1 at block 0 with PSM `0xa` (PSMCT16S), 640 wide,
and SMODE2 1 (interlaced). The game draws its frame into a 32-bit buffer at
`0x8c000`. The frame's last draws are forty 32-pixel sprite strips from that
buffer into the 16-bit one: PRIM `0x15e`, TEX0 `0x2680288c0`, ALPHA
`0x44` with the vertex alpha, and TEST `0x30002`. Their alpha and colour
are 0x7f normally, and 0x30 and 0xa0 under the gamebreaker's blur. Every
strip is written with DTHE 1. The state's DIMX, the field after FOGCOL in
the dump's GS state (version 9: PRIM, PRMODECONT, TEXCLUT, SCANMSK, TEXA,
FOGCOL, DIMX, DTHE, COLCLAMP, ...), is `0x2637405137265140`: rows
y & 3 of (0, -4, 1, -3), (-2, 2, -1, 3), (1, -3, 0, -4) and (-1, 3, -2, 2).
Sony's library default differs. COLCLAMP is 1. No other draw writes the
16-bit buffer, so the HUD and the front end go through the copy too.

So each pixel shown is the blend `(Cs - Cd) * As >> 7 + Cd`, plus the
matrix's step for its x & 3 and y & 3, clamped to 0..255. The store keeps
each channel's top five bits, and the display reads them back with the
low three clear (the GS's 16-bit expansion, as PCSX2's software renderer
does it). The original's PCSX2 grabs carry the matrix's mark: differences
between pixels 4 and 8 apart dip below those 3 and 5 apart, on every grab
checked. The rewrite's grabs rose with the distance until now.

Built (`street_render::screen_fx`, `screen_copy.wgsl`). The copy is now the
original's (0x7f, 0x7f) rather than the frame as drawn. It runs on the
camera drawn last to the window on every screen, the front end's 2D
cameras included (`street_game::screen_fx::show`): only one camera, or the
dither would go over the picture twice. The dither falls on output pixels,
exact at 640 wide; at larger windows that is PCSX2's default "unscaled"
dithering. On average the picture is about 4 levels darker, as the
original's is: the matrix averages -0.5 and the store drops 3.5. The
original's grabs cannot confirm that by brightness, because the rewrite's
grabs from 2026-10-19 of the same screens differ by more (8 to 10
levels). PCSX2 filters its window bilinearly and its grabs scale 448
lines to 480, which together soften the matrix's mark in them.

**Not settled here:** the display's handling of the interlaced fields
(the CRTC reads the 448 lines a field at a time), and whether the 5-bit to
8-bit read-back replicates the top bits instead of clearing them: the
grabs' share of channels ending in three zero bits (0.32) lies between
the two models once their scaling is simulated.

### Tricks let go into a pass: who deals the pass (2026-10-24)

The third capture's eight tricks that end in a pass are not passes out of
a trick script. In each, a play one or two updates old deals the holder a
"pass" node (vtable `0x437448`, `FUN_00100a08`) while his trick bit is
still set. The play was a pick and roll (`0x439dd0`) or pick off the ball
(`0x439f80`) four times each by `play_head`, the pull up once. The node
presses on its first update and the trick ends with the release. The
pattern: the holder's queue empties, the role pick gives him "carrier
pause", whose first update jukes (`FUN_00101b50`). Carrier pause within 12
of the rim ends the running play (`FUN_001285d0`). `FUN_001281c8` starts
the next one, and a pick play's start hands the ball off at once
(`FUN_0011e900` → `FUN_0011ea68`, `FUN_0011f628` → `FUN_0011f940`).
Neither start tests the juke.

Every other dealer read here does test it: `FUN_00182c58` (the playbook's
pass check, `FUN_00128b30`) and the fast break, drive, iso3pt and shoot
nodes call `FUN_001819f8`, which answers yes on actor+0xa4 bit 10 first.
The off-ball call (`0x158f50..0x159098`) needs a caller who is not
controller 4, and every attract actor is. Keep-away needs a full meter
on the other side. The holder lookup `FUN_0017d550` asks the ball's
holder (ball+0xf4) for vt+0x44, which returns the actor itself (vtable
`0x43f568`, `FUN_0017c588`, read from save state 1's RAM).

The rewrite deals the same way: its pick plays' starts hand off without
a juke test, and in four games on the third capture's rosters each of its
7 mid-trick pass nodes let the trick go on the next update. It starts
plays while the holder tricks as often or more (153 of 826 starts, 19%,
against the original's 22 of 204). But pick plays are 18 of those 153,
where they are 9 of the original's 22, and penetration takes 88. So the
let-go gap (2.4 against 8 a match) follows the play mix, the pick and roll
at half the original's rate ("The computer's offence", attract-measuring),
not a missing rule.

### Level 2 shots: the shooter's own launch (2026-10-24)

Vj: in a Level 2 "the ball goes straight to the basket", and the dunks
look wrong. The staged ball's shot command (opcode 2) was launched in a
fixed time, `max(cue frames, 15)`, and every plan's shot cue lasts 1 to
10 frames, so each staged shot flew a quarter second along a nearly
straight line. That timing is `FUN_00334330`'s: it reads the record's
destination words (+0x14, +0x18), and with `t = max(duration, 15)` solves
the vertical speed to land the ball on the floor (`−(½·g·t² + y)/t`). It
is a toss to a point, not a shot.

The shot is the player class's vt+0x12c (vtable `0x49cba0`, read from
save state 1's RAM: +0x12c `0x333ef8`, +0x134 `0x334088` the vertical
launch, +0x13c `0x334178` the finishing VM). `FUN_00333ef8`, with the
ball in the actor's hands, bars him (actor+0x1a8 = fps / 3), then calls
the launcher `FUN_0014aaf8` as a gameplay release does. Its arguments
come from the record: made unless word 8; the aim kind word 12 when it
is 5 or less, else 0; the angle 67.5° (`0x3f96cbe4`) with word 16 of 1,
else `π/4 + π/8 × max(0, (20 − d) / 20)`, with d his distance from the
rim (`FUN_001404e0`'s x, z 0) across the floor. The fourth argument is
`FUN_00141038`'s band of the running script (−1 or under 5: 3; 5..12: 2;
over: 0), which the launcher only stores in the basket's record
(ball+0x15c by side). Every shipped plan's shot record has words 8, 12
and 16 at 0, so all sixteen are makes straight at the basket at the
distance's angle. The launcher then takes its dunk path at the hand's
pace when the shooter's state has bit 7 (`0x14ad14`), as a gameplay dunk
does.

Built: `World::staged_shot` launches through `launch_shot`, from the
staged hand, the angle from the staged pose's place, and the dunk path
when his staged clip's state word has 0x80 at that frame
(`gamebreaker::Config::clip_pose`, the pose bank's `state`); a staged dunk
scores as a Level 2 dunk. The flight now takes the launcher's floors, so
a low release can outlast its script; the ball then flies on and scores
as the Level 2's.

Checked against the School's sixteen Level 2 runs (2026-10-24,
`codex-street-recovery-20261022/level2-batch/*-rich.json`, a sample
every few updates). Families 1..8 end in a shot (ball state 5), 9..16 in
a dunk (the ball held through the finish, then two updates of flight).
Every captured shot is a make straight through the aim (the rim's x,
z 0, height 11.956775), so its samples fix the arc. Fitted against the
ball's timer, its release is one update before the timer's start
(`flight_updates`), as a pass's is. Two families steepen into the rim's
entry (`steepen_entry`); the other six fly at the actor's own angle,
45° for the three at 20 or more from the rim, 47.1° to 48.0° for the
three nearer.
From each release the rewrite's angle rule, steepening and flight time
give the original's updates to the aim within 0.07 (all but one within
0.01; the one's distance is from a sample a few updates before the
release). The test is `staged_level_two_shots_fly_the_originals_arcs`
(`STREET_L2_RICH`).

### Level 2 dunkers on their finishing bodies (2026-10-24)

Vj: the Level 2 dunks still didn't look right. The staged dunker never
left the floor. A dunk plan's opcode-1 cue starts the dunker's finishing
body (`FUN_00334178`, rows at `0x415740`: `gbdunk_1_01_3_mo`,
`gbdunk_1_02_10_ho`, `gbdunk_2_04_5_mo`, `gbdunk_2_02_8_ho`,
`gbdunk_3_01_5_ho`, `gbdunk_3_03_6_ho`, `gbdunk_4_01_1`,
`gbdunk_4_02_11_mo` for plans 0..7). The body runs on the actor's own
channel and physics. Its `9e` aim sends his hand to the rim, gravity brings
him down and a landing wait ends it, like a gameplay dunk's script. The
port read only the body's clip waits, so he played the dunk clip standing
up. In the School's run of script 1 (`level2-batch/case09`) he rises from
frame 100 to 7.86 above the floor at 137 and lands at 177.

Built: the eight bodies are move rows (`timing::FINISH_ROWS`, their steps
generated into `held_tables.rs` and checked by `trick_disc.rs`), and a
plan's finish carries its row. From the cue's frame the sequence hands the
role over (`Sequence::finishing`): his action is a dunk on the body's row,
`flight_tick` moves him, the ball sits in the hand his clip shows
(`place_held`), the script's poses leave him and the view draws him as a
dunking player. The script's shot command still releases the ball. It
falls on the update of the body's callback 21.

Three more places the dunk needed:

- The lob to a flying receiver. `FUN_0032fe28` (word 16 of 0) aims at a
  dunker's or a leaper's hand (actor bits 7 or 15, `FUN_0016e9d0`,
  `FUN_0016e9e0`) through `FUN_00330f98`: his model posed at the frame his
  channel shows by the pass's end, on his place and wanted facing, the
  hand by the clip's flags (bone 19, 15 or their middle), moved on by his
  velocity and `½·g·n²` over the updates, y never under 12.5
  (`World::lob_endpoint`). Any other receiver goes through `FUN_003301b8`.
- The ball's motion. The staged update now keeps `Ball::moved` as the
  game's update does. The dunk path paces itself by it, and with a stale
  value the dunk left at the launcher's slowest pace (0.244 an update, the
  original 0.489).
- The ball through the net. A loose ball falls on its own update during
  the sequence (`FUN_0014a968`). It used to hang at the rim until the
  script ended.

Script 1 against the School's run: the dunker within 0.15 across and 0.3
up at every sample, the lob within 0.3, the catch and the carried ball
within 0.2, the basket on the same update, the ball through the net
within 0.01 to 0.45. Test: `every_staged_dunker_flies_on_his_finishing_body`.
The other runs, matched by script header, now fly too, within 0.4 to 1.8 of
the original's heights. Not yet the original's: bounce passes (a pass
cue's word 12 of 1, plans 0, 4, 5 and 7, which go through the floor in
the original), `FUN_0032fe28`'s waypoint path (word 16 of 2 is a whole
(x, y, z) point from words 20..28, the port reads two), and script 2's
dunker, who leaves the floor about 15 frames before his body starts.

Since (2026-10-25):

- Bounce passes. `FUN_0032fe28` keeps each pass record's word 12 in its
  waypoint entry (+0x18), `FUN_001488e8` hands it to the leg (+0x28) and
  `FUN_0014c0c0` launches with it as the bounce flag: the floor once on the
  way (`FUN_0014f728`'s first contact, `self_toss::solve_bounce_time`, the
  floor's 0.8; the captures show the speed across kept and about 0.8 of
  the rise). Plans 0, 4, 5 and 7 bounce where the School's runs do;
  script 7's lob leaves at 9.65 (the original 9.37), meets the floor about
  frame 61 and is caught at 80 at 16.95 (17.40).
- The board-disable record (`FUN_00148760`). A leg to the backboard (word
  16 of 1) carries its side (+0x14, `FUN_001404c0`); while it or the leg
  before is such a leg aimed 13.25 or higher, the backboard, collar, rim
  and basket on that side meet nothing. Script 4's bounce to the board
  passed through the rim's ring in the original (14.0 at the board at
  frame 58, ours 13.95) and on to the dunker; ours had hit the rim and
  dropped loose (`Ball::fly_pass_with`). Flown straight, script 7's lob
  had done the same.
- Word 16 of 2 reads words 20, 24 and 28 as the point (`0x330060..`).

`every_staged_dunker_flies_on_his_finishing_body` now runs all sixteen
plans, the ball with the first pass's or shot's thrower, and fails a ball
loose before the shot; each scores once (a shot that outlasts its script
scores as the Level 2's). Still open: script 2's dunker leaves the floor
about 15 frames before his body starts.

## Level 2's picture against the School's grabs (2026-10-25)

The School's Level 2 alley-oop (script 3, fired from (-24, 1)) was set
beside our run frame by frame (the original's grabs bracketed by script
frame, ours by the game's update and the sequence's frame and freeze
count). Positions, the cameras and the freeze at 70 (its white flash and
close-up) agreed. Three things did not:

- The court's shade. The courts carry their lighting in vertex colours,
  which the GS multiplies into the texel in its own display values
  (`texel * colour >> 7`). We multiplied in linear light, which lifts a
  0.5 shade to about 0.73 on screen. The building's shadow across the
  court and the light on the wall were nearly gone. `convert::mesh` now
  passes the factor as sRGB, as the crowd's tint already did. This
  applies to every court, prop and player mesh.
- The ball's sparks and streamer. `0x14a968` sets the ball's +0x1a0 to 2
  while the game's gamebreaker word (+0xf4) is on and not 9. It stays on
  through the staged script, so the white sparks and the flag streamer
  follow the ball from the burst to the basket. `ballfx::glow` only
  counted a gamebreaker shot. It now counts `World::sequence` and
  `World::staged_start` too.
- Root travel drawn twice. The script's curves move each actor's proxy
  along his clip's travel. Script 3's passer is 11.6 along by frame 108
  while his clip's root has gone 9.1 at his 1.27 scale. The original
  draws the body on that proxy. We also played the clip with its root's
  travel, so a moving actor ran ahead of his place (the passer was drawn
  past the rim where the original's catches the lob). Staged clips now
  play in place,
  keeping only the root's rise and fall.

Still differing: the crowd stands at the wall in our School game (the
lesson has none), the backboard is about 15% darker, and the original's
motion ghosting on fast actors is stronger.

## The finishing body's lift (2026-10-25)

A second capture from (-24, 1) drew script 2. Its dunker held at 1.85
from frame 87 (his last height key) and then rose slowly on his body,
peaking 0.8 below the original's. The School's sample shows the lift
directly. At frame 94 the staged clip 982 carries his root 2.0 above the
body's (local root y 6.64 against 4.64). At 98, the body's first update,
his height is 4.03, and the root is back at rest on clip 984. The body's
script (`0x44b62a`) turns on the channel's resync flag (0x10000) in its
first step, so its first frame lowers him by the root's rise from the
clip the channel held. Coming down from 982 to 984, that lifts him.

Two faults hid it. The body's channel started from the gameplay clip,
not the staged one: `step_staged_finishers` now hands it the clip and
frame the staged channel showed the update before (the record changes
clip on the body's first frame too). The script's first update shows no
clip yet, and `flight_root` cleared the resync there. An update showing
no clip now takes no root step and keeps the flag, and a step from no
clip measures from the held one. Script 2's dunker now leaves at 4.03
and peaks at 9.62 on frame 130 (the original about 9.6 at 129;
`every_staged_dunker_flies_on_his_finishing_body`). The other plans are
unchanged.

The capture's cast differs from the School's earlier runs. The person
shooting was Shaq, and the Nets had Van Horn (101) for Mutombo. The plans
cast by place and shooter, so a frame-by-frame comparison needs the same
six at the same spots.

## The held ball's hand, the shorts, the tip-off's freedom (2026-10-25)

- The staged held ball. `attach_hands` built each clip's hand track with
  the root's travel, which the drawing had dropped the same day (the
  script's curves already move the actor along it). Script 2's ball sat
  7 beyond its holder and left on its pass that far ahead. The tracks
  are now in place. In the staged test the ball is at (-23.37, 6.87) at
  frame 71 (the capture -23.57, 7.28), and off the board in the dunker's
  hands at 123 and 133 within 0.15 of the capture.
- The shorts. The model loader draws the shorts from the C library's
  generator, `rand() % count` over the team's sheet (`0x315cb8..0x315cf0`,
  `FUN_003b42a0`). The rewrite always took the first pair. `load_match`
  now draws one per player from the session's `Libc` as each model loads
  (`player::look_dressed`).
- The tip-off. Players move while the toss is up, as in the original.
  `FUN_001587d8` reads the person's pad only when actor+0x298 is clear
  and his controller is not the computer's (`FUN_00156d98`, game+0x24).
  `FUN_00140880` clears the word for all six once the ball is in state 7.
  While the referee holds the ball (state 3) the actor runs his AI frame
  instead. In the opening capture of a played Pick Up game nobody moves
  more than idle sway until the jumpers' scripts (378 to 380, 376 to 384
  at frame 168). The rewrite freezes during the hold and frees at the
  toss, and its jump falls on update 167.

## The tip-off's timing (2026-10-25)

Vj could not win tip-offs. `FUN_0015eaa0` (`0x15eb5c..0x15ebfc`) takes the
toss's apex as `y + vy t - g t^2 / 2` at `t = vy / g`, with `g` the ball's
gravity size per update (ball+0x118, positive). The port used the signed
acceleration, so the apex fell below a rising toss. A press any earlier
than the last update or two before the top scored below 1, was clamped to
1, and locked the side's timing. A person then lost to the computer's
`r % 20 + 1` every time (a tie goes to team 0). The old test pressed
within a hair of the apex, where the error vanishes. With the sign right,
a person on team 0 pressing 20, 15, 10 and 5 updates before the apex
scores 9, 14, 18 and 20, and wins 16, 28, 36 and 40 of 40 seeded tosses.
A press below the window (before the toss is in the top fifth of its
rise) sets nothing, so a later one still counts
(`a_persons_press_scores_by_how_near_the_apex_it_falls`).

## The legs' clock and the standing facing (2026-10-25)

Vj saw players almost gliding, and the computer's players flipping.

- The gait clock. `0x167cd0..0x167ce8` takes the frame step as the motion
  over the record's root distance (+0x18) times the skeleton's first
  scale (`FUN_0019cd48` on actor+0x108, 1.2 to 1.3 for most). The port
  used `height_ft / 6.5` clamped, about 1.0, so the legs cycled a fifth
  too fast for the ground covered. The step now uses `Player::body`'s
  height scale. One measure: the planted foot's travel in the clip
  against the body's over the same updates, for every gait clip in the
  two pads captures and in an AI match with the same six players' scales.
  The original's run_1, run_3, BPJOG and RUN_CYCLE_WBALL_04_2_R show 0.52,
  0.43, 0.52 and 0.61. The rewrite showed 0.64, 0.57, 0.66 and 0.78
  before, and 0.55, 0.50, 0.54 and 0.60 now. Walks: walk_1 0.80, now 0.83.
  (`the_gait_clock_runs_by_the_skeletons_scale`)
- The standing facing. The view turned every standing player without
  the ball to face it, and snapped him back to his own facing when he
  moved. The original draws the actor's facing. In its played captures
  a standing player is 26 degrees off the ball at the median and past 90
  a quarter of the time. The simulation's facing gives 39 and 26%, and
  turns at most 46 units an update (7 larger changes in 4,000 updates of
  an AI match). The view now draws the simulation's facing.

The gait clock's change moved two seeded checks:

- A restart's scoop never took the ball. `World::touch_ball` lets the
  restarting side start a scoop on a loose ball in phase 6 (`0x15c3c8`,
  no phase test), but `scoop_tick` gave it only in live play. Callback 0
  (`FUN_001775d8`) takes it whenever nobody holds it. The scoop now takes
  it in a restart for the restarting side too. In the School's replay
  test, lesson 10's third relaunch had an inbound go uncaught and roll to
  a corner. The restart now ends at update 850 instead of waiting out the
  rewrite's 20-second limit. The test's window is now that limit less an
  update.
- The second capture's standing share is 0.430 (se 0.007) against 0.398,
  the seed sets +500 and +777 0.433 and 0.425 (0.425 before). The bound
  is set again by the rule at 0.055.

## The drawn dribble (2026-10-25)

The simulation has carried the held ball as `FUN_0014e060` does since
2026-10-16 (`street_sim::dribble`, checked against the played captures'
ball). The view kept its own stand-in for a holder on his gait, a bounce
of `|cos(pi phase)|` between the floor and his hand, once a clip cycle.
It now draws the simulation's ball for any holder whose channel the
simulation runs, the gait's dribble included. The bounce stays only
without the disc's animations.

The gait clock against all 23 played captures (`staged.rs` `gait_clock`):
each actor's frame advance over the production step stays one constant,
update for update (99.5% of 324,382 within 0.001 of his median), and the
constant is 1 to 1e-6 for every actor, his record found by the
persistent id (field 0) the capture names. That id is not the record's
index for all: the generic ballers' is their record less 11 and
Mutombo's (record 81) is 101. Taking it as an index had given the story's
generics and the School's actor 0 other heights.

## The blur on a fast display (2026-10-25)

`FUN_0030fce0` copies the drawn frame over the shown one once a 60 Hz
frame, at alpha 2880 / fps (48) and colour 0xa0, which leaves the moving
players' ghosts. The rewrite blended every frame it drew. On a display
faster than 60 Hz the picture changes only with the game's updates, so
the ghosts faded two to three times faster (at 144 Hz, 0.32 of a trail
left after a 60th of a second instead of 0.625). A frame drawn with no
new update since the last now shows the last picture again
(`ScreenCopy::hold`), so the blur blends once an update.

## Screens, the strip's knock and the swipe's clock (2026-10-25)

A demo match logged the actions the view still drew with its own clips
and fades: swipes (32 in four minutes), stumbles (9) and a pass. The
stumbles were the rewrite's own.

- **The screen** (`0x135400..0x135640`): the screener plays
  `0x3d0fb0[r % 3]` (`0x44c211`, `0x44c26a`, `0x44c2c3`, the setroughpick
  clips) with `FUN_0019c2f8`; their callback 16 (`FUN_00175720`) sets his
  wanted facing toward his nearest opponent (`FUN_00170dc0`), then an
  `a6 2 1` window (the clearout's arms) stays open while the clip plays.
  The defender plays `r %` the list at `0x3d0f28` (heavy: 5/3/4/3) or
  `0x3d0f38` (light: 1/2/1/2) by the quarter the screener hit him from.
  Nine of those twenty scripts were not yet among the rewrite's reactions
  (`street_sim::trick::SCREENED_HEAVY`, `SCREENED_LIGHT`, `SCREENER`). The
  rewrite had stumbled the defender for 1.3 s on a clip picked by his
  index and played nothing on the screener. Every one of these scripts
  sets channel 0's flags 0x42 (`91 42 00 82 00`): off balance, no
  locomotion, as `down()` reads any reaction.
- **R3**: the original's R3 only calls the side's strategy (vt+0x44,
  `FUN_0018a708`, the AI's "set pick"). The rewrite also ran its own pick
  (`World::strategy`: the nearest teammate sent at the handler's defender,
  who stumbled 1.3 s on touch), for computer holders too through the
  strategy bit. It is gone; screens come from the contacts alone.
- **The strip's knock** (`0x16d3e0..0x16d7d4`): a holder on the floor
  (actor+0x34 0) plays `0x44ca8b` (`steal_reaction_l_1`, record 397 or 396
  by his dribble hand) whether the swipe takes the ball or knocks it; a
  knock bars him `r % 5 + 25` updates (actor+0x1a8) and the swiper `fps ×
  0.84` (`fps × 0.17` in lesson 16), then draws a spin axis and rate (four
  draws, as a block's knock) before `FUN_0014c408` frees the ball. The
  rewrite had stumbled the holder for the bar's length and drawn none of
  the four. Not ported: the message `0x3c64e0` to the swiper's
  controller.
- **The swipe on screen**: the view drew the swipe's clip with its own
  0.1 s fade, sped up to the script's span. It now shows the script's
  five-update blend from the held clip and the wait's even steps to frame
  23 (30 for the post-up steal), as the simulation poses it
  (`channel::swipe_shown`).

Left for the view's own fades: a pass without a catch row (a handoff, a
lob to a leaper) and a trick whose clip the disc lacks.


## The spot run's radius (2026-10-25)

`FUN_0015a060` stops a spot run's receiver (`DAT_003d70b4`) within 1.0 of
the spot while he moves and 2.0 when his velocity (actor+0x40) is the
zero vector: `0x15a5fc..0x15a610` before the release and
`0x15a86c..0x15a880` in the flight load 1.0 and replace it with 2.0 only
when the four-word compare with `0x4c9880` (zero) held. The distance is
`FUN_0018cdb0`'s from his place, height included, to the spot on the
floor. The rewrite had the radii the other way round, so a running
receiver stopped about 1.8 short of the spot the pass is aimed at and
`catch_stand_1` held him there while the ball went by: the catch-row-0
passes lost at their timer on the third capture's rosters went from 12 to
1 over the 16 bases (all such losses 29 to 25). Passes lost at their timer
a match: first capture 2.31 to 2.44 (its losses are the trick passes off
the glass), second 0.75 to 0.13, third 1.81 to 1.56.

## Lost passes, lobs to a leaper and the block test's progress (2026-10-25)

**The first capture's passes lost at their timer** (2.4 a game against the
original's 1, one match): over 16 games 39, of four kinds. Nine are head
passes (`Via::Head`): the ball meets the point aimed at, but the defender's
head is 3 to 4.5 away by then, past the passer's `Handles × 4 / 99`; at the
throw the head bone stands 1.8 in front of him and low (a crouch or a
step), the rest is his walk. 34 of the 42 head passes connect. Eight are
lobs to a leaper, six of them the board's (a passout special to him, timed
by the board route, 34..44 updates, longer than his span). The original
does the same: of its five passout specials to a leaper in the person's
captures none reached him (loose, or another's), and about one in five of
its ordinary lobs is lost. Neither is a defect on this evidence. The rest
are standing or reacting receivers.

**`FUN_0016d888`'s late-flight let-go reads ball+0x60 over +0x64**
(`0x16de54..0x16de6c`, `cvt.s.w` of both, then `div.s`): the updates flown
since the launch over the launch's own flight, with no holder, ball+0x180
5 and not the glass pair (`FUN_0016ea60`). The rewrite divided its
`flight_left` by the flight, but for a shot `flight_left` is the timer
(ball+0x158, `straight_timer`, which runs past the aim): the progress came
out low and often negative, so the 0.7 test seldom let a ball go and the
defenders under the rim swatted shots coming down. `Ball::flight_start`
now keeps the count the timer started from, and `Ball::flown_ticks` counts
from it (a pass's start is its flight, as before). In the original the
three captures' 145 shot ends are 75 clean makes, 15 off the rim in, 29 off
the rim as rebounds, 6 knocked loose near the rim and 2 elsewhere; on the
first capture's rosters the knocks near the rim fall from 6.1 to 3.3 a
game (the original's 3), on the third's from 4.5 to 2.8 (2), the balls let
go mostly going in off the rim. Rebounds stay short (10.2 against 14, 6.5
against 11): the rewrite misses fewer shots (71% and 74% made against 68%
and 65%), the shot's odds (`gameplay.core_shot_finish_defense`).

### The dunk a taller man meets reads his posed hands (2026-10-25)

At the release the original's jump shots stand farther out and are more
often open than the rewrite's (three captures: 72..79% beyond 22 against
58..60%, open 34% and 29% against 24% and 15%), and its computer all but
never lays up: one dunk-row lay-up (state 0x2008) in 53 shots against the
rewrite's 5 a game. Most of the rewrite's were lay-ups in the air
(`FUN_0016b1e8`): the drive and the post-up, committed to a dunk,
press shoot every other update once `FUN_001837d0` says a taller man meets
it, and that press turns the dunk into a lay-up. `FUN_001837d0` (raw
`0x1837d0..0x1838ec`): no gamebreaker task (`FUN_00140ff8`), in a dunk
(`FUN_00181670`), at least 1 up (`FUN_0013ab18`'s y) and 5 from the rim
(`FUN_0017dd10`), the nearest opponent (`FUN_0017f2e0(ai, 0, 0)`) within 6
(`FUN_0018cde0`) and nearer the rim, and the ball (ball+0x34) under the
highest of his left hand, right hand and head (`FUN_00181f50`: bones 15,
19 and 21, the larger y of each pair). The rewrite stood his height plus
his standing reach in for the bones, about 8.5 for a man flat-footed,
over almost every dunk's ball. Now `World::top_bone`, as the pass out of a
shot already read it. On the third capture's rosters the lay-ups in the
air out of a dunk fall from 5.2 a game to 1.4 (the first's 2.3).

### A computer screener's pick press (2026-10-25)

`FUN_0015a060` without the ball (raw `0x15b5c4..0x15b798`): the pick press
(pad +0x7c & 0x4000000, the "set pick" and "pick off ball" nodes' press
as the screener steps into the defender). For a computer's actor
(`FUN_00172220`: no controller, game+0xf4 0 or 1, else controller type 4)
on the floor, outside a restart and with none of +0xa4's 0x9388 (a shot, a
dunk, a rebound jump, his catch row, a block jump, a leap), `0x43f028[rand()
% 3]` on his channel: `0x44c222`, `0x44c27b`, `0x44c2d4`, the setroughpick
scripts entered 0x11 past their callback 16 (records 650..652, the
clearout's arms' window `a6 2 1` for 30 or 38 updates). A person's press
wants both turbos (+0x78 & 0x40000) and half the bar (`FUN_00190c08`) for
the whole scripts (`0x43f038`), else the "no turbo" message (`0x3c6520`);
held 0x400000 is the stance (`0x44ba7b`, record 64), not played. The sim
dropped the press (`Input::with` masks to the buttons), so a screen only
knocked a man by the still screener's contact count. Now
`button::PICK`, `World::pick_press`, `trick::PICKS` (the reactions grow to
85: celebrations from 64, the loser's from 81). "Set pick" also waits
while the screener plays a pick clip (`FUN_001817e0` → `FUN_0016eb80`:
channel 0's record, or the blend's, minus 650 under 3, at `0x1187a4` and
`0x118b08`), `Ctx::picking`. On the third capture's rosters the handler's
updates at a shot quality of 6 or 7 (12..29.75 out) go from 117 to 144 a
game (the original's 203), block jumps at a shooter from 8.4 to 9.9 (10),
shots 38.8 to 40.0 (43); the first capture's are about the same.

## The post man's call and state bit 11 (2026-10-26)

- "post pause" (`FUN_00113768`) presses 0x800000 while a lob is on (`0x113800`); the actor reads it as the held post call (`FUN_0015a060` `0x15b544`: actor+0x78 & 0x800000, plays `post_call_for_ball_2` `0x456d96`, whose `9b` sets state bit 11). The rewrite's pad never mapped the bit, so the computer's post man never posted. It is now the held trick button off the ball (`World::post_call`).
- The defence's three bit-11 reads (`FUN_001816f0` at `0x107ee4`, `0x108264`, `0x108684`: the post defender's stand, the spot's post branch, the sag's hold-off) called `Ctx::posting_up`, a never-true stand-in for `FUN_00181710` (the post-up's clips). They now read bit 11 (`Ctx::posting`); `posting_up` and `post_defending` (`FUN_0016ec80`, `FUN_0016ebe8`: records 704..707/712/713 and 716..719/398/399) read the channel, as the switches (`0x123c34`) want.
- The post pause's call holds `fps` frames while he is not posting and runs down once he is (`0x113808..0x113830`); the lob test also refuses with a teammate other than the holder posting (`0x113c10`).
- Capture 3: post-pause men carry bit 11 in 355 of 2,192 updates (16%); the rewrite now 10.9% (0 before). Drives per 1000 half-court updates 3.35 -> 3.11 (original 2.58).
- Checked and faithful on the way (no change): shot quality and its contest, iso3pt/RunPick thresholds, open three (game+0x3ec = players by distance to the offence's rim), penetration check, play interrupt flags (all true but pull up/rotate/iso/oop), zone (16 of 36.75), back down's start. LIVE PINE: channel flag 0x40 ("down") is set exactly on reaction scripts.

## The pick stance, a person's pick, post pause's screens (2026-10-26)

- **The pick stance** (`FUN_0015a060`, `0x15b7d4..0x15b85c`): a computer's actor holding the pad's 0x400000 off the ball, on the floor, outside a restart and neither in a rebound jump, catching nor posting (+0xa4 & 0xb00), starts `0x44ba7b` (record 64; `91 0x820002` holds his walk) every update it is held, so he stands at its blend's start. Let go on record 64 (`FUN_0016eb18`, the blend's target while one runs; `0x15add8..0x15ae14`) he stands ready (`0x4570bb`). "Set pick" and "pick off ball" hold it while they wait at the spot; the pad never mapped it. Now `button::PICK_STANCE` (not a button), `stand::PICK_STANCE` (stand index 51, the flair from 52; snapshot 86) and `World::pick_stance`.
- **A person's pick** (`0x15b5f0..0x15b7d0`): the strategy press (pad +0x7c & 0x2000000, `USRGAME` action 11 at `0x12f97c`, R3) off the ball, past the pick press's tests (+0xa4 & 0x9388, floor, not game+0xf4 6): two turbos held (actor+0x78 & 0x40000) and half the bar buy the whole setroughpick (`0x43f038[r % 3]`, the same three as the screen's `0x3d0fb0`); short of the bar, the "no turbo" message; without two turbos nothing, and the press ends his buttons either way (`World::person_pick`). A computer's strategy press skips it.
- **Post pause's screens** (`FUN_00113768`, `0x113a34..0x113abc`): with the owner not yet on the far side for `fps x 1.1` frames, "pick off ball" for the third man (`FUN_00114050`: out of `|z| < 12, |x| < 27`, within 15 and running no more than 216 off straight at the post man, the holder across the court's length from him or in zone 2) or "set pick" in mode 1 (`FUN_00114480`: the set-pick window open `fps x 2.5` frames in, the holder within 35.75 of the rim with his defender within 15), each pushed behind the node, which ends. Neither in lesson 15 (Pick). Before, the rewrite said the sim had no screens.
- **The catches' state word**: the catch scripts' `9b` replaces actor+0xa4, so the post catches (`0xa00`, the 0x8000 rows a posting receiver takes) keep bit 11 and the others clear it (`World::catch_step`).
- **The Pick lesson** (vt+0xb4 0xf) has no screen reaction (`0x135444`); the rewrite's note said no game had it, but the School plays it.
- Measured after (capture 3's rosters, 6 seeds x 30,920 updates): posting actor-updates 259..653 (original 492), runs 11..28 (15). Still open: the original's holder passes to a post man within 6 updates of his posting in all three of its post catches (3 a match; the rewrite 0..1), through the playbook's pass check, not a posting read.

## The School's defence lessons and the post man's catch (2026-10-26)

- **The playbook's lesson branches** (`FUN_001276f8`, `0x127844..0x127f08`, a side without a person, game vt+0xb4 the lesson, 5 s or more on the shot clock): Dive (16) puts an owner with the ball, not juking, at `x > 0` on "playbook hold" (vtable `0x43ae88`, update `FUN_0012b7e0`: a juke every other update, never done); Steal (8) pauses an owner 12..33.75 from the rim for `fps x 30` (type 0x10) during the playbook's first `fps x 3.5` such updates (`[0x41]`, cleared at its start); Trick Counter (10) gives an owner 12..30.75 out "pause 0x10d090" (type 0x10, `fps x 30`: facing the nearer of his nearest opponent and the rim, he jukes whenever he is not); Block and Turbo Block (9, 11) pause him too and make him shoot (`FUN_001296b0`) once the first person has stood within 8.1, his way to the rim within 71 of the owner's and on his feet, for `fps x 0.75` updates (`[0x42]`; at `fps x 0.55` the playbook posts the lesson's prompt `0x3c6970`, which the rewrite's lesson manager already shows). Most branches end the play; Trick Counter's keeps it. The rewrite had said no game it plays has a lesson id.
- **The post man's catch**: in the original a pass's state is 4 from the press while ball+0xf4 stays the passer through the windup (capture 3, frames 13483..13502), so post pause keeps its owner, and the call, until the ball leaves the hand. Starting the catch script ends the receiver's actor update (`0x15ab48`), and from then on the channel shows the catch row, so the post call's release (`0x15ad84`, rows 728/729) never sees him; the post catches' `9b 0xa00` keep bit 11. The rewrite ran the release in the catch's update and stood him up. It also ran the release only for the offence's men off the ball, so a man posting when his side lost the ball posted until the next possession (689 updates in one run); the release comes before the update splits actors by side or ball.
- Measured after (capture 3's rosters, 6 seeds x 30,920 updates against the capture's one match): passes to a post-pause man 2..9 (original 4), to one posting 1..8 (3); catches while posting 1..6 (3); posting actor-updates 245..570 (492); runs 10..19 (15), longest 61..80 (76).
- **A catch row replaces a stand that holds the walk**: the stand path kept the post call's stand (its `91 0x2` holds the locomotion) over a post catch's row, so the hands stayed on the post call's clip and the pass ran out at its timer (4.6 a match against the original's 0 once the post man kept his post to the catch). The catch script replaces the channel's script (`FUN_0019c2f8`); the stand path now drops any stand under a catch row. Passes lost at their timer: about 1 a match again.
- **The other lesson stand-ins**: the Dunk, Alley-Oop and Advanced Dunk lessons (3, 5, 0x15) skip the opponent's cone in `FUN_0016c500` (`0x16c6d4..0x16c720`); the computer's block (`FUN_00106a88`) returns at once in lessons 1..5, 9, 0xb, 0xd, 0xe, 0x12, 0x13 and 0x15 (`0x106ab4..0x106c5c`, each returning 0, so the steal and the spot still run), and always bites on Pump Fake's fake (0xc, `0x107000`).
- **The lessons' other branches** (the 77 reads of game vt+0xb4): the shoot node waits in the Block lessons (9, 11: `[0x28]` set with no draw, `0x100e94`), stops its wait at once and never pump-fakes (`0x1010e0`, `0x101188`), and the pass out of a shot is off (`FUN_00181c68`, `0x181df8`); their holder's shoot press always plays `0x44ed73` (`FUN_0016b4b8`, `0x16b514`) and a person's block jump rises on Blocks 99 (callback 35, `0x1764b8`). The Dunk and Advanced Dunk lessons (3, 0x15) roll the make on Dunks 99 (callback 21, `0x172bc8`) and drop the press's easy rows (`FUN_001693b8`, `0x169744`). A person's swipe strips on Steals 99 in the Steal and Dive lessons (8, 16, `0x16d0fc`) and the Dive lesson always knocks the ball loose (`0x16d2e8`). The Alley-Oop lesson (5) skips the leaper's open lane (`FUN_0012a568`), adds 100 to the odds for a person's holder under 6 s (`0x12a1d0`) and waits `fps x 3` after a person's teammate leaps (`0x12a298`); the Pick lesson (0xf) ends a person's play instead of calling a pick (`0x128130`, and the subclass's `0x124594`). Left: the Dive lesson's actor fields (`0x157188`, `0x15f1f8`: actor+0x1b8/+0x1d4) and the pocket lessons' `0x159db8..0x159e04`.

## The trick fumble (2026-10-26)

- **The roll** (`FUN_0015f0f0`, `0x15f620..0x15f828`): a trick press at value class 2 or more rolls `r % 100` under a chance from Handles (`FUN_00171670(actor, 7)`) in tiers of 20 (an exact multiple the tier below with `f = 1`; 81..90 tier 5, 91..99 tier 6 by tens; 0 no tier: 100) less the class: three or more short 100, 3 short `(1 - f) 100 + f 90`, 2 `(1 - f) 90 + f 60`, 1 `(1 - f) 60 + f 30`, 0 `(1 - f) 30`, else 0 (truncated). A fumble sets actor+0x1d4 and +0x1b8 = `DAT_004c99b4` (`fps x 0.5`, `0x178b70`); every press first clears them, and in the Dive lesson (16) sets them (`0x15f1f0..0x15f22c`). The rewrite drew the roll and dropped it.
- **The fumble** (`FUN_0015a060`, `0x15a1c8..0x15a3f0`; the wait counted down in `FUN_00156f48` at `0x157168`; the defence's update clears the flag, `0x15da70`): once the wait is 0 the flag clears, and a holder falls (off rows 188, 189, 262..265: `r % 100` 75+ `0x44bdf7`, 50..74 `0x44be13`, 25..49 `0x44be4a`, else `0x44be66`), the ball leaves at 0.35 an update along his facing turned by `(r & 0xff) - 128`, rising 0.125, spinning about the up axis (`0x4c98b0` = (0, 1, 0)) at `0.075 + u x 0.075` (`FUN_0014c408`), and he and his nearest opponent are barred `fps` updates; his update ends. Now `World::fumble_tick` (snapshot 88).
- **The Dive lesson** also takes the dive from the computer's pads while the ball is loose (`0x157188..0x1571f4`).
- Measured (capture 3's rosters, 16 seeds): held balls knocked loose 11.8 -> 12.7 a match (the original's match 10), trick moves 82 -> 84 (81).
- **Two more gates**: the clearout (`FUN_0015c778`) refuses a posting holder (bit 11, `0x15c844`), which the rewrite could skip while its men never posted; the gamebreaker's start (`FUN_00159cf0`) runs only outside the lessons or in the Gamebreaker and Level 2 lessons (game vt+0xb4 -1, 14, 25, `0x159db0..0x159e1c`).
- **The clearouts measured** (capture 1 and 3 against capture 1's rosters, 8 seeds): the original's 6 and 4 are 5 and 2 rebound landings (callback 37, state 0x100 the update before) and 1 and 2 jukes pressed by "post up" right out of a catch; the rewrite's 0.75 landings and 1.6 jukes a match. The landings follow the rebounds caught (6.7 a match against 11), the make-rate gap.

## The hands' poses (2026-10-26)

- `FUN_001719f0` (from the actor's update at `0x1573d0`) picks a pose for each hand every update and hands them to the model through actor vt+0x54 (`FUN_0017c5a8`: actor+0xb8 the right, +0xbc the left, then the model at +0x28c). The body models' hand groups, in order, are the poses: right 0 `ARfist`, 1 `ARfthru`, 2 `ARpoint`, 3 `ARrelax`, 4 `ARrhang`, 5 `ARstress`; left 6..11 the `AL` ones (the defaults 3 and 9 are the relaxed pair, the pick stance's 0 and 6 fists). The rules: relaxed; in a jump shot or lay-up (bit 3) the hand the frame's flags name (channel +0x14: 4 right, 8 left) alone follows through; a swipe, a block jump, a dive, a rebound jump, a catch, a post (bits 6, 12, dive, 8, 9, 11) or a pass flying to him stress both; hanging on the rim (bit 14) off rows 847, 848, 921, 922 the named hand or hands hang (rows 996 and 997 hang the other); rows under 40 (the locomotion) and row 957 clench both; holding the ball below row 958 relaxes the hand the dribble does not use (+0x25c); record 64 (the pick stance) and records 650..652 (a pick) clench both. Its per-row table at `0x3e91a8` holds other data on the disc (script entries), so no row matches.
- The rewrite drew the relaxed pair always. Now `World::hand_shapes` (street-sim `hands.rs`) and the renderer spawns all twelve groups (`street_render::player::HandPose`), the game showing the pair each update (`view::sync_hands`, a replay's recorded world while it plays).

## The call for the ball, the shot mix and stale notes (2026-10-26)

**The call-for-it gesture.** `FUN_0015a060`'s shoot press off the ball with a turbo, when the man cannot leap (raw `0x15b300..0x15b384`): a person (`FUN_00172220`) on the floor, in live play (`FUN_00140558`), with no pass on its way to him (ball state 4 with ball+0x100 him), plays script `0x44c95e` (`FUN_0019c2f8`, flags 0xc10) and his update ends. The script: callback 4 turns his wanted facing (+0xb0) to the rim; callback 47's `84 2f` (`FUN_001779f8`: the ball `fps / 2` updates on, against +0xb0 by `FUN_0018cd78`'s signed difference) picks the side: the plain path turns the wanted facing +256 and blends record 411, the branch at `0x44c9b1` turns it -256 and blends 410 (the mirror); both join at `0x44c9cc` for the wait, a hop toward the rim (`a1 f0`, 0.15 along, 0.2 up), the landing and the tail. Its first `91` (0x820042) sets channel 0's 0x40 (down) until the last `91 0` clears it, so it plays as a reaction: `leap::CALL_FOR_BALL`, `trick::CALL_FIRST` (85, 86), `World::call_gesture`. The reaction runner now applies callback 4 (`Want`) and `a1 f0` (`Toward`) relative to the rim, and `Toward` sets the hop's landing (held moves keep their flight's own `land_at`). Tests: `the_call_for_the_ball_is_the_executables`, `a_person_who_cannot_leap_calls_for_the_ball`.

**The shot mix, measured on capture 3's rosters (nothing ported).** The original's 24 jump shots in its match (17 from 24..30) against ours ~18 (7 from 24..30). The playbook's shot check (`FUN_001290b8`) and its call order in the update (`0x12806c`), the plays' vt+0x3c (all 1 but pull up, rotate, iso), the oop slot test (`FUN_0012a618`: playbook+0x8c against slot +0x84, which the constructor always fills) and the early-outs (`FUN_00181508`, `FUN_001816d0`, `FUN_00181938`, `FUN_001819f8`) all re-read and match. Every native good look (quality 6..7 from the record's ratings, down defenders skipped) in carrier pause has the holder in a trick (bit 10) or a catch (bit 9), which the check skips; ours too. Free good looks by node: carrier pause 22 vs 12, iso3pt 68 vs 31, run pick 25 vs 13, in a few long stretches of one match. Down defenders near the holder match (nearest down 16.5% vs 16.7%). The gap is how often the holder stands open, not the decision.

**Notes found stale.** The contact handler table `0x3d0c30` has four kinds, all ported (contact.rs). `FUN_00141448` skips the screen's message while the script manager runs (game+0x424 +0x50 in 3..7, the rewrite's `sequence`): now gated. The putback is ported (lib.rs, shooting.rs). The leap's `a6 01 02 50` is the generic knock window. The walk-to-point node (type 0x18, `FUN_0010f278`) is built only by the strategy of vtable `0x43aa60` (no constructor makes it) and by `0x1847c8` (nothing calls it): dead on the disc. The staged hooks `0x32b910`/`0x32b998` save and restore a staged actor's x and z around his update while the script manager runs a script (vt+0x14 of actor+0x1c).

**The dribble's bounce sound.** `FUN_0014e060` posts the floor event `0x3c6670` from the held ball (raw `0x14f410..0x14f5b8`; the sound listener's slot 44, `0x1a50b8`, plays event 0x44 at the holder at 1000 whatever the event's speed). A single flag (`DAT_003d5600`) is armed while the ball falls on a dribbling clip (in flight before the floor's share of its way, `t <= fall` at `0x14f41c`, or lower than the update before, ball+0x84); once armed, the first other update on a dribbling clip with the ball under 2 now or the update before posts the event and disarms. Clips that carry the ball on the floor (222, 223, 300, 301; 270, 271 to frame 23; 252, 253 to frame 44) or hand to hand (302, 303) clear actor+0x388 and leave the flag be. Ported as `World::dribble_bounce` (`dribble::ball_place`'s `Dribbled`, `World::dribble_armed`, snapshot 89) and `Event::Dribble`; `street-game`'s sfx plays it in place of its stand-in timer (0.32 s running, 0.48 s otherwise). The channel blends' own path (s2 with actor+0x384) is not modelled, as the blends themselves are not. Test: `the_dribble_sounds_once_a_bounce_as_the_ball_rises_off_the_floor`.

## The defence's three leftovers (2026-10-26)

**Standing still** (`FUN_001089d8`, raw `0x1089d8..0x108b40`, called by the steal's odds at `0x1075d4` and the press spot at `0x108230`): besides the man's frames stood still, the defense node keeps the man's x each update he holds the ball in a ring of `DAT_004b3904` = `(int)(fps × 0.666667)` = 40 floats (node+0x64 the count, +0x68 the still frames, +0x6c the vector the constructor allocates zeroed at `0x106364..0x1063b8`; written at `0x106960..0x106994`, cleared when he lets go). With the window full and the holder short of 3 into the half he attacks (`FUN_0017dcf0`'s sign times his x), he counts as standing when he has come less than `40 × 0.1` that way since the ring's sample two on from the newest (38 updates back). Ported (`ai::defence::Ring`, `crawled`; snapshot 90). Test: `a_handler_crawling_up_his_back_court_counts_as_standing`.

**The pass jump at a board passout's leg back** (`0x106d8c..0x106dd8`): game+0x160 (`FUN_00142038`) ±1, a board passout, with the ball's waypoints (game+0x134 +8 less +0x10) at the last one, lets the pass jump go though the receiver (the passer himself) is not leaping. Ported as `board_return` (the rewrite's passout tossed, its leg back spent at the board).

**The goal-tend while a gamebreaker runs** (`FUN_0012de70`, `0x12df68..0x12df9c`): game+0x434 is the gamebreaker task; while it runs (`FUN_00140ff8`) the computer's goal-tend odds are `skill(0, 0, 10, 40)` for `skill(0, 0, 50, 80)`. Ported on `World::gamebreaker_task`.

## The block jumps' clip root (2026-10-26)

No block script's `91` sets 0x1000 (all thirteen and the goaltend pair set 0x820002, then 0x10002 before the aim, 0x2 and 0 at the tail), so `FUN_0019a658` (raw `0x19b4bc..0x19b70c`) moves the jumper by his clip's root change on x and z every step outside a blend, times his height scale, turned by his facing. The clips travel 0.5 (the turbo block, 437) to 5 (457) over the jump. The rewrite left it out ("the rewrite's own"). Measured on the third capture: native jumpers move before they leave the floor, their velocity stopped by the script's `97`, by script length 0.016 (6 updates), 0.10 (7), ~0.7 (8: 0.40..0.76), 0.24..2.45 (10); the rewrite moved 0 there. Ported as `World::block_root`; on capture 3's rosters ours now moves 0.008, 0.18, 0.58 and 0.45.
The glass pair (`0x442273`, `0x4422fa`) aims with `a9` (`FUN_00199e20`), which sets channel flag 0x1000 before the step (`0x199ef0`): their root stops moving them from the aim on.

## The game's distance tables sort as the original's (2026-10-26)

The game builds its tables once an update (`0x13f748`, `0x13fc50..0x13fe40`): each player's five others in actor order (game+0x23c), all six by distance to the ball (split per side in order, game+0x3a4) and all six by distance to the offence's rim (game+0x3ec), each sorted by `FUN_003b3868`, the C library's `qsort`, with `LAB_0013f728`'s comparator `(int)(a - b)` (`cvt.w.s` truncates). Under seven entries `qsort` is an insertion sort (`0x3b38cc`), so distances within a unit compare equal and keep actor order. The rewrite sorted exactly. Ported as `ai::native_sort`, used by `Ctx::by_distance` (nth opponent and teammate, so the shot quality's nearest defenders), `by_rim` (nearest to the rim, the side's k-th, the rebound spots, the defence's nearest two), `by_ball` and the switch target's side's nearest. Test: `the_tables_sort_keeps_actor_order_within_a_unit`.

## Callback 53's hand: the root's change, the palm (2026-10-26)

`FUN_001741a8` (raw `0x174490..0x174644`) saves the model's 22 bones, poses the clip at the cue's frame (`FUN_0019bda8`), takes the hand the frame holds out by `FUN_0014da20` (the ball's radius off the palm; 8 the right, else the left), and moves it by the clip's root at that frame turned and scaled into the world less the model's root now (+0xe0); while the channel blends (flag 0x200000) it adds nothing (`0x4c9880`'s zero). The rewrite's passout toss (`World::toss_hand`) added the root at that frame in full, counting again the root motion the player had already taken, and used the bare hand bone. Both fixed. The trick self-tosses still aim at `self_toss`'s fitted displacement (checked against the played capture's script 69 to 2e-5): posed this way script 69's hand at frame 21 stands 2.15 farther forward for a 1.2 body, so they stay until a capture comparison settles it.

**The self-tosses on callback 53's hand.** The played pads capture's script 69 (frame 8009: actor 3 at (-31.13, -10.13) facing 340, record 296 at frame 7.5, the cue's frame 21) tossed to (-34.346, 3.864, -13.297); the posed hand with the root's change meets it within 0.02 for a body of 1.25, while `self_toss`'s fitted displacement misses by 8.9 at that facing. `World::self_toss_launch` now aims every self-toss trick at `World::toss_hand` (the fitted displacement only without the disc's animations). Test: `callback_53_aims_at_the_hand_the_played_capture_tossed_to`. The automatic catch stays the rewrite's.

## The rebound and block jumps on channel 0 (2026-10-26)

`World::channel` (what channel 0 shows: the held ball's clip for `FUN_0014e060`, the hands' poses, the contacts' poses) fell back to the gait for a rebound jumper and a block jumper, so a ball caught in the air rode the gait clip's hands until he landed. It now gives the rebound script's clip and frame on its clock (`World::rebound_channel` over `rebound_clock`, the held clip and weight through its 4-update blend) and the block script's (`block::Jump`'s record, frame and blend). Test: `a_ball_caught_in_a_rebound_jump_rides_the_jumps_clip` (without it the channel showed the gait's record 65).

**The gait's own blends on channel 0.** A change of gait row blends from the old row's clip over 6 updates (flag 0x200000), which the body's pose already showed (`World::actor_pose`) but the channel did not, so `FUN_0014e060`'s blend path (the target at its frame on the model posed in the blend, the stored offset eased away) never ran for the held ball on the gait: it jumped to the new clip's hands. `World::gait_channel_shown` now reports the blend (weight, held clip, counter).
