# Player speech recovery

This note records the static trace and reconstruction of player action speech
for the USA `SLUS_206.51` image in the `street2ee` Ghidra project. These are
observations from Ghidra's analysis of the executable; no target execution was
used.

## Lookup path

`FUN_001ad068` is the player-line request entry point. It suppresses requests
while the Script Manager is in states 3 through 7 and while the manager's
speech-blocking globals or per-manager flag are set. For an accepted request,
it reads the selected player's entry from `param_1[param_2 + 0x0c]` and posts
event type `4` with the category mask, mood, and priority. The normal category
mask is `1 << category`.

The speech callback `FUN_001ac9c8` passes the event packet to
`FUN_001ac9f8`. For packet type zero, `FUN_001ac9f8` resolves the player record
from packet word 0 with `FUN_00363318`, then passes packet word 1 unchanged to
`FUN_001af1e8`. For the `0x2002` player-record kind, `FUN_001af1e8` calls
`FUN_001af230(0x2002, record_voice, packet_word_1)`. A nonnegative return is
the PLAY selector slot passed to event `0x1f`.

`FUN_001af230` first bounds the 16-bit voice number to 1..151, subtracts one,
and loads a code address from the 151-entry table at `0x45d970`. The selected
voice tree compares packet word 1 and returns a stream index. The decompiler
timed out on this 88,300-byte function, so the mapping was recovered from its
raw MIPS instruction listing and the table pointers.

The header start is stored as a big-endian `u16` in units of `0x100` bytes.
The lookup key is that start multiplied by `0x100`; it is not the line ordinal,
category, or mood. For voice 1, `action\001.hdr` begins with starts `0x0000`,
`0x0039`, and `0x007d`. The tree compares keys `0x0000`, `0x3900`, and
`0x7d00`, returning stream indices 3258, 3259, and 3260. The first voice 8
header line returns stream 3586, matching the existing disc anchor.

## Reconstruction and checks

`crates/street-audio/src/speech_lookup.rs` contains the exact `(voice, header
start) -> stream` results from the per-voice trees. It covers all 22 action
header voices in the disc data (1..16 and 145..149, 151), for 3,196 lines. A
static MIPS branch-and-delay-slot walk over each tree and each header key
resolved all 3,196 lines to distinct selector slots; the highest result is 3968. The focused disc test parses
`SPCH_PLY.AST` and checks that every index has nonempty stream metadata and
byte bounds inside the AST. The `PlayerSpeech::load` path now
uses this table directly instead of searching stream lengths and guessing
forward or reverse runs.

The focused check is:

```powershell
$env:STREET_ISO = '<path to>\the original game (USA).iso'
cargo test -p street-audio --test audio_disc the_player_lines_find_their_streams -- --nocapture
```

It checks that all lines are placed, selector values are unique, and the voice
1 and voice 8 anchors match the executable's return values.

## Candidate selection and remaining limits

PLAYERS.EVT root 0x2000 at byte 0x1c branches through word offsets
4 and 14 to records at 0x2c and 0x54. Their child descriptors at 0x38
and 0x60 name header types 0x2002 (action) and 0x2003 (shot).
`FUN_00364938` checks the category against mask 0x7fff, and the
second metadata byte against mask 7 (action) or 0x3f (shot), then
intersects those bits with the corresponding request words. For action, selectors [2,4] mean
category against packet word 2 and the second metadata byte against
word 4. The latter is the third `FUN_001ad068` argument, not a direct
mood ordinal or the previously assumed priority value.

The observed action requests use mask 1 for categories 0,1,2,4,6,7,8;
mask 2 for 3,5,14; and mask 4 for 9,10. Unknown categories have no
inferred default. These masks are now wired. The trick handler's
request `(2,8,0)` therefore selects category 3 with second metadata
value 1. Its nonzero event+8 gate still controls crowd 0x29 and speech.

Header byte +6 is a five-entry recent-line ring for every action voice.
The mode-0 selector gathers fresh eligible lines; if none are fresh,
it chooses the oldest eligible line. The shared six-word random generator
and 32-result keyed ring are ported, including short-circuit carry
assignment on overflow and the native 33-slot scan boundary.

The shot descriptor instead uses selectors [2,3]. All 22 shot headers
have ring length 4, category 14, and second metadata values 0,2,4,5.
Their callback takes `FUN_001c4b20`, not the action lookup. All 773 shot
lines now resolve through that native tree too, giving 3,969 distinct
selector values across both catalogs (0..3968). The event chooser now tries the two equally weighted branches in a
randomized order without replacement, preserving both ordering draws
and each branch's chance draw. Their weights are byte 0x39 decoded as
25 times scale-table entry 4 at 0x422bac, giving 100 each. Launch
requests use category 14, shot mask 1 and action mask 2; shot lines
with metadata 0 are eligible, with a separate four-entry history ring.
Both managers draw from the one global 6-word state at
`0x423038..0x42304c` (`FUN_00366c80`, `FUN_00366d70`). Since 2026-10-15 the
rewrite keeps a single `SpeechMiddleware` in the session's
`audio::SpeechManager`; see "The speech middleware, re-read" below.

The independent raw-instruction check is reproducible without Ghidra:

```powershell
python tools/verify_speech_lookup.py <path-to-user-ISO>
```

It reads the unpatched executable and every action and shot header from the ISO,
executes each lookup including MIPS branch delay slots, and compares all
3,969 action/shot results with the Rust tables. It exports no disc assets.

## PLAY resolution and startup ordering

Lookup results address the player module's PLAY sound list, rather than the
AST directly. `SPCH_PLY.ABK` module `0x1f6e`, node 27 has 3,971 list slots
for 3,969 streams. Slots 2173 and 2174 alias streams 2139 and 2138;
subsequent slots map to stream ordinal `slot - 2`. For example native shot
lookup 3244 resolves through list value `0xccaa` to AST stream 3242, at
byte offset 49,441,408. Original unpatched instructions at `0x34f538` read
PLAY input 6, `0x34f540..55c` clamp and index the four-byte sound list,
`0x34f564` masks its returned sound with `0x3fff`, and `0x34f568..574`
multiply by twelve and address the ABK stream table. This independently
corroborates the selector indirection without relying on patched decompilation.
Disc tests now follow this table and check its
stream offsets against AST metadata. The two unused trailing selector slots
and alias entries mean 3,969 unique lookup results do not prove that each AST
stream is reachable exactly once.

The crowd controller supplies speech volume through event 27, parameter 5:
SPCH_PLY PARAM `0xa1f8c` feeds SCALE 106. In CRD_RUCK owner 40 module
`0xf549`, SEND node 107 obtains that value from SCALE 106/RAMP 79.
Crowd-loop event 32 start and release drive EDGE 40; SUM 43 adds one to its
output and SELECT 59 chooses among 32767, 32767, and zero. Coalescing the
start and release before a graph tick lets the start pulse win and holds
speech volume at zero. The no-intro startup now completes the pending
10 ms AEMS tick before releasing the crowd loop, preserving the native
intro's ordering without changing gain. Partial ticks advance only the
remaining time. Catalog and control-flow recovery still require an original
runtime comparison; local PCM tests alone do not establish parity.


Focused startup checks cover callbacks in the same frame, halfway through a
pending tick, and after a complete tick. They assert EDGE output 1, SUM
output 2, SELECT output 32767, and the exact four-input SCALE volume product.
The speech integration check waits one second of normal 10 ms graph ticks
for the court ramps, then tests the native chooser's shot and action lines
separately. It resolves the actual PLAY sound, excludes crowd and effects
voices, compares rendered gains with the graph's volume/level formula, and
requires nonzero PCM. The former invented RMS floor of 0.002 was removed:
at early startup four simultaneous ramps legitimately produced gain
0.0017395549 and isolated shot RMS 0.00032250167. This is a timing context,
not evidence for boosting the native volume. The isolated consumer reports
completion through the renderer's bus so the one-voice module can accept the
next request. No production gains or ramp values are overridden.


## Trick commentary event 4

`FUN_001a5a78` tests the event word at +8 before both the crowd/player
speech request and Bobbito's setup. Its normal commentary request calls
`FUN_001ad178(manager, 4, 1, actor_role, opponent_role, ...)`.
`FUN_001ad178`'s event-4 case dispatches exactly three words; the computed
move-kind argument is not part of that packet. `FUN_001a82d8` returns 3
when player property 0x0c is positive, 5 otherwise, and -1 for an absent
player. This property is sex (the `female` database field), not human
control: `FUN_001a8240` reads property 0x0c, and the player/property
mapping and Gender: Female UI at `0x2cec70` corroborate it. Mask 3 permits
neutral and female metadata, mask 5 neutral and male metadata.

The opponent is the nearest opposite-team player found by
`FUN_00170dc0`, with a strict distance below 1000 raw court units and
ascending-index tie handling. The distance cache builder `0x13fbc8` calls
`0x18cdb0`; original instructions at `0x18cdb0..18cdd4` read only X/Z,
square their differences, and take the square root. The implementation
uses the same metric directly in the simulation's court units. No audio
coordinate conversion belongs in this cache-distance path.
It does not substitute the defender who happened to be faked out.

`COMNTARY.EVT` root 4 at byte 0x944 has three weight-100 branches, chance
100, to header types 20, 21, and 57. The headers are `Move\MoveA`,
`Move\MoveB`, and `Move\MoveC`, containing 56, 56, and 39 lines with
recent rings 12, 12, and 8. The first two descriptors select packet
words [1,2,3,0], the third [0,2,3,0], against masks [1,7,7,15].
Their big-endian header starts address exact byte offsets in the matching
DAT files. The old fakeout-only, random Move-file request with invented
40-percent probability has been replaced for `Event::Trick`. Its player
category-3 request also removes the invented 40-percent caller gate: the
original handler posts it directly and the EVT chooser owns its chance.
Requests with caller chance 100 bypass the separate effects RNG gate; real
fractional caller gates retain their draw. The isolated disc integration test
checks both: seed 7's rejected 40-percent request advances only the effects
RNG, while the following direct launch leaves that RNG unchanged, advances
the native EVT selector RNG, and posts the exact selected speech stream.
The current event producer starts at at least 500 trick points and halves
at most four times (its four-entry repeat ring), so every emitted trick
has positive value; the native +8 nonzero gate is satisfied on this path.
Dunk releases still carry zero through the separate SFX path.

Native branch ordering consumes every positive-weight order draw, including
the last remaining branch: raw `0x3650d4` calls `0x366d70(total,-1)` and
`0x365138..36513c` subtracts the chosen weight and loops while the total
is positive. Thus event 4 consumes order draws with ranges 300,200,100,
then a chance draw and a keyed candidate draw for the winning branch.
The disc test checks those state transitions, all 18 sex/opponent candidate
counts, every header offset against actual SCHl metadata, and 600 requests
against metadata and recent-ring predicates. The nearest-opponent test
checks teams, ties, and the native range limit.

This recovers the normal Move catalog and packet routing, not the entire
commentary manager. The nickname interception of that route is now recovered
below. The middleware's queue/priority timing, global blocking flags,
cross-manager RNG draws and header-key registration, and ordinary commentary's
local playback cooldown remain unresolved. Other commentary categories still
use the earlier approximate prefix selection. No runtime parity is claimed
for those paths.


## One-shot nickname interception of a trick

`FUN_001a83e0(..., 5)` is a nickname-action predicate, rather than event 5
or a street-challenge lookup. The constructor `0x1d1e48` selects a row of
`XDBALIAS.ADF`: loader `0x1da250` calls `0x1d1fd0` for that file, and the
latter uses the table returned by `0x1d1f70`. Raw `0x1a84c0..1a84d0`
loads positive actor property 6 into argument 1 of that constructor.
The property is the nickname row already copied into the active star's
player data. Alias field 2 must equal 5 (Handles), after which alias field
0 becomes packet word 1 of COMNTARY event `0x12`. Actor sex mask 3/5 is
packet word 2. A successful predicate suppresses the normal event-4 Move
request; the independent player taunt still follows in the original handler.

The predicate rejects Script Manager states 3..7, an active Bobbito voice,
non-Be-A-Legend mode (native mode 5), a cleared pending bit, nonpositive
actor property 6, and a nickname whose skill does not match the action.
Rust uses the existing staged-sequence gate for both normal Move and nickname
requests. Actual Bobbito playback entities carry a marker used to refresh the
playing gate; a newly queued voice sets it immediately in the same frame.
The special nickname request does not use the earlier invented extra 0.4
second cooldown and does not skip its attempt merely because speech volume
is zero. Normal commentary's older timing model remains a separate open gap.

Native `0x1e70e0` sets profile byte `+0x19`, bit 0, when it assigns a new
nickname at reputation tier 3. Raw Career construction `0x1e693c` supplies
length `0xc88` and `0x1e6954..1e6958` binds the serialized payload at object
`+0x18`, proving that this pending bit is original save byte 1, bit 0.
It is separate from whether a nickname has been earned. `0x1a854c..1a8560`
clears the bit after the matching event-0x12 attempt, even if the event manager
finds no playable candidate. The new `nickname_pending` career field follows
that lifecycle, imports the original bit, and persists in optional `NKPN`
records; older Rust saves default to false. Audio clears the active field
and calls `Save::store`, preserving the user's autosave setting.

`COMNTARY.EVT` root `0x12` at byte `0x1570` has two equal weight-100,
chance-100 branches. Descriptor kind 159 resolves a nickname-specific neutral
header using packet word 1, with metadata selector [0] and mask [15]. Kind
213 resolves the alternate headers for IDs 1,2,4, with selectors [2,0] and
masks [7,15]; those lines require male metadata value 2. All starts are zero
in their matching one-stream DAT files. There are 46 neutral audio headers
and three alternate headers, while the nickname database has 48 rows: audio
ID 48 has no corresponding database nickname row. The parser preserves that
asset distinction. Nickname and Move selection now share one local native
middleware RNG and result ring. Original global interleaving with player
speech and other managers is still not established.

Disc tests resolve all 48 database nickname IDs for both sexes, verify the
male-only rejection, decode the exact selected PCM, and prove that nickname
selection advances the state subsequently used by Move selection. Audio
checks matching versus unrelated skill, mode, pending, scripted, and positive
row gates. A live-playback entity test verifies that busy Bobbito retains the
pending attempt, muted matching speech still queues its exact nickname voice,
and a no-candidate attempt suppresses Move without queuing a replacement.
A separate positive-control test confirms that staged playback blocks both
requests and that the same ordinary Move request posts a voice when idle.

The pending nickname interception now also covers Blocks then Power
(`0x1a4ce8`), held-ball Steals and successful counters (`0x1a63e0`), and
loose-ball pickups as Rebounds (`0x1a65c0`). These are rooted in the native
event constructors and listener dispatch, rather than inferred event labels:
`3c6580 -> 104d58 -> listener+84 -> 1a4ce8`,
`3c6570 -> 104da8 -> listener+7c -> 1a63e0`, and
`3c6590 -> 104df8 -> listener+8c -> 1a65c0`. Native `14cb28` posts the
Rebounds event for old ball states 0/6 becoming held, and the Steals event
for an opposing-team transfer from held state 3. `14c408`/`14cb28` post
Blocks for an opposing deflection from shot state 5. The rebound caller has
no ordinary Bobbito fallback; existing approximate Block/Steal commentary
remains the fallback when the native nickname attempt is not consumed.
All added attempts retain the mode, pending, alias skill, staged playback,
and actual commentary-busy gates, and clear pending after the attempt.

Pass interception deliberately retains ordinary Steal commentary and does
not consume pending nickname. Native old airborne state 1 has additional
flags and previous-owner branches that are not represented by Sim's single
interception event. Sim now carries `Steal.intercepted_pass` directly from
the two emitters, false for a held-ball swipe and true for an intercepted
pass. This survives World snapshot version 8, so coalesced fixed ticks cannot
misclassify the event using a later player action. Focused tests exercise
both actual emitters, round-trip both origins, and validate every positive
alias row against the accepted skills from actual disc metadata. Native old
state 4 can also post Steals on an opponent's incoming catch; Sim has no
corresponding distinct event source yet.

Score nickname callers are recovered from `0x1a3b10` and `0x1a83e0`: on dunk and
alley-oop scores (`iVar4` in {3, 4}), `FUN_001a3b10` checks Dunks (skill 1) then
Power (skill 2). For other made scores (jumpers, layups), it checks Shots (skill 0).
These routes are wired into `nickname_action` in `crates/street-game/src/audio.rs` and
fall back to standard score commentary ("Dunk\", "Shot\") if unconsumed.
Validated against all disc nicknames in `disc_nickname_action_routes_only_proven_possession_events`.
The shared native request queue, callback arbitration (including the player
callback's Bobbito-playing guard), other Script Manager/global flags, and
RNG interleaving remain open. This is static recovery plus Rust integration
verification, not an original-runtime speech comparison.

## The speech middleware, re-read (2026-10-15)

This section supersedes the three Gemini-era sections that stood here
(ordinary commentary timing, "speech-blocking globals", "request queue
priority"). Their key claims were misreadings; the corrections, from the
decompiled functions checked against raw MIPS where it mattered:

- **`param_1[0x17]` (+0x5c) is the speech pause flag**, not a busy flag:
  `FUN_001ad958` sets it (with the stream reset and `FUN_00364190`'s
  queue clear), `FUN_001ad9d0` clears it. Bobbito playing is
  `param_1[5]` (+0x14): set by the callback when a line starts, cleared by
  `FUN_001acf58` when the stream reports no data left. `param_1[0x1a]`
  (+0x68) lets a new commentary line start over a playing one; the
  initialisation and request 6 set it, listener slot 22 (`0x1a5d10`, the
  tip-off's take) clears it. `param_1[0x13]` (+0x4c) is the speech volume.
- **`DAT_003e9ae8`, `DAT_003e9ba8`, `DAT_003e9bc8` are debug switches**
  registered at `0x1a2770..`: `audio_no_sound`, `au_no_player_speech`,
  `audio_no_commentary_speech` (each object at `0x3e9ad0 + 0x20k`, value
  at +0x18, stored zero). They are not the speech volume option.
- **The `1000` that `0x1ac9f8` passes to `FUN_0019e840` is the AEMS
  event's level**, as at every other `FUN_0019e840` call; there is no
  1000 ms hold on player speech. The rewrite's `talk_free` hold and every
  caller-side chance (80, 70, 60, 15, 50) are gone, except the 21-second
  shot clock call's own `rand() & 1` (`0x1a5cb4`), which the rewrite draws
  from its effects generator.
- **Requests** (`FUN_00363a10`): the root's chance byte (+9) against one
  `FUN_00366d70(100, -1)` draw, compared as signed bytes, then a slot
  (`FUN_00363898`): the first free one, else the first request older than
  its root's hold (+2, VBlanks), else the first request of the same manager
  whose root priority (+4) is at most this one's. A sequence number counts
  requests within one VBlank (`gp-0x7dbc`).
- **Service** (`FUN_001acf58` each frame: `FUN_00364620(0)` then `(1)`):
  `FUN_003644c0` drops the manager's expired requests and takes the best
  (`FUN_00363d58`: highest priority, then the youngest, then the highest
  sequence), drops every older request of the manager (`FUN_00364350`),
  and runs the chooser (`FUN_00366320`). Every speech root has hold 200,
  priority 500 and no flags, so the newest request of a frame wins.
- **The chooser**: `FUN_00365020` orders the branches by weight without
  replacement (every positive-weight draw taken); each branch takes its
  chance draw (`draw < chance`), finds its header (key type 0: the first of
  the type, `FUN_003633a0`; key type 1: the type and the id in request word
  `+2`, `FUN_00363408`), gathers (`FUN_00364c38`: a request word sharing no
  bit with its column's header word rules the header out; each line must
  pass `FUN_00364938` and be enabled, `FUN_00364af0`; fresh lines up to 200,
  else the one chosen longest ago by `FUN_00363480`'s age) and picks with a
  keyed draw (`FUN_00365928`, the header's table slot the key). The posted
  result (`FUN_00365ba0`) switches a partitioned line off and pushes the
  ring (`FUN_00363538`) before and after the callback, whether or not the
  callback plays it.
- **Headers live in one table**: `0x1ac6f0` registers `plyrhdrs.big`
  (slots 0..43) and then `spchhdrs.big` (44..), `FUN_00363060` taking the
  first free slot and refusing a header whose ring is not clean. The ring
  (cursor, then +6 entries) and, with +4 bit 7, a partition bitmap (count,
  then a bit per line) live in the loaded header. `FUN_003635d0` enables
  run `seed % count`: lines `k*n/p .. (k*n+n)/p`. The seed is the manager's
  counter `param_1[1]`: `0x1ac5e0` starts it at `rand() % 255`, the first
  `0x1ac6f0` seeds the generator with it (`FUN_00366f40`) and registers,
  each later one only re-runs the partitions (`FUN_00363230`); each counts
  it up. gp-0x7dd0 (`0x4b3220`, gp `0x4baff0`) holds 1 and is never stored,
  so a used partitioned line stays off until the next initialisation.
  Block, Steal, Move, Dunk and most commentary headers are partitioned; the
  player headers and the nickname headers are not.
- **Commentary requests** (`0x1ad178`) pass when the manager is up, the two
  debug switches are clear, the game is not Street School (game +0x9c = 4),
  no staged script runs (event 3 excepted), and `FUN_001a0db8` is 0: the
  profile's Trax word bit 0 (interactive) set, or FE mode 1. The Playlist
  Music Experience silences the commentator. There is no busy gate here;
  `0x1ac9f8` type 1 drops the chosen line when paused, at speech volume 0, or
  while Bobbito plays without the override. Type 0 (players) drops it when
  paused or while Bobbito plays (no override), and plays event 0x1f where
  the last requesting player (`param_1[0x12]`) stands.
- **Packets**: event 0 (block, `0x1a4ce8`) is `[FUN_001a8310(blocker),
  sex(listener +0x5c, the last shooter), sex(blocker)]`; event 8 (steal,
  `0x1a63e0`) `[FUN_001a8310(stealer), sex(stealer), sex(victim)]`, every
  fifth steal (`DAT_004bb2d4`) event 0xd instead; slot 52 (`0x1a6d88`) posts
  0xd every third violation (`DAT_004bb2e0`), else 0x11 (ShotClock, root
  chance 75). `FUN_001a8310` is a created player's nickname id, 0 otherwise
  (and 0 in Be A Legend while the nickname's announcement is pending); events
  0 and 8 key their third branch, NicknameAction (type 66), by it.
- **Callers** (raw `jal 0x1ad068` immediates, tested): the block handler
  (slot 16) asks category 9 and the steal handler (slot 15) category 10,
  both with action mask 4; the rebound pickup (slot 17, `0x1a65c0`) asks
  category 0, or every other time (`DAT_004bb2d8`) a teammate's 1; the jump
  shot's launch (slot 11, ball kind 2) category 14 with shot mask 1. A pass
  posts class `0x3c6500`, whose listener slot 9 is an empty handler
  (`0x142ff8`): the rewrite's pass sounds and lines were mislabelled block
  and steal handlers.

`crates/street-audio/src/speech.rs` now ports this (`SpeechHeader`,
`HeaderTable`, `SpeechEvent::order/choose`, `SpeechQueue`,
`SpeechMiddleware::post`), reading every event, weight, chance, selector and
mask from the disc. `crates/street-game/src/audio.rs` keeps one
`SpeechManager` for the session (`init` per match, `speech_service` per
tick after the listener systems) and posts the recovered commentary events;
`sfx.rs` posts the players' requests. Rewrite-own choices, all labelled in
code: the counter starts at 0 (the PS2's `rand() % 255` state is not
reproduced), the queue clock counts fixed 60 Hz ticks for VBlanks, the
front end's own initialise-and-shutdown (`0x19fbb8`) does not advance the
counter, and situations whose native requests are untraced (baskets, pass
outs, turnovers, the outro, game point, the court intro) keep prefix
commentary drawn from the rewrite's own generator without queueing.

Tests: `speech.rs` (ring ages and wrap, partitions and use-up, gathering,
weighted order, queue service and eviction, the acceptance draw, the disc's
events and slots, Move/Block/Nickname choices against an independent draw
sequence); `audio.rs` (`match_sounds` and `speech_service` in a small app:
event 4 served with the exact middleware state, a busy commentator's line
chosen and used up but dropped, closed gates taking no draw, the steal and
violation counters, the manager outliving a match); `sfx.rs` (the newest
player request served with the exact state and its stream audible, the
call-site immediates read from the ELF).

Still open: the original-runtime comparison of queue timing and draw
interleaving within a frame (the rewrite posts the listener's player
requests in `fire_events` and the commentary's in `match_sounds`; within one
frame only the count of acceptance draws matters before the service, which
holds), the commentary callback's crowd loop 0x24 and music duck
(`FUN_001abde8`/`FUN_001abe78`), the pause flag during the pre-tip intro
(slot 57 `0x1a70a0`, whose sender the rewrite lacks), slot 27's missed-shot
requests (`0x1a4550`), and the intercepted-pass Steals post (`FUN_00140558`).

## The remaining commentary requests, the pause and the song rotation (2026-10-15, later)

A helper's pass, merged after review against the decompile (`FUN_001ad178`,
`FUN_001a3b10`, `FUN_001ad958`, `FUN_001ad9d0`, `FUN_00366320`,
`FUN_00365f48`, `FUN_00363018`, `FUN_001a81b0`, `FUN_001a85f0`,
`FUN_001aabc8`, `FUN_001aaf78`, `FUN_0019f810` re-read for the merge).

- *Every commentary request now goes through the native chooser*; the
  rewrite's own prefix calls (`say`, the Shot/Dunk/PassOut/Turnover/Outro/
  GamePoint files and the court intro) are gone. `FUN_001ad178`'s cases, with
  the words it packs: 0 Block, 4 Move and 8 Steal (3 words); 1 Dunk (7: the
  alley-oop finish 2 else 1, the dunk animation's three words, the sex, the
  nickname row, the manager's length word +0x60; the caller's fifth value
  `FUN_001a7e78` and its last `FUN_001a85f0` are dropped, the function
  taking nine arguments of the caller's ten); 2 the gamebreaker hype and
  0xb/0xf/0x12 (2 words); 3 the court (4); 0xc Shot (4: a shot of 3 points
  or more 2 else 1, the row, the sex, the length word); 0xe a miss and 0x13
  a slam (1); 6 the end, 0xd and 0x11 (none; 6 first sets the override
  +0x68 so the closing line plays over a call); 5, 7, 9, 0xa and 0x10
  return. The gates before the switch: a staged script (any but event 3),
  the Playlist Trax bit (`FUN_001a0db8`), the two debug globals and Street
  School. `0x1a3b10`'s basket branches: `DAT_004bb2c0` counts the
  session's baskets; the winning basket (`+0xbc`) posts 0xf; otherwise,
  unless both scores are under 4 or the game is a trick-point or
  gamebreaker race (profile +0xe8 1 or 6), every fourth basket posts the
  score 0xb (standing 1 ahead, 4 behind, 2 level, then the margin or the
  level score, from the first side unless it is the computer's or both are
  people's and the second scored); else a dunk (ball kind 3 or 4) tries
  the Dunks then Power nickname (`FUN_001a83e0`) and posts 1, and a shot
  tries Shots and posts 0xc. The length word (`0x1ac6f0`): points to win
  under 16 → 1, under 26 → 2, under 36 → 4, under 43 → 8, else 0x10. The
  court word (`FUN_001a81b0`) and the dunk-on-court word (`FUN_001a85f0`,
  3 for kind 4 on courts 2, 3, 5, 7, 8 and 9) are tables.
- *The chooser* reads what these events need: key type 2 (the first header
  of the type whose sub-key count admits the request word, `FUN_00363350`;
  the per-line sub-key bits `FUN_00364af0`), the root's word predicates
  (+7's high nibble; a branch passes when one of its words is 0 or shares a
  bit with the request word, `FUN_00366240`) and the repeat limit (+8,
  against `DAT_00422b28`'s per-manager run of one event, `FUN_00365f48`,
  cleared with the queue by `FUN_00363018`). On the disc only Dunk (word
  7) and Shot (word 4) carry predicates, keying their game-length
  variants; only MissedDunk (0xe) a repeat limit (1); only
  `ScoreUpdate.hdr` sub-keys (22, by the margin). The roots differ by event
  (`disc_speech_events_are_the_native_subset_with_equal_weights` lists
  them): Dunk holds 0 at priority 600 with weights 76 and 60, Shot 599 with
  weights 52, 30 and 25, the winning basket holds 400, the hype, the court
  and the end sit at 700, ShotClock and the score call accept three
  requests in four, the slam one in two.
- *The speech pause* (+0x5c, `FUN_001ad958`; cleared by `FUN_001ad9d0`,
  either clearing the queue when asked): a player's request is refused
  before its draw (`0x1ad068`) and a chosen line is dropped. Its senders,
  now wired: the pause menu (slot 57, `0x1a70a0`, paused with the queue
  cleared on the interactive Music Experience; resumed unless a meter is
  full or a staged gamebreaker plays), replays outside a staged
  gamebreaker (slots 58 and 59), a full meter (slot 66, paused and
  cleared) and the meter dropping below full (67), a pocket below full
  (70) and a cancel (71), and slot 61's first call, which resumes with the
  queue cleared and introduces the court (event 3). The rewrite's own: a
  Level 2 gamebreaker's hype is posted with the basket rather than from the
  listener's update as the sequence ends; the court event's third and
  fourth words (the bracket's rounds, the second call's animation words)
  are 0.
- *The song rotation* (`FUN_001aabc8`, `FUN_001aaf78`): `DAT_003ea23c` is
  the in-game song, seeded `rand() % 13 + 1` at the front end's
  initialisation (`FUN_0019f810`; 3 with `audio_demo_cd`) and lasting the
  session; a match plays it unless it is switched off, when the first song
  on from it up to 15, else from 1, takes over (the Rucker Classic plays
  12). When the music moves on, in turn the next song or the next one on;
  at random a draw of `rand() % count + 1` which, when it is the song just
  played or one switched off, gives way to the next one on after the song
  just played, and when the next one on is the song just played, that
  song again without a draw. The draw is the rewrite's own generator.
- *Still the rewrite's own*: the dunk's three words were 0 until the next
  section (they are the dunker's identity, not an animation, and are
  ported there); the missed shot's shooter is the
  last shot let go; the slam's high-flying word is read from an advanced
  dunk; the commentary callback's crowd loop 0x24 and music duck
  (`FUN_001abde8`/`FUN_001abe78`); the queue clock in fixed ticks.

Tests: `speech.rs` (the roots and weights of all fifteen events from the
disc, the repeat limit's run per manager); `audio.rs` (the match's song and
the first song on, rotation in turn and at random, the score call's words
and tables, the pause with and without the queue cleared).

## The Dunk event's three identity words (2026-10-15, the dunk words)

Re-read for the Dunk packet: `FUN_001a7800`, `FUN_001a7948` and
`FUN_001a7bd0` (raw `0x1a7800..0x1a7c10` for the argument passing: each
takes the listener and a player index, which goes straight to
`FUN_001408e0`), their callers `FUN_001a3b10` (the basket) and
`FUN_001a56b0` (slot 61), `FUN_001ad178` case 1, the record accessors
`FUN_001d7840`, `FUN_001da4e0` and `FUN_001da3c8`, the actor's constructor
`FUN_001563a8`, and the roster `XDBPLYR.ADF` on the disc.

- *They are not animation words.* `FUN_001408e0(game, player)` finds the
  actor, whose `+0x288` is its roster record object (`FUN_001563a8` makes
  it with `FUN_001d7840(new, record)`), and `FUN_001da4e0(record, 0)`
  points the record's `+0x20` at field 0: the persistent player id
  (`docs/FORMATS.md`'s "0x1a7804 turns ids 145..151 into unlock bits" is
  this function, and the bits are Bobbito's name words, not unlocks). The
  one value that reads the dunk move, `FUN_001a7e78` (`actor+0x1e8`, the
  dunk id, to an index 1..0x16), is the caller's fifth, which
  `FUN_001ad178` drops.
- *Word 2, `FUN_001a7800`:* the Street Legends by id: 147 Biggie Little 1,
  146 Bonafide 2, 148 Dime 4, 151 Osmosis 8, 145 Stretch 0x10, 149
  Whitewater 0x20; anyone else 0 (no record holds id 150). No flag.
- *Word 3, `FUN_001a7948`:* the Legends by id, 0 while the listener's
  `+0x74` is set: 169 Erving 1, 157 Bird 2, 160 Johnson 4, 163 Chamberlain
  8, 154 Maravich 0x10, 166 Dawkins 0x20, 155 Gervin 0x80, 158 Wilkins
  0x100, 230 Frazier 0x200, 164 Russell 0x400, 153 Baylor 0x800, 170
  Drexler 0x1000, 172 Monroe 0x2000, 171 Hawkins 0x4000, 229 Robertson
  0x8000, 161 Worthy 0x10000, 165 Archibald 0x20000, 159 Thomas 0x40000,
  162 Malone 0x80000, 156 Thompson 0x100000, 231 Barry 0x200000, 168
  Cousy 0x400000, 167 Jordan (the Legends' record 168; neither his Wizards
  record, id 140, nor id 401) 0x800000, 152 West 0x1000000. Walton (232)
  has no bit; no bit is 0x40.
- *Word 4, `FUN_001a7bd0`:* the league stars by id, 0 under the same flag: 5
  Pierce 1, 501 Jay Williams 2, 51 Miles 4, 512 Wagner 8, 20 Finley 0x10,
  21 Nowitzki 0x20, 22 Nash 0x40, 32 Wallace 0x80, 40 Francis 0x100, 16
  Andre Miller 0x200, 50 Odom 0x400, 55 O'Neal 0x800, 56 Bryant 0x1000,
  60 Gasol 0x2000, 75 Garnett 0x4000, 80 Kidd 0x8000, 82 Martin 0x10000,
  86 Davis 0x20000, 95 McGrady 0x40000, 100 Iverson 0x80000, 105 Marbury
  0x100000, 115 Webber 0x200000, 116 Bibby 0x400000, 130 Carter
  0x800000, 30 Stackhouse 0x1000000.
- *The flag `+0x74`:* cleared by the listener's reset (`FUN_001a30a8`) and
  set at slot 61's first call (`FUN_001a56b0`, after the court's event and
  whether or not Playlist silences it): of the six players
  (`FUN_001408e0(game, 0..5)`) those whose word 4 or word 3 is not 0 are
  counted, and fewer than two sets it. So with a single named star or
  legend on the court nobody is called by name in a dunk. (The reading
  here: the words are switched off, not the lines; the loop reads the
  flag as the reset left it.)
- *What they select* (the Dunk root's 27 branches in `comntary.evt`,
  dumped from the disc): DunkA/B/C/E and their clones key column 0 by
  word 1 (the finish) and column 1 by word 5 (the sex); DunkH (114 lines)
  and DunkHB (13, column 0x10: Stretch) key column 1 by word 2 (mask
  0x3f); DunkI/J/K (152/152/169 lines, columns 0x1bf/0x1fe00/0x3fe0000)
  by word 3 (mask 0x3ffffff); DunkL/M/N (143/151/168, columns
  0xff/0xff00/0x1ff0000) by word 4 (mask 0x1ffffff); the last branch is
  the nickname header by word 6. A header whose column shares no bit with
  its word is ruled out (`FUN_00364c38`), so with the words 0 those nine
  headers never played.
- *The rewrite:* `speech::street_legend_word`, `league_legend_word` and
  `league_star_word` map the roster's field 0 (`db::Player::id`, the lineup's
  record in `match_sounds`); `speech::dunker_words` packs them under the
  manager's `name_words_off` (+0x74), which `speech::names_off` computes
  at slot 61's first call as the original counts. No simulation change:
  the words depend on who dunks, which `Event::Score` already names, not
  on the dunk that plays, and the move word is dropped before the packet.
- *Lead, not ported:* the court's second event 3 (`FUN_001a56b0`, posted
  when either word is not 0) carries the same two tables, `FUN_001a78a0`
  and `FUN_001a7a90`, for the record index at `*(game+0x428)+0x2c` (a
  featured player; 0x40 instead when `FUN_001e0bc0`'s `+0x90` is 0x2f):
  the "second call's animation words" of the section above are these
  identity words.

Tests: `speech.rs` (`the_dunkers_identity_words_follow_the_three_tables`:
every id of the three switches and the flag's count); `audio.rs`
(`disc_a_dunk_packs_the_dunkers_identity_words`: a Legends line-up's dunk
posts the words of the dunker's disc record through `match_sounds`, with
the flag computed at the intro's end, and a single named player switches
the legend and star words off).

## The duck, the counter's seed, the queue's clock and the service's order (2026-10-18)

Traced raw and read off a live boot of the original (the project's PCSX2,
PINE every frame from power-on through an attract match) and the save
states' RAM; the details are in `docs/TRACE.md` "The audio's generator,
the commentator's duck and the speech clock". What changes here:

- *The commentary callback's crowd loop and duck are ported.* As a line
  starts (`0x1ac9f8` type 1), out of Street School, AEMS loop 0x24 starts
  with `audio_aems_sfx_ducking` 38, `audio_aems_crowd_ducking` 40 and
  `audio_aems_plyr_spch_ducking` 25 (the crowd banks' mixer and the courts'
  ambience take them), and the music ducks to (100 - 35)% of its volume
  over 100 ms (`FUN_001abde8`, 98 -> 63); as the stream runs out
  (`0x1acf58`) the loop is released with that message and the music comes
  back (`FUN_001abe78`). The manager has one stream: a line started over
  another (the override, +0x68) replaces it, where the rewrite had played
  both. The pause fades the line to nothing and lets it run; the resume
  stops it. (`audio::speech_service`, `MusicDuck`, `duck_music`,
  `sfx::Effects::duck_for_commentary`, `commentary_over`.)
- *The counter's seed is the original's.* The constructor's `rand() %
  255` is the C library's generator's first draw after `AuController::Init`
  seeds it from the 100 Hz timer (`srand(timer)` in the path manager's
  constructor, then the speech manager's, then `srand(timer)` again in the
  ambience manager's). A live boot read `srand(632)` and the counter 18;
  every captured boot's middleware generator walks back to a seed of 18.
  The front end's first initialisation (`FUN_0019fbb8`) runs `0x1ac6f0`
  once and shuts the manager down, so a boot's first match runs at 20, as
  the save states hold. (`session::Session::speech_counter`; the earlier
  sections' "the counter starts at 0" and "the front end's initialisation
  does not advance the counter" are replaced.)
- *The queue's clock is the 100 Hz timer*, `FUN_003998c0`, which
  `0x1ac6f0` hands the middleware as its clock callback: hundredths, not
  VBlanks and not the rewrite's 60 Hz ticks. A root's hold of 200 is two
  seconds. The capture read 243 queued requests stamped with the timer.
  (`SpeechManager::tick`, `hundredths`, the `speech_clock` system in every
  state.)
- *The players are manager 0 and served first.* The earlier sections had
  `FUN_00364620(0)` as the commentary's: the manager number is the order
  `0x1ac6f0` registers the event tables (players 0, commentary 1), the
  number a queued request keeps at +0x6b, while the post's key byte
  (players 1, commentary 0) is the event table's own id. The capture read
  the queue's bytes and the per-manager runs (`DAT_00422b28`) that way. In
  a frame that serves both, the players' chooser now takes the first draws
  (`speech::PLAYERS` 0, `speech::COMMENTARY` 1).
- *The pause follows the menu.* The rewrite resumed speech (its queue
  cleared) on every frame's write of `Paused`; it now acts on the menu
  opening and closing (slot 57, `0x1a7198`).
- *A correction: slot 53 is the game's reset* (`FUN_0013d388`), which also
  resets the speech manager (`FUN_001ada80`: the partitions run again, the
  override set, the counter counted up); no attract match posted it. Not
  ported.

A first comparison against the original's attract match (no PCM): 68 lines
in 34,084 updates, mean 2.47 s, against the rewrite's demo's 51 in 32,147,
mean 2.40 s; the events' mix differs as the two games do (TRACE's table).

Still open: a PCM or gain comparison; the C library's later draws in the
original's order (the song rotation's random pick and the 21-second coin
still use the rewrite's generators); the music's base share; slot 53's
reset; the pre-tip intro's pause sender; slot 27's missed shots; the
intercepted pass's Steals post.

Tests: `audio::rotation_tests::{the_music_ducks_to_the_originals_share_over_a_tenth_of_a_second,
the_speech_clock_counts_the_timers_hundredths,
disc_a_commentary_line_ducks_the_music_until_it_ends,
disc_the_players_are_served_before_the_commentary,
disc_the_speech_counter_starts_from_the_boots_c_library,
speech_follows_the_pause_menu_not_the_frames}`,
`sfx::tests::the_commentators_duck_steers_the_crowd_mixer_with_its_percents`,
`session::tests::the_audio_draws_its_first_values_from_the_c_library_in_the_boots_order`.
