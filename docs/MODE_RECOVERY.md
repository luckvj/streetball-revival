# Post-game reward recovery (2026-10-13)

New static evidence from the live Ghidra MCP server, project street2ee,
program SLUS_206.51, `/get_functions` and `/read_memory`. The project uses
EE opcode substitutions, so the branch and grant paths below were also
checked against the original disc executable with tools/mipsx.py.

## Pick Up threshold cards

FUN_001ded40, 0x1df6b0..0x1df780, executes within the winning-side
human-seat gate, only for front-end modes 2/3. It increments game record
+0x1c once, then walks all 113 XDBCARD rows in table order. Field 5 must
be positive, at most the counter, and FUN_001d26e8 must return false.
That helper returns true for zero-price cards or already-owned cards
(FUN_001e0398, the ownership bitset). Thus the threshold is Pick Up wins,
not games played, and reaching it grants ownership without spending RP.
The shop still allows purchasing cards before their automatic threshold.

Category 0 passes the card ID through notice kind 10, jump-table entry
0x45ffe8 -> 0x1de7d4 -> FUN_001e0420. Only one such notice fits per
post-game dispatcher invocation. Other categories call FUN_001e0420
directly. This marks ownership and related milestone bits; a court-10
card additionally enables the native cheat flag. Threshold rows on the
US disc are Street Legends, tips and cheat cards, not courts.

Built: shared pickup_wins counter, optional PKUP save section (old saves
default to zero), original-save import from unlock section +4, automatic
card ownership and multi-player-card grants after eligible local wins.
The helper skips already-owned and free cards and retains the single
Street Legend notice limit. Regression test covers threshold boundaries,
RP preservation, reload, duplicate suppression and Nelly's whole crew.

## Win/loss development bonus

FUN_001ded40 calls FUN_001e9b90 only in mode 5, for the selected user's
winning or losing side (call sites 0x1deebc / 0x1df85c). Modes 2/3 pay RP
50/100/150 on wins; mode 4 pays RP 100/200/300 on wins. Neither calls the
story development-point updater. FUN_001e9b90 calls FUN_001ea900 with
kind 1 and won; its 100/150/200 award is already implemented for story.
The alleged missing non-story W/L development grant is therefore closed
as absent in this executable's dispatcher, not a missing rewrite feature.

These are static US PS2 findings; no original runtime replay was captured.

## Shop category availability

FUN_001d2b38 counts every contiguous row in a category. FUN_002c2a90
populates every slot below that count, without filtering field 4 (picture).
FUN_001e05e0 charges FUN_001d27d0's price and grants ownership; it does not
require a nonzero picture. Thus the prior shop exclusion of the final eight
tips and five code rows was unsupported. The rewrite now retains all rows
and treats zero-price cards as owned, matching FUN_001d26e8. Missing-image
presentation still uses the existing face-down fallback.

Also corrected the dispatcher gates: lesson practice cannot pay Pick Up RP
or advance the threshold counter, and Trick Point Challenge / Gamebreaker
Wins cannot grant a shutout from their ordinary score fields.

## Create A Baller cost tables and split

Decompiled `FUN_001e6518`, `FUN_001e2470`, `FUN_001e4af8`, `FUN_001e4580`, `FUN_001e4c50`, `FUN_001e4e30`, `FUN_001e5138`, `FUN_001e51d0` from the live Ghidra MCP server (`SLUS_206.51`).
The table at `0x4602a0` contains `{count, pointer}` pairs for all categories of Create A Baller:
- Entry 0 / 1 (`0x4602a0` / `0x4602a8`): Builds (Men: 4 at `0x460368`, Women: 2 at `0x4603a8`), 16 bytes per entry: price (0/200/300/400), size count (4), first preset, preset count (9), size pointer, preset pointer.
- Entry 2 / 3 (`0x4602b0` / `0x4602b8`): Face Presets (Men: 18 at `0x4603c8`, Women: 6 at `0x460410`).
- Entry 4 / 5 (`0x4602c0` / `0x4602c8`): Hairstyles (Men: 18 at `0x460428`, Women: 12 at `0x460450`).
- Entry 6 / 7 (`0x4602d0` / `0x4602d8`): Shoes/Socks (Men: 8 at `0x460468`, Women: 8 at `0x460478`).
- Entry 8 / 10 (`0x4602e0` / `0x4602f0`): Tops/Jerseys (Men: 23 at `0x460488`, Women: 6 at `0x4604b8`).
- Entry 12 / 13 (`0x460300` / `0x460308`): Apparel category with 20 items (Men: `0x4604c8`, Women: `0x460518`), where each 4-byte entry is `(cost, item_id)` with item prices 0, 25, 50, 75, 100, 125 for items 0..19. This is an apparel customization table (read by `FUN_001e4c50` and `FUN_001e4e30`), not a skill rating table.
- Entry 14 / 15 (`0x460310` / `0x460318`): The 7-step skill point-buy ladder `[0, 100, 200, 300, 400, 500, 600]` at `0x460568` (Men) / `0x460578` (Women). `FUN_001e5138` computes the cost to advance from current rating `uVar1` to `uVar1 + 1` by reading `table[uVar1 + 1]` (`(uVar1 & 0xffff) * 2 + table + 2`), exactly matching `skill_step_price` in `ballers.rs`.
- Move costs come directly from `XDBMOVE` records on disc (`m.cost`), charged when `!b.owns(row)`.
- In Be A Legend Gym (`story.rs`), skill upgrades are purchased with Development Points (`DP`) using the clean 25/50/75/100/125 ladder (`rating_cost`).

## Be A Legend calendar games and week counter

Decompiled `FUN_001e70a8` and `FUN_001e9b90` from the live Ghidra MCP server (`SLUS_206.51`).
- `FUN_001e70a8(param_1, index, delta)` updates four career counters at `param_1 + 0x24 + index * 4`:
  - `index == 0` (+0x24): `games` (calendar game counter, save offset `0x0c`)
  - `index == 1` (+0x28): `wins` (save offset `0x10`)
  - `index == 2` (+0x2c): `losses` (save offset `0x14`)
  - `index == 3` (+0x30): `development_points` (save offset `0x18`)
- In `FUN_001e9b90`, every completed game increments `wins` by 1 on a win (`FUN_001e70a8(param_1, 1, 1)`) or `losses` by 1 on a loss (`FUN_001e70a8(param_1, 2, 1)`).
- For `games` (+0x24), the calendar advances by event kind:
  - Pickup games: `FUN_001e70a8(param_1, 0, 2)` (+2 games).
  - Street Challenges: `FUN_001e70a8(param_1, 0, 2)` (+2 games).
  - Tournaments: `FUN_001e70a8(param_1, 0, 6)` (+6 games) upon tournament conclusion (either elimination or championship). Intermediate tournament games do NOT increment `games`.
- Displayed calendar week is `games / 6 + 1`.
- In `crates/street-game/src/original_save.rs`, `games` is read directly from `native.counter(0).unwrap_or(0).max(0) as u32` rather than synthesizing `wins + losses`.

## Result notices and missing-card picture fallbacks

Decompiled `FUN_001de528`, `FUN_001de8d0`, and `FUN_002c2a90` from the live Ghidra MCP server (`SLUS_206.51`):
- `FUN_001de528` evaluates post-game reward notices through the 11-entry jump table `PTR_LAB_0045ffc0` at `0x0045ffc0` (kinds 0..10):
  - Kind 0 (`0x001de5a8`): Reward points (`story.reward_points`).
  - Kind 1 (`0x001de5cc`): Team unlock (`xdbteam.adf` via `FUN_001ebc88` and player roster resolution).
  - Kind 2 (`0x001de65c`): Court unlock (`xdbcourt.adf` via `FUN_001d3490` and category-2 card mapping).
  - Kind 3 (`0x001de71c`): Player recruit pool addition (`xdbplyr.adf`).
  - Kind 10 (`0x001de7d4`): Milestone / threshold card notice.
- `FUN_002c2a90` resolves card artwork presentation. When a card is locked or its poster index is 0/missing:
  - Queries category table at `0x0048f678` (`[3, 3, 4, 4, 4, 4, 4]`).
  - Category 0 (Street Legends) and Category 1 (Legends) have entry 3, selecting image `0x66` (102, `LEGEND_FACE_DOWN` silhouette).
  - Categories 2..6 (Courts, Jerseys, Development Points, Tips, Codes) have entry 4, selecting image `0x65` (101, `FACE_DOWN` generic card back).
- Implemented in `crates/street-game/src/rewards.rs` (`LEGEND_FACE_DOWN`, `card_picture`) and verified with unit tests.



