# Simulation recovery notes

This file records target-side evidence gathered while closing simulation parity gaps. Addresses are from Ghidra program `SLUS_206.51`; names are the current Ghidra labels. “Observed” means directly present in decompilation or executable data. “Inference” is labeled. Values hidden in runtime BSS remain unresolved.

## Trick callback 53 and self-toss

The callback table at `0x003e86d0` has entry 53 at `0x003e87a4`, pointing to `FUN_001741a8` (`0x001741a8`). Parsed Vol. 2 move scripts invoke callback 53 in these scripts (script args shown in stored order):

| Script id | Script name | Script address | Args |
|---:|---|---:|---|
| 21 | `juke_slipslide_roll` | `0x0044de67` | `[42, 70, 0]` |
| 23 | `juke_selfbounce` | `0x0044dedc` | `[54, 30, 1]` |
| 36 | `juke_flip_ball` | `0x0044d840` | `[32, 32, 0]` |
| 60 | `juke_lob_fake_xover` | `0x0044e044` | `[69, 50, 0]` |
| 61 | `juke_flipball_underleg` | `0x0044e0b9` | `[38, 34, 0]` |
| 69 | `juke_lob_aroundbody` | `0x0044e365` | `[28, 21, 0]` |

Callback 53 resolves the actor associated with the callback invocation. If the current ball action is not already state 4 with both stored actor pointers matching, it calls `FUN_0014bff8(ball, actor)`. That function stores the actor at `ball+0x100`, resolves and stores its counterpart at `ball+0xfc`, sets `ball+0x180` to 4, clears `+0x1a4`, `+0x60`, and `+0x64`, and enables actor flags at `+0x288`. The callback then samples animation/skeleton transforms and calls `FUN_0014da20`; it derives the ballistic destination from an animation pose rather than a fixed target. `FUN_001741a8` chooses the live channel and samples frame 21 from that channel's animation record through `FUN_0019bda8`/`FUN_0019bde0`; `FUN_00192780` reads the frame-21 state flag. All three script arguments are used: the first supplies the initial flight duration, the second is the sampled animation frame, and exactly third-argument value 1 enables bounce. The earlier unused-argument conclusion came from a false CALL_RETURN override at `0x00174648`; clearing that override exposed the continuation through `0x00174c0b`. Raw MIPS at `0x0014da20..0x0014dc70` confirms that selected animation metadata (`param_4`) selects one of nine handlers through the table at `0x0043ed40`; the raw table words point to `0x0014dc78`, `0x0014dc94`, `0x0014dcb0`, `0x0014dcd8`, `0x0014dcfc`, `0x0014dd20`, `0x0014dd48`, `0x0014dd6c`, and `0x0014dd9c`. `FUN_0019c2e0` (`0x0019c2e0..0x0019c2f0`) is only an indexed accessor for the live animation-channel metadata array, while `FUN_0019c770` (`0x0019c770..0x0019c774`) returns its live channel count. The static live-channel selection is recovered below, while the runtime channel contents and conversion from the sampled rig transform to sim world coordinates remain open; the VM callback is one-shot, and no continuing per-frame ball-follow update has been established.

`FUN_0014c408` is the collision-side updater. When ball state is 4 and its collision actor (`param_6`) equals the stored target at `ball+0x100`, it sets `ball+0x1a4=1`. `FUN_0014d1f8` also decrements `ball+0x158` during state 4; when the timer reaches zero it changes state to 1, sets `+0x1a4=1`, clears `+0x1ac`, and emits a timeout event. `FUN_0014c0c0` initializes the timer as `param3 + DAT_004b84b0` and copies `param3` to `ball+0x64`; the updater compares the decremented timer to `DAT_004b84b0`. The raw initializer is `FUN_00153b68`: `FUN_0030fa50` returns the singleton at `0x004da218`, whose constructor `FUN_0030f938` writes default integer 60 to `+0x24` (`0x0030f9c8`); the reset divides that value by 12 (`0x00153ec8`) and stores the integer quotient to `DAT_004b84b0` (`mflo`/`sw` at `0x00153f9c..0x00153fa0`). Thus its default initialized value is 5 frames. A generic timer started with `param3` expires after `param3+5` decrements, and callback 53 directly reaches this launcher at `0x0017480c` when `game+0x160 == -2` and the path queue is empty. Script 69 initially supplies 28 flight updates at default 60 Hz, hence 33 timeout decrements for this direct path. The planner branch when `game+0x160 != -2` can replace the duration; the same 33-decrement claim does not apply to that branch. The collision path uses `FUN_00153278`, `FUN_00153428`, `FUN_00150cc8`, `FUN_00151928`, `FUN_00152418`, `FUN_001526d0`, `FUN_001515a8`, and `FUN_00153550`; there is no single verified catch-distance constant, and the relevant dynamic target geometry still needs complete tracing.

The move row map at `0x0045f9c8` maps equipped XDBMOVE rows to script ids. Callback-53 script rows observed in the disc are 9→23, 17→23, 28→36, 29→21, 65→69, 68→36, 82→23, 83→60, 85→69, and 91→21. The default `TRICKS_SET` contains script id 69, so that is the only callback-53 self-toss reached by the default dribble set; other callback-53 scripts require equipped rows. The current game layer resolves equipped rows in `clip::move_selection`, while `street-sim` only models a trick level and randomized variant. Passing equipped ids into authoritative sim state would need snapshot and online setup changes; that should follow after the action-to-row selection parity is established.

The self-toss setup also emits a separate handler-transition event. In `FUN_0014bff8`, the `DAT_003c6500` record stores the previous handler's player id at `+4` and the passed actor's id at `+8`. Its listener proxy `FUN_00104b78` dispatches to listener vtable offset `+0x4c`, which is `FUN_00131d88`. That callback resolves `+4` and tests the previous handler's `+0x34` vertical coordinate; it does not test planar movement, and the event's tested actor is not inherently the receiving player or a defensive actor.

## Drain scoring fields

`FUN_00131ff8` is the combo/drain event listener: `DAT_0043c208` routes vtable offset `+0x9c` to it, and proxy `FUN_00104e98` dispatches event records to that slot. Its class selection is: only enter the builder if manager `+0x16c != 0` (`0x00132010..0x00132018`); if `game+0x130+0x1ac != 0` (`0x00132078..0x00132088`), use class 17 (`0x0013208c`) and special value/extra; otherwise use manager `+0x154` (`0x001320cc..0x001320d8`). A nonzero `+0x154` normally selects class 8, with value 2 conditionally selecting class 14 from incoming actor flag queries `FUN_0016e9e0`/`FUN_0016e9d0` (`0x001320d0..0x00132104`). If `+0x154==0`, resolve previous actor from event `+4`; only `player+0x268==5` selects class 9, otherwise this notification does not push (`0x00132108..0x00132124`). In the normal path, extra and value start at `-1` and the final event dword at zero. `FUN_00131788` does not branch on that final dword: raw instructions `0x001317a0..0x00131814` gate the push on game mode/class, and `0x00131890..0x001318bc` copy the full 24-byte record into ring slot 5. Therefore the field called folded here is simply stored at slot `+0x14`; it does not alter the push/shift/gate in `FUN_00131788`. Normal classes 8/9/14 carry `extra=-1,value=-1`; the drained meter amount is a different quantity. The special path pushes class 17 with value from `FUN_00143a88(player)`, extra set to whether `player+0x5c != 3000`, and the final dword still zero.

The relevant event is emitted by `FUN_0014cb28` during a state-4 handler transition. In the branch at `0x0014cdd8..0x0014ce10`, when the incoming actor is the controlled handler, its `DAT_003c65b0` record stores the previous handler (`ball+0xfc`) player id at `+4` and incoming actor `iVar3` id at `+8`; the shared event call is at `0x0014d03c..0x0014d04c`, and `DAT_003c65b0+0xc` points to proxy `FUN_00104e98`. Thus `FUN_00131ff8` resolves `+8` for incoming-player flag checks and the class-17 value/extra, while its class-9 guard resolves `+4` and checks `player+0x268 == 5`. Those are the previous/from and incoming/to actor roles used by a sim possession handoff, but the source does not establish that every sim steal/counter enters this event path, or that a shot block does.

`FUN_00131d88` is a different listener on the `DAT_003c6500` event emitted by `FUN_0014bff8`. It resolves event `+4` (the previous handler) and reads that actor's `+0x34` vertical coordinate, not planar speed. The actor physics helper `FUN_00153278` clamps `+0x34` against floor baseline `+0x11c` and integrates it using vertical velocity `+0x44`. The callback sets combo-manager `+0x154` to 0 when `+0x34 <= 0`, 1 when positive and `+0x164 == 0`, or 2 when positive and `+0x164 != 0` (`0x00131d98..0x00131dd8`). Constructor `FUN_001313d0` for the `DAT_0043c208` listener initializes `+0x154`, `+0x164`, `+0x168`, and `+0x16c` to zero and stores the game pointer at `+0x15c` (`0x001313d0..0x0013149c`). `FUN_00131e00` sets `+0x164=1` when its event actor id matches the currently selected ball actor (`0x00131e18..0x00131e5c`); `FUN_00131e78` recomputes it from the selected actor's `+0xa4` bits 7/15 (`FUN_0016e9d0`/`FUN_0016e9e0`) and clears it when neither bit is set or there is no actor (`0x00131ebc..0x00131f5c`). Since `+0x16c` starts at zero, `FUN_00131ff8` skips the class builder on its first notification and sets `+0x16c=1` (`0x00132174..0x00132178`); later notifications pass that gate. No connection from these manager transitions to sim block/steal/counter inputs is proven.

The constructor defaults player `+0x268` to 4. The resumed raw-instruction audit found the omitted runtime writer at `0x00160f78..00160f7c`: the selected 28-byte pass record at `0x003d65e8` supplies its `+0x18` word to player `+0x268`. Value 5 belongs to kick-pass records, not a defender state (details below). `FUN_001a5f48` checks this same pass classification for the extra crowd call. A shot swat in `street-sim` is not shown to produce the native state-4 handler-transition event, and the native class for any sim steal/counter still depends on manager/animation state not represented in sim snapshots. Leave the sim's class-17/points fallback unchanged until those associations and class fields are proven; there is no supported event-to-row rewrite yet.

## Remaining probes

- Confirm which live channel `FUN_001741a8` selects for script 69, and map its sampled bone transform through the player rig into sim world coordinates before wiring a self-toss cue.
- `DAT_004b84b0`'s default initializer is resolved to 5 (`0x00153b68`, `0x00153ec8`, `0x00153f9c..0x00153fa0`; source default 60 at `0x0030f9c8`). The callback direct path now proves a `FUN_0014c0c0` call and first-argument duration. Determine when gameplay enters its direct versus planned path; argument 21 remains the sampled animation frame.
- Continue the collision call chain (`FUN_00153278`, `FUN_00153428`, `FUN_00150cc8`, `FUN_00151928`, `FUN_00152418`, `FUN_001526d0`, `FUN_001515a8`, `FUN_00153550`) into `FUN_0014c408` to establish self-target catch geometry and the exact state transition.
- Player `+0x268 == 5` is now resolved as kick-pass metadata; identify which native actions produce the `DAT_003c65b0` previous/incoming handler event and correlate manager `+0x154/+0x164/+0x16c` values with those actions before changing drain rows. The current record proves actor roles and class-selection branches, not sim block/steal/counter equivalence.
- Add authoritative equipped-script identities only after row selection can be made deterministic from input and included in world snapshots/network match setup.

## Default self-toss timing audit

The original ISO and extracted ELF match at file offset `0x34f389`; the
bytes contain callback 53 with arguments `[28, 21, 0]`, so these values
are from the shipped image. The preceding bytecode blends selector `0xf8`
for 6 frames, names animation records 296 and 297, then runs timed frame
move `8d ee 07 0e` (frame 7 over 14) before callback 53. The VM dispatcher
masks opcode bytes with `0x7f` and uses the table at `0x3e8610`; callback
opcode `0x83` resolves to `FUN_00195a78`, blend `0x8e` to `FUN_001978b0`,
and wait `0x8d` to `FUN_00197298`. Selector `0xf8` chooses the third
animation id only when the script entity's `+0x25c` equals 2; otherwise it
keeps record 296. Record 297 is the mirrored row.

The zero `+0x10` row pointers in the ELF are pre-initialization values.
`FUN_00191c08` loads animation resources, allocates frame-state storage,
and writes the live pointer into each row. The `/ANIM/ANIBIG.BIN` state
track for `DRIBBLE_FREESTYLE_12_2_B_R` has 26 frames and source frame 21 has
flag `0x8`. The records are adjacent rows at `0x003e0900` and `0x003e091c`:
they share name pointer `0x00451080`, while their final words at `+0x18`
are 0 and 1. In `FUN_00191c08`, a nonzero final word selects the copied
source-flag path through `FUN_00191440`. That helper's pair table at
`0x004546a8` includes `(0x4, 0x8)`, so row 297's mirrored frame 21 flag is
`0x4`; row 296 retains `0x8`. `FUN_001741a8` maps `0x8` to dispatcher mode 1
and `0x4` to mode 2. Both records enter branch 0 of `FUN_0014da20`
(`0x0014dc78`), whose common selector uses skeleton bone 19 in mode 1 and
bone 15 in mode 2. That handler applies a local offset whose initialized x
component is `0.4` at `DAT_004b84c0` (`0x00153ed8`); the y component is zero
in the initial BSS image. Thus row 296 samples bone 19 and row 297 samples
bone 15 at animation frame 21, each plus the same local offset. The new
`Clip::joint_position_with_offset` helper samples this authored bone-local
point through the full rig parent chain in player-local space; unit tests
cover both a rotated joint and a rotated parent. It does not yet claim
equivalence with the native result: live-channel selection and the native
actor/floor/mirror transform into sim world coordinates still need to be
matched, especially mode 2's third-lane reflection.

The blend transition's counter starts at 1 on its setup update and commits
when it reaches 7, after 6 later channel updates. The 14-frame move starts
at 1 and completes at 15, after 14 later updates. The VM resumes on that
completion update and executes callback 53 then. Thus the callback follows
20 additional channel updates after the blend setup, assuming this script
runs on the ordinary single-channel path; this is a relative schedule, not
an absolute game tick. At callback time, native code samples pose frame 21
from the selected animation record; this is the second callback argument,
not the callback delay. No authoritative sim cue is wired yet because the
runtime move identity, live channel and native-to-sim world transform are
not all represented or proven.

## Callback-53 continuation and launch recovery

The shipped raw instructions at `0x00174650` continue after the singleton
call at `0x00174648`; Ghidra had incorrectly marked that call CALL_RETURN.
Only that callsite override was cleared and the following bytes disassembled.
The repaired decompilation reaches `0x00174c0b`. Raw `0x00174664..0x00174678`
loads callback argument 1 from `sp+0x36c`, multiplies by singleton `+0x24`,
divides by 60, and stores `DAT_003d65a0`. At `0x0017467c..0x00174694`,
argument 3 from `sp+0x374` sets `DAT_003d65a4` to 1 only if it equals 1.
Argument 2 still supplies animation sampling frame 21. Thus `[28,21,0]`
means initial 28-frame direct flight, pose frame 21, and no bounce.

`FUN_00142038` copies the current game target-planner fields into the local
record. If `game+0x160 == -2`, an empty queue (`game+0x134 -> +8 == 0`)
launches `FUN_0014c0c0` at `0x0017480c`; a nonempty queue calls
`FUN_001488e8` at `0x001747e4`. For a planner target other than -2,
the continuation can recompute duration from segment distances and enqueue
intermediate/final waypoints. It is therefore not safe to assign every
callback-53 launch a duration of 28 updates. The callback finishes by
writing the final duration minus one to actor `+0x1a8` (`0x00174b70`) and
the final duration to channel 0 `+0x20`.

The direct non-bounce launcher subtracts ball `+0x30` from destination
(`0x0014c0f8..0x0014c104`), divides xyz by the requested update count
(`0x0014c1b4..0x0014c1d4`), and adds `0.5 * ball+0x118 * updates` to
the vertical velocity (`0x0014c1d8..0x0014c1fc`). It stores the target
at ball `+0x90`, sets timer `+0x158` to flight updates plus `DAT_004b84b0`,
and retains the flight count at `+0x64`.

The bounce launch path (`param_4 == 1` in `FUN_0014c0c0`) executes `FUN_0014f728`
to solve for the first floor contact time `t1` (in updates) from the continuous
physical condition that the ball impacts the floor at `t1` and rebounds to reach
target height at `T`. This yields the cubic polynomial $c_3 t_1^3 + c_2 t_1^2 + c_1 t_1 + c_0 = 0$,
with coefficients $c_3 = -0.5 \cdot g \cdot (1 + e)$, $c_2 = (1 + 0.5 e) \cdot T \cdot g$,
$c_1 = -0.5 \cdot g \cdot T^2 - e \cdot h_0 - h_T$, and $c_0 = e \cdot T \cdot h_0$,
where $h_0 = y_0 - y_{\text{floor}}$ and $h_T = y_{\text{target}} - y_{\text{floor}}$.
Native restitution $e$ is stored at `ball+0x138`. The initialization at
`0x0014a54c..0x0014a564` evaluates $-1.2 x^2 + 2.2 x$ on default table entry
$x = 0.5$ from `0x0043edd8`, yielding exact default restitution $0.8$.
`FUN_0014f728` evaluates the derivative's critical points, brackets the root,
executes binary bisection until interval width $\le 1.0$, and finishes with a
single secant interpolation. `FUN_0014c0c0` then initializes vertical velocity
$v_y = 0.5 \cdot g \cdot t_1 - (y_0 - y_{\text{floor}}) / t_1$, and horizontal velocity
$(x_{\text{target}} - x_0) / T$.

`street-sim::self_toss::Cue` now implements both direct and bounce launch arithmetic,
discrete bisection/secant solving matching `FUN_0014f728`, and unified `launch_velocity`
across all six equipped callback-53 scripts (21, 23, 36, 60, 61, 69). Tests confirm
exact timeline durations, frame-rate scaling, downward initial bounce velocity,
floor impact, rebound velocity, and terminal target arrival.

The callback's target correction at `0x00174598..0x00174644` uses the
sampled animation root xyz, scales by skeleton `+0x40.x`, rotates by actor
`+0x50`, subtracts actor `+0xe0`, and adds the sampled bone result. When
the originally selected channel has flag `0x200000`, it instead uses the
zero vector at `DAT_004b9880`. The original LQ/SQ transfer is sixteen bytes;
patched Ghidra's eight-byte display must not be treated as the whole vector.
Native rig conversion, third-lane reflection, and skeleton scale remain
separate from the visible-rig helper's current implementation.

The callback initially selects channel 0, switches to channel 1 when that
channel exists and has `+0x48 & 4`, then prefers channel 2 if it exists,
has flag 4, and its link at `+0x58` is -1. On a channel without
`0x200000`, animation id comes from `+0x50`. Otherwise a linked channel
is followed when `+0x58 != -1`: if that linked channel lacks `0x200000`,
its `+0x50` is used; otherwise its `+0x54` is used. Without a link the
selected channel's `+0x54` is used. The target correction's flag check
still uses the originally selected channel pointer saved before following
the link. This algorithm is static evidence from `0x00174338..0x00174430`;
which channels are active during script 69 still needs runtime observation.

## Runtime pointer locators and script-69 catch path

`game = *(u32*)0x003d30bc`; `ball = *(u32*)(game+0x130)`;
`queue = *(u32*)(game+0x134)`. The ball's `+0x24` points back to game.
`FUN_00140920` resolves a player by team and slot as
`teamObject = *(u32*)(game+0x184+4*team)`,
`playerArray = *(u32*)(teamObject+0x44)`, and
`player = *(u32*)(playerArray+4*slot)`. During callback-53 state 4,
ball `+0x100` is the receiving player, and `+0xfc` the previous player;
for a self-toss they are equal. Skeleton is `*(u32*)(player+0x108)`.
The animation controller is inline at `player+0xc4`; its first word is
channel count (`FUN_0019c770`) and `+4` points to the channel-pointer array
(`FUN_0019c2e0`). Channel N is the Nth u32 pointer from that array.
Channel `+0x14` is current animation state flags, `+0x20` the callback
duration, `+0x48` channel flags, `+0x50/+0x54` animation ids, `+0x58`
channel link, `+0x5c/+0x60` current/target frame, and `+0x6c/+0x70`
timed frame-move duration/counter.

After callback 53, script 69 clears actor-state bit `0x10`, writes actor
`+0xa4 = 0x600`, reaches animation frame 21 with bytes `8d ee 15 ef 00`,
then reaches frame 25 over six updates and clears `0x600`. VM opcode
`0x9b` is an assignment, not an OR: raw `0x001958f4` stores the assembled
word directly to actor `+0xa4`. The duration sentinel `0xef` is signed -17;
raw `0x001973b8..0x001973dc` reads channel `+0x20`, adds the next signed
byte (zero here), and clamps the duration to at least 1. Thus script 69
waits from its current frame 7 to frame 21 over the final flight duration
written by callback 53; the default direct path uses 28 later channel
updates. The following six-update move keeps the catch state active through
frame 25. This does not establish an absolute simulation tick or catch tick.

Actor-state bit `0x100` is tested by `FUN_0016e9f0` (`actor+0xa4 >> 8 & 1`).
In `FUN_0015da28`, that bit selects `FUN_0016ed18` after the actor `+0x1a8`
positive-delay gate. Callback 53 writes final flight duration minus one to
that delay, so catch checks do not simply run from the launch update.
Raw `0x001572e8..0x00157314` in `FUN_00156f48` decrements actor
`+0x1a8` and `+0x1cc`, clamping each to zero, after the AI/player branch
and animation VM update. Thus the callback-written duration-minus-one
is decremented on that same player update; the ball-versus-player update
order and observed catch tick still need runtime verification.

`FUN_0016ed18` rejects the rim-area condition from `FUN_0016e830` and an
already held ball. It samples current channel-0 animation/frame, reads
channel `+0x14 & 0xc`, and calls `FUN_0014da20` with its seventh argument
1 to suppress ball-radius offset: flag 8 samples mode 1, flag 4 mode 2,
and flag 0xc samples both. An animated ball-attachment point distance no greater than
`max(ball+0x11c, 1.5)` qualifies (`0x0016edbc..0x0016edd8`,
`0x0016ef84..0x0016efb8`); for two hands either attachment point or their midpoint
can qualify. The VU sequence computes Euclidean length (square, sum, sqrt),
not a squared-distance comparison. Independently, swept actor-rig collision
`FUN_00137db8` can qualify hand-part pairs: flag 4 accepts both part indices
14/15, flag 8 accepts both 18/19, and flag 0xc accepts either pair.
Other collision contact applies the response at `FUN_00137ed8` instead of
immediate possession. The successful ordinary state-4 path calls
`FUN_0014cb08` at `0x0016f1b8`, which forwards to `FUN_0014cb28`; that
function sets ball state 3, assigns the held actor at ball `+0xf4`, writes
actor `+0x70 = ball`, clears state-4 targets/delays/queue, and zeros velocity.
State 6 uses additional rating/random gates; those do not establish a
random gate for this state-4 self-toss catch. These static links now identify
the direct script-69 catch mechanism, but runtime pose/world coordinates and
update order remain necessary for authoritative integration.

The offensive-player route is independently corroborated in raw MIPS at
`0x0015ba60..0x0015bac4`: reject positive actor `+0x1cc`, nonzero
`+0x1a8`, a held ball (`ball+0xf4 != 0`), and animation id `0x49`;
then `FUN_0016e9f0` flag 8 calls `FUN_0016ed18` at `0x0015bac0`.
Ghidra's offensive routine `FUN_0015a060` currently truncates at bad
instruction data, so its missing decompiled xref is not evidence that
only the defensive routine calls the catch helper. The raw executable
contains both callsites `0x0015bac0` and `0x0015e5b4`.

Native rig storage is now resolved: skeleton `+0` is bone count,
`+0x54` points to an array with stride `0xb0`. Per bone, `+0x00` is
evaluated translation, `+0x10` local translation, `+0x20` local quaternion,
`+0x30` evaluated quaternion, `+0x40` parent index, and `+0x44` bone index.
Skeleton `+0x20/+0x30` are root **translations**, not quaternions: raw
`0x0019cefc..0x0019cf14` copies skeleton `+0x20` into root bone `+0x10`,
evaluates the hierarchy, then copies that translation to skeleton `+0x30`.
The initial `(0,0,0,1)` values do not establish quaternion type. Skeleton
`+0x40` remains the full sixteen-byte body scale. Raw `FUN_0019ce50`
(`0x0019ce68..0x0019ce98`) copies sampled quaternion arrays into each
bone's `+0x20` and marks the rig dirty; this is the method callback 53 uses
for its temporary pose and restoration. `FUN_00156f48` sets the root
translation through `FUN_0019ccc8` from actor `+0x30` with actor `+0xe4`
added to Y. The callback target correction subsequently subtracts actor
`+0xe0` from the rotated sampled root-motion correction. Capture full
root translations, scale, actor orientation/compensation, and bones 15/19
when comparing original and native rig targets.

## Existing input latch and cross-layer regression

The sim already latches initiating pad buttons in `Player::action_buttons`
(`World::buttons` writes it only after `Player::free()` succeeds). An
active trick is not free, so releasing turbo buttons during the move
does not change that field. The game renderer's trick branch already
passes this latched value into `clip::move_selection`. Snapshot version 7
serializes the field and its checksum includes it. No extra
`Action::Trick` button field or snapshot version bump is needed. An earlier
lane message incorrectly described the renderer as using live buttons;
inspection of the actual callsite disproved that concern.

A disc-backed cross-layer regression initiates L2+R2+TRICK, confirms that
selection belongs to parsed default script 69, restores the snapshot,
and releases all buttons for eight actual world updates while checking
that the selected clip stays the same. A sim regression independently
checks snapshot/replay equality and that changing only the latched turbo
pair changes the checksum. Both targeted tests passed against the supplied
ISO. The remaining authoritative gap is a configured equipped/default
script map and resolved script identity in sim state, plus the native rig
destination and catch/update ordering; initiating buttons themselves are
already stable and deterministic.

## Original-runtime branch-0 attachment comparison

The unmodified original game was sampled through the parent's read-only
PCSX2 PINE reader (serial `SLUS-20651`, UUID `92aee337`). The capture
`street-internal-flight.json` has 1,000 samples. No script-69 execution was
identified in this report, so it does not close the frame-21 toss target.
The offline tool `tools/self_toss_oracle.py` derives branch-0 attachment
points directly from captured EE world-bone transforms, independently of
the Rust animation sampler. It records every compared row and all errors,
including nonmatches, in its optional output report.

The exact branch-0 order from raw instructions is:

1. Choose bone 19 in mode 1 or bone 15 in mode 2 (`0x0014dc78`,
   `0x0014ddb4..0x0014ddbc`); load evaluated quaternion `bone+0x30` and
   evaluated translation `bone+0x00` (`0x0014dde8..0x0014de0c`).
2. Componentwise multiply local offset `(0.4,0,0,0)` by skeleton `+0x40`
   body scale (`0x0014de48..0x0014de5c`). This is vector multiplication,
   not rotation or uniform scale.
3. With argument 7 zero, add ball radius to local Z
   (`0x0014dee0..0x0014df04`). Argument 7 equal to 1 skips this addition
   for catch sampling. In mode 2 negate the resulting Z lane
   (`0x0014df40..0x0014df4c`).
4. Rotate with `q * vector * conjugate(q)` without an extra quaternion
   normalization (`0x0014dfac..0x0014e004`), then add evaluated bone
   translation (`0x0014e010..0x0014e01c`). The two VU quaternion
   multiplication sequences form one rotation; they are not two rotations.

For the first captured held-ball sample (animation 78, frame 9, flag 4,
mode 2), body scale is `(1.199999928,1.264316082,1.264316082,1)` and radius
`0.46875`. Local vector becomes `(0.479999971,0,-0.46875,0)`. The oracle
predicts world `(-7.001745365,4.529040800,-5.618972456)` while EE stores
`(-7.001745224,4.529040813,-5.618972301)`, Euclidean error `2.10e-7`.
All 594 compared ordinary-branch held samples use mode 2; no mode-1
sample was available, so this capture directly corroborates only mode 2.
Of these 594 samples, 382 match within
`1e-4`; the matching rows' largest error is `3.91e-6`. A stricter group
with matching single-hand flag, no channel blend flag, and stable root
pointers has 235 compared samples, 213 matches and 22 nonmatches (largest
error `1.3531`). The full group's largest error is `2.4673`. The capture
reads are not atomic; non-hand dribble, dual-hand motion, pose timing and
other attachment logic remain possible explanations for nonmatches. They
were not discarded or attributed solely to read races. The oracle uses
Python double precision while EE uses float32 VU operations.

The current-frame catch point is derived by the same ordering with the
radius addition suppressed. These derived catch points are not captured
catch events. The native offset transform is now directly corroborated
for many held-ball poses, while script 69's temporary frame-21 hierarchy,
root-motion correction, mirror selection and authoritative catch order
still need a targeted original-runtime observation.


## Pass-kind writer and handoff gates (parent audit, 2026-10-13)

A scan of original EE store opcodes with immediate +0x268 identifies player
writes at 156900 (default4), 160e58 (fallback4), and 160f7c (selected record).
The latter is inside FUN_0015f930, whose Ghidra decompilation previously
stopped after false CALL_RETURN boundaries at calls to the returning config
accessor. Those boundaries at 15fc44 and 15fd68 were cleared. Raw MIPS remains
the authority for its omitted tail; no original instructions were changed.

At 160f28..160f7c, record=3d65e8+selectedIndex*28 and player+268=record+18.
The table contains 93 records before its null body at 3d7014. Its stored kind
counts are 0:4, 1:1, 2:40, 3:4, 4:40, 5:4. All four kind5 records are:

| Index | Record | Body | Registry name |
|---:|---:|---:|---|
| 6 | 3d6690 | 454cd6 | pass_dropkick |
| 7 | 3d66ac | 454d3d | pass_knee |
| 8 | 3d66c8 | 454da4 | pass_sidekick |
| 9 | 3d66e4 | 454da4 | pass_sidekick |

The registry's 632 rows are {body,name} at 3e7240, not {name,nextBody}.
An initial off-by-one registry lookup was corrected before recording these
names. Static evidence resolves the kind5 guard as previous actor kick-pass
metadata. It does not establish a generic block/steal action mapping.

FUN_00131d88, at raw131d88..131de8, sets comboManager+154 from the releasing
actor's Y coordinate: grounded ->0, airborne ->1, or ->2 if manager+164 was
already set. FUN_00131e78 refreshes +164 from held actor flag7/flag15 and
clears +168 each update. FUN_0016e9d0/e0 read actor+a4 bits7/15 respectively.
Thus +154 is not intrinsically a steal kind. The full event lifecycle and
native class17 special-drain flag still need mapping before replacing legacy
sim steal/block logic. Normal class8/9/14 pushes enter the ring without
immediately running the payout matcher, whereas FUN_00132588 class20/21
passout pushes run it. These differences must be retained in any pass rewrite.

The combo manager is game+d0, proven by constructor FUN_001313d0's call at
13d5d8. Team actor arrays are *(game+184/188)+44, as FUN_00140920 shows.
The expanded read-only PINE probe captures all six actors' pass kinds,
channel0 scripts, handoff flags, and all six full combo records. A 1,000-read
original capture (street-internal-pass-kind.json in temporary storage) has
six actors throughout and observes actor2 changing from kind4 to kind2.
It observes +154 values0/1 and +164 values0/1 with +16c enabled. No kind5
execution was captured in that burst; kick-pass runtime coverage remains open.
The live class-value table exactly matches the existing 22-value Rust table;
class9's fallback is250 and class20's is1000. These are combo record bases,
not proof of the rewrite's immediate displayed pass awards.

`tools/pine_probe.py --rig` now also captures all native local/evaluated bone
translations/quaternions and parents in one batch, to address the next
self-toss hierarchy comparison. Native FUN_0019d4c8 is not simple componentwise
bodyScale for every bone: it normally scales local translation uniformly by
bodyScale.x; the player branch can instead scale X at bones13/17 and Z at
bones1/5 by bodyScale.y when y>x. Root and other morph cases have further
branches. The visible-rig sampler cannot safely supply authoritative toss
targets until those differences and frame21 root compensation are resolved.


The first three complete-rig attempts had zero active-game samples after the
attract match ended; they provide no rig validation. The comparison reader
now writes coverage counts (active game, six actors, ring, complete rig) to
make such empty captures explicit. A fresh match is needed before testing
the full hierarchy; no no-data result was treated as a match. Subagent lanes
stopped on the shared usage allowance during this continuation. The parent
completed the pass-kind audit independently.

## Native mirror correction and observed script-69 target

`FUN_00192438` calls `FUN_001914c8` for animation rows with `+0x18 != 0`;
`FUN_001925f0` separately negates root translation X. The fourteen triples
at `0x003e6790` cover main player bones 0..21: root `(0,0,type1)`, body
pairs `(1,5),(2,6),(3,7),(4,8),(12,16),(13,17),(14,18),(15,19)`, and
self-pairs 9,10,11,20,21, all type4. Raw type1 code
`0x0019159c..0x001915fc` permutes root xyzw to zwxy; raw type4
`0x00191658..0x001916a4` maps body xyzw to `(x,y,-z,-w)`. These are
not the renderer's former uniform X-reflection quaternion rule. The
decoder and renderer both retain xyzw directly: no intervening axis
conversion occurs in skeleton rotations, clip rotations, or mesh positions.

The renderer now recognizes the shipped main-player bone-name layout,
uses this exact index table and component mapping in both `pose_skeletons`
and `Clip::joint_position`, preserves root translation's native X reflection,
and leaves auxiliary bones 22+ untouched. Arbitrary other skeleton layouts
retain their existing generic mirror fallback. `Rig::new` provides the
same recognized layout to the visible-pose path. `Clip::state_at_mirrored`
also implements the five pair swaps at `0x004546a8`: `(1,2),(4,8),
(0x40,0x200),(0x80,0x400),(0x100,0x800)`, preserving paired/other bits.
Staged hand-track generation uses those mirrored state flags before
choosing bone 15/19; it does not mirror the pose twice. Nontrivial root/body
component tests, native-layout disc tests, row296/297 frame21 forward and
mirrored visible-versus-sampled tests, and staged mirrored-hand-track/setup
roundtrip checks passed against the supplied ISO.

The later capture `street-native-full-rig-active.json` contains 1,000 active
game samples and 798 complete rigs. The offline hierarchy oracle now
compares main bones 0..20 inclusive (head21 needs uncaptured head scale;
facial/auxiliary22..42 are outside `FUN_0019d4c8`'s main loop). Its formula
is `Qworld = Qparent*Qlocal`, `Pworld = Pparent +
Qparent*scaled(P_local)*conj(Qparent)`, copying root local pose directly.
Local translations normally use uniform bodyScale.X. In the player-shape
branch, when Y>X, bones13/17 use Y on local X and bones1/5 use Y on local Z.
Of 16,758 position comparisons, 16,663 match within `1e-4`; 16,660 quaternion
comparisons match under sign equivalence. Max position/quaternion errors
are `0.424739`/`0.084692`; all nonmatches are retained. The virtual-method
player-shape flag was not captured, so that branch is an explicit assumption
of this independent oracle, supported by the large match count.

This capture **does observe actual script69**, unlike the earlier flight
report: actor `0x014fd080` has script PCs in `[0x0044e365,0x0044e3da)`
from samples767..811. The first observed self-flight is sample783, row296
frame7, duration28, timer32, target `(39.250740051,3.713146925,2.491582155)`.
Sample805 is still state4 at frame20/timer6; sample806 is held state3 for
the same actor at frame21. The target and flight duration remain stable
through the observed flight. This is mode1, nonbounce, direct planning
(`game+0x160=-2`, empty queue).

An independent world-bridge reconstruction uses sample782's prelaunch
root translation/body scale/local bone translations, sample806's frame21
local quaternions (its root already includes actor orientation), and the
frame21 root-motion compensation. It applies the hierarchy and mode1
attachment offset/radius, then adds frame21 root compensation and subtracts
prelaunch compensation, matching callback `0x00174598..0x00174644`. It
predicts `(39.250760380,3.713146677,2.491582708)`, error `2.03e-5` from
EE callback target. `tools/self_toss_oracle.py --script69` retains this
comparison; `--hierarchy` retains all main-bone comparisons. Reports stay
outside the source tree.

The first disc-decoded target missed by `0.00457`, exposing real decoder
differences instead of an axis-mapping guess. Raw `FUN_0038e558` uses
12-bit key multiplier `0x3a000801` (2/4095), not 1/2047; raw
`0x0038e4a0..0x0038e528` uses 16-bit min/range multiplier `0x38000080`
(2/65535), not 1/32768. Delta accumulation uses multiplier `0x3c820821`
(1/63). Integer-frame `FUN_0038d188` copies the accumulated quaternion
without normalization. Those coefficients are corrected, and
`Bank::native_rotations` now provides unnormalized QFast values separately
from the renderer's safely normalized `Bank::rotations`. Other formats
return None for the native API until their exact raw path is mapped.
A coefficient-boundary fixture and an actual ISO+external-capture frame21
callback-target regression pass within `1e-4`. Native guest pose data is
read from `STREET_NATIVE_CAPTURE`, never embedded as a shipped quaternion
array. The renderer API remains normalized.

**Authoritative World self-toss wiring remains open.** The callback target
bridge is now corroborated on an actual script69 event, and native raw
decoding has a real disc/capture consumer. World still needs configured
equipped/default script identity, per-player native body scales, authored
root-motion progression and the VM action timeline, plus state4 flight/catch
and snapshot integration. The current generic trick action duration and
renderer in-place playback do not implement script69's 6+14+28+6 update
sequence. The existing mirror/state fixes are production behavior; a
validated target test is not a claim that the runtime toss is wired.


## Native self-toss production bridge inputs

The player constructor `1563a8` initializes the live selected hand at
`156638` to **2**, independently of the database hand preference placed at
actor `+c0`. Holding the ball updates `+25c` at `14cd54` only for an
exclusive hand flag: state8 selects1, state4 selects2. A both-hand state
retains the previous selected hand. Script69 selectorF8 therefore needs
snapshot-owned live hand state, rather than choosing a row from the
player's database hand trait.

`315790` supplies `3153d0` with database fields10 (height),11 (weight),
2b/2c/2d (head morph parameters). The normal constructed player model
uses the weight/body-shape branch; the special prebuilt model path uses
uniform height scale. Height scale is `(height * 1.2) / 78`. The shape
branch uses build-specific base weights `[215,218,254,150]`, base radii
`[1.215444,1.215444,1.246152,1.2]`, and expansion factors
`[1,.875,.75,.875]`; preserve the raw multiply/divide order rather than
algebraically simplifying the formula. Raw `315628..31563c` writes the
full scale vector `[heightScale,bodyScale,bodyScale,1]` through `19cd30`.
The tiny-player option halves the first three components afterward;
head-size options modify the separate skeleton head scale fields.

Fractional QFast sampling is also required by authoritative flight/catch
poses: raw `38d7bc..38d7c0` stores the floating interpolation factor's bits
(the decompiler's integer cast is misleading). `38d834..38d84c` subtracts
current from next, multiplies the four components by the factor, and adds
current. It performs **linear component interpolation without quaternion
normalization**. A compact production configuration must retain the raw
22-bone tracks and hierarchy, not linearly interpolate finished hand
positions. The existing integer-frame capture regression verifies the
callback target; it does not establish fractional frames or the initial
six-update blend from a prior live animation.


The native constructor and reset path choose an idle row through RNG%5,
using the runtime-initialized `3d65c0` table (the ELF contents are zero
before initialization). Raw `178ca8..178e0c` supplies rows
`[66,50,52,53,67]` and rates `fps/60 * [5,6,5.5,5,5]`; constructor
channel duration is `ceil((frameCount-1)*rate)` and its playback speed is
`1/rate`. The observed script69 began from row88/frame6 instead, so
startup idle alone does not establish a continuous live channel bridge.
The initial `8e` blend clears channel root-lock bit1000 and, because
script69 requested bit800000, calls heading matcher `196698`. Bit10000
for vertical root compensation remains inherited. Opcode97 clears all
four actor physical-motion lanes after the initial blend. A standalone
frame0 seed or continued generic trick walking would therefore invent
production behavior. Continuous held/movement channel state remains the
prerequisite for authoritative script69 startup; no unused seed/config
API has been added as a substitute.


The captured pre69 row88 is `RUN_CYCLE_WBALL_05_3_R`, whose native gait
record is index104 in the 108-entry runtime table at `4b8790` (record
`4b97d0`). Read-only PINE words give script `443eb8`, rows88/89, kind1,
fields10/14=-1, threshold `.5903900265693665`, span
`.2999333143234253`, speed `.36666664481163025`, flags1002. The script
uses F8 live-hand selection for rows88/89, blends for six updates, and
jumps to the shared loop at `456326`. That loop executes opcodeAB with
float-1 then waits at81: `197168` sets infinite looping flags2008 and
uses the live channel+3c frame step. Captured PC `45632b` therefore does
not imply that the row0 prelude of the separate `4562e2` script ran.
Read-only live script bytes agree with the ELF; no operand patching was
observed. Native `168b30` retains a currently eligible gait row; otherwise
it filters the 40-byte records and draws a candidate using native RNG.
The renderer's generic held run clip is a different native row. These
selection and frame-step producers must be connected before claiming
authoritative ordinary-channel state at a later script69 start.


## Continuous gait bridge: exact next probes

`168b30` first consumes native RNG for `%100 < 50`, even if an eligible
current row later returns without a new candidate draw. Record eligibility
requires `record+0c & categoryMask != 0`, `record+14 <= handlesRating`,
and all requested flags among
`1,2,8,10,20,40,80,100,200,400,800,1000,2000,4000`. Bit4 is not tested.
`163d40` derives categoryMask from displayed DB weight field9 and gender
field12: weight<240 selects male1/female2, otherwise4. Handles/speed
come from `171670` -> `1d7d00` ratings7/8 (DB fields23/24), including
clamping and rating options. Record+10 is a player identity restriction,
not a body-build index: the DB virtual45fc70+14 -> `1d78c8` returns the
combined player record index. Eligible current rowA/rowB returns first.
The scan retains earlier generic candidates; after a player-specific
match, later generic candidates are filtered by the first RNG gate and
record+14=-1. Nonempty candidates require a second native RNG draw.

Raw `167c18..167c30` computes per-update motion magnitude at actor+190
from record+1c/+20 interpolation. Raw `167cd0..167ce8` computes
`(record+18 / actor+190) * bodyScale.x`; `167e34..167e3c` stores its
reciprocal at actor+32c. The channel receives that frame step at +68/+3c
only when blend flag200000 and flag10 are both clear. A production gait
bridge must also recover the `163d40` flag-family decision, contact and
defensive movement branches before treating arbitrary existing sim gait
values as native selector inputs.

Next bounded read-only packet should include actor+88/+90 requested
heading/intensity, +a8/+ac actual heading state, +b0/+b4 motion heading,
+78 option/turbo bits, +190 physical magnitude, +194 branch state,
+1a0 held transition state, +32c frame step, +330 previous effective row,
+70 held-ball pointer, +25c selected hand, and channel+28 row variable,
+3c configured step, +48 flags, +50/+54/+58 rows/link, +5c integer frame,
+64 floating frame, +68 current step, +6c/+70 duration/counter, +90
previous local root vector. Preserve actor+e0 root compensation, +40
physical motion, skeleton+40 body scale, and local/evaluated main bones
in the same packet. Read six native RNG words4309f0..430a04 separately
and report non-atomic capture limitations rather than asserting a matching
call count from snapshots.

The native generator `3a16b8` uses six carry-add state words, whereas the
current simulation uses xorshift32. Seed routine `3a1790` initializes
those words through successive wrapping additions of
`f22d0e56,96041893,3df3b646,40dde76d,97327ae1,d1a9fbe7`. This recovered
algorithm does not justify claiming original RNG call order for the
existing simulation. The next raw function probes are `163d40` family
selection/physical producer, `168b30` ordering, `196698` quantized heading
match, `193300` held-ball blend offset, and `19a658` fractional root/pose
advance. No standalone seeded authority API was added without a real
gameplay consumer.


### Opt-in gait capture verification

`tools/pine_probe.py --gait` now records the above per-actor/per-channel
fields, six native RNG words per sample, and the 108 runtime gait records
once in report metadata. Existing captures without this flag preserve
their output shape. Python compilation and a six-actor reader fixture
verified the optional/default paths. After the parent restored original
emulator state9, a read-only 240-sample capture at interval `.015` seconds
produced external `street-native-gait-state9-active.json`: 240 active
game/six-actor/stable-root samples, 1,440 actor/channel gait packets,
240 RNG packets and 198 selected-player full rigs. The first preceding
120-sample attempt had no active game; it remains a separate external
report rather than being counted as gameplay evidence.

External `street-native-gait-state9-comparison.json` retains all compared
rows. For 50 selected-player observations with a matching positive-root-
distance gait record and no channel blend200000/flag10, the recovered
frame-step formula with f32 rounding at each native arithmetic operation
matches actor+32c exactly in all50; channel+3c also matches all50. Current
animation rows have 675 candidate-record observations; 658 match the
held/nonheld flag bit implied by actor+70. All17 other observations are
retained, not discarded. This limited flag check does not establish the
full category/rating/family selector or original RNG call order. Every
adjacent sample has a changed global RNG state, and packets remain
non-atomic despite stable root pointers. No guest memory writes or input
were used by the capture.


### Gait family hysteresis and physical producer

Raw `167480..167554` establishes analog-intensity hysteresis, rather than
a direct turbo-button family map. Current-record flags1000 use lower40/
upper59; flags800 use39/61; flags400 use41/60; otherwise40/60. Intensity
at or above upper selects1000, below lower selects400, otherwise800.
The required mask also adds held2/nonheld1. Human input `1587d8` writes
actor+90 from the maximum of analog magnitude and rounded directional-
button magnitude, and `163d40` caps it at64. Turbo bit actor78&2000
separately multiplies physical magnitude by
`1.25 - min(speedRating/99,1)*.1` only while the corresponding turbo
bar is positive. Existing sim Run/Turbo labels cannot be substituted
for these native family inputs.

Raw `167d88..167db4` produces horizontal physical motion as
`X=-actor190*Sin(actorB0)`, `Z=-actor190*Cos(actorB0)`. `18ccc0` uses a
257-float quarter-wave table at440ea0 with native 1024-angle wrapping;
`18ccc0` is Cos(angle); `18cd58` is Cos(angle-256), hence Sin(angle). This write skips rows48/49 and preserves
existing nonzero X/Z when the actor is the state4 target. Native option
and post-update friction/contact branches remain separate. Offline
comparison against the original raw trig table in external
`street-native-gait-physical-comparison.json` retains all675 matching-
candidate-row observations: 136 are bit-exact and 567 differ by less
than1e-5, with maximum discrepancy `.4179681981`. All108 larger errors
remain in the report; no action, friction or cross-tick explanation is
asserted as proven for them. The family, RNG, held blend, quantized
heading and root state prerequisites still prevent treating the generic
sim walking loop as an authoritative continuous native channel.


## Production native body/head scale consumer

The model-path flag is bounded by raw `315790`: it starts at0; the
team37 created-kit path and index>144 prebuilt paths jump to315fd0
without changing it; the normal morphed league path sets it to1. These
match the parser's League versus Unique/Created model variants.
`db::Player::native_body_scale` now reproduces the recovered body
formula in native scalar operation order and refuses invalid normal
build/height/weight inputs. It flows through assembled Look and
PlayerAssets into the actual spawned player joints. The outer player
transform retains uniform height scale; only bones1/5 localZ and13/17
localX receive width/height when width>height. Tiny-player options
continue scaling the outer transform uniformly, leaving the ratio
unchanged. Arbitrary kit choices without a recovered database model
keep their existing transforms.

The original315640 tail also establishes per-axis head scaling at
skeleton+4/+8/+c: normal league base factor1.2/heightScale, prebuilt/created
base1, multiplied by `(DB43/44/45 /100 +1)` in storage order.
Independent raw review at19d59c and the scale-array writer19d864 proves
bone21 applies these as XYZ=`DB44,DB45,DB43`. Parent corrected this
permutation before full checks. Raw19d6f4/19d738 additionally multiplies
the complete head translation XYZ by the applied X factor. Both origin
and per-axis scale now reach spawned bone21 through Look/PlayerAssets,
including the existing head-size option. This is a static consumer
mapping; runtime head-axis morph parity has not been established.
The legacy database field name body_scale is retained, with its
head-axis meaning documented.

The external active gait capture identifies each selected player using
actor+274. With `STREET_ISO` and `STREET_NATIVE_GAIT_CAPTURE` set, the
new disc test compares198 original body-scale observations: all198
normal league cases pass a1e-6 numerical bound. Maximum absolute error
is1.1920929e-7 forX and2.3841858e-7 forY/Z. This is **not bit-exact EE
arithmetic**: record56 demonstrates a one-ULP height discrepancy even
with the raw1.2 constant and multiply/divide order. No guessed global
EE rounding emulation was added. Runtime capture contains zero unique/
created cases; a separate disc-backed model-path test verifies uniform
prebuilt scale and distinct synthetic head-axis percentages. The render
regression verifies the four affected components, unaffected main/aux
bones, uniform/fallback paths, and uniform size-option behavior.
Targeted asset/render tests and both packages' strict clippy checks pass.
This fixes visible native limb/head proportions; staged hand sampling
still receives uniform scale and authoritative self-toss startup remains
open. It does not establish a complete continuous gait/root bridge.


## Fractional QFast display interpolation review

Clips backed entirely by supported QFast channels now retain raw decoded
quaternions through fractional component interpolation, as raw38d834..84c
does, then normalize at the Bevy display boundary. Previously normalized
integer keys were spherically interpolated. The raw-key lengths affect
interpolated direction; the synthetic unequal-length fixture distinguishes
both operations and covers clamping/wrapping. Unsupported native channel
formats retain the existing presentation decoder. Crossfades and physics
poses still need native continuous channel state and are not made exact by
this display change. Render-normalized quaternions are not native authority.


## Reviewed production integration, 2026-10-13 continuation

The earlier xorshift and missing authoritative trick-identity descriptions
above are historical. Production now uses the original six-word carry-add
RNG (003a16b8) and successive-add seed routine (003a1790). Snapshot version
10 stores all six words; the all-zero carry-wrap state is valid. The bounded
original-instruction interpreter tools/native_rng_reference.py executes the
actual ELF instructions rather than a copied arithmetic kernel. The ignored
production regression compared 106 original-instruction cases and 1,238
adjacent captured state advances, all matching, with no excluded pairs.
Runtime draw counts come from the last counter word; non-atomic reads and
uncaptured reseeding remain limitations. Native gameplay caller ordering,
probability formulas and the generic unit() conversion remain unresolved.

Direct trick masks now select snapshot-owned NativeTricks identities built
from the disc's default/equipped script tables. The selected script and native
value class are latched in Action::Trick and consumed by rendering; released
buttons cannot reselect a move. Known slots avoid the invented generic variant
draw. Native value classes index the point table directly, and native repeat
keys use script identity. Unmatched-input fallback selection and all VM action
timing remain approximate. Script69's flight/catch still is not wired in World.

Raw human input 00158ddc..00158de4 preserves turbo bits in actor+78, and the
raw selector 0015f1dc..0015f2d8 contains no bar-budget gate. Direct known mask
selection therefore uses the retained action buttons even when the generic
turbo budget is empty. Upstream virtual input production remains unproved;
this is a bounded correction, not complete empty-bar native action parity.
The original selector raw supplement is retained externally as
%TEMP%/native-trick-selector-authority-raw.txt.

Staged hand endpoints now retain full f32 base positions and a linear width
basis. Height and width use the same native limb translations as rendering;
head bone21 is not an ancestor of either hand. The canonical GBCv5 payload is
losslessly byte-plane/XOR compressed, bounded to 128KiB encoded and 1MiB
inflated. An actual six-player configuration was 117,604 encoded / 167,249
inflated bytes. Online protocol7 permits 255 bounded fragments and a 192KiB
world payload; reverse-order transfer of a large setup with native identities
passes. Snapshot checksums include canonical configuration content.

The endpoint evaluator 00330f98 computes channel frame+speed*time then uses
CVT.W.S at00331104 before integer pose sampling. The PCSX2 2.8.2 execution
path truncates this conversion. Production endpoint lookup now truncates before
wrapping, replacing nearest-frame lookup. Independent review compared
8,601,120 quarter-frame samples over 240 database players and 121 clips against
truncated production joint sampling: maximum coordinate error7.6293945e-6.
This compares the geometry bridge, not original live continuous actor poses.
Previous nearest lookup could differ by43.5462 court units at clip wrap.
Native actor display retains fractional phase; its production correction is a
separate ongoing trace. The external review and source hashes are retained in
%TEMP%/street-staged-geometry-review/review.md.


The direct repeat key was independently traced through15f334/348 (selected
script return),15f5c8/600 (same index stored at actor+264), and1438c4/974
(recent ring). The earlier label "clip id" was misleading for this path.
The review also found a real rounding error: original14397c..1439b8 rounds
the final discounted award upward and clamps to one. Production now uses
ceiling division for the bounded table/four-slot ring; four repeats of native
classes3/4 award188/313. The regression covers both and snapshot restoration.
The empty-budget regression now covers all11 direct slots through completion,
base score and120-step resumed checksum replay. Original actor+218 conditional
base halving and class<2 ring bypass remain unresolved fallback scoring scope.
Retained original award/selection evidence:
%TEMP%/native-trick-repeat-authority-review-raw.txt,
SHA2567637f3b83f6139127074a8e8cfe81523bf8a205cb3784c079abab2369ff696b5.


## DeltaQuat raw integer pose support

Bank::native_rotations now also retains original type11 DeltaQuat accumulated
values for the four lowcomp tracks. Original003943e8 copies integer-frame
quaternions directly; display normalization remains a separate API. Independent
raw review found and corrected operation order in the existing decoder:
0039565c/660/664 (16bit),003956dc/6e0/6e4 (8bit),00395860/864/868 (4bit)
multiply code by scale, add minimum, then add current. Production uses the same
order; an exact-bit cancellation regression distinguishes it from adding current
first. Both lowcomp disc tests pass. Fractional DeltaQuat interpolation and EE
rounding are not established by this bounded integer decoder change; type19
SingleQ and raw pose formats remain unsupported by native_rotations.
Retained independent original evidence:
%TEMP%/native-delta-quat-order-review-raw.txt,
SHA256faa57ef206d1e2816fbc9a685c67de2094d1cb6c6271289a8f94f5b2fb9e492e.


## Staged actor fractional display correction

Raw original32b458 staged player proxy writes the configured frame step to
channel+3c/+68 and launches one of VM bodies456c34/45/5b/71. Their immediate
89 or six-update8e animation selection proceeds through loopAB or finite8C.
Original19a954..19a968 advances floatphase+64;19add0/19add4 passes this float
to rotation sampler192438 and19adf4/19adf8 to translation1925f0. Integer+5c
is a separate event/state key. The display path therefore retains fractions
independently of prospective endpoint330f98.

Production sync_players now preserves fractional staged phase. The actual
sync_players→pose_skeletons disc regression checks script0 tick7 slots0..2,
bothteams,12hands: fractional normalized Clip FK error0, all12 differ from
nearest-frame FK, max difference0.12726173 model units. It starts with the
same clip already playing and excludes terminal wrap, isolating phase assignment.
This is a renderer regression with original disc inputs, not live native-pose
parity. Startup offset/task ordering, six-update inherited-pose blend, finite
versus looping terminal policy/reverse playback remain under trace. Native
looping last-to-first blend19aa38..19aadc and raw quaternions differ from the
current normalized display boundary. Evidence and original assembly retained
in %TEMP%/street-staged-display-native/findings.md.


## Native trick full-base qualification

Raw actor+218 writers1375b8/1377d8/15cda4 latch a successful defender reaction.
Constructor156908 initializes zero. Update156f48 observes current actor+ a4
bit400 cleared after previous+208 bit400 was set; it dispatches DAT3c6550 at
157e40..157e68 before clearing+218 at157e6c. Synchronous190d90→104cb8→132198
invokes1438b8 while the flag is still available. Original base value is full
when this latched flag is nonzero, half otherwise, then upward-discounted by
the repeat count. The current native-identity World adapter now applies half
for its plain result and full for its newly successful fakeout. That repairs
base arithmetic but deliberately does not claim native flag persistence or
VM award timing; the existing fakeout producer remains approximate.

Restored portable state9 internal read-only capture retains1000active samples,
6000actor award/gait packets. Actor1 has one62-packet flag1 interval at
samples396..457; its falling transition at458 accompanies a class11/extra2
ring value2000. Other distinct captured class10 extras3/2/4 carry1500/1000/2500.
The observations support qualification arithmetic but are non-atomic and do
not prove an instruction-level event boundary. Current regression covers
plain fourth-repeat awards94/157 and qualified188/313, with snapshot replay.
Capture %TEMP%/street-native-trick-awards-restored9.json contains explicit
restored-state provenance and is not fresh-boot or PS2 hardware evidence.

### RawPose integer quaternion authority (2026-10-13)

The native rotation API now accepts quaternion-only FnRawPoseChannel tracks.
Original setup 00392480 binds signature kind 0 to direct reader 00392c00;
that reader copies four float words without normalizing them. The decoder
preserves these words, including signed zero, while display sampling retains
its previous normalization. Native Euler-kind RawPose remains unsupported;
host trigonometry is not substituted for the original 00392ab8 reader.

The disc-enabled exhaustive check compares all 6,072 quaternion component words
in the two shipped ANIBIG nocomp tracks (41 and 28 frames, 22 bones). All match
exactly. A separate malformed/signature and non-unit-word regression passes.
This establishes integer decoding from disc bytes and the statically recovered
copy reader, not fractional RawPose interpolation or live multi-channel poses.
Original raw reader and source hash evidence is retained outside Git in
%TEMP%/street-raw-pose-root. Next probe: original fractional reader 00392ef8
and Euler conversion 00392ab8 before expanding those authoritative domains.

The consumer now retains an explicit RawPose interpolation policy instead of
classifying every native integer track as QFast. Original 00392ef8 delegates to
VU helper 00379b20: compare squared lengths of second-first and second+first,
then normalize second*t plus or minus first*(1-t). The negative-dot branch
flips the first endpoint; raw endpoint lengths contribute to the blend.
Production display follows this recovered formula using host arithmetic.
Unequal-length and negative-sign fixtures distinguish it from normalized-key
SLERP and the QFast component formula. Exact VU accumulation, reciprocal-square-
root rounding and live fractional pose comparison remain open; this is a
structural formula correction, not bit-exact fractional authority.


### Ordinary movement asset prerequisite integrated (2026-10-13)

`street-assets::gait` now reconstructs the108 ordered records built by
00163d40/00191410, plus callback52's144 ordered classification pairs and
callback15's158 membership words. The bounded evaluator verifies the exact US
ELF identity, ten-store callee, delay slots, defined registers and declared
60 Hz time input; unknown instructions/calls/inputs fail. All1,080 constructed
words match both retained and fresh original tables. Truncating multiplication
removes93 one-ULP nearest-mode differences. Both divide policies give identical
words here, so this table does not establish division rounding mode.

Production `setup::load_clips` retains the extracted data in GameClips and its
actual-disc regression resolves every216 referenced animation rows. Seven
focused actual-disc tests and strict assets/game lint pass. Parent independently
reran the table tests against both captures; independent review validates all
source hashes, ordered table oracles and unchanged Dunk parser semantics.

This completes immutable data loading, not ordinary gait execution. Next is
continuous actor/channel construction and VM dispatch on the same channel used
by the rig. Preserve RNG idle phase, outside-update wait2, opcode83's PC advance
before callback32 replacement, retained interrupted blend alternate/key fields,
and97's actor64/60 writes. Do not seed production from observed row88/frame6.
Shared RNG caller order, real ownership/input/rating producers, opposition and
contact branches, root/held offsets, FC0 tracks and outgoing/linked poses remain
open. Evidence is retained outside Git in the continuous-probe directory.

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
