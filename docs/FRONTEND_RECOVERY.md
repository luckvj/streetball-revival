# Kind-10 front-end nodes (2026-10-13)

Live Ghidra MCP, street2ee / SLUS_206.51, `/get_functions` and
`/read_memory`, traced FUN_002e5d30, FUN_00221ac8, FUN_00221ba8,
FUN_00221cf0, FUN_00222590, FUN_00222678, FUN_0021f8f0 and
FUN_00222420. These are static observations from the EE patched project.

The factory does have case 10 -> FUN_00221ac8 (0xd0 allocation).
The old scratch front-end report's assertion that case 10 is absent is
obsolete. Vtable 0x468e50 belongs to these navigation nodes. They do not
name their own texture; node+0xb0 binds to a display widget, and +0xb4
binds to a list. FUN_00222420 positions the bound widget at the node's
coordinates and propagates its selection/visibility. Drawing art therefore
requires tracing the bound widget, not guessing a texture for kind 10.

FUN_00221ba8 loads paired direction modes and link IDs: 25/26 up,
27/28 down, 29/30 left, 31/32 right. Modes are 0 disabled, 1 scroll the
bound list, 2 follow an explicit link. FUN_00222590 overrides the mode
for a bound kind-36 widget; FUN_00222678 likewise remaps its direction.
FUN_0021f8f0 follows links and recursively skips unavailable bound widgets.
33 is the list binding; 34 is the row ordinal. The left/right properties
are general direction pairs, not exclusively map-only data.

Built: Screen graph stepping now respects disabled direction modes,
leaves scrolling to the existing list path, and stays on a malformed link
instead of wrapping through the fallback. Regression coverage distinguishes
all three modes and invalid targets. Dynamic widget enablement and the
kind-36 direction override are still outside this generic Screen model.

Next art probe: FUN_0021f230 pairs the list's display-widget vector with
its navigation-node vector through FUN_00222420. Trace each screen's list
population to the bound widget kind and its renderer (kind 19's
FUN_0021c4c0 is text with optional arrow children, not a universal tab).

## Authored sprite frames

`FUN_0039a900` locates SSH block kind `0x7c` and returns block+8.
The word at block+4 counts signed integer corner pairs; four pairs
make one frame. `FUN_00210048` consumes 32 bytes per frame, derives
UVs from all four corners, and derives dimensions from corners 0 and 2.
This block uses an eight-byte header, unlike the pixel block header.

The asset reader now retains these corners beside each decoded texture.
Disc regression checks cover the tab, stars, and ten common bar frames.
Menu bullets and the user column tab now crop their authored frames;
the stars are 23x21 and 27x25, rather than the prior guessed sizes.
Corner order is retained for mirrored frames, although the current crop
helper accepts ordinary axis-aligned frames only.

`FUN_00218950` constructs kind 31 with common sheet 0 image 6
frames 6/7/6; the last end is mirrored through `FUN_00210e10`.
`FUN_00218a80` stretches the middle to the element width minus
`2*(endWidth-2)`. Its generic layout renderer now uses the authored pieces.
Screen-specific widget bindings and selection animations still need their
native paths reproduced.

## List display widgets, kind 36, and segmented-bar renderer

The list vectors are distinct. `FUN_0021f508` appends a display widget to
the kind-9 list's `+0xb0..+0xb8` vector; `FUN_0021f580` inserts a kind-10
navigation node, sorted by prop 34, into `+0xbc..+0xc4`. Raw ELF
disassembly (`tools/mipsx.py`, original `dec/SLUS_206.51`) confirms
`FUN_0021f230` pairs the vectors in order and calls `FUN_00222420(node,
widget)`. That call stores the widget at node+0xb0, places it at the node's
position, and propagates visibility, selected state, and colours. Surplus
widgets/nodes are hidden or skipped; this is not a kind-10 texture binding.

Disc layout plus screen setup evidence:

| Screen | Layout binding | Screen setup binding |
|---|---|---|
| Main Menu, class 3 | kind-9 list id 19; six kind-10 nodes have prop 33=19 and prop 34=0..5. Five kind-19 row templates are IDs 0, 1, 23, 21, 26. | `FUN_0029e230` registers those five row widgets, then creates kind-19 id 31 with text id `0x0306` and registers it sixth. `FUN_0021f580`/`FUN_0021f508` make the pairs; kind-19 is text (font rendering), not a sprite. Its optional arrow children select common sheet 0, image 1, frame 0 (`FUN_0021c628`). |
| Play It, class 5 | kind-9 list id 0; four kind-10 nodes, prop 33=0 and prop 34=0..3. Four kind-19 rows are IDs 15, 17, 18, 16. | `FUN_002beef8` registers rows in that order on list 0. These are font text widgets; no menu-item atlas image is selected by kind 10. |
| My Team, class 6 | list id 5, two nodes (rows 0..1) and kind-19 row IDs 1 and 2. | `FUN_002a3c60` (invoked by constructor `FUN_002a3a30`) registers elements 1 and 2 onto list 5 via `FUN_0021f508`. |
| Be A Legend map, class 56 | Separate node groups: prop 33=0 (six landmark rows) and prop 33=26 (five read-out rows); kind-9 list IDs 0 and 26. The layout also has kinds 2, 3, 4, 19, 29, and 39. | `FUN_002a03c0` (invoked by constructor `FUN_0029f200`) registers landmark elements 100, 102, 101, 103, 104, 105 paired with text widgets 27, 29, 28, 30, 31, 32 onto list 0; `FUN_002a0fe0` iterates table `DAT_00489bd8` to register 8 read-out row widgets onto list 26 (0x1a). |
| Inside Vol. 2, class 102 | kind-9 list id 0; five nodes, rows 0..4; five kind-19 row elements (IDs 15, 17, 18, 19, 22). | `FUN_00289a10` (invoked by constructor `FUN_00289890`) registers elements 15, 17, 19, 22, 18 in exact sequence onto list 0 via `FUN_0021f508`. |
| What's New, class 103 | kind-9 list id 0; seven nodes, rows 0..6; five kind-19 row elements (IDs 16, 18, 19, 21, 22). | `FUN_0028a970` (invoked by constructor `FUN_0028a7f0`) registers elements 22, 16, 18, 19, 21 onto list 0 via `FUN_0021f508`. |
| Bobbito, class 104 | kind-9 list id 0; four nodes, rows 0..3; two kind-19 row elements (IDs 15, 16). | `FUN_00288be0` (invoked by constructor `FUN_00288a60`) registers elements 15 and 16 onto list 0 via `FUN_0021f508`. |

The class-3 and class-5 row order above is from the screen setup functions,
not inferred by sorting the layout's element table. The other listed screens
are layout facts only. A disc regression now checks all 61 FE layouts: none
contains a static kind-31 or kind-36 element. Kind 36 is runtime-created (or
otherwise outside these XLAY element tables), so its per-node binding cannot
be reconstructed by the current `Layout`-only `screen_step` model.

`FUN_00222590(node, dir)` returns the node's mode prop (25/27/29/31), except
when node+0xb0 is kind 36, when it forces mode 2. `FUN_00222678(node, dir)`
then uses bound-widget byte +0xb0 as the direction index (0..3) and selects
the corresponding explicit node link at +0xc0/+0xc2/+0xc4/+0xc6. This is a
real override, but the widget byte and creation site are not in the layouts;
it is deliberately not guessed into `screen_step`.

`FUN_002e5d30` dispatches kind 31 to `FUN_00218950`. Its three children use
common sheet 0, image 6, authored frames 6/7/6; `FUN_00210e10(child, 1)`
mirrors the final end horizontally. On the US disc, image-6 frame 6 is
`[343,4,11,25]` and frame 7 is `[357,4,10,25]`. `FUN_00218a80` keeps both
ends at authored width and places the middle at x+endWidth-2 with width
`elementWidth - 2*(endWidth-2)`; the final end begins at
x+elementWidth-endWidth. The generic `Style::Layout`/`Style::Screen` draw
pass renders a kind-31 element using those pieces.
Complete xref analysis of `FUN_00218950` in `SLUS_206.51` proves that its sole
caller in the entire executable is `FUN_002e5d30` at `0x2e61b8`. No screen
constructor or runtime system creates kind-31 elements dynamically; it is an
BIG engine layout primitive not authored into any of the 61 disc `.XLAY` files.

Additional art mapping: factory kind 6 (`FUN_002e5d30` → `FUN_0021d148` →
`FUN_002370c0`) uses common sheet 2, image 0, frames 4/0/1/0 for its track
and three moving markers; the last marker is mirrored. Disc corners are
frame 4 `[3,44,176,20]`, frame 0 `[7,7,4,11]`, frame 1 `[14,7,2,11]`.
The native kind-6 movement code is `FUN_0021d410`; the existing list slider
is not claimed to reproduce that animation.

Remaining: list registration and highlight/selection animation for class 6,
class 56, and the other screen classes; kind-36 widget construction/state;
per-screen confirm/cancel/key dispatch; and image properties for the map's
kind-2/kind-3/kind-29/kind-39 widgets. The source shows their constructors,
but the layout-only renderer does not yet reproduce those runtime bindings.

### Native Options slider (kind 6)

The Options column now uses the native continuous fill rather than the legacy
skill-meter ticks. `FUN_002370c0` selects XACOMMON sheet 2/image 0 frames
4/0/1/0, mirrors the right end, trims the middle UV by +0.005/-0.0025,
and sets content width 170/end width 4. Derived constructor `FUN_0021d148`
places the ends/middle at x 6/9/14 and y 3. The vtable slot +0x154 points
to raw leaf 0x237948: `(widget+0xbc)/(widget+0xb4)*(widget+0xc8)`.
`FUN_0021d410` hides middle/right at fill <=4, hides only middle through
fill 8, and thereafter rounds `fill-8` and places the mirrored right end
at `9+middle_width-1`. Unit checks cover zero, both thresholds, full fill,
and rounding. This ports local widget geometry; surrounding menu placements
and native runtime widget bindings still require comparison with the original.

The native updater `FUN_0021d410` was decompiled from `SLUS_206.51`. It is the sole
child updater invoked by decrement `FUN_002378c8` and increment `FUN_00237908`.
In the Options screen, discrete volume levels 0..10 map to fill `(level / 10) * 170.0`.
At level 0 (fill 0.0 <= 4.0), `FUN_0021d410` sets child 1's width to 0.0 and hides
children 2 and 3. At level >= 1 (fill >= 17.0 > 8.0), children 2 and 3 are shown,
child 2 width is set to `ROUND(fill - 8.0)`, and child 3 is placed at `9+width-1`.
In `crates/street-game/src/look.rs`, `native_slider` evaluates `fill.min(4.0)` for
child 1, emitting width 0.0 at level 0 and restoring width 4.0 at levels >= 1,
preserving exact geometry and visibility across shrink-then-grow lifecycles.
Verified in `native_option_slider_matches_small_fill_boundaries_and_full_width` and
`native_slider_preserves_child_widths_across_shrink_then_grow_lifecycle`.

Live rewrite smoke: a fresh-save scripted Options visit rendered all three
continuous volume bars (`street-recovery-options-slider.png` in the local
temporary directory), with no panic. This is rewrite display validation,
not a screenshot comparison against the original PS2 runtime.

## Dynamic navigation, unavailable widget skipping, and kind-36 direction override

Decompiled `FUN_0021f8f0`, `FUN_00222590`, `FUN_00222678`, and `FUN_00222770` from the live Ghidra MCP server (`SLUS_206.51`):
- `FUN_00222590`: when a navigation node has a bound display widget (`node + 0xb0 != 0`) of kind 36 (`0x24`), the direction mode is forced to 2 (follow explicit link).
- `FUN_00222678`: for kind-36 widgets, the direction used to select the link is overridden by `*(char *)(bound_widget + 0xb0)`.
- `FUN_00222770`: checks whether the bound widget of a target navigation node is available (`*(byte *)(bound_widget + 0x5b) >> 1 & 1`). Returns 1 if available, 0 if unavailable.
- `FUN_0021f8f0`: when stepping in a direction with mode 2 (follow link):
  - Resolves target link node via `FUN_00222678`.
  - Checks target availability via `FUN_00222770`.
  - If available, returns the target row.
  - If unavailable (0), recursively calls `FUN_0021f8f0` from the target in the same direction, skipping unavailable widgets until an available widget is reached.
- Implemented `screen_step_filtered` in `crates/street-game/src/look.rs` matching these native functions, supporting all four directions (`0` up, `1` down, `2` left, `3` right), recursive skipping of unavailable candidate rows with cycle detection, and kind-36 direction override. Verified with unit tests covering single and multi-widget skips, dead ends, and kind-36 overrides.

## Movie panel placements, loops, and loading screens

Decompiled `FUN_002c3948`, `FUN_002c5190`, `FUN_0029dfb0`, `FUN_0029c3a8`, `FUN_0029a828`, and `FUN_0029a8c0` from `SLUS_206.51`:
- `FUN_002c3948` and `FUN_002c5190`: The Rewards screen loads `/MOVIES/XLP0009.NFX` for its column and `/MOVIES/XLP0012.NFX`..`XLP0015.NFX` for earned trophies 0..3. Panel positions are initialized at `DAT_003fc540` (461.0) and `DAT_003fc544 - 16.0` (70.0), matching the layout slot at `Rect::new(448.0, 112.0, 608.0, 260.0)`.
- `FUN_0029dfb0`: (Corrected below, "City loop placement": `FUN_0029dfb0` is the Main Menu's transition-movie slot, and the city loop sits at (265, 240), not over the left frame; Title shows none.) Edit Moves uses `/MOVIES/XLP0004.NFX` at `Rect::new(62.0, 150.0, 268.0, 282.0)` (wrong: 2026-10-20, `XLP0004` is User Setup's loop; "The movies", below), and completed League Challenge uses `/MOVIES/XLP0006.NFX`. Film room move previews share the left panel at `Rect::new(62.0, 150.0, 268.0, 282.0)`.
- `FUN_0029c3a8`, `FUN_0029a828`, `FUN_0029a8c0`: Loading art sequences through all 25 legend cards (0..24) before repeating, displays mode 0 logo (34) for School, and alternates cards with magazine covers (35..46) in story modes. Full-screen 640x480 textures are constructed by assembling ten 128x256 strips from `/FE/XAFELOAD.BIN`.
- Unified via `loop_movie_for_page` and `movie_panel_rect` in `crates/street-game/src/menu.rs` and verified in unit tests `frontend_movie_panel_placements_and_loops_match_native_screens` and `legends_go_round_and_stories_add_covers`.

## Screen key dispatch, action codes, and page lifecycle

Decompiled `FUN_002e8c90`, `FUN_00221cf0`, `FUN_0021fa48`, and `FUN_00245b88` from `SLUS_206.51`:
- Native pad/input action table (`FUN_002e8c90`):
  - `ADVANCE_PRESS` (`0x23` = 35) maps to Cross/South button for menu confirm.
  - `CANCEL_PRESS` (`0x25` = 37) maps to Triangle/North button for menu cancel.
  - `HELP_PRESS` (`0x27` = 39) maps to Square/West button for help.
  - `UP_PRESS` (`0x1b` = 27), `DOWN_PRESS` (`0x1c` = 28), `LEFT_PRESS` (`0x1d` = 29), `RIGHT_PRESS` (`0x1e` = 30) map directional pad and left stick inputs.
  - `START_PRESS` (`0x3d` = 61) maps to Start button.
  - Face buttons: `TRIANGLE_PRESS` (`0x29`), `CIRCLE_PRESS` (`0x2a`), `SQUARE_PRESS` (`0x2b`), `CROSS_PRESS` (`0x2c`).
  - Shoulder buttons: `L1_PRESS` (`0x31`), `L2_PRESS` (`0x32`), `R1_PRESS` (`0x33`), `R2_PRESS` (`0x34`).
- Directional dispatch (`FUN_00221cf0`):
  - On receiving directional input (27..30), queries focused node's direction mode (0 disabled, 1 scroll list via `FUN_0021fa48`, 2 follow link via `FUN_0021f8f0`).
  - Plays audio cues: sound 0/1 for vertical scroll/step, sound 2 for horizontal step, sound 3 for confirm, sound 4 for cancel, sound 6 for error / boundary block.
- Screen lifecycle and navigation stack:
  - Confirm transitions forward by pushing target page/cursor onto stack (Title -> Main -> PlayIt / Options / MyTeam / etc.).
  - Cancel pops stack back to parent, preserving root (Main menu never pops below root), saving options when leaving Options (`keep_options`), and resetting cheat entry on Users.
- Verified in `frontend_screen_key_dispatch_and_page_lifecycle_match_native` in `crates/street-game/src/menu.rs`.

## Runtime row binding and screen-specific widget registration

Decompiled native screen setup functions from `SLUS_206.51`:
- Main Menu (class 3, `FUN_0029e230`): List 19 registers kind-19 element templates `0`, `1`, `23`, `21`, `26`, and dynamic row element `31` (`0x0306`).
- Play It (class 5, `FUN_002beef8`): List 0 registers elements `15`, `17`, `18`, `16`.
- My Team (class 6, `FUN_002a3c60`): List 5 registers elements `1`, `2`.
- Be A Legend map (class 56, `FUN_002a03c0` / `FUN_002a0fe0`): List 0 registers landmark elements `100`, `102`, `101`, `103`, `104`, `105` paired 1:1 with texts `27`, `29`, `28`, `30`, `31`, `32`; List 26 (0x1a) iterates table `DAT_00489bd8` to register 8 read-out row widgets.
- Inside Vol. 2 (class 102, `FUN_00289a10`): List 0 registers elements `15`, `17`, `19`, `22`, `18`.
- What's New (class 103, `FUN_0028a970`): List 0 registers elements `22`, 16`, `18`, `19`, `21`.
- Bobbito (class 104, `FUN_00288be0`): List 0 registers elements `15`, `16`.

The native engine does not sort layout elements by `(y, x)` coordinate. Each screen's constructor explicitly appends its row elements into the kind-9 list via `FUN_0021f508`. Navigation nodes (kind 10, sorted by prop 34) are paired in order with these display widgets via `FUN_0021f230` / `FUN_00222420`, scoping by list tag (prop 33).
Reconstructed via `screen_registered_row_element_ids` in `crates/street-game/src/look.rs` and wired through `Style::Screen` and `Style::Layout`, verified in `native_screen_row_registration_element_ids_match_disc_layouts` (`crates/street-assets/tests/fe_disc.rs`) and `screen_row_registrations_preserve_native_order_over_geometry` (`crates/street-game/src/look.rs`).

## City loop placement (XLP0005)

Static trace of `SLUS_206.51` (decompile plus raw vtable words):

- Producers: screen vtable slot +0x15c returns a movie name and a float
  pair. The slot is `FUN_0029f018` for the Main Menu (class 3, vtable
  `0x489690`, not decompiled by Ghidra; read from the raw MIPS),
  `FUN_002bfb08` Play It (5), `FUN_002a45f0` My Team (6), `FUN_0028a6f0`
  Inside Vol. 2 (0x66), `FUN_0028b6a0` What's New (0x67), `FUN_002897b0`
  Bobbito (0x68) and `FUN_00284f78` class 0x69 (a Play It child). Each
  answers `xlp0005.nfx` with (265.0, 240.0), or `sameloop` with (0, 0)
  when the previous screen (+0xb6) also ran it: Main from 5, 6 or 0x66;
  Play It from 3 or 0x69; My Team from 3; Inside from 3, 0x67 or 0x68;
  What's New from 0x66; Bobbito from 3 or 0x66; 0x69 from 5. Title (class
  4) and the other screens keep the base slot `FUN_002d8fb0` (empty name),
  so no loop (2026-10-20: wrong for the screens with loops of their own, User Setup, Choose Team, Choose Players, Choose Court, My Rules and the boards; "The movies", below). `FUN_0029dfb0` is a different slot (+0x154): the Main Menu's
  transition movie.
- Consumer: `FUN_002ddb40` (screen transition). At the start of a change it
  stops the running loop (front-end +0x58) unless the incoming screen says
  `sameloop`. When the change completes it prefixes `movies/`, creates a
  movie widget (`FUN_0022dfc8`, parentless, so layer 2 by `FUN_00248a90`),
  sets it looping (`FUN_0022e1d0`), subtracts 16.0 from y, and positions
  it through widget slot +0x124 with the pair: top-left at (265, 224). The
  full-screen transition movies (640x480) are placed at (0, -16) through
  slot +0x11c by the same function, so -16 is the 448-line display offset
  and the loop lands at (265, 240) in the 640x480 frame the backdrops use.
  `movie_dump` gives XLP0005 as 320x240 (XLP0002, the other caller of this
  path with (480, 1), is 160x416), drawn at that size.
- Content: XLP0005's frames are the bottom-right quarter of the doubled
  `XAFEBG` 18 backdrop, the right window's city and its frame edges. A
  search over offsets puts the best match at (265, 240) (mean error about
  6/255 per channel, rising sharply a few pixels away), so the loop
  repaints the window in place. Its first columns repeat the backdrop's
  own frame edge, so the legends poster in the left frame is not covered.
- Order: the backdrop is the screen's +0xc4 picture widget (640x480 at
  centre (320, 240), layer 1, `FUN_002d7720`); the screen and its rows are
  in render view +0x38 at layer 2, the loop in view +0x30 at layer 2. How
  the views interleave per layer was not traced; the order backdrop, loop,
  rows is inferred from the content: the backdrop is opaque (it would hide
  a loop drawn first) and the Main Menu rows (y 230..362) overlap the loop
  (they would be hidden if it were drawn last).

Built: `loop_movie_for_page` names XLP0005 on Main, Play It, My Team,
Inside, What's New and Bobbito; `movie_panel_rect` gives (265, 240)-(585,
480); `movie_under_rows` draws it right over the backdrop and under the
rows. The film room keeps one loop running across these pages (each wants
the same path), matching `sameloop` for every move between them the
rewrite offers; any other page stops it. A loop that ends restarts into
the same picture (no blank frame at the wrap), and menu clips start clear.
Tests: `frontend_movie_panel_placements_and_loops_match_native_screens`,
`city_loop_frames_line_up_with_the_main_backdrop_window` (disc: frame 60
against the backdrop at the panel and at offsets), and
`film_room_loop_wraps_into_the_same_picture` (disc). Running-game
screenshots of Main Menu, Play It and Inside show the moving window with
the rows over it.

Unproven: the cross-view draw order (above) and the original's frame
timing; no screenshot of the original was compared.

## Row availability on Screen pages (review G10)

`FUN_00222770` makes a node available when it has a bound display widget
whose byte +0x5b has bit 1 set. `FUN_002490f8` (vtable +0x10c, enable)
sets that bit and recurses; `FUN_002491d8` (+0x114, disable) clears it and
with its third argument sets bit 3, which stops later non-forced enables.
`FUN_002ddb40` enables the incoming screen recursively (+0x10c with 1, 0),
so every registered row starts enabled. Searching the code of classes 3,
5, 6, 0x66, 0x67, 0x68 and 0x69 for +0x110/+0x108 calls, the enable/
disable/show/hide helpers and +0x5b writes finds only whole-list disables
on the way out (`FUN_0029e608` list 19, `FUN_002bf320`, `FUN_002a3fa8`
list 5, `FUN_00289e68`, `FUN_0028adb0`, `FUN_00288fd0`, `FUN_00284740`)
and reads of the screen's own bit in their input handlers. The
constructors (`FUN_0029e230`, `FUN_002beef8`, ...) register every row
unconditionally. The map (class 56) toggles whole panels
(`FUN_002a14f8`); its locked events are dimmed through `FUN_00224e00` and
stay selectable, refusing a confirm with sound 6 (`FUN_0029f4b0`), which
the rewrite's City page already does. So no row on these pages depends on
save progress; the only unavailable nodes are those with no widget
(`FUN_0021f230` pairs in order; What's New and Bobbito author more nodes
than rows), which the graph skips. `screen_row_available` in `menu.rs`
states that rule in place of the old fixed per-page bounds; test
`screen_rows_bound_to_widgets_are_available_and_surplus_nodes_are_not`
(synthetic chain plus disc layouts 3, 5, 6, 102).

Not covered: functions Ghidra did not decompile in those ranges, the
controller layout (class 55, whose widgets toggle as a whole through
`FUN_00274950`/`FUN_002749e8`), and the screen-level disable while a
transition runs (the rewrite has no transition).



## Screen rows, legends, the help line and the navigation rule (2026-10-15, merged)

A helper's pass on `look.rs`, `menu.rs` and `street-render/app.rs`, merged
after review against the decompile (`FUN_0021c590`, `FUN_0021cd28`,
`FUN_0021c628`, `FUN_0021c9e0`, `FUN_0021af18`, `FUN_0021afa8`,
`FUN_0021a530`, `FUN_0029e230`, `FUN_00222420`, `FUN_0021f230`,
`FUN_0021f580`, `FUN_0021f8f0`, `FUN_00221ba8`, `FUN_002e3b18`,
`FUN_00222590`, `FUN_00222678`, `FUN_00222770` re-read for the merge).

- *Where a bound row draws.* `FUN_0021f580` keeps a list's nodes in prop-34
  order and `FUN_0021f230` pairs them with the registered widgets in turn;
  `FUN_00222420` then moves each widget, children and all, onto its node's
  point (vtable +0x124). So a row draws where its node is, not where the
  layout authored the row's template; the text keeps the template's font
  (prop 13) and alignment. `FUN_0029e230` (Main Menu, class 3) first moves
  every node of list 19 by (0, -20) (the literal at 0x29e2d4), registers
  widgets 0, 1, 0x17, 0x15 and 0x1a, and creates a sixth row (widget 0x1f,
  font 4 through `FUN_002e5578(4)`, text 0x0306). The traced bindings:
  class 3 on list 19 with the move, 5 on list 0, 6 on list 5, 102..104 on
  list 0 (`screen_binding`, `screen_rows`); other screens keep the generic
  dominant-group layout. A widget beyond the nodes stays hidden.
- *The row widget* (`FUN_0021c590`): idle colour 0xff88a0be (+0xd4),
  highlighted 0xfffbac00 (+0xd8), A-R-G-B words (the text draw
  `FUN_00238d20` packs bits 16..23 into the GS red byte); the layout loader
  `FUN_0021c750` never reads prop 10, so the layouts' cream does not reach
  the text. The highlighted row's text alpha pulses (`FUN_0021cd28`, byte
  +0xc0): down by 5 from 255 until under 127 (125 kept), up by 5 until it
  would reach 255 (255 stored), 52 updates a period (`row_pulse`), every
  row starting at 255 when the screen builds. The two stars either side of
  a highlighted row are the widget's arrow children (`FUN_0021c628`:
  `XACOMMON` sheet 0, image 1, frame 0; anchors right/middle and
  left/middle), placed by `FUN_0021c9e0` at the text's left edge less the
  5-pixel margin (+0xcc, 0x40a00000) and its right edge plus 5, on the
  text's middle.
- *The help line* (kind 5, `FUN_0021af18`): colour 0xffc99b0d; its loader
  `FUN_0021afa8` passes 1 to `FUN_002e5578` whatever prop 13 says and reads
  only the text (prop 15), so font 1 at the element's point and anchor,
  breaking where the text's `\n` does.
- *Button legends* (kind 34, `FUN_0021a530`, loader `FUN_0021a600`): up to
  four pairs of a label (props 56..59, text ids; the first 0 ends them) and
  an icon (props 60..63, action ids) in the element's font, colour
  0xff98a8b9 unless prop 10 is set (-1 keeps it). The icon frames are
  `XACOMMON` sheet 1, image 0, as `FUN_002127c0` picks them with the region
  word (`FUN_0030fa50()+0x1c`) at 0, the US default: action 0 Cross (frame
  3), 1 Triangle (2), 2 Square (1), 3 and 5 Circle (0), 4 frame 9, 6
  Select (19). `FUN_0021ab10` lays the pairs out from the last: icon, 6
  pixels, label, 22 pixels between pairs; the loader flips the element's
  horizontal anchor, so a right-anchored legend ends at its point. On
  these screens the legends replace the rewrite's own key prompt.
- *The navigation rule* (`FUN_0021f8f0`): `FUN_00221ba8` reads a node's
  modes (props 25, 27, 29, 31) and links (26, 28, 30, 32) through
  `FUN_002e3b18`, which answers 0 for a prop the element lacks, so a
  missing mode is 0 (no step) and a missing link is node 0; mode 1 scrolls
  the list (`FUN_0021fa48`); mode 2 follows the link to the node of that
  id in the same list, lands there when `FUN_00222770` finds a bound widget
  with +0x5b bit 1 set, else recurses from that node in the same direction
  (a ring of unavailable nodes would recurse for ever; the rewrite stops).
  `FUN_00222590`/`FUN_00222678` force mode 2 and take the direction from a
  bound kind-36 widget's byte +0xb0; no layout authors kind 36 and none of
  these screens binds one, so the node's own props decide. The rewrite's
  earlier reading of a node prop 36 as the override was its own invention
  and is gone.
- *Window size.* `STREET_WINDOW=WxH` opens the window at another size for
  layout checks (640x480 and widescreen screenshots of Main Menu and Play
  It taken for the merge).

Tests: `look.rs` (`steps_follow_the_navigation_graph`,
`navigation_flags_disable_or_scroll_instead_of_following_links`,
`dynamic_navigation_recursively_skips_unavailable_widgets`,
`a_disabled_mode_takes_no_step_and_node_props_carry_no_kind36_override`,
`the_highlighted_row_pulses_like_fun_0021cd28`,
`legend_icons_follow_the_us_region_table`,
`screen_rows_sit_on_their_nodes_in_registration_order`,
`screen_row_registrations_preserve_native_order_over_geometry`).

Not compared: a screenshot of the original against the rewrite's at the
same screen (the row positions, colours and pulse are static readings);
the other screens' constructors; the legend table for the other region
words; the help line's authored line breaks on every screen.


## The way into a Pick Up Game: Play It to Choose Players (2026-10-15)

Static trace of `SLUS_206.51` (the Ghidra decompile, with the raw MIPS
where Ghidra cut a function short at `FUN_0030fa50`, the settings getter
it takes for a non-returning call), checked against the original in the
project's PCSX2 copy from a fresh boot (screenshots under
`$EVIDENCE/street-prematch-original/`, 640x480 in the layouts' space:
the PS2 frame holds rows 16..464, so each grab is scaled to 640x448 and
set 16 down).

- *Play It's confirm* (`FUN_002bf428`, the class-5 input handler, action
  0x23): the focused row (`FUN_00220880` of list 0) picks the mode
  (`FUN_001dba68`) and the next screen through `FUN_002d7cb0`: row 0xf
  Pick Up Game mode 2, 0x11 League Challenge mode 4 and 0x12 Be A Legend
  mode 5 all go to class 0x69, Difficulty; row 0x10 Street School mode 1
  goes to class 0xc (User Setup from Play It, next the loading class
  0x2c). Cancel (0x25) returns to class 3.
- *The factory* (`FUN_002e55c0`): 0x69 → `FUN_00284100` (0xf0 bytes); 0xc
  → `FUN_00297848(.., 1, 5, 0x2c)`; 0xd, 0xf, 0x10, 0x12, 0x13 →
  `FUN_00297848(.., 1, 0x69, 0x1b)`; 0xe, 0x11 → `FUN_00297848(.., 1,
  0x69, 0x19)`; 0x19 → `FUN_0026f4f0`, 0x1b → `FUN_00270098`, 0x1c →
  `FUN_00270ee0`, 0x1d → `FUN_00269b28` (Choose Players), 0x21 →
  `FUN_00267b00` (Choose Court), 0x27 → `FUN_002c8ad0` (Set Challenge).
  So classes 0xc..0x13 are one User Setup screen told where it came from
  (`+0x100`) and where it goes (`+0xfe`); 0x19..0x1c are Choose Team.
- *The order for a Pick Up Game*: 5 Play It → 0x69 Difficulty → 0xd User
  Setup → 0x1b Choose Team (0x1c when people hold both sides) → 0x1d
  Choose Players → 0x21 Choose Court → 0x27 Set Challenge. After a game
  `FUN_0029d920` sends modes 2 and 3 to 0x1c when `FUN_001db280` finds
  people on both sides, else 0x1b (0x14 for a new reward, 0xd for an
  accepted challenge).
- *The transition* (`FUN_002ddb40`) enables the incoming screen and
  places its loop movie as before (see "City loop placement"): Difficulty
  (`FUN_00284f78`, vtable slot +0x15c) answers `xlp0005.nfx` at (265,
  240), or `sameloop` from class 5; User Setup and Choose Team keep the
  base slot, so the loop stops on them.

Built: `Page::Difficulty` between Play It and Choose Sides (`menu.rs`);
the pages keep the rewrite's order after it (home team, home players,
away team, away players), which differs from the original's one Choose
Team and one Choose Players screen for both sides.

## Difficulty (class 0x69, layout 105)

- Constructor `FUN_00284100`: `FUN_002d7720(screen, 0x69, 0x12)` (the
  Main Menu's backdrop), vtable `0x482220`, the three transition tables
  at `0x4823e0`/`0x4823a0` like Play It's.
- Build `FUN_00284280`: marks the previous screen 3 or 5 for the blur
  (`+0xd4` 0xff), clears the four controllers' flags
  (`FUN_001dbda0(rec, k, 0)`), pairs list 0's nodes, registers rows 0x11,
  0x10, 0xf in that order (`FUN_0021c628(.., 1)` for the stars,
  `FUN_0021f508`), so the nodes at (312, 142), (312, 181), (312, 220)
  carry Legendary (text 0x4803), Mad Game (0x4802), Got Game (0x4801),
  the hardest on top; selects the row of the saved level (`rec+0x114+0x18
  & 3`: 0 → 0xf, 1 → 0x10, 2 → 0x11) and sets the help (element 10) to
  0x4804, 0x4805 or 0x4806 ("Basic / Medium / Hard Difficulty."); the
  update `FUN_00284b60` keeps the help on the focused row.
- Input `FUN_00284848`: confirm stores the level (`FUN_001d74a8(rec+0xfc,
  0 | 1 | 2)`), copies three words of the rules (`+0x114..+0x124` to
  `+0x12c..`), flags the pressing controller active (`FUN_001dbda0(rec,
  pad, 1)`) and goes to 0xd (mode 2), 0xe (mode 4) or 0x36, the story
  menu (mode 5); cancel returns to Play It (class 5).
- Layout 105: title 0x4800 "Difficulty" at (60, 42), font 6; help at
  (58, 420); legends Back (540, 400, font 1, action 1) and Help (555,
  67, action 2).

Built: `screen_binding(105)` = list 0, rows [17, 16, 15]; the page's
rows, help and legends come through the registered-row path
(`Style::Screen(105)`). Confirm sets `MatchSetup.rules.difficulty`
(Hard, Medium, Easy by row) and opens Choose Sides. Test:
`the_prematch_layouts_and_sprites_hold_the_traced_pieces` (disc).

Compared: the original's screenshot (`difficulty.png`) and the rewrite's
show the three rows on their nodes with the stars on the saved level and
the help under the picture; the row text sits a few pixels lower on the
PS2 than at the node's point (the row widget's own text offset is not
traced).

## User Setup (classes 0xc..0x13, layout 11)

- Constructor `FUN_00297848(screen, 1, from, next)`: `FUN_002d7720(screen,
  0xb, 0xd)`, so layout class 11 (blob 5) and backdrop 13, vtable
  `0x487840`; `+0xfe` = `next`, `+0x100` = `from`, `+0xf8` = 4.
- Layout 11: title 0x800 "User Setup" at (56, 58); "vs" (0x801) at
  (329, 154), font 5, centred; the help at (58, 424); the Help legend at
  (555, 67); four kind-13 columns, ids 1..4, at x 57, 189, 321, 453, y
  113, 129x276 authored, prop 49 the controller 0..3.
- Build `FUN_002979a8`: the help (element 7) is `main_help.txt` 14 (text
  0x40e, "Choose sides and enter a name to track records.") in mode 2,
  else 13 (0x40d); every column's input goes to its own controller
  (`FUN_00249368(col, k, 1)`); the owner's column (`rec+0xd4`) starts in
  state 10 (the side chooser) in the Pick Up modes, 3 or 4 (the names or
  the keyboard) in modes 4 and 1, the others in state 1; the owner's is
  the controller that confirmed on Play It.
- The column widget (`FUN_002401b8`, kind 13, 0x930 bytes): its frame
  (children 0..9: four 4-wide bars in the player's colour, the top-left
  corner cut, 2-wide black lines inside; `FUN_00243730` colours them
  0xbd2908, 0xefce19, 0x083ae6, 0x639c29 for 1P..4P and swaps the tab's
  palette, child 10, `XACOMMON` sheet 0 image 2 frame 0 at (18, 0)); then
  its texts and lists, each placed at the column's point plus an offset
  (the tail of the constructor is cut in the decompile; read from the raw
  MIPS at `0x240b30..0x2436a0`):
  - 0xb, the message (font 0, 1 left of the middle, 18+21+37 = 76 down):
    0x4d02 "Press START To Enter Game", or from `FUN_00243730` and the
    update `FUN_002445c0` when the port is empty 0x4d00 with the port's
    name 0x4b03/0x4b04 ("1", "2") for ports 1 and 2 and 0x4d01 (the
    multitap) for 3 and 4, in 0x88a0be; the frame at alpha 0x7f
    (`FUN_00244258`, `FUN_002444a8`);
  - 0xf "Player" and 0xe the number (font 5, 18 and 39 down; 1P's number 7
    left);
  - 0x10 and 0x11 (font 3, 75 and 110 down): "Enter" / "Name" (0x804,
    0x805, or 0x80a, 0x80b over the keyboard), "Select" / "Name" (0x81b,
    0x81c);
  - list 0x18 with nodes 0x19 (65.5, 151) and 0x1a (65.5, 185) and rows
    0x1b "Yes" and 0x1c "No" (`common.txt` 2, 3, font 3, stars when
    lit);
  - 0x1e "No Name Entered" (0x817, font 2, 45 down) and 0x1f "User stats
    cannot be saved." (0x818, font 0, 146 down);
  - 0x20 the name, 0x22 "%1dW - %2dL" (0x81d), 0x23 "WIN%" and 0x24 the
    average, 0x26..0x2f the five career rows (labels 0x80f..0x813 and
    values "%1N", 0x81f), 0x21 "CAREER" for the School panel;
  - 0x32 and 0x33 "Ready" (0x819, font 5), 0x3f the side's word (font 2,
    76 down);
  - 0x43 the button icon and 0x44 "Continue" (0x816) at (16, 285), 0.8 of
    the icon's size;
  - 0x1d the keyboard (`FUN_0023c7b8`) at (6, 140);
  - the side chooser, list 0x34: nodes 0x35, 0x36, 0x37 at (65.5, 108 +
    72k), rows 0x38 "AWAY", 0x39 "Choose", 0x3a "HOME" (0x828, 0x829,
    0x82a, font 2, centred, stars on 0x38 and 0x3a); with the region word
    (`FUN_0030fa50()+0x1c`) 0, the US, the middle row is hidden
    (`FUN_00249020`) and the end nodes move to (65.5, 90) and (65.5,
    230); 0x40 the controller icon (`XACOMMON` sheet 1 image 2 frame 0,
    48x37) at (65.5-25, 151); 0x3b and 0x3c the arrows (kind 37,
    `FUN_0020f0b0`; `FUN_0020f280`'s table at `0x4659f0`: `XACOMMON`
    sheet 0 image 0 frame 0, 20x15, the second flipped top to bottom) at
    (65.5, 128) and (65.5, 198).
- The states (`FUN_00246880`; every child hidden first, then the state's
  shown): 0 empty/idle; 2 Enter Name with Yes/No; 3 Select Name over list
  0x12 (`FUN_00248118`: "Enter New" 0x81a, then each saved name, font 0,
  five nodes 20 apart from 155 down, scrolling past five); 4 the
  keyboard, its text "PLAYER%1d" (0x82d) until a key is pressed; 5 "The
  name already exists" (0x820/0x821); 6 "Maximum number of names"
  (0x826/0x827); 7 the career panel; 8 the Street School panel (0x824,
  0x825 "%1d%% Complete" of 26 lessons); 9 the guest; 10 the side chooser
  (`FUN_002445c0` moves it: no side, icon at 151 with both arrows; AWAY
  chosen, icon at 68, the AWAY row at 108, no arrows; HOME chosen, icon at
  208, the HOME row at 248, no arrows); 11 Ready (0xf, 0xe, 0x3f with
  0x828 or 0x82a by the side byte `+0xc4`, 0x32); 12 Continue (0x33, 0x43,
  0x44 as well); 13 backing out.
- Input `FUN_00245b88` (only the column's own controller): state 0 START
  → 11 (mode 4), 10 (modes 2, 3) or 2; 10: X with a side → 2, X on the
  hidden middle → sound 6, cancel → 13; 2: Yes → 3 if any name is saved
  (`FUN_001ef1b0(rec+0x1c3c)`) else 4, No → 9; 3: X on "Enter New" → 4 (6
  when 32 names exist), on a name → 7 (8 in mode 1); 4: DONE (0x5d) →
  7/8, a name already held → 5; 7/8/9: X → 11, cancel → 3 or 4; 11/12:
  L1 (0x31) turns on the cheat checker `FUN_002453e8` (see "Screen key
  dispatch"), cancel → 7, 8 or 2.
- The screen's update `FUN_00298210`: at most two columns a side (a
  third is pushed back to no side, `FUN_002487c0`; a full side disables
  that row, `FUN_002486d0`); once every active column is in 11, every
  column goes to 12 and the help becomes 0x82c "Enter cheat codes now."
  (`+0xf0`); a column in 13 takes the screen back to class 0x69 (5 in
  mode 1), clearing the controllers.
- The screen's confirm `FUN_00297c40` (X on a column in state 12 once
  `+0xf0` is set): mode 2 counts the columns' sides (`+0xc4` 0 away, 1
  home) into `FUN_001dbac0`, sets mode 3 for two people on one side, and
  goes to 0x1c when both sides have someone (or 1 and 1) else 0x1b; mode 4
  goes to 0x29 or the class in `+0xfe`; each named column's record is
  loaded (`FUN_001ee450`).

Built: `users.rs` draws the columns from these offsets for every state
the rewrite reaches: Choose Sides shows every controller's column in
state 10 (its own pad moves it: up or left to AWAY, down or right to
HOME) and "Press START" for one with no side; the name page shows each
seat's column through Enter Name (Yes/No), Select Name (the saved names,
"Enter New" first), the keyboard (cursor on DONE, "PLAYER%1d" until a
key), the career or guest panel, Ready and Continue, driven by one cursor
that left and right move between columns (the rewrite's own; the
original's columns each answer only their controller, at once). Empty
ports show the insert message with the port's number. `menu.rs`:
`UserStage`, `users_navigate`, `user_columns`. Tests: `users.rs`
(`the_side_chooser_keeps_its_order_and_moves_the_icon_past_the_chosen_row`,
`the_prompt_and_list_rows_sit_where_the_constructor_puts_them`,
`the_default_name_numbers_the_player`), the disc test above.

Compared: the original's `usersetup_sides.png`, `usersetup_away.png`,
`usersetup_home.png`, `usersetup_ask.png`, `usersetup_yes.png`,
`usersetup_typed.png`, `usersetup_named.png`, `usersetup_ready.png`
against the rewrite's. Not ported: the message's blink on the empty
columns, states 5, 6 and 8 (the rewrite reports a taken name on its
message line), the cheat checker's own column states, the Street School
panel.

## Choose Team (classes 0x19..0x1c, layout 26)

- Constructors: `FUN_0026e1a8(screen, class, backdrop)` is the base (vtable
  `0x477ee8`): `+0xf0`/`+0xf4` 1, `+0xf8`/`+0xfc` -1 (the sides' teams,
  not yet picked), elements 0x36..0x39 hidden, every controller's input
  to the screen. `FUN_0026f4f0` makes class 0x19 (layout 0x1a) for the league
  Challenge; `FUN_0026fb00` makes 0x1b and 0x1c as `FUN_0026e1a8(screen,
  0x1a, 7)`, layout class 26 (blob 7) and backdrop 7, with `+0x100`/
  `+0x104` (the sides ready) and `+0x10c`/`+0x110` (the lists) cleared.
  0x1b's vtable is `0x4779a8`, 0x1c's `0x4777e8`.
- Build `FUN_0026fb88` (both): the lists are elements 0 (AWAY, left) and
  0x21 (HOME, right); each gets, through `FUN_0021f508` in this order
  (`FUN_0026e5c8`): a hidden placeholder (`FUN_00221770`, id base +
  0x800, `+0xb0` 3), the node for team 0x27 All-Stars, the node for 0x26
  Old School, a placeholder (base + 0x803, `+0xb0` 2); then
  (`FUN_0026e3e0`) each created team in the save's list (`rec+0xb57c`)
  and team 0x2f Team Lunatics when `FUN_001ed118` says it is unlocked;
  then (`FUN_0026e558`) teams 0..0x1c, the league's 29; then 0x30 Random league
  Team; then (`FUN_0026e7f0`) placeholders to the end of the row of four
  (`+0xfa` 4 columns). A team's node (`FUN_0026e2e8`): a kind-24 logo
  widget (`FUN_00222a50`) with id base | team, sheet archive 1
  (`FUN_00222dd8(w, 1)`, `fe/xateams.bin`), the team's logo field as the
  sheet (`+0xc0`, slot +0x14c `FUN_00222d78`) and image 1, the 55x53 tile
  (`+0xd0` 0, `FUN_00222da8`; image 0 is the large picture); the two
  legend teams' nodes show the sheet's double tile instead
  (`FUN_00222eb8(w, 1, 0x28, 0, 2 | 1)` for All-Stars by the setting bit
  `rec+0x114 >> 9 & 7 == 2`, `(w, 1, 0x27, 0, 1)` for Old School), half a
  cell left (`+0xd4`). The nodes sit on the list's kind-10 nodes
  (`FUN_0021f230`/`FUN_00222420`): x 110, 165, 220, 275 and y 321, 373
  on the left, 252 further on the right, so the row of four spans the
  list and the double tiles cover cells 0-1 and 2-3. `FUN_0026eb08` takes
  elements 0x13/0x12 (0x34/0x33) as the list's scroll-arrow places, (69,
  304) and (69, 387) left, (567, 304) and (567, 387) right, and deletes
  them. The base build `FUN_0026ee20` gives the corner pieces 0x1a and
  0x20 `XACOMMON` sheet 0 image 2 frame 3 turned -90 (`+0xec`) and the
  heading tabs 0x19 and 0x1f frame 6; the kind-32 lines (`FUN_00214138`)
  are frame 2 stretched to the element's size in the palette of prop 8
  (0 left, 2 right); `FUN_0026f0c0` later swaps the palette of elements
  0x15..0x1a or 0x1b..0x20 to the controller's (0..3) or back to the
  sheet's own for the computer (-1 in the table at `0x3f7f80`).
- 0x1b's build `FUN_00270108` (one side of people): the side of the
  owner (`FUN_001db308(rec, rec+0xd4)`) is enabled (`FUN_00270848`: its
  frame in the owner's colour, its list live, the cursor tags) and the
  other darkened (`FUN_00270428`: a 263x321 black rectangle at alpha 0x7f
  (`FUN_002332d0`, element 0x15b at (53, 97) or (323, 97)), the badge
  0x15c (`XAFESCRN` sheet 0 image 7 frame 33) at (53, 401) or (323, 401),
  its meters' `+0xb4` cleared (the values stay, `+0xba`: the grab shows
  the computer's full crowns under the dark), its lines 0x16, 0x19, 0x41
  (or 0x1c, 0x1f, 0x42) disabled, its list off). 0x1c's build `FUN_00270f50`
  (`FUN_00271478`) gives each side's list to the controllers on it and
  colours each frame by its last controller.
- The cursor tags (`FUN_0026e8e8`, `FUN_0021b2f8`): 0x154/0x155 show
  `XAFESCRN` sheet 0 image 0 frame 1 (55x53) on a team's tile, 0x159/0x15a
  frame 0 (111x54) on a double tile (`FUN_0026fdb0` picks by the team,
  `FUN_0026e9c8` moves it to the node, with an offset for the double
  tiles).
- The update `FUN_00270000` → `FUN_0026f238` on a change of the focused
  team: the large logo (elements 0xe/0x2f, `FUN_0026f1c8`: image 0 at
  scale 0.9, `FUN_00222e88`), the name "%s %s" (city, nickname; elements
  10/0x2b, scale 0.9 in `+0xb0`/`+0xb8`), the meters 0x3c, 0xc, 0x3d
  (0x3f, 0x2d, 0x40) set to `FUN_001ec028(team, 4 | 5 | 6)`.
- `FUN_001ec028`: kinds 4 (offense) and 5 (defense) take each player's
  bars (`FUN_001d7f48`: Shots, Dunks, Handles rounded over three; Blocks,
  Steals over two), average them as `round((sum + n - 1 + 0.5) / n)`,
  capped at 5; 6 (overall) is `round((offense + defense + 0.5) / 2)`;
  teams 0x26 and 0x27 answer 6 for all three.
- The meter (kind 20, `FUN_002182f0`/`FUN_00218458`): child 0 `XAFESCRN`
  sheet 1 image 0 frame 0 (54x17, the crown) and child 1 frame 10 (51x8,
  the five segments) in the palette of the level (`0x467180`, levels
  1..5; the sheet's own at 0); level 6 uses frames 1 and 11.
- Input 0x1b (`FUN_00270c78`): confirm by a plugged controller marks the
  person's side ready (`FUN_00270b28`: "Ready", element 0x14/0x35 shown,
  the cursor hidden, the list off) and enables the computer's side for
  the same person; with both ready `FUN_0026fd00` stores the teams
  (`FUN_001dbb20(rec, 0, left)`, `(rec, 1, right)`, Random drawing `rand()
  % 29` in `FUN_0026ef88`) and goes to 0x1d. Cancel with neither ready
  returns to the User Setup class for the people (`FUN_0026ec20`: mode 2
  by their count 0xd, 0x10, 0x12, 0x13; mode 3 0xf; mode 4 0xe/0x11); with
  one ready it undoes it. 0x1c (`FUN_002710a0`): each list's confirm
  readies its own side; cancel un-readies it, or leaves when neither is.
- Layout 26: title 0x100 "Choose Team" at (50, 38), font 6; the help
  0x109 at (58, 420), font 1; Help legend at (555, 62); AWAY/HOME
  (`chooseteam.txt` 0, 1, font 0, 0xf6f4d7) at (276, 78) and (379, 78)
  over the tabs at (218, 74) and (323, 74); the large logos at (183, 149)
  and (454, 149); the names at (183, 196) and (454, 196), font 2,
  0xdcdcbb; the meters at x 70, 187 (centred), 237 and 341, 458, 508, y
  231, 60x25, their labels Offense, Defense, Overall (`common_fe.txt` 1,
  3, 2, font 1, 0x8394df) centred at x 96, 182, 263 and 367, 453, 534, y
  266; "Ready" (`common_fe.txt` 10, font 6) centred at (192, 345) and
  (443, 345); the lines 21..24 (53, 93, 4x306), (53, 93, 262x4), (311,
  93, 4x324), (72, 413, 240x4) and 27..30 (324, 93, 4x306), (324, 93,
  261x4), (581, 93, 4x324), (342, 413, 240x4); the corners at (52, 396)
  and (323, 396).

Built: `chooseteam.rs` (`grid`, `step`, `scroll`, `pick`, `meters`,
`pieces`) and `art.rs` (`logos_big`, `logos_wide`). The page for a side
draws both panels: the one being picked on with the cursor frame (the
pulse of the row widgets) and the scroll arrows, the other as it stands:
darkened with the badge for the computer's side before it is picked, with
its frame in its person's colour, or under "Ready" once chosen. Up, down,
left and right step the grid as the nodes link. Tests: `chooseteam.rs`
(`steps_follow_the_rows_and_skip_blanks_without_wrapping`,
`the_window_scrolls_a_row_at_a_time`,
`random_draws_one_of_the_league_teams`,
`the_panels_mirror_the_layout_271_apart`), the disc test above.

Compared: `chooseteam.png`, `chooseteam_down.png`, `chooseteam_ready.png`
and `chooseteam_cpu_right.png` against the rewrite's pages. The rewrite's
own: the meter's unlit segments are shaded (the level palettes of
`XPAL_FE` are not applied), Random's draw is a hash of the save's game
count, and the created teams are those the rewrite appends to the
database. Fixed in the third pass (2026-10-19): the double tiles sit at
their node -28 (All-Stars) and +28 (Old School), the cursor's offsets
(`0x4d1200`, `0x4d1208`, read from RAM; the tiles' own `+0xd4` hold -14
and +14), so Old School covers cells 2-3; the name is anchored by its
top's middle (align 1), not centred; the computer's panel keeps its
meters' values; the live list dims every tile but the focused one to
0x9f (`FUN_0026eb08` sets its `+0x124`); Team Lunatics joins the grid
once Rewards card 2 is the save's (`FUN_001ed118`, `FUN_001d26e8`).
`chooseteam.png` scores 13.0 at (1, 2) (15.5 with the name alone
fixed). Not captured: class 0x1c with people on
both sides (one pad on this machine).


## Choose Court (class 0x21, layout 33)

Traced 2026-10-15 from the decompile and checked against the original in
the project's PCSX2 copy from a fresh boot (grabs under
`$EVIDENCE/street-prematch-original/choosecourt*.png`).

- Constructor `FUN_00267b00`: `FUN_002d7720(screen, 0x21, 4)` (layout class
  33, blob 10, backdrop 4), vtables `0x475f48`/`0x475f88`; `+0xf0` the
  focused widget (6), `+0xf4` 1000 (the widget last seen), `+0x100` the
  Random flag, `+0x108` the Random court, `+0x10c` 0.25 (the Random
  period), and the four frame lines' ends at `+0x114..+0x150`: (318, 144)
  to (574, 144), (319, 144) to (319, 323), (318, 323) to (574, 323), (572,
  144) to (572, 323).
- Build `FUN_00267f10`: pairs list 0x61 with its kind-10 nodes
  (`FUN_0021f580`, `FUN_0021f230`); registers twelve plain widgets
  (`FUN_00248a90`, ids 0..11) each with a picture (`FUN_0020fb20`, ids
  12..23) showing `XACOURTS` sheet `k`, image 0, frame 1 (the 81x77 tile;
  `FUN_00210048(w, 3, k, 0, 1)`): widget 11 sheet 12's frame 0 (Random:
  that sheet authors the tile alone), widget 10 sheet 13 (the "?") while
  court 10 is locked (`FUN_001d37b0` through `FUN_001d3490`); moves each picture onto its widget's node (vtable
  +0x124) and enables it when its court is open (+0x10c) or disables it
  (+0x114); creates "Locked" labels (ids 0x28..0x32, `FUN_00238a78`, font
  1 through `FUN_002e5578(1)`, colour 0xff838497, shadow 0xff000000,
  right-anchored, text 0x012b `common_fe.txt` 43) at the tile's (x + 81 -
  12, y + 77 - 25), shown for a locked court; the big picture 0x15b3 (sheet
  6, frame 0, 248x174) at (321, 146); "Locked" 0x115d (font 5, 0xffdcdcbb,
  centred) at (450, 382); the cursor tag 0x15b4 (`FUN_0021b2f8(.., 2)`:
  `FUN_0021b550` case 2, `XAFESCRN` sheet 0, image 0, frame 3, 80x77,
  anchored left/top) on widget 6's point; the stars 0x51/0x52 as
  `XACOMMON` sheet 0, image 1, frame 2; the lines 0x24..0x27
  (`FUN_0021e350`, 3 wide, `FUN_00233e00(.., 0xff91adc0)`); a transition
  table `FUN_002d83a8(.., 0, 0x3fdf58)`; and selects node 0x5a
  (`FUN_0021f6a8`: prop 34 = 6, the top-left tile). The nodes by prop 34:
  0 (230, 98), 1 (147, 98), 2 (66, 254), 3 (66, 176), 4 (147, 176), 5
  (230, 176), 6 (66, 98), 7 (147, 254), 8 (230, 254), 9 (66, 332), 10
  (147, 332), 11 (230, 332); their links (props 26, 28, 30, 32) wrap the
  grid in both directions.
- Enter `FUN_00267ca8`: list 0x61 takes every plugged controller
  (`FUN_00249368(list, k, 1)`), the pictures are placed again, and
  `+0x104` (the update's gate) is set when `+0x110` is -1.
- Update `FUN_002688f0`: with the Random flag set, every 0.25 s
  (`DAT_003f7ce8` counts frames over 60) `rand() % 11` (`FUN_003a16b8`)
  is redrawn until it is an open court other than `DAT_003f7cec`, the
  last shown; the pictures are re-placed; on a change of the focused
  widget (`FUN_00220880`) `+0xf0` becomes its id (11 sets the Random flag)
  and `FUN_00268ce0` runs.
- Change `FUN_00268ce0`: the help (element 0x50) is 0xd04 "Highlight and
  select your court." (0xd09 "Randomly choose a court to play on." on
  Random); the name (0x62, font 2, 0xffdcdcbb, left/middle at (338, 107))
  takes the court's name (`FUN_001d3670`), 0xd08 "Random Select" on
  Random, "?????" (`0x475f40`) for court 10 while locked; the stars move
  to (name.x - 20, 106) and (name.x + width + 2, 106); the location (0x60,
  font 1, 0xff7c83d4, (338, 128)) takes `FUN_001d36e8` unless the region
  word (`rec+0x12c >> 9 & 7`) is 2, 3 or 4, and hides for Random and the
  locked court 10; the big picture takes sheet `+0xf0` (`+0x108` for
  Random; 13 for court 10 locked), disabled with the lines in 0xff3b45a2
  and "Locked" shown for a locked court, enabled with 0xff91adc0
  otherwise; the cursor moves to the focused widget's point.
- Input `FUN_002686e0` (a plugged controller): 0x25 goes to class 0x1d
  (Choose Players); 0x23 on a locked court plays sound 6; on an open one
  sound 3, `FUN_001dbc88(rec, court)` (`rec+0xcc`) and `FUN_00269610`: the
  dialog `FUN_0024f508(.., 0xd05)` with a hidden picture and the rows
  "Play Game" (0xd06) and "My Rules" (0xd03) (font 3, centred, stars) at
  (402, 298) and (402, 332) (`DAT_004cf4a8`, `DAT_004cf4b8`), over the
  base dialog's 660x500 black at alpha 0x4f (`FUN_0025beb0`,
  `FUN_00233e00(.., 0x4f000000)`); 0x27 opens the help popup
  (`FUN_00250000(.., 0x28)`); 0x3e the exit dialog 0xd06 (`FUN_0024bfa0`).
  The dialog's answer `FUN_002696a8`: row 0 goes to class 0x2c (loading),
  the other to 0x27 (Set Challenge); the exit dialog's Yes to class 3.
- The loop movie `FUN_00269a58`: `xmt0004.nfx` from classes 0x1d and
  0x2b, `xmtblur.nfx` from 0x1a.
- The dialog's picture is `XAFEBGEX` entry 6 ("My_r", 512x256), whose
  painted part spans (112, 94) to (541, 374) in the grab: 1.85 times its
  texture. How the dialog layer (`FUN_00260a68`, `FUN_002d9a30`) places
  it is not traced.
- Open courts: `FUN_001d37b0` answers through the reward records
  (`FUN_001d2b38(2)`, `FUN_001e0398`): the court's category-2 card is
  owned; the fresh save opens courts 0, 1 and 6 (the three cards that
  cost nothing: `XDBCARD` indices 0..2 through `COURT_RECORDS`).

Built: `choosecourt.rs` (`NODES`, `sheet`, `random_court`, `court_of`,
`open`, `pieces`), `art.rs` (`courts`, `rules_dialog`),
`rewards::court_unlocked`, `look.rs` (`cursor_court`, `name_star`),
`menu.rs` (`Page::ChooseCourt` over `Style::Screen(33)` with the layout's
graph for all four directions, `Page::PlayGameAsk`, `dialog_pieces`).
The Random tile redraws every 15 updates (the rewrite's hash of the
update count and the save's game count stands in for `rand()`); the
locked picture's shade and the dialog picture's placement are the
rewrite's. Tests: `choosecourt.rs`
(`the_tiles_fill_three_columns_and_four_rows_in_registration_order`,
`the_hidden_court_and_the_random_tile_take_the_extra_sheets`,
`the_random_tile_draws_an_open_court_other_than_the_last`,
`the_frame_lines_box_the_picture_three_wide`), `fe_disc.rs`
(`the_court_and_rules_layouts_and_sheets_hold_the_traced_pieces`).

Compared: `choosecourt.png` (the top-left tile, Uptown, on entry),
`choosecourt_down.png` (a locked court: dimmed picture, dark lines,
"Locked"), `choosecourt_locked10.png` ("?????" and the "?" picture),
`choosecourt_random.png`, `choosecourt_dialog.png` and
`choosecourt_dialog_rules.png` against the rewrite's pages.

## Set Challenge, "My Rules" (class 0x27, layout 39)

- Constructor `FUN_002c8ad0`: `FUN_002d7720(screen, 0x27, 0x19)` (layout
  class 39, blob 11, backdrop 25), vtables `0x490c60`/`0x490ca0`; `+0xf4`
  the focused widget (0x458), `+0xfc` the mode text id (0xe06, Pick Up),
  `+0xfe` the clock text id (0xe10, 24 seconds), `+0x100`/`+0x102` the
  arrows' flash counters (-1), `+0x104` 1 (Gamebreaker level 1); three
  "Play To" records: points at `+0x108` (value 21, min 1, max 50, step 1,
  side 1, spot 0, 50; the texts "Points" 0xe31 at +0x1c and "pts" 0xe30 at
  +0x9c), trick points at `+0x1e4` (100,000, min 100,000, max 10,000,000,
  step 50,000, side 1, spot 0, 10,000,000), the Gamebreaker at `+0x2c0`
  (100, 100, 100, step 10, side 0, spot 0, 90; "Level 1 Gamebreaker"
  0xe27 and "%%%%" 0xe32); `+0x39c` points at the points record.
- Build `FUN_002c8d08`: the arrows 0x45f (`XACOMMON` sheet 0, image 0,
  frame 1, 17x20, pointing left) and 0x460 (frame 0 turned 180, `+0xec`)
  at `DAT_004d5408` (254, 110) and `DAT_004d5410` (554, 113); list 0x26
  paired with its nodes and the rows 0x25, 0x27, 0x29, 0x28, 0x24
  registered in that order (nodes (151, 104), (151, 169), (151, 243),
  (151, 315), (395, 374); the Play Game template right-anchored; no
  `FUN_0021c628`, so these rows have no stars and only change colour); the
  values 0x458 (the mode, 0xe06..0xe0d), 0x459 ("%1N %2s" 0xe2b of the
  record's value and its unit) and 0x45c (the clock, 0xe0e..0xe11), font
  2, centred, 0xff88a0be, at (408, 110), (408, 174) and (408, 320); the
  groups 0x45a (`FUN_0023e6b0`, kind 0x12) at (285, 244) with child 0
  right-anchored 118 in and child 1 left-anchored 133 in, font 1, holding
  the sides' names (each controller's name at `rec + k*0xd4 + 0xbd8`, or
  "Player%1N" 0xe33; home (`FUN_001db308` 1) in child 1, away in child
  0, two names joined by "\n" and that child moved up 18; "CPU" 0xe34 for
  a side with nobody) and 0x45b at (285, 268), its children 113 and 133
  in and 3 down, holding the spots.
- Input `FUN_002c9740` (a plugged controller; left 0x1d, right 0x1e, the
  flash counter set to 10): Game Mode cycles 0xe06..0xe0d and points
  `+0x39c` at the mode's record (modes 0, 2, 3, 5, 7 the points record
  with min 1; 4, League Scoring, min 2; 1 the trick record; 6 the
  Gamebreaker); Play To steps the value (`FUN_002cb6a0`: the record's
  step, 100,000 for trick points from a million) and clamps it
  (`FUN_002cba40`, sound 2 at a limit); in Gamebreaker Wins it toggles
  the level (`+0x104`, the record 100/90 or 200/190); Spot Points moves
  the spot toward one side (left: a home spot of nothing switches to
  away, a home spot shrinks, an away spot grows by `FUN_002cb620`'s step;
  right the mirror); Shot Clock cycles 0xe0e..0xe11; 0x23 on Play Game
  stores the mode (`FUN_001dbd48`), the clock (`FUN_001dbd60`), the value
  or the level (`FUN_001dbd78`) and the spot (`FUN_001dbd80(rec, side,
  amount)`), sound 8, class 0x2c; 0x25 sound 3 and the dialog 0xe14
  "Cancel Changes and Return to Choose Court?" (`FUN_002cb548`, Yes and
  No, No selected); 0x27 the help popup; 0x3e the exit dialog 0x457.
  `FUN_002cb490`: Yes goes to class 0x21; the exit's Yes to class 3.
- Update `FUN_002ca060`: the value under its minimum is raised to it;
  the focused row (`FUN_00220880`) sets `+0xf4`, moves the arrows to its
  pair (`DAT_004d5418..`: (254, 174)/(554, 177), (254, 255)/(554, 258),
  (254, 320)/(554, 323)), shows them (Play To hides the right one at the
  maximum and the left at the minimum except in Gamebreaker Wins; Play
  Game hides both), colours its value 0xfffbac00 and the others
  0xff88a0be, and sets the help (element 0x23): Game Mode 0xe15..0xe1b
  and 0xe3c by mode, Play To 0xe1d (trick 0xe1f, Gamebreaker 0xe1e), Spot
  Points 0xe20 (0xe21, 0xe22), Shot Clock 0xe23, Play Game 0xe25; Play To
  shows 0xe27/0xe28 for the levels; the spots show "%1N %2s" with "pts",
  or in Gamebreaker Wins "%1N %%" (0xe35) under 100, "GB1" (0xe2f) at it
  and "GB1 + %1N %%" (0xe36) past it, the unspotted side at 0; the
  arrows' alpha is `~(n * 10) & 0xff` while the counter n runs down.
  `FUN_002cb6d0` hides the spot row's arrow past the limit (the spot may
  reach the value less a step; both hide at 1 point with no spot) and
  drags a spot at or past the value back under it.
- The loop movie `FUN_002cbe70`: `xmt0005.nfx`.

Built: `myrules.rs` (`Edit`: the three records, `left`/`right`,
`settle`, `arrows`, the texts, `help_id`; `side_names`; `pieces`),
`look.rs` (`screen_binding(39)`, `arrow_left`/`arrow_right`),
`menu.rs` (`Page::MyRules` over `Style::Screen(39)`: the registered rows
with the values drawn over them, `Page::RulesCancelAsk`, the rules kept
from Choose Court's dialog and restored on Yes). The turbo spot of
Gamebreaker Wins is edited and shown but has no place in the sim's rules;
the cancel dialog's panel is the rewrite's drawing of the original's; the
rewrite's earlier Difficulty, turbo and cheat rows are gone from this
screen (the original has none: the difficulty is its own screen, the
cheats come from the codes). Tests: `myrules.rs` (eight), `fe_disc.rs`
(the layout and sheet test above).

Compared: `setchallenge.png`, `setchallenge_playto.png`,
`setchallenge_spot_right.png`, `setchallenge_spot_left.png`,
`setchallenge_clock_right.png`, `setchallenge_playgame.png`,
`setchallenge_mode1..7.png`, `setchallenge_mode6_spot.png`,
`setchallenge_cancel.png` against the rewrite's.

## The row offset, the palettes and the two-person Choose Team (2026-10-15)

- *The row text "a few pixels lower" on the PS2.* Measured over the
  earlier pairs (`difficulty.png`, `chooseteam.png`, `usersetup_*.png`
  against the rewrite's) by correlating each region's horizontal edges:
  the original's title, rows, help line and backdrop all sit 2 rows lower
  than the rewrite's, uniformly, so the difference was the grab's
  placement of the frame, not the row widget. `FUN_0021c9e0` (the row's
  layout) and `FUN_00238d20` (the text draw) add no vertical offset: the
  text draws top-anchored at its point (through `FUN_0023a3f8` and the
  font library's `FUN_003a2c98`), and the glyphs land at the font's
  per-glyph `top` as the rewrite's renderer puts them. What the two do
  change: the text widget's box (`FUN_00239e70`) is the font's text
  height (the FNTS header byte at 0x12: 29 for font 3, 33 for font 4) for
  its last line and the line spacing (the word at 0x24: 37, 43) for the
  others, so a one-line text's middle, bottom and the stars' middle sit
  by the text height; the rewrite used the line spacing, putting the
  stars 4 pixels low on font 3. `Font::text_height` carries the byte and
  `screen_row` measures by it. The grab offset: `tools/pcsx2_screens.py`
  now finds the game area under the window's menu bar by its first black
  row rather than assuming 22 pixels, since the two-row difference is the
  menu bar's height on this machine; the earlier grabs keep their two
  rows.
- *The palettes.* `XPAL_FE` sheet 0 holds the four controllers' palettes
  (`FUN_0026f0c0` and `FUN_00243730` swap them into the frame lines, the
  corners, the tabs: `FUN_00210368(w, 0, k)`), sheet 1 the meter's five
  level palettes (`0x467180` maps levels 1..5 to palettes 0..4), sheet 2
  twelve for the brackets' plates, sheet 3 four. Each entry is a 256x1
  palette block (type 0x21) with no pixels, stored in the GS's CSM1
  order. `ssh.rs` now keeps every palettised image's index plane and the
  palette-only entries in index order (`Sheet::palettes`,
  `Texture::in_palette`), and `look.rs` draws the tabs, lines, corners
  and the meter fills in the swapped palettes instead of tinting the
  decoded sprites.
- *Class 0x1c, people on both sides.* The runtime's `PCSX2.ini` now binds
  `[Pad2]` to the keyboard (8 5 7 9 the directions, M Cross, N Circle, B
  Triangle, V Square, P Select, 0 Start), the driver presses any letter
  or digit, and the two-person path was captured (`cc2_sides.png`,
  `cc2_sides_home.png`, `cc2_ask.png`, `cc2_guest.png`, `cc2_ready.png`,
  `chooseteam_two.png`, `chooseteam_two_ready.png`, `cc2_players.png`):
  both panels live at once, each framed in its person's colour with its
  own cursor and scroll arrows; a side's confirm covers its grid with
  "Ready" (`FUN_002710a0`) while the other keeps picking. The rewrite's
  Choose Team page draws the panel a page is picking on; with people on
  both sides it now draws the other side's panel as picking too (its
  cursor on its team), one page per side as before.

## The in-game screens' rest and the controller widget (2026-10-18)

Audit item 7's in-game screens. Traced raw; compared with the retained
grabs under `$EVIDENCE/street-ingame-ui-original` and `-after` (PCSX2 was
in use by another session, so no new grab was taken).

- **Disabled pause rows** dim to half brightness. Vtable `+0x114`
  (`FUN_002491d8`) disables a widget and its children; the text widget's
  disable hook (`+0x8c`, `FUN_00239990`) sets the brightness byte `+0x5a`
  to 0x7f when `+0x5b` bit 4 is set (every widget's constructor
  `FUN_00248a90` sets it), and the text draw `FUN_00238d20` scales red,
  green and blue by `+0x5a / 255`. `pause::pieces` dims a disabled row's
  label and value (`DISABLED_BRIGHTNESS`).
- **The court behind the end dialog is empty.** Game vtable `+0x17c`
  (`FUN_0013f0b0`) hides the referee, the ball and the six actors and
  `FUN_00314298` the people's markers once the game is won and no replay is
  asked for, delayed or playing (`FUN_0013e4c8` case 9, `FUN_00142648`);
  `post_3.png` shows the dialog over the empty court. `view::game_ended`
  does the same.
- **The controller widget** (`FUN_002144c8`) is one widget for the pause's
  Controller Layout page (at (70, 105), `FUN_002bc3f8`, last argument 0)
  and the front end's (class 0x37, at (70, 90), `FUN_0029c578`, last
  argument 1; `FUN_00273148` builds another). Its rows (`FUN_002164a0`,
  font 0, 0xffdcdcbb, at x 299 of the widget, the turbo rows at 329) and
  icons (`FUN_00216dd8`, at x 259 and 294) sit at y 25 (the stick, Move
  Player), 60 Cross, 85 Circle, 115 Triangle, 145 Square, 168/170 R1 R2,
  193/195 L1 L2, 225 R3 (Strategy, 0x401a) and 258 (Pause, 0x4018); on
  offence the face rows read, by the player's layout (`+0x100`), B: Shoot,
  Pass, Trick Move, N/A, and A or C: Pass, Shoot, N/A, Trick Move. The
  front end's page (`diagram.rs`) had the turbo rows and Strategy/Pause
  swapped and Triangle as Block / Rebound. Both pages now draw the rows
  with one function, `pause::controller_rows`, at the widget's place:
  `FUN_00216dd8` adds each icon middle-anchored (`FUN_0024a4e8(0x20)`)
  with its left edge at x 259 (the stick 262, R3 265, R2 and L2 294) and
  its middle at y 25 (the stick), 60, 85, 115, 145, 170 (R1, R2), 195
  (L1, L2), 225 (R3) and 255 (the arrow, frame 0x11), and `FUN_002164a0`
  the words (`diagram::offence_face` on offence). Against
  `orig_pause_layout.png` (Layout A, offence) the words fall where they
  did, the R2 and L2 icons within 2 pixels (the capture-placed rows had
  them 3 to 6 right), the face icons 1 to 2 left and the turbo icons 2
  low: the icons' rectangles are the rewrite's own from the sheet, not
  `FUN_00212748`'s frame table. Not traced: the frame 0x12 icon the
  widget adds at (344, 210), the cases the `FUN_0030fa50` cut hides
  (10..15), the pages' frames, the pad's picture and the band (placed as
  before, the pause's by its capture), the build's rest past its cut
  (`0x217a50..0x218074`) and the setters each caller runs after it; the
  song list (`FUN_00218e68`) likewise.
- **The pause panel's frame** measured against `pause_top.png`: the left,
  right and top edges within 1.5 pixels, the bottom 3.5 low, while the rows'
  text drifts from 0 to 2 pixels low down the panel. The panel is
  `FUN_00211760`'s drawing, already ported; the remainder looks like the
  grab's vertical mapping (about 1%), so nothing was moved.
- **Not ported:** the rematch rules dialog (`FUN_0025dc88`, 0x5df; the
  original's look `post_4.png`: Play To 0x570c, Spot Points 0x570d with a
  column per side, Continue 0x5709), so REMATCH still plays again at once;
  the Level 2 banner's element 9 (`FUN_002a8538`, twelve sprites); see
  `docs/TRACE.md` "The presentation's rest" for what each holds.

## The front end's grabs and how they compare (2026-10-18)

The second audit's piece 5 started here (branch `frontend-legend`). Every
screen that audit's section 5 listed was grabbed from
the original where a fresh boot and a few PINE writes could reach it: 160
grabs in `$EVIDENCE/street-frontend-original/`, indexed by its `INDEX.md`
(screen, class, layout, what each shows, the save states written and what
was not reached).

- *Driving without the focus.* `tools/pcsx2_screens.py` now posts its keys
  to PCSX2's display window (the `QWindowIcon` child of the top window,
  `WM_KEYDOWN`/`WM_KEYUP`) and grabs through `PrintWindow`, so the game is
  driven behind other windows and nothing is typed into the one in front;
  `STREET_SENDINPUT=1` keeps the old SendInput way. A minimised window is
  shown behind the others without activating it for the grab. `launch`
  starts pcsx2-qt detached and only when none is running. The default
  press is 30 ms: Play It and Choose Baller take a 60..120 ms press as two
  steps.
- *Reaching the story screens.* The cards stay unformatted (every "Save to
  memory card?" answered No); the career lives in save states (`INDEX.md`
  lists slots 3..8 and 10). PINE writes stand in for play where only the
  numbers matter: the match's target (`*0x3d30bc + 0xf8`), each actor's
  controller (`game + 0x24 + 4 * id`, 4 the computer; the person's own team
  on the computer and the other on an unused pad wins a game in about a
  minute), and the career's event tallies (the live block at `0x4bce38`,
  its counters at +0x0c.., tallies at +0x1c..; the copy at `0x4bf33c`).
- *The frame's offset.* The PS2's displayed frame sits 1 column right and
  3 rows down of the layout space on every screen measured (Play It,
  Difficulty, Choose Baller, the map; the shift that minimises the
  difference is the same in each quarter of the frame, so it is not a
  scale). PCSX2's display area is exactly 4:3 (684x513 under the menu bar
  here), so the crop is right and the offset is the game's own output.
  Comparisons move the rewrite by (1, 3); nothing in the rewrite moves.
  With that shift the matched Difficulty screen differs from its grab by
  10.9 a channel on average (11.3 before the titles below), which is the
  floor the PS2's filtering and the grab's scaling leave.
- *Translucent black in linear light.* The GS blends in the picture's own
  values; the rewrite's window composites its pieces in linear light (an
  sRGB target), so a black layer at the PS2's alpha left more light:
  Choose Baller's list behind its dialog kept 0.50..0.57 of its light in
  the rewrite against 0.26..0.45 in the original. `look::black_alpha` gives
  the alpha that darkens as the PS2 does (`1 - (1 - a) ^ 2.2`), used for the
  dialogs' dim (`pause::dialog`, Choose Court's and My Rules' 0x4f), the
  panels' bodies (`boards::panel`, so the result boards and the pause
  panel darken more than before), Choose Team's darkened side and the
  map's veil. Coloured translucent pieces are not corrected; where they lie
  over a dark panel (the city page's dimmed events) their alpha is drawn as
  a tint instead.

## Titles (kind 35) and the text widget's outline

The screens' titles drew as cream text over a two-colour drop shadow. The
title widget (`FUN_0020f6c0`, loader `FUN_0020f778`: prop 23 to `+0x4a8`,
prop 24 to `+0x580`) draws twice (`FUN_0020f7d0`) through the text widget's
draw (`FUN_00238d20`): first in prop 24's colour with a 3.5-pixel outline in
the same colour and no shadow, then in 0xffdcdcbb, whatever prop 10 says,
with a 1.3-pixel outline (`+0x4b8`/`+0x4bc`) in prop 23's colour. The text
draw lays an outline down as eight copies at the outline's x and y offsets
each way (alpha the sixth power of the widget's), a shadow (`+0x4ac` colour,
`+0x4b0`/`+0x4b4` offset) as one copy, then the text (alpha squared).
`look::title` draws every front-end title that way through `hud::Writer`
(which already had the text widget's outline). Against the grabs of Main
Menu, Play It, Difficulty and Choose Baller the titles now sit at the frame
offset with the original's light outer and dark inner outline.

## Choose Baller (class 0x36, layout 54)

- Constructor `FUN_002cd930(screen, mode)`: `FUN_002d7720(screen, 0x36, 2)`
  (backdrop 2), `+0xf8` the mode (0 Be A Legend, 1 for class 0x3d, My
  Team's Create A Baller list over the same layout).
- Build `FUN_002ce270`: for each of the ten slots it registers widgets `k`
  (the number, "%d." at `0x491778`), `k + 10` (the name) and `k + 20` (the
  progress) on list 0x46, so slot `k` sits on the nodes of prop 34 `3k`,
  `3k + 1`, `3k + 2`; only the name has the row widget's stars
  (`FUN_0021c628(.., 1)`). A used slot shows the baller's name
  (`FUN_001eb4d8`: first name, a space, last name) and "%1d/%2d" (0x1903)
  of `FUN_001eb6e0` and `FUN_001eb798`, or "Complete" (0x1902) when every
  event is done (`FUN_001eb5f8`), and hides its star (element `0x5a + k`).
  The list steps three widgets at a time (`+0xfa` 3), so the three light
  together; its nodes link straight up and down.
- Refresh `FUN_002ce618` (on a change of the focused slot): an empty slot
  reads "<empty>" (0x1905) with the blue star (`XACOMMON` sheet 0, image 5,
  frame 0) in the progress column, or "CREATE NEW BALLER" (0x1904) and
  "NEW" (0x1906) while it is highlighted; the help (element 85) is "Create
  New Baller." (0x1909), "Enter Be A Legend Mode with %1s." (0x1907) or, in
  mode 1, "Edit Baller." (0x1908).
- Input `FUN_002cdce8`: confirm opens the slot's dialog (`FUN_002cdf10`):
  an empty slot's has no words and Male, Female, Premade, Cancel (dialog
  0x56, `FUN_0025c970(.., 2)` with `+0x104` set to 3: the rows in font 3); a
  career's has its name over Continue Career, Edit Baller, Edit Moves,
  Cancel (0x52, font 2), mode 1 Edit Baller, Edit Moves, Cancel (0x58).
  The answers (`FUN_002cd9c8`): Male, Female, Premade go to classes 0x4a,
  0x4b, 0x4c; Continue Career to the map (0x38), or to the career's
  tournament bracket (0x58, 0x59, 0x5e) when one is under way; Edit Baller
  0x4f, Edit Moves 0x55. A career whose bit 0 is clear goes to class 0x62
  first. Cancel returns to Difficulty (0x69) in Be A Legend, to My Team
  (6) in mode 1; Help opens the help popup.
- Difficulty now comes before Choose Baller, as `FUN_002bf428` sends Be A
  Legend (and the League Challenge) to class 0x69; `FUN_00284848` goes on by
  the mode.

Built: `careers.rs` (`slots`, `row_texts`, `help`, `dialog_rows`,
`pieces`), `Page::CareerAsk` over `pause::dialog` (whose rows now take the
dialog's row font, `Dialog::row_font`), the route through Difficulty
(`after_difficulty`, `career_choice` in `menu.rs`). The progress total is
500, not 450: `FUN_001e74c0` adds `DAT_00460a78`'s six reputation entries
(0, 10, 10, 10, 10, 10) to the events' targets (`story::overall_total`).
Compared: `bal_choose_baller.png` (mean difference 10.5 at the frame
offset), `bal_new_dialog.png` (4.1), `bal_choose_baller_career_dialog.png`
(11.4: the dialog's words and rows sit about 3 rows low). The rewrite's
own: Male, Female and Premade all open its premade list (`Page::NewCareer`;
the original's classes 0x4a..0x4c, a new baller's wizard and the premade
picker, are not built), Edit Moves opens through Edit Baller, and Continue
Career always opens the map (a tournament under way is reached from its
city).

## Be A Legend's event intros (classes 0x31..0x33, layouts 49..51)

- Constructors `FUN_002be4a0` (0x31, Pick Up Game), `FUN_002d5800` (0x32,
  Tournament), `FUN_002d14f8` (0x33, Street Challenge): `FUN_002d7720(..,
  class, 0xe)`, backdrop 14.
- Builds `FUN_002be518`, `FUN_002d5878`, `FUN_002d1570`: the current court
  (game data `+0xcc`) and the career's event (`+0xf28` bits 2..7). Element 2
  (centred at (186, 190)) takes the court's big picture (`XACOURTS` sheet
  `court`, frame 0); element 0 (font 2, centred and middle at (186, 297))
  its name, with two stars (`XACOMMON` sheet 0, image 1, frame 2,
  `FUN_002be910`/`FUN_002d5cc0`/`FUN_002d1a18`) centred 20 beyond the name's
  half width; element 4 (font 1, centred at (186, 312)) its location, empty
  for the region words 2..4; element 1 (a kind-11 text box,
  `FUN_0023aa68`/`FUN_0023b7f8`: font 1 in prop 40's 0xff98a8b8, wrapped to
  its 250 width, lines a line height apart) the court's story from the
  screen's table (`0x48e958`: 0x1302 + court; `0x493e58`: 0x1404 + court;
  `0x492618`: 0x1505 + court); element 3 (font 2, centred at (320, 366)) the
  progress: "Progress: %1d/%2d at %3S" of the event's tally, its target
  (`FUN_001e7550`) and the court's name (0x1301, 0x1401, 0x1504 of the
  target once cleared), a tournament with no target "You have (not)
  completed this tournament." (0x1403, 0x1402), a street challenge with
  none 0x1502/0x1503. The street challenge adds its rule (element 5 at
  (186, 78), `0x492630`: 0x1510 + court) and, on court 10, its title "Be A
  Legend" (0x1501). Back and Continue are the layout's legends.

Built: `event_intro.rs`; `Page::EventIntro` has no rows (Cross plays,
Triangle goes back). Compared: `bal_intro_pickup.png` (11.2; the bottom
legends sit about 3 rows high). Not compared: the tournament and street
challenge intros against the rewrite (grabs `bal_intro_tournament*.png`).

## The map and a city's events (class 0x38, layout 56)

`map_view.rs` was traced (2026-10-12) but never compared with a grab; the
comparison found the map's picture missing and the page's controls the
rewrite's own.

- The United States is a picture widget the build (`FUN_002a03c0`) puts at
  (120, 113): `XAFEBGEX` entry 7, frame 0 ((5, 0) 387x229 of the 512x256
  image), shown on the map and hidden on a city's page (`FUN_002a14f8`).
- The read-out (`FUN_002a0c00`): the baller's first and last names
  (elements 10, 11; the first alone at (65, 355) when the last is empty),
  "Win-Loss" (0x1c04) before the first game or "%1dW-%2dL" (0x1c03),
  "Progress: %1d/%2d" (0x1c02) of `FUN_001e73d8` and `FUN_001e74c0`, out of
  500. Its two stars (elements 0x2c, 0x2d, `XACOMMON` sheet 0 image 1
  frame 1) are disabled by `FUN_002a14f8`, so they draw at half brightness.
- The reputation meter (element 20, kind 39: built by `FUN_00234f10`,
  placed by `FUN_00234a50`, filled by `FUN_00234d50` and `FUN_002349c8`):
  the level's word (`reputation.txt` 0..5, from "REPUTATION" at level 0;
  font 1 in 0xffffba00) at its point, five stars 18 apart from 110 across
  (frame 7 lit, frame 1 unlit), the bar (`XAFESCRN` sheet 0, image 7, frame
  0x22, 200 wide) 22 below and over it a 3-high 0xffffba00 fill at (5, 27),
  19 pixels a point within the level (`FUN_001d8440`: the record's field
  15 less 11 a level; `FUN_001d83c8` the level).
- The prompts: element 0x21 reads "Select City" (0x1c0b) on the map and
  "Play Game" (0x1c0c) on a city's page, element 0x22 "Edit Baller", each
  after its button (the kind-29 icons by their prop 9: Cross 0, Circle 3).
  The help (element 46, bottom-anchored at (58, 446)) is "Select City."
  (0x1c0a) on the map. The title is "The Map" (0x1c00) or "Choose Event"
  (0x1c01).
- The cursor: list 0's six rows are the cities in registration order (100,
  102, 101, 103, 104, 105: New York, Chicago, Philly, Seattle, Oakland, Los
  Angeles), each on the node of its prop 34; left and right follow the
  nodes' links (mode 2), skipping a city that shows only its star, so from
  New York left walks Philly, Chicago, (Seattle,) Oakland, Los Angeles.
  Cross opens the city, Circle opens Edit Baller (`FUN_0029f4b0`). The
  rewrite's "Your Team" row and its Gym are gone: the crew is picked when
  the career starts (the original's class 0x4d) and changes through Update
  Team.
- A city's page lists every court of the city (`FUN_002a1bb0` lights the
  markers whose event is shown and open; the others stay, dimmed): the
  names are row widgets (lit 0xfbac00, idle 0x88a0be, a locked court's at
  half brightness), the picked event's second line its kind
  (`mapscreen.txt` 19..23) or "Locked" (25) in 0x88a0be, the icons at full
  brightness picked, half open, a quarter locked (a picked locked event's
  frame lit, its icon at half, no gold focus). Up and down move a court at
  a time keeping the slot (`FUN_002a2238`), left and right change the slot.
  The help reads "Play a %1s at %2s." with the help's own kind words
  (0x1c07..0x1c09, "Pick Up Game"), or what unlocks the event.

Built: `map_view.rs` (`map`, `readout`, `prompts`, `ROW_CITY`,
`row_available`, `city_step`), `bracket_view::ScreenArt` (`usa`,
`rep_bar`), the map and city controls in `menu.rs`. Compared:
`bal_map.png` (10.3), `bal_city_events.png` (9.4), `bal_city_events_2.png`
(9.7). Tests: `the_map_rows_pair_with_the_nodes_by_registration_order`
(disc: the rows' nodes and the left walk),
`the_city_cursor_moves_by_court_and_keeps_its_slot`.

## Choose Team and Choose Players for both sides (classes 0x1b/0x1c, 0x1d)

- Choose Team (`FUN_00270c78` for 0x1b): a person's confirm readies their
  side (`FUN_00270b28`) and goes live on the computer's side for the same
  person; both ready, `FUN_0026fd00` goes to Choose Players. 0x1c gives each
  side's list to its own controllers at once.
- Choose Players (`FUN_00269b28`: `FUN_002d7720(.., 0x1d, 5)`; build
  `FUN_00269c08`): two picker widgets (`FUN_0022ff80`, the second argument
  the element ids 10 and 0x20) at (118, 218) and (450, 218), one a side,
  each side's read-out filled by `FUN_0026a198` (the focused player's
  name, height and weight and meters); the grabs show each picker as a
  portrait grid (`XAPORTS`) with the picked marked. With one person the
  same person picks the computer's players on the same screen.

Built: the flow is the original's order: Choose Team for side 0, then side
1 on the same screen (side 0 under "Ready"), then Choose Players for side
0, then side 1, then Choose Court (`after_team`, `after_players`). Missing:
the two sides live at once for two people (the rewrite keeps a page per
side and one cursor), and Choose Players' drawing: the rewrite's
`choose.rs` read the picker widgets' points as two standing 3D players;
the original's portrait grids (`FUN_0022ff80`) are not traced or drawn.

## Choose Players' portrait grids (class 0x1d, 2026-10-19)

The second front-end pass (branch `frontend-screens`). Choose Players is
one screen for both sides, each side a picker widget; the rewrite now
builds it from the constructors (`choose.rs`), with both sides live at
once.

- *The picker* (`FUN_0022ff80(widget, screen, id, point, side)`, kind
  0x15): six cell nodes (`FUN_00221ac8`) two across and three down from
  the first tile's middle ((118, 218) on the left, (450, 218) on the
  right), linked across in reading order (the first tile's left and the
  sixth's right stop) and up and down a row, the top row's up and the
  bottom row's down scrolling the list (`FUN_0021e7e8`, `+0xfa` 2) a row;
  the cursor (`FUN_0021b550` case 0: `XAFESCRN` image 0, frame 1,
  stretched to the tile and 2); the list's scroll arrows 48 left of and 26
  above the first tile (the right picker's 2 tiles less 27 right of it),
  the down arrow three rows less 20 below; "Ready" (element 8, font 6,
  0xffdcdcbb, centred 35 right of and 74 below the first tile) and the
  Continue legend under it (element 9, `FUN_0021a530`, Cross and 0x14,
  top-anchored at Ready's point plus its height and 4).
- *The tile* (`FUN_00223220` over `FUN_00222a50`, set up by
  `FUN_00223698` with `+0xd0` set): the back (`XAFESCRN` sheet 0, image
  4, frame 1, 66x65) in `XPAL_FE` sheet 2's palette by the portrait (0 for
  the league's, 2, 9 and 10 for the legends' and streetballers' sheets, 8 for
  none, 11 for a created baller), the portrait (`XAPORTS` sheet field 14,
  a created baller's icon 0xb0 + its field 14, `FUN_00223d88`) cut to its
  sheet's frame 0 and stretched to the back's size, the frame over them
  (image 5, 74x73: the tile's size), and the locked secret legend (record
  0xa7) as the question mark (image 9) over the whole tile. The check
  (child 0x453: image 0xe, frame 4) is right-anchored 27 right of and 1.5
  above the tile's middle; the label (0x452: font 1, 0xfbac00, a 2-pixel
  black outline) 13 below it: "1".."4" by controller for two people on a
  side, "CPU" for their third (`FUN_002315b8`), "Locked" in 0xff838497.
- *Brightness.* The picker's list has `+0x124` set, so a tile it binds
  sits at 0x9f (`FUN_00222420`, `FUN_00222368`) and the focused one at
  0xff (`FUN_002222c8`); a picked or locked tile 0x5f (`FUN_002310a0`),
  the computer's last pick lit; the tiles not picked once a challenge's
  side is ready 0x7f (`FUN_00230938` disables them); Be A Legend's empty
  places and a ready side's checks 0x3f (`FUN_00230dc8`). Measured on
  `bal_choose_players*.png`: an unfocused tile keeps 0.60 of the focused
  one's light, a picked one 0.34.
- *What the tiles are* (`FUN_00231b40`): a side nobody holds lists only
  its line-up's players (`FUN_002321b0`), legends' teams their three; league
  Stars (39) its 25 (`0x46ba28`) to a person; Old School (38) the legends
  0x9e..0xb7, the unlocked first and the locked after them with the
  secret one last (`FUN_002320d8`); other teams their roster, up to six;
  Be A Legend's crew pads to five with empty places (widgets 0xb, 0xc).
- *Picking* (`FUN_00230e98`, `FUN_002315b8`, `FUN_002312e8`,
  `FUN_002318b8`): Cross puts the focused tile in the line-up (a picked,
  locked or empty one only sounds 6) and moves on to the next tile not
  picked (`FUN_00220dd0`); the third pick readies a Pick Up side at once
  (`+200`) and a challenge's side to "Ready" with Continue (`+0xcc`), a
  further Cross finishing it; Triangle takes the last pick back and puts
  the focus on it, Be A Legend's baller (picked from the start,
  `FUN_00230938`) staying. Two people on a side take turns (`+0xb8` over
  `+0xba`, `+0xbb`, `FUN_002322a8`). A side nobody holds is picked at the
  build (`FUN_002323b8`), its cursor on its last pick.
- *The screen* (`FUN_00269c08`, input `FUN_0026a860`): the widgets answer
  first, then the screen: a Cross with both sides finished goes on (Choose
  Court in a Pick Up Game, the loading screen in the challenges), so the
  last pick goes on in the same press; Triangle with nothing to take back
  goes back. The read-outs (`FUN_0026a198`, elements at `0x476940` and
  `0x47696c`): "" (or "?" for the secret one) over the first and last
  names in 0xfbac00, "%s %s" of height and weight in the layout's
  0x98a8b9, the seven meters. `FUN_0026a068` colours each side's four
  lines, corner and tab by the controller whose turn it is (`XPAL_FE`
  sheet 0, or the sheet's grey for the computer) and its word black on
  2P's yellow tab; the tabs read AWAY (0xc03) and HOME (0xc04).
- *Be A Legend* (mode 5) puts the career's crew on the left (the
  original's side 0, `FUN_001db308`), which the rewrite plays as home; its
  games now go Intro (or Bracket) → Choose Players → loading. The league
  Challenge's games (mode 4) go through it too, the challenge's three on
  the left (`FUN_00231b40` lists teams 0x24 and 0x28..0x2e by their
  three).

Built: `choose.rs` (`Picker`, `Screen`, `tiles`, `pieces`, the shared
`tile_pieces`, `readout`, `frame`, `cursor_piece`), `portraits.rs` (the
`XAPORTS` sheets read once and cut to their frames; the result boards use
it too), `menu.rs` (`Page::ChoosePlayers` for both sides, `begin_players`,
`players_navigate`, `begin_story_players`, the per-controller
`MenuInput::confirm_of` / `back_of`). Compared (each rewrite shot moved by
the grab's offset): `chooseplayers.png` against the same home team (the
Bulls), the person's panel 12.7 (the whole frame 15.9: the computer's
three differ); `bal_choose_players.png` 15.4 (different crew and
opponents). Left: the computer's three are its roster's first three (the
original draws them in `FUN_001dbe30`, not ported); the pick's variants
(keys 0x31, 0x33, 0x34: `FUN_002315b8(.., 3, 1, 2)`, stored at `+0xb4` of
the line-up) are not read; the full crown's sparkle (`FUN_00218628`
turning `0x467198`'s frames) is not drawn; two people on one side were
checked by tests, not by a shot (the keyboard is one device).

## Meters, translucent palettes and portraits (2026-10-19)

- *The kind-20 meter* (`FUN_002182f0`): the crown (`XAFESCRN` sheet 1,
  image 0, frame 0 or 1, 54x17) at the element's corner and the bar
  (frame 10 or 11, 51x8) 2 right and 17 down (`FUN_002185c0`, the meter's
  move), filling the 60x25 cell from its corner. The shared
  `chooseteam::meter` had them centred, 3 pixels right and 1 low: Choose
  Team's crowns now line up with `chooseteam.png`.
- *Translucent palettes.* `XPAL_FE` sheet 0's controller palettes hold
  alphas of 0x25 and 0x49 (of the PS2's 0x80), so the panels' lines,
  corners and tabs are translucent; composited in linear light they came
  out half again as bright as the PS2's (145 against 100 red on a line).
  `look::gs_blend` gives each partly transparent pixel the alpha that
  darkens what is under it as the GS does and scales its colour so it
  leaves the GS's light over black (107, 19, 17 against the grab's 100,
  13, 15). It is applied to the frame pieces in every palette (Choose
  Team, User Setup, Choose Players, Update Team).
- *Portraits.* An `XAPORTS` sheet is a 128x128 image whose frame 0 is
  the face (75x84 from (26, 21), or 74x83 from the corner);
  `FUN_00210048(w, 4, sheet, 0, 0)` draws that frame at the widget's
  size. The rewrite drew the whole 128x128.

## Update Team (class 0x60, layout 96)

- Constructor `FUN_002d61a0` (backdrop 5), build `FUN_002d6248`, input
  `FUN_002d6c00`, dialogs' answers `FUN_002d6fd0`, read-outs
  `FUN_002d66c0`, refresh `FUN_002d7238`.
- Two roster widgets (`FUN_002325e8`): five tiles each, two across, every
  tile at 0x9f (`FUN_0024a728`) and the focused one full: the career's on
  the left (0x55, first tile at (113, 220); the baller's tile at 0x3f,
  `FUN_00232e18`) and the beaten team's on the right (0x56, at (457,
  220); `FUN_00232e50` fills a side's team, the three for teams 0x24,
  0x26, 0x27 and 0x28..0x2e).
- The question first (0x3d04, dialog 0x57 on Yes, `FUN_002d7238`); then
  the beaten team's list takes the focus with "Select a player to add to
  your team." (0x3d05); Cross there moves to the career's list on its
  first tile with "Select a roster position for your new player."
  (0x3d06); Cross on the baller's tile only sounds; on another it asks
  "Accept roster changes and continue?" (0x3d02, dialog 0x59 on No);
  Triangle on the career's list goes back to the beaten team's, on the
  beaten team's asks "Exit without making changes?" (0x3d01, 0x58 on
  No). The career's read-out is blank while the beaten team's list is
  picked from. "Cancel Update" (0x3d08, element 3) after Triangle's icon
  (element 2). The panels keep the layout's palettes (the lines' prop 8: 0
  and 2, 1P's red and 3P's blue) and the tabs no words.

Built: `update_view.rs` over Choose Players' tile and read-out; the
rewrite's `UpdateStage`s keep its flow, now drawn on the layout with the
dialogs over it, the cursors as the original's. Compared: the Update Team
grabs sit 4 rows higher than the others (best offset (1, -1); taken right
after a match); there the question scores 9.9, the confirmation 6.8, the
beaten team's list 18.0 and the roster's 14.9 (different players). The
page is reached after a won story game; its shots were opened through a
local hook (not committed).

## Create Team and the crew (classes 0x35, 0x62, layout 53)

- `FUN_0027dd80(screen, mode)` (backdrop 9): class 0x35 is My Team's
  Create Team (mode 0, five picks) and class 0x62 a new career's crew
  (mode 1, two picks); Choose Baller opens 0x62 for a career whose bit 0
  is clear. The layout's class 0x4d is Choose Team's "join a team"
  screen, not the crew.
- Build `FUN_0027e458`: the category row on list 8 (rows 1000 "league
  Players", 0x3ec "Street Legends", 0x3eb "Legends", 0x3e9 "Created
  Ballers" in 0xfbac00 between the L1 and R1 icons and arrows; the crew's
  one row 0x3ea "Local Ballers" in 0x88a0be), the grid's ten small tiles
  (elements 10..19, `+0xd0` clear: image 4's frame 0 and image 6, 55x53)
  and the five slots (20..24, big). The crew's first slot holds the
  career's baller and the slots' order turns so the picks fill 21 and
  22; 23, 24 and the baller's are shut (0x7f).
- Lists (`FUN_0027f430`): each league team's first five (`FUN_001ecd00`)
  with its name over its row (elements 32, 33), the streetballers
  0x91..0x9d and the legends 0x9e..0xb7 unlocked (`FUN_001d9ca0`), the
  created ballers; the crew's ten locals (`FUN_001ed1f8`, kept once a
  boot at `0x3ec9c8`): the generic ballers 0xb8..0xe9 not taken, men and
  women each shuffled with the C library's `rand() % (k + 1)`, ten drawn
  from the ends (one in twelve of the first eight a woman, `rand() % 12`,
  the last two women while any are left), the ten shuffled.
- Paging (`FUN_00280040`): the grid shows two rows of five from row
  `+0x150`, scrolling a row past either end; a step onto the last row's
  gap comes to its last player; a player picked dims to 0x7f; L1 and R1
  (0x31, 0x33) turn through the categories with players.
- The read-out (`FUN_0027fc48`): "%1s\n%2s" of the names on element 9,
  "%s\n%s" of height and weight right-aligned on 30, the meters (kind 20,
  anchored on their middles at x 520) beside the layout's words.

Built: `create_team.rs` (`categories`, `locals`, `Picker`, `pieces`), the
crew on `Page::StoryCrew` and My Team's on a new `Page::CreateTeam`.
Compared: `bal_crew.png` and `bal_crew_pick1.png` 13.7 (the locals are a
random draw: other faces); `my_team_create_team.png` 14.5 and
`my_team_create_team_pick1.png` 14.6 at that grab set's offset (1, -1).
Left: the Welcome board before the crew, the team's city and name
keyboards and Choose Logo after the picks (`FUN_00280cd0`; the crew is
made at the second pick and My Team's fifth pick opens the rewrite's
editor for the city, name and logo), "Cancel and Return to Choose
Baller?", and the career's team name and logo, which the rewrite's save
does not keep.

## Choose Region (class 0x29, layout 41)

- Constructor `FUN_0026b408` (backdrop 6; it loads `fe/REPLAY.ssh`, the
  "Vol. 2" logo), build `FUN_0026b760`, refreshes `FUN_0026c430` (the
  region list), `FUN_0026c5c8` (the team list), `FUN_0026c6f8` (the
  panels). The regions' table at `0x3ec9e8` (teams, their list, the boss,
  the court) is the rewrite's `REGION_MATCHES` and `REGION_REWARDS`.
- One screen in two states. A region picked: its court's big picture
  (`XACOURTS`, frame 0) on node 14 between the list's arrows (at (76,
  327) and 214 right), its name ("North West" .. "South East",
  `choosergn.txt` 7..11, font 3) over its court's, the "Vol. 2" logo, the
  meters at nought and the region's team tiles (each logo sheet's image
  1, frame 0, left-anchored on nodes 15..20) disabled, a check
  (0x22..0x27) on each team beaten, the boss's card (`XAFESCRN` image 1,
  frame 5, the question mark on the star) on node 21, "Select a region
  in which to play." (0xf0e). A team picked: the region list disabled, the
  team's big logo (element 5) and name (27), its meters, the tiles live
  with the cursor, which steps across to the boss's card, and "Play
  against the %1S %2S." (0xf01).

Built: `region_view.rs` for the rewrite's `Page::League` (left and right turn
the regions, round) and `Page::LeagueRegion` (the tiles). Compared at that
grab set's offset: `league_choose_region_central.png` 12.5,
`league_region_matches.png` 13.0. The disabled pieces keep 0.31 of their
light as measured (a tile, the court picture and the boss's card 0.30 to
0.33; the list's 0x9f under the disabled 0x7f), the "Vol. 2" logo (a
widget of its own, `FUN_0036a0d0`) 0.15. Left: the League Challenge's
Choose Team (class 0x19, the person's league team) in place of the
rewrite's own three-player pick, the boss's card's other frames.

## Create A Baller (class 0x3d)

`FUN_002cd930(screen, 1)` is Choose Baller in mode 1 over the same layout
and backdrop: the ten slots, "CREATE NEW BALLER" and "<empty>", a baller's
name and progress, "Edit Baller." for help. The rewrite's six created
ballers fill the first six (`careers::baller_slots`); an empty slot opens
the rewrite's wizard and a used one its editor (the original's dialogs,
Male, Female, Premade and Edit Baller, Edit Moves, are not shown).
Compared: `my_team_ballers.png` 11.5 at that grab set's offset.

Since the fourth pass (2026-10-19) the slot's dialog is shown
(`FUN_002cdf10` in mode 1, `careers::dialog_rows(.., true)`, row font 3
on an empty slot, 2 on a baller): Male and Female open the wizard on
the slot (classes 0x4a, 0x4b), Premade the picker (0x4c) for it, whose
YES saves the premade in the slot as its record has it and EDIT opens
the wizard on it; Edit Baller and Edit Moves open the editor and its
moves. Compared: `my_team_ballers_new.png` 5.6, `my_team_baller_dialog.png`
5.9 (the baller ANTHONY there, PLAYER in the grab), both at (2, -1).

## The Help legend's white, looked at again

Ruled out: the legend's loader (`FUN_0021a600`) keeps 0xff98a8b9 for prop
10 = 0xffffffff (the prop getter `FUN_002e3b18` returns the word through
`lwl`/`lwr`, sign-extended, and the compare at `0x21a6cc` is against a
sign-extended -1); the legend class has no draw of its own (its vtable
`0x467d38` differs from the base only in its loader and destructor); the
screens' widget passes (`FUN_002d83a8`, slot `+0x13c`) set a transition
slot, not a colour; the text draw (`FUN_00238d20`) scales by `+0x5a` and
halves for the GS. The white screens' and the grey screens' layouts
carry the same element (id 0xffff on Choose Team, grey, as on Choose
Players, white). Next: read the legend's text widget's `+0x4a4` in RAM on
a white screen and a grey one (PINE), then find its writer.

Read 2026-10-19 (PINE on the runtime's save states 5, 6 and 7): the
legend (vtable `0x467d38` at `+0x20`) keeps its texts at `+0xc0..`; the
Help text's `+0x4a4` is 0xff98a8b9 on Main Menu (state 5, grey) and
0xffffffff on Create A Baller's list (state 6) and Soul in the Hole's
intro (state 7), both white. The legend's own `+0xd8`, the colour
`FUN_0021a7c0` gives each text it adds, is 0xff98a8b9 on all three, and
the loader cannot write 0xffffffff (the getter's `lwl`/`lwr` pair
sign-extends; `0x21a67c` builds the -1 it compares with). So something
after the legend's load writes the text's `+0x4a4` directly. The grabs
show it changes within one screen, so it is state, not layout: the
wizard's Build, Look, Hair and Skills panels grey, its Facial, Outfit,
name and Icon panels white; Update Team's question grey, its pick white;
Street School's lessons another colour, (148, 131, 103). Not the writer:
the legend's own setters (`FUN_0021aa70`, `FUN_0021a968`, a few screens'
callers), the colour setters `FUN_0021ce50` / `FUN_0021ce90` (another
class's child 0). Next: a write breakpoint on a white screen's text
`+0x4a4` in PCSX2's debugger.

## Who plays for a side nobody holds (`FUN_001dbe30`, 2026-10-19)

The third front-end pass (branch `frontend-third`). `FUN_001dbb20(setup,
side, team, seed, n)` sets a side's team; with no controller on the side
(`FUN_001db280`) it calls `FUN_001dbe30(setup, side, seed, team, out)`,
which seeds the C library's generator and the shared one (`FUN_003b4290`,
`FUN_003a1790`) with `seed`, or with the 100 Hz timer's count
(`FUN_003998c0`) when the seed is below 1, and fills the side's slots
(`setup + 0x9c + side * 0xc`) by the mode (`setup + 0x78`). Ported in
`lineup.rs` (`choose`, `Card`, `Book`, `Career`), drawn through
`Session::choose`.

- *Pick Up* (modes 0, 2, 3): a team's first three (`FUN_001ebf80`, fields
  4..6). All-Stars (0x27) lists every league team's first five with field 28
  set, Old School (0x26) the legends 0x9e..0xb7 unlocked
  (`FUN_001d9ca0`); a player the other side holds (`FUN_001db3e0`) goes
  to a spare list. The League Challenge (mode 4): the first three.
- *Be A Legend* (mode 5), by the event (`setup + 0xf28`, bits 2..7):
  - teams 0x28, 0x2a..0x2c (Stretch's, Biggie Little's, Dime's,
    Whitewater's crews): the rival in slot 0 and generic ballers
    0xb8..0xe9 of level 1 (2 for 0x2b, 0x2c); 0x29 and 0x2d (Bonafide's,
    Osmosis's): their fixed three;
  - a generic crew (0x24) at a pick-up or a tournament: five lists by
    level of the league's first fives, the generic ballers, the streetballers
    0x91..0x9d and the created ballers (the list at `+0x33f4` without the
    setup's own, `+0xd0`), each shuffled and cut to
    `trunc(mix[level] * len)` of the court's tier mix (`FUN_001d3880`,
    fields 5..9; `cvt.w.s` cuts toward zero); a tournament's boss team
    (`FUN_001eadd8`, until the event is cleared) first draws its own three
    into a list the crew leaves out (`FUN_001dbe30` called again with
    `out`), then the seed is applied again;
  - a street challenge (kind 2), by its court: Broad Street level-2
    slashers and playmakers (one style 1, two style 2), Foster Beach level-3
    league slashers, Soul in the Hole level-2 league players, Mosswood level 4 of
    every list, Greenlake level-3 non-playmakers, Lincoln College a league
    team's first three (team 11, the Lakers, first, then the others from
    the strongest: a stable sort by the sum of their three's levels,
    `FUN_001dac18`, `FUN_001e11c0`), Rucker Park '78 team 0x2e's three
    not on the career's team with the streetballers and the level-4 league
    stars behind them, the stars taken first and nothing shuffled again.
- *Who may play against a career* (`FUN_001e79a0`): not on the career's
  team (`career + 0xca0`) and unlocked; a created player not one of the
  career's (by field 0); a boss team's player (the rival triples at
  `0x4609e0`: teams 40..46, events 2, 3, 5, 7, 8, 9, 10) once its event is
  cleared (`FUN_001e7ff8`); no streetballer 0x97, 0x98, 0x9a..0x9d; no
  generic baller in slot 1 or 2 of teams 0x29 and 0x2d.
- *A level* (`FUN_001d7e40`): the mean of the seven skill bars
  (`FUN_001d7f48`) against 1.75, 2.49, 3.0, 3.45 and 6.0 (`0x45fd78`), the
  first not passed. *A style* (`FUN_001d8088`): field 27 when it is 0..4,
  0 past it, and for -1 the best bar of dunks (1), handles (2), shots (0),
  blocks (3) and rebounds (4), the first of equals winning.
- *The draw* (`LAB_001dde3c`): short of three, the spare list is shuffled
  and its end makes up the rest; the list is shuffled (not for Rucker Park
  '78); each empty slot takes the list's last. For teams 0x24, 0x26 and
  0x27 a woman (field 12) is kept one draw in three (`rand() % 3`) and no
  second player of a style (`FUN_001db438`); the ones passed over go to
  the spare list, which becomes the list (turned) once it runs dry. A slot
  nothing fills gets record 0. Every shuffle is `std::random_shuffle`
  (`j = rand() % (i + 1)` for `i` from 1).
- *A tournament's rounds* (`FUN_001e91a0`): every game of a tournament
  seeds with your entry's number (`FUN_001e8790(.., 0, 0, 0)`; the bracket
  counts the crews up from it, `FUN_001e9888`, which is the rewrite's
  `Bracket::new`), and a generic crew throws three draws away a round you
  are in (`FUN_001e82f8`; in double elimination the opponent's entry past
  yours, `FUN_001e86e0`), so each round meets the next crew of the same
  stream. With a seed given, both generators end seeded from the timer.

In the rewrite, Choose Team's commit for a side nobody holds draws its
three (`menu.rs` `computer_three`), and `story::prepare` draws a story
game's opponents (team 0x24 for pick-ups, street challenges and the
tournaments' crews; a boss team's own), both after the session's reseed.
The street challenges now play team 0x24 as the original's do (the
rewrite had the Rucker Park Legends' and a league team's).

Checked against the original in the project's PCSX2 (PINE reads of
`0x4bbf10 + 0x9c..`, the C library's word at `*0x434284 + 0x58` and the
timer at `0x4b32e4`): save state 4's career starting NYC Rec Center's
pick-up drew 214, 213 and 229 and left the word at 0xd4ad32fb; of the
timer's counts between the reads before and after the press only 120569
gives both. Save state 7's Soul in the Hole (bracket seed 666, the boss
Stretch's crew 40 at 673) drew 133, 94 and 190, as the port does from 666.
Both are `lineup.rs`'s disc test. All-Stars, Old School and the street
challenges are checked by the tests only. Left: the bracket screen still
names a generic crew by the rewrite's own draw (`story::entrant_name`).

## The League Challenge's Choose Team (class 0x19, 2026-10-19)

- Constructor `FUN_0026f4f0`: `FUN_0026e1a8(screen, 0x1a, 0)`, the same
  layout 26 as classes 0x1b/0x1c over backdrop 0, whose picture of league
  players fills the right panel's place (layout class 25 is another
  screen, a 3x3 grid with five meters, which nothing opens). Build
  `FUN_0026f568`: the corner and tab pieces (`FUN_0026ee20`), list 0's
  nodes for the created teams and Team Lunatics once unlocked
  (`FUN_0026e3e0`), the league's 29 (`FUN_0026e558`), Random League Team (0x30)
  and blanks to the row's end (`FUN_0026e7f0`), no legend teams' row; the
  list's arrows (`FUN_0026eb08`); the list and the screen to the person
  whose challenge it is (`setup + 0xd4`), the left panel in that
  controller's palette and its word black for 2P (`FUN_0026eff8`), the
  right panel's palette the computer's; the cursor tag 0x154.
  `FUN_0026f7c0` hides every element from element 0x1b's corner (324, 93)
  on except 0x1b..0x20, then 0x42 (HOME), 0x1b..0x20 and "Ready" (0x14).
- Input `FUN_0026f958`: Cross (0x23) stores the focused team
  (`FUN_0026ef88`, Random `FUN_003a16b8() % 29`) for side 0
  (`FUN_001dbb20(setup, 0, team)`, which in mode 4 writes the user's
  `+0x4a`, `FUN_001ee3d8`) and opens Choose Region (class 0x29); Triangle
  (0x25) goes back to User Setup (`FUN_0026ec20`); Square (0x27) opens the
  help. A focus change redraws the logo, name and meters
  (`FUN_0026fa98`, `FUN_0026f238`).
- The challenge's games: Choose Players (mode 4) lists the team's roster
  on the left for the person to pick three (`FUN_00231b40`), the league
  opponent's three on the right. A side nobody holds moves its highlight
  through its picks a second at a time (`FUN_002324d8`, the timer
  `FUN_002323b8` sets with `FUN_002da198(.., 1, 0)`; the next pick by a
  counter every picker shares, `0x3f68ac`), so `league_choose_players.png`
  shows Jalen Rose lit though Jay Williams is the last pick.

Built: `Page::LeagueTeam` in place of the rewrite's own three-player list
(`chooseteam::league_grid`, `league_pieces` over the shared `panel`); the
team is `Story::league_team` (the user's `+0x4a` in the original's save);
`story::prepare_league` plays its first three until Choose Players, which
lists its roster (`begin_story_players`); `Picker::cycled` moves a
computer side's highlight. Compared at that grab set's offset (1, -1):
`league_choose_team.png` 13.7, `_right` 13.8, `_down` 13.6;
`league_choose_players.png` 13.5 (the same team and opponent). 640x480 and
1280x720: no overlap. Left: the shared counter starts at 0 when the
screen opens (the original's carries on from the last picker), Random's
draw is the rewrite's own, and the help screen (Square) is not built.

## The brackets on their layouts' screens (2026-10-19)

The bracket page drew on `Style::List` under its pieces: the title stood
at the list's place (6 right and 4 below the layout's (53, 37)) and the
list's key prompts showed under the frame. It now takes its screen's
style (classes 88, 89, 94, whose layouts hold the title and the Help
legend). Two more from the grabs: a boss crew is named by its team's
nickname ("Stretch", "Osmosis"; the fill `FUN_002cc050` is cut in the
decompile at `FUN_0030fa50`), and middle-anchored words sit by the
font's text height (as `choose::place` places them), which lowers
"Single Elimination" 4 pixels onto the grab's. Your crew goes by the
career's team name (`Story::crew_team`, below), "BALLERS" in the grabs.
Compared at that grab set's offset (2, -1) (the brackets were grabbed
after games, like Update Team): `bal_bracket_single.png` 12.1,
`bal_bracket_double.png` 12.9, `bal_bracket_best_of_three.png` 11.6
(15.4, 15.6 and 14.8 before; the grabs' tournaments and crews are other
ones).

## The crew's boards and the career's team (`FUN_002810d0`, `FUN_00280cd0`)

Create Team's dialogs run as a state machine (`+0x1fc`, `FUN_002810d0`
when a dialog closes): the crew's Welcome board first (state 5,
`FUN_0024c2a8`); after the last pick the city's keyboard (state 0,
`FUN_0024d8a8(dialog, 0x1d10, screen, 0x3ed)`, "Where's Your Team
From?", typed text "NEW YORK" to start, `createteam.txt` 19 for the crew,
20 for My Team), the name's (state 1, 0x1d11, 0x3ee, "BALLERS", 21) and
Choose Logo (state 2, `FUN_0024cac0(dialog, screen, 0x3ef)`); then
"Cancel and Return to Choose Baller?" (state 3, 0x1d16; My Team's
0x1d17) and My Team's "Do you want to save your team?" (state 4, 0x1d12).
A dialog's answer (`FUN_00280cd0`) stores the text (`+0x1b0` city,
`+0x1a4` name) or the logo (`+0x1bc`) and moves to the next state; a
cancel (`FUN_00280f98`) to the one before (the city's back to the grid,
two places back in the slots). The help line is emptied while they are
open (`FUN_0027fb88(.., "")`). With all three done `FUN_002808c0` makes
the crew's team record (`FUN_001ed960`: city `FUN_001edc50`, name
`FUN_001edc88`, logo `FUN_001edd20`, the two picks in slots 1 and 2),
copies it into the career (`FUN_001e6e48(career, team + 0x18)`, the
career's own record at `+0x664`), sets the career's bit 0 and opens the
map.

- *The Welcome board* (`FUN_0024c2a8`): the screen dimmed (black at 0x40
  of 0x80), the dialog's frame (kind 28, `FUN_00211278`) 40 round the
  words (`createteam.txt` 24, font 2, a block 420 wide, white) and the
  Continue row (`common.txt` 20, font 2, lit with its stars) 10 under
  them, the title "Welcome" (25, font 6) by its bottom-left 15 above the
  frame. The block and row centre 10 below the dialogs' place
  (measured; the board is placed on `DAT_003f7170` with its title).
- *The keyboards* (`FUN_0024d8a8`): the heading (font 2, 0xffdcdcbb), a
  rule 10 under it (0xff424a9c, 4 tall, 16 narrower than the frame), the
  keyboard (`FUN_0023c7b8`, User Setup's, `users::keyboard`) 15 under
  that, stacked about the dialogs' place; the frame 18 round the heading,
  8 round the rule, 40 round the keyboard (116x128, measured). The
  keyboard opens on DONE.
- *Choose Logo* (`FUN_0024cac0`): the focused logo's large picture
  (`XATEAMS` image 0), the rule, two rows of five tiles (`XATEAMS` 0x1d..
  0x26, image 1) 10 apart, linked left and right in a row and up and down
  between the rows, the edges stopping; the frame 40 round each, moved 10
  down and 20 shorter; the title "Choose Logo" (`chooselogo.txt` 0) by
  its bottom 10 under the frame's top (as the grab has it); the dialog
  placed half the title's height below the dialogs' place
  (`DAT_003f699c + title_h / 2`). The list's `+0x124` is clear: every
  tile at full light.

Built: `create_team::Stage`, `welcome_pieces`, `keyboard_pieces`,
`logo_pieces`, `logo_step`; `menu.rs` `crew_navigate` runs the states on
the crew's page (typing through `Field::CrewCity`, `Field::CrewName` on
User Setup's keyboard). The career's team is `street_sim::story::
CrewTeam` (city, name, logo) on `Career::team` and `Story::crew_team`,
saved in a section of its own (`CTEM`, after every older one, so an older
save reads with empty teams) and imported from an original save's career
record (`+0x664`: name parts 1 and 2, field 10). Compared (pre-match
offset (1, 3)): `bal_crew_welcome.png` 15.7 (14.4 at (-3, 3)),
`bal_crew_city.png` 10.9, `bal_crew_name.png` 10.8, `bal_crew_logo.png`
15.5 (the locals behind are a random draw: other faces). Left: Triangle
on a keyboard accepts the text as typed (the rewrite's typing ends on it)
rather than cancelling back; "Cancel and Return to Choose Baller?" is not
asked; My Team's Create Team still uses the rewrite's editor for its
city, name and logo.

## The completion boards (classes 0x3b, 0x3c)

Not grabbed (no save state reaches them); built from the constructors.

- *The League Challenge's* (`FUN_002a46b8`: `FUN_002d7720(screen, 0x3b,
  0x14)`, layout 59, backdrop 20): "Congratulations!" (`nbatcend.txt` 0)
  as the title; "You have completed / League Challenge Mode." (1, font 3)
  and "%1d Reward Points" (2, element 1, of `setup + 0x1c0c`) between two
  stars (elements 2 and 3: `XACOMMON` sheet 0, image 1, frame 0); the
  Continue legend. Cross restarts the challenge (`FUN_001de8d0`) and
  leaves (`FUN_002d8460(.., 0x3f)`; to class 0x4d when the challenge's
  team holds a created player, `+0xf0`, `FUN_002a4898`).
- *Be A Legend's* (`FUN_00261c48`: `FUN_002d7720(screen, 0x3c, 0x10)`,
  layout 60, backdrop 16; fill `FUN_00261cc0`): "Be A Legend"
  (`bealegendend.txt` 10) as the title; "Congratulations!" (0, font 5)
  and "You have completed / Be A Legend Mode." (1, font 2); the baller's
  name on element 0 (the first name in its font 3 when the last is empty,
  else "first last" in font 1) with a star 14 beyond each end
  (`FUN_00262100`); "Win / Loss Record:" (4) over "%1d - %2d" (7, element
  3, the career's `+0x28`, `+0x2c`); "Reputation Level:" (5) over its name
  (`reputation.txt` 1..5 by `FUN_001d83c8`, element 1); "Street
  Challenges:" (6) over "%1d / %2d COMPLETE" (9, element 2, those
  cleared of all); Continue. Cross marks the career complete (`+0xf28 |
  2`, `FUN_002621a0`) and leaves.

Built: `completed_view.rs` (`pieces`, `Board`), the pages on their
layouts' screen style with their backdrops. The league board's trophy loop
(`XLP0006`, which an earlier pass put in the Rewards' movie slot, over
this layout's words) is dropped: no call was traced to it and the
backdrop holds the trophy. 640x480 and 1280x720: no overlap. (2026-10-20,
the idle demo's pass: the runtime's board does play it, turning in the
left panel; `presentation.ui_league_board_trophy_loop`, not built. 2026-10-20, the movies pass: built, with Be A Legend's `XLP0016`; "The movies", below.)

## Create Baller's screen and the premade picker (classes 0x4a..0x4c)

`FUN_00273a08(screen, mode, gender)` makes classes 0x4a and 0x4b (mode 3,
the wizard for a man and a woman) and 0x4c (mode 1, the premade picker):
`FUN_002d7720(screen, 0x3d, 3)`, layout 61 (the title, the L1 and R1
marks, kind 37 elements 155..158, and the Select / Cancel legend) over
backdrop 3. `FUN_00273f58` makes twelve panel slots (`+0x100..`), each a
panel over its own layout (`FUN_002748a8(panel, screen, layout)`): slot 1
the premade picker (`FUN_00277be8`, layout 63), slot 3 the wizard's first
step (`FUN_00276cc8`, layout 65), slots 5..11 its others (layouts 67..73:
`FUN_0027b908`, `FUN_0027a7e8`, `FUN_00279aa0`, `FUN_00278748`,
`FUN_0027cfe8`, `FUN_0027bfa0`, `FUN_00279628`); the mode is the slot
first shown (`FUN_00274188`). The 3D baller is the model viewer
`FUN_002db8b8` in the backdrop's picture panel.

- *The panels' pictures* (`FUN_00274a40`, the base build): by element
  id, the rows' bars 101.. (`XACOMMON` sheet 0, image 6, frame 0), 120..
  (frame 2), 100 (frame 1), 119 (frame 3); the tab icons 90.. (`XAFESCRN`
  sheet 0, image 7, frames 8, 6, 5, 4, 2, 1, 7, 9 by the table at
  `0x478e38`); 130.. (frame 0x1b), 134.. (0x16), 142..145 (0x1d), 146..149
  (0x1c, stretched between 142 + k and 150 + k), 150..153 (0x1e); the
  stars 110..112 (`XACOMMON` image 1, frame 8) and 113.. (frame 1). The
  layouts list their pieces front first: drawn back to front, the panes
  (kind 2, prop 10's colour) under the bars under the words.
- *The premade picker* (`FUN_00277cf0`, input `FUN_00277fa8`, update
  `FUN_00278098`): the screen's title "Choose Baller"
  (`createplyr_choose.txt` 0, 0x2200; "Create Baller" again on leaving);
  the list 0x14 (node 21 at (435, 121)) holds the six premade ballers by
  name (`FUN_001e6518` loads each into the created player), L1 and R1
  (actions 0x31, 0x33) step it, each step loading the premade into the 3D
  baller and the read-outs: the name (element 8), height and weight (13,
  17, right-aligned), the seven skill bars on the meters 62..68, "Total:
  %1d DP" (`createplyr.txt` 10). Cross asks "CREATE / %1S?"
  (`createplyr.txt` 14, `FUN_00274260`: YES 12, EDIT 11, CANCEL 13, row
  font 2; CANCEL lit as the grab has it); YES makes the career with the
  premade (`+0x158`), EDIT opens the wizard on it (slot 3), CANCEL stays
  (`FUN_002743d0`).

Built: `cab_view.rs` (`CabArt`, `panel_pieces`, `premade_values`,
`header`), `Page::NewCareer` as the picker (the six create-a-player
records 234..239 as the premades) and `Page::PremadeAsk` as its dialog;
EDIT makes the career and opens the rewrite's editor on its baller over
the crew's page. Compared (pre-match offset (1, 3)): `bal_premade.png`
14.6, `bal_premade_next.png` 15.9 (R1), `bal_premade_confirm.png` 10.3.
Left: the 3D baller stands whole in the rewrite's default outfit where
the original frames it chest-up in the record's outfit (the model
viewer's camera per panel is not traced); the career dialog's Male and
Female still open the premade picker (the wizard, slots 3..11, is the
rewrite's list pages). (Since the wizard's pass, below: the premade
wears its record's outfit, chest up, 13.4; EDIT opens the wizard.)

## Create Baller's wizard (classes 0x4a, 0x4b, 2026-10-19)

`FUN_00273f58` makes the screen's panel slots; mode 3 opens slot 3. The
wizard's panels and their order, from each panel's next (vtable +0x164)
and back (+0x16c):

| Slot | Panel | Constructor | Layout | Title (text) | Lit tab |
|---|---|---|---|---|---|
| 3 | Build | `FUN_00276cc8` | 65 | 0x2300 | 90, frame 0x12 |
| 5 | Look | `FUN_0027b908` | 67 | 0x2a00 | 91, 0xd |
| 6 | Hair | `FUN_0027a7e8` | 68 | 0x2800 | 92, 10 |
| 7 | Facial Hair | `FUN_00279aa0` | 69 | 0x2600 | 93, 0x10 |
| 8 | Outfit | `FUN_00278748` | 70 | 0x2400 | 94, 0xe |
| 9 | Skills | `FUN_0027cfe8` | 71 | 0x2c00 | 95, 0xb |
| 10 | Name (first, last, icon) | `FUN_0027bfa0` | 72 | 0x2b00, 0x2b09 | 96, 0x11 |
| 11 | "Is This Your Baller?" | `FUN_00279628` | 73 | 0x2500 | |

- *The flow.* Build, Look, Hair, Facial Hair, Outfit, Skills, Name; Hair's
  next (`FUN_0027a8f0`) and Outfit's back (`FUN_00278a48`) skip Facial
  Hair when the player's field 12 (gender) is set. Skills' next goes to
  the slot in its +0xc0 (10). The name panel's next asks "CREATE / %1S?"
  (`FUN_0027c088` → `FUN_00274260`, rows YES, EDIT, CANCEL); YES saves the
  player (`FUN_002745d8`: +0x158, `FUN_001e6888`), EDIT goes back to slot
  3, CANCEL stays (`FUN_002743d0`). Slot 11 is not on this way.
- *Text files.* The layouts' text ids name files of the game's whole
  table at `0x3ec2f0` (101 files): 0x20 `createplyr.txt` .. 0x2c
  `createplyr_skills.txt`, 0x2d `wdtm.txt`, 0x30.. `editplyr*.txt`.
  `street_assets::locale::FE_TEXT_FILES_ALL` holds it and
  `Look::text_full` reads it; `Look::text_id` keeps the first 30, which
  the screens built before this resolve their layouts' words through.
- *Every panel* (`FUN_00274950`): the first and last names on elements 8
  and 12 in 0xfffbac00 (`FUN_002763c8`, fields 2 and 1), the height and
  weight on 13 and 17 (`FUN_002765c0`, fields 8 and 9), "Total: %1d DP"
  on 3 (`FUN_002764c8`, the screen's +0x130 +0x30), "Item: %1d DP" on 19
  (`FUN_00276520`, the panel's +0x1bc of the focused row). The front end's
  `%1d` groups by thousands ("1,500").
- *The lists.* List 0x14 (element 20) runs over the kind-10 rows 21..,
  list 0x2c (44) over 45..; each row's words come from a kind-19
  template (160 for 0x14, 220 for 0x2c) at the row's point, the row
  widget's colours (0xff88a0be, lit 0xfffbac00). `FUN_00274a40` hands the
  arrows 4, 5 (0x14) and 6, 7 (0x2c) to the lists, shown while a list
  runs past its rows (Look, Hair, Facial Hair and Outfit show the down
  arrow; the icons both).
  - Build: `0x479418` (Stocky, Athletic, Lean, Massive) or `0x479420`
    (Fit, Athletic), text and help ids (`wdtm.txt` 0..5); a woman's two
    rows leave bars 103.. hidden and box 18 shown. The sizes M, L, XL,
    XXL (`0x479410`) over the blocks 142..153, the cursor box
    (`FUN_0021b2f8`) 62 wide and 41, 50, 61, 73 tall (`0x479400`), 5 and
    9 inside block 142 + size. Item: `FUN_00277478`, the build's price
    and the size's.
  - Look: the rows' faces through `FUN_001e3168` (the table at
    `0x4602b0`): a man's 18 at `0x4603c8` (15, 13, 8, 14, 2, 6, 9, 17,
    11, 5, 1, 10, 3, 16, 4, 0, 7, 12), a woman's 6 at `0x460410` (3, 4,
    0, 5, 1, 2). "Look A" is face 15, not 0.
  - Hair and Facial Hair: Bald or None, then the styles
    (`editplyr_hair.txt` 2.., a woman's 20..; `editplyr_facial.txt` 2..);
    the colours are boxes of 19 by 24 (`FUN_00275f68`, `0x3f8178`,
    `0x3f8174`) over the swatch frames 134.., filled from `0x3f8828`
    (eight) and `0x3f8680` (six), drawn at half brightness.
  - Outfit: the build's nine (`editplyr_clothes.txt` 6 + 9 x build..),
    Color A, B, C on list 0x2c.
  - Skills: three kind-10 columns a row (`FUN_0027d360`, `FUN_0027d5f0`):
    the name (template 160), the crowns (a kind-20 meter at column 1's
    point) and the next crown's price (template 220, font 2:
    `createplyr_skills.txt` 4 "%1d DP", 5 "Max").
  - Name: "Enter First Name", "Enter Last Name", "Select Baller Icon"
    (`createplyr_name.txt` 6..8) on element 10; the keyboard
    (`FUN_0023c7b8`, User Setup's, with RANDOM under DONE) at element 11,
    opening on DONE with "PLAYER" (`entername.txt` 45) for the first
    name; the icons are list 0x2c's three of seven (`FUN_00276230`,
    `0x3f8b80`), portrait tiles of sheets 0xb0.. (`FUN_00223d88`), the
    title "Choose Icon" (`createplyr_name.txt` 9).
- *The 3D baller* (`FUN_002db8b8`: viewport (66, 134) 199 by 272 on the
  layouts' screen, `FUN_003235d0(66, 134 - 16, 199, 272)`). The screen's
  +0xf0 state picks the framing (`FUN_002745d8`, through the wipe
  `FUN_0023f440`), and `FUN_002dc0e0` places the camera by the viewer's
  +0x88:
  - 1, the whole baller (`FUN_00274808`: Build, Outfit, Skills, Name;
    pose 0x477): the camera 76.13 in front, 4.29 above the hips, level.
  - 0, the head (`FUN_002747b8`: Look, Hair, Facial Hair; pose 0x476):
    level with bone 21, 23.74 less the zoom (held at -3) in front.
  - 3, the premade picker (`FUN_00274858`; pose 0x477): 36.7 in front;
    the traced height (6.18 above the hips) puts the head at the panel's
    top where `bal_premade.png` has it near the middle, so the rewrite's
    is measured from the grab (20 above the hips).
  - 2: 30.32 in front, tipped 13.61 degrees; no panel asks it.
  The viewer's units are inches; the field of view is not traced (the
  camera's projection is set past `FUN_003235d0`'s cut) and is fitted to
  `my_team_wizard_build.png` (1.1 radians).

Built: `cab_view.rs` (`Step`, `Framing`, `list`, `wizard_values`,
`keyboard`, `header`'s marks), `ballers.rs` (`fresh`, `wizard_move`,
`item_price`, `skill_prices`, `LOOK_ROWS`, `viewer_camera`), the name
panel's keyboards in `typing.rs` (`Field::CabFirst`, `CabLast`,
`CAB_KEYBOARD`), `Page::Wizard` on layout 61 and `Page::WizardAsk`. The
baller's icon is `CreatedBaller::icon` (the record's field 14, the
save's BICN section, imported from the original's +0x58). My Team's
Create A Baller opens it on an empty slot (a man's, until its dialog).

Be A Legend's dialog (`FUN_002cd9c8`'s answers on an empty career):
Male and Female open the wizard (classes 0x4a, 0x4b) on a new baller in
the first free star slot (the career-only 6..9 past My Team's six, then
0..5, as the original's careers each keep their player at +0x6d4); YES
starts the career on it and opens the crew (class 0x62); Premade opens
the picker (0x4c), whose EDIT opens the wizard on the premade the same
way. The dialogs' YES, EDIT and CANCEL are their pages' rows, so up and
down reach them (the picker's dialog had no rows: only CANCEL answered).
Driven by keys from Choose Baller through every panel to YES: the crew's
Welcome board follows.

Compared against the My Team wizard's grabs (the same screen; their
offset (1, -1)), the matched Difficulty at 10.9:

| Grab | Panel | Mean difference |
|---|---|---|
| `my_team_wizard_build.png` | Build | 13.6 |
| `my_team_wizard_look.png` | Look | 15.9 |
| `my_team_wizard_hair.png` | Hair | 15.5 |
| `my_team_wizard_facial.png` | Facial Hair | 15.8 |
| `my_team_wizard_outfit.png` | Outfit | 14.0 |
| `my_team_wizard_skills.png` | Skills | 14.8 |
| `my_team_wizard_first_name.png` | First name | 14.2 |
| `my_team_wizard_last_name.png` | Last name | 13.9 |
| `my_team_wizard_icon.png` | Icon | 14.0 |
| `my_team_wizard_confirm.png` | CREATE PLAYER? | 12.4 |
| `bal_premade.png` | the premade picker (framing, outfit) | 13.4 (was 14.6) |

640x480 and 1280x720: no overlap. Left: "Total" shows the rewrite's
points left (0 on a new save) where the original starts a created player
at 1,500 (`+0x130 +0x30`, not traced); the original's Help ("What Does
This Mean?", `wdtm.txt`) and the back-up question (`createplyr_build.txt`
15) are not built; the head pose (0x476) is the rewrite's stand.

## Edit Baller (class 0x4f) on its panels (2026-10-19)

`FUN_0028b768(screen, .., backdrop)`: `FUN_002d7720(screen, 0x4f, ..)`,
layout 79 (the title "Edit Baller", 0x300c; the L1 and R1 marks, kind 37
elements 150, 17, 18, 149; the legend "Select" and "Exit", 0x13 and
0x300d). `FUN_0028b930` builds the model viewer as Create Baller's does
and five panels over the editor's base `FUN_0028c798`: Edit Outfit
(`FUN_0028e190`, layout 80), Edit Shoes (`FUN_00295ab8`, 81), Edit Hair
(`FUN_0028ff18`, 82), Edit Facial Hair (`FUN_0028f4f8`, 83), Edit Skills
(`FUN_002966f8`, 84). Their layouts are the wizard's but for the list
templates (152 and 210), the weight (element 14) and five tabs (90..94,
`0x4846e8`: frames 2, 3, 5, 4, 1; lit 0xe, 0xf, 10, 0x10, 0xb by each
panel's enter). Each panel's legend at (57, 424) is "Edit Moves" with
Circle.

Built: `cab_view::Edit` (layouts, tabs, framing, L1 and R1 round the
panels skipping Facial Hair for a woman), `cab_view::EDIT` (the screen's
marks), the shoes' list (`editplyr_shoes.txt` 2.., in price order),
`ballers::edit_move` and `edit_price`, `Page::EditBaller` on layout 79:
the lists move the baller, Start (Enter) saves and leaves, Circle opens
Edit Moves, Triangle leaves without saving. The names, gender, build,
height and look are not on the editor (`wdtm.txt`: "can only be adjusted
during the initial Create Baller process"); the rewrite's list editor
with them is gone. The Help legend's "Exit" shows Start's icon in the
grab where layout 79 gives action 1 (Triangle): set after the load, not
traced; the rewrite gives it Start.

Compared at (1, -1): `my_team_editor.png` 14.4, `my_team_editor_outfit_b.png`
14.6 (Item 50 DP both; Total is the rewrite's points). Be A Legend's
Edit Baller from the map (Circle) on a premade career star opens the
editor on its record as the record has it (Outfit D, as the grab):
`bal_edit_baller.png` 12.6 at (1, 3). 1280x720: no overlap. Left: the original asks before buying an item ("Use %1d
development points to unlock %2S?", `editplyr.txt` 4,
`my_team_editor_buy.png`) and before leaving (9); the rewrite charges on
saving, as before.

## Edit Moves (class 0x55) on the editor's screen (2026-10-19)

Edit Moves is the editor's screen with one panel. `FUN_00290b40` makes the
screen (`FUN_0028b768(screen, 5, 12)`: class 0x4f, layout 79, over backdrop
12, whose picture panel holds the "Vol. 2" still and whose left panel has
step 1's three slot bars drawn in); its enter (`FUN_00290bb8`) builds the
panel `FUN_00290d80` over layout 85 (class 0x55, the editor's panel base
`FUN_0028c798`), names the screen "Edit Moves" (`editplyr_moves.txt` 0,
0x3600), moves the screen's L1 and R1 icons (17, 18) to (104, 109) and
(279, 109) by the category header and deletes its arrows (149, 150). The
same screen class with `FUN_00293bc0`'s panel (`FUN_00293a00`, title
0x360e) is Inside Vol. 2's The Moves.

- *The categories* (`0x3f9bc8`, seven of eleven words): the slot table (0
  dunks `0x43d3b0`, 1 tricks `0x43d408`, 2 shots `0x43d460`), the move
  kind, the level (0 for any), the first `XDBMOVE` row and how many (Dunk
  Lvl 3: rows 10.., 16; Lvl 4: 26.., 23; Lvl 5: 49.., 5; Trick Lvl 3: 54..,
  14; Lvl 4: 68.., 16; Lvl 5: 89.., 4; Shots: 0.., 10), the name (0x3604..)
  and the steps' word (0x360b shot, 0x360c dunk, 0x360d trick); the enter
  counts each category's slots and their first (the words 9 and 10).
- *The button combinations*: each slot table entry is (turbos, level); the
  turbos are bits 14 L1, 15 L2, 16 R1, 17 R2 (dunks' and tricks' eleven:
  0x3c000; 0x1c000, 0x2c000, 0x34000, 0x38000; 0xc000, 0x14000, 0x24000,
  0x18000, 0x28000, 0x30000; the shots' six the last six). The panel adds
  the move's button, 0x10 (Square) for the tricks' table and 0x80 (Circle)
  for the others, and builds a widget per slot (`FUN_00212880`): twelve bits
  at `0x466878` with their `XACOMMON` sheet 1 image 0 frames at `0x4668a8`
  (L1 5, L2 8, R1 6, R2 7, Square 1, Circle 0), a "+" (0xffdcdcbb) 8 pixels
  past each icon and the next icon 5 back from its end.
- *Step 1* (`FUN_002921f8`, its tail past `FUN_0030fa50` read from the
  bytes): list 20 holds the category's slots, two rows each (`+0xfa` 2):
  "%d.  %s" (`0x485b78`, template 152) and the slot's combination (list
  items 1, 3, 5, anchored by their right and middle on rows 22, 24, 26);
  the list dims the slots it does not focus (`+0x124`) and the panel's
  three boxes (281 by 72 at (51, 149), (51, 227), (51, 303), 0x7f000000;
  `FUN_002916c0` hides the one over the focused slot) darken the backdrop's
  bars. The header list 142 wraps (`+0x11c`); L1 and R1 (0x31, 0x33) step
  it and refill the slots on the first. The screen's legend's first pair
  becomes Exit on Start (`FUN_0021aa70(legend, 0, 0x13)`,
  `FUN_0021aac0(legend, 0, 4)`), its icon raised 3 pixels; the help line
  (element 1) "Step 1:  Pick a %1s to replace." (2) with the word. This is
  where Edit Baller's "Exit" on Start comes from.
- *Step 2* (`FUN_00291c70`): list 440 (rows 441..453, template 260, over
  the bars 101..107 which the enter sets to `XACOMMON` image 6 frame 9)
  holds the category's moves in no slot, those to be had first, then the
  locked, each in row order; "Item: %1d DP" (`editplyr.txt` 20, element 19
  on the gold box 463) and "Total: %1d DP" (21, element 3 on 464); the
  header's name turns cream (0xffdcdcbb) and its arrows and the L1 and R1
  icons go; the legend's first pair is Cancel on Triangle (`common.txt` 5,
  action 1); the help "Step 2:  Pick a new %1s." (3).
- *What a move is* (the panel's +0x184, +0x18c, `FUN_00293400`, +0x17c over
  the user record): the kind's own moves are free (`FUN_001d9e60`: shots
  0..5; dunks 49, 26, 27, 48, 28, 10..15; tricks 89, 68..71, 54..59; the
  player's reset `FUN_001d96a8` puts them in a new baller's slots); sixteen
  moves are a reward card's (`FUN_001e57a8` keeps them from sale: the
  Street Milestones table's moves and rows 0x28, 0x29, 0x2c, 0x2f;
  `FUN_001d6b50` names each one's card: 8 → 5, 9 → 0x44, 0x27 → 8, 0x28 →
  0x3c, 0x29 → 0x3a, 0x2a → 0x41, 0x2b → 0x46, 0x2c → 0x22, 0x2d → 3, 0x2e
  → 0x43, 0x2f → 0x3e, 0x4e → 4, 0x4f → 6, 0x50 → 0x45, 0x51 → 7, 0x52 →
  0x42), owned once the card is (it costs nothing or was bought,
  `FUN_001d26e8`), else "Locked" (0x3617); the rest sell at `XDBMOVE` field
  4 once (the record's bitset at +0x664, `FUN_001e7618`).
- *Cross in step 2* (`FUN_00292788`): an owned move goes in the slot; a
  locked one says "This move is currently locked." (`editplyr.txt` 22,
  `FUN_0025d5c8` 250 wide, Continue); one the points do not cover "You do
  not have enough development points to unlock %2S." (6, `FUN_0025d660`
  350); else "Use %1d development points to unlock %2S?" (4, `FUN_0025d6d8`
  350: Yes, No, No lit). Yes (`FUN_00292bb8`) buys (`FUN_001e5ac0`: the
  points, then the bit) and fills the slot. The width dialogs wrap their
  words to the width and are at least that wide.
- *The film room*: the screen plays no loop (its +0x15c is the editor's
  empty `FUN_002d8fb0`; `XLP0004`, which the rewrite used to play here, is
  User Setup's at (464, 277), its screen's +0x15c `FUN_00298e20`). Circle
  (`FUN_00292ce0`'s 0x2a, "Preview Move", the panel's legend 15) on a move
  to be had puts a movie widget at (350, 110) playing
  `movies/filmroom/` + the move's clip; element 147 names the focused move
  (or "Locked").

Built: `edit_moves.rs` (`Category`, `CATEGORIES`, the slot and combination
tables, `FREE`, `CARD_MOVES`, `standing`, `pick_list`, `select`, `buy`,
`Ask`, `values`, `extra`), `cab_view::Pic` (pictures a panel's own base or
lists give: `XACOMMON` crops, list arrows, none), the kind-5 help line and
dimmed rows on `panel_pieces`, `cab_view::LegendPair` (the screen legend's
first pair), `pause::wrap` and `Dialog::width`, `Page::MoveSlots`,
`MovePick`, `MoveAsk` with `menu::moves_keys` and `open_moves`, the film
room at `edit_moves::FILM_ROOM`. Edit Moves from a baller's dialog saves
the baller when it is left; from Edit Baller it returns there. A new
baller from the wizard starts with the kinds' own moves in its slots. The
rewrite takes the move's points when the baller is saved, as the rest of
its editor (the original takes them at Yes).

Compared at the My Team grabs' offset (best within a pixel of (1, -1)):

| Grab | State | Mean difference |
|---|---|---|
| `my_team_edit_moves.png` | step 1, Dunk Lvl 3, slot 1 | 11.5 |
| `my_team_edit_moves_row2.png` | slot 2 | 11.5 |
| `my_team_edit_moves_pick.png` | step 2 | 12.9 |
| `my_team_edit_moves_buy.png` | "Use 50 development points..." | 8.6 |

1280x720: no overlap. Left: the preview runs until the focus moves (the
original's end of the clip is not traced); the buy dialog's body lets the
screen through in the original (the screen behind keeps 0.46 of its light
there against the pause's 0.34), not ported.

## The Help legend's white, found (2026-10-19)

The writer is the legend's own loader. `FUN_0021a600` (the legend, vtable
`0x467d38`, built from its kind-34 element) adds each label's text through
`FUN_0021a7c0`, which gives it the legend's `+0xd8` (0xff98a8b9); then it
reads the element's prop 10 twice through `FUN_002e3b18` and, unless the
value equals -1 (`beq $v0, $s4` at `0x21a6cc`, `$s4` = -1 as a 64-bit
register), stores it in the text's `+0x4a4` (`sw $v0, 0x4a4($v1)` at
`0x21a6e8`). Nearly every legend's prop 10 is 0xffffffff ("none").

`FUN_002e3b18` loads the 32-bit value with `lwl 5(v0)` / `lwr 2(v0)` (the
props are six-byte entries, so the value is unaligned). Traced in the
project's PCSX2 runtime through PINE: with every `sw rt, 0x4a4(rs)` in RAM
rewritten to store `$ra`, the white legends' texts held 0x21a6e4 (the
return address of `0x21a6dc`'s read: the store at `0x21a6e8`), the grey
ones 0x21a89c (`FUN_0021a7c0`'s); with that store rewritten to `sd $v0,
0x4a0($v1)`, the value's upper half read 0. So under PCSX2 2.8.2's EE
recompiler the load comes back zero-extended (0x00000000ffffffff), the
compare fails and the text takes 0xffffffff, but only where the value's
bytes sit at a layout offset of 2 mod 4 (`legend_align.py` over
`XLAY_FE.BIN`: the Main Menu's Help at 0 is grey; Create A Baller's list,
the event intros, Update Team, and the wizard's Facial Hair, Outfit and
Name panels at 2 are white, its Build, Look, Hair and Skills at 0 grey,
exactly as the grabs show). With the runtime's EE switched to the
interpreter (`[EmuCore/CPU/Recompiler] EnableEE = false`, restored after),
the same Edit Baller Hair panel (its Help at offset 2) draws grey
(`legend_interpreter_hair.png` (152, 168, 181) against
`legend_recompiler_hair.png` (253, 253, 252)): the interpreter follows the
R5900's LWL/LWR (LWL sign-extends, LWR keeps the upper half), so the value
is -1 and the store is skipped.

So on the EE's own semantics every 0xffffffff legend keeps the grey, and
the white in the grabs is PCSX2's recompiler. The rewrite keeps the grey
(`look::legend`: prop 10 0xffffffff is the legend's 0x98a8b9; an explicit
colour, 0xff98a8b9 or 0xfff6f4d7, is used as it is); a grab that shows a
white Help is a recompiler grab.

## Create Baller's and Edit Baller's questions and starting points (2026-10-19)

- *Starting points.* The wizard's start (`FUN_002740b8`, mode 3) resets
  the created player (`FUN_001e68a8(record, 1)`: `FUN_001d96a8` writes the
  kinds' own moves into its slots, shots 0..5, dunks 49, 26, 27, 48, 28,
  10..15, tricks 89, 68..71, 54..59) and sets its points (`+0x30`,
  `FUN_001eb0d0`) to 1,500; a fresh user record gets the same 1,500
  (`FUN_001e6ac8`, `FUN_001e70a8(record, 3, 1500)`). The rewrite's wizard
  baller spends from `ballers::START_POINTS` (`Editing::start`) and keeps
  the rest when it is saved ("Item: 0 DP / Total: 1,500 DP" as
  `my_team_wizard_build.png`). The premade picker's YES (mode 1) keeps the
  record's own balance (not traced).
- *Build's Triangle* (`FUN_00276f08`): "Are you sure you want to exit
  Create Baller? All your changes will be lost." (`createplyr.txt` 6,
  `FUN_00274208(screen, 1)`), or, when the premade picker opened the wizard
  (its enter `FUN_00277cf0` sets the screen's +0x138), "Are you sure you
  want to back up? All your changes will be lost." (`createplyr_build.txt`
  15, `FUN_00275838`); both Yes and No (`FUN_0025d328`), No lit. Yes leaves
  the wizard, or backs up to the picker on its premade.
- *The wizard's points* (`FUN_002758a0`): "You do not have enough
  development points to unlock this item." (`createplyr.txt` 8,
  `FUN_0025d660`, 250 wide, Continue) where the rewrite printed a message.
- *Edit Baller's keys* (the editor base `FUN_0028ce30` over its panels):
  Triangle does nothing (`FUN_0028e140`); Start asks "Are you sure you want
  to exit Edit Baller?" (`editplyr.txt` 9, `FUN_0028c0b8`; Yes leaves:
  `FUN_0028c118` to the map, Create A Baller or Choose Baller); Cross on an
  outfit or shoes not owned (`FUN_0028e710`, `FUN_00295e40`) asks "Use %1d
  development points\n to unlock %2s?" (`editplyr.txt` 3, `FUN_0025d388`,
  Yes and No, No lit) or, past the points, "You do not have enough
  development points to unlock %2s." (5, `FUN_0025d660`, 200 wide). The
  rewrite's Yes owns the item (`ballers::edit_buy`) and takes its price with
  the edit's when the baller is saved; Total leaves out the item under the
  cursor until it is bought (the original only shows it on the baller).
  The rewrite's lists still dress the baller as they move (the original
  wears an item on Cross), and an item worn but not bought is charged when
  the baller is saved. The refunds' questions (`editplyr.txt` 10, 11) are
  not built.
- *The model viewer's projection*: the viewer's camera class (`0x49b2d0`,
  set by `FUN_003235d0`) projects through `FUN_00323728`:
  `FUN_00378328(5.0, w / h, 10, 300)`, a field of view in degrees that
  `FUN_00378328` scales by the context's `+0x190` and `+0x120..+0x12c`
  (not resolved); the rewrite's field of view stays the fitted 1.1 radians.
- *Help* ("What Does This Mean?", `wdtm.txt`): the front end's Help
  overlay (`FUN_00250000(overlay, 0x28, screen, class, ..)`: the class's
  rows from the table at `0x46fbc8`, the title `help.txt` 171, "What Does
  This Mean? Overlay" (174) on action 0x45, `FUN_00250b28`) is not built
  for any screen. Square posted to the project's PCSX2 runtime did not open
  it (other keys did), so it is not grabbed.

Compared (My Team grabs at (1, -1)): `my_team_editor_buy.png` 11.9;
`my_team_wizard_build.png` 13.5 (Total now 1,500 DP as the grab).
1280x720: no overlap.

## Options (class 0x28, layout 40, 2026-10-19)

`FUN_002a4c58` makes the screen (`FUN_002d7720(screen, 0x28, 0x1c)`):
layout 40 over backdrop 0x1c. List 55 holds the rows, two widgets each: the
label (a kind-19 row, font 2, left on the even nodes 75..89 at x 92) and
its value widget (centred on the odd nodes 76..90 at x 446). The rows in
the order the screen shows them (`options.png`, `options_scrolled*.png`),
by their label element: Difficulty (1), Controller Layout (16), Game Data
(126), Trax (15), Audio Options (7), Speech Volume (5), Music Volume
(6), SFX Volume (4), Replays (3), Onscreen Help (10), Visual Effects (14),
Autosave (12), Widescreen (13), Adjust Screen (9), Exit (17); eight show,
the list scrolls a row at a time (it does not wrap) with its arrows at (80,
101) and (80, 363).

- *Values*: the option widgets `0x35` (Difficulty: `options.txt` 21..23),
  `0x33` (Audio Options: 24..26), `0x36` (Replays: 47..49), `0x31`, `0x2e`,
  `0x30` and the widescreen's (On 27, Off 28); the volumes are the sliders
  `0x6f..0x71` (`FUN_0021d148`, the pause's). A focused option widget's
  words are gold with the orange arrows 117 pixels each side; the others
  are the rows' 0x88a0be.
- *The help line* (element 114) follows the focused row (`FUN_002a5150`:
  `options.txt` 30, 43, 42, 29, 36, 34, 35, 33, 32, 38, 41, 39, 40, 37, 44).
- *Keys* (`FUN_002a4d18`): Cross on Controller Layout opens class 0x37, on
  Game Data class 0x61, on Trax class 0x30, on Adjust Screen the screen
  adjuster (`FUN_0025b150`), on Exit keeps the settings (`FUN_002a5870`)
  and leaves (class 3; when they changed, through `FUN_002d8460(screen,
  0x3f)`);
  Start does as Exit; Triangle leaves, asking "Do you want to cancel your
  changes?" (`options.txt` 46, `FUN_0025d328`, No lit) when an option
  differs from the settings (`FUN_002a53b0`); Circle opens the credits
  (class 9; "The movies", below); Select asks "Exit to Main Menu?"
  (`FUN_0024bfa0(.., 0x85)`; "Select's questions", below); Square the
  Help overlay.
- *The legends*: layout 40's own, Credits on Circle (left), Cancel on
  Triangle and Exit on Start (right), Help.

Built: `options_view.rs` (the rows on the nodes, the values, the arrows,
the sliders), `options::row` in the screen's order, `options::value`,
`options::changed`, `Page::OptionsAsk`, `menu::options_keys`.

*The rewrite's own settings* are off the original's rows: a window or
full screen, the Wide picture (the rewrite's widened 16:9 picture, with
Widescreen On) and controller vibration are `Page::PcSettings` ("PC
Settings", `options::pc`), in the rewrite's own list look, which Adjust
Screen opens in place of the original's TV picture adjuster. Widescreen
on the Options screen is the original's Off and On.

Compared ("the rest" grabs; best at (2, -1)): `options.png` 12.8,
`options_row2.png` 13.0, `options_scrolled.png` 13.0,
`options_scrolled3.png` 11.7. 1280x720: no overlap. Left: the slider's
look when its row is focused (no grab; the rewrite fills it amber as the
pause's), and the label words sit about 2 pixels left of the original's.

## My Team's Create Team on the crew's boards, and the full team list (2026-10-19)

- *My Team's way in* (`FUN_002a3c60`'s second row): Create Team (class
  0x35) on a new team, straight from My Team; the original has no list of
  the created teams there (they are deleted through Game Data). The
  rewrite's own list of the created teams (to edit or delete one, its
  `Page::Teams` and editor) stays, a row of the rewrite's PC Settings
  page since 2026-10-20 (My Team's Select asks the original's question;
  "Select's questions", below).
- *After the fifth pick* (`FUN_002810d0`'s states as the crew's): the
  city's keyboard (state 0, "Where's Your Team From?", typed "NEW YORK",
  `createteam.txt` 20 for My Team), the name's (state 1, "BALLERS", 21),
  Choose Logo (state 2), then the memory card widget's "Create team ...?"
  (corrected 2026-10-20, "The front end's transitions, join a team and the
  rematch rules" below; "Do you want to save your team?", state 4, has its
  answer's code but nothing opens it). Triangle on Choose Logo goes back
  to the name's keyboard.
- *Cancel* (state 3): Triangle with no pick to take back asks "Cancel and
  Return to My Team?" (`createteam.txt` 23, 0x1d17) or, on the crew,
  "Cancel and Return to Choose Baller?" (22, 0x1d16), Yes and No, No lit;
  Yes goes to class 6 or 0x36 (`FUN_00280cd0`'s 0x3f2).
- *A full list*: the card widget's question says so (`delete.txt` 3,
  dialog 0xd4) and a team of the same name is overwritten after
  `delete.txt` 11 (0xce), below. "There are already 10 existing Created
  Teams." and "Overwrite" (`createteam.txt` 12, 13) are in the text but no
  code reads them (no instruction builds 0x1d0c or 0x1d0d).

Built: `create_team::Stage::Cancel` and `Stage::Save`, `create_team::
question`, `menu::team_navigate` on the crew's states, the boards drawn
for both pages, `question_keys`. Checked by driving keys from My Team
through the grid, both keyboards, Choose Logo and Yes (the Main Menu
follows, the team saved); no grab of My Team's boards exists (the crew's
compare at 10.8..15.5).

## Inside Vol. 2's The Moves on the Edit Moves panel (2026-10-19)

- *The screen* (`FUN_00293a00`, Inside Vol. 2's row to screen 101 in
  `docs/TRACE.md`): the editor's screen (class 0x4f, layout 79, backdrop
  12) titled "The Moves" (`FUN_00293a78`, 0x360e), holding
  the Edit Moves panel (`FUN_00293bc0` over layout 85) with its browse
  flag (`+0x1c0`) set from the start: no baller, no slots, only step 2's
  list.
- *The list* (`FUN_00294658`): list 440's seven rows (441..453, template
  260) hold every move of the category in row order, the reward cards'
  moves last (`FUN_00295688` keeps a card's move dark until the card is
  the player's, `FUN_001e57a8`); the list's arrows 460 and 461 when there
  is more. No Item or Total: a box of 0xff010955, 279 by 27 at (53, 368),
  covers them (`FUN_00294000`). The stars stay dim (no level to show).
- *The header* (list 142, node 143): L1 and R1 step the seven categories
  and wrap (`FUN_00295460`), the name gold with both arrows, from
  `editplyr_moves.txt` 16..22 ("Dunks - Level 3" .. "Shots", `0x3f9de0`'s
  third words) rather than Edit Moves' names; the screen opens on Dunks -
  Level 3, the first row lit.
- *The help line* (kind 5, element 1): "Preview the highlighted %1s."
  (0x360f, `editplyr_moves.txt` 15) with the category's word.
- *The legends*: the screen's pair 0 is Exit on Triangle, pair 1 hidden
  (`FUN_0021a9b8`); the panel's legend 15 "Preview Move" on Cross
  (action 0). Cross plays the focused move's clip in the film room
  (`FUN_00294e28`), not a locked one; Triangle leaves to Inside Vol. 2.

Built: `edit_moves::Shown::browse`, `browse_list`, `browse_locked`,
`BROWSE_NAMES`, `BROWSE_COVER`; `cab_view::LegendPair` now names its
element and can stand alone (pair 1 hidden); `menu::moves_keys` takes
`Page::TheMoves`; the old kind pages (`ballers::all_move_items`) are
gone. Compared with `the_moves.png`, `the_moves_row2.png` and
`the_moves_cat2.png` (the rewrite's after E): 11.9, 12.0 and 11.8 at
(2, -1), the film room's clip playing after Cross, Triangle back on
Inside Vol. 2's second row; 1280x720 without overlap.

## High Scores (class 10, layout 10, 2026-10-19)

`FUN_002d1e80` builds the screen over backdrop 0x17 (the rewrite had
Options' 0x1c). Its build, `FUN_002d1f08`, puts ten tabs on list 1 (the
kind-9 strip at (190, 106) with one node, 2, at (320, 106)): row widgets
3000 and 3002..3010 holding `score.txt`'s names 0x700 and 0x702..0x70a in
font 1, centred on the node's middle. Fastest Rise To Fame (0x701, widget
3001) and Pass Outs (0x70b) are never made: the screen has ten tabs, not
twelve. The list opens on 0xbba, Win Percentage, not on the first tab.
`FUN_002d3278` steps the tabs round on L1 and R1 (actions 0x31, 0x33; the
d-pad's left and right, 0x1d and 0x1e, go the same way), and
`FUN_002d3438` maps the focused tab to the table `+0xf0`: 3000 Street
Legends (0), 3002 Win Percentage (1), 3003..3005 trick points and the two
gamebreakers (2..4), then 3006 steals (7), 3007 blocks (6), 3008
alley-oops (5), 3009 long range shots (8), 3010 dunks (9). The L1 and R1
icons are kind 29, elements 4 and 6 (`FUN_00212748(.., 5)` and `(.., 6)`),
the list's arrows kind 37, elements 3 and 5. Cross and Triangle both go
back to the Main Menu (`FUN_002d30f0`, 0x23 and 0x25), Square is Help.

`FUN_002d2398` fills ten row groups (2000..2009, kind 0x12, three font-1
texts each, the first 0xff88a0be, the others 0xffdcdcbb) from the point
(310, 133) (`DAT_003fd638`/`DAT_003fd63c`, set by `FUN_002d35b8`), the
second text 20 and the third 140 right of it:

- Street Legends (5 rows, `DAT_0045e2a8`): each row two groups, the names
  centred at (320, 127.5 + 25.5 * 2k) ("%s %s", or "%s \"%s\" %s" with
  the nickname), then at (210, 124.5 + 25.5 * (2k + 1)) the record
  (`score.txt` 24, "%1dW-%2dL") and the share (25, "%1S W%");
- Win Percentage: the name right-anchored at 280, the record at 300 and
  the share at 420, 24 apart;
- the single-game tables: the name right-anchored at 310, the figure at
  330. The grab shows the figures grouped ("500,000") though the string
  is `sprintf("%d")`; where the commas come from is not traced.

The share (`FUN_002d2f08`, English) is ".%03d" of wins over games times
1000 plus a half, "1.000" with no losses.

The tables behind it are the original's now (`street_sim::records`):

- A fresh profile's Win Percentage holds the executable's ten rows
  (BONAFIDE 95-5 .. CLYDE 25-75; the import had dropped them, and the
  rewrite's table was the users who had played).
- `FUN_001d4438` (after a game, modes 2 and 4, per named user): the
  user's record goes to Win Percentage, the user's side's totals for the
  game (`FUN_001871e0(.., 1, stat)`: 0xb trick points, 0xc and 0xd the
  gamebreakers, 2 alley-oops, 6 blocks, 5 steals, 8 long range, 1 dunks)
  to the eight single-game tables under the user's name. The rewrite had
  offered every player's own figures under the player's name.
- `FUN_001d4070`'s placing: a single-game row goes in above the first row
  whose figure is no more than its own (a tie puts the newer row first);
  Win Percentage looks the name up (`strncmp(.., 8)`): a name there is
  replaced and the table sorted again with the C library's `qsort`
  (`FUN_001d43a0`), unless its wins are no more than the row's and it has
  under ten games; a new name needs ten games and goes in above the first
  row whose share is no more. Street Legends (mode 5, at the career's
  finish) places by share the same way and keeps the star's first and
  last name, nickname and record (`+0xf38`, `+0xf3c`).
- The save keeps the legends' records and nicknames and Win Percentage in
  two new tagged blocks at the end of the records (`LGWL`, `WINT`); an
  older save reads its legends' games as wins and starts Win Percentage
  empty.

Compared with the grabs at (1, -1) or (2, -1): Win Percentage 14.9
(`high_scores.png`), Trick Points 13.5 (`high_scores_table2.png`), Level 1
Gamebreakers 12.1 (`high_scores_table3.png`). 640x480 and 1280x720: no
overlap. Code: `score_view.rs`, `highscores::record_match`,
`street_sim::records`.

## Game Data (class 97, layout 97, 2026-10-19)

`FUN_00298eb0` builds Options' Game Data over backdrop 0x1c.
`FUN_00299600` registers six rows on list 9 (the nodes 3..8 at x 92, 110
to 273 down): elements 10 (Save), 11 (Load, made by the code: font 2,
text 0x3e02), 12, 13, 14 (Delete Name, Delete Created Team, Delete Created
Baller) and 15 (Exit, `common.txt` 19). The rows carry no stars (no
`FUN_0021c628`), so a lit row only turns gold. The layout's legends are
Back on Triangle (17) and Help (16); the help line follows the row
(`FUN_00299480`: `gamedata.txt` 6..11). The same build makes the memory
card widget 0x3c8c (`FUN_002294e0`, 0xc1c0 bytes), and `FUN_00299110`
hands it each row's work: Save and Load (`+0x16c`, `+0x174`, 0x13f), the
deletes (`+0x17c` with 8 names, 0x20 teams, 0x10 ballers); Exit and
Triangle go back to Options (0x28).

Save and Load are the card widget's own work, its checks, questions and
messages as the original shows them, the rewrite's file standing for the
card ("The memory card's messages", below; until 2026-10-20 they stood
as file notices). A delete with nothing to delete says so
(`FUN_002278c0` case 1: `delete.txt` 15, 16, 17 for names, teams,
ballers, with OK, `game_data_delete_none.png`). Otherwise the card's
list dialog (`FUN_00258a48`) shows its title (0x2e00 Delete Name, 0x2e02
Delete Team, 0x2e01 Delete Baller) and a row an entry in font 2: a user's
name, a team's name (`team + 0x3a`), a baller's slot and name ("%d.  %s");
Cross on one asks "Are you sure ...?" (`delete.txt` 12, 14, 13; YES and
NO, NO lit), and Yes deletes and ends with "Delete successful." (6, case
10, a message alone for 2 s). A baller on a created team asks first
whether to delete the team (18), as the original's question does. The
questions and notices are the card widget's own dialogs, drawn in its
box. Leaving Game Data after a delete saves to the card (`+0xf0`). The
rewrite's old flow (press twice, a line of its own words) is gone.

`look::screen_binding` binds the rows (list 9, no stars); the Main Menu's
code-made row and Game Data's now share one table of such rows
(`look::dynamic_row`). Compared with the grabs at (2, -1): Game Data 10.2
(`game_data.png`), the empty Delete Name 6.5 at (1, -1) in the card's box
(`game_data_delete_none.png`; 12.6 in the front end's dialog, which
stood 5 rows off). 640x480 and 1280x720: no overlap. Code:
`game_data.rs`, `card.rs`, `menu.rs` (`Page::Delete`, `Page::DataAsk`).

## Trax (class 0x30, layout 48, 2026-10-19)

`FUN_00265848` ("BIG RADIO") builds Options' Trax over backdrop 0x1c,
and its build (`FUN_002658c0`) makes a single widget, the radio
(`FUN_00218e68(.., 0, ..)`, 0xc0 bytes), which owns the rows. It makes its
own list (`FUN_0021e7e8`, five rows shown, 269 high) and ten nodes: for
row k, one at (510, 128 + 52k) for the choice and one at (92, same) for
the label. Its rows come from the table at `0x467710` (16 bytes a row:
the label's text, the kind, a bit, the two choices' texts, the help):

| Kind | Rows | Option bit (`+0x134`) | Choices |
|---|---|---|---|
| 1 | Music Experience (`bigradio.txt` 2) | 0 | Playlist (8), Interactive (9) |
| 6 | nine Playlist songs (11, 10, 12..17, 20) | 3..11 | On, Off |
| 5 | fifteen Interactive songs (21..33, 36, 37) | 12..26 | On, Off |
| 2 | Song Rotation (3) | 1 | Sequential (7), Random (6) |
| 0 | Exit (`common.txt` 19) | | |

`FUN_00219da8` keeps on the list only the songs of the experience chosen
(Playlist the kind-6 rows, Interactive the kind-5 ones) and rebuilds it
when the experience changes. The table has no Track Info row: only the
other table (`0x4678d0`, `FUN_00218e68(.., 1, ..)`) has one (kind 4, bit
2), and this screen never uses it. The rewrite's Trax had put Song
Rotation and Track Info at the top and the songs under them; it now
holds the original's rows in the original's order, and Track Info keeps
its default (it has no row in the front end).

`FUN_00219588` makes each row: the label (font 1 for songs, title and
artist on two lines; font 2 otherwise) left at 92 on the row's middle;
the option widget (`FUN_0023ec50`) right-anchored at 510, its two choices
(font 2) centred in it, 40 wider than the wider choice (`FUN_0023ef70`),
its arrows `FUN_0023efa8`'s 18 left of that width and 20 right of the
column (measured: "Interactive" 391 and 530, "On" 462 and 530). The
focused row's label and choice are the list's gold; the list's "more"
arrows sit at its left edge (the down one at (78, 366) on `ea_trax.png`).
The help line is the row's last short (`FUN_00219bf0`): 40, 39, 41, 45.

The screen's keys (`FUN_00265978`): Triangle cancels, asking
"Do you want to cancel your changes?" (47, `FUN_0025d328`, answer 100)
when the radio says something changed (`FUN_0021a108`), Yes leaving with
the options as they were; Start keeps them (`FUN_00219f58` writes the
bits); the radio's left and right change the row's choice; Cross is
Preview. The rewrite previews a Playlist song by playing it in the
menu's place (`audio::Preview`); an Interactive song plays only through
its PathFinder map in a game, so its Preview does nothing yet.

Compared with the grabs at (2, -1): `ea_trax.png` 11.7, `ea_trax_row2.png`
12.0. 640x480 and 1280x720: no overlap (the Playlist's longest label,
"Not In My House (Vol. 2 Exclusive)", clears the choices). Code:
`trax_view.rs`, `menu.rs::trax_keys`, `audio::Preview`.

## Rewards and Trophies (class 20, layout 20, 2026-10-19)

`FUN_002bfbe8` builds Rewards over backdrop 0x15 and opens on the Street
Legends grid (`+0x25c` 0, `+0x260` 0). The rewrite had a list of
categories, a list per category and a list of trophies; it is now the one
screen, `rewards_view.rs`.

**The build** (`FUN_002bfe10`, its tail past the `FUN_0030fa50` cut read
from the raw code): list 0x1a's tabs (8 row widgets, `0x48f608` + 0x3f2,
texts `0x48f628`: `poster.txt` 10, 9, 11..16, font 2, gold 0xfbac00) on
node 27 (258, 94); L1 and R1 (elements 24, 33, icons 5, 6) and the strip's
arrows (28, 35); two rows of six card sprites from templates 31 and 32 (x
271; y 180, 292) and six "Locked" texts from 36 and 37 (font 2, 0x80
grey, a black two-pixel outline); the four trophy slots 57, 55, 51, 53 at
x 518 (each `XAFESCRN` sheet 0 image 8 frame 0, `0x48f5c8`) with their
panels 39..42 and bars 43..46 (kind-2 rectangles 0x030637, 0x313897); the
grid's arrows 58 (up) and 21 (down); "Total Reward Points:" (20) with the
points (30) nine right of it; "Reward %1d/%2d" (19); the legends 17 (its
word set by code: Unlock Reward 0x911 or View Reward 0x912) and 18 (Exit
on Start, `FUN_0021a7c0` + `FUN_0021aa70(.., 0, 0x13)`); the trophy's
name (54, font 3), its stars 56 and 52 (`XACOMMON` sheet 0 image 1 frame
7), the three under it 47..49 (frame 1) and "Completed" (50); the
close-up's widgets (6, 9, 10, 15, legends 5 and 8); the focus frame
(`FUN_0021b2f8`, id 0x44d) and the price text (font 1, gold, black
outline).

**The tabs** (`FUN_002c1df8`): Street Legends, Legends, Courts,
Jerseys, Development Points, Trophies, round. Categories 5 and 6 (Tips,
Codes) are stepped over: they are not on this screen. A category's cards
stand in record order, four a row for the two legends' categories and
three for the rest (`0x48f648`), two rows shown (`+0x26c` scrolls).
`FUN_002c2d98` puts the middles a card's width plus nine apart about x 271
(cards 86x111 upright, 111x86 wide; the development points' "Locked" 20
lower, `DAT_003fc534`). `FUN_002c2060` lights each card `0xff >> n`
(n: one if not focused, one if not owned) and steps the grid along its
cards, a step past them staying put except down onto a short last row,
which takes its last card. The focus frame is image 8 frame 2, turned a
quarter for the wide cards (`0x48f678`: kinds 3 and 4). The help line is a
locked card's way to unlock (`0x48f658`'s base for the category plus the
card's place: `poster.txt` 135.., 144.., 124.., 170.., 195..) or "Select
small picture to view close up version." (32).

**Cross** (`FUN_002c1370` case 8): on a locked card whose price is over
the points, "You do not possess enough Reward Points to unlock %1s."
(`poster.txt` 8, `FUN_0025d660` 265 wide, Continue); on an affordable one
"Use %1N Reward Points to Unlock %2s?" (4, `FUN_0025d6d8`, Yes and No),
%2s being `FUN_002c4dd8`'s "Reward #%1d %2s %3s" (its place, category and
name; "Reward #%1d" for a card hidden until owned). Yes buys it as the
rewrite's shop always has (a development card goes on to its recipient).
On an owned card, the close-up. Start leaves for the Main Menu (the
keyboard's Escape too); Triangle does nothing on the grid.

**The Trophies tab** (`+0x260` 3, `FUN_002c3948`): the focused slot's
frame (image 10 frame 0, stretched to `0x3fc588`'s sizes, its bottom
middle on `0x3fc568`'s points); a trophy not earned shows `XAFEBGEX`
entries 8..11 side by side at (61, 114) (the "?" star on its rays), an
earned one its movie (`XLP0012`, `XLP0014`, `XLP0013`, `XLP0015` for
trophies 0..3, `0x4d4eb0`) and, in its slot, image 11's frame for it;
"Trophy %1d/%2d" (21); the help 37..40 or 200..203. Up and down move
among the four (clamped).

**The column**: when the movies are on (`DAT_003fe848 + 0x58`), `XLP0009`
(the shelves of trophies) plays at (461, 70) and each slot not yet earned
covers it with its panel, bar and star. The rewrite plays it once any
trophy is earned; before that the slots' sprites alone (as the grabs,
taken with no trophy earned, show).

**The close-up** (`+0x260` 2, `FUN_002c3220`, `FUN_002c2fb8`): the screen
swaps to backdrop 0x16 (its frame without the column, the rays and stars;
the one its grab matches) and hides its title; the card's big picture is
`XAFERWD`'s entry (the picture, less 0x1c past 0x43; 0x49 for Nelly's
0x67; 0x4a for the development points), decoded when asked; "%1d / %2d"
(element 10) between the arrows 9 and 15 counts the cards one may look at
(`FUN_001d2c28`: the owned cards of the five categories, in record
order); left and right step them round; Triangle (and Cross, except on
development points, whose Cross picks the baller) goes back. Its legend
is 8 (Back, Info) for a legend's card and 5 (Back, with Assign Points for
the development points) for the rest. The help is the card's
(`poster.txt` 27..31). The picture's size there is measured, not traced
(the group `+0x214` takes it from somewhere not found); Info's turn to
the card's back is the seventh pass's (below).

Compared with the grabs at (1, -1): `rewards.png` 13.3, `rewards_row2.png`
13.3, `rewards_league_legends.png` 13.0, `rewards_courts.png` 12.7,
`rewards_jerseys.png` 12.8, `rewards_dev_points.png` 12.9,
`rewards_trophies.png` 11.1, `rewards_cannot.png` 12.9 at (1, -2) (the
dialog wraps "You do not possess / enough ..." where the original fits
"You do not possess enough" on its first line: `pause::wrap` measures the
last glyph's advance), `rewards_card_view.png` 14.7. 640x480 and
1280x720: no overlap. Code: `rewards_view.rs`, `menu.rs::rewards_keys`,
`art::RewardArt`, `art::Art::card_frames`, `Art::trophy_unknown`,
`rewards_view::BigCard`.

## The loading screen (class 0x2c, layout 44, 2026-10-19)

`FUN_0029c310` builds it on the loading base (`FUN_0029aac8(.., 0x2c,
0x11)`: backdrop 17 under the picture). The rewrite had shown the picture
alone with a line of its own ("Lakers vs 76ers  Rucker Park  Loading...")
and gone into the game as soon as the load ended. It now runs the
original's states (`loading_view.rs`, `game.rs`'s `loading_keys`):

- **Loading**: the picture (`loading.rs`)
  with, on its star, the spinning ball (group 14 at (110, 334): `XACOMMON`
  sheet 3's nineteen 50x50 images in turn, one each 52 ms,
  `FUN_0029b228`), the ring of twelve small stars round it
  (`FUN_00238478`: sheet 0 image 1 frame 6 at radius 45 from the top
  clockwise, offset (17.5, 16) from the group; `FUN_002386c0` lights the
  first n as frame 5, n counting up every 2.2 s of the load), and
  "Loading" (element 25: `load.txt` 0 in font 5 at 0.9, 0xffd03f, a navy
  0x010954 outline 3.5 out, `FUN_0029ac18`).
- **Continue**: at the load's end (`FUN_0029b438`) a game other than the
  attract (mode 0) shows elements 0 and 1 (the Cross icon at (205, 402),
  `load.txt` 1 at (220, 391)) and waits for Cross (`FUN_0029b120`). The
  rewrite waits in a game started from the menus; the attract, online
  games and `--quick` matches go straight in (`loading_view::Waits`).
- **The controller pages** (`FUN_0029c578`, when Help is on, option bit
  2): over backdrop 0x22 (`FUN_0029b650`), the controller widget
  (`FUN_002144c8` at (70, 90)) on Offense, then Defense
  (`FUN_002156c8`), its rows `pause::controller_rows`' and its pad 15
  higher than the pause screen's; under it the game's mode (element 11,
  font 3: `setchallenge.txt` 55, 57, 58, 59, or "%s : %s" of 1 and the My
  Rules mode 6..13 when one is set, `FUN_0029cbc0`) and the difficulty
  (15, font 2: `load.txt` 11 and `options.txt` 21..23), both 0xffd03f;
  Continue at the bottom right (the icon at (490, 431), the words at (503,
  420) in font 1, 0xdcdcbb). Cross on Defense starts the game; with Help
  off Cross on Continue does.

Not built here: Street School's welcome page (`FUN_0029ca10`, mode 1,
after the controller pages: backdrop 0x1e, "Welcome", "Stretch\nMonroe",
the difficulty; item 2's work); the Pick Up loads' tip pages (modes 2 and
3 alternate a legend card with one of four street tips, `FUN_0029a8c0`,
`FUN_0029b828`: pictures 26..29 with their buttons); the first card's
random start (`FUN_0029a510` marks the first `FUN_003a16b8() % 25` cards
shown at the first load: a draw from the shared generator that the
rewrite's construction stream does not count, so it is left out); and Be
A Legend's covers by the career's progress (`FUN_0029a670`; the rewrite
alternates them in turn). The seventh pass (below) builds the tip pages
and the random start and settles the covers.

Compared with the grabs (the card differs: the original's first was James
Worthy, the rewrite's Nate Archibald): the spinner corner 13.4 and 15.9
(`loading_legend.png`, `loading_legend_continue.png`, region (60, 280) to
(330, 430)); the Offense page 14.5 (`loading_controller.png`, Be A Legend
there and Pick Up here), Defense 14.9. 640x480 and 1280x720: no overlap.

## The results pages compared (classes 52, 38, 35, 36, 87; 2026-10-19)

The post-game boards were built on their layouts earlier (`boards.rs`,
"The post-game boards" in `docs/TRACE.md`); this pass compared them with
the grabs of a Be A Legend win (`results_*.png`) and fixed what differed:

- **The scene's dim.** Every board's constructor turns its screen's own
  full-screen rectangle (`FUN_002d7720`'s `+0xc4`, a 640x480 widget at
  (320, 240)) black at alpha 0xa0 (`FUN_002d8350(screen, 1)`: classes 52,
  37, 38, 35, 36, 87 and 95; the pause, `FUN_002bc328`, passes 0 and does
  not dim). The court keeps 0.37 of its light behind them (measured: the
  rooftop court's 96 under the pause, 36 behind The Rundown). The rewrite
  had no dim: the court showed at full light round the panel. Drawn now as
  the boards' first piece (`BOARD_DIM`, through `look::black_alpha`).
- **Anchored texts.** `boards::place` moved a middle- or bottom-anchored
  text by its font's line spacing; the text widget's box is its text
  height for one line (`FUN_00239e70`), so the Development Points' rows
  (bottom-anchored, align 8 and 10) sat 7 rows high. The hookups' stack
  measures each line the same way (the widget's `+0x54`), and places 4
  after a label and 8 after a value (`FUN_00262ba8`'s last loop: 4 after a
  line whose `+0x18` is not 0xff); the Progress item's gap was 8.
- **The hookups' stars.** The rewrite put two stars beside the header
  ("Your performance has earned you the following hookups:", element
  100). `FUN_00262ba8` and `FUN_002c5cf8` put them beside the single amber
  lines they record (`iStack_f0`, `iStack_ec`, `iStack_e8`): the
  reputation's "Your Reputation Went Up!/Down!", the completed game's, the
  Street Champ jersey's, and Pick Up's "%1d Reward Points", each star
  middle-anchored 4 left and 8 right of the words. `boards::Item::starred`.
- **Continue.** The boards drew the confirm button by the device (a
  keyboard showed "Enter"); the layout's legend (kind 34 at (320, 400),
  text 0x14, icon 0) now draws as the front end's legends do
  (`look::legend`): the Cross icon and "Continue", centred on its point.

Compared at (1, -2) or (2, -1) (the grabs' court is a dark story court; the
rewrite's runs were on Center Court, its darkest, and the data differ:
the crew's names, the 3D star, the stats): The Rundown 21.7, the Baller
Breakdown 19.7 (without its 3D star, which a forced board does not
stand), Development Points 17.4, the hookups 16.9, Street Milestone 2
14.7. 640x480 and 1280x720: no overlap. Not grabbed: Top Ballers (37) and
Pick Up's hookups (34).

## Street School's lesson menu and its one game (XLAY_GM class 2, 2026-10-19)

In the original the School is one game, its lessons picked from a menu
inside it; the rewrite had run each lesson as a match of its own from a
front-end list (`Page::School`). It now runs the original's way:

- *The game.* User Setup's Street School loads the School's game
  (`FUN_001dba68` mode 1, `FUN_001db968`): court 0 (`FUN_001dbc88(.., 0)`),
  the Nets (team 16) against the Lakers (11), each its first three
  (`team * 5 + k`), the person on the Lakers, side 1, as the School's
  captures hold it (`school::prepare`; the School binds its one person on
  either side). The loading screen's controller pages show "Instructor :
  Stretch Monroe" (`setchallenge.txt` 56) under the mode (`FUN_0029cbc0`
  mode 1) and end on the welcome page (`FUN_0029ca10`: backdrop 0x1e,
  "Welcome" on element 21 in its title colours, "Street School / Featuring"
  on 23 in font 3, "Stretch / Monroe" on 20 in font 6, 0xffd03f with a
  0xff040735 outline 3.5 out, `loading_view::Stage::Welcome`). The game
  opens on the lesson menu (`FUN_002b1ae8`) with no intro, and the
  Opposition panel is not shown in it (`FUN_002b2a98` skips game `+0x9c`
  4). The HUD's elements keep their layout points in the School
  (`FUN_002ad190` moves them up 16 only outside it): 16 lower in layout
  space than in a match, as `lesson0-4.png` shows (`hud::draw`).
- *The menu* (`school_menu.rs`). The code-made panel 1000 in a darker ink
  than the boards' (measured: its navy edge 0x00002e, the description box's
  own navy, its blue 0x30398d and grey 0x808e9c; `boards::PanelInk`) over
  an opaque body; the category row and the description's box in navy with
  the panel's blue lines between (measured); the title (element 72, kind
  35, `overlays.txt` 42); the count (70, "%1d/%2d Lessons Complete",
  `school.txt` 0, `FUN_002bb518`); the category on node 74 between its
  arrows (83, 82) and L1 and R1, its template's word (75..77, `overlays.txt`
  39..41) filling list 73's 17-pixel height; three check rows on nodes
  79..81 (`FUN_0021d9d0`: `XACOMMON` sheet 0 image 6, frame 4 the box and
  5 the tick at (3, -4); the words 5 right of the box on its middle in font
  1 at 0.9, wrapped at 115, 0xff91adc0, the lit row 0xfffbac00 pulsing),
  the list's arrows (84, 85) while rows hide above or below, one row a
  step; the lesson's title on 71 and its description or tip on the text
  box 86 (font 1 at prop 0x2c's 0.88, wrapped at 253, 0xff91adc0); the
  legend (69, measured at the panel's foot): Pause, Play (`overlays.txt`
  10) or Replay (8) on a lesson done, and Back (7) in state 1.
- *Its states and keys.* `FUN_002b3cf0`: 0 a lesson not done (Play, its
  description `+4`), 2 one done (Replay, its tip `+8`), 1 Replay pressed
  (Play, the description, Back shown, the other rows dimmed to 0x7f).
  `FUN_002b3ad0`, while no lesson runs: Cross plays the lesson
  (`FUN_002b4400`), on a done one asking first (state 1); Triangle backs
  out of state 1; L1 and R1 step the category and wrap, the new one on its
  first row (`FUN_002babf0`), not in state 1; Start pauses; up and down move
  the list (not past its ends). The records (`PTR_DAT_003fb3e8`, by task;
  the PS2 build's descriptions and tips) and the categories
  (`PTR_DAT_003fb450`: Offense 0..6 and 14, Defense 7..11, Advanced 12, 13
  and 15..25) are `school_menu::RECORDS` and `CATEGORIES`.
- *One game for every lesson.* Cross launches the lesson
  (`school::stage_world`, the manager's `FUN_0018ec28`); its commit (the
  manager's message `0x3c6960`, the overlay listener's slot `+0x27c`,
  `FUN_002b9988`) brings the menu back on the lesson, ticked when it
  passed, and the menu holds the gameplay (`FUN_0033d380(0x19d2)`; the
  rewrite skips the simulation's step while it is up). With every lesson
  done and the user not yet paid (`+0x54` bit 0, once a game by `+0x200`)
  the game ends on the completion instead, its boards paying the School's
  rewards, then Play It on Street School. The simulation's state is
  unchanged (snapshot VERSION 58); the seven School captures pass.

Compared (the court behind differs: the rewrite's camera and players):
the menu 18.3 at (2, -1) over the panel's region (70, 45) to (570, 425)
(`school_lessons.png`; 21.7 the whole frame), scrolled 19.2
(`school_lessons_scrolled.png`), Advanced 17.8
(`school_lessons_advanced.png`), a lesson done 20.1 at (1, -1) (the
School's capture `short-after.png`, 14 lessons done there and 1 here);
the welcome page 12.2 at (1, -1) (`school_welcome.png`); the School's
controller page 15.3 at (1, -2) (`school_loading_controller.png`).
640x480 and 1280x720: no overlap.

Not built: the lesson prompt (`FUN_002ba3b0`: the move's name and its
"press" and button at GUI (55, 320) and (50, 346), posted by the manager
(`0x3c6970`) when its per-lesson readiness check `FUN_0018f740` passes, 23
of them, hidden again by `0x3c6980` (`FUN_002bab48`), fading 4 an update
in `FUN_002b4c38`; the checks belong in `street_sim::school`); the
tutorial pages (`0x3c6990`, `FUN_002bb338`, `FUN_001eedb0`'s nibbles 26..32:
"Player Indicator" and the like, with Continue, as `short-launch.png`
shows); state 1's elements 87 and 88 (code-made, not traced). All three
are the seventh pass's (below).

## The Help overlay (`FUN_00250000`, 2026-10-19)

The overlay shows only while Square is held: a posted tap opens and closes
it within the press, which is why the last pass saw nothing. Holding J
(Pad1's Square) in the project's PCSX2 runtime grabbed it on the Main
Menu, Play It and Difficulty (`help_main.png`, `help_play_it.png`,
`help_difficulty.png` in `street-frontend-sixth-orig`). `help_view.rs`:

- The rows are the screen class's entries in the table at `0x46fbc8`
  (class, icon id, text id in `help.txt`), in its order; a class it does
  not list (Options, High Scores and the later screens) shows nothing.
- An icon id is a record at `0x4702a0 + id * 0x1c` (`XACOMMON` sheet 1,
  image 0, its frame, the text's colour 0xff98a8b9 and font 1); id 20 is
  L1 and R1 side by side (records 8 and 6), a quarter of both widths
  either side of the middle.
- Each row its icon and its words (`FUN_0023aa68`, wrapped at 220) 40 right
  of the icon's middle on the row's middle; a row as tall as its icon or
  its words; rows 10 apart (`0x2505ac`: the spare height `DAT_003f6b68`,
  0 in the executable, shared out with a floor of 10); the icons' half
  width, 40 and the widest text centred on the overlay's point (measured
  (321.5, 239.5); its run-time `DAT_003f6b70` is not in the executable).
- The panel (`FUN_00211278(.., 0x15, 0x1c)`) fitted 30 round the rows
  (`FUN_00212008`), its body opaque; "Help" (`help.txt` 171, font 5,
  `FUN_0020f6c0`) on its corner (measured); the screen behind dimmed (it
  keeps 0.33..0.45 of its light). The menus hold their keys while it is up;
  with L1 held Square stays User Setup's code checker's.

Compared at (1, 3): Main Menu 6.1, Play It 6.9, Difficulty 6.8 (the
panel's region 10.0, 12.3, 12.1). 1280x720: no overlap.

## Create Baller's and Edit Baller's leftovers, settled (2026-10-19)

- *The refund questions* (`editplyr.txt` 10, 11, and 7, "Use %1d development
  points to increase your skills?"): the executable never builds them. No
  instruction loads 0x3007, 0x300a or 0x300b (`addiu`, `ori`, `slti` and
  the like over the whole ELF), where the same scan finds the editor's
  0x3003, 0x3005, 0x3009 and 0x3016 at their callers. Left unbuilt.
- *Slot 11, "Is This Your Baller?"* (`FUN_00279628`, layout 73): built with
  the screen's other panels (`FUN_00273f58`), but the screen opens only in
  modes 3 and 1 (`FUN_00273a08`'s three callers), no panel steps to slot 11
  (`FUN_00275128`'s targets are 0..10, and Skills' next, `+0xc0`, is 10 or
  8). Unreachable; left unbuilt.
- *The buy question's dim*: the screen behind the front end's dialogs keeps
  0.40..0.48 of its light (the Edit Moves and editor buy questions, the
  baller's dialog, the wizard's confirmation, Rewards' "cannot"), the
  pause's Quit Game 0.34. `pause::Dialog::dim` is now a level
  (`FRONT_END_DIM` 0.55, `PAUSE_DIM` 0.66); the dialog's body is not
  translucent (it keeps 0.06). `my_team_edit_moves_buy.png`: 8.6 to 6.9.
- *The viewer's projection*: `FUN_00323728` calls `FUN_00378328(5.0, w / h,
  10, 300)`: a 5° field of view, near 10 and far 300; the matrix's x scale
  is `+0x120 / (tan(2.5°) × +0x190)` and its y scale `−aspect × +0x124`
  over the same, the context's `+0x120..+0x12c` and `+0x190` set at run
  time (zero in the executable). A read of the editor's camera context
  through PINE would give them; the rewrite keeps its fitted 1.1 radians.
  (The seventh pass read them: below.)

## The front end's seventh pass: item 5 closed (2026-10-20)

Branch `frontend-seventh` (not merged), each piece its own commit.
Compared as before (the Difficulty screen scores 10.9).

- **Street School's lesson prompt** (`85279fa`): the manager's second
  half (`FUN_0018eec0`) in `street_sim::school`: the 23 readiness checks
  of `FUN_0018f740` (`FUN_0018f9b8..FUN_001907a8`) over a `Court` view of
  the game; the prompt's show and hide posts (`0x3c6970`, `0x3c6980`) each
  standing fps × 0.75 (`DAT_004baa74`); Dunk to Lay-up's and Give 'n Go's
  latched second prompts; the tutorial pages (`FUN_0018f540`, `0x3c6990`:
  two updates after the launch, the Shot Clock page at the clock's 9),
  the clock refilled once its page is seen and a meter over 45,000
  emptied at the commit. Against the seven School captures the checks
  make 16,118 of the original's 16,162 prompt decisions (99.7%). The game
  draws the prompt (`FUN_002ba3b0`, `school_prompt.rs`: the move's name
  at (55, 320), "press" and its button at (50, 346), `FUN_002b4c38`'s
  signed-byte fade) and the five pages (`FUN_002bb338` on `FUN_00250eb8`:
  Turbo, Player Indicator with `onscr2.ssh`'s indicator, Shot Clock with
  its count, Trick Points, Gamebreaker); state 1 hides elements 87 and 88
  (the menu's L1 and R1 icons) and the arrows (`FUN_002b3cf0`). The
  lesson's restart runs as the captures' (the launch takes the seats'
  controllers; the inbound waits fps × 2, × 0.5 with a page due,
  `0x10b744`); snapshot VERSION 59 (`World::lesson_page`). Compared, the
  held game behind differing: the prompt 20.4 at (-1, -1)
  (`school_lesson0-2.png`; its icon 15.7), the pages 17.3, 19.0, 15.3,
  17.2 and 16.2 (`school_dunks-launch.png`, `school_short-launch.png`,
  `school_advdunk-stuck.png`, `school_trick-launch.png`,
  `school_gb-1.png`).
- **Help on every screen** (`33878c7`): `help_view::TABLE` is the whole
  table at `0x46fbc8` (291 rows to its -1), the page's class the panel's
  where a screen has panels (`FUN_00250000`'s fourth argument); the rows
  wrap at 220 in the executable's own font (an FNTS at `0x430b30`: the
  widget gets its text and width before its font), and only a pair's
  first icon counts toward the icon column. Square on Create Baller shows
  Help (`help_wizard_build.png` 12.6, `help_editor_outfit.png` 10.7):
  "What Does This Mean?" (`wdtm.txt`, `FUN_00250b28`) is action 0x45,
  which this disc neither names (`FUN_002ec008` registers 0x01..0x44) nor
  binds (`CSCRIPTS/USERFE.CFG`), so no press reaches it.
- **The results' Top Ballers (37) and Pick Up's hookups (34)**, grabbed:
  `top_ballers_pickup.png` 22.8 (its panel 17.7), `pickup_hookups.png`
  19.2 (its panel 13.3), the court behind differing.
- **The development points' recipient** (`33878c7`, `cdbb3cc`;
  `FUN_002c1370` → `FUN_0024ed88`, `FUN_002585f8`'s list dialog): the
  title, the numbered ballers, Accept and Cancel over the card's close-up,
  the screen undimmed and the panel's body keeping 0.096 (its size is set
  at run time: measured, 112..529): `dev_recipient.png` 12.7 (the
  dialog's region 14.1). The close-up draws the card's amount (element
  14, kind 35, `FUN_002c4730`'s "%d" of `FUN_001d2818`).
- **Rewards' Info turn** (`f4b131f`): Circle on a legend's card turns it
  (`FUN_002c4c38`: `+0x2d4` 0.125 an update to 0.5, the picture swaps,
  back to 0; the card draws `1 - 2t` of its width) to its back, the
  `XARWDBK` picture by `FUN_002c2fb8`'s tail (0x67 → 0x22, under 0x1f
  less 1, else less 0x42). `FUN_002c4730` puts the legends' card group at
  y 240, the rest at 220. A jersey's close-up shows the Mitchell & Ness
  label (`941b31e`; element 23, `XAFESCRN` sheet 0 image 0x11, the
  jerseys' alone in `FUN_002c3948`). `rewards_legend_closeup.png` 12.3,
  `rewards_card_back.png` 11.5. The dialogs fit a line to its last
  glyph's ink (`381f820`, `Font::ink_width`; `FUN_0025d1e8` sets the
  font before the words): `rewards_cannot.png` 12.9 → 11.7.
- **The loading screen** (`f4b131f`): `FUN_0029c3a8` by the front end's
  mode: the attract's game the logo (`FUN_0029a668`); the School, the league
  Challenge and Be A Legend a legend card; Pick Up a card and a street
  tip in turn (`FUN_0029a8c0`). A tip's page is picture 0x1a + tip with
  its name (the `whatsnew.txt` ids at `0x3fa4f0`, element 2 at (321, 63))
  and its lines (`FUN_0029b828`: elements 28..30, `FUN_00212880`'s button
  combinations, Back 2 Papa's at 0.8): `loading_tip_b2p.png` 12.0. The
  cards' object (`FUN_0029a510`) is made at the boot's class 4
  (`FUN_0029dca8`), after the clock's seed and before any reseed, and
  marks the first `draw % 25` cards shown: the session's generator draws
  it at startup (`Session::draw`). The covers left the to-game loads
  (they belong to the load back to the front end, below).
- **The 3D baller viewer** (`56fd915`, `5405af4`): the projection read at
  run time through PINE (`FUN_00378328(5.0, 199 / 272, 10, 300)`, the
  camera context at `0x9a7380`): 6.83° down the viewport; the cameras'
  translates are the model's own units and the picker's camera its traced
  6.18. Look, Hair and Facial Hair play animation 0x476
  (`createplayer_look`; the table at `0x3de8a0`), the rest 0x477, and
  their camera follows bone 21's posed height each update
  (`FUN_002dbb50`). Build 13.7 → 13.2 (the picture panel 14.2 → 11.8),
  Look 19.2 → 17.8 (24.8 → 17.2; the rest is the head turn's phase: shot
  0.6 s into the panel, 16.7 and 12.6), Edit
  Baller 14.2 → 13.5 (13.6 → 10.5), the premade picker 13.7 → 13.1 (20.1
  → 16.6).
- **Options with a slider focused**, grabbed: `options_slider.png` 13.3 at
  (2, -1).

The new pieces were checked at 640x480 and 1280x720: no overlap. The
grabs are in `street-frontend-seventh-orig`, the rewrite's shots in
`street-frontend-seventh`.

## The load back to the front end (class 0x2f, layout 44, 2026-10-20)

Branch `return-loading` (not merged). Whenever a game hands back to the
front end (the results' end, the pause's Quit, a challenger taken, the
attract's Start), the Front End task's restart (`FUN_002e1bf0`, its
vtable +0x2c) sets class 0x2f: `FUN_0029d368` on the loading base
(`FUN_0029aac8(.., 0x2f, 0x11)`: layout 44 as the to-game screen's,
`+0x118` 0x2f). The rewrite's `Screen::Returning` (`game.rs`'s
`spawn_return` and `return_after_the_load`) follows it:

- **Its picture** (`FUN_0029d420`, `loading::LoadingCards::back`) by the
  game's mode (`rec + 0x78`; `loading::Mode`, set as the game loads):
  - Pick Up (2, 3): `FUN_0029a5c0` marks the Street Tips cards owned
    (`XDBCARD` category 5 through `FUN_001d3200(.., k)` and
    `FUN_001d26e8`, k under 20; bits are only ever added) and
    `FUN_0029a980` picks. State 0 shows the first owned card not shown, a
    pair of them before a tip (`+0x14` turns over at each card); none
    owned, or every one shown (which starts the round over), goes to state
    1: the first of tips 4..7 not shown (`+0x8`'s high four bits; the
    to-game tips keep the low four). A tip is picture 0x1a + tip (30..33:
    Be the Oop, Dunk 2 Layup, Nerve Damage, Trick Counter) with its name
    (element 2 at (321, 63), `whatsnew.txt` 6, 7, 12, 11 by `0x3fa4f0`)
    and one line, element 28 in font 2 (`FUN_0029b828` cases 4..7:
    `whatsnew.txt` 15, the Give 'n Go lesson's, for 4; 13 "Check It Out /
    In Street School" for 5 and 7; 14, the Advanced Dunk lesson's, 10
    further left for 6). A card is the Street Tips page (`FUN_0029b720`):
    its `+0x140` 0 makes the picture `XAFEBG` backdrop 0x1a
    (`FUN_00235758`), element 2 says "Street Tips" (`load.txt` 2) at (321,
    73) and element 27, the kind-11 box at (318, 198) 505 wide, holds
    `streetTips.txt` card + 1 (the ids at `0x3fa4b8`, 0x3901..0x3914) in
    font 3 and the box's gold 0xfffdb813, each line centred and the lines
    on the box's middle. It draws once from the shared generator
    (`FUN_003a16b8`, its `% 12` used by nothing): `Session::draw`.
  - Be A Legend (5): `FUN_0029a670`. With the career's progress
    (`FUN_001e73d8`) under 500 and Rucker Park '78's challenge not won
    (`FUN_001e8140`), the cover its progress reaches: 0, 1, 2, 3 from 0,
    9, 17, 45 points, 6..11 from 130, 160, 200, 300, 400 and 450
    (pictures 0x23 + cover). A won tournament's cover (5 once event 3 is
    cleared, else 4 for event 2) stands in for the first four, and for a
    later one when that showed last (`+0x1b`, set only by a cover past 3).
    After the career's end, every cover in turn (`+0x10`).
  - Everything else (the attract, the School, the League Challenge): the logo
    (34, `FUN_0029a668`).
- **Its time and its presses**, measured in the project's PCSX2 runtime
  (PINE on the Front End task, `*0x3fe910` `+0x3c`, and the 100 Hz timer
  `0x4b32e4`) after Pick Up games won and quit to Choose Team (six runs:
  every tip and a Street Tips page), Be A Legend's results (three), the
  School's pause Quit (one) and the attract's Start (two). Every time the
  screen came 0.86 to 0.87 s after the game's end (the game's last frame
  stays up), its load (`FUN_0029b340`, one call while a callback draws the
  screen) began 0.46 s later (0.19 s for the Street Tips backdrop) and
  took 327 to 329 hundredths (`+0xf4`, eleven loads read); at its end
  `FUN_0029d538` hides Continue and `FUN_0029d5a8` opens the next screen
  at once (Update Team's question 0x60, the title 99), the picture
  standing about 0.3 s more while that one streams in. Cross pressed every
  0.3 s through a load changed nothing (3.79 s from the screen to the
  next, as without). The rewrite shows the picture from the game's end for
  3.27 s (`RETURN_SECONDS`, the shortest load), takes no press, then opens
  the page the game left the menus on (`Menus::set_return`).
- **The ring** lights its first star at once and one more each 2.2 s
  (`FUN_0029b228`: its next time is 0 as made), and all twelve at the
  load's end (`FUN_0029b520(.., 0xc)`). The rewrite's to-game screen had
  lit none for the first 2.2 s and gone on lighting them through
  Continue; both screens now light them so.
- **After it**: when a Pick Up game was finished (`DAT_00400054` bit 2,
  `FUN_002f3918`) or its options changed (bit 4), and on every School
  exit, `FUN_0029d5a8` runs the memory card's save of all six sections
  (`FUN_002d8460(.., 0x3f)`, the card manager `FUN_002294e0`,
  `FUN_00227160`) over the dimmed screen before the next: with the
  runtime's unformatted cards "Checking for memory card..." for about
  1.9 s, then "Save the original game data ...?" YES / NO. Built since
  (2026-10-20, "The memory card's messages", below): the rewrite runs the
  card's save there, over the load back's page dimmed.

Compared at the best shift as before (the grabs in
`street-return-loading-orig`, the rewrite's shots in
`street-return-loading`): the cover 14.4 (`return_cover.png`, cover 0
after the first won story game), the logo 12.0, tips 4..7 14.1, 14.3,
13.8 and 13.6, the Street Tips page 11.9 (card 2, owned through PINE).
640x480 and 1280x720: no overlap.

Other screens the rewrite skips or adds, found on the way: the boot shows
a loading screen of its own before the front end exists (`FUN_002f57f8`:
`xldscrn.bin`, `xldfrscn.bin` in French, a 320x240 picture doubled with
its bar filling as the boot loads, then the memory card's check;
`DAT_00403af4`, cleared by `FUN_002f4f80`); and a Pick Up rematch goes
straight back into the game (its rules, then the court's intro) where the
rewrite showed the to-game load. Both built since (2026-10-20, below).
Street School has no load between lessons (one game, the lesson menu in
it) and replays none.

## The memory card's messages, the boot and the rematch (2026-10-20)

Branch `card-messages` (not merged). Vj did not accept the card's dialogs
standing as the rewrite's file notices (2026-10-20): the rewrite keeps its
game data in a file, but shows the memory card's own messages and dialogs
as the original does. Measured in the project's PCSX2 runtime with its own
card files only (the runtime's unformatted pair backed up first and put
back after): unformatted; formatted with the game's data (made by the
game's own Format and Save); formatted without it (the same card with the
game's directory entry renamed, its page's ECC recomputed); PINE on the
card widget's dialog (`+0xc090`, its id at `+0xee`) against the 100 Hz
timer `0x4b32e4`, PrintWindow frames each 0.1 s and grabs at each dialog.
Save states keep the card's state as it was when they were made (`+0x4c4`
of the card system `*0x416ac0` read "unformatted" after loading state 5
over a formatted card), so every card run starts from a fresh boot.

**The widget.** The card widget 0x3c8c (`FUN_002294e0` over
`FUN_00226ae0`) runs every card work: its entry points are `+0x16c` save
(`FUN_0022c9d0`), `+0x174` load (`FUN_0022cbd8`), `+0x17c` the deletes'
lists (`FUN_00228418`), `+0x180` a delete op (`FUN_0022ccb8`, no caller
in the front end); its dialogs are `FUN_002298d0`'s states, their answers
`FUN_0022c150`'s, the card's results `FUN_0022b360`'s and its steps
`FUN_0022a9b8`'s. Op bits (`FUN_00227160`, `FUN_002271c0`): 0..5 the
sections, 6 `+0xd4` the boot's load, 8 `+0xdc` Game Data's (it clears
`+0xb4`, Options' Autosave, the settings' bit 3). The texts are
`memcard_ps2.txt` (ids 0x4900 on): the message (element 2) and, while
the card is busy (Checking, Saving, Loading, Deleting, Formatting), the
caution "Do not remove ... Do not turn the power off or reset the
console." (0x4900, element 3); the title argument (0x490c ...) is passed
but never drawn. The slot is "1" (`FUN_00339e78`, no multitap).

**The flows**, each step's time in seconds as measured (a message alone
stands for its timer of 2, `FUN_002da198`: 1.99 to 2.03 over fourteen):

| Work | The game's data on the card (the file) | Formatted, no data (no file) | Unformatted |
|---|---|---|---|
| Boot (`FUN_0029dc18`, `+0x174` 0x7f) | Checking 2.08, Loading 3.10, "Load successful." | Checking 2.09, "No game data found ..." | Checking 2.03, "... is not formatted." |
| Game Data Save (0x13f) | "Save ...?" (YES lit), Checking 3.42..3.44, "Overwrite existing ...?" (NO lit), Saving 3.12 (3.19 once), "Save successful." | "Save ...?", Checking 2.09, "Create a new file ... File size: 103KB (7,897KB available)" (NO lit), Saving 4.42, "Save successful." | "Save ...?", Checking 2.02, "... Would you like to format ...?" (Format / Cancel, Cancel lit), Formatting 3.78, Saving 4.0, "Save successful." |
| Game Data Load | Checking 2.09, "Load ...? All unsaved data will be overwritten." (NO lit), Loading 3.12..3.13, "Load successful." | Checking 2.07, "No ... data found" | (static) Checking, "... is not formatted." |
| Op 0x3f, Autosave on | Checking 2.08, Saving 4.41, "Save successful." | Checking 2.08, "Save ...?" again (state 15, YES lit), then the file's question | Checking ~1.9, "Save ...?" (state 15), then Format |
| Op 0x3f, Autosave off | as Game Data's Save | as Game Data's Save | as Game Data's Save |

NO to "Save ...?" ends the work; NO to the overwrite, the new file or
the format asks "Save ...?" again (state 15), whose YES goes straight to
that question again. A failed write asks "Save failed. Retry?" (state 22,
NO lit; Retry checks again with the questions), a failed read "Load
failed. Retry?" (23; either answer puts the game data back to its
defaults, `FUN_00227208`). The work over, the screen that asked goes on
(`FUN_002d7b00` calls its `+0x14c` once the widget's `+0xb0` is set).

**The file as the card** (`card.rs`): the file there reads as a
formatted card holding the game's data; no file as a formatted card
without it, with a fresh card's room (8,000 KB: the runtime's card had
7,897 left beside a 103 KB save); a file that does not read back as data
that fails to load. The file can always be written, so the card is never
missing, unformatted or full: "No memory card ... found" (32, 33, 38),
"is not formatted" and the format question (34, 41), the room's messages
(36, 37, 53, 56, 57) and "cannot be saved because a memory card is not
inserted" (55) have no state of the file to stand for, and do not show.
The file is written at Saving's end and read back at Loading's; between
the save points a change stays in memory (`story::Save::store`), as the
original's stays in its game data in RAM (`0x4bbf10`). Options' Autosave
picks the op 0x3f path as the original's `+0xb4` does.

**The box** (`card::box_pieces`, `FUN_00259380`, `FUN_0025a400`,
`FUN_00259e38`): the words' widest line and 100, their height, 30 for each
row and 100, at least 4:3 (a taller box is 1.33 of its height wide), its
middle at (320, 240); the message 35 under its top, the caution 25 under
the message, NO (or OK) 65 and YES 95 over its bottom, all in font 2,
white, centred; the rows lit gold with stars as the front end's. It keeps
0.39 of the light behind (0.390 to 0.393 measured on Game Data and the
load back), and stands on black at the boot.

**Where the card saves** (`FUN_002d8460(.., 0x3f)`, twenty-one callers),
each now through the card's save over the screen that asked, the next
screen after it: Options left with changed settings (`FUN_002a4d18`,
`FUN_002a53b0`, `FUN_002a5870`; also before Trax and Game Data); the publisher
Trax left with changed songs (`FUN_00265978`
case 0x18); Controller Layout's Exit (`FUN_00273388` case 0x22); Game
Data left after a delete (`FUN_00299110`); Create Baller's CREATE
(`FUN_002745d8`); "Is This Your Baller?" (`FUN_002796a0`, Be A Legend's
"Save to memory card?" over Choose Baller); the crew's Create Team
(`FUN_002808c0` mode 1; My Team's saves after the card widget's own
question, `FUN_00227348` case 5); Edit Baller and Edit
Moves left with a changed baller (`FUN_0028c3f0`, `DAT_003f93d0`);
Rewards left after a purchase (`FUN_002c1370`, `+0x2b0`); Update Team's
accepted change (`FUN_002d7238`); the completion boards' Cross
(`FUN_002621a0`, `FUN_002a4898`); a decided tournament's bracket left
without Update Team (`FUN_002cd0f8`, `FUN_00288290`, `FUN_00266e70`);
and the load back after a game (`FUN_0029d5a8`): every School exit, and a
Pick Up, League Challenge or Be A Legend game that was finished
(`DAT_00400054` bit 2, `FUN_002f3918`) or whose options changed in it
(bit 4, `FUN_002f2e50` against the start's copy, `FUN_002f2d58`), unless
Update Team, a completion board or a decided bracket follows (each saves
itself). The class 0x4d "join a team" screen's save (`FUN_00271dc0`)
belongs to a screen the rewrite does not build (below).

**The boot's loading screen** (`boot.rs`, `Screen::Boot` before the
movies): while the boot loads, the game's own frame is held back
(`DAT_00403af4`, `FUN_003129c8`) and the load callback `FUN_002f4d00`
draws `xldscrn.bin` (`FUN_002f57f8`) over the PS2's whole 640x448 frame
(the layouts' rows 16..464) and the bar, a gold `FUN_002332d0` widget
(0xfffdb813) 7 high at (226, 380) of the frame ((226, 396) here), 0.14
wider each call (`DAT_004005d0`) up to 0.95 of 146 (`DAT_004005cc`), the
whole 146 once `FUN_0029dca8` sets `DAT_004005d4` between the front end's
last two packages; `FUN_002f4f80` ends it and the boot's screen (class 4)
loads the card. Three fresh boots: the flag set at 103 or 104
hundredths, the picture 0.47 s later, the bar empty 2.44 s, 1.12 in two
calls, standing while the IOP's modules load (`FUN_002f5578`'s 35 more is
lost: the bar widget is made anew each call), 8.17 a second to 69.58,
whole at 12.57 s, the flag cleared at 1484 to 1492, the card's first
dialog 13.53 to 13.68 s after the picture. The rewrite shows the picture
(the HD pack's when one is set) and the bar along that fill for 13.62 s,
takes no press, then runs the card's boot load over black and goes on to
the movies. `xldfrscn.bin` is the French console's (`FUN_00342088` = 2);
the rewrite runs in English.

**The rematch** (`game.rs`, `Rematch`): the pause's Quit Game REMATCH
(`FUN_002bd3f8`) and the post-game dialog's build the new game in the
game's task with no load; the rewrite rebuilds its match with no picture,
wait or Continue (the new game stands 0.1 s after the press in a
scripted quick game). The rematch rules the original shows first
(`FUN_0025dc88`, 0x5df: Play To, Spot Points per side, Continue,
`post_4.png`) are drawn since the transitions' pass, below.

**Found and fixed on the way**: a won tournament's final standings come
before Update Team, whose bracket Cross opens it (the final's winner 0x25
to 0x60), where the rewrite opened Update Team first; Game Data's delete
questions open on NO and "Delete successful." is a message alone.

Compared at the best shift as before (originals in
`street-card-orig`, the rewrite's in `street-card`, same names): the save
question 9.8, Checking 12.2, the overwrite 10.2, Saving 11.1, "Save
successful." 8.3 (under the emulator's own notice), the load question
8.5, Loading 10.4, "Load successful." 6.4, no data 7.6, the new file 10.5
(its "7,897KB" against the rewrite's "8,000KB"); Options' save: Checking
11.8, Saving 10.7, "Save successful." 9.8; the load back's: Checking
11.9, the question 10.0; the boot: the picture 11.1 to 11.3 at 1, 6 and
12.8 s (its colours about 6 levels darker in every grab, as every loading
picture's), the bar within a pixel, Checking 10.1, Loading 9.1, "Load
successful." 3.5, no data 5.9. The matched Difficulty screen scores 10.9.

## The front end's transitions, join a team and the rematch rules (2026-10-20)

Branch `fe-transitions` (not merged). Vj ruled (2026-10-20) that screens
the player sees are shown as the original shows them. Measured in the
project's PCSX2 runtime with its own card files (backed up and put back,
hashes equal): PINE on the front end `*0x3fe910` (screen `+0x3c`, request
`+0x40`, transition `+0x4c`), the screen's state `+0x6c`, the 100 Hz timer
`0x4b32e4`, PrintWindow frames every 42 ms; grabs in
`street-transitions-orig`, the rewrite's in `street-transitions`.

**Transitions** (`FUN_002ddb40`; `crate::transition`). A change runs in
four steps: the old screen leaves (state 1 forward, `FUN_002d7cb0`, or 2
back, `FUN_002d7d40`; its widgets' records: on the main backdrop's
screens the title slides out left at 2.4 px/ms over 333 ms, the rest
fade over 167, `0x3fdf58`..`0x3fe0f8`; deadline 266.67 ms there, 0
elsewhere, `FUN_002d8398`); the front end builds the new screen
(`FUN_002dcff8`), hides it and starts the movie, whose first frame shows
about 0.2 s later; the movie plays at 29.97 a second, full screen at (0,
-16) of the 448-line frame (all 640x480 of the layouts; frames placed so
score 7.2 to 8.2 against 9.3 to 14.9 shifted 16), no screen updating
(`FUN_002dd338`), presses queued (2026-10-20: dropped, the front end
empties its pad events every update while a movie plays, `0x2de7b8`;
"The movies", below); the new screen arrives (state 0, its
records, deadline by screen), then state 4 takes the queued presses. A
press while the old screen leaves is dropped. The movie's start plays
`FE_EVT` event 2 (`FUN_001a19b8`): the blurs `k` 0 three updates late,
`XMT0000`/`07`/`08`/`11`/`20` `k` 2 at once, `01`..`06`, `09`, `10`,
`12`..`14`, `19` `k` 1 three updates late.

Which movie: forward, the new screen's slot `+0x154` (empty for none;
the base `FUN_002d8f68` gives `xmtfblur`); back, `xmtblur` when its slot
`+0x174` allows (base `FUN_002d8f60` yes). By the new screen's factory
class, the old one's reported class (`+0x40`, `+0xb6`):

| To (factory class) | Forward movie |
|---|---|
| Main 3 | `xmt0000`; from Create Team (0x35) `xmtfblur`; from join a team (0x4d) `xmt0013` |
| Play It 5, My Team 6, Inside 0x66..0x68, Difficulty 0x69, the league board 0x3b, Create Baller 0x4a..0x4d, the title 0x63, 7/8, the boot 4 | none |
| User Setup 0xc..0x13 | `xmt0001` |
| Choose Team 0x1b/0x1c | `xmt0002` |
| league Choose Team 0x19 | `xmt0019` |
| Choose Players 0x1d | `xmt0003` (Be A Legend's `xmtfblur`) |
| Choose Court 0x21/0x57 | from Choose Players (0x1d, 0x2b) `xmt0004`, from Choose Team (0x1a) `xmtblur`, else none |
| Choose Region 0x29 | from User Setup (0xb) `xmtfblur`, else `xmt0020` |
| league Choose Players 0x2b | `xmt0010` |
| The loads 0x2c/0x2d/0x2f | from Choose Court `xmt0008`, the map (0x27) `xmt0006`, 0x2b `xmt0011`, 0x1d `xmtfblur`, else none |
| The map 0x27 | `xmt0005` |
| Create Team 0x35/0x62 | from My Team `xmt0007`, else `xmtfblur` |
| Rewards 0x14 | `xmtblur3` |
| the rest | `xmtfblur` |

Back, `xmtblur`, but none between the main backdrop's screens: Main from
Play It, My Team or Inside, Play It from Difficulty, Inside's pages from
each other, Difficulty from Play It.

Measured, the press (state 1) to the new screen's state 4: the title to
Main 3.11 s (`xmt0000`, 77 frames); Main to Options 0.93 (`xmtfblur`, 9);
Options to Main 1.51 (`xmtblur`, 30); Main to Play It 0.60 (none); Play It
to Main 0.47 (0.26 leaving, 0.04, 0.17 arriving); Play It to Difficulty
0.59; Difficulty to User Setup 1.89 (`xmt0001`, 36); User Setup to
Choose Team 1.48 (`xmt0002`, 28); Choose Team to Choose Players 1.10
(`xmt0003`, 17); Choose Players to Choose Court 1.39 (`xmt0004`, 25);
Main to High Scores (0.50 movie, 0.03 arriving) and back (1.18 movie,
0.29); Choose Court to the load 0.38 + 1.9 (`xmt0008`, 52). Each is the
leaving deadline + a hold (0.22 s with a movie, 0.033 without; Options
+0.09, the loads +0.38) + frames / 29.97 + the arriving deadline; the
rewrite's model is within 70 ms of every one (test
`the_steps_add_up_to_the_runtimes_times`). The movies are silent,
640x480, from `/MOVIES/`. The load back to the menus (class 0x2f) plays
`xmtblur` into Choose Team. Scores (best shift): Main held under the
movie 9.9, Main settled after the title's 9.0, Main arrived 11.8,
Options arrived 12.9. 1280x720: the movie fills the same pillarboxed 4:3
area. Scripted runs (`--keys`, `--at`) count on a clock that stops while
a transition runs, so old scripts reach the same pages; `--at-real` and
`--at-transition N` catch a transition.

**Join a team** (class 0x4d, layout 77, `crate::join_view`): reached in
the runtime from a fresh card (Create Team with a created baller, the league
Challenge on that team, its won state written through PINE). The league
board's Cross goes forward to 0x4d when the challenge's team holds a
created baller; "Congratulations! ..." (`wdtm.txt` 20) over Continue on
arrival; the baller's name, "Joins:" and the focused team's five on the
left, the league's 29 on Choose Team's right panel; Cross asks "Are you sure
you would like ... to join ...?" (`common_fe.txt` 20) or the 6th-man
replacement (21), No lit; Yes puts the baller on the team as its sixth
(`FUN_001e0030`, game data `+0x3c`, 29 shorts; the save's `SIXM`
section; an original save's sixth men import) and the next baller gets
its turn; after the last the card's save and Main. Scores: the message
9.8, the screen 13.6, a step down 13.5, the question 6.8, the 6th-man
replacement 9.7, the board before it 14.4.

**The rematch rules** (`FUN_0025dc88`, dialog 0x5df,
`crate::rematch_rules`): over the court after REMATCH (the pause's Quit
Game and the post-game dialog): Play To, Spot Points with each side's
names and spot, Continue lit; up and down wrap, left and right step the
record or move the spot (trick points by 25,000 under 100,000 here,
`FUN_00260368`), Cross on Continue starts the game again. My Rules'
shrinking step from a million was 100,000 where the original takes
50,000 (`FUN_002cb620`): fixed. Score: the panel 13.7 (the court under
it is another game's).

**Found and fixed on the way:**

- *The title screen* (class 0x63, `FUN_002d38b0`): backdrop 27 (was 17),
  "Press START button" (`titlescreen_ps2.txt` 0, font 2, pulsing, between
  row stars) and the copyright (1, grey). Score 11.0.
- *Main opens on Play It* whichever way it is reached (its build focuses
  its list's first row), and My Team on Create A Baller (seen after
  Create Team's save, 11.6).
- *Both completion boards go back to Main* after their save.
- *My Team's Create Team asks the card widget's question*
  (`FUN_002808c0`, `FUN_002272e8`): Choose Logo's answer (0x3ef) sets
  `+0x1d8`, and the card widget shows "Create team\nNEW YORK BALLERS?"
  (`delete.txt` 10, dialog 0xcd, YES lit) in its own box. YES
  (`FUN_00227348` case 5) makes the team (over one of its name after
  "Overwrite existing created team ...?", 0xce, NO lit; with the ten slots
  used "Only 10 teams can be created. ..." over Continue, 0xd4), then the
  card's save (0x3f) and back to My Team (`FUN_002d7d40`). Measured with
  Autosave on and the game's data on the card: Checking 2.07 s, Saving
  4.42, "Save successful." 2.02, My Team. NO closes it on the grid with
  the fifth pick taken back and the help line left empty (nothing sets it
  until a board is cancelled, `FUN_00280f98`). The rewrite asked "Do you
  want to save your team?" and went to Main unsaved. Scores: the question
  7.9, My Team after 11.6, the grid after NO 14.7 (another player
  focused). A save declined or failed ends as NO does (the widget's
  `+0xb0` clear): read from the code, not measured.

## The idle attract demo (2026-10-20)

Branch `idle-attract` (not merged). Vj ruled (2026-10-20) that the demo
is built as the original shows it. Measured in the project's PCSX2
runtime (its own card files, unchanged; five fresh boots, `street-idle-orig`
runs 1b to 5): PINE on the front end `*0x3fe910` (screen `+0x3c`, its
class `+0x40`, state `+0x6c`, the title's `+0xf0`/`+0xf8`/`+0xfa`, Main's
`+0xf0`/`+0xf4`/`+0xf6`, the front end's `+0x6c`/`+0x70`), the turns
`DAT_003fd704`, the front end's timers (`0x4d6450 + 0xa24`), the game
data `0x4bbf10` (mode `+0x78`, teams `+0x8c`/`+0x90`, the six `+0x9c..`,
court `+0xcc`, options `+0x114`/`+0x12c`, last court `+0x1c08`, kept level
`+0x1c0a`), the game `*0x3d30bc`, the shared generator `0x4309f0`, the
audio controller `*0x3e9838` and the front end's music `*0x3ea834`;
PrintWindow frames every 0.25 to 0.5 s; the 100 Hz timer `0x4b32e4`.
The rewrite's `crate::attract`.

**The idle timer.** The title and Main arm a front-end timer of
`demo_delay` seconds as they stand (their slot `+0x16c`, which
`FUN_002dcff8`/`FUN_002dd238` call as the transition into them ends:
`FUN_002d3848`, `FUN_0029e0d0`; `FUN_002da198`). `demo_delay` is 90 in
the executable (`FUN_0029ed60`, `DAT_003fa5f0`) but the disc's
`/CSCRIPTS/AUTOEXEC.CFG`, run at the boot, sets `demo_delay 120`: the
timer read 12,000 hundredths in every run, and fired 120.00 to 120.05 s
after it was armed. A change of screen drops every front-end timer
(`FUN_002dd1b0`, `FUN_002da0e0`), so each screen arms afresh as it
arrives: the first title at the end of the boot's movies (count 8,051 to
8,067), the title after the demo at the end of its `xmtblur`, Main 2.76 s
after Start on the title (its `xmt0000`). Every message Main takes
restarts its timer (`FUN_0029e700`, `FUN_002da258`): Down and Up each
re-armed it 0.15 s after the press, and it fired 120 s after the last.
The title's handler (`FUN_002d3cf0`) does not: Cross, Down and Select
while it stood left its timer as it was. A covered screen pauses its
timers (`FUN_002d7ad8`/`FUN_002d7b00` on `FUN_002da128`/`FUN_002da160`):
the rewrite pauses Main's while Square holds the Help overlay up, or the
card's dialogs are over it.

**"Loading Demo...".** At the timer's end (`FUN_002d3d68`,
`FUN_0029e8f8`) the screen's `+0xf0` is set and a 4 s timer starts. The
title's prompt widget (0xe) says "Loading Demo..." (`main_help.txt` 6,
pulsing as the prompt) with its stars (0x14, 0x15) hidden; Main's help
line says it and its list goes unlit (`FUN_0029e980`). Main takes no
press meanwhile (Cross did nothing; the turn came on time). The title
still takes Start (`FUN_002d3cf0` does not read `+0xf0`): Start 1.5 s
into its "Loading Demo..." went to Main, and the turn was not taken (the
timers went with the title). The rewrite's title now takes Start alone,
as the original's does (Cross is eaten there, 0x23).

**The turns.** At the 4 s (`FUN_002d3ed8` on the title, `FUN_0029ec08` on
Main) the front end goes by turns (`DAT_003fd704`, 0 at the boot, flipped
by each turn taken, on either screen) to the attract's game or to the
title again.

- *The attract's game* (`FUN_001dba68(.., 0)`, `FUN_001db7f8`): the court
  `rand() % 10` (the shared generator) drawn again until it is open
  (`FUN_001d37b0`) and not the last attract's (`+0x1c08`, none at the
  boot); side 0's team `rand() % 28`, set (`FUN_001dbb20`: for a side no
  person holds, `FUN_001dbe30` reseeds the generator from the timer and
  draws the side's three in Pick Up's mode); side 1's `rand() % 28` from
  the reseeded generator, drawn again while it is side 0's. Read in the
  runtime and replayed exactly: the first demo of a boot, from the
  generator as the boot left it, rolled court 2 (closed on a fresh save),
  then 0, team 8, and at the reseed's count 20,468 team 9; the second
  rolled court 1 (not 0) and team 3, and at 123,324 team 25; each side its
  roster's first three. The options' level goes to Legendary
  (`FUN_001d74a8(.., 2, 0)`, the player's kept at `+0x1c0a` until Main's
  build puts it back, `FUN_001de450`); the rules are those Main's build
  leaves (`FUN_001ded20`: to 21, by 2). The screen leaves forward to the
  load (class 0x2c, no movie: the title 0.47 s, Main's title sliding
  off), which shows the logo and "Loading Demo" (`load.txt` 6) in its
  line's place at (292, 375) (`FUN_0029c490`, mode 0) and takes no press
  (Start there was not the game's). It stood 7.2, 11.9 and 13 s in three
  runs (the disc's reads; the game object 5.5 to 5.9 s in), the rewrite's
  as long as its other loads. The game opens as any game, the court's
  intro and an away player's, then the tip; "Press START to exit demo"
  (`overlays.txt` 44, pulsing) stands from the tip, through the replays,
  not during the intro. Start on any pad ends it (`FUN_00141f18`: game
  `+0xf0` -4, the task's exit two updates on, `FUN_00143180`): the game
  went 0.22 s after the press, its last frame standing, and the load back
  showed 0.97 s after it; Cross and Triangle did nothing. Untouched, it
  plays to its end (run 1b's: 23 to 21 after 37,028 updates, 10.3 min),
  shows the won basket's replay with the line, holds its last frame and
  goes with no results (`FUN_002f3918`: mode 0 exits the task), the load
  back 1.3 s after the replay (within half a second). The load back
  (class 0x2f, the logo, "Loading", 3.71 s) goes back to the title
  (`FUN_0029d920`, mode 0) through `xmtblur` (1.5 s).
- *The title again*: the front end's song is stopped at once
  (`FUN_0019fd60(.., -5, 0)`: `FUN_001a8da0` ends its stream; the music
  manager's `[5]` 1 to 0) and the screen goes forward to a new title.
  Its transition plays the title's own movies, which `FUN_002ddb40` names
  itself for class 99 once a title has stood (the front end's `+0x6c`,
  set by the first title's transition at `0x2de07c`; the title's slot
  `FUN_002d41d0` names nothing): `movies/xmt0018.nfx` (the intro, 1,318
  frames, with its soundtrack, the front end's song stopped for it;
  `xmtpl18` on a PAL console) and, queued
  after it by `FUN_002e1618`, `movies/xmt0015.nfx` (31 frames, silent: the
  zoom onto the billboard; `xmtjp15` in Japan). The new title armed 45.59
  s after the turn on the title and 45.89 s after it from Main (Main
  leaving first); the front end's next song started as the intro ended,
  1.29 s before. Cross during the intro did nothing; Start ended it (0.09
  s after the press), the song started then, the zoom played whole and
  the title stood 1.41 s after the press, the press going no further.

The rewrite (`attract.rs`): `Idle` (the screen's timer and its pause, the
4 s, the turns, the last court) stepped each front-end update before the
screens' keys (`attract::idle` in the menus' chain; `navigate` waits
while Main loads the demo); `demo_delay` read from the disc's
`AUTOEXEC.CFG` (`--demo-delay` sets it for a run); `picks` and `set_up`
for the game (the session's generator and its reseeds through
`menu::computer_three`; the menus' setup kept in `Before` and put back
when the front end is back); the title's "Loading Demo..." (`Style::Title
{ demo }`), Main's help and unlit list; the load's line
(`LoadingScreen::demo`); the game's Start and end (`game::pause`,
`game::results`: its last frame held `EXIT_SECONDS`, 0.97 s, then the
load back; no boards); the demo line from the tip (`hud`); the title's
turn through `transition` (its forward movie `xmt0018.nfx`, the follow-on
`xmt0015.nfx` after the same hold, the intro's soundtrack, Start ending
the intro, `TransitionSound`), the menus' song held off meanwhile
(`audio::MenuSongCue`). The command line's `--demo` keeps the rewrite's
pause and boards. Tests: `attract::tests` (the timer on both screens, the
pause, the turns across screens, a change of screen, the runtime's two
draws, the end's hold) and `transition::tests::
the_title_comes_again_with_the_intro_and_the_zoom` (the model within 110
ms of both measured turns).

Compared (`street-idle-orig` run 5's grabs, the rewrite's in
`street-idle`, the mean difference at the best shift as before; the
Difficulty screen, matched, scores 10.9): the title standing 11.0 (`o_title`), its "Loading Demo..." 10.7, the demo's load 12.1, the load back after it 11.9, the title after the demo 10.9 and after the intro 10.9, Main 11.7 and its "Loading Demo..." 11.4; the intro's frame ten seconds in 13.7 against the movie's own frame 300 (the rewrite plays the disc's frames full frame, as the original); the demo's line in play the original's font and place within 3 pixels (the games under it differ). 640x480 and 1280x720:
no overlap.

**Found on the way, not built (for Vj's ruling, each a mismatched
parity case; all three built 2026-10-20, "The movies", below):**

- *The boot's movies* (`presentation.ui_boot_movies`). In the runtime
  they play as the first title's transition (`FUN_002ddb40` with the
  front end's `+0x6c` 0, `0x2dfea8`): `xmt0017` (the publisher), `xmt0016`
  (the league's notice and logos), `xmt0018` (the intro), then `xmt0015` (the
  zoom onto the billboard), from the title's class showing (18.5 s into
  the boot) to its arming (80 s). NuFX (`xnufx`) is not among them (the
  credits play it, `FUN_00283b40`); Cross and Down during them did not cut
  them short. The rewrite plays `XMT0017`, `XNUFX`, `XMT0016` and
  `XMT0018` on its intro screen, any button skipping each, with no zoom.
  Whether Start skips the boot's (it ends the title's intro, above) is
  not measured.
- *The league board's trophy* (`presentation.ui_league_board_trophy_loop`).
  The board's left panel shows `XLP0006` turning (160 by 208, 119 frames;
  the previous pass's grabs, `street-transitions-orig` join14, show it at
  different turns); the rewrite dropped the loop (above, "The completion
  boards": no call to it was traced).
- *Inside Vol. 2's DVD movies* (`presentation.ui_inside_dvd_movies`).
  Measured on the Vol. 2 Trailer (run 5): Cross on its row and Inside
  leaves (the front end's song stopped), and the trailer plays as the
  transition from Inside into Inside (0x66 to 0x66, `FUN_00289730`'s own
  name), full frame (9.1 against its decoded frame 200), with its soundtrack;
  Cross during it did nothing, Start ended it 0.08 s after the
  press; a new Inside arrives on its first row (What's New In 2 lit) and
  the song comes back. The rewrite plays it on its intro screen, any
  button skipping, and comes back to Inside on the trailer's row.

## The movies: the boot's, the DVD pages', the credits and the screens' loops (2026-10-20)

Branch `movies` (not merged). Vj ruled (2026-10-20) that the boot's
movies, the league board's trophy and Inside Vol. 2's movies play as the
original plays them, and that what the player sees or hears is made to
match. Measured in the project's PCSX2 runtime (its own card files,
unchanged) over four fresh boots and runs driven on from them
(`street-movies-orig`, INDEX.md): PINE on the front end `*0x3fe910`
(screen `+0x3c`, its class `+0x40` and state `+0x6c`, the transition
`+0x4c`, its movie widget `+4` and that widget's movie task `+0xb0`: the
index `+0x199`, the count `+0x198`, done `+0x19c`, the file
`+0x178[index]`; the loop widget `+0x58` and its task's file), the front
end's music `*0x3ea834`, the 100 Hz timer; PrintWindow frames every 0.25
s and grabs at events. The rewrite's in `street-movies`.

**The movie task and its Start check.** A transition's movies are one
movie task's queue (`FUN_0022dfc8`, `FUN_0022dce8`, five at most), each
opened as the one before ends (`FUN_0022d478`, `FUN_0022d958`); a movie
shows its first frame about 0.22 s after it opens (the grabbed frames:
the publisher's 29 at 1.21 s, the intro's 295 at 10.03 s, the zoom's 8 at
0.50 s) and its task opened the next 0.137, 0.127 and 0.083 s after its
frames. While the movies play, every update `FUN_002ddb40` first lets the
new screen's Start check look through the pad events (`0x2de71c`,
`FUN_002e14f8`: it pops them until event 0x3d, `START_PRESS`, dropping
the rest) and then empties them (`0x2de7b8`, `FUN_002e6ed0`), so a press
under a movie reaches no screen: Down 1.2 s into Main's `xmt0000` was
dropped (Main opened on Play It), and Start in the publisher was not kept
for the intro's check. Which screens check: the title, at the boot only
while the task's index is 2 (the intro; the front end's `+0x6c` clear)
and on its later turns until a Start was taken (`+0x70`, set as its
movies start, `0x2e10c4`, cleared by the Start, `0x2de7ac`); the DVD
pages (0x66..0x68) always; none other. On the title a Start makes a new
movie widget with the zoom alone (`FUN_002e1618`); on a DVD page a
finished one, so the change ends. The rewrite's
`transition::queue_presses` drops presses under the movies (the
transitions' pass had them wait: its press landed as a 9-frame
`xmtfblur` ended) and keeps those while the new screen arrives.

**The boot's movies** (`presentation.ui_boot_movies`). The card's last
message gone, the boot's screen (class 4) asks for the title, whose
transition (the front end's `+0x6c` clear, `0x2dfea8`) queues
`movies/xmt0017.nfx` (the publisher, 75 frames, its sound at the movie
player's own level), `xmt0016.nfx` (the league's notice and the teams'
logos, 399, silent), `xmt0018.nfx` (the intro, 1,318, at the music's
level) and `FUN_002e1618`'s `xmt0015.nfx` (the zoom onto the billboard,
31) (French consoles `xmtfr17`/`xmtfr16`, Japanese `xmtjp17`/`xmtjp16`,
PAL `xmtpl18`). NuFX's logo is not among them: the credits play it. The
title's class showed 9.9 to 10.1 s after the boot screen's (counts 1,898
to 1,913), its state 4 61.56 s later (8,069 untouched). Start in the publisher
SPORTS BIG or the league's notice did nothing; Start 8 s into the intro
opened the zoom 0.08 s later and the title stood 1.43 s after the press;
Start in the zoom did nothing. The front end's song started as the zoom
opened (`FUN_0022d958` on `xmt0015`: the music manager's `[5]` 0 to 1 at
7,939, the zoom opened at 7,927). The rewrite (`boot.rs`, `transition.rs`):
the card's check over, the boot begins the change from class 4 to the
title and opens the menus under it; the title's transition plays
`BOOT_REEL` while no title has stood (`Transition::titled`, set as the
title arrives) and `TITLE_REEL` after; its movies chained 0.127 s apart,
each from its first frame and over on its clock at its picture count
(`mpeg2::pictures`; the original's player ends a movie by its time
against the frame count, `FUN_00346d98`); the song held until the zoom
(`Transition::holds_song`). The intro screen, its playlist and the movies'
any-button skip are gone (`Screen::Intro`). Model: 61.56 s, within 20 ms;
the intro's first frame within 50 ms; Start to the title 1.46 s (1.43,
1.41 measured). The title's turns after the idle demo keep the intro and
the zoom (now 0.10 and 0.14 s short of the two measured turns).

**The DVD pages' movies** (`presentation.ui_inside_dvd_movies`). Cross on
a movie row (`FUN_00289f98`, `FUN_0028aee0`, `FUN_00289100`) stops the
front end's song (`FUN_0019fd60(.., -5, 0)`), sets the movie's name
(`DAT_003fdf44`; What's New's Play All "xmtall") and sends the page
forward into itself (`FUN_002d7cb0` on its own class); the page built
anew names it (`FUN_002ddb40`: the four `xmt0024..27` for "xmtall"), and
it plays full frame with its soundtrack. Measured on the trailer, Bobbito's
New York City and Vol. 2, Play All and Take On The League: Cross during one
did nothing; Start ended the change 0.08 to 0.09 s after the press, Play
All's four at once; the page stood 0.53 to 0.58 s after the end on its
first row (New York City lit after Vol. 2, Play All after Take On The
league); the city loop made anew; the song back at the end. The rewrite: the
rows set `Menus::dvd` and `menu::watch` begins the change into the same
class (`Change::dvd`), the cursor on row 0; Start ends it 90 ms on
(`DVD_STOP_MS`), the page arrives in 0.57 s (`settling_ms` 300 for a DVD
page after its own movie); the song held from the press to the end. The
movie screen and `play_movies` are gone.

**The screens' loops** (`presentation.ui_screen_loops`,
`ui_league_board_trophy_loop`). A screen's slot `+0x15c` names a loop and
its top-left corner, which `FUN_002ddb40` starts as the change ends
(stopping the one running unless the new screen answers "sameloop"). Read
from the screens' vtables and confirmed in the runtime's loop widget on
each screen: User Setup `xlp0004` at (464, 277) (the cars passing low on
the right), Choose Team `xlp0003` at (0, 0) and the League Challenge's
`xlp0011` there (the trees), Choose Players and the League Challenge's
`xlp0002` at (480, 1) (the right column's smoke and water tower), Choose
Court `xlp0001` at (19, 0), My Rules `xlp0010` at (17, 0), the league
board `xlp0006` at (100, 146) (the trophy turning in the left panel) and
Be A Legend's board `xlp0016` at (69, 141) (its trophy), beside the main
backdrop's city `xlp0005` and Rewards' `xlp0009`. `XLP0007` and
`XLP0008` are not played. Each but Be A Legend's repaints its backdrop
in place (frame against backdrop 2.3 to 7.7 at its place, worse a few
pixels off; Be A Legend's trophy turns over a dimmed copy of the panel);
the runtime's grabs put User Setup's, Choose Players' and both boards'
at their places within the grabs' own (1, 3) offset. The rewrite
(`menu::screen_loop` by class, the film room under the screen's widgets,
`transition::sameloop` and `FilmRoom::restart` as a change ends) played
only the city and Rewards'. An earlier note here put `XLP0004` on Edit
Moves: it is User Setup's.

**The credits** (`presentation.ui_credits`). Options' Circle
(`CIRCLE_PRESS` 0x2a; `FUN_002a4d18`) goes to the credits' screen (class
9, layout 9 over backdrop 31, `FUN_00283bd8`: "Credits", NuFX's and the publisher
SPORTS BIG's logos, "NuFX" on Cross and "THE PUBLISHER CANADA" on
Circle, Back, "Select and view game credits."; `xmtfblur`). Its keys
(`FUN_00283d08`): Cross to NuFX's roll (the screen built as class 8,
`+0x134`: `FUN_00283b40` names `xnufx.nfx`, 181 frames, and
`FUN_002ddb40` adds `xmtfblur` for a screen reporting class 7; Start and
Cross during the logo did nothing), Circle to the developer's (class 7:
`xmtfblur` alone), Triangle back to Options. A roll (`FUN_00281a08`,
`FUN_00281cb0`, `FUN_00282398`, `FUN_00282b08`; layout 7 over backdrop 8)
reads `/CREDITS/XCREDITS.BIN` (a `C0FB` archive of five UTF-16 texts: the publisher
Canada's in English, again for PAL consoles, NuFX's in English and
French, the developer's in French; a US console reads 0 and 2), a row a line,
its cells by tabs, `/T`, `/H`, `/L` the styles (team font 2 gold, heading
font 1 gold, name and part font 1 cream, plain font 1 cream; "Credits"
itself, `credits.txt` 0, font 5). Its picture, `XAFEBGEX` entry 3 (the
skyline, 512x256 shown 640 by 256 at (0, 224), over the rows), fades in
by 6 an update once streamed; "Credits" starts at y 530; each update the
lines rise 0.8 pixels, fade by `(min(y, 480 - y) - 50) / 50` and go when
their bottoms pass 50; the next row starts while the newest line's
bottom plus 0.6 of its height is above 430; cells at `640 k / (n + 1)`
across, an `L` row's 120 apart; with none left the roll goes back to
Options through `xmtblur` (NuFX's stood 55.44 s untouched; the model
55.43 s), and Cross or Triangle leave too. The rewrite had a page of its
own words there ("Streetball Revival: an unofficial rebuild ...");
it now has the screen and both rolls (`credits.rs`).

**Found on the way and not built** (`presentation.ui_select_dialogs`,
mismatched): Select (`SELECT_PRESS` 0x3e) opens a question on Main, Play
It, My Team, Difficulty, User Setup, Choose Players, the League Challenge's
Choose Team and the DVD pages (dialog 0x55: `FUN_0029f4b0`,
`FUN_002bf428`, `FUN_002a3fe0`, `FUN_00284848`, `FUN_00297c40`,
`FUN_00299110`, `FUN_0026a860`, `FUN_00273388`, `FUN_00289f98` and its
kin), on Options (0x85, `FUN_002a4d18`; Yes back to Main) and on the
credits' screen (1000, `FUN_00283d08`; Yes back to Main); the rewrite
gives the pad's Select to its own rows (Online, Watch A Game) and has
none of these questions. (Built on 21 screens: "Select's questions and
the rewrite's own rows", below; `FUN_0029f4b0` is the map's, not Main's,
and `FUN_00273388` Controller Layout's.)

Compared (the mean difference at the best shift; the matched Difficulty
screen 10.9): the publisher at 1.21 s 7.3 (frame 29 in both), the intro
at 26.10 s 13.8 (frame 293 against 295), the zoom at 60.65 s 10.1 (frame
7 against 8), the title after them 10.9; Bobbito's New York City and
Play All's first in the original's grabs 8.4 and 9.5 against their
decoded frames 83 and 78, which the rewrite plays full frame; Bobbito
after a Start 11.3 and What's New after Play All's 12.0 (each on its
first row); the league board 13.0 and Be A Legend's 13.1 (their trophies at
other turns); User Setup with its cars 15.5; the credits' screen 10.3,
NuFX's logo 9.1, NuFX's roll 12 s in 11.4 and the developer's 10 s in 15.1.
640x480 and 1280x720: no overlap; at 1280x720 the movies and the rolls
fill the pillarboxed 4:3 as the transitions' do, the boards' loops under
their panels' words, the credits' legends in place.

## Select's questions and the rewrite's own rows (2026-10-20)

Branch `select-questions` (not merged). Vj ruled (2026-10-20) that the
original's Select questions are built, the rewrite's own uses of Select
moving off the original's screens, and that the project's own notice stays,
on the rewrite's PC Settings page. Read from the executable and measured in
the project's PCSX2 runtime (its own card files, unchanged) over a fresh
boot and runs driven on from it (`street-select-orig`, INDEX.md: PINE on
the front end `*0x3fe910`, its screen's class `+0x40` and state `+0x6c`,
the transition `+0x4c`, and the question among the screen's children by
its vtable `0x46e700`, its id `+0x40` and lit row `+0xb4` against the Yes
widget `+0x110`; the 100 Hz timer; PrintWindow grabs). The rewrite's in
`street-select`.

**The question.** `FUN_0024bfa0(dialog, screen, id)` is the base Yes/No
dialog `FUN_0025d328` with text id 0x110 (`common_fe.txt` 16, "Exit\nto
Main Menu?"), its rows "Yes" and "No" (`common.txt` 2, 3; `FUN_0025d410`)
in font 2 (`FUN_0025c860`'s last argument) laid out by `FUN_0025cda0`, and
`+0xe0` set: the dialogs' keys (`FUN_0025c100`) close on Triangle (0x25,
sound 9) only while `+0xe0` is clear, so Triangle does nothing; Cross
(`+0xd0`, 0x23) answers (`FUN_00260b88`, sound 0xf). Every caller plays
sound 3 as it opens it. The id is only what the screen's answer (its slot
`+0xe4`) tests: Yes (`FUN_0024c028`, the lit widget `+0xb4` the Yes widget
`+0x110`) goes back to Main (`FUN_002d7d40(screen, 3)`), No plays sound 9
and leaves the screen as it was. Measured: it opens on No; Down on No
stays, Up lights Yes (no wrap); Triangle and Start leave it open; Cross on
No closes it, Cross on Yes puts the screen in state 2 in the same update.

**Where Select asks** (each factory class's keys, slot `+0xa4`, and answer,
`+0xe4`, through `FUN_002e55c0`; the table in `exit_ask.rs`): Play It
(0x55), My Team (0x55), the credits' screen (1000), High Scores (0x3e9),
User Setup (0x55), Rewards (0x44e), Choose Team and the League Challenge's
(200; not join a team, class 0x4d: `FUN_0026ed50` tests `+0x40`), Choose
Players (0x55), Choose Court (0xd06), My Rules (0x457), Options (0x85),
Choose Region (0x457), Trax (10), Choose Baller and Create A Baller
(535, `DAT_003f814c`), Controller Layout (0x55), Be A Legend's map (0x55),
Create Baller (535), the editor's screens: Edit Baller, Edit Moves, The
Moves (535), a tournament's bracket not yet decided (0xd05; `+0x10c`, set
as the champion shows, `0x266a98`, turns it off), Game Data (0x55), the
DVD pages (0x55) and Difficulty (0x55). Main takes no Select
(`FUN_0029e700`'s switch passes 0x3e to `FUN_002d7db0`), nor do the title,
the boot, the rolls, the results, the loading screens, the event intros,
Create Team and the crew, the completion boards, join a team and Update
Team. The movies pass's list had the map's keys (`FUN_0029f4b0`, class
0x38) as Main's and Controller Layout's (`FUN_00273388`, 0x37) as the league
Challenge's Choose Team's, and missed the rest.

**What Yes does besides.** Trax (`FUN_00265c10`) applies the radio's
rows to the settings and restarts the music (`FUN_00219f58`,
`FUN_001a0208`) without the card's save its Exit makes; Options
(`FUN_002a5cb8`) puts its settings back as the game data holds them
(`FUN_002a6d40`, as its cancel question's Yes); User Setup (`FUN_00298168`)
clears the codes entered (`FUN_00298ae8` on `FUN_00131208`'s settings);
the editor (`FUN_0028bd60`) keeps the baller as edited in the game data
(`FUN_001e6888`, as its save does) without the save, and stops the film
room's clip (`FUN_002e4690`); Inside's (`FUN_0028a208`) plays the
transition sound itself (`FUN_001a1b88(.., 0)`, as Main's rows and the
other movie-less changes among Main's screens do; Play It's and My Team's
answers do not). Game Data's leaving save
after a delete and Rewards' after a purchase are not made.

**Who it answers.** User Setup, Choose Players, Choose Court and My Rules
take Select only from a pad with a person in the game (the game data's
`+0xd8[pad]`, `FUN_001dbda0`, set as a pad confirms Play It and
Difficulty; User Setup's build clears them); the map and the brackets only
from the user's pad (`+0xd4`); Choose Region's screen took no key at all
until `+0xd4` named the pad. Requested through PINE with those words
written, the screens asked; User Setup asked on the real way in (Play It,
Pick Up Game, Difficulty).

**Measured** (runs 1..10): the question on 22 screens with their ids, the
same box over each (Play It, My Team, Difficulty, Options, the credits'
screen, High Scores, Rewards, Trax, Game Data, Controller Layout,
Inside, What's New, Bobbito, Choose Team, the League Challenge's Choose Team,
Choose Region, Choose Baller, Create A Baller, User Setup, Choose Court,
My Rules, Create Baller's wizard); Main's Select did nothing. Choose
Players hung the runtime when requested over My Rules, and the map, the
brackets and the editor need a career or a baller the runtime's card has
not: those four rest on their code. From the answer's update to Main's
state 4: 0.52 to 0.53 s from Play It, My Team and Inside (no movie, Main
keeping its loop), 1.49 to 1.53 s from the rest through `xmtblur`, 1.75 to
1.79 s from Difficulty and the DVD pages (the leaving records, the blur,
Main's loop made anew: 0.11 s), 1.84 s from Trax and 1.98 to 2.00 s
from Create Baller's wizard (their changes held 0.30 and 0.41 s longer
before the blur).

**The rewrite** (`exit_ask.rs`; `menu.rs`: `Menus::exit_ask`,
`select_asks`, `exit_to_main`, `put_back_options`; `MenuInput::selected_by`:
a pad's Select, P on the keyboard as in a game): the question over the
screen in `pause::dialog`'s box, its keys before the screen's (the Help
overlay held off), its open sound through `Menus::cue`. Yes truncates the
pages to Main, a back change, with the screens' extras and none of their
leaving saves; the seats stand for `+0xd8` (a seated controller where the
seats are not empty), the user's pad was not modelled (any controller on
the map, the brackets and Choose Region; since 2026-10-20, branch
`online-rerun`, their Select answers only the pad seated for the run,
`+0xd4`; since 2026-10-21, branch `legend-rules`, every key on Choose Region
answers only that pad, as the original's takes no key but the user's). The transitions: Main makes its loop anew after Difficulty and the
DVD pages (`settling_ms` by `sameloop`), and Select's Yes from Trax and
the wizard adds their measured hold (`Change::hold_ms`,
`exit_ask::hold_ms`); the model within 70 ms of all nineteen pairs. Found
on the way and fixed: the changes with no movie among Main's screens
(Main to Play It, My Team and Inside; Play It to Difficulty; Triangle back
from Play It, My Team, Difficulty, Inside, What's New and Bobbito; Inside
to What's New and Bobbito) play the transition sound at the press, their
keys' `FUN_001a1b88(.., 0)` (`transition::hand_whoosh`); the rewrite
played it only from a movie's name.

**The rewrite's own rows.** Online (Main's Select), Watch A Game (Play
It's) and the created teams' list (My Team's) are rows of the rewrite's
PC Settings page (Options, Adjust Screen: `options::pc::ONLINE`, `WATCH`,
`TEAMS`), not of the original's screens. The project's notice stands under
its rows (`options::NOTICE`, `look::notes`), word for word as the
rewrite's credits page had it before the original's credits screen
replaced it (d3af7d9).

Compared (the mean difference at the best shift; the matched Difficulty
screen 10.9), the question over each screen: Game Data 6.6, My Rules 6.7,
Create Baller 6.9, Choose Baller and Create A Baller 7.1, High Scores 7.2,
Difficulty, Options and Trax 7.3, Play It, My Team, the credits'
screen, What's New and Bobbito 7.5, Inside 7.6, User Setup 7.7, the league
Challenge's Choose Team 8.5, Rewards 8.8, Choose Court and Choose Region
9.1, Controller Layout 10.1, Choose Team 10.3 (its picks differ). 640x480 and 1280x720: no overlap; at 1280x720
the question stands in the pillarboxed 4:3 with its screen.

## Still to build, and what each needs

(Rewritten by the movies pass, 2026-10-20. The boot's movies, the league
board's trophy and Inside Vol. 2's movies are built, with the screens'
loops and the credits, above; Select's questions by the pass after.)

Known difference, accepted (Vj, 2026-10-20):

- *Options:* Adjust Screen opens the rewrite's PC Settings (a window or
  full screen, the Wide picture, vibration) instead of the TV picture
  adjuster (`FUN_0025b150`).

Found and not built: none. (Select's questions, the last, are built:
"Select's questions and the rewrite's own rows", above. Grabbed on 22
screens; Choose Players, the map, the brackets and the editor's screens
rest on their code.)

Out of the executable's reach, so left out as the original leaves them:
Create Baller's "What Does This Mean?" (action 0x45, unbound), the
editor's refund questions (10, 11) and slot 11; the card's delete op
(`+0x180`); Create Team's state 4 question (0x1d12).

Built from their constructors but not grabbed (unscored: the runtime's
boots never reached them): a jersey's close-up, Pick Up's other three
tip pages, Dunk to Lay-up's and Give 'n Go's second prompts; the card's
"Save failed. Retry?" and "Load failed. Retry?" (no runtime card fails);
Create Team's "Overwrite existing created team ...?" and "Only 10 teams
can be created." (0xce, 0xd4).
