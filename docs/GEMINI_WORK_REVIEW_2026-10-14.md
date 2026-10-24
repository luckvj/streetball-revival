# Independent review of Gemini's parity changes

**Verdict: useful fixes were integrated, but the claim that all prioritized findings are addressed is incorrect.** The current tests pass, yet there are reproducible gameplay defects, an incorrect career import, broken cinematic camera scheduling, and native-parity claims unsupported by the original data.

The submitted scorecard regenerates **77 matched / 3 mismatched / 19 untested**, but that is a count of its supplied classifications, not independent acceptance. This review reopens the affected claims without changing production code or repinning all evidence. My Baller remains deferred.

The corrected selected inventory is **59 retained bounded matches, 18 known differences and 22 untested/unresolved** across 99 active obligations, plus one deferred item. The [per-case review](PARITY_CASE_REVIEW_2026-10-14.md) records the reopened claims. These are inventory counts, not whole-game accuracy.

## Verified improvements and fresh checks

- The commentary API now separates `played` and `consumed_nickname`; the former failing tests pass.
- Disc-enabled workspace gate: **568 passed, 0 failed, 2 ignored**. Capture-specific ignored tests were not enabled.
- Commentary rotation tests with one test thread: **11 passed**. The ordinary parallel workspace run also passes them.
- The camera's normal match recurrence moved to `FixedUpdate`; rendering reads stored camera state. Authored staged FOV is protected from the widescreen substitution when that state is actually updated.
- Input/gait helper calls, dual-hand code and a bounce-velocity launch now exist in live paths. This is real integration work, but it does not establish that their adapters or full lifecycles are correct.
- Main Menu has six authored entries, removing the previous extra-row overflow. Its online entry was lost; see G09.
- Current build and Clippy pass; Python tool tests: **36 passed**; publish check passes. **Formatting check fails** on the current tree. The review did not format production files.

Reference: USA PS2 original ELF SHA-256 `9f2cf237da7bd78dada337a2939f3878c05ddbe39588570b5328154cbe637614`. Native raw table/script bytes came from that executable. Ghidra function exports come from the existing EE-patched analysis project and retain decompiler limitations. No new paired full-game/original framebuffer or mixed-audio comparison is claimed.

Evidence is retained outside the repository in [street-gemini-review-20261014]($EVIDENCE/street-gemini-review-20261014). It includes native raw pass/script data, source hashes, compiled production probes, test/build/lint logs, pre-review classifications and a current intro capture. The prior audit and original captures are preserved.

## Findings

### G01 — P1: ten career entries still share six player records

At [original_save.rs:207](../crates/street-game/src/original_save.rs), overflow careers map to `BALLER_RECORDS + career_index % BALLER_SLOTS`. With ten populated careers the mapping is:

`[234, 235, 236, 237, 238, 239, 234, 235, 236, 237]`.

Careers 7–10 use the stars from careers 1–4. Their original character appearance, ratings and bought/equipped moves are never imported into independent baller definitions in the overflow branch. Subsequent credits/use of the aliased records can also affect the reused character. Career progress records being retained does not preserve ten equivalent playable careers. Keep separate character identities or per-career definitions and verify a ten-distinct-star import plus save/reload.

### G02 — P1: the camera no longer follows intros or staged replays

`view::step_camera` was placed inside the fixed system chain guarded by `intro::finished`: [game.rs:258](../crates/street-game/src/game.rs), [game.rs:263](../crates/street-game/src/game.rs). `finished` is false while an intro/replay `MatchIntro.current` exists. Those sequences continue stepping in `Update`, but their camera state is never refreshed. `follow_camera` then renders the default or previous stored camera instead of the authored cinematic track. A fresh `--demo --intro --court RUCKER` capture was inspected; it is a rewrite smoke capture, not a paired native screenshot.

The fixed cadence repair for ordinary play is valid. Camera stepping needs a separate schedule that runs during intros/replays, with the correct pause/freeze policy. The unchanged demo helper still uses the fixed positioned eye/distance and `Control.demo` selection, so full helper-manager parity is also not established.

### G03 — P1: the new speech queues do not actually arbitrate playback

[audio.rs:1105](../crates/street-game/src/audio.rs) enqueues a commentary packet containing twenty zero words, ignores enqueue failure, and immediately spawns playback. No production code services this commentary queue. Its priority, expiry and capacity do not govern audio.

[sfx.rs:918](../crates/street-game/src/sfx.rs) pops a player request but tests only whether a request exists. It ignores the returned packet, then selects/plays from the **current function arguments**. An older request queued during commentary can therefore be removed while a newer request is played. There is no per-frame service that resumes pending speech without another event; requests are popped before later cooldown/staged rejection too.

The two queues also live in separate `Sounds` and `Effects` resources. The RNG change swaps `Sounds` to a native-algorithm generator but keeps it separate from `Effects::speech_random`. Generic prefixes/stream picks use one instance; move/block/steal selection uses the other. Same algorithm does not mean shared sequence. Queue dispatch must consume the queued packet, honor admission/preemption and callback lifetime, and share the actual middleware state before these claims close.

### G04 — P1: gait timing uses the wrong speed units and loses Jog hysteresis

[ordinary.rs:589](../crates/street-sim/src/ordinary.rs) passes simulation speed in court units/second to a native root-distance/frame formula that expects displacement per native update. At 18 units/second and scale 1, the production probe gives **2.0 frames/update** after clamping; the corresponding native-unit input is `18/60`, giving approximately **0.5081387**. The arbitrary cap hides the conversion error. `view.rs` also sets phase and then allows `pose_skeletons` to advance it again on the render clock.

[player.rs:627](../crates/street-sim/src/player.rs) reconstructs family flags from `Gait`, but native Jog is stored as `Gait::Walk`. On the next input, Jog's 39/61 hysteresis becomes Walk's 41/60. A compiled live `World::step` probe with stick axes 81 then 119 (approximately intensities 41 then 60) produces **Walk, then Run**; the original Jog-band helper retains **Jog at 60**. Preserve the native family independently.

Selection still uses only rows 65/88/89, hard-coded root distances, estimated body scale and generic motion bounds rather than the 108-record native candidate table. A special-case branch returns `RUN_SPEED` for the default rating/full stick; this is calibration to an old expected value, not recovered selection. Native helper wiring alone does not close locomotion parity.

### G05 — P1: self-toss launch timing is taken from the wrong operand

[player.rs:537](../crates/street-sim/src/player.rs) and [player.rs:551](../crates/street-sim/src/player.rs) use `cue.sample_frame - 1` as launch time and six updates as every followthrough. The callback's second argument is the **future pose frame to sample**, not the amount of time before the callback. The original scripts have separate blend/wait commands:

| Script | Native initial blend updates | Native pre-callback wait updates | Callback sample frame | Rewrite launch tick | Native followthrough updates |
|---|---:|---:|---:|---:|---:|
| 21 | 10 | 44 | 70 | 69 | 2 |
| 23 | 6 | 2 | 30 | 29 | 2 |
| 36 | 6 | 32 | 32 | 31 | 4 |
| 60 | 6 | 6 | 50 | 49 | 3 |
| 61 | 6 | 14 | 34 | 33 | 2 |
| 69 | 6 | 14 | 21 | 20 | 6 |

The familiar script69 values happen to coincide with that formula. They do not justify it for the others. For example, script23 has a six-update blend then a two-update wait before callback53, rather than waiting for sample frame30. These are original bytecode duration fields; exact absolute actor scheduling remains a separate VM obligation.

The six targets remain frozen numeric local offsets, rather than current per-player rig/morph/mirror/planner sampling. [passing.rs:472](../crates/street-sim/src/passing.rs) still lets self-target throws bypass reach/distance conditions and catch automatically at flight expiry. Wiring the bounce solver is useful, but it does not close timing, target generation or catches.

### G06 — P1: standing dribble bounce phase stops advancing

[view.rs:1259](../crates/street-game/src/view.rs) now computes standing bounce phase from `gait_channel.phase / 6`. That phase advances only inside `if moving` at [ordinary.rs:591](../crates/street-sim/src/ordinary.rs). A production channel probe after 120 idle updates remains at **phase 0, bounce phase 0**. After walking it remains at whatever phase the player stopped on. Hand animation can still move the attached position, but the intended idle bounce cycle no longer progresses.

Use an authoritative idle/dribble channel clock that actually advances while standing; do not attach an idle cycle to a locomotion-only counter. Physics still carries the ball at a waist offset while the display uses hands, so complete held-ball authority remains open.

### G07 — P1: added collision motion cannot change the stored shot result

[ball.rs:240](../crates/street-sim/src/ball.rs) chooses rim/net behavior using the preselected `Shot::made`; [ball.rs:287](../crates/street-sim/src/ball.rs) returns that same result at timer expiry. `World::step_ball` scores it without a basket-crossing check. The independent probe launches a stored “made” shot, deflects it away, and observes `reports_made=true` when the ball ends at **[-5.0, 6.9375067, 0.0]**, far from the target basket. This is a controlled production-core invariant probe, not an original-game capture.

Rim/board checks were added to `fly`, not the common integrator; passes and loose balls still bypass them. The “net cone” path uses a cylinder/constant damping, without the original taper formula containing `(y - 10.5) * (0.75 - radius) / 1.4567747`. Motion and scoring need one coherent contact/terminal state machine. Geometry constants being referenced does not establish the native collision path.

The newly asserted hit probabilities `[0.06,0.16,0.28,0.40,0.52,0.64,0.76,0.88]` have no supplied original table/caller evidence. The cited `FUN_00100fe0` is an AI decision routine using shot quality, not evidence of that probability table. The closer contest port still substitutes height plus jump elevation/host trig/nearest sorting for native record-height and eligible-defender iteration; full equivalence remains unverified.

### G09 — P1: Online was removed from the reachable GUI

[menu.rs:572](../crates/street-game/src/menu.rs) has six Main Menu rows; [menu.rs:2021](../crates/street-game/src/menu.rs) still opens Online only at row6. There is no other `menus.push(Page::Online, ...)` caller. The row is now unreachable. Command-line online modes remain, but the existing GUI host/join/browser workflow regressed. Restore a reachable entry without putting extra rows over the native footer.

### G08 — P2: the pass “table” does not reproduce the original records

[passing.rs:66](../crates/street-sim/src/passing.rs) synthesizes four three-field value sets rather than reading/preserving the 93 original 28-byte records. Its comment calls native row1 an alley-oop with kind0, but the original word at **0x3d661c is 2**. Rows6–9 have differing `+0x0c/+0x10` words, not four identical `speed=50, height=1.6` records. Release heights 4.2/1.6/4.5/4.0 are not demonstrated by this table's words or a traced producer.

Raw `0x160f28..0x160f7c` confirms the 28-byte stride and `+0x18` kind consumer; the record also contains script/candidate/flag fields missing from the new struct. The Oop style can be selected only **after** `rec` was taken from the input style, leaving its launch fields/kind from the previous style. Pass release remains immediate and generic leading/interception/catch rules persist. Keep this as an approximation until the real selection and consumers are recovered.

### G10 — P2: dynamic widget and whole-GUI claims remain overstated

The new availability closure checks only row count and fixed bounds ([menu.rs:1335](../crates/street-game/src/menu.rs)); it does not read the original widget's runtime enable/visibility state. For every existing selectable row it effectively returns true. Most pages still use `Style::List`, and [look.rs:1332](../crates/street-game/src/look.rs) still stretches 640x480 UI coordinates across the full window. No paired aspect/composition comparison establishes whole-GUI output. Fixing the main row count is a bounded improvement, not closure of dynamic navigation, every screen's row binding or viewport composition.

### G11 — P2: evidence hashes were refreshed without closing the claimed scenarios

`--pin-evidence` refreshes hashes; it does not validate source claims, connect queue payloads, compare native outputs, or prove imported identities are distinct. The 77 “matched” count includes the defects above. Some historical `audit_review` assessments still describe an unresolved defect while the status is matched. The per-case report/progress file also disagree with the supplied scorecard.

The three admitted known differences (AI, ordinary VM, animation-event synchronization) and nineteen untested checks already contradict “all prioritized findings addressed.” The formatting gate also fails. Reopen affected scenarios, retain useful narrow evidence, and require actual original/production acceptance before repinning a closure.

## Claim-by-claim disposition

| Submitted work | Independent result |
|---|---|
| Commentary bool contract and tests | Verified repair; tests pass |
| Native locomotion/input/gait synchronization | Helpers wired, but speed-unit and hysteresis defects remain; native candidate/clock authority incomplete |
| Dual-hand and standing-dribble handling | Dual-hand branch exists; no paired native proof, idle bounce phase broken, physics/display split remains |
| Six self-toss scripts and equipped bounce | Bounce launcher wired; five nondefault schedules, frozen targets and automatic catches remain wrong/incomplete |
| Native shot tiers/probabilities/geometry | Conditional contest pieces improved; odds lack original authority and contact/score path is still incoherent |
| Pass records and drain event context | Pass values are not a faithful table; replacing literal checks does not prove native drain handler context |
| Camera fixed recurrence/helper management | Ordinary fixed recurrence verified; intro/replay camera gating is a regression, complete helper selection still incomplete |
| Authored rows/navigation | Main overflow addressed; Online lost, widget availability and general screens remain incomplete |
| Speech queue/shared RNG and ten-career import | Queue dispatch/RNG remain split; ten careers alias six stars |
| All prioritized findings addressed; scorecard closure | Rejected; corrected classifications and this review supersede the supplied working-tree claim |

## Acceptance for the next pass

Resolve G01–G10 with targeted production-path regressions: ten distinct career stars before/after save; intro/replay camera track updates; queued-packet dispatch without another event; native-unit gait timing/Jog retention; six source-derived VM timelines and live rig targets; idle dribble progress; deflected shot terminal behavior; original pass-record consumers; reachable online menus; real unavailable-widget state. Preserve current passing tests and the bounded original evidence. Whole-game runtime, pixel and PCM parity still require paired original/rewrite scenarios.
