# Street School user payload and rewards (2026-10-13)

Static US PS2 evidence from original `SLUS_206.51`, SHA-256
`9f2cf237da7bd78dada337a2939f3878c05ddbe39588570b5328154cbe637614`.
Ghidra decompilation was checked against original ELF instructions.
Retained functions, raw lists and the user vtable are in
`%TEMP%/opencode/street-re/mode-school-profile-probe-20261013/`.
No original runtime School session was replayed.

## Observed native behavior

`FUN_0018e9f0` binds manager +0x4c4 to the selected controller's user object
at game +0xbc0 + selected-controller * 0xd4. `FUN_002bb468` fills the lesson
list from that same user's 26 completion nibbles. User Setup uses complete
payload copies for an existing name (`FUN_00245b88` / `FUN_001ee1f8`). Its
explicit guest-confirmation arm calls user vtable +0x14 with a null payload;
the original table at 0x4611c4 resolves this to `FUN_001ee1f8`, which clears
all 0xbc payload bytes. `FUN_001ee450` excludes empty/space names from
persistent storage. A guest therefore has live lesson state without a stored
named-user record. Precise alternate user-screen routes remain untested.

The native payload layout is:

| Object | Serialized user | Meaning |
| --- | --- | --- |
| +0x54 | +0x3c | flags; bit0 means completion payout consumed |
| +0x58..+0x68 | +0x40..+0x50 | five packed words holding 33 nibbles |
| +0x6c..+0xd0 | +0x54..+0xb8 | 26 signed lesson-launch counters |

`FUN_001eef88` reverses both word and nibble order: lesson0 is the high
nibble of word4, lesson32 the high nibble of word0. `FUN_001eebb8` reads bit3
as completion. The low three bits are an outcome code, not a remaining task
target. `FUN_001eecd8` writes4 after failure; success sets bit3 and preserves
any prior low bits. It ignores updates to already completed lessons. Thus a
failed lesson followed by success stores12, while a first success stores8.
`FUN_001eec28` returns the low bits only for a completed lesson, otherwise4.
`FUN_001eedb0` separately sets extra tutorial nibbles26..32 to8.

`FUN_0018ec28` launches a lesson and calls `FUN_001eeca8`, incrementing its
counter even on a replay. The raw integer add wraps at32 bits. Counts describe
launches, not successful executions. The manager later commits a result in
`FUN_0018eec0` through `FUN_001eecd8`. The reset studied here is whole-user
initialization; no per-School completed-run reset was observed in these paths.

Mode1's `FUN_001ded40` arm at0x1dfcd4..0x1dfd80 checks every one of the26
lessons (`FUN_001eeb38`) and user +0x54 bit0. When complete and unpaid it:

- queues1000 shared Reward Points at0x1dfd1c;
- reaches shared milestone10, plus11 if all26 completed outcome codes are0;
- sets shared game trophy bit0 through `FUN_001e0b10`;
- sets the user's paid bit0 at0x1dfd78 and flushes named user payloads.

This is one payment per user payload, with shared balances/cards/trophy. A
completed lesson replay retains its outcome code and cannot repeatedly pay
for an already paid profile.

## Implementation and compatibility

`Records.User.school` now keeps all33 raw nibbles,26 signed counters and the
full paid-flags word. The optional `SCHU` records extension follows `LEAGUEU`;
the decoder accepts either order, bounds counts and nibble values, rejects
duplicate sections and truncated data, and accepts earlier records without
School payloads. A payout-known marker distinguishes an unmapped original
flag from a known paid or unpaid profile.

`Story` uses a selected-name live cache and an explicitly unassigned older
rewrite mask/failure snapshot. User Setup binds School to the entering
controller; lesson launch and the game result use the same tested helpers.
The game retains its School run until results, so the native completion
payment and School milestones dispatch on a School result. The shared
lifetime trophy is stored independently in `SCTR` and imported from the
original PS2 game flag. User switches and guests retain this shared trophy.

Original PS2 imports preserve each user's exact payload rather than ORing
lesson completion across names. Earlier rewrite progress has no recoverable
owner: it stays unassigned and is never assigned to a guessed name. Its
earned shared trophy/RP/cards remain retained. A new guest starts a fresh live
payload when User Setup continues; its lesson progress never enters a stored
user or the unassigned snapshot.

The existing GameCube conversion deliberately leaves compiler flag fields
unmapped. Imported GCI School payout ownership is therefore marked unknown
and the rewrite suppresses ambiguous completion payments. This is an explicit
policy to avoid inventing a1000RP grant; original GameCube payout fidelity is
unverified. Its original container remains preserved. New users and traced
PS2 imports have known paid flags.

Focused tests cover launch wrapping, failure then success, completed replay,
A to B to A, guest/reload isolation, one payment per user, shared trophy,
unknown GCI payout state, unassigned legacy ownership, codec bounds, and a
real-USA-ISO production preparation/result helper route plus multiuser native
save import. The trigger adapter below replaces the earlier stateless pass
predicates and unsupported150-second failure timer. Generic practice layout,
AI, input/action engines and extra-tutorial presentation remain adaptations.

## Native lesson callback adapter

The retained trigger probe is
`%TEMP%/opencode/street-re/mode-school-trigger-probe-20261013/`.
It contains all26 readiness/success dispatch arms, original School raw code,
event proxy and publisher mappings, and producer traces for basket values.
The manager task is the lesson row0..25; table `+1c` values-1..7 are input
categories, not those26 task indexes. Ghidra's call-return override at30fa50
truncates18e350,18f6b0 and190850; original raw tails were retained and used.

Production `step_sim` drives a match-owned `LessonRun`, since 2026-10-19 a
wrapper of the simulation's `street_sim::school::Run` (below, "Against the
original running"). Pass0 andKick17
wait for an actual catch; release records source/target/kick history.
Heezay18/Back2Papa19 require the bounce leg to finish before their catch.
Give'nGo22 requires a CPU catch followed by a human catch. Switch7 compares
the controller's actual selected actor against its baseline. Trick6 uses a
landed callback; AdvancedTrick20 checks the disc-configured native script
identity exactly in{5,20,24,21}, irrespective of its value class. Block9
requires the event actor to have a human seat; PumpFake12 checks possession.

Raw16b158..174 selects base2/3 through16c908 and stores3d658c;1734f4 passes
that value to14aaf8, which stores ball+15c/160. Basket publisher1515a8 puts
that base into packet+c and ball+184 into packet+10. Displayed Street points
are1/2 and cannot be compared to the native base directly. Short1 therefore
uses the ordinary jumper category with short range; long2 uses long range.
Ordinary shooting Gamebreakers carry the same range category; staged shots
use launcher14c408's kind0, so they cannot pass short1's kind2 predicate.
The aggregate Dunk3 predicate accepts the dunk/oop category corresponding to
native kind3/4. GB14 requires a made GB basket, rather than activation.
Task4's genuine fallthrough also permits a GB basket. Level2 task25 checks
made GB level2 and waits for an active staged sequence to end when needed;
ordinary baskets fail it and level1 baskets do not pass it.

Common failure callbacks now include shot-clock violation, lost expected
possession except task25, missed baskets for8/9/10/11/14/25, and wrong-task
Pocket/Cancel. A steal/counter fails25. Pocket23 andCancel24 succeed on their
respective completed callbacks. `18f6b0` queues round(Hz*.9f), or
round(Hz*1.5f) forCancel:54/90 updates at60Hz. Later callbacks can replace a
pending outcome and restart this delay. Commit waits for an active staged
sequence to finish, writes the selected profile once, and retains the run
through the results screen. The150-second rewrite timeout has been removed.
The complete native ScriptManager state3/4/5/6/7 blocking predicate remains
unrepresented; staged-sequence blocking is the available adapter.

Lesson entry stages the expected side from table3de308: opposite the human
side for7..11/16, the human side otherwise. GB14/23/24/25 start full. The
Cancel pocket writes are mapped through the native side convention. Mode1
Choose Sides297c40 sets configteam0 via1dbac0 at297fb4;13cf58 calls1db308
and XORs that side with1 at13d0c8, giving native School human game-side1.
Native c8 team0 is therefore the enemy. With the rewrite human onHome0, the
Cancel entry setsAway1's pocket and clearsHome0's pocket, then fillsHome's
meter. Original raw chain is retained separately in
`%TEMP%/street-school-team-entry-root/side-mapping-raw.txt`, SHA256
`d33c0da7424b923e6ba11394eee4c0ff0135090c38cff41d8b6f41f7932d13b2`.
The broader generic practice layout/AI remains open.

The predicates once mapped loosely (layup 4, alley-oop 5, counter 10, turbo
block 11, fadeaway 13, pick 15, dive 16, advanced dunks 21) now follow the
listener's slots (below): 4 a lay-up after the take-off's slot 4, 5 an
alley-oop not to oneself, 10 the person's counter (slot 15's 10), 11 a block
whose channel-0 record is `0x1b5`, 13 a basket from a fadeaway script, 15 the
pick's contact (slot 74), 16 a take while diving or within `trunc(fps x 0.12)`
of the dive's frame, 21 a basket by a mover who caught his own pass with an
advanced move.

The unit tests are `street_sim::school`'s
(`each_lessons_listener_slot_decides_as_the_originals`,
`the_inbound_pass_and_results_before_the_start_do_not_count`,
`a_result_commits_after_its_delay_and_a_later_callback_replaces_it`) and the
fixed-step integration tests in `crates/street-game/src/game.rs`.

Eight actual FixedUpdate regressions cover release versus catch, late run
without the old deadline, stable one-time persistence through150 results
updates, actual controller switching, Give'nGo catch order, made/missed GB
versus activation,54/90 delays, replacing a pending success with a later
failure, Street range values, mapped Cancel entry/inbound/control, and actual
ISO-resolved native trick IDs.
## Against the original running (2026-10-19)

Seven captures of one School session from a fresh boot (`%TEMP%/
street-played-school-0..6.jsonl`, schema 3 of `tools/pine_played_capture.py`,
indexed in `%TEMP%/street-played-school-index.json`, out of git) hold all 26
lessons: 45 runs, 44 commits; all passed but Pick (three runs, no pick bit
under the R3 press) and Advanced Dunks (three runs, the self-toss from 33
across, too close for its second leg; docs/TRACE.md "Street School as a
capture suite"). What they settle:

- The listener and the manager are the simulation's (`street_sim::school`):
  the listener's slots, then the manager inside the game's update after the
  shot clock and the meters (`FUN_0013d868`). A result an actor or the ball
  posts commits 54 updates after its own (53 of the simulation's steps), one
  the game's update posts 53 or 54; all 44 captured commits at the delay,
  the user's nibble 8 after a pass (12 after an earlier failure), 4 after a
  failure (`played_capture::school::
  the_lessons_commit_their_results_after_the_managers_delay`).
- The launch (`FUN_0018ec28`): the restart of the lesson's side, the snap to
  the restart spots with the row forced to 2, the shot clock refilled and
  the gamebreaker lessons' meters (`World::launch_lesson`); all 45 launches
  on the captured spots (`the_lessons_launch_into_their_restart_spots_as_
  fun_0018ec28`). In a lesson the computer neither reaches nor counters nor
  goal-tends, and only lessons 23 and 24 pocket and cancel.
- The gamebreaker lessons: the task's end 63 updates after the press, no
  replay shown, and the Level 2 run on the disc's script 9 from two updates
  after the press (docs/TRACE.md).

What differs: the original plays every lesson in one game, the Lakers (the
person on side 1) against the Nets on the School's own court, under an
in-game lesson menu (tabs Offense 0..6 and 14, Defense 7..11, Advanced 12,
13 and 15..25; ticks; the title and description, or the tip once passed)
with tutorial pages that hold the game; the rewrite starts each lesson from
the front end's School page as a match of its own, the person's crew against
the generic ballers at Rucker. The lessons' rules are the original's; the
game around them is left for a pass of its own.

