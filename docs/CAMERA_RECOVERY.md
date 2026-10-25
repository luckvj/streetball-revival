# Camera recovery

## Native pose

The evidence is from the shipped `SLUS_206.51` ELF and the connected Ghidra `street2ee` project. `FUN_002f7c38`, installed at vtable slot `+0x14` (`0x4985c4`), reads the camera mode at `+0x1f0` and writes eye XYZ, two view angles, and an additional row value. The six rows begin at `0x498478`, with a `0x34`-byte stride. The fields mapped here are x bound, signed x offset, eye height, focus-z scale, focus-z offset, and raw row word 7:

| Row | View | X bound | X offset | Eye Y | Z scale | Z offset | Row word 7 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | Street | 4 | 5 | 12 | 12 | -30 | 0.33 |
| 1 | Court | 15 | 5 | 16 | 12 | -40 | 0.23 |
| 2 | Slide | 18 | 12 | 22 | 0 | -65 | 0.18 |
| 3 | Solid | 0 | 0 | 28 | 0 | -75 | 0.15 |
| 4 | Behind | 0 | -60 | 19 | 26.5 | 0 | 0.281 |
| 5 | Wide | 4 | 5 | 12 | 8 | -40 | 0.315 |

For rows 0, 1, 2, 3, and 5, the pose helper computes:

```text
eye.x = clamp(focus.x + side * x_offset, -x_bound, x_bound)
eye.y = eye_height
eye.z = focus.z * z_scale / 26.5 + z_offset
```

Behind (row 4) is special: zero `side` is treated as +1; the eye is `(focus.x - 60*side, 19, focus.z)`, with x clamped only toward zero. The native helper fixes yaw to `side * pi/2` and derives pitch toward ground level using the original, unclamped 60-unit horizontal offset. Rust therefore supplies `Transform::looking_at` a constructed view point `eye + (side*60, -19, 0)`. This preserves the native ground pitch for a jumping owner and at the clamp edge.

## Native record and update law

`FUN_002ff1c8` builds a 13-float controller record consumed by `FUN_002fda08`. For the ordinary camera-state path (`state +0x88 == 0x12`), fields 4..12 mean:

| Field | Meaning | Evidence / units |
| ---: | --- | --- |
| 4 | Increment for camera scalar `+0x98`; `FUN_002fd808` computes `(cam[+0x114] - cam[+0x110]) / (S/2) * mode_sign`, then clamps it so the next scalar remains inside those bounds. | Scalar units per native update; `S = DAT_004d8cfc`. |
| 5 | X tracking / command step, `42/S * 0.5`. | Court units per native update. Used while producing X velocity command field 0 and for directional input. |
| 6 | Depth tracking / command step, `26.5/S * 0.5`. | Court units per native update. Used while producing depth velocity command field 1 and for directional input. |
| 7 | Pitch-command step, `atan_approx(cam[+0x98]) / (2*S)`. | Radians per native update; field 7 changes command field 2. |
| 8 | Yaw-command step, `4/3 * field 7` (`FUN_002ff818`). | Radians per native update; field 8 changes command field 3. |
| 9 | X-velocity correction, normally `(field 0 - cam[+0x9c]) * 0.25`. | Per-update velocity delta. |
| 10 | Depth-velocity correction, normally `(field 1 - cam[+0xa4]) * 0.25`. | Per-update velocity delta. |
| 11 | Pitch-velocity correction, normally `(field 2 - cam[+0xa8]) * 0.25`. | Per-update velocity delta. |
| 12 | Yaw-velocity correction, normally `(field 3 - cam[+0xac]) * 0.25`. | Per-update velocity delta. |

The four command fields 0..3 are desired rates for X, depth, pitch, and yaw. `FUN_002fb690` derives tracking commands; `FUN_002fcd50` applies controller flags and uses fields 5..8. Fields 9..12 are recalculated from the command/current-velocity difference; mode and sign-change branches can halve the nominal quarter-step (to an eighth-step) or zero a velocity across a sign change. They are discrete per-update corrections, not acceleration in seconds.

For the ordinary state-`0x12` path, `FUN_002fb690` builds planar tracking commands before edge-zone flags modify them. Rows 2 (Slide), 3 (Solid), and 5 (Wide) enter the same row-follow block directly. After `FUN_002f7bb0` selects a mode, these rows use the row's `+0x10` word as the depth offset: the depth target is `focus.z * row[+0x0c] / 26.5 + row[+0x10]`. The controller steps toward that target by at most `max(record[6], abs(camera+0xf8))`, using the exact remaining difference when it is smaller. It similarly tracks `focus.x + side * row[+4]` by at most `max(record[5], abs(camera+0xf0))`, then clamps the next X position to the row's `±row[+0x00]` bound. If that clamp changes the proposed step, the X command becomes the exact distance to the bound. After clamping, X command field 0 is zeroed when camera `+0xf0 == 0`; depth command field 1 is zeroed when `+0x12c == 0` and `+0xf8 == 0`. Thus the common block does not keep moving a stationary focus toward the row target. The entire command-generation helper returns zero commands when `+0x13c != 0`. Camera `+0xf0/+0xf4/+0xf8` are components of the full 128-bit source-motion vector produced by `FUN_003003f8`; they are separate from integrated camera velocities (`+0x9c/+0xa4/+0xa8`). Its exact raw VU arithmetic and descriptor-type gates are recorded in the focus section below.

After these planar commands, `FUN_002face8` computes wrapped pitch/yaw corrections from the current eye-to-focus direction and a predicted direction that incorporates the commands and `+0xf0/+0xf4/+0xf8`. It uses the native polynomial atan/atan2 path. `FUN_002fcd50` then applies the edge masks and state-specific rules described below; `FUN_002ff1c8` derives velocity corrections from the final command fields. Behind is a separate base-command path: it seeds X and depth commands from camera `+0xf0/+0xf8`, then the Behind-specific flag path suppresses depth/yaw and applies its asymmetric framing logic. Modes 0 and 1 also take row-pointer, possession, and play-state branches before the shared row-follow block; the equations here are specifically the common rows 2/3/5 path.

`FUN_002fda08` first adjusts velocities `+0x9c/+0xa4/+0xa8/+0xac` toward record fields 0..3 using corrections 9..12, with zero/crossing guards. It then integrates X `+0x84 += +0x9c`, depth `+0x8c += +0xa4`, pitch `+0x90 += +0xa8`, and yaw `+0x94 += +0xac`. `+0x88` (row eye height) is not integrated. In state `0x12`, X is clamped by the selected row's X bound, and hitting the edge zeros the X command/velocity; Behind has its special one-sided clamp. Finally it adds field 4 to `+0x98` once per substep.

Raw EE instruction checks corroborate those stores and loop semantics: `0x002fdecc` is `add.S f6,f6,f1` (`80310146`) and `0x002fdee0` stores the result to `+0x84` (`swc1`, `840006e6`); `0x002fded4` adds the depth velocity (`00210046`) and `0x002fdee4` stores to `+0x8c` (`8c0004e6`). At `0x002fdfb0..0x002fdfc8`, the instructions load `+0x98`, add record field 4, decrement/store `+0x124`, loop while positive, and store `+0x98` (`lwc1 980000c6`, `lw 2401028e`, `add.S 00000146`, `addiu ffff4224`, `sw 240102ae`, `bgtz 0aff401c`, delay-slot `swc1 980000e6`). `FUN_002fa4f8` writes 1 to `+0x124` at `0x002fa520` (`sw`, `240111ae`) immediately before dispatching the object's vtable `+0x1c` update callback. Thus an ordinary wrapper invocation has one internal update; `FUN_002fabe8` separately resets it to 1 for each iteration of its settling loop. The native update is tick-based with no `delta_secs` multiplier; the ordinary world camera callback runs once per 60 Hz simulation tick.

The scale is established from the native initialization path. `FUN_00300c00` obtains the config record from `FUN_0030fa50` and copies its `+0x24` integer into `DAT_004d8cf8` shifted left once (120), then converts the same integer to float for `DAT_004d8cfc` (60). `FUN_0030f938` initializes that record field to 60. The same setup derives `DAT_004d8d00` from the record's `+0x28` field. `FUN_002fd808` caches `S/2` (=30) at `DAT_004d8bd4`; raw stores show `S` loaded at `0x002fd840`, halved at `0x002fd84c`, and cached at `0x002fd854`. `FUN_002ff1c8` computes fields 5 and 6 as `(42/S)*0.5` and `(26.5/S)*0.5`, which are 0.35 and approximately 0.2208333 at the default `S=60`; field 7 divides the atan-like value by `2*S` (=120). `FUN_002fd808` uses the cached `S/2` for field 4. The default native world update is 60 Hz and calls the camera update once per tick, so these values are per-update rates/steps; the Rust controller still needs to translate all command-generation branches and state transitions before it can reproduce them.
`FUN_002ff830` projects the focus into the 640×480 view and returns an edge-zone mask consumed by `FUN_002f8370`: `0x1000/0x2000` mark horizontal offscreen, `0x400/0x800` vertical offscreen, `0x100/0x10` left edge bands, `0x200/0x20` right bands, `0x40/0x04` top bands, and `0x80/0x08` bottom bands. `0x1` marks the interior horizontal zone; `0x2` is added when no vertical edge-zone bit is set. Projection failure returns `0x4000`. `FUN_002f8370` combines those results with camera/object state into update flags at stack `+0x34`. `FUN_002fcd50` consumes those flags differently for Behind versus other rows: normal rows use bits 1/2 to adjust yaw command field 3 by ±field 8, and bits 4/8 to adjust pitch command field 2 by ∓/±field 7; bits `0x10/0x20` can zero yaw/pitch when the camera state permits. Behind instead zeros depth and yaw commands, uses bits 1/2 to adjust X command field 0 by ±field 5, and uses 4/8 to adjust pitch. These are automatic framing flags from projected points and scene state, separate from pause-menu camera selection.

## Native mode selection and projection

`FUN_003038b0` constructs four camera helper objects: `+0x24` via `FUN_00300e90` (vtable `0x498848`, callbacks `0x003010f8/+0x1c:0x00301598`), `+0x28` via `FUN_002f7b00` (vtable `0x4985b0`, pose callback `0x002f7c38` at `+0x14`, update callback `0x002f8370` at `+0x1c`), `+0x2c` via `FUN_003017c8` (vtable `0x498950`, callbacks `0x003018a0/0x00301fa8`), and `+0x30` via `FUN_003057a0` (vtable `0x498dd0`, callbacks `0x002fabb0/0x003058c8`). The six row index belongs to object `+0x28`. In the non-override path, saved camera type bits `((ushort at descriptor+0x12c) >> 6) & 7` map types 0..4 to row indices 0,1,2,3,5 (unknown/default values map to 0). `FUN_003038b0` forces row 5 for its special descriptor type 3, and also for type 4 when the player-count check succeeds.

`FUN_002f7bb0` changes object `+0x28`'s mode `+0x1f0` and selects the corresponding row pointer at `+0x1f4`; it preserves the alternate row `+0x14`/`+0x18` pointer variant when switching to modes 0 or 1, otherwise it selects row `+0x10`. It then calls `FUN_002fa4f8`, which resets the one-step count and dispatches the selected object's `+0x1c` callback. This distinguishes the six table rows from the four helper objects/callback sets.

The integer row words at byte offsets `+0x20/+0x24` are `[185,115]`, `[185,115]`, `[170,100]`, `[230,90]`, `[300,90]`, and `[185,115]` for rows 0..5. Raw instructions and decompilation show `FUN_002f8370` loads the first value from `0x498498 + mode*0x34` into one horizontal threshold slot (left or right, selected by the sign branch), and the second from `0x49849c + mode*0x34` into the top threshold slot. `FUN_002ff830` consumes its eight-int argument as left threshold/band width, right threshold/band width, top threshold/band width, and bottom threshold/band width, comparing those values against projected screen coordinates. The row values are screen-framing margins, not angular lens values; the camera state can add 100 to the top threshold or adjust the horizontal thresholds. The projection path is also traced below; the screen-edge controller response still is not reproduced in Rust.

The projection sequence establishes the camera-angle conversion. `FUN_002fa1e0` stores `FUN_002ff818(camera+0x98)` at camera `+0x80`; `FUN_002ff818` multiplies by 4/3. `FUN_00300990` computes twice the atan-like function of camera `+0x80`, in degrees, and `FUN_003046c0` passes that value as the first float argument to the renderer's camera callback. Raw EE instructions at `0x3049e0..0x304a08` load the callback and pass `f20` in `$f12`; the callback at vtable `+0x34` is `FUN_00323450`.

For the default 4:3 branch, `FUN_00323450` calls `FUN_00378328` with aspect 4/3. Raw instructions in `FUN_00378328` compute the projection coefficient as `1 / (tan(fov/2) * context+0x190)`, write X coefficient `context+0x120 * coefficient` at matrix `+0x20`, and write Y coefficient `-aspect * context+0x124 * coefficient` at matrix `+0x34`. `FUN_00377fe8` sets `context+0x120` and `+0x124` to 1 for the full viewport; otherwise they are viewport-width/height ratios. `FUN_00379810` initializes the viewport subobject through `FUN_003797e0`, which sets its absolute `+0x190` field to 1.3. Thus for a full 4:3 viewport, the horizontal half-angle tangent is `(4/3)*camera+0x98`; the matrix applies zoom 1.3 and the Y coefficient's additional 4/3 factor. The equivalent vertical half-angle tangent is `1.3*camera+0x98`.

Rust sets Bevy's vertical FOV from the traced matrix terms. It now evaluates the native atan polynomial with exact coefficient bits `3e2bba25/3d8a908b/3d55033a/3d6137ab`, emits the horizontal degrees using `57.295776f`, and reconstructs the renderer half-angle using `0.017453294f` before applying tangent and zoom. This preserves the small native approximation error rather than canceling atan and tangent algebraically. Rust's tangent can differ from the EE fdlibm kernel by an ulp. In the ordinary state-`0x12` path, `FUN_002f8370` always adds controller flag `0x40`, which makes `FUN_002fd808` skip the `+0x98` zoom step; this particular zoom integrator does not change the scalar during ordinary play. Street (row 0) separately applies persistent height/lens offsets before command generation, so its ordinary-play lens is not necessarily the base row scalar. Other camera states can update `+0x98`, but that state-specific behavior is not ported. When the saved Widescreen setting is on, Rust follows the observed renderer branch with aspect 16:9 and `context+0x10 == 0`, which keeps the incoming horizontal FOV; the equivalent vertical tangent is `0.975*row_word7`. The renderer has a second widescreen branch that scales the horizontal half-angle tangent by 4/3 to preserve vertical FOV, but its `context+0x10` condition is not exposed as a game option and is initialized to zero in the inspected constructor. Near/far mapping and viewport crop behavior remain renderer-specific.

Street-specific offsets are globals `0x400b58` (eye-height offset) and `0x400b5c` (row-scalar offset), both initialized to zero. At the start of each normal callback, `FUN_002f8370` assigns the row height/scalar to `+0x88/+0x98`; Street then adds these two globals. A depth-offset pointer `+0x1f4 == row+0x14` moves height toward 4 by -0.16 per update and lens toward 0.18 by `(0.18-0.33)/50`; pointer `row+0x18` moves height toward 18 by +0.12 per update and lens toward 0.33 by `(0.33-0.18)/50`. The base-pointer path moves the offsets back toward the base row targets using row tail fields. A separate `0x401080` depth-offset pointer, enabled by global `0x400b1c`, uses globals `0x400b24/0x400b28` as its lens/height targets (initialized .25/12). These offset increments occur after the current callback has applied the old offsets. Ported in `NativeCameraState` (the played-match section below).

`tools/camera_oracle.py` is a standalone state-value comparison kernel. It accepts explicit native focus/motion/flags/offset inputs or an already observed 13-float command record, produces float32 ordinary planar/angular commands, and predicts the discrete velocity/eye/angle/scalar integration. It rejects unimplemented Behind, offscreen nonlinear pitch scaling, and Street/Court transition-correction branches rather than silently approximating them. It does not derive native gameplay predicates and is not wired to production rendering. Its `projection` helper uses matching native forward/right/up basis vectors at `+0x10/+0x20/+0x30` to reproduce `FUN_002ffac0`'s pixel equations and `FUN_002ff830`'s ordered zone comparisons; this framing projection uses the raw `+0x98` scalar with no renderer 1.3 zoom. A nonzero `+0x164` yaw offset requires a separately rotated basis and is currently rejected. The `normal_flags` helper converts explicit focus/reference/alternate zones into the non-Behind flags from `FUN_002f8370`, including the final `0x40`; it requires the alternate-zone input because Street/Wide and offscreen-reference branches can replace that zone. Stationary-focus, commanded movement, X-boundary, quarter/eighth-step and integrator zero/overshoot guards were exercised with synthetic snapshots; original-runtime agreement remains to be checked.

The persistent row-camera pointer chain is `context = u32[0x400058]`, `manager = u32[context+0x10]`, `row_camera = u32[manager+0x28]`. The directly rendered helper is `u32[0x402020]`, which can instead select manager `+0x24/+0x2c/+0x30` during authored or transition sequences. The row helper has vtable `u32[row_camera+0x1b8] == 0x4985b0`. Capture the full 16-byte `+0xc0/+0xd0/+0xf0` vectors, pose `+0x84..+0x98`, rates `+0x9c/+0xa4/+0xa8/+0xac`, and gates `+0x124/+0x128/+0x12c/+0x13c/+0x14c/+0x158/+0x15c/+0x160/+0x1a0/+0x1f0/+0x1f4`. The ordinary invocation sets `+0x124` to 1 before integration and leaves it 0 afterward; original runtime also confirmed native scale `0x4d8cfc == 60`.

## Focus and side

The default camera descriptor is type 3 with selector 0. `FUN_003052a0` reads the active ball object through `DAT_003d30bc + 0x130`. If its current-owner pointer at `ball + 0xf4` is nonnull and the camera follow flag is zero, it copies the owner's position at `+0x30`; otherwise it copies the ball position at `+0x30`. Raw EE instructions at `0x305338` and `0x305348` are `lq`/`sq`, transferring the full 128-bit position vector. `FUN_003003f8` stores the selected focus at camera `+0xc0`, copies it to `+0xd0`, and clamps focus z (`+0xd8`) to -26.5 when it falls below that value.

### Focus motion vector and state gates

`FUN_003003f8` reads the focus descriptor type at camera `+0x1a0` (descriptor `+0x20`), not the camera row mode at `+0x1f0`. Its special types 2/3 therefore do not mean Slide/Solid; ordinary ball/owner follow uses descriptor type 3. At entry, raw `lq/sq` at `0x300470/0x300474` saves the old full `+0xc0` vector on the stack before `FUN_003052a0` refreshes `+0xc0`. For a positive substep count at `+0x124`, the ordinary vector calculation is:

```text
source_motion = (new_focus_c0 - saved_old_focus_c0) / substep_count_124
```

This is a difference divided by the count, with no blend of the prior `+0xf0` vector. At `0x3008b4/0x3008b8`, raw words `da0300c0/dba40000` are `lqc2 vf3,+0xc0(s0)` and `lqc2 vf4,0(sp)`. Word `4be4196c` at `0x3008bc` is `vsub.xyzw vf5,vf3,vf4`. After storing/reloading the difference and transferring the count to `vf3.x`, `4a6303bc` at `0x3008e0` computes `Q = vf0.w / vf3.x = 1/count`; `4a0003bf` waits for Q and `4be0211c` multiplies all four components by Q. `fba40030` stores the full result, then `lq/sq` at `0x3008fc/0x300900` transfers it to `+0xf0`. The equivalent type-2/3 sequence is at `0x3007dc..0x300828`. The instruction fields and Q arithmetic are corroborated by PCSX2's [EE COP2 opcode table](https://github.com/PCSX2/pcsx2/blob/master/pcsx2/R5900OpcodeTables.cpp) and [VU operation implementation](https://github.com/PCSX2/pcsx2/blob/master/pcsx2/VUops.cpp). Standard Rust float arithmetic would still require checking EE/VU rounding and exceptional-value behavior for a bit-exact port.

`FUN_003052a0` returns a boolean, not the selected owner's pointer: it returns true for the ownerless ball path, and false for owner-follow or descriptor types 0/1/2. Camera `+0x14c` saves that boolean; the local change flag compares the old and new boolean, not player identity. For descriptor types 2/3, a nonpositive count or changed boolean seeds the vector from the global zero vector at `0x4d8be0`. Unless `+0x13c != 0`, a changed descriptor type (`+0x128`), changed boolean, or vector XYZ magnitude greater than `5280/(config[+0x28]*3600)*250` replaces this result with `FUN_00305360`'s source motion vector. Config `+0x28` initializes to float 60 (`0x42700000` in `FUN_0030f938`), giving a threshold about 6.111111. Type 2 returns the selected object's `+0x40` vector; type 3 returns the owner's `+0x40` when owner-follow is active, otherwise the ball's `+0x40`. Other descriptor types return the global vector at `0x4d9090`. These source-object offsets are observed motion vectors; their precise producer and units still need tracing.

Before that calculation, a separate stationary-source gate can write the global zero vector to `+0xf0` and restore `+0xc0` from `+0xd0`: it requires `+0x13c == 0`, an unchanged ownerless boolean, the same nonnull source entity as global `0x401098`, no entity `+0xa4` bits from mask `0x7ffdc`, all four `FUN_00305360` vector words equal to the zero vector, and `FUN_00300b98(entity) == 0`. The helper then records the entity globally and the descriptor type at `+0x128`. Finally it copies full `+0xc0` to `+0xd0`; if the latter's z is below -26.5, it clamps `+0xd8` and zeros motion z at `+0xf8`. The unclamped `+0xc0` remains the next invocation's difference reference. None of these source-motion gates is currently executed by the Rust presentation proxy.

Rust uses the held player's court-plane position and `Player::height` for the active-owner case, and the ball's full XYZ position for Shot, Pass, and Loose. The actor's vertical coordinate is at `+0x34`: `FUN_00153278` clamps it against the actor's floor baseline, and sim evidence maps it to rendered player root Y. Release and flight handlers clear the native owner pointer, so those states use the ball position. `camera_focus` applies the native z floor before the mode pose. A controlled original-runtime camera capture would still be useful to check the complete reconstructed focus vector in context.

The side helper `FUN_001404c0` reads a signed byte at manager `+0x98 + selector`; the bytes initialize to +1 for team 0 and -1 for team 1. `FUN_001420e8` selects the side index at `+0x110` from `+0x1a8` and toggles the latter. Rust maps possession team 0/1 to +1/-1 (`street_sim::restart::side`), and since 2026-10-16 the simulation is in the original's frame, team 0 attacking +x as every capture's side bytes have it (docs/TRACE.md "The court's frame").

## Active helper selection and original attract runtime

A 1000-sample, approximately 11.6-second unmodified original attract-mode capture (`street-internal-attract.json` in the temporary directory) has the row helper at `0x1931a00` inactive throughout, while the rendered helper has vtable `0x498950` (manager `+0x2c`). The saved row helper's pose, focus and motion remain constant while the active helper's values change. This establishes the observed attract path, not the helper used in every normal match. Consequently the six row formulas and Pause selector are not a sufficient account of the actual camera used in this capture.

The `+0x2c` helper is constructed by `FUN_003017c8` with descriptor `0x4988c0`, whose state `+0x88` is 3. `FUN_00301858` sets its eye from an event-selected position; `FUN_00303c80` chooses randomized positions and distance scales for this path. Its update `FUN_00301fa8` calls `FUN_003003f8` with the alternate ball-focus argument 1, derives `camera+0x1c4 = camera+0x1c0 / length(eye[+0x84] - focus[+0xe0])`, and calls `FUN_00301e08` to update `+0x98`. The target ratio is separate from the row lens values. Raw instructions at `0x301d50..0x301e04` recover the easing omitted by the current decompilation's call-return boundary:

```text
delta = (target_ratio - scalar_98) * 0.25
damping = 0.25 if global_400b40 == 0 else global_400b44
scalar_98 += delta - clamp(delta*damping, -60/config_fps, 60/config_fps)
```

The inspected defaults and live original EE reads have `0x400b40 == 1`, `0x400b44 == .8`, and config FPS 60, so ordinary small differences advance by 0.05 of the difference per update. `camera_oracle.py` accepts `helper: "positioned_lens"` with `scalar`, `distance_scale`, `eye`, and `focus_e0` to check this arithmetic. Synthetic exact-distance cases pass. Read-only original PINE checks on the active positioned helper recover the target ratio within about 1.5e-8 on stable snapshots; a 100-sample burst also contains a 1e-4 outlier consistent with the documented non-atomic read limit. A second 1000-read burst produces four unique scalar/target pairs whose three adjacent lens changes match the 0.05 recurrence within about 1e-8. A saved repeat at `$EVIDENCE/street-camera-positioned-lens.json` contains seven unique samples: all six adjacent updates match the float32 oracle bit-exactly, and the geometric target ratio differs by at most 2.98e-8. Its metadata records the stable active helper, damping, read-only intervention status and non-atomic limitation. This is an unmodified active-helper value comparison, not a row-controller comparison.

This helper dispatches `FUN_002ff1c8` with initial flags `0xc0`; that path invokes `FUN_002fed00` to derive angular framing commands and state-3 correction helper `FUN_002fc3f8`, not the row/state-0x12 controller. `FUN_002fed00` uses the `+0xe0` focus projection and camera `+0x134` framing state; its angular step is `atan_approx(+0x98)/(S*.167)`, while planar step constants are `42/(S*.5)` and `26.5/(S*.5)`. It can apply squared screen-error factors while recovering the focus into the interior. The oracle does not implement these active-helper angular/zone gates; the Rust camera does (opening toss section below).

Manager `+0x4c` is initialized to `config[+0x78] == 0`, where the native config descriptor returned by `FUN_001e0bc0` is `0x4bbf10` (type field `0x4bbf88`). `FUN_00303248` chooses the row helper only when manager `+0x40 == 0`, the active helper is not the blend object, manager `+0x4c == 0`, and its event listener's `+0x20 == 0`; otherwise it selects the positioned helper through `FUN_00303c10`. `FUN_003037b8` similarly returns toward the row helper through a native blend only for `+0x4c == 0`. Pause's selector bits still affect manager `+0x28` through `FUN_002f7bb0`; they do not change the positioned helper's table or force it active. No controlled intervention has yet been used to change these selection gates.

### Camera manager events and helper selection (retraced 2026-10-14)

An earlier description here said the positioned helper runs whenever `Control.demo` is set and that every staged exit blends to the row over 15 ticks. Neither is the original's behaviour. The manager below is traced from the ELF and ported in `crates/street-game/src/view.rs` (`CameraManager`, `CameraTick`).

**Attract flag.** Manager `+0x4c = (config[+0x78] == 0)`. Config `+0x78` is the front end's mode (2/3 Pick Up, 4 League Challenge, 5 Be A Legend). Mode 0 is the attract match: `FUN_0029ec08` and `FUN_002d3ed8` alternate it with the attract movie, and `FUN_001dba68(config, 0)` calls `FUN_001db7f8`, which picks a random court and two random teams. The rewrite's `--demo` watch match (`Control::demo`) stands for this mode.

**Events.** `FUN_003038b0` builds a listener (vtable `0x498a68`, eight bytes per slot) and registers it on the game's event list with `FUN_00190e00(game + 0x124, listener, 7)`. Game code posts a record whose first word is a descriptor in `0x3c6480..0x3c6960`; the descriptor's accept function (`0x1048f8..0x106108`) calls one listener slot. The overridden slots are listed below. The senders were found from the descriptor addresses:

| Slot | Handler | Sender and event | Effect | Ported |
| --- | --- | --- | --- | --- |
| `+0x24` | `FUN_003028c0` | Actor VM jump opcodes (`0x199868`, `0x19a5b4`) while the actor is on the floor | When the jumper owns the ball: replay request `+0x58 = actor + 3` and listener `+0xc` = 3 if ball z > 0.793225, else 2. Attract: a coin (`FUN_003b42a0() & 1`) re-places the courtside camera. | Yes, as "the owner leaves the floor" |
| `+0x5c` | `FUN_00302a78` | Shot launcher `FUN_0014aaf8` (`0x14bf4c`); kind `ball + 0x184`, made flag `+0x10` (the launcher's `a1`) | Kinds 3 and 4 (dunk, alley-oop) when made: the row's height shake `FUN_002f8338` (0.5, 15, 40) while the row is on screen. Other kinds: `+0x58 = 1`, and in attract the coin. | Yes: the shake on a made slam, the request and the attract coin on other releases |
| `+0xb4` | `FUN_00303600` | `FUN_0014cb28` when the previous ball state was 7 (the toss) | Resets the active helper's focus to the default. | Yes: the toss's catch |
| `+0xc4` | `FUN_00302f00` | `FUN_0014cb28` (`0x14d160`): every change to a held ball | In live play, attract, with the courtside camera on screen: the coin. A new owner with actor `+0xa4` bit 7 or 15 (dunking, alley-oop): request `owner + 3` and `+0xc` as for `+0x24`. | Yes (a catch while dunking or in an alley-oop) |
| `+0xd4` | `FUN_00302c20` | Basket publisher `FUN_001515a8` | Unless `+0x40`: a gamebreaker basket (`ball+0x178`), the won game's or with Explosive Rims any basket notes the scorer (`+0x10`); the first two turn the helper on screen (a blend's target) to the point `(±36.75, 11.957, 0)`; the row on screen (or blended to) shakes (0.75). A gamebreaker basket sets `+0x4` (not in a lesson or a won game); Level 1 also `+0x50 = 120`. Always: the winning basket (`ball+0x178 < 2`) sets `+0x50 = 60`, `+0x4`, `+0x8` and the request `scorer + 3`. | Yes |
| `+0xe4` | `FUN_00302510` | Game phase 6 to 4 in `FUN_0013e4c8` (`0x13e6a0`): the restart after a basket | `+0x20 = 0`. Ordinary match, unless `+0x40`: the row with the default focus, settled, unless it is on screen and `+0x4` is clear. Attract: a new courtside camera only when the active focus is not the default one. Clears `+0x4`. | Yes, mapped to `Event::Inbound` |
| `+0x144` | `0x303640` | The opening toss launch (ball state 7, `0x14cac0`) | Clears `DAT_0040200c`. | Yes (the rewrite's toss, below) |
| `+0x14c` | `FUN_003025e8` | AI task `FUN_0010b820` posting `0x3c6510` as the inbounder walks to the check spot (x `+4`, z `+8`, actor `+0xc`) | Street row, a human side in possession, `|z| < 26.5`: an eye at `(x ± 7.5, 12, z ∓ 3)`, scale 6.75, on that actor. | Yes (`CameraManager::watch_inbound`, played-match section below) |
| `+0x1a4` | `FUN_00302488` | `FUN_0013f1f0`'s shot-clock violation (`0x3c67b0`) | Unless `+0x40` or a blend: a courtside camera on the offending team's last carrier (`FUN_0014aac8`). | Yes (`Event::ShotClockViolation`) |
| `+0x1ac` | `0x3051f0` | Game restart `FUN_0013d388` | Owes the intro cut again (listener `+0x28`). | Once per match |
| `+0x1c4` | `FUN_00303868` | Game over (phase 9) | Stops the manager loop (`+0x44`). | No |
| `+0x1d4` | `FUN_003030c0` | Replay start (`FUN_00328ab8`, `0x328e34`) | Nothing during a blend. A courtside camera (`FUN_00303c80`, argument 0), settled; `+0x48 = 1`. With `+0x40` and `+0x18 & 3` clear (a gamebreaker's first replay), a coin: even, and no ceiling (`ball+0x140`): y 100 (`+0x18 |= 1`); otherwise x = `+0x1c * 42` (`+0x18 |= 2`). | Yes |
| `+0x1dc` | `FUN_00303248` | Replay end (`0x328b70`, `0x328c7c`) | Nothing with `+0x40` or during a blend. Ordinary: the row, settled (a courtside camera on actor `+0x24` when `+0x20` is set). Attract: a new courtside camera. | Yes (`+0x20`'s setter, slot `+0x134`, is not mapped) |
| `+0x1ec` | `FUN_00303370` | Script start (`FUN_00337110`) | The staged helper. | Yes |
| `+0x1f4` | `FUN_003033a8` | Script end (`FUN_00337250`) | The row, settled, unless the pregame intro sequence still runs (`FUN_0013e358`). | Yes (no blend) |
| `+0x1fc` | `FUN_00303458` | Intro sequence end (`FUN_00184df0`, `FUN_00184e58`) | Once per match: the midcourt cut, then the one-second hold and the blend below. | Yes |
| `+0x204` | `FUN_00303650` | Gamebreaker activation (`FUN_00159cf0`; `+8` is 2 when the team has one pocketed) | Skipped for kind 2 at a spot `FUN_00141298` stages. `+0x18 = 0`, `+0x1c` = team 1's side byte, `+0x40 = 1`; a narrower courtside camera (half width 23, scale 2..5) blended in over `fps / 2` updates (attract: cut). | Yes |
| `+0x20c` | `FUN_003037b8` | Gamebreaker task end (`FUN_00144ef8`) | `+0x40 = 0`; a blend back to the row over `fps / 2` updates (attract: a courtside camera). | Yes (timing below) |
| `+0x22c` | `FUN_003027f0` | Countdown `game + 0x104` reaching zero (`FUN_0013d868`) | As `+0xe4`. | Folded into `+0xe4`: the rewrite's dead-ball timer ends in its inbound |
| `+0x234` | `FUN_00302b98` | The pocket button (`FUN_00159eb8`, `0x3c68d0`) | The row's shake, if the row is on screen. | Yes (`Event::Pocket`) |
| `+0x23c` | `FUN_00302be8` | A cancel (`FUN_00159f40`, `0x3c68e0`) | The row's shake, whatever is on screen. | Yes (`Event::Cancel`) |
| `+0x274` | `FUN_00302798` | Lesson launch `FUN_0018ec28` | The row. A lesson (game mode 4) has no intro sequence, so no intro cut. | Yes |

**Per update.** `FUN_00304548` (manager vtable `+0x34`) runs `FUN_00304a38` and then the active helper's update `FUN_002fab68`. `FUN_00304a38` does the following:
1. More than one second after the midcourt cut, it settles the row and starts the blend helper over 55 updates (`0x37`). The cut time comes from the global clock at `gp - 0x7d0c`, counted in `DAT_004b32e0` units per second. The outgoing pose is frozen when the row is Slide or Solid.
2. A blend whose count reached zero hands the screen to its target.
3. It runs the `+0x50` countdown: down one while the game is won (`FUN_0012e800`) or while no replay (`+0x420+0x28` 2 or 3) or script (`+0x424+0x50` 3..7) runs; at zero with `+0x4` set, a courtside camera on the noted scorer (`FUN_00303c10` with an actor focus).
4. It applies the saved Pause row again. `FUN_002f7bb0` settles the row helper only when the row changes.
5. When the courtside camera is on screen and `+0x40` is clear, two checks run. During a replay (`game + 0x420 + 0x28 == 2`), `FUN_003041f0` handles the replay request once `DAT_0040200c` is clear (raw `0x3041f0..0x304540`): a release (request 1), or a request from another update (`+0x60 != +0x5c`), re-places the courtside camera; otherwise after the winning basket (`+0x8`) it sets `+0x50 = 60` and `+0x4`; otherwise, with `+0xc` set and Explosive Rims off, `FUN_003b42a0() % 101 < DAT_00402024` (60) and `+0xc != 1` put the eye beside the basket on the ball's side, `(±37.75, 12 + 6 rand, ±3.25)` (z negative for `+0xc == 3`), aimed at `(±34.75, 12, 0)` with scale 3; else `FUN_00303c80`'s baseline view (argument 1); then `+0xc = 0`. `FUN_00304d70` re-places the camera when any of the six actors is within 3.0 of the eye in 3D (raw `0x304d70..0x304e30`).
6. At the end it moves the request `+0x58` to `+0x54`, clears it, and records the game's update counter (`game + 0x128`, raised at the end of `FUN_0013d868`) in `+0x60`. A handler posted during the game update therefore meets `+0x60 == +0x5c` in the same frame.

**Helpers.**
- Courtside placement (`FUN_00303c80`; raw `0x303c80..0x3041e8`, where Ghidra drops z). With argument 0:

  ```text
  scale = rand * 3 + 3.5
  x = 2 * 36 * (rand - 0.5)
  y = rand * 6 + 0.8
  z = -35 - rand * 9
  ```

  With `+0x40` set, scale is `rand * 3 + 2` and the half width is 23. The z spread is 0 on config courts 0 and 6 (`config + 0xcc`). x is negated when it would land on the same side of centre as the previous eye.
- With argument 1 (the replay baseline view, raw `0x303f90..0x3041c4`): x is `rand * 18 + 24` on the ball's side, y is `rand * 1.5 + 0.5`, z is `-8 - rand * 4` and scale is `rand * 2.5 + 5.5`. It falls back to argument 0 when an actor is within 3. `rand` is `FUN_003a16b8`, ported as `NativeWideRandom` with the ELF's initial words; in the original the sequence is shared with every other caller.
- Midcourt cut (`FUN_00303458`): `rand() % 4` selects `(4.1, 0.5)`, `(-4.1, 8)`, `(-4.1, 0.5)` or `(4.1, 8)` at z -15, with scale 6.75.
- Focus `+0xe0` (`FUN_003003f8` with its alternate argument, raw `0x300638..0x300678`; `FUN_00305488`): `+0xc0` plus the owner's bone 21 and bone 10 offsets from the actor's position, mixed by `w = clamp((scale - 4) / 15, 0, 1)`. A free ball uses `+0xc0` alone.
- Lens (`FUN_00301fa8`): `+0x1c4 = scale / |eye - e0|`, then `FUN_00301d50`'s easing. A selection settles (`FUN_002fa4f8` then `FUN_002fabe8`): the pose callback `FUN_003018a0` aims at e0 and takes `+0x1c4` as the lens.
- The eye does not move. For descriptor state 3, `FUN_002fb690` leaves the planar commands at zero; only pitch and yaw change, through `FUN_002face8`, `FUN_002fed00` (flags `0xc0`) and `FUN_002fc3f8`, ported as `PositionedCameraHelper::steer` (section below).
- Blend (`FUN_00305840(blend, from, to, count, freeze)` and `FUN_003058c8`): each update counts down, sets `t = count / duration`, and mixes every pose word as `t * from + (1 - t) * to`, angles unwrapped. Both helpers keep updating unless the source is frozen.

**Runtime corroboration (read-only captures, retained outside the repository).**
- `street-native-full-rig-active.json` comes from an attract match (manager `+0x4c == 1`). The staged helper stays on until the intro ends. The positioned helper then sits at exactly (-4.1, 8, -15) with scale 6.75, and `+0x1c4` equals `6.75 / |eye - e0|`. The blend helper appears about one second later. Its integer counts 53..1 reproduce every blended eye from the two fixed eyes (error below 1e-4). The row takes over 55 updates after the blend started, and the referee's toss (ball state 7) is on the next sample.
- At sample 500 of the same capture, the positioned focus equals `+0xc0` at the origin (game phase 1 zeroes its x and z, raw `0x300600..0x300618`) plus the bone 21/10 mix, within 1e-7.
- `street-internal-attract.json` has three courtside eyes: (-17.07, 4.92, -35), (28.49, 4.66, -35) and (-19.21, 3.75, -35). They alternate sides inside the traced ranges. The camera was re-placed at a shot release (ball state 3 to 5) and at a catch (4 to 3).

**Scenery by camera side.** After each update `FUN_00304548` calls `FUN_00304e38`. It sets flags from the active eye: x > 44.1 sets 1, x < -44.1 sets 2, z > 27.825 sets 4 and z < -27.825 sets 8. The opening referee phase also forces 8. `FUN_002f9d10` then hides every registered object whose mask meets the flags and shows the rest. While a script plays (`DAT_0040108c`, set by `FUN_00303370` and cleared by `FUN_003033a8`), it shows them all. Masks come from tags in the name (`FUN_002f9ab0`, `strstr`): `clef` 1, `crit` 2, `ctop` 4 and `cbot` 8. They are registered for court model parts (`FUN_00318f58`) and for crowd seats (`FUN_0030b708`, called from the seat loader `FUN_0030b9b8` with the seat's name). Rucker's near-side bleacher crowd is 80 seats named `*_cbot` (z -49.4..-35.7), and Foster Beach has 9. Before this change the rewrite always hid the `cbot` court groups and never the crowd, so a courtside eye sat inside the near crowd. `view::cull_scenery` now applies the flags each frame (`scenery_culls_the_sides_behind_the_camera_like_fun_002f9d10`).

**Rewrite mapping.** The manager reads its events at each 60 Hz camera update, and only when the simulation advanced. Posted events come from the update's `TickEvents` (the live tick's, even when a basket swaps the world for a replay's snapshot), in their order:
- A script start or end is a change in the staged camera.
- The intro's end is the pregame `MatchIntro` part finishing. With no intro, it is the match's first update; a School lesson has none.
- Replays are `tape::Tape` playing (the manager's state 2: an instant replay or a gamebreaker's two replays; `Replay::playing` mirrors it).
- A restart is `Event::Inbound`.
- A takeoff is the held owner's height leaving zero.
- A release is the ball entering `BallState::Shot` from someone not dunking; a made slam is `Event::Slam` with the slammer's `Event::Score`.
- A pickup is a new owner in live play; a dunking or alley-oop catcher's is a replay request.
- A gamebreaker is `Event::Gamebreaker`, its task's end `tape::GamebreakerTask::ended`, a basket `Event::Score`, the won game `Phase::Over`, a pocket, cancel or shot-clock violation its own event.
- The toss's launch and catch are the world's `OpeningToss` changing.

Script and director gating is the staged camera or intro being on screen.

Not ported:
- Slot `+0x134`'s `+0x20` (sender not mapped) and slot `+0x154`'s `+0xc = 1` (`FUN_00156f48`'s post at `0x157350`, predicate `FUN_0016ea50` not identified).
- Behind's own update (the `+0x20c` turn, its flags and x commands); Pause never offers it, and nothing else selects row 4 (dead code in the original: "The row's selection, the source adapter's inputs and the shared roll (2026-10-18)").

The coin flips and placements use the native generators, but not the original's shared sequence (the placements draw `FUN_003a16b8`, the gameplay generator, in the Camera task; the coins `FUN_003b42a0`).

### Opening toss, gamebreakers, celebrations and replay cuts (2026-10-15)

**The opening toss.** The original opens every match with the referee's jump ball (game phase 1). When the intro sequence ends, the game's listener (`0x43da50`, slot `+0x1fc`, `FUN_00142368`) calls game vtable `+0x15c`, `FUN_00141fd8`: phase 1, both teams to their tip-off spots (`FUN_0018af20`: table `0x3cfbb0` read by `FUN_0012e418`, `(-3.25, 0)`, `(3.25, 7)` and `(-7, 0)` by slot, x times the team's side byte; `FUN_00170608` stands each facing the origin), the referee (`AIPhysReferee`, `game+0x170`) placed by `FUN_0017cf28` at x -0.9, and the ball given to him (`FUN_0014cb28`). His task `FUN_0011aea0` waits until the six "wait for toss" tasks report ready, counts `DAT_004b5270` (the frame rate, 60, written at `0x11b230`), then presses `0x200000`; `FUN_0017c8e0` starts the toss script `0x44cb8f`, which blends 8 frames to record 949, reaches frame 6 over 50 updates and calls callback 44 (`0x3e8780` = `0x17d278`) into `FUN_0014c858`. That launch rises to 20.0 and lands on the centre spot: with the per-update gravity `g` (`ball+0x118`) and the resting height `r` (`ball+0x11c`), `up = sqrt(2 (20 - y) / g)` (the EE `sqrt.s` reads `ft`, raw `0x14c8b0`), `T = up + sqrt(2 (20 - r) / g)`, velocity `(-x / T, up g, -z / T)` per update, a random unit spin axis, state 7 and the post that reaches the camera's `+0x144`. Phase 1 lasts until someone has the ball (`FUN_0013ee10`); `FUN_00140880` unfreezes the players at the launch. In phase 1 `FUN_00304e38` adds scenery flag 8 and `FUN_002f8370` lifts the row's focus to at least 4.

The retained attract capture agrees: the intro cut at sample 490, the referee at (-0.9, 0, -3.0) holding the ball, the blend about 62 updates later, the row at sample 583 and the ball in state 7 at sample 584, about 118 updates (16.75 ms each) after the cut, launched from (-0.3217, 6.3693, -0.7814) with (0.0029746, 0.55380, 0.0072253) per update. The two jumpers tip it at about 14.7 on its way down (sample 655, ball state 4 to a teammate).

The rewrite ports it in the simulation (`World::start_opening_toss`, `OpeningToss`, `toss`; snapshot version 23), started by `setup::load_match` for three-on-three matches in place of the old formation inbound. The players stand at the spots facing the centre, the ball waits at the capture's release point and goes up after `toss::WAIT_UPDATES` = 60 + 8 + 50 = 118 updates (the capture's figure) with `FUN_0014c858`'s law, which reproduces the capture's velocities within 2e-6 per update (`the_opening_toss_waits_at_the_tip_off_spots_then_goes_up_like_fun_0014c858`). Its own: no tip: the players' rebound chase takes the falling ball high, as the jumpers do, and the catch ends the phase without a rebound award. Online and other non-three-a-side matches still start with an inbound.

The camera follows `DAT_0040200c`: the intro cut sets it, the toss clears it, the intro cut's blend needs it (a toss within the hold leaves the cut on screen) and the replay cuts need it clear. Through the wait the default focus is the referee (owner path, stationary). `CameraManager::observe` reads the toss from the world.

**Gamebreakers.** `FUN_00303650` and `FUN_003037b8` are ported (`CameraManager::gamebreaker_on`/`gamebreaker_off`). The rewrite's activation is `Event::Gamebreaker`; a Level 2 one whose staged sequence starts is the skipped kind. The original's task (`FUN_00144df0`/`FUN_00144ef8`) waits `DAT_004b7d34` (60) updates, or `DAT_004b7d38` (40) when its owner is dunking, then plays the last `DAT_004b7d3c` (90) frames twice (`FUN_00328fc0` at 0.8125 and at 0.53125, mode 5), and only then posts the end; the values are written at `0x145574..0x1455c8`. A staged gamebreaker's task ends once the script manager is idle (its `+0x38 == 0` branch), mapped to the script's camera going away. The replays are now ported (second pass, below).

**Celebrations.** `FUN_00302c20`'s branches, the `+0x50` countdown in `FUN_00304a38` and `FUN_00302510`'s use of `+0x4` are ported as traced above (`CELEBRATION_TICKS`: `DAT_004d908c` is twice the frame rate, written at `0x305174`). `FUN_00131208()` is the cheat block (`FUN_002453e8` writes it from the code entry), whose `+0x14` is Explosive Rims.

**The row's shake.** `FUN_002f8338(amplitude, row, cycles, updates)` sets `+0x208`, `+0x204`, `+0x200` and `+0x1fc = 1/updates`. It is not a lens: each row update (`FUN_002f8370`, after it assigns the row's height) counts `+0x200` down and, while it is still positive, lowers the eye height by `amplitude * sin(cycles * pi * left / updates)` with an inline single-precision sine. Callers: a made dunk (`DAT_00402008` 0.5, `DAT_00402000` 15, `DAT_00402004` 40), a pocket, a cancel, and the basket publisher (0.75). Ported as `RowShake`.

**The positioned helper's angular controller.** `FUN_00301fa8` builds a framing window from descriptor `0x4988c0` (edges 110, 50, 50; bands 165, 110, 210 for the sides, top and bottom), scales the bands by `2 / scale` past scale 2 (and by a half while recentring at scale 4 or less), rounding each, and `FUN_00301e38` shifts the side edges by an eased lead: 15 pixels per pixel of `+0xe0`'s screen motion (`FUN_002fd970`), times `(scale - 2) / 4` below scale 6, within ±40, eased by a sixteenth (a quarter on a reversal; a settle takes it at once). After the lens easing, `FUN_002ff1c8` with flags `0xc0` gets turn commands from `FUN_002face8` on `+0xd0` and its motion, then `FUN_002fed00`: steps `atan(+0x98) / (S * 0.167)` for pitch and 4/3 of it for yaw; the zones of `+0xe0` (`FUN_002ff830`, no top margin for this helper); `+0x134` goes to 2 outside the inner region, to 1 or 0 back in the middle (by whether the descriptor's source vector moves), and while it is 0 a still subject zeroes both turns (`0x30`) while a moving one starts following. Recentring adds a step toward the window's middle on each axis, scaled by the squared offset (`/ 320`, `/ 240`, the downward one times four, each at most 3). `FUN_002fc3f8` applies the flags; off screen the turns grow by an eighth with quarter corrections; otherwise each correction is a quarter of the difference, cut again to a quarter (an eighth inside the bands) unless it opposes a running rate (raw `0x2fc574..0x2fc684`). `FUN_002fda08` steps the rates and integrates. The projection (`FUN_002ffac0`) uses the native basis and the raw lens. Ported as `PositionedCameraHelper::steer` with plain float arithmetic; a settle aims straight at `+0xe0` (`FUN_002fabe8` then `FUN_003018a0`) and zeroes the rates and `+0x134`, folding the original's two settle updates into one.

### Referee, toss spin, gamebreaker replays and the restart snap (2026-10-15, second pass)

**The referee's position.** `FUN_0017cf28` stores the whole vector `(-0.9, 0, -3.0, 1)` at `+0x30` (raw `0x17cf2c..0x17cf7c`, `0xbf666666` and `0xc0400000`); the decompiler showed only the low half. The z the capture shows is this constant. His heading is the quantized angle of that position (`0x17cfb4..`, the same arithmetic as `FUN_0018ce78`).

**His model.** The referee's render object `FUN_003168b8` (built by `FUN_0017c600`, `AIPhysReferee`) loads `/XF/XF000002` entries `0x3a1` (model `reff`, 929) and `0x3a2` (its sheet), the shadow `0x3ac` and the 43-bone skeleton `0x3ad`; there is no morph target. `street_assets::player::referee` and `PlayerAssets::referee` load the same entries; `setup::load_match` puts them in the scene and `game::spawn_scene` spawns him hidden.

**His scripts.** The constructor and `FUN_0017c778` run script `0x4574e2` (a record-65 `breathe_1` assignment, opcode `0x89`). `FUN_0017cf28` runs `0x457412`: an 8-frame blend to record 656 (`stndrlx_cradle`), then `ab 2.5`, the clip played at one frame each 2.5 updates. His update `FUN_0017c8e0` reads his task's pad (`FUN_0013a970`); with the ball held and no script lock (`+0x48 & 2`), the press `0x200000` starts `0x44cb8f`: `91` flags, an 8-frame blend to record 949 (`ref01_regcourt_toss`) from frame 0, `8d ee 06 32` (frame 6 over 50 updates), callback 44 (the launch), `8c 1.5` (the rest of the clip at a frame each 1.5 updates) and a jump to `0x44cb78`: an 8-frame blend to record 947 (`ref01_regcourt_runoff`), `ab 1.0`. The neighbouring `0x44cb61` (record 948, `ref01_regcourt_runon`) is started by `FUN_0017cdd0` once no script holds him: with his task's stick past 16 he turns to it, moves a third of a unit an update and plays 948 (pointer `0x3d9a38`), so 947 shows for one update (the 2026-10-15 captures). The same press sets every player's `+0x1a8` to 20. None of the three scripts sets the root lock `0x1000`, so the clips' root travel moves him (`FUN_0019a658`). The opening phase ends when somebody holds the ball: `FUN_001415c0` (the game listener's slot `+0xc4`) and `FUN_0013e4c8` case 1 call his vtable `+0x3c` (`FUN_0017d1f0`) with 0, hiding him.

The rewrite draws him (`crate::referee`): placed at the intro's end facing the centre, cradling the ball at a frame each 2.5 updates; the toss clip from 50 updates before the simulation's launch, frame 6 at the launch; the rest of the clip at a frame each 1.5 updates, then the run-off clip at a frame an update, his root carried by the clips' travel (rendered in place); hidden once the toss is caught. The ball is in his hands until the launch, placed as a player's held ball by his clip's hand flags (`view::referee_hold`). Its own: the toss script's start is placed 50 updates before the simulation's launch (the six-ready wait and the count are folded into `toss::WAIT_UPDATES`), the 8-frame blends are the renderer's crossfades, and root travel during a blend follows the incoming clip.

**The toss's spin.** `FUN_0014c858` takes three draws of the shared generator `FUN_003a16b8` (raw `0x14c8bc..0x14c9ec`), each `(d & 0x7fffffff) * 2^-31 - 0.5`, normalizes them on the VU0 (or uses `(0, 1, 0)` at `0x4b83b0` when the length is at most 1e-4, raw `0x14ca10..0x14ca44`) and passes the axis with `DAT_004b854c` to `FUN_0014c408`. The ball constants' initializer writes `DAT_004b854c` at `0x154020`: `(1 / 60) * 2pi * 0.5` (the config's frame rate, `0x40c90fdb`, `0x3f000000`), half a turn a second. Ported in `World::launch_opening_toss` (`toss::SPIN`, `toss::SPIN_FALLBACK`; the normalization in plain float arithmetic).

**The music's tip-off.** The record `FUN_0014c858` posts at the launch (`0x3c6700`) reaches the music listener's slot `+0x144`, `0x1a6f38`, which posts path event 1 (`FUN_001aae48(manager, 1)`) after a speech call (`0x4a`). `song::tip_off` now starts the groove when the toss goes up; a match without the toss keeps the old start once the court's name has shown.

**Gamebreaker replays.** The replay manager (`FUN_003288b0`, "Replay Manager", game `+0x420`) records every update into a 300-slot ring (`FUN_00329248`; the actors through `FUN_00327450`: position, root height, rotation, 22 bone rotations, 21 packed vectors and flags; the ball, the camera manager's `+0x54/+0x60`, and other registered objects). `FUN_00328fc0(speed, manager, frames, 0, mode)` asks for a replay; the manager's next update (`FUN_00328ab8`) records one more frame, suspends the gameplay task groups (`FUN_0033d380(0x401b2)`), sets the position `+0x38` to the span's first frame and `+0x64` to `frames - 1`. Each update after it subtracts the speed from `+0x64`, moves the position on by it (`FUN_003290d0`) and shows the two frames either side, blended by the fraction (`FUN_00329718`, the actors' `FUN_003275d8` slerping each bone). With `+0x64` below one it asks to stop; the next update shows the newest frame, resumes the gameplay (`FUN_0033d3a0`) and posts the end (`0x3c6820`; the start `0x3c6810` carries the mode). The gamebreaker task's `FUN_00328fc0(0.8125, m, 90, 0, 5)` therefore shows 109 frames and `0.53125` shows 166, each followed by a stop and a restore update. The camera manager's recorder (`0x49b890`: `+0x54` and `+0x60` saved, written back to `+0x58`/`+0x60` on a frame's first showing) is why `FUN_003041f0` sees the recorded play's replay requests. The trick button's replay (`FUN_00329c68`) uses the same call at 0.8125, mode 2, and is refused while a gamebreaker task runs (`FUN_00140ff8`, raw `0x329cac`); the automatic instant replay plays at 0.90625 (third pass, below).

The rewrite ports the manager and the task in `crate::tape`: `Tape` records each 60 Hz update's world, each player's drawn place, rotation and animator, and the ball's drawn place; `GamebreakerTask` waits 60 (40 when the owner dunks) live updates after `Event::Gamebreaker`, plays both replays and only then ends (`CameraTick::gamebreaker_over`, which now drives `gamebreaker_off`); the simulation holds while the tape plays (`game::step_sim`), the renderer shows the blended recorded poses (`view::sync_players`, `view::sync_ball`), and the instant replay is skipped while the task runs. Its own: online matches skip the replays (lockstep peers), ending the task after the wait. The task order, the per-bone blend and the slow-motion post are settled in the third pass below.

**Restart formations.** The six players are placed by four routines:
- `FUN_00189cf0` (team construction): each actor created at `FUN_0012c748`'s spot when game vtable `+0x4c` is positive, else at `0x440988`.
- `FUN_00141fd8` (the opening, game vtable `+0x15c`): the tip-off spots (ported).
- `FUN_001407c8` (game vtable `+0x154`): both teams' restart strategies place their players (`FUN_0018adf8`), the possessing team first. Its callers are the game listener's replay-end slot `+0x1dc` (`FUN_00142648`: game phase 6, replay mode not 6 or 7, no gamebreaker task busy) and the lesson launch `FUN_0018ec28`.
- `FUN_001420e8` (the listener's slot `+0x1b4`, `FUN_0012c748` spots, alternating possession `+0x1a8`, clearing the period flags `+0xd8`/`+0xdc`): only the accept function `FUN_00105988` of descriptor `0x3c67d0` calls it, and no code builds a record with that descriptor or any copy of it (no `lui`/`addiu` pair and no data word refers to them). It is unreachable in the USA ELF; the rewrite's `World::restart_formation` port stays unused.

Restarts themselves are game phase 6 (`FUN_001406b8`, vtable `+0x14c`), entered after a basket (the listener's slot `+0xd4`, `FUN_00141818`, the other team) and after a shot-clock violation (`FUN_0013f1f0`); slot `+0x134` (`FUN_00142270`) has no sender either. Phase 6 runs the restart strategy (`FUN_0012bc40`): the inbounder (a carrier on the team, else the player nearest the ball), the row of `0x3cea20`/`0x3ceaf8` whose first spot is nearest the ball (all six rows: `FUN_00143068` answers 1 for every game class; only rows 2..5 while the shot clock is not full), the next spot to the nearer remaining player, and walk tasks to the spots (`FUN_0010b658`, `FUN_0010ba18`); once all six are ready and nobody holds the ball (`FUN_00140840`) it posts `0x3c6640` (the camera's `+0xe4`) and goes to phase 4. The modes have no period restart: phases 8 and 9 end the game.

Since the flow pass (2026-10-15) restarts are the original's phase 6: all six walk to their spots under the computer (`World::start_restart`, `ai/inbound.rs`) and the inbound pass ends it. The snap is ported: `World::snap_formation` (`street_sim::restart`) places both teams at the strategies' spots (`FUN_0012b8b8`, pushed past the lines by the player's height, `FUN_001819a8`), facing the next spot or the first (`FUN_0012c1c0`) or, for a defender, the middle of the first spot and his own (`FUN_0012c6a8`), and gives the ball to the inbounder; `tape::step` calls it when a replay ends with a restart pending (third pass). Its own: the second pick takes the branch for a side with fewer than two people playing it, the first spot it measures from uses the new inbounder's height, the shot clock is treated as full, and a lesson's forced row 2 is not applied; after the snap the restart finishes phase 6 as above.

### Instant replays, the replay director and the task order (2026-10-15, third pass)

The ordinary instant replay was the rewrite's own (an input re-simulation at a third of speed). It is now the original's, through the replay manager above.

**The director.** `FUN_0013c4e8` (the game's constructor) builds the manager (`+0x420`), the script manager (`+0x424`) and, last, the replay director `FUN_00324668` (`+0x42c`, vtable `0x49b518`, on the game's event list at `+0x124`), with a recorded object (`FUN_003244b0`, vtable `0x49b4f0`) registered with the manager. Its slots note the game counter (`game+0x128`) and the actor:

| Slot | Handler | Sender | Note |
| --- | --- | --- | --- |
| `+0x24` | `0x324838` | actor VM jump (`0x199868`, `0x19a5b4`) | `+0x20` takeoff, `+0x14` actor |
| `+0x4c` | `FUN_003247c0` | pass start `FUN_0014bff8` (`0x14c084`) | to an alley-oop flier (actor `+0xa4` bit 15) or to himself: `+0x28`, `+0x14` the receiver |
| `+0x5c` | `0x324798` | shot release `FUN_0014aaf8` (`0x14bf4c`) | `+0x18` release, `+0x14` shooter |
| `+0x64` | `0x3247b0` | the same, first, for a dunk or alley-oop release (bit 7 or 15) | `+0x1c` |
| `+0x84` | `0x324ca0` | `FUN_0014c408`, `FUN_0014cb28` | `+0x38`, `+0x3c` (only read by slot `+0x134`) |
| `+0x94` | `FUN_00324850` | pass launch `FUN_0014c0c0` (`0x14c2a4`) | `+0x24`; on Manual in game phase 6 it withdraws the offer (`FUN_00329c18`) |
| `+0xd4` | `FUN_003248d8` | basket `FUN_001515a8` (`0x1518cc`) | picks the replay, below |
| `+0x134`, `+0x174` | `FUN_00324c20`, `0x3248a8` | none: no code builds descriptors `0x3c66e0`/`0x3c6750` | a mode-6 replay and `+0x2c`, both dead |
| `+0x18c` | `0x3248b8` | `0x13f60c` | `+0x30` (never read) |
| `+0x1dc` | `0x324cb8` | replay end | `+0x10 = -1` |
| `+0x204` | `0x3248c8` | gamebreaker | `+0x34` |

The recorded object (`0x324500`) saves `+0x14` and three bits: `+0x18`, `+0x28` and `+0x20` equal to the counter. Its show slot (`FUN_00324588`), during state 2, when the frame's actor is `+0x14` and the bit for the kind `+0x10` is set (0: the release, 1: the takeoff, 2: the alley-oop pass), ramps the speed to 0.53125 over 16 updates (8 for kind 2) with `FUN_003299c8`.

**The basket** (`FUN_003248d8`; raw `0x3248d8..0x324c18`, where Ghidra stops at `FUN_0030fa50`, which returns the config `0x4da218` whose `+0x24` is the frame rate 60). The record is `[1]` the shooter, `[3]` the points, `[4]` the ball's kind `+0x184` (2 jump shot, 5 lay-up, 4 a dunk, 3 an alley-oop finish whose move lacks flag 0x800; raw `0x14ad10..0x14ad3c`). `+0x40 = shooter / 3`; nothing for no points. `+0x10` is 0 unless `+0x18 == +0x1c` (the last release was a dunk's or alley-oop's), then 2 for kind 3, else 1. A won game (`FUN_0012e800` through the game's vtable `+0xbc`) forces `FUN_00324cc8(+0x18, mode 2, delay fps*2.4 = 144)`. Otherwise the start frame, mode and chance are: a combo payout (`game+0xd0` `+0x150`) `FUN_00131d18` (the matched combo's oldest event tick), mode 1, `min(2 * payout, 35)`; else after a dunk release kind 3 `+0x24`, mode 3, 30, other kinds `+0x18`, mode 2, 10; else 0, mode 2, 0 (the `+0x2c` branch is dead). Config `+0xe8` 1 (Trick Point game) keeps only the combo's chance; 2 (dunks only) only kinds 3 and 4. On Manual (`+0xc42f0 == 2`, the save's `flags >> 12 & 3`), with no gamebreaker ball (`ball+0x178`) and no full meter (`game+0xc8/+0xcc` `+0xc` 50000), it offers instead: `FUN_00329af0(team, (now - start) + fps*1.5 clamped to the frames recorded, 0)`. Always (0) sets the chance to 100. Then `rand() % 100 < chance` plays `FUN_00324cc8(start, mode, DAT_004dbfe4 = fps*0.9 = 54)`.

`FUN_00324cc8` (raw `0x324cc8..0x324ec0`) refuses (and sets `+0x10 = -1`) in game mode 4 (a lesson), while a gamebreaker task runs, with a gamebreaker ball, while a script plays (`+0x424+0x50` in 3..7) or with `auto_replay` (`0x40f9c8`) off; unless forced, also with a full meter or within `DAT_004dbfe8` (fps*9 = 540) of the last gamebreaker (`+0x34`). The span is `(now - start) + 90`, clamped to the frames recorded; unless the game is won it offers it (`FUN_00329af0(..., 1)`); then `FUN_00328fc0(0.90625, span, delay, mode)`. A delay counts down in the manager's update (`+0x68`) and then asks for `span + delay - 1` frames.

**Offers and buttons.** `FUN_00329af0` (raw `0x329af0..0x329c14`) is refused while a script plays, with a gamebreaker ball or in game modes 4 and 5; it sets `+0xc42f4` (the span), `+0xc42f8 = fps*0.9`, `+0xc4300 = 1`, and either (automatic) `+0xc42fc = 54`, or (Manual) shows the prompt (`FUN_002bafe0(hud, 1, team)`) and asks for state 4, which records 54 more frames and then stops recording while the game goes on. In state 2 `+0xc42fc` counts down and at zero shows the prompt again. The pad dispatcher `FUN_0012f2c8` (in the "action tap" task, which `0x401b2` does not suspend) maps `AIACTION_START_REPLAY_PRESS` (0xe, Square by `TRACE.md`'s config) to `FUN_00329c68`: on offer, `+0xc42fc` zero and no gamebreaker task, and (when the other side has a person, `game+0x9a`) only for the scoring side (`FUN_0013ee00` is the director's `+0x40`), it replays `+0xc42f4` frames at 0.8125 in mode 2, holds the prompt off for `DAT_004dcb14` (60) updates and hides it. `AIACTION_CANCEL_REPLAY_PRESS` (0xf, the jump table at `0x43bbb0`) reaches `FUN_00130b88`: a script is skipped; in state 2, with the same side check, `FUN_00329d38` stops a replay that is on offer (`FUN_00329c18`: the prompt hidden, `+0x30 = 1`, the offer off). Leaving Manual in the options (`FUN_00328a50`) withdraws an offer too. The gamebreaker's replays are never on offer, so they cannot be skipped.

**The prompt is the only replay mark.** `FUN_002bafe0` is called only by the manager (`0x328e18`, `0x329bd8`, `0x329c38`, `0x329d14`). It shows `XLAY_GM` blob 1's element 0x3c (an image, align 4) at (50, 411) unless the attract match (config `+0x78 == 0`), and with Onscreen Help (config `+300` bit 2) element 0x3d (a button icon) after it and 0x3e (font 2, `0xfff6f4d7`) 3 further: `overlays.txt` 53 "Replay" (48 "Repeat" when element 0x41's `+0x1c` is set). `fe/REPLAY.ssh` belongs to the front end (`FUN_0023f440`). The rewrite's old blinking mark over every replay is gone; `hud::draw` shows the atlas's REPLAY part (the image is matched by the rewrite; which part code puts in element 0x3c is not traced), the Square icon (`XACOMMON` sheet 1) and the word.

**Slow motion.** `FUN_003299c8` posts `0x3c6830` (descriptor slot `+0x1e4`) for speeds below 0.8125 when no gamebreaker task runs and no script plays. No listener vtable in the ELF overrides `+0x1e4` (all, the audio's `0x45c3a0` and the camera's `0x498a68` included, keep the base `0x132d48`, `jr ra`), so the post has no audible effect and nothing in `street-audio` is owed. Its only senders are the moment's slowdown (0.53125) and the gamebreaker's second replay (refused by its task).

**Ending.** The restore update shows the newest frame (`FUN_00329718`, which writes back the recorded objects only) and posts `0x3c6820` with the mode. The game listener's `FUN_00142648`: game phase 6, mode not 6 or 7, no gamebreaker task: `FUN_001407c8` snaps both teams; phase 9 (game over) moves the game to its end (vtable `+0x17c`); the director clears `+0x10`.

**Task order.** `FUN_0033c998(task, name, mask, priority)` and `FUN_0033cf98` keep the task list sorted by priority, a new task after every one of equal priority, and `FUN_0033ce00` runs it in order: "action tap" 2, "AIGame" 3 (the game update `FUN_0013d868`, which raises `game+0x128` at its end), "Script Manager" 4, "AITeam" 5, "AIDetect" 6, "Camera" 10 (`0x800`), "Replay Manager" 14 (`0x1000`) and "game breaker" 14 (`0x1000`, created at the activation, so after the manager). The suspension mask `0x401b2` holds AIGame (2), AITeam (`0x10`), AIDetect (`0x40000`) and groups `0x20`, `0x80` and `0x100`, not the camera, the manager or the pad.

**The rewrite** (`crate::tape`): `Director` takes the update's events and world (a takeoff is a height leaving 0; a jump shot's or lay-up's release the ball entering `BallState::Shot`; a dunk's `Event::Slam`; an alley-oop pass `Event::Pass` to a player in `Action::Oop`; the phase-6 withdrawal `Event::Inbound`); `Tape` is the manager with its states 1, 2 and 4, the delay, the speed ramp, the offer and the prompt; `tape::step` runs the buttons, the director, the manager and the gamebreaker task, in that order, after the simulation's step, and then the restart snap. While a replay plays the recorded world is on screen and the live one is kept (`Tape::live`, which the HUD reads); at the end the live world comes back with the newest frame's places, heights, headings and ball. Each frame now records every joint of each rig, and `tape::pose_bones` slerps them per bone over the animator's pose (PostUpdate, after `pose_skeletons`). Its own: the camera's update runs after the manager (in the original before it, its start and end handlers inside the manager's update); the combo's payout is the scoring side's chain before the basket's update (the original's single `game+0xd0` object keeps a stale `+0x150` without a match) and its start is when that chain began (the rewrite's ring keeps no ticks); an alley-oop finish is kind 3 whatever its move's flag (ported 2026-10-18: kind 4 with the move's 0x800); the buttons are the logical trick and pass buttons (`USRGAME*.CFG` per layout is not traced); the random draws are the director's own `NativeWideRandom`, not the shared sequence (ported 2026-10-18: the simulation draws the roll at the basket, `Event::ReplayRoll`; the combo branch reads `+0x150` and `FUN_00131d18` as the original does); the results wait for the won game's replay and then keep their 2.5 seconds; online matches have no replays. Tests: `tape.rs` (`a_replay_steps_the_last_frames_at_its_speed_like_fun_00328ab8`, `a_delayed_request_waits_and_grows_its_span_like_fun_00328fc0`, `the_subjects_moment_slows_the_replay_like_fun_00324588`, `recorded_joints_blend_per_bone_like_fun_003275d8`) and `game.rs` (`an_instant_replay_ending_on_a_restart_snaps_both_teams_like_fun_00142648`, `manual_replays_wait_for_the_trick_button_like_fun_00329c68`, `instant_replays_keep_out_of_gamebreakers_and_full_meters`).

### The row in a played match (2026-10-15, played-match evidence)

Everything above about the courtside camera's use in the demo is attract evidence (manager `+0x4c == 1`). This section is from matches a person plays: the user's own Pick Up saves (slots 1 and 2, front-end mode `0x4bbf88 == 2`, manager `+0x4c == 0` in every sample), run in a separate PCSX2 copy with PINE, the keyboard driving team 0 (standing still, passes, drives, jump shots, a dunk, trick presses, the tip-off press) and `tools/capture_camera_played.py`'s reads (no writes). The captures are `$EVIDENCE/street-camera-played-session1.jsonl` .. `session3.jsonl`, `-idle.jsonl` and `-opening1.jsonl` (slot 1: the intro cut, the toss, the tip, the first possessions).

**What is on screen.** The Street row almost all the time. The courtside (positioned) helper appears only for: a shot-clock violation (slot `+0x1a4`, on the offending side's carrier), the inbound walk after the other side scores (slot `+0x14c`, below), instant replays (start, baseline and beside-the-basket cuts, all inside `FUN_00303c80`'s and `FUN_003041f0`'s ranges) and the opening's midcourt cut. The restart's end (`+0xe4`) and a replay's end select the settled row. No celebration or gamebreaker camera came up (the computer's meter reached 42,500; ours did not fill).

**The row update** (`FUN_002f8370` with `FUN_002ff1c8`, `FUN_002fb690`, `FUN_002face8`, `FUN_002fe780`, `FUN_002fcd50`, `FUN_002fda08`), traced from the ELF and ported as `NativeCameraState::update`:
- Head: the row's height and lens each update; Street adds `DAT_00400b58`/`DAT_00400b5c` and then moves them by the depth pointer: low (row `+0x14`) toward 4 and 0.18 (-0.16, -0.003 an update), high (`+0x18`) toward 18 and 0.33 (+0.12, +0.003), the shot depth (`0x401080` = `DAT_00400b20`, -30) toward the lens 0.25, the base back by `(12 - 4)/50` from below, `(18 - 12)/50` from above (stopping at zero) and the lens by +0.003 while below 0.33 (no stop: the capture's offset sits at 0.0024 above).
- Depth pointer (rows 0 and 1, `FUN_002fb690`): low for an owner with actor bits 7, 13 or 15 (dunk, lay-up, alley-oop), a pass to a flier or a gamebreaker shot; the shot depth for a shot launched within 38.75 (`ball+0x154`, set by `FUN_00153790` at each catch and launch, the planar distance to the possession's rim) on every row but Court; high when the possessing side has a flier or a computer carrier while a person plays it (`FUN_00300b08`); `+0x12c` steps the depth by `(low - high) / 45` an update (Street's three depths are all -30, so its changes arrive at once).
- Windows (`FUN_002ff830`): the focus window from descriptor `0x4983e8` (edges 100/100/75/30, bands 80; low +150 on the sides and +100 at the top; Street below 9.33 +100 at the bottom); the reference window with the row's side threshold (185) on the attacking side, band 40, the row's top threshold and band 40. The focus's top tests read the focus raised by the owner's height × 1.2 (`FUN_00305428`, actor `+0x260` in feet). `FUN_002ff830` always projects with the yaw offset `+0x164` (it ignores its last argument, raw `0x2ff834`).
- Flags: yaw toward a focus past a side edge, else toward the rim past an edge, else away from the attacking side's band (`0x11`/`0x12`, `0x10` holding); pitch to keep the focus or the rim in the top band (`4`, `8`, `0x28`, `0x20`); `FUN_002fe780` adds a pitch flag and a squared-error step when the raised focus sits away from the band's middle (`DAT_004d8d00` = `5280/(60*3600)*3.46`, raw `0x300df8..0x300e28`).
- `FUN_002fcd50`: the steps, an off-screen focus turning by 1.25 (pitch, in double precision) and 1.125 (yaw), quarter corrections halved for a turn that keeps its direction while the depth is not moving; the shot depth scales the depth command by `DAT_004d8d08`.
- `FUN_002fda08`: a resting Street or Wide camera (`+0x15c`, set when commands and rates fall below 0.001) damps x and yaw by `(drift / 125)^4` until the focus has moved 125 pixels across the screen (`+0x160`); the yaw offset `+0x164` ramps by 0.03 toward `0.18 × (d - 17.75) / 12` for a held ball on the far half near the basket and recovers by 0.03 otherwise; the eye integrates within the row's x bound.
- The published view is the integrated pitch and yaw (`FUN_002fa558`), not a look at the focus. The rewrite used to aim at the focus.
- Settle (`FUN_002fabe8`): the pose aimed at the focus, one update holding the controller, then up to 501 updates with the world still until the commands and rates fall below 0.001. The camera comes to rest with the focus framed, not centred. The listener runs it inside the game update (reads land inside the loop: it is long), and the manager's next update follows.

**Comparison** (`tools/compare_camera_played.py`, then `view::played_match_row_follows_the_original`): each case seeds the production `NativeCameraState` with the original's row state and reads and runs one update (or a selection's settle and the next update). Over the four played captures: 49,262 updates (60 settles); depth pointer and `+0x12c` exact in all; 49,250 within 1e-4 on eye, angles, lens, rates and the yaw offset; 52 of the 60 settles within 1e-4. In the other 8 the first read with the row on screen already shows a later game update (phase 4 where the others read phase 6, or the update after a replay), so the focus the settle used was not read; they are kept in the count, not excluded. Carried from update to update on the capture's reads alone (no reseeding), 48,547 of 48,586 updates stay within 1e-3 of the original's pose. The 837 skipped pairs fail the capture's own integration identity (reads straddling a camera update). The opening (slot 1): the intro's staged camera, the midcourt cut at (-4.1, 8, -15) with scale 6.75, the 55-update blend 61 updates later, the row settled aimed at (0, 4, 0) (no possession: no rim to frame; pitch 0.2603), pitching up to 0.1623 by the handover, the toss 118 updates after the cut.

**Phase 1** zeroes the default focus's x and z (raw `0x300600..0x300618`): the opening frames the centre spot at the holder's height (`CameraSourceSample::for_focus`). The attract capture's "+0xc0 at the origin" (above) is this, not the stationary gate.

**Side.** `FUN_001404c0`'s side byte is the direction the possession attacks. The rewrite took +1 for team 0, the original's byte, in the simulation's frame, where team 0 attacked -x (`street_sim::restart::side`); it then used the simulation's side, so the Street eye leads toward the attacked rim as the original's does. The simulation's whole frame was the original's mirrored in x (the captures have team 0 attacking +x, on the screen's left); the camera's other native constants (the midcourt eyes, the no-possession rim at +36.75, the inbound eye's `z ∓ 3`) are kept as the original's. Since 2026-10-16 the simulation is in the original's frame (docs/TRACE.md "The court's frame"): `restart::side(0)` is +1, the home side attacks the left-hand basket as in the original, and the camera needed no change.

**The inbound walk** (slot `+0x14c`, `FUN_003025e8`, posted by "throw inbound" `FUN_0010b820` on its first update): ported as `CameraManager::watch_inbound`. With a person on the possessing side, the Street row, no blend on screen and the spot within the sidelines, the update after the inbounder has the ball puts a courtside eye at `(x ± 7.5, 12, z ∓ 3)` beside the strategy's first spot (`restart::spot_at` with the previous restart's order as the pushers), scale 6.75, on the receiver (the walk's `+0x1c` player: the capture's `+0xc0` is the second spot's player). The capture: eyes (-55, 12, 19), (-55.125, 12, -13) for spots (-47.5, 16), (-47.625, -16).

Not established or not ported: Behind's own update (dead code: the original never selects row 4); the source adapter's stand-ins (a held owner's physical vector as velocity × tick, idle for actor `+0xa4 & 0x7ffdc`, no stop-clip exemption, no `+0x158` vertical gate; replaced 2026-10-18, last section); the referee's focus margin (taken as 0; his `+0x260` reads as 0); `FUN_00140558`'s `vt+0x6c` term (live is phases 2..5); the original's rim state 6 (the rewrite's shot stays a shot until it is loose).

**The source law in play.** Fed the original's own inputs (owner `+0x30`, `+0x40`, `+0xa4`, channel 0's clip, ball `+0x30`/`+0x40`), the ported `FUN_003003f8` law reproduces the row's `+0xc0`/`+0xf0` in 25,423 of 26,255 updates of session 3 (96.8%); the rest are the `+0x158` vertical gate on a bouncing ball and straddled reads. Of moving actors in session 2, 72% have `+0x40` equal to their move and 23% zero (animation root travel), which is what the rewrite's velocity stand-in cannot reproduce.

**The tip** (slot 1, for the restart and tip cases): the person pressed shoot at update 167 with the ball at its apex (y 20.0); both jumpers left the floor about update 177 (the computer's rose higher, 4.59 against 3.74 at update 200); the tip at update 200 (ball 14.38, velocity (-0.147, 0.055, 0.056) per update) was a pass to the person's side (actor 2, bit `0x200`), phase 4, possession 0.

## Rust reconstruction and remaining limits

`crates/street-game/src/view.rs` applies all six native eye mappings, uses the active player/ball focus source described above, and converts each row's base camera `+0x98` value to Bevy's vertical FOV with the initialized 1.3 projection zoom and the saved Widescreen option. The common state-`0x12` planar tracking equations and the `+0xf0` focus-vector producer are traced above. The row helper's whole update for rows other than Behind is ported as `NativeCameraState::update` and checked against played matches (section above). Helper selection is described in the camera manager section above. Rust does not reproduce Behind's update or the per-tick adjustments of the authored and blend helpers beyond the blend's mix. Staged sequence cameras use their authored tracks through the staged helper as the original does (the section "The staged helper's lens, roll and following cameras").

Focused tests check all six table rows' pose words and screen-zone integer words, the normal offset/clamp/z formulas, Behind's ground-level pitch target and asymmetric x clamp, the owner/ball focus source, the focus-z floor, and the matrix-to-Bevy FOV conversion for each row at 4:3 and 16:9. A Rucker render was checked at `$EVIDENCE/street-recovery-camera.png`; it confirms the scene remains visible, not a 1:1 original-runtime match.

## Complete normal-row input adapter

A production controller must advance after each actual 60 Hz simulation step, before presentation interpolation, and retain its own native camera state. `Sim.world.tick` supplies the tick identity; the existing `FixedUpdate` chain already steps sim/network state before view event bookkeeping. Replay restore, rollback, camera-mode change and staged-camera exit require explicit state initialization/restoration, not a blend between a native result and the proxy. A presentation resource should expose the previous/current completed camera poses so rendering can interpolate without adding a second controller update. The selected row's native settling path (`FUN_002fa4f8` / `FUN_002fabe8`) must establish the initial pose, scalar and zeroed rates before enabling the controller.

The ordinary rows 2/3/5 share the fixed base depth-offset pointer and do not take the Street/Court actor-dependent offset selection branches. Their geometry, ordinary scalar, command recurrence, screen projection, zone comparisons and final normal flag mapping are recovered. They still need a complete source adapter and the geometric pitch-acceleration/state branches; translating the common planar equations alone would change behavior under real possession transitions.

| Native input / retained state | Existing source or required adapter | Status |
| --- | --- | --- |
| Unclamped focus `+0xc0` | Held player's `[position.x, height, position.z]`; otherwise full `Ball.position`. Save this raw vector before separately flooring `+0xd0.z` at -26.5. | Source selection established; retain raw/clamped histories separately. |
| Ownerless boolean `+0x14c` | `!matches!(Ball.state, Held(_))`; compare the boolean, not owner identity. | Established. |
| Source motion `object+0x40` for a free ball | `Ball.velocity * TICK_SECONDS`, sampled at the equivalent native tick boundary. `FUN_0014d1f8` adds full `+0x40` directly into `+0x30` before subtracting half a gravity step from position Y and one full step from motion Y. | Native per-update units established. |
| Source motion `actor+0x40` for a held owner | Separate physical displacement/velocity for that update, excluding animation root travel. `FUN_00156f48` adds full `+0x40` directly to `+0x30` after VM update and before later foot/root compensation: its units are also court units per update. `Player.velocity()` and a height derivative are potential sim-side quantities, but equality with the native physical source must be established before using them. | Native units established; no complete current adapter. |
| Stationary-source suppression | Native actor `+0xa4 & 0x7ffdc`, zero physical source vector, source-entity identity, and the exact animation exemption from `FUN_00300b98`. | Actor mask comes from native script/actor state; the existing sim `Action` enum is not a proven replacement. |
| Animation exemption | Channel-0 animation record IDs 46/47 (`RunToStandDribble_L`, mirror 0/1), 174/175 (`jogdribblestop02_r`), 176/177 (`jogdribblestop_r`). Record names/mirroring can identify the exemption once the authoritative selected channel is available. | Exact predicate established; these are stopping clips, not a general locomotion exemption. |
| Vertical suppression `+0x158/+0x1b4` | Retain previous physical source Y, detect the owner's absence plus near-floor negative-to-nonnegative crossing, and clear on the native shot/pass ownership transitions. Native ball floor is `ball+0x11c`. | Equations known; tick order and all transition resets need adapter integration. |
| Native special game state `game+0xf4 == 1` | `FUN_00141fd8` gives the ball to the `AIPhysReferee` at `game+0x170`, after resetting both teams. It is an opening/referee-ball state. Ordinary live player possession is outside this branch. | Meaning established; do not map all Rust `DeadBall` phases to it. |
| Side / reference focus | Native signed byte at `game+0x98+selector`, selector `game+0x110`; reference is `(side*36.75,12,0,1)`. Rust possession team maps to the initialized +1/-1 bytes. | Constructor mapping established; possession/event order still needs normal-match verification. |
| Row-zone thresholds | Descriptor `+0x48/+0x4c/+0x58/+0x5c/+0x68/+0x6c`, then row-specific side/top substitutions. Use native scalar, pitch/yaw and near .5 for framing projection; exclude renderer zoom 1.3. | Static equations recovered; exact pre-update projection inputs required. |
| Pitch acceleration / corrections | `FUN_002fe780` consumes ordinary focus plus margin (or ownerless ball XYZ), source Y, pixel thresholds and flags. It may replace field 7 and halve pitch-velocity correction. | Geometric branches traced; not all executed by current oracle. |
| Wide's extra framing | For ball state 3, replace the alternate reference zone with the signed hoop reference and apply `+0x15c/+0x160` screen-motion accumulation plus `+0x164` yaw offset/recovery. | Geometric recurrence traced; not yet implemented end-to-end. |

The fresh unmodified `street-native-full-rig-active.json` capture contains 417 row-active samples (mode 0) after sample 583, unlike the earlier attract-only capture. In 95 adjacent held-owner row pairs where `motion == new_c0-old_c0` and actor position equals camera focus, only 18 actor-position differences match physical `actor+0x40` within 5e-6. Numerous other pairs show changing focus with physical motion `[0,0,0]`. This proves that the camera's source vector cannot be replaced by complete root travel or assumed zero for all ordinary movement. Of those 95 pairs, 38 have physical motion exactly `[0,0,0]` despite changing focus. Across all 417 row-active, 49 positioned and 44 blend-helper samples, the recorded native basis matches `forward=(sin(yaw)*cos(pitch),-sin(pitch),cos(yaw)*cos(pitch))`, `right=(cos(yaw),0,-sin(yaw))`, and `up=(sin(yaw)*sin(pitch),cos(pitch),cos(yaw)*sin(pitch))` within 1.27e-7 maximum component error. The staged helper `0x498848` is deliberately excluded because its extra roll/transform inputs are not captured by this check. Running `tools/camera_oracle.py` on the complete capture reproduces these counts and errors; a saved report is `$EVIDENCE/street-camera-input-adapter-checks.json`. These captures are live and not atomic; the filtered comparisons establish distinct motion sources and normal projection geometry, not a bit-exact complete controller match.

The remaining integration should therefore supply one authoritative per-tick actor/ball snapshot containing raw focus, physical source motion, actor script mask, selected animation record/mirroring, possession/referee phase and shot/pass/floor transitions. A camera-state resource can then run the recovered normal-row chain and expose completed poses to the renderer. Creating a second disconnected Rust arithmetic kernel or switching opportunistically between native/proxy frames does not resolve these input/state-transfer obligations. Until the producer and state initialization are complete, the presentation proxy remains in use.

## Pause and saved selectors

`FUN_002bc3f8` constructs Pause choices from the u16 table at 0x48e210.
The camera row is `[0x3ec,0x580f,0x5810,0x5811,0x5812,0x5814,0xffff]`:
five choices, explicitly skipping Behind's text ID 0x5813.
`FUN_002bdc60` maps those text IDs to saved selector values 0..4 via
`FUN_001d74f8`; `FUN_003038b0` and the live update apply mode rows
`[0,1,2,3,5]`. Invalid saved selector values 5..7 default to mode 0.
The rewrite's six-choice Pause menu was an error based on locale text
availability. It now cycles five choices and imports invalid original
selectors with the native default. Row 4's Behind pose remains mapped
as internal machinery, not a selectable native Pause option.

Validation: original ISO bytes confirm the same five-choice table; regression
coverage checks forward/reverse cycling and all eight saved-selector values.
A rebuilt live Pause run reaches Wide after four right presses from Street
(`street-recovery-pause-camera.png` in the local temporary directory).
Final workspace gate: 385 disc-enabled tests, formatting, clippy and publish
checks pass. This confirms the mapping change, not native camera dynamics.


Parent review: a fresh 1,000-sample PINE capture independently reproduced
1,000 positioned lens targets within 2.99e-8 and 88 exact easing changes
among 97 observed changes (nine nonmatches retained, maximum 4.20e-4).
`tools/pine_compare.py` makes this check repeatable and preserves every row.
Unpaused mixed/skipped updates limit the comparison. The parent also found
and cleared an erroneous Ghidra CALL_RETURN flow override at 301d90. The
restored decompilation now exposes the complete 301d50 easing equation,
matching the original raw MIPS and the recorded six bit-exact steps.

### Authored root travel and physical source adapter

`FUN_0019a658` samples/blends the channel root, applies skeleton body scale and `0.32573292`, subtracts the previous channel-local root at `+0x90`, then rotates the delta by actor quaternion `+0x50`. It advances actor X/Z unless channel flag `0x1000` locks root travel or a non-looping blend is pending; `0x20000` suppresses pending-blend X/Z and clears after the blend completes. Flag `0x10000` applies old actor `+0xe4` minus new world root Y to actor Y, then saves the world root at `+0xe0`. These channel flags are distinct from actor `+0xa4`. `FUN_00156f48` subsequently adds the separate physical vector `+0x40`, applies gravity unless channel `0x40000`, and applies floor/friction rules. Camera source motion therefore requires the physical vector at the camera update boundary, not total actor position delta.

Script 69 opcode `0x91` preserves the high 16 channel flag bits while replacing the low 16; its final zero does not clear inherited Y compensation. Blend opcode `0x8e` clears root lock `0x1000`, but does not supply an authoritative initial root/Y-compensation seed. Opcode `0x97` zeroes the full actor physical vector. A complete runtime adapter must carry the channel flags, previous sampled root, root compensation, heading/quaternion, actor A4 and post-physics vector across clip changes and camera resets. A fixed clip asset or generic Trick action alone cannot initialize those values. The production follow proxy remains until these inputs and reset/state-transfer rules are supplied together.

The read-only restored portable savestate-9 check (`%TEMP%/street-camera-native-projection-restored9.json`) sampled active positioned helper `0x1950b80` and renderer context `0x8d6980` in one PINE batch. Scalar `0.06590481847524643` produced native horizontal degrees `10.043790817260742`, exactly equal to the float32 polynomial oracle. Observed M00 `8.75381851196289` and M11 `-11.671757698059082` differed from host-tangent oracle values by one float32 ulp each (`9.5367431640625e-7`). Aspect was 4/3 and zoom 1.3. This is a restored original state, without guest code patches or memory writes; a PINE batch is not atomic with guest execution.

### Self-toss transition appendix: quantized heading and held-ball offset

This appendix traces the original ELF without changing guest memory. `FUN_001978b0` (blend opcode 0x8e) first stores incoming animation `channel+0x54`, target frame `+0x60`, duration `+0x6c`, counter zero `+0x70`, blend flag `+0x48 |= 0x200000`, and reciprocal duration `+0x68`. It copies the outgoing pose into `+0xbc`, sets the output buffer relationship, loads the incoming pose/root into `+0xc0`, then checks `0x800000`: if set, clear it and call `FUN_00196698(channel,1)`. Only after heading matching does it call `FUN_00193300(channel,1)`, clear root lock `0x1000`, and run `FUN_0019a658(actor,channel,-1)`. Thus the offset capture uses the updated actor quaternion, while the old world ball and actor positions still provide its displacement.

`FUN_00196698` rotates the explicit seed `(0,1,0,0)` by the incoming root quaternion at `channel+0xc0` (conjugate on the right), then takes native polynomial atan2 of the result's X/Z. This seed is the model root's axis; do not substitute the actor's conventional forward vector. Quantization is `cvt.w.s(float32(angle*162.97466 + 0.5))`, add 1024 if the integer is nonpositive, then `(integer+512)&1023`. Conversion follows EE FCSR rounding, not unconditional integer truncation. `FUN_00154230(actor)` independently uses the evaluated world root-bone quaternion (model callback bone 0, transform quaternion at +0x20), the same seed/atan2/quantization, and returns the rendered root heading. Both atan coefficient tables `0x4b3170` and `0x4b3110` contain the same four bits as the camera polynomial.

Let `incomingHeading` be the first result (zero when matcher argument 2 is zero), `worldHeading` the `154230` result, `oldHeading=actor+0xa8`, and `secondaryHeading=actor+0xac`. Compute `newHeading=(worldHeading-incomingHeading)&1023`, then `delta=(oldHeading-newHeading)&1023`. Raw `19669c..1966d0` converts float bits `0x41ba0b3f` (23.255491256713867) into the tolerance integer: default round-to-nearest gives 23. Raw `196a00..196a18` returns when `delta<tolerance` or `delta>1024-tolerance`; consequently default correction includes delta 23 through 1001. On correction, write A8=newHeading and AC=(secondaryHeading-delta)&1023, then rebuild the full actor quaternion `(0, sinTable(newHeading/2), 0, cosTable(newHeading/2))`, with integer division before the 1024-entry angle lookup. Raw `196a88..196a8c` proves all four quaternion lanes are stored. With argument 2 nonzero, construct a yaw quaternion from `float32(float32(delta*360)*1/1024)` degrees and pre-multiply the outgoing root quaternion at `channel+0xbc`, storing all four components. This compensates the outgoing pose for the actor heading change; merely updating actor yaw loses blend continuity. Required state includes the evaluated outgoing world root, incoming root sample, outgoing quaternion buffer, A8/AC, exact lookup table, and the actual FCSR conversion behavior.

`FUN_00193300` writes a held-offset-valid flag at `channel+0xb0`. If `actor+0x70` has no held ball, clear B0 and return. If B0 was already set, capture again regardless of the compatibility check. If B0 is zero and argument 2 is nonzero, obtain incoming per-frame flags via `1927f0(database,channel+0xb4,channel+0x60)`. With `oldHand=channel+0x14 & 0xc` and `newHand=returnedFlags&0xc`, skip capture (leave B0 zero) when `(oldHand&newHand)!=0` and `(oldHand==newHand || newHand==4 || newHand==8)`. Otherwise set B0=1 and compute `offsetLocal=rotate(conjugate(actorQuat), ballPosition-actorPosition)`. Full LQ/SQ stores at `1933ac..1933b4` and `1933dc..1933e0` preserve XYZ, and `1933e8` explicitly writes offset W=1 at `channel+0xac`. Argument 2 zero forces capture when a ball exists. Offsets are in court units; this producer does not apply body scale or radius.

The consumer is `FUN_0014e060`, raw block `14f2c4..14f410`. It requires both `channel+0x48 & 0x200000` and `channel+0xb0 != 0`. It reads the complete stored offset at `14f2e4`, forces W=0 for rotation, reconstructs `startWorld=rotate(currentActorQuat,offsetLocal)+currentActorPosition`, and calculates `alpha=float32(counter70/duration6c)` using integer-to-float conversions before division. It writes `startWorld*(1-alpha)+generatedTarget*alpha`, then forces W=1. `generatedTarget` is the already evaluated held-ball attachment/hand-state result in this function, not a fixed clip endpoint. No alpha clamp appears in this block. With a duration of six, the producer's immediate `19a658(...,-1)` advances the counter to one, so the first consumed blend is ordinarily 1/6; subsequent counters reach six before completion. Completion is tested with `counter>duration`, promotes incoming to current, and clears B0. Do not defer the immediate update or clear B0 at counter==duration.

Static regression obligations are: tolerance boundaries delta 22/23/1001/1002; wrapped AC adjustment; odd heading uses integer-halved lookup; argument-zero capture; absent-ball clearing; every old/new hand mask pair 0/4/8/12; moving/rotating actor during captured-offset blending; counter sequence 1..6 followed by completion. Raw instruction checks above establish producer/consumer ordering and full-lane writes. Existing read-only full-rig captures demonstrate ordinary unblended attachments and native evaluated root transforms, but omit channel A0/B0, actor A8/AC, and FCSR; they do not validate a six-update script-69 transition. A runtime parity claim needs those fields plus evaluated outgoing/incoming root buffers and per-update generated target at the same observation boundary. No continuous controller or animation port is claimed by this appendix.

Optional `pine_probe.py --gait` now records captured offset A0 (full vec4), B0 enable, outgoing buffer BC and its full root quaternion, for all six actors and the selected holder channel. Actor A8/AC remain in the existing gait packet; no invented FCSR value is added. A bounded 1000-sample capture immediately after authorized emulator LoadState of original portable slot 9 contains 6000 actor gait packets and 739 evaluated rigs. The selected-holder scan observes 62 enabled-offset blends, including 47 duration-six packets covering counters 1 through 6, but no incoming self-toss clips 296/297. The retained reports are `%TEMP%/street-native-held-blend-restored9.json` and `street-native-held-blend-restored9-checks.json`. Eight alpha-one attachment checks yield four errors below 2.81e-6 and four nonmatches, maximum 3.25918 court units; every nonmatch is retained. Captured evaluated hand pose can differ from the temporary incoming pose used by the native ball target generator. Derived implied targets and calculated dynamic starts are explicit derived fields, not independent target observations or full blend parity. The tool stays read-only; emulator restoration was issued separately. Python compilation and a mocked full-four-lane offset/root-pointer check passed.


Parent all-six actor review separately counts80 enabled-offset blend packets,
65 of duration6, with every counter1..6 and every captured offset W equal1.
These include actors outside the selected-holder scan and must not be merged
with its62/47 counts. None establishes independent incoming-target parity.

## Observed viewport and retained projection failure (2026-10-13)

The validated live chain is root0x400058 -> context+0x10 manager -> manager+0x34
renderer wrapper (0x4dbc10) -> *wrapper projection object (0x8d6980).
Wrapper+0xc/+0x10 are widescreen flags; projection-object+0x10 is a framebuffer
pointer. Projection+0x114 records actual aspect, +0x190 zoom, +0x120/+0x124
viewport scales, and +0x128/+0x12c offsets. Active camera+0x170 stores wrapper.
Both new blend-helper observations have full viewport, aspect4/3, zoom1.3 and
zero wrapper flags, with matching field/identity bookends. Reads remain live.

The compiled production comparison at scalar0.06234413758 fails existing bounds:
angle native41180f22 versus Rust41180f25 (3ULP), matrix diagonals4/5ULP.
Captured scalar*4/3 equals native+0x80 tangent3daa3db6 exactly; wrapper+0x14
and projection+0x110 angles agree. Runtime polynomial constants match original
ELF. A separate follow-up produces0/1/0ULP and passes. Both are retained;
no residual exclusion or relaxed tolerance. Arithmetic versus render-consumption
scheduling cause remains unresolved. Next probe: native intermediate atan/sqrt
values at failed input or render-consumption timing.

Evidence: external street-camera-production-viewport-pair-summary.json retains
both complete reports and SHA256 bindings; tools/capture_camera_projection.py
reproduces the read-only field capture. No per-mode/controller conclusion.

## Camera arithmetic correction (2026-10-13)

Raw instructions in 0x300990 use the same operation order as Rust. PCSX2 2.8.2's
ordinary EE add/multiply operations chop; division and square root use nearest
rounding. Its addition path masks alignment bits. Local portable and per-game
settings have no EE rounding override; this game's index overrides VU clamping
only. Production now follows that policy for normal finite camera inputs without
changing process rounding. PCSX2 documents multiplication differences from real
PS2 hardware. Exceptional values, overflow and subnormal handling remain outside
this implementation's domain.

Three retained and 100 fresh observations were compared through the compiled
production conversion and Bevy projection: 102/103 pass, 102 angles are exact,
204/206 matrix diagonals meet the unchanged four-ULP bound, and 22 observations
match all three outputs bit-for-bit. The formerly failing scalar 3d7f5c91 now
produces exact outputs 41180f22/41140f83/c1456a04. All observations remain in the
report. Fresh sample 48 (batch index 51) still differs by 35534/45312/30208 ULPs.
Stable bookends do not establish when the renderer consumed those inputs.

The next probe is renderer consumption timing or native atan intermediates for
that outlier. The external street-camera-rounded-production-batch/summary.json
binds all rows to compiled-source FNV and consumer/input/source hashes. Raw
captures and probe/source snapshots are retained in the external directory
street-camera-rounding-fresh-20261013-164237. Full controller and real hardware
parity remain unproven.

## Camera generation and renderer publication (2026-10-13)

Raw ELF fills a missing continuation in Ghidra's 0x3046c0 decompilation.
The manager callback 0x304548 first updates the active camera through 0x2fab68,
then invokes render callback 0x3046c0. Positioned update writes scalar+0x98
at0x301e24; 0x2fa558 writes tangent+0x80 at0x2fa7ec. Render refreshes that
tangent, calls0x300990, then eventually passes the saved angle to0x323450.
Wrapper+0x14 is written at0x323470, projection+0x110 at0x378364, and M00
at0x3784c0. The integration count+0x124 is not a generation marker.

A dense read-only timeline retained10000 captures with zero errors and matching
bookends. Every9999 adjacent pair is classified:9321 unchanged,603 both lens
input/output changed,50 input changed while renderer tuple stayed old,25 input
stayed unchanged while renderer tuple changed. Twenty-one immediate triples
show new scalar/tangent alongside old angle/matrix, followed by new renderer
publication under identical recorded lens inputs. This supports a real temporal
window. It does not conclusively attribute the old fresh48 failure.

A shared production consumed-angle helper preserves existing arithmetic. Using
native projection+0x110 directly, actual Bevy matrices pass103/103 comparisons,
206/206 diagonals within the unchanged4ULP bound (maximum3), and23 observations
have both outputs bit-exact. No scalar reconstruction or residual filtering.
Scalar-to-render comparison still records102/103 matches; its generation pairing
is unresolved. The scorecard classifies that broader obligation as untested
rather than asserting a known arithmetic difference. Full camera parity is open.

Evidence: external street-camera-consumed-production-batch/summary.json,
street-camera-stage-scalar-production-batch/summary.json,
street-camera-render-timeline-audit-reviewer.json, and raw trace directory
street-camera-render-timing-20261013. The timeline retains capture-time source
snapshots. Next: a verified render-completion marker or read-only execution trace
for same-generation comparison, then the unimplemented native controller.

## Passive lens 1,000-sample comparison and torn read resolution (2026-10-14)

The 1,000-packet passive lens comparison (`street-camera-lens-triple-guarded-state9.json`) evaluated 1,000 unpaused native runtime observations against the compiled production projection. 998/1,000 consumed projection comparisons pass within the unchanged 4-ULP bound (maximum observed error 3 ULPs), and 990/1,000 scalar-to-projection end-to-end comparisons pass.

The two residual consumed projection outliers, sample 102 and sample 344 (which exhibited nominal diagonal errors of 84,995/113,327 ULPs and 10,111/13,482 ULPs), are confirmed single-frame torn reads between the camera angle update (`FUN_00300990` / `0x00323470` storing `wrapper+0x14`) and the renderer projection matrix store (`FUN_00378328` / `0x003784c0` storing `projection.m00/m11`):

- **Sample 101 -> 102 -> 103 progression:**
  - Sample 101: `scalar=0.099996`, `angle=15.188936°`, `m00=5.769366`, `m11=-7.692488`.
  - Sample 102: `scalar=0.099299`, `angle=15.084204°`, but `m00=5.769366`, `m11=-7.692488` (stale matrix from sample 101 held while angle updated).
  - Sample 103: `scalar=0.099299`, `angle=15.084204°`, and `m00=5.809895`, `m11=-7.746527` (matrix publication complete; matches Bevy projection within 1 ULP).

- **Sample 343 -> 344 -> 345 progression:**
  - Sample 343: `scalar=0.108018`, `angle=16.391510°`, `m00=5.340895`, `m11=-7.121193`.
  - Sample 344: `scalar=0.107921`, `angle=16.376925°`, but `m00=5.340895`, `m11=-7.121193` (stale matrix from sample 343 held while angle updated).
  - Sample 345: `scalar=0.107921`, `angle=16.376925°`, and `m00=5.345717`, `m11=-7.127623` (matrix publication complete; matches Bevy projection within 1 ULP).

When paired with their true corresponding matrices in the completed publication frame (samples 103 and 345), both observations pass within `< 1e-6` and `<= 1` ULP. This proves that both residual consumed differences were sampling artifacts of unpaused PINE reads capturing memory during the intra-frame pipeline execution, rather than arithmetic discrepancies in the Rust projection model.

## Native camera source motion adapter (`presentation.camera_source_state_adapter`)

The earlier adapter was not native. It zeroed motion for any owner in an action or standing still, and it used velocity for moving owners. It is replaced by `CameraSourceState::update`, a port of the law traced above (raw `0x3006d8..0x30094c`). Each helper keeps its own copy, as the native `+0xc0/+0xd0/+0xf0/+0x14c` fields are per helper. The last source entity is shared, as the global `0x401098` is.

- **Difference law.** With a count of one, motion is the raw focus minus the previous raw focus.
- **Substitution.** The source vector replaces that motion on the first update, on a change of the ownerless flag (the flag is compared, not the owner), or for a difference longer than `5280 / (60 * 3600) * 250` (about 6.11). Settling (`+0x13c`) skips the substitution.
- **Stationary gate.** The gate applies when all of the following hold: the same source entity as last update, an unchanged flag, an idle owner, and a zero source vector. Motion is then zero, and `+0xc0` is restored from `+0xd0`, so the focus holds still.
- **Clamp.** `+0xd0` copies `+0xc0` with z floored at -26.5, and the z motion is dropped at the floor.

The row helper consumes `+0xd0` and the motion. The positioned helper adds the owner's upper-body offset to `+0xc0`.

Remaining approximations (as of 2026-10-14; the 2026-10-18 section below replaces all three):
- A held owner's source vector is the rewrite's planar velocity times the tick. Its equality with the native physical vector `actor + 0x40` is not established: 38 of 95 captured held pairs had a zero physical vector while the root moved.
- "Idle" stands in for the native actor mask `+0xa4 & 0x7ffdc`.
- The stop-clip exemption `FUN_00300b98` (clips 46/47 and 174..177) and the vertical suppression `+0x158/+0x1b4` are not modelled.

Tests: `source_adapter_follows_fun_003003f8` and `native_focus_uses_owner_xyz_and_ball_xyz_when_free`.

## Native animation display state and root heading compensation (`presentation.animation_display_state`)

Decompilation of `FUN_001978b0` and `FUN_00196698` in `SLUS_206.51`:
- **Quantized 1024-Circle Heading Correction**:
  - `FUN_00196698` converts heading angles into a 1024-unit circle (`0x400`, `162.97466 = 1024 / (2*PI)`).
  - Heading delta `delta = (old_heading - new_heading) & 0x3ff`.
  - If `0x16 < delta < 0x3ea` (delta > 22 units / ~7.7 degrees), outgoing root rotation is pre-compensated by rotating around Y by the quantized heading angle: `Quat::from_rotation_y(delta * 2*PI / 1024.0)`.
- Implemented `quantized_heading_correction` in `crates/street-render/src/animation.rs`.
- `Animator` now tracks `last_heading` and applies outgoing root quaternion pre-compensation on bone 0 during crossfades.
- Verified by unit test `quantized_heading_correction_matches_native_1024_circle_and_deadzone`.


## The three pictures of the Widescreen row (2026-10-15)

The renderer's camera callback `FUN_00323450(angle, .., wrapper)` picks the
projection by the wrapper's two flags: `wrapper+0xc`, the saved Widescreen
option (`config+0x12c` bit 4, `FUN_001d7348`, off by the original's reset:
`docs/TRACE.md` "The original's default options"), and `wrapper+0x10`,
which nothing in the executable sets (zero from the constructor).

1. Neither flag: `FUN_00378328(angle, 4/3, ..)`, the 4:3 picture.
2. The first flag only: `FUN_00378328(angle, 16/9, ..)`: the same
   horizontal angle at aspect 16/9, so the vertical half-angle tangent is
   9/16 of the horizontal instead of 3/4. This is the PS2's anamorphic
   frame, the 4:3 output a 16:9 television stretches; a 4:3 display shows
   it squeezed.
3. Both flags: `angle' = (atan(tan(angle * 0.017453292 * 0.5) *
   1.3333334) * 2) * 57.295776` (`FUN_003af898`, `FUN_003af3f8`), then
   `FUN_00378328(angle', 16/9, ..)`: the horizontal half-angle tangent
   widened by 4/3, which puts the vertical back at the 4:3 branch's. The
   16:9 picture then holds the whole 4:3 picture plus a strip at each side.

With the viewport zoom 1.3 (`+0x190`) the shown picture's half-angle
tangents are `tan(angle/2) * 1.3` across and that over the aspect down.
For the Street row's lens 0.33 (`tan(angle/2) = 0.44`, angle 47.50):

| Picture | Widescreen row | `+0xc`, `+0x10` | horizontal | vertical |
| --- | --- | --- | --- | --- |
| 4:3 | Off | 0, 0 | 59.54 | 46.44 |
| anamorphic | On | 1, 0 | 59.54 (the same court width, stretched) | 35.67 |
| wide | Wide (the rewrite's) | 1, 1 | 74.66 | 46.44 |

At the row's near lens 0.18 the same three are 34.66/26.34, 34.66/19.91
and 45.17/26.34.

The rewrite: `view::lens_vertical_fov(scalar, Picture)` gives Bevy the
vertical angle (Standard the 4:3 branch, Anamorphic the 16/9 branch, Wide
the third through `wide_horizontal_degrees`, with Rust's tangent and
arctangent for the EE kernels) and the horizontal follows the viewport:
`options::letterbox` draws Off in a 4:3 viewport in the middle of a wider
window and On or Wide across the whole window, so a 16:9 window shows
exactly the three pictures above (a window of another aspect keeps the
vertical angle and shows more or less width). The default is Off, the
pillarboxed 4:3 picture, as the original's reset leaves it; the "looks
like a demo" picture was the rewrite's old default, Widescreen on, 35.67
degrees of height across the whole window. The Options page's Widescreen
row cycles Off, On, Wide; `--widescreen off|on|wide` picks one for a run
without saving it. Evidence: `$EVIDENCE/street-options-defaults/off.png`,
`on.png`, `wide.png` (`street-game --quick --screenshot <png> --at 8
--widescreen <mode>`, the 1280x720 window) and the tests
`mode_fov_matches_native_projection_for_4_3_and_16_9`,
`the_three_pictures_of_the_street_lens` and
`follow_camera_converts_the_published_lens`. Not captured: the original's
own anamorphic frame from PCSX2 (the Widescreen option on), which would
pair the On picture with its source.

## The row's selection, the source adapter's inputs and the shared roll (2026-10-18)

Audit item 7's camera part. Traced raw from the ELF; the played and attract
captures above are the evidence (PCSX2 was in use by another session, so no
new capture was taken).

**Behind is never selected.** `FUN_002f7bb0` has four callers (`0x303a68`,
`0x303ac4` in `FUN_003038b0`; `0x304c44`, `0x304ca0` in `FUN_00304a38`):
row 5 for the front end's mode 3 and for mode 4 with more than one user
(`FUN_001db248`), else the saved type through the switch (1, 2, 3 as
themselves, 4 as 5, anything else 0). The mode word `+0x1f0` is written
only there and by the constructor (0 at `0x2f7b68`). Row 4 is dead in the
original, which is why Pause's table skips 0x5813; its own update (the
`+0x20c` turn, its flags and x commands) is dead code and stays unported.
`view::native_camera_row` selects rows the same way (a saved 4 reads
Street; the League Challenge with two users forces Wide), test
`the_saved_camera_type_picks_the_originals_rows_and_never_behind`.

**The source adapter's inputs** (`CameraSourceSample::for_focus`):
- An actor's source vector is his physical vector as the simulation moves
  him (`World::actor_motion`): his flight on a shot's, dunk's, lay-up's,
  leap's or dive's script (the jump opcodes' `+0x40`), none on a trick's or
  a reaction's script (`97` zeroes it and the clip's root carries him),
  else his locomotion's speed along his course with no rise. The captured
  share of moving actors with a zero `+0x40` (23% in session 2) is the
  root-travel moves, which now read zero.
- The busy test is the actor's state bits inside `0x7ffdc` as the
  simulation's actions show them (`World::actor_state_bits`: 3, 4, 7, 8, 9,
  10, 12, 13, 15 and 16, the bits the computer players read; 2, 6, 11, 14,
  17 and 18 are not modelled), in place of "any action".
- `FUN_00300b98` (raw `0x300b98..0x300bf0`): channel 0's record `+0x50` in
  46, 47, 174, 175, 176 or 177 keeps the stationary gate off, so the focus
  follows a stopping clip's root travel; the simulation's gait row stands
  for the record (`CAMERA_STOP_CLIPS`).
- The vertical gate (raw `0x300520..0x30062c`): on the ownerless path a
  ball that leaves its owner as a shot (state 5, after `FUN_00300150`) or a
  pass (4, after `FUN_003000b0`) clears `+0x158` and `+0x1b4`; within 0.25
  of the ball's resting height (`ball+0x11c`) a source rise turning from
  below zero to zero or more sets `+0x158`, in the row's copy only (the
  function's second argument 0; the positioned helper passes 1); `+0x1b4`
  keeps the last rise; while `+0x158` is set and the ball has no owner the
  raw focus's height is 0. A ball knocked loose keeps the gate.

Tests: `source_adapter_follows_fun_003003f8`,
`a_loose_balls_bounce_flattens_the_rows_focus_until_a_shot_or_pass`,
`a_stopping_clip_keeps_the_stationary_gate_off_like_fun_00300b98`. The
played comparisons (`played_match_row_follows_the_original`,
`played_match_courtside_follows_the_original`) seed the controllers with
the original's own source state and are unchanged.

**The replay director's roll** (`FUN_003248d8` at `0x324bac`) is the
gameplay generator `FUN_003a16b8`, drawn inside the game's update at the
basket: the simulation now makes it (`Event::ReplayRoll`, after the
basket's `Event::Score`), and `tape::Director` reads it. See
`docs/TRACE.md` "The presentation's rest".

Still the rewrite's own: the courtside placements' and cuts' draws
(`FUN_00303c80`, `FUN_003041f0`: `FUN_003a16b8` in the Camera task, after the
update) and the coins (`FUN_003b42a0`, the libc generator the crowd's node
picks share) use the camera's own `NativeWideRandom` and LCG; the
positioned controller's projection and lead in plain float arithmetic
rather than the VU's operation order (7,255 of 7,848 played updates within
1e-6, 7,806 within 1e-4, unchanged); no played-match capture with a
gamebreaker or a celebration camera.

## The staged helper's lens, roll and following cameras (2026-10-21)

Measured in the pcsx2-runtime on the Street School's Level 2 rerun (GamLR2; `$EVIDENCE/street-legend-rules/rerun-camera`). The staged helper (manager `+0x24`, vtable `0x498848`) holds ten script camera nodes (`+0x1c0..`, `FUN_003318b0`), one per camera id; the active node's update (`FUN_00331d08`) writes the helper's eye (`+0x84..`), its look (the pose callback `0x3010f8`: yaw and pitch from the eye to the target), its lens `+0x98 = tan(fov / 2)` for a field of 1..180 degrees (1.0 for 0, the last lens otherwise) and its roll `+0x1b0 = (heading - 180)` degrees in radians. The renderer reads the lens through the projection object as for every helper (zoom 1.3: a field of 50 is 63.74 degrees across, 62.4 down) and turns the view by the roll's negative about its axis (`FUN_003046c0`). A record with byte `+0x23` set follows its script actor (`+0x20`, -1 the ball; `+0x25`: bone 0, bone 21 or the floor, at the actor's root x/z): the target's x is then a sideways offset (the yaw less `asin(x / 10)`, the reach `sqrt(h^2 + x^2)`) and its y a height over the point. The rewrite had read the field as a 4:3 horizontal angle (38.5 degrees down for 50), applied no roll and aimed following cameras at the court's origin. `view::staged_pose`, `StagedView`, `ActiveCamera::roll`; tests `the_staged_camera_has_the_originals_lens_roll_and_view` and `a_following_staged_camera_aims_at_its_actor`.

## The framing projection's arithmetic (2026-10-24)

`FUN_002ffac0` builds its view with three calls of `FUN_002f7700(out,
axis, angle)`: the roll `-[+0x1b0]` about the axis at `0x4d8c20` (onto the
matrix copied from `0x4d8ca0`), the pitch `-[+0x90]` about `0x4d8c00`, and
the yaw `-([+0x94] + [+0x164])` about `0x4d8c10` (the offset only when its
third argument is set). The axes and the base are runtime globals past
the file's end: z, x and y and the identity in save states 1 and 5. Each product is the VU0 chain `vmulax`, `vmadday`,
`vmaddaz`, `vmaddw` per column. Then a translation by the negated eye
`+0x84..+0x8c` (w set to 1.0), and the point's transform. The depth is
tested against `[FUN_0030fa50() + 8]`, and the divide is `vdiv Q, vf0w,
vf3x` then `vmulq`. The screen x and y come from `FUN_002ff818(lens)`, 640
and 480 halved by `-0.5`.

`FUN_002f7700` is an axis-angle (Rodrigues) matrix: `1 - c` times the
axis products, plus `c` on the diagonal and `±s` times the axis off it.
Its sine is its own: the quarter turn is `cvt.w.s(x * 2/pi ± 0.5)` with
no whole-turn step first, then `r = x - q * pi/2`. The odd polynomial is
`r * (1 + r2 * (-1/6 + r2 * (1/120 + r2 * (-1/5040 + r2 * 2.7557e-6))))`,
coefficients `0x3638ef1d`, `0xb9500d01`, `0x3c088889`, `0xbe2aaaab`. The
cosine is `sqrt(1 - s * s)`, swapped and negated by `q & 3`. That differs
from `FUN_002f8370`'s inline sine (`native_sin`), which has a cosine
polynomial.

Ported (`framing_projection`, `axis_rotation`, `rotation_sin_cos`,
`vu_transform`). The retained cases chose the rounding: VU0's products and
sums each chop with no guard bit (`vu_add`), its `vdiv` chops too, and the
EE's FPU keeps the rewrite's measured policy (`camera_add`, `camera_mul`,
division and square root to nearest). `FUN_002fd970` adds the focus and
its motion with `vadd` and projects both with the yaw offset's flag set.
`FUN_00301e38` scales a small subject as `target * ((scale - 2) * 0.25)`.
`FUN_00301fa8` scales the bands as `cvt.w.s(band * factor)`, factor
times `2 / scale`. Both `cvt.w.s` (the bands and the lead's shift of the
window) truncate on the EE, where the port rounded to even. That was most of the
yaw's misses: through `+0x134` a one-pixel window moved the framing. With
all of it `played_match_courtside_follows_the_original` lands every one of
the 7,848 updates within 1e-6 (worst 2.4e-7) and `+0x134` in all, up from
7,255 and 7,837, and now asserts both.

## The staged cameras' clocks (2026-10-24)

The script manager sets its camera nodes at a different frame in each of
its states. In play (state 3) its update (`FUN_00336628`) runs the frame's
records first, then the nodes at that frame (`+0x58`), then counts the
frame on: the camera after an update shows the frame before the manager's.
A record that changes the state (the freeze's or the replay's command)
ends the update before the nodes. The freeze (`FUN_00336340`) keeps its
command's duration at `+0x78` and sets the freeze's camera at the
command's frame. Each later update (`FUN_003367c8`) counts `+0x78` down
and sets it at `frame + duration - left`. The update that reaches 0
(`FUN_00336450`) turns camera 0 back on without setting it. GamLR2's
freeze shows camera 1 from frame 100 through 159. The replay
(`FUN_00336868`) sets the replay's camera at the integer frame `+0x5c`,
then adds `percent / 100` (`div.s`, then a chopped `add.s`) to the clock
`+0x60` and truncates it into `+0x5c`.

A key (`FUN_00336d18`) runs only while its previous record's frame is
below the frame and its own frame isn't. A step key sets its value on its
frame. The others go through `FUN_00336f48` at the float clock `+0x60` in
the replay, else at the frame. That gives `t = (time - previous) / delta`,
held at 1 above with no lower clamp, and `FUN_00337088` (linear) or
`FUN_00337098` (Hermite, tangents times `delta + 1`) in chopped FPU
arithmetic. A previous record that isn't a key gives the node's current
field (`vt+0xd0`) and a zero tangent.

Ported (`Timeline::camera` and `Key::interpolate` in street-assets, with
the EE's arithmetic in `street_assets::ee`; `SequencePlan::camera_at`;
`StagedReplay::shown`, the clock the cameras saw). The four GamLR2 probes
(`codex-street-recovery-20261022/native-level2-camera-03.jsonl` and
`street-legend-rules/rerun-camera/cam-run4..6.jsonl`: each node's fields
and the manager's words) agree bit for bit. The camera-03 probe has 1,017
on-screen samples: 843 exact, and the other 174 torn reads, each field
either this update's value or the next one's. The test is
`staged_level2_cameras_follow_the_original` (`STREET_L2_CAMPROBE`). The
other fifteen plans run the same code. Their cameras weren't probed.

## A Level 1 gamebreaker in the original (2026-10-24)

The Street School's Gamebreaker lesson (14) gives the person a full meter
and a Level 1. It was captured twice with `tools/capture_camera_played.py`'s
per-update sampler, driven from a copied PCSX2 profile
(`$EVIDENCE/street-level1-camera`; manager `+0x4c` 0, as in a played
match). Run 1's shot missed, run 2's scored.

The sequence is the one the rewrite ports. The press puts the blend on
screen (`+0x40` set) toward the courtside helper, narrow: x within ±23,
low, z −35 on the School's court. The blend takes 30 updates. The shot
leaves on that camera. Sixty updates after the press the first replay
films from 100 up, its x a new courtside draw. The second replay is on
another courtside eye. The task's end clears `+0x40` and blends back to the
row over 30 updates, the outgoing courtside helper still updated. In run 2
the basket turned the row to the basket (its focus descriptor `+0x1a0`
from 3 to 0, the point type), shook it, and set the celebration countdown
`+0x50` to 119. The School ends the lesson before it runs out.

Numbers: the courtside helper's 60 updates on screen land within 1e-6
(`played_match_courtside_follows_the_original`). The blend back's eye is
`t × from + (1 − t) × row` within 4e-6 on all 57 updates. The row, carried
from update to update on the captures' reads, stays within 1e-3 for all
675 updates (`played_match_row_follows_the_original`).
`tools/compare_camera_played.py` had raised the focus by the holder's
height even when the row's focus is a point. The original and the
rewrite's `row_inputs` don't (`FUN_00305428` reads an owner only for types
3 and 2), so a catch under the basket's focus failed seven carried
updates. The tool now reads `+0x1a0`. The four played sessions' cases are
unchanged (their rows never had a point focus).

Not the original's: the courtside eyes and the replays' coin. They come
from the shared generators (`FUN_003a16b8`, the libc LCG), which the
crowd, animation and speech also draw from, so the rewrite's own copies
can't pick the same eyes.

## A Level 2 gamebreaker's camera in the original (2026-10-25)

The School's Level 2 lesson (25), fired from (-18, 22), captured with the
same per-update sampler (`$EVIDENCE/street-level2-camera`). Manager `+0x40`
stays 0 from the press through the script. The row stays on screen until
the staged helper takes it, two updates after the press (the script's
start). It holds through the basket and the replay. The update the script
ends, `+0x50` is 120 and counts down from the next, and a blend runs 30
updates into the row on its default focus (the ball under the basket).

`FUN_003033a8` (slot `0x1f4`, the script's end) sets `+0x50` to
`DAT_004d908c` when its message's word `+4` is 1, unless the intro still
runs, then settles the row (`FUN_002fa4f8`). `FUN_00302c20` (the basket)
does what it does for any gamebreaker basket with `+0x40` off: turns the
camera on screen to the basket, shakes a row and marks a celebration
owed outside lessons. Only a Level 1 starts the countdown there.

The rewrite had two faults. It checked for a running script at the
press, but the script starts two updates later, so a Level 2 took the
Level 1 path: a blend to a courtside camera and `+0x40` set, never
cleared by the script's end. It also never started the countdown at the
script's end. Now `World::staged_start` counts as the script, and the
script's end of a gamebreaker sets the countdown
(`a_staged_level_two_keeps_the_row_until_its_script_and_owes_a_celebration`).
In a match the celebration's courtside camera finds the scorer two seconds
after the script ends. The School ends the lesson first.

## The picture's zoom (2026-10-25)

`FUN_003797e0` sets the projection object's zoom `+0x190` to 1.3, and the
rewrite drew every camera through it: a picture 1.3 times wider than the
original's. The same object's viewport scales (`+0x120` and `+0x124`)
undo it, so on screen a camera's lens `+0x98` is the tangent of half the
4:3 picture's height. Grabs show it. Under the staged camera of the
School's Level 2 alley-oop (script 3) the rim, and under the Street row
two players, land within a few pixels of the original's picture only
without the 1.3. With it, everything sits a quarter too close to the
centre. `SCREEN_ZOOM` (1.0) is now the zoom the picture is drawn with.
`CAMERA_PROJECTION_ZOOM` stays only for the tests that check the native
matrix (`mode_fov_matches_native_projection_for_4_3_and_16_9`,
`the_three_pictures_of_the_street_lens`).
