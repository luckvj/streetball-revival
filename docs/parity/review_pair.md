# Paired component measurement review

Reviewed 2026-10-13 against the unpatched USA executable and the existing local
captures. This review changes no parity manifest or production behavior.

## Native ball update boundary

`FUN_0014d1f8` was read through the existing Ghidra endpoint on port 8089 and
cross-checked against raw instructions in the original `SLUS_206.51` using
`tools/mipsx.py`. Original executable SHA-256:
`9f2cf237da7bd78dada337a2939f3878c05ddbe39588570b5328154cbe637614`.

- `0014d280..0014d28c` loads ball position/velocity, adds their VU0 vectors,
  and writes position. The velocity is distance per native update.
- `0014d29c..0014d2b4` subtracts half the captured gravity from position Y
  and the full gravity from velocity Y when `ball+0x188` is nonzero.
- Collision helpers run after this movement and can change the resulting
  state or velocity. `ball+0x19c` records a later gravity fallback branch;
  it is not present in these captures.
- `ball+0x190 != 0` bypasses progress-counter and timeout advancement at
  `0014d510`, while position integration and outward court clamps still run.
  This field is not present in these captures.
- `0014d61c..0014d62c` increments progress at `ball+0x60` toward duration
  at `ball+0x64`, before timeout handling. Progress is not captured.
- State 4 decrements timeout at `0014d790..0014d79c`. At timeout zero it
  switches to state 1. At timeout `DAT_004b84b0` (default 60/12 = 5), a
  target-player helper can halve velocity (`0014d7f0..0014d81c`).
- The end of the updater clamps outward motion at X = +/-42 and Z =
  +/-26.5. This is distinct from Rust's loose-ball bounce boundary.

Therefore a positive timeout decrement is an **inferred** update count,
conditional on uninterrupted flight and no uncaptured bypass/reset. It is
not a global simulation tick. The existing captures cannot prove all
interior branch continuity, even with equal endpoint signatures.

The production adapter should guard both endpoints using native state 4,
gravity enabled 1, holder 0, bounce 0, FPS 60, root stability, game/ball
identity and stable captured launch fields. Require the after timeout to
exceed 5 and the before timeout not to exceed duration + 5. Retain every
guard rejection and every eligible numerical failure. Annotate input-based
native boundary risk without removing those rows based on an output error.
The adapter must execute the actual `Ball::integrate`, converting incoming
velocity by x60 and outgoing velocity by /60. Its fixed Rust acceleration
must remain part of the comparison rather than being replaced with the
captured native gravity.

Independent input-domain recount (before production execution):

| Capture | Samples | Candidate adjacent pairs | Positive eligible gaps | Gap 1 / gap 2 / gap 0 |
|---|---:|---:|---:|---:|
| `street-native-full-rig-active.json` | 1000 | 125 | 124 | 96 / 28 / 1 |
| `street-native-held-blend-restored9.json` | 1000 | 99 | 99 | 64 / 35 / 0 |

All samples have native FPS 60. Median capture intervals are 20.033 ms and
22.375 ms, respectively. Adjacent capture indices cannot stand in for one
native update. The independent offline arithmetic check is only a recount
and alignment check; the retained compiled-production report supplies the
component result.

For a stronger subsequent capture, read `+0x190`, `+0x60`, `+0x19c`, the
catch-ready flag `+0x1a4`, the timeout trailing constant and the game/ball
identity before and after the same compact ball packet. A single PINE batch
remains non-atomic with running guest execution. Bracketed field stability
is a guard, not proof that the sample occurred at a completed update.

## Camera projection boundary

The row helper is manager `+0x28`; the rendered helper is the pointer at
`0x402020`. They cannot be interchanged. The full-rig capture contains 417
active row observations, samples 583..999, mode 0, descriptor state 0x12,
actual scalar `0.33000001311302185`, Street lens offset zero. It contains no
renderer matrix observation. The restored held-blend capture instead has
1000 active positioned-helper observations and no active row observation.

The single restored projection capture names active positioned helper
`0x1950b80` and context `0x8d6980`. Its actual scalar is
`0.06590481847524643`; this must not be substituted with a row seed or
relabeled as a base-row controller observation. A shared production scalar
conversion can nevertheless consume this native input as a bounded
projection-component check.

The native sequence is scalar x4/3, native atan polynomial, doubled angle
in degrees, renderer half-angle conversion, tangent, then captured zoom.
For captured aspect A and zoom Z, the full-viewport coefficient convention
is M00 = 1/(tan(horizontal-half-angle)*Z), M11 = -A*M00. Rust derives the
vertical FOV with tangent/A and should compare the actual Bevy projection
matrix, negating Bevy's Y coefficient to match the native convention.
Native viewport scales at context `+0x120/+0x124` and the alternate
widescreen branch are not captured; preserve that limitation. Basis, pose,
roll, depth, camera selection and scalar generation remain outside scope.

The adapter's declared acceptance is exact emitted angle and at most four
float32 ULPs for each matrix diagonal. It must retain native and produced
values/bits, absolute errors, ULP errors and the original input bytes,
including provenance and helper/context identity. The four-ULP criterion
is a numerical bound, not proof of the cause of any rounding difference.

## Retained-result gate

Count compared and rejected observations from retained rows, account for
all 999 adjacent pairs per flight capture, and check every declared bound.
Bind the report to the executed consumer and the compiled source content;
an on-disk source hash alone can describe a file edited after compilation.
Keep exact-bit agreement separate from tolerance-based agreement. Neither
shared free-flight integration nor one scalar projection establishes live
World/camera-controller or whole-game parity.

Final retained reports were independently recounted:

- `%TEMP%/street-production-ball-flight-pairs.json`: 223 compared, 223 within
  the predeclared component bounds, zero numerical failures; 1775 rejected
  adjacent pairs remain recorded. Each capture's 999 adjacent pairs is
  accounted for exactly once. Maximum position component error is
  `7.62939453125e-6`; maximum native velocity component error is
  `2.9802322387695312e-8`. Three rows match all six float32 bit values.
  All saved per-row errors, exact-bit flags, update histograms and rejection
  reason counts were recomputed and agree. One geometric native-boundary
  risk row is retained. Compiled-source and on-disk source hashes agree.
- `%TEMP%/street-camera-production-projection-comparison.json`: one input,
  one compared, zero rejected, one within the declared numerical bounds.
  Emitted angle is bit-exact; M00 and sign-mapped M11 each differ by one
  float32 ULP (`9.5367431640625e-7`). All six consumed input words agree
  with the original observation. Source, lockfile, adapter, bit packet,
  retained original and result hashes were checked; the compiled source
  FNV binding agrees. The report correctly records that the complete
  observation is not bit-exact.

No blocking finding remains for these **bounded component** conclusions.
The native timing and camera-input limitations above remain unresolved.

## Clock-qualified follow-up review

Independent Sol6.1 review recomputed all2997 adjacent-pair classifications,
275 matches and2722 exclusions, including52 fresh counter-qualified pairs.
It checked raw clock guards,989 stable/11 unstable packets, per-component
errors, exact-bit flags, cadence/reason counts and nine BLAKE3 bindings.
The reviewer identified native signed-counter comparison; consumer now rejects
progress/duration high-bit values and tests that input-domain guard.
No blocking finding remains for the bounded flight conclusion.

The camera follow-up retains a failed original/production sample. Angle3ULP
and Y coefficient5ULP exceed the original exact-angle/4ULP policy. A second
sample passes0/1/0ULP. Root retained both and changed the broader projection
obligation to mismatched. The cause remains unassigned; stable bookends do not
prove render-stage coherence. Old single-sample evidence remains historical.

Final camera review independently confirms angle1/2 exact, matrix3/4 within
4ULP, all consumed words and16 current report/source bindings. Both original
captures retain their capture-time probe hash; the probe was subsequently
edited to validate game serial before fixed-address reads. Capture-time probe
source is not retained, so those recorded hashes are not claimed to match
today's tool. Original bytes/hashes remain unchanged. This provenance limit
is explicit in the paired summary. The output failure remains open.

## Camera rounding correction audit

Independent Sol 6.1 review found no arithmetic bug for normal finite camera
intermediates. An exact rational reference checked 24,000 signed add/multiply
cases. The review confirmed the emulator rounding policy, raw native instruction
order, and absence of local EE rounding overrides. Real PS2 multiplication and
exceptional/subnormal behavior remain outside this conclusion.

The compiled batch audit recomputed all output errors from float32 words and
verified every input origin, consumed word, report hash, compiled source FNV,
current source SHA, consumer binary, lockfile and adapter binding. All 103
observations remain: 102 pass, 102 angles are exact, 204/206 matrix diagonals
meet the unchanged four-ULP bound, and 22 observations match all outputs exactly.
Only index 51 (fresh 48) fails, with ULP errors 35534/45312/30208. No exclusion
or wider tolerance was applied. The final external summary SHA256 is
79980ad0fdaaf2ed670a1413fb6c591ca4ebde6ae6fe93e3df353cd30a643e98;
compiled view source FNV is 58e10e9c687718f8. No blocking consistency finding.

## Consumed-stage and temporal audit

Independent Sol6.1 audit validates103/103 consumed-angle observations and all
206 coefficient residuals. Histogram:84 exact,95 at1ULP,23 at2ULP,4 at3ULP;
23 pairs exactly equal. Current source/compiled FNV, consumer, adapter, capture
and per-row report hashes agree. One historical source has float32-exact
serialized values;102 observations additionally retain raw native field words.

All10000 timeline captures and9999 pair classifications were reviewed.
Twenty-one immediate triples show changed lens inputs with previous render
values, followed by publication while recorded lens inputs remain identical.
No observation is omitted or residual-filtered. Historical scalar row51/fresh48
still fails; consumed row51 is0/0ULP. Same-generation scalar/render pairing is
unresolved. The independent audit supports classifying that broader obligation
as untested, while retaining its numerical difference as evidence.

Final external audit SHA256:63705944ddafc95bc7139ef2b15f998461848df160c3a9b3a4398d4c943c65ee.
Current view source SHA256:dafe42c31bdc3070d6846de43577c854f002364c3025bd045927e7603b481114;
compiled FNV:f0742600a64f147f. No blocking consistency finding.
