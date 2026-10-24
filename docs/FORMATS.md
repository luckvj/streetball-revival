# the original game (PS2, SLUS-20651) data formats

What the code in `crates/street-assets` relies on, and how each point was
established. Everything is read from the user's own disc image at runtime.

## Disc layout

| Path | Contents |
|---|---|
| `/SLUS_206.51` | Game executable (MIPS R5900, no symbols, no network code) |
| `/XF/<COURT>.VIV` | One court: EAGL models (`*.o`) and texture sheets (`*.ssh`) |
| `/XF/<COURT>C.VIV` | That court's crowd: `*.e3b` (not decoded yet) and `*.ssh` |
| `/XF/XF000001`, `XF000002` | BIG archives with empty entry names (3,418 and 1,102 entries): textures and models addressed by index, most likely players |
| `/XF/XF000003` | Shared: per-court `*w.bin` crowd placement, balls, `net.ssh`, `skybox.o`, effect sheets (`flare.ssh`, `smoke.ssh`, ...) |
| `/XF/XF000004` | Litter for `DRIFT` (`leaf_red`, `leaf_gr`, `trash`), and traffic the PS2 game never uses: cars, street paths, stop files |
| `/XF/XF000007` | The EAGL VU1 microprogram library (one ELF object, see below) |
| `/DATABASE/*.ADF` | Players, teams, courts, moves |
| `/ANIM/*.BIN` | Animation banks |
| `/CSCRIPTS/*.CFG` | Plain-text config scripts |
| `/MOVIES/*.NFX` | Video |
| `/MUSIC`, `/SFX`, `/SPEECH` | audio (`.mus`, `.asf`, `.ast`, `.abk`) |
| `/PAD1.000`, `/PAD2.000` | 1 GB padding files |

Court IDs: CENTER, FOREST, FOSTER, INDOOR, MOSSWD, NYCAGE, PHILLY, RUCKER,
RUCKRC, SEATTL, SOLHOL (`street-viewer --list`), plus GREENC, a small
floor-and-backdrop scene.

Model files in a court archive, by the letter after the court prefix:
`o` surroundings, `a` more scenery (alpha-blended), `f` floor, `l` floor lines
and decals, `h` light flares, `b`/`d` the two hoops, `bs`/`ds` their skeletons,
`c`/`c2` the hoops' projected shadows. Besides the court's own sheets the
game loads `flare.ssh` from `XF000003` (the name sits with the court file
names in the executable).

## BIG archives (`.VIV`, `XF00000n`)

`BIGF`, u32 LE total size, u32 BE entry count, u32 BE header size, then per
entry u32 BE offset, u32 BE size and a NUL-terminated name.

## SSH texture sheets

`SHPS`, u32 size, u32 image count, 4-byte tag (`G335`), then a directory of
4-character names and u32 offsets. Each image is a chain of blocks with a
16-byte pixel header: `u8 type | u24 next`, u16 width, u16 height, then centre and
position fields. Sprite metadata type `0x7c` instead has an eight-byte header:
`u8 type | u24 next`, u32 corner count, then signed i32 (x,y) pairs. Four
pairs make one 32-byte frame. The reader retains corner order for flips;
`FUN_0039a900` returns block+8 and `FUN_00210048` reads these frames.

| Type | Meaning |
|---|---|
| `0x02` | 8-bit indexed pixels |
| `0x01` | 4-bit indexed pixels |
| `0x04` | 24-bit RGB pixels (`/FE/XAFEBG.BIN`'s strips; every pixel opaque) |
| `0x05` | 32-bit RGBA pixels |
| `0x21` | 32-bit RGBA palette (width = entry count) |
| `0x69`, `0x70` | Metadata, full name |

- **8-bit palettes are in CSM1 order.** In each group of 32 entries, 8..15 and
  16..23 are swapped. Verified visually: the Rucker Park banner (`ban1`) is
  scrambled without the swap and reads "RUCKER PARK NYC" with it.
- **A palette's width is the number of colours used, not the number stored.**
  Entries are stored in CSM1 order up to the next block. The asphalt (`ashf`)
  says 178, but CSM1 moves indices 168..175 to 176..183, and the block holds
  exactly 184. Reading only `width` entries left magenta specks. Checked over
  all 126 8-bit images in Rucker: the block always covers every swizzled index.
- **Alpha is PS2-scaled**, 0x80 = opaque; doubled to 0..255.

## EAGL models (`*.o`)

Each model is a MIPS ELF32 relocatable object (`EAGL toollib 4.06.01,
June 2002`). Only `.data`, `.symtab`, `.strtab` and `.rel.data` matter.
Relocations are `R_MIPS_32` and turn words in `.data` into pointers.

Symbols:

- `__Model:::<name>`: model header. Word 23 holds a `vec3` of half-extents that
  the skinned format uses to dequantise positions (see below).
- `__BBOX:::<name>`: min xyz, max xyz.
- `__EAGL::TAR:::tar_<tex>_<serial>`: texture slot; `<tex>` is the image name in
  the court's sheets.
- External: `ClipTextureCompress0__EAGLMicroCode` (static geometry),
  `Texture_Skin0__EAGLMicroCode` / `Street2_ScrollTexture_Skin0__EAGLMicroCode`
  (skinned), `EAGLAnimationBuffer`, view matrices.

### Model header and draw groups

`__Model` words used so far:

| Word | Contents |
|---|---|
| 23..25 | half-extents for skinned position quantisation |
| 39 | draw group count |
| 40 | pointer to the list of group name pointers |
| 51 | pointer to the draw list |

The draw list is `(0xa0000000, 0, total words)`, then for each group in name
order `(0xa0000001, 0, 2n)` and `n` entries `(0xa000ffff, pointer)`. The
pointer leads to a pointer to a render-method record whose first word points
at the render packet; the chain's CALL tag is the 16 bytes before that packet.
Every mesh on the disc maps to a group this way.

Blending is the same for every court mesh. Each group's render-method record
starts with a pointer to a VIF state block (`STCYCL`, unpacks, a `REF` to the
texture) that points at one shared GIF packet (offset 0x160 in `ruckera.o`):
five A+D writes, `TEST_1` = 0x50003 (no alpha test, depth GEQUAL),
`ALPHA_1` = 0x44 ((Cs - Cd) * As + Cd, ordinary alpha blending), and
`FRAME_1`, `ZBUF_1`, `FBA_1` filled in at run time. Glows, light beams and
`alpha_*` groups are ordinary blended textures; nothing is additive.

Group names carry meaning: `_cbot`, `_cleft`/`_clef`, `_crit`, `_cfrt` mark
scenery on one side of the court (the game presumably hides it when the camera
is on that side), `alpha_*`, `*Glow*` and `*VolLight*` are translucent (by their textures' alpha), and a
`~<microprogram>` suffix names the microprogram (`breflection_~Street2_ScrollTexture_Skin`).
Every outdoor court's `o` model has groups `opaque_riml` and `opaque_rimr`: a
copy of the hoop's rim lying on the court near each basket (same vertices and
colours as the hoop's rim). They are not seen in normal play, so the viewer
hides them; presumably they appear when a rim comes off.

### Finding geometry and textures

- **Every vertex chain is drawn through a DMA CALL tag** (`id 5`) whose address
  word is relocated to the chain start. Scanning `.data` at 16-byte alignment
  for CALL tags with a relocated address finds every chain (341 in the main
  Rucker model).
- **A texture change is `0x0000000e` followed by a relocated pointer to a TAR
  slot.** It sits in the render method after the CALL it applies to, and only
  appears when the texture changes. Chains without one keep the previous
  texture; for example 14 consecutive asphalt chunks share one `ashf` reference.
- Structure per mesh in file order: vertex chain, GS setup packet, CALL packet,
  render method (`+0x80` points back at the CALL), mesh header.

Known data quirk: the INDOOR court has a `tar_botl` slot, but no texture sheet
on the disc contains an image named `botl` (every `.ssh` and every BIG archive
in `/XF` was searched). The viewer draws that mesh untextured.

### Microprograms

`/XF/XF000007` is one ELF object holding every EAGL VU1 microprogram by
name (`Texture_Skin0__EAGLMicroCode`, ...). A record `<name>0__EAGLMicroCode`
gives the instruction count and the code address, so each program can be
disassembled (VU upper/lower pairs, 8 bytes each). Every layout below was read
from these programs, not inferred from data alone.

Common to all of them:
- Each batch writes a GS triangle strip with three quadwords per output vertex
  (ST, RGBAQ, XYZF2; GIF tag `NREG = 3`, `REGS = 0x412`).
- **Strip marks:** the three V4-8 quadwords just before the positions hold 12
  bytes. For each byte `m` the program sets the no-draw (ADC) bit on output
  vertices `m / 3` and `m / 3 + 1`, which starts a new strip there. Unused
  marks are 0, which only touches vertices 0 and 1 (they never draw anyway).
  Confirmed on the Rucker floor cracks: marks `[30, 60]` split 30 vertices into
  three 10-vertex strips.
- Strips alternate winding.
- Vertex colours are V4-8, 0x80 = 1.0. Units are feet-like: the floor lines
  span about ±47 × ±32.

### Static geometry (`ClipTextureCompress`, `Export_Texture`)

| VU address | Unpack | Contents |
|---|---|---|
| `0x000` | V4-32 | vertex count in x |
| `0x014`..`0x016` | V4-8 | strip marks |
| `0x017` | V3-32 (masked, w = 1.0 from STROW) | positions |
| `0x05f` / `0x067` | V2-32 | UVs |
| `0x0a7` / `0x0b7` | V4-8 unsigned | vertex colours |

The second addresses are `Export_Texture` (used by the `h` flare models). The
decoder finds the arrays by format rather than address.

### Skinned geometry (`Texture_Skin`, `Street2_ScrollTexture_Skin`)

Used by the hoops (`b`, `d`). With positions at VU address P (0x0c; 0x0d for
the scrolling-texture variant, which keeps a UV scroll vector at 0x09):

| VU address | Unpack | Contents |
|---|---|---|
| `0x000` | V4-32 | (output vertex count, input vertex count, bones, blend matrices) |
| `0x004`.. | V4-32 | one row per blend matrix: three weights whose low mantissa byte is a bone index × 4 |
| P-3..P-1 | V4-8 | strip marks |
| P | V4-16 unsigned, offset mode (STROW = 1.0) | position xyz, blend-matrix selector in w |
| P+60 | V4-16 signed | UV xy (4.12 fixed point), output slots z and w |
| P+120 | V4-8 unsigned | vertex colours |

- **Positions:** `(raw - 32768) / 32768 * half_extent[axis]`, with
  `half_extent` at word 23 of `__Model`. Decoded bounds equal `__BBOX` for
  every model on the disc.
- **Output slots:** the program writes each input vertex to output quadword
  `z` and again to `w` (equal values write it once), so a strip can reuse a
  vertex. Every output slot `0 .. header.x` is written; the strip marks
  refer to output vertices.
- **w** selects a blended bone matrix (0/7/14/21 quadword offsets); 0xffff
  keeps the current one. In the rest pose the matrices are identity, so
  positions are in model space.
- STROW is set in a model's first chain only; the VIF keeps it for the rest.
- The `bs`/`ds` files are skeletons (`__Skeleton:::Root`, `__Bone:::Root.<x>bone`):
  per bone, scale and parent index, a rotation quaternion, a translation and a
  4×4 inverse bind matrix. The hoops have a five-bone chain (pole to rim).
- `c`/`c2` use `ArbitraryShadow_Skin` (projected shadows drawn in
  `gShadowColour`): positions as above, a second V4-16 array at P+72, no
  colours. Not decoded yet.

### Hoop placement

Hardcoded in the executable. The scene setup around `0x13c740` creates two
baskets at (-41.012, 9.3691, -0.0026) with rotation quaternion (0, 0, 0, 1)
and at (41.023, 9.3691, -0.0026) with (0, 1, 0, 0). The basket constructor
(`0x148eb0`) passes them to the hoop constructor (`0x316ee0`), which loads
`<court>d.o` when x <= 0 and `<court>b.o` otherwise (plus `c`/`c2` and the
`s` skeleton), and uses the position as the model's origin. In the rendered
scene the backboard sits about 3.7 ft inside the baseline and the rim over
the key.

## Players

Players are assembled from parts in two archives whose entries have no names:

- `/XF/XF000001` (3,418 entries): 36 body models `body<b>_outfit<o>` (builds
  0-5 by outfits 1-6; outfit 6 is the team uniform with a `jers` slot, builds
  4-5 have no beard or glasses), each followed by 43 morph targets
  (`__Morph:::morph.modelname`); 1,356 head-and-hair models
  `malehair<h>_head<n>_body<b>` / `femalehair...` made to fit one build; and
  886 single-image sheets (blank team jerseys, shirts, hair).
- `/XF/XF000002` (1,102 entries): five per player for 220 players (three
  morph targets, then two texture sheets with `jers`, `head`, `hand`, `body`,
  `rarm`, `larm`, `rleg`, `lleg`, `shoe`, `eyes`, `mout`). Player 0 is Shareef
  Abdur-Rahim (Hawks #3), the first name in `XDBPLYR.ADF`. Entry 941 is the
  player skeleton (45 bones: torso, limbs, fingers and face bones such as
  `upperlipR` and `browR`).
- `/ANIM/*.BIN` are ELF objects with one `__AnimationBank` each:
  `body_ingame` (2.2 MB), `body_gamebreaker`, `body_intro`, `body_outtro`,
  `body_fe`, `face_ingame`. See [Animations](#animations).

The `body<b>_outfit<o>` kit in `XF000001` is only used for created players;
league players, streetballers and legends wear a `Small/Medium/Large` body from
`XF000002` with a per-player morph target (see
[Where a player's model is](#where-a-players-model-is)).

Texture binding: 0x318d20 binds an image to a model's TAR slot by name.
For league players every slot of the body has an image of the same name in the
player's own sheet (`jers`, `head`, `hand`, `body`, `rarm`, `larm`, `rleg`,
`lleg`, `shoe`, `eyes`, `mout`, and `dred` for the three players who have
it), except `shor` (an image of the team's shorts sheet) and `hdbd` (the
headband sheet). Streetballers, legends and generic ballers likewise find
every slot in their own sheet. For created players the game binds the
jersey image to the kit body's `jers`, `shir` and `tank` slots (around
0x31f6ac) and `berd` and `shoe` to their own images.

league body draw groups: `body`, `HEAD`, `MOUTH`, six hand poses per hand
(`ALfist`, `ALfthru`, `ALpoint`, `ALrelax`, `ALrhang`, `ALstress` and the `AR`
set), and the optional `HEADBAND` and `zdreds`. The loader (0x3165f8) looks
up `zdreds`, reads field 39 and leaves the group hidden when it is 0;
otherwise it binds the `dred` image. Kit body groups: `Head2`, `MadCoach<o>`,
`shoes`, the hand poses, and optional `zzBeard`, `zzGlasses`; kit heads have
one group (`zyHair`). Unique models have one group named after the player
(`Stretch`, `DJ`, `Male1`) plus the hand poses.

### Morph targets

`__Morph:::morph.modelname` objects hold per-vertex offsets for a skinned
model. The symbol points at a header: u32 target count (always 1), a
pointer to the target names (`foo`), a pointer to their weights, u32 0, and
the attribute count (2). Each attribute follows: a pointer to its name
(`Coordinates` or `UV`), u32 target count, u32 0, u32 chunk count, and the
chunks. A chunk is a u32 `0xe0000000 | size` (coordinates) or
`0xa0000000 | size` (UVs), the vertex count, the target index, and the batch
number in the model, then one i16 per component per input vertex, padded to
four bytes.

The batch number counts every microprogram run in the model, with the
render packets in file order (the order `eagl.rs` decodes them). Each chunk
covers every input vertex of its batch: on `Small1Body` the 27 coordinate
chunks of player 0 are batches 150..173 (`HEAD`) and 179..181 (`MOUTH`), and
their vertex counts equal the batches' (16, 36, 33, ...). Offsets are in the
batch's stored units: positions in quantised steps
(`offset / 32768 * half_extent`), UVs in 4.12 fixed point. league players apply
their target with weight 1 (`tests/player_disc.rs` applies all 435 to their
bodies). Most targets move only the head; the largest moves are Ben
Wallace's head (0.27 units) and Steve Nash's `zdreds` hair (1.05 units),
which the game never shows since his field 39 is 0.

Scale: the game sizes each player by `model height * 1.2 / 78` (database
field 10), so the bodies are built for a 6'6" player at 1.2.

Geometry (`Texture_ToonShade_Skin`, `Texture_ToonShade2_Skin`, the same
skinned scheme as the hoops with a 36-quadword stride):

| VU address | Unpack | Contents |
|---|---|---|
| `0x000` | V4-32 | (output vertices, input vertices, bones, blend matrices) |
| `0x001`..`0x003` | V4-8 | strip marks |
| `0x004` (FLG off, absolute) | V4-32 | blend rows (weights with bone slot in the low mantissa byte) |
| `0x004`..`0x00c` (FLG on) | V4-32 | lighting: light directions, material colour (127.0 × 4), ... |
| `0x00d` | V4-16 unsigned, offset mode | position xyz, blend selector in w |
| `0x031` | V4-16 signed | UV (4.12) and output slots |
| `0x055` | V4-32 | normal xyz (float), bone selector in w |
| `0x079`..`0x07b` | V4-8 | toon lighting parameters |

The program draws two passes per batch: the textured base and an
environment/specular overlay whose UVs come from the normals. Colour comes
from toon lighting of a per-batch material colour, not from vertex colours.

Bodies CALL shared sub-chains (normals and the kick) after unpacking their
own positions, so only CALL tags that a render method uses start a draw.
Each render method also lists the skeleton bones it needs:
`0x0e000000, 0x090000nn, EAGLAnimationBuffer, <pointer>, count, <pointer to
count u32 bone indices>` (for example 7 bones `[20, 21, 22, 28, 30, 33, 35]`).
The bones are uploaded to VU memory in that order (7 quadwords each: a
position matrix and a normal matrix, from VU 34); blend rows refer to them by
slot. The numbers are positions in the skeleton's bone list (checked: hand
vertices follow `LHand`, sleeves the upper and lower arms, socks the lower
legs and feet, face vertices their own face bones).

Skinning state persists in VU memory: the blend rows at absolute address 4
and the uploaded bones carry over to later chains until a render method
uploads new ones, so a decoder must keep both across chains. Per vertex,
position w selects the blend row (`w / 7`; 0xffff keeps the current one);
each row holds up to three (bone slot, weight) pairs with the slot in the
weight's low mantissa byte (`byte / 4`).

Skeleton files: `__Skeleton:::Root` points at `(hash, 0, count, 0)` followed
by 112 bytes per bone in parents-first order: scale xyz and the parent's
list position (-1 for the root), the local rotation quaternion (x, y, z, w),
the local translation and an id, then the 4x4 inverse bind matrix (rows;
translation in the last row). Composing the local transforms reproduces the
inverse bind matrices to 1e-6. `__Bone:::Root.<name>` symbols point at
records whose first word is the bone's list position.

Rest pose: decoded bounds equal `__BBOX` (body: T-pose about 6.7 ft tall with
the origin at the hips; feet at y = -4.05).

## Animations

`/ANIM/*.BIN` are ELF objects with one `__AnimationBank:::<name>` symbol
(read from the decoders in `SLUS_206.51`). The symbol points at
`(0x18, count, 0, <records>, <names>)`: `count` record pointers and as many
name pointers. Each animation is three records: `<anim>_X` (state and events,
`FnRawStateChan`), `<anim>_r` (rotations) and `<anim>_t` (root translation).

A record is an `FnCompoundChannel` (u16 type 15). Its u16 at +8 is the
number of channels (1 or 2) and the word at +12 points at the first; a second
channel follows the first back to back, at the next 2-aligned offset that
holds a delta-channel header (most at the next 4-byte boundary). Channel
objects start with a u16 type that the factory (jump table at 0x387180) maps
to a class; every delta channel on the disc has flags `0xc016`. Animations
play at 30 frames per second, and every channel on the disc has no key-time
table (the word at +8 is 0), so frames are evenly spaced.

| Type | Class | Evaluate / setup | Used for |
|---|---|---|---|
| 18 | `FnDeltaQFast` | 0x38d188 / 0x38e438 | rotations of most bones |
| 19 | `FnDeltaSingleQ` | 0x38aa80 | hinge bones such as the knees |
| 20 | `FnDeltaF3` | 0x38ef48 / 0x3906e8 | root translation |
| 21 | `FnDeltaF1` | 0x390b30 / 0x391d68 | root translation, one axis |
| 16 | `FnRawStateChan` | | the `_X` records |

All delta channels store blocks of `2^shift` frames: one key frame, then
`2^shift - 1` delta frames that add to the running value, each block padded
to an even length.

**`FnDeltaQFast`**: header u16 type, u16 flags, u16 frames, u8 channels, u8,
u32 key-time table, u32 pointer to one bone (skeleton list position) per
channel, u8 block shift, u8. Then per channel 8 u16: delta minimum xyzw
(`u / 32768 - 1`) and delta range xyzw (`u / 32768`). Key: 6 bytes per
channel, three u16 whose top 12 bits are x, y and z and whose low nibbles
together make a 12-bit w (bytes 0, 2, 4, high to low); each is
`v / 2047 - 1`. Delta: 3 bytes per channel, the top 6 bits of each byte are
x, y, z and the three low 2-bit fields together w; each adds
`min + range * v / 63`. The quaternion is normalised after every frame.

**`FnDeltaSingleQ`**: one rotating axis per bone. Header u16 type, u16 flags,
u16 frames, u8 channels, u8 block shift, u32 key-time table, u32 bone map.
Then per channel 7 u16: two fixed angles (`u * 2pi / 65536 - pi`), delta
minimums for the axis component and w (`u / 32768 - 1`), delta ranges
(`u / 32768`) and the axis (0 x, 1 y, 2 z). Key: 2 bytes per channel (axis
component and w, `b * 2 / 255 - 1`). Delta: one byte per channel, high nibble
for the axis component and low nibble for w, each adding
`min + range * n / 15`. The normalised (axis, w) quaternion sits between a
fixed pre- and post-rotation built from the two half angles; `anim.rs`
transcribes the game's products term for term (0x38b8e8..0x38bb20).

**`FnDeltaF3` / `FnDeltaF1`** (`dims` 3 or 1): header u16 type, u16 flags,
u32 pointer to one u16 float slot per channel, u32 key-time table, u16
frames, u16 channels, u8 block shift, u8, u16. Then per channel `dims` f32
key minimums, `dims` f32 key ranges, `dims` u16 delta minimums and `dims` u16
delta ranges. Key: `dims` u16 per channel, `min + u * range / 65535`. Delta:
`dims` bytes per channel, each adding `range * (2 dmin / 65535 - 1)` plus
`b * 2 range drange / 65535 / 255`. Every `_t` track on the disc writes float
slots 8, 9 and 10: the root's x, y and z.

Units and facing: translations are about three times the skeleton's units
(standing animations hold the root at 12.1 to 12.3 where the skeleton's hips
are 4.05 above the feet); no constant for this was found in the code, so the
decoder uses 1/3 as a fit. Animations face the opposite way from the
skeleton's bind pose (half a turn about y).

Coverage: all 646 rotation tracks decode. The last six carry channel types
beyond the first five: the four `lowcomp` tracks (a dive reaction and three
dive passes) evaluate as `FnDeltaQuatChan` (type 11, setup `FUN_00395020`,
vtable 0x4af440), and the two `nocomp` standing-dribble tracks as the raw
pose channel (type 0, `FUN_0038eca0`, vtable 0x4ae758). Both are handled
since 2026-10-12 (`anim.rs`).

## Locale text (`/LOCALE/*.EUS`)

`XLOC_GM.EUS` (in-game text) and `XLOC_FE.EUS` (front end) are small
archives: `c0 fb`, a big-endian u16 header length (0x1f4) and u16 file
count (24), then per file a big-endian u24 offset, u24 size and a
NUL-terminated name (`common.txt`, `pause.txt`, `overlays.txt`,
`school.txt`, ...). Each file is a `LOCH` chunk (0x14 bytes) followed by a
`LOCL` chunk whose strings are UTF-16LE; commas are written as the literal
text `\x2c`.

## Rules

What the game's text says about the rules, which `street-sim` follows:

- Games are three on three, to a set score ("Play To"), and must be won by
  two (`win by two`, `XLOC_GM`). The default of 21 and full-court play are
  from reviews of the game; the disc agrees with full court: both baskets
  are placed (0x13c740), the scenery has camera-left and camera-right
  groups, and the executable has an inbound throw (`throw inbound`,
  `inbound pos` near 0x338a20).
- "A Short Range Shot is any shot taken within the Long Range Arc", and the
  "League Scoring" mode plays "with 2 point shots and 3 point shots", so normal
  scoring is 1 and 2. "Old School Scoring" plays "with no long range
  shots". Other modes: Pick Up, Trick Point Challenge, Dunks Only, No
  Gamebreakers, Gamebreaker Wins, Back To Basics.
- The shot clock can be off or 12, 24 or 48 seconds; "If you fail to shoot
  the ball before time expires, a shot clock violation will occur and you
  will lose possession of the ball."
- Street School lists the moves: pass, short and long range shots, dunk
  ("While moving towards the basket, hold any Turbo and press Shoot"),
  alley-oop, trick moves, steal ("press Steal when standing close to the
  ball carrier"), block ("press Block when standing in front of the
  shooter"), rebound, switch players, pump fake, fadeaway, gamebreakers,
  pick, dive for a loose ball, and more.

The long-range arc was measured from Rucker Park's floor texture in a
top-down render (the floor model spans ±47.9 by ±32.6): the painted court is
±42.3 by ±27.4, the arc is 24.5 from the rim's spot on the floor with
straight lines at |z| = 22.4 in the corners, and the lane runs 19.4 from
the baseline and is ±6.1 wide.

## Database (`/DATABASE/*.ADF`)

Six tables in one layout, read by one generic table class in `SLUS_206.51`.
0x1da250 loads each file whole (`database/xdbplyr.adf`, ... through 0x1d3a38),
0x1da3c8 returns a field's kind and 0x1da4e0 its address. Decoder:
`crates/street-assets/src/db.rs`; report: `examples/db_report.rs`.

| Offset | Contents |
|---|---|
| 0x00 | u32 field count; every field is an `i16` |
| 0x04 | u32 record count |
| 0x08 | u32 offset of pool A (u32 values) |
| 0x0c | u32 offset of pool B (u32 values) |
| 0x10 | u32 offset of the string pool (NUL-terminated) |
| 0x14 | u32, always 0 |
| 0x18 | field kinds, 2 bits each, first field in the top bits of the byte |
| next 4-byte boundary | the records |

Kinds: 0 a plain value, 1 a byte offset into the string pool, 2 an index of
a u32 in pool A, 3 the same for pool B (0x1da4e0 stores the resolved address
at +0x2c, +0x28 and +0x24 of the table object). Only the courts use a pool
(pool A holds `f32`s). In all six files the records end exactly at pool A
(the teams after two bytes of padding), which confirms the layout
independently of the code: the players' 101 kinds take 26 bytes, so their
records start at 0x34; the other tables start at 0x1c.

| File | Records x fields | Contents |
|---|---|---|
| `XDBPLYR.ADF` | 240 x 101 | players |
| `XDBTEAM.ADF` | 49 x 11 | teams and their rosters |
| `XDBCOURT.ADF` | 11 x 10 | courts |
| `XDBMOVE.ADF` | 93 x 6 | named layups, dunks and dribble moves |
| `XDBALIAS.ADF` | 48 x 3 | nicknames |
| `XDBCARD.ADF` | 113 x 6 | the Rewards cards: id, category (0 Street Legends, 1 Legends, 2 Courts, 3 Jerseys, 4 Development Points, 5 Tips, 6 Codes), index within it, cost, picture (`XAFESCRN` sheet 2; 0 none), unlock parameter |

### Players

Record order: 0..144 the league players, five per team in team order (each league
team's roster in `XDBTEAM` lists exactly these records and their team field
points back); 145..157 streetballers (Stretch .. Murphy Lee); 158..183
legends; 184..233 generic ballers; 234..239 the create-player team. The model
loader 0x315790 splits them the same way: it tests `team == 0x25` first,
then compares the record number with 0x91, 0x9e and 0xb8.

Named fields come from the code that reads them. 0x2cec70 is a debug dump
("Story Mode Test Bed") that prints "First Name: %s", "Height: %d' %d\"",
"Build: %d", "Hair Style: %d" and so on, each next to the field it reads.

| Field | Meaning | Evidence |
|---|---|---|
| 0 | persistent id (Abdur-Rahim 0, 2002 rookies 501+) | data; 0x1a7804 turns ids 145..151 into unlock bits |
| 1, 2 | last and first name (strings) | 0x2cec70 |
| 3 | team | rosters; `== 0x25` tests |
| 4 | league position: 0 C, 1 PF, 2 SF, 3 SG, 4 PG | data (Shaq 0, Duncan 1, Pierce 2, Kobe 3, Nash 4) |
| 5 | jersey number, -1 for most non-league players | data (Abdur-Rahim 3) |
| 7 | guess: skin tone, 1..4 men light to dark, 5..6 women; streetballers carry their id | data; 0x1acb68 picks from per-tone lists |
| 8, 9 | height (inches) and weight (lb) as shown | 0x2cec70 divides by 12 for feet and inches |
| 10, 11 | height and weight the model is scaled to | 0x3153d0; equal to 8, 9 except six celebrities (+30 lb) |
| 12 | female | 0x2cec70 "Gender: Female" |
| 13 | guess: left-handed (in-game hand value 2 instead of 1, 0x156648) | Van Exel, Mobley, Odom, Coleman; but also Yao Ming |
| 14 | portrait index in `/FE/XAPORTS.BIN` (245 images) | traded players keep their old index; created players get 0xb0 + n (0x223dd0) |
| 16..26 | ratings 0..99 | getter 0x1d7d00 (field 16 + n, cheats, clamp to 99) |
| 27 | guess: play style 0..4 (shooter, slasher, playmaker, blocker, power) | data only |
| 28 | star flag | 0x1dbe30 builds star lists from it |
| 29 | four-letter code (`sabd`) | string |
| 30 | build | "Build"; loader picks the body by it |
| 32 | look (face) | "Look" |
| 36 | clothing: low 14 bits, colour variant in the top 2 | 0x1d81a8 (`& 0x3fff` for team 37), 0x1d8340 (`>> 14`) |
| 38 | shoes | "Shoes" |
| 39, 40 | hair style and colour | "Hair Style", "Hair Color" |
| 41, 42 | facial hair style (-1 none) and colour | "Facial Hair Style", "Facial Hair Color" |
| 43..45 | percentages added to three scale factors (`1 + v/100`) | 0x3153d0 |
| 46..67 | accessories (below) | 0x1d8568 maps slot n to field 46 + n |
| 68..73, 79..89, 90..100 | a created player's moves as `XDBMOVE` rows: 6 lay-ups, 11 dunks, 11 dribble moves (-1 for none; 74..78 unused); -1 in every disc record | 0x1d8778 (kind 0, 1, 2 at +0x44, +0x4f, +0x5a); 0x13a1f8 plays the row's move id for a slot |

Ratings by index (field 16 + index): 0 shots, 1 three-pointers, 2 dunks,
3 rebounds, 4 power, 5 blocks, 6 steals, 7 handles, 8 speed, 9 and 10
unknown. The front end shows seven skills, "Shots, Dunks, Power, Blocks,
Steals, Handles, Rebounds" (locale text), and 0x1d7f48 reads skill n through
the jump table at 0x45fbb0 as ratings 0, 2, 4, 5, 6, 7, 3. Ratings 1 and 8
are named from the data: three-pointers never exceed shots and are highest
for Bird, West and Peja; speed is 99 for Isiah Thomas, Cousy, Archibald,
Biggie Little. The nickname table uses the same seven skill numbers.

### Where a player's model is

The loader 0x315790 (with 0x316260 for accessories) builds each player from
`/XF/XF000002` entries, except created players, who come from
`/XF/XF000001`. Note that league players do **not** use the `body<b>_outfit<o>`
kit.

**league players** (record `i` < 145, build `b` = field 30, 0..2):

- Body: `[931, 934, 937][b] + v`, i.e. `Small<v+1>Body`, `Medium<v+1>Body`,
  `Large<v+1>Body` (table at 0x49a318). Variant `v`: 0 shorts, 1 shorts and
  T-shirt, 2 pants and T-shirt. It is 0 except in story mode, and a bicep
  band (fields 56, 57) forces 0.
- Morph target: `5i + v` (the player's face and shape on that body).
- Sheet: `5i + 3` when the side flag at +0x284 is set, else `5i + 4`.
  Their `jers` images show `5i + 3` is the coloured road jersey and `5i + 4`
  the white home one (Hawks red/white, Payton's Bucks purple/white). Record
  70 (Gary Payton, Bucks) finds his own sheets at 353/354, so the archive
  follows the record order, not field 14.
- `shor` gets a random image of `1014 + team`; variants 1 and 2 bind `tsht`
  to `1043 + 2 * team + home`, variant 2 binds `pant` to a random image of 1013.
- Field 39 non-zero shows the body's `zdreds` group (Brian Grant, Latrell
  Sprewell), textured from the `dred` image in their own sheets. Steve Nash's
  sheets have one too, and his morph target reshapes the group, but his
  field 39 is 0, so the game does not show it.
- Accessories: 22 slots, fields 46..67, value = image index in the slot's
  sheet (the exe falls back to image 0 past the end), -1 for none. Table at
  0x49a620, eight bytes per slot (large model entry, sheet entry); the model
  is offset by `[4, 2, 0, 0][b]` (0x49a6d0) for small/medium/large:

| Slot | Model | Sheet |
|---|---|---|
| 0 | none | none |
| 1 | headband: shows the body's `HEADBAND` group, image bound to `hdbd` | 949 |
| 2, 3 | `elpd` elbow pad L, R (965, 966) | 947 |
| 4, 5 | `elbr` elbow brace (959, 960) | 946 |
| 6, 7 | `knpd` knee pad (989, 990) | 951 |
| 8, 9 | `knbr` knee brace (983, 984) | 950 |
| 10, 11 | `bcep` bicep band (953, 954) | 945 |
| 12, 13 | `farm` forearm band (971, 972) | 948 |
| 14, 15 | `fawd` (977, 978) | 948 |
| 16, 17 | `wrap` (995, 996) | 952 |
| 18, 19 | `wrst` wristband (1001, 1002) | 952 |
| 20, 21 | `wthn` (1007, 1008) | 952 |

- Scale: `field 10 * 1.2 / 78` (0x3153d0); the width also depends on field
  11 and per-build constants at 0x49a2f8 (not reproduced yet).

**Everyone else with a model of their own** (0x315ef0..0x315fac):
streetballers `725 + 2(i - 145)` (model, then one sheet: `strc`, `bona`,
..., `lmur`); legends `751 + 3(i - 158)` (model, road sheet, white home
sheet: `jerv` for Julius Erving .. `orob`); generic ballers
`829 + 2(i - 184)` (`male01`..`male38`, `female01`..`female12`). The disc
test checks that every computed entry is a model, morph or sheet as
expected, and spot-checks the names.

`/XF/XF000002` as a whole:

| Entries | Contents |
|---|---|
| 0..724 | league players, five each (three morph targets, road sheet, home sheet) |
| 725..750 | streetballers |
| 751..828 | legends |
| 829..928 | generic ballers |
| 929, 930 | referee model `reff` and sheet (0x3168b8) |
| 931..939 | league bodies, small/medium/large x variants 1..3 |
| 940 | `Medium_Shadow` |
| 941 | body skeleton; 942..944 shorts rigs |
| 945..952 | accessory sheets |
| 953..1012 | accessory models, 10 kinds x large/medium/small x L/R |
| 1013 | pants (5 images) |
| 1014..1042 | team shorts (6 images each) |
| 1043..1100 | team T-shirts, road and home per team |

**Created players** (team 37) are assembled from `/XF/XF000001` by the
appearance object (0x31e938; slots set from the fields at 0x315900..0x315ae8,
resolved to entries by 0x31ee10). Per build there is a row of the table at
0x40f530: (entries per outfit, first body, first morph, facial hair count,
first head, hair styles, first face morph, looks).

| Build | Per outfit | Body | Facial hair | Heads | Hair styles | Looks |
|---|---|---|---|---|---|---|
| 0..3 (male) | 44 | 886, 1456, 2026, 2596 | 22 | 1150, 1720, 2290, 2860 | 17 | 18 |
| 4, 5 (female) | 10 | 3166, 3292 | 0 | 3226, 3352 | 11 | 6 |

- Outfit `o` (1..6) = table 0x49ad58[build][group], where the clothing
  value maps to a group: itself if below 11, 9 for 81, 10 otherwise (team
  uniforms 54..80). Body `body<b>_outfit<o>` = first body + per-outfit x (o - 1).
- After each body: facial hair morphs, then face morphs (one per look),
  then three shoe morphs (shoes 0..6, 7..12, 13+; 0x317750).
- Head `<male|female>hair<h>_head<n>_body<b>` = first head + hair x looks +
  look (hair style and look clamped to the counts). Hair style -1: no head.
- Sheets: skin `782 + 4 look + b` (men) or `854 + 2 look + (b - 4)`; hair
  `514 + 8 style + colour` (men) or `426 + ...`; facial hair `650 + 6 style
  + colour`; shoes `866 + shoes`; tops `84 + 28 b + 3 clothing + variant`
  (clothing 81: `84 + 28 b + 27`; 54..79: `clothing + 4`; team uniform
  (80): `team` road, `team + 29` home);
  bottoms `252 + 29 b + 3 clothing + variant` (clothing 9+: 27, 81: 28).

The six created-player records resolve to names that agree with all of this
(ANTHONY: `body0_outfit2`, `malehair03_head06_body0`; KATE:
`body5_outfit2`, `femalehair12_head19_body5`).

### Teams, courts, moves, nicknames

Teams: 0 index, 1 name ("LA Clippers"), 2 city ("Los Angeles"), 3 nickname
("Clippers"), 4..9 roster records (-1 empty), 10 logo in `/FE/XATEAMS.BIN`
(50 images; "Random League Team" uses 49). Teams 0..28 are the league in
alphabetical order, 29 Street Legends, 30 Legends, 31..35 regional
legends, 36 Generic Ballers, 37 "CP Team" (create player), 38..48 special teams.

Courts: 0 index, 1 name (getter 0x1d3670), 2 location (0x1d36e8), 3 the court
archive in lower case (`rucker` for `/XF/RUCKER.VIV`; the eleven ids are
exactly the court archives except GREENC), 4 unknown (0..2), 5..9 indices of
`f32`s in pool A (getter 0x1d3880). The five floats sum to 1 (0 for Rucker
Park '78) and 0x1dbe30 scales pool sizes by them; NYC Rec Center is
1, 0, 0, 0, 0 and Rucker Park 0, 0, 0, 0.1, 0.9, so probably the mix of
opponent difficulty tiers.

Moves: 0 index, 1 name, 2 kind (0 layups such as "Tear Drop", 1 dunks,
2 dribble moves, from the names), 3 level 2..4, 4 cost 50/125/250 by level,
5 an id (unique except for five dribble moves listed at two levels).

Nicknames: 0 id, 1 name, 2 skill in front-end order: "Splash" and "Rain" are
shots, "Poster" dunks, "Granite" power, "Denial" blocks, "The Claw"
steals, "Blender" handles, "Glass" rebounds.

## Audio (`crates/street-audio`)

Every sound on the disc is the publisher's PS2 audio in one of two codecs:

- **XA** (codec 0x0a): music, crowd beds and calls, commentary.
  15-byte frames of 28 samples: a byte with the predictor (high nibble) and
  shift (low nibble), then 14 bytes of nibbles, high nibble first. Inside
  SCHl stream blocks each channel starts with a 4-byte history (two
  little-endian `i16`); **bank sounds have no history** and start from
  silence. Read as history, their first frame is misaligned and the sound
  comes out rough and clipped, which was checked on every XA bank sound.
- **PS-ADPCM** (Sony VAG): the sound effects. A bank sound with no codec tag
  is PS-ADPCM, and one with no rate tag plays at 22,050 Hz.

Containers:

| File | Contents |
|---|---|
| `.ASF` (`/MUSIC/FE_MUSIC`) | one stereo 44.1 kHz SCHl stream: a front-end song |
| `.MUS` `MAIN` | a song cut into SCHl segments (stereo 36 kHz); `TRICKS` and `WHOOSH` are `BNKl` banks of stingers |
| `.MPF` | PathFinder map (`PFDx` 3.4) choosing the next segment; see below |
| `.ABK` (`AB`, v9) | AEMS module bank: scripts (below) plus a `BNKl` (offset at 0x1c); `SPCH_PLY.ABK` has none |
| `.AST` | SCHl streams: a court's crowd bed (the longest, stereo 36 kHz) and ~70 mono crowd calls |
| `/SPEECH/SPCHDATS.BIG` | 156 `.dat` files of commentary lines (mono XA SCHl, 36 kHz), named by situation |
| `.AEM` | AEMS event map: events to module actions (below) |

SCHl blocks: `SCHl` (`PT` patch header), `SCCl` (data block count),
`SCDl` (u32 samples, a u32 offset per channel, then per channel the
history and frames), `SCLl` (loop marker), `SCEl` (end). `PT` headers are
tag/length/big-endian values. The ones used: 0x82 channels, 0x84 rate,
0x85 samples, 0x86/0x87 loop, 0x88/0x89 bank data offsets per channel
(from the `BNKl` tag), and 0xa0 codec. `BNKl`: u16 version, u16 count,
u32 size, 8 bytes, then one u32 per sound giving the offset of its `PT`
header from that u32.

What plays when, as the game's code does it:

- Menu sounds are FE_MOD bank sounds by index (`0x1a1908` to `0x19e750`):
  3 move (117 call sites), 9 select (68), 4 back, 2 and 6 change and limit.
- Each court loads an ambience bank and a crowd bank plus stream (`0x19e160`,
  by `XDBCOURT` record), and six shared banks. Street School uses the Rec
  Center ambience with the Broad Street crowd. The table is in
  `street_audio::courts`.
- In a match the game fires AEMS events 0x20-0x51 through `0x19e840` from
  an audio listener (vtable `0x45c3a0`), whose slots match the scoring
  listener's (`0x43c208`). For example, slot 25 (baskets) fires the crowd
  events 0x3d, 0x3e, 0x41, 0x48 and 0x4d with a parameter. Animation event
  tracks fire sounds too (`0x331630`).
- Which sound each event plays is up to the scripts: see below.

## AEMS scripts (`.AEM`, `.ABK`; `crates/street-audio/src/aems.rs`)

the publisher's AEMS runtime is at `0x34a000..0x34f800`; its entry points are a table
at `0x418798` (add the event map, register a bank, fire `0x34b968`, update
`0x34bad0`, release `0x34bbb8`, set a global `0x34bc88`). Little-endian;
offsets are relative to the file and the loader adds the base.

**Event map** (`AE`, version 0x15): u32 project id at 4; u16 counts at 8
(parameter actions), 0xa (start actions), 0xc (events, 0x52 in
`GAME_EVT.AEM`), 0xe (globals, 7); offsets of those tables at 0x10, 0x14,
0x18, 0x1c.

- Parameter action, 0x18 bytes: i32 min, i32 max (the value is clamped),
  u16 bank id, u16 module id, u32 node hash; then the loader's bank, module
  and node index (-1 in the file).
- Start action, 0x10 bytes: u16 bank id, u16 module id, u32 node hash, and
  the loader's resolution.
- Event, 0x20 bytes: i16 parameter count; at 4 an offset to one `{u32
  offset, i16 count}` list of parameter actions per parameter; at 8, 0x10
  and 0x18 `{offset, count}` lists of start actions for firing, updating and
  releasing the event.
- Globals: `{offset, count}` lists of parameter actions (the volumes below).

Some actions name modules or banks this disc doesn't have; they do nothing.

**Module bank** (`AB`, version 9): i32 module count at 0xc, modules at the
offset at 0x10 (0x1c bytes each from version 8, else 0x14), u16 bank id at
0x14, i16 stream count at 0x1a, `BNKl` offset at 0x1c, stream table at 0x34
(twelve bytes per stream: u16, u8, u8 flags with bit 0 looping, u32 offset
of its `SCHl` in the `.AST`, u32 loop offset).

- Module: u16 id, i16 instances allowed, i16 node count, u16 rate group (0
  everywhere), i32 the event that owns it (its start creates instances;
  other events only pulse or set the live ones), u32 offset of its nodes,
  then the counts of connections and of nodes that run.
- Node: u32 name hash, u8 kind, u8 input count, u8 output count, u8, u32
  offset of `{u32 offset, i16 count}` per output, each a list of `(u16
  node, u16 input)` it feeds; then the kind's own words. Sizes by kind are
  at `0x418694` (eight bytes per kind: size, runtime size, output kind; the
  code pointer is the word before). Node 0 is the start trigger, the last
  node the end.

Each instance runs every node in file order every 10 ms (`0x34c448`, timer
`0x34c350`), copying each node's outputs into the inputs it feeds; constants
feed theirs once at creation. The kinds (code address):

| Kind | Node | Kind | Node |
|---|---|---|---|
| 1 | named input the map sets | 0x10 | modulo |
| 2, 3 | trigger, lasts one pass (`0x34c640`) | 0x11 | i16 table by range (`0x34d1b8`) |
| 4 | **player** (`0x34c8a8`) | 0x12 | one-shot delay (`0x34d300`) |
| 5 | range gate with hysteresis | 0x13 | delay line |
| 6 | envelope of (ms, value) points | 0x14 | constant |
| 7 | counter stepping on a trigger | 0x15 | LFO: sine, square, saw, triangle |
| 8 | random: weighted, uniform, or shuffle | 0x16 | pass |
| 9 | select input n | 0x17, 0x18 | max, min |
| 0xa | demultiplex | 0x19 | value on a rising edge |
| 0xb | sum, capped | 0x1c | end when its input rises |
| 0xc | difference, floored | 0x1d | fire another event on change |
| 0xd | product chain over a divisor | 0x1e | table lookup by range (`0x34d240`) |
| 0xe, 0xf | product capped, quotient | 0x1f, 0x20 | ramp; any input set |

The player's inputs: 0 pitch (0x1000 = the sound's rate), 1 a second rate
factor, 2 volume (0..0x7fff), 3 direction (0..0xffff, 0 ahead, growing to
the left), 4 spread, 5 reverb send, 6 the index into its sound list, 7 the
trigger (1 plays, 0 stops, 2 holds), 8 and 9 a filter, 10 a second volume
(the bank's level). A sound id `0x4000 | n` is the bank's sound n, `0xc000 |
n` its stream n (`0x34e8b0`, `0x34f4f0`). Its outputs: playing, ms left, ms
played.

The game's side (`0x19e840`): a position becomes an angle and a distance
from the camera (`0x19ebe0`: camera-space x and z/4, the angle
`atan(-x/z)` in degrees turned to 0..0xffff, the distance `(x² + z²) × 4`
up to 1000), and the event's parameters are laid out per event by
`0x19ed30` (most are `[angle, distance, value]`; the HUD's 0x4d is `[side
(0x2000 home, 0xdfff away), kind]`). Event 0x4b (`SFX_TRIC`) is missing
from that switch, so the game never plays it.

Globals, `(option × percent) / 100` with the option 0..0x7fff and the
percents the game's variables default to (`0x1a2340`): 0 crowd bank 54, 1
crowd stream 40, 2 player speech 88, 3 crowd reactions 90, 4 effects 87, 5
gamebreaker 86, 6 environment 60. Each module's own volume comes from event
0x1b, which a mixer module in the crowd bank sends (kind 0x1d) as it ducks.

## In-game music maps (`.MPF`, `crates/street-audio/src/path.rs`)

Each song folder's `<SONG>.MPF` is a PathFinder 3.4 map over three
tracks: 0 `MAIN.MUS` (SCHl segments), 1 `WHOOSH.MUS` and 2 `TRICKS.MUS`
(BNKl banks). Little-endian:

- Header: `PFDx`, version 3, 4; 0x0d tracks (3), 0x0e sections (1), 0x0f
  events (16), 0x10 routers, 0x11 variables, 0x12 u16 node count; at 0x24
  a u16 per node, its offset in words.
- Node: u16 segment (1-based over all tracks; 0 a control node, 0xffff a
  router), u8 track, u8 flags (high nibble a loop count, 0xf for ever; bit
  3 marks a transition's entry); u8 transition list, u8 branches (low 5
  bits the count, top 3 bits 1 = pick at random), u8 beats (high nibble x
  low nibble), u8; a word; then the branches, a word each: i8 low, i8 high
  (inclusive), u16 next node. The walker (`0x342be8`, ranges `0x342b08`)
  follows control nodes by the value until a node with a segment; no
  match ends the track.
- Then the event table (a router per track x event x section, bytes), the
  routers on a word boundary (0xff, flags, u16 node; flags 0x40 = at the
  next beat, else at the segment's end; node 0xffff with 0x40 stops the
  track), the transition lists (word offsets, then `from << 16 | to`
  words: while a node with list k plays, an event aimed at `from` plays
  `to` first and then `from`, `0x342dd8`), a word offset per track into
  the segment table, and the segments: u32 offset (MAIN: in words into
  `MAIN.MUS`; banks: the first of `beats` consecutive sounds), u32 length in
  ms. Every length equals its stream's.

The game's events (`AuPathManager`, `0x1aa4b0`..`0x1abf70`, fired from the
audio listener `0x45c3a0`): 0 the intro, looping (match start); 1 the main
groove after the current segment (play starts, `0x1a6f38`; after a
gamebreaker, `0x1ab5f8`); 2 the groove on the next beat (a meter emptied,
`0x1abbc0`); 3 and 6 the ready themes for a level 1 or 2 gamebreaker
(`0x1ab2f8`, by team+0x20); 5 and 8 their endings; 9 and 10 a gamebreaker
going up (`0x1ab3b8`); 11 a break in play (`0x1a5410`); 12..14 trick
stingers on `TRICKS` (`0x1a61b8` fires 10 + the trick level for levels 3
and 4 of 0..4, at most every two seconds); 15 the outro (`0x1a5638`). The
value the branches read is set on every basket to the score x 127 / the
points to win (`0x1a3b10`): it picks where the groove enters its loop.
The gamebreaker and stinger segments are the same recordings in every
song.

## Movies (`/MOVIES/*.NFX`, `crates/street-video`)

An `.NFX` is a run of chunks (four-byte tag, little-endian u32 size
counting the header). `MPCh` chunks carry an MPEG-2 video elementary
stream, one picture per chunk (the first has the sequence header); joined
they are the stream. Movies with sound interleave a SCHl stream
(`SCHl`, `SCCl`, `SCDl`..., `SCEl`) between them, which `street-audio`
plays once joined. The video is MPEG-2 main profile, 4:2:0, progressive
frame pictures with frame prediction and frame DCT, I/P/B with B pictures
reordered (the "MPEG Gimex module" encoder's user data): every one of the
153 movies decodes to its end with the small decoder in `street-video`.

| Movies | What |
|---|---|
| `XMT0017` | the publisher logo (sound) |
| `XNUFX` | NuFX logo (sound) |
| `XMT0016` | the league notice, then the team logos board (13 s) |
| `XMT0018` | the intro (44 s, sound) |
| `XMT0000`..`XMT0015`, `XMT0019`, `XMT0020`, `XMTBLUR*` | front-end animations: the title board, camera moves, transitions |
| `XMT0021`..`XMT0027` | long clips with sound: a host's segments, game footage, trailers (SSX Tricky) |
| `XMTFR*`, `XMTJP*`, `XMTPL*` | French, Japanese and PAL (640x512, 25 fps) versions |
| `XLP0001`..`XLP0016` | small loops (panels on screens and loading) |
| `FILMROOM/XMT0000`..`XMT0089` | 224x144 clips of each move, by its XDBMOVE move id (field 5); `TEMP.NFX` for none |

## Crowds (`crates/street-assets/src/crowd.rs`)

`/XF/<COURT>C.VIV` holds the crowd models (`<court>B.e3b` the body, and
`M`, `C`, `F`, `P` heads) and their sheets (`b`, `m`, `c`, `f`, `p`, all
64x64 images). It also holds `<court>t.bin`, two texture numbers per seat.
`/XF/XF000003` holds the seating plan `<court>w.bin`: a count, a group count,
then per seat a name, rotation (degrees), position, animation group, the
head kind (0..3: `M`, `C`, `F`, `P`), an integer the game drops, and an RGBA
colour. The colour tints the greyscale shirts,
with 0x80 as full intensity. Broad Street has no crowd. The game's loader
is `0x30b9b8`.

The `.e3b` mesh is not stored as triangles. Each part carries its own VU1
microprogram, unrolled: it loads the animated input vertices (V3-32 at
quadword 12 and up), transforms them, and stores each result into the
output slots of a GS triangle strip held in the part's data image. Each
output vertex is a texel UV quadword, then an XYZ2 template whose w bit 15
marks a strip restart. The decoder simulates the program's data flow (VU1
interlocks, so registers act in order) to find each slot's input vertex.
Every model on the disc then yields exactly its header's triangle count.
Animations are a key pose plus, per frame, three unsigned bytes per vertex
times per-axis scales, as offsets from the key (`0x3242b0`'s `lbu`). Each
record says 60, but the game shows a frame every four ticks (TRACE.md
"The crowd"). The game
scales spectators by 0.3257 (x, z) and a random height (y). The full layout
is in the module docs.

## Court effects (`/COURTFX/<COURT>.BIN`)

Plain text, ended by `~`:

- `SUN_DIRECTION` and `SHADOW_COLOUR` (gamma-space RGBA, 0..1) drive the
  projected shadows;
- `INDICATOR_*` colour the player indicator;
- `WIND speed x z`, `DRIFT n1 model1 n2 model2 x z`, `PS_STEAM x y z dx dy
  dz` and `COLD`/`HOT` are the ambient effects the PS2 game knows (with
  `PRECIPITATION`, `FOG`, `PS_FOUNTAIN`, `PS_SPARKS`, `PS_SPLASH`, which no
  court uses). Each becomes an effect object through the factory at
  `0x20dac0` (DRIFT type 8 at `0x1f4248`, PS_STEAM type 7 at `0x20a4d0`
  with 40 particles, COLD six of type 3, one per player).
- DRIFT scatters `n1` pieces of `/XF/XF000004` `<model1>.o` (resting at
  height 0.25, drawn at half size) and `n2` of `<model2>.o` (0.375, full
  size) over x in -x..x, z in -z..z; the model `0` means none.
- `PS_DUST` (Seattle) and the `E STREET`, `E STREET CONNECT`, `E STREET
  STOP` and `E CAR` lines (Center, Mosswood, NYCAGE) are not read by the
  PS2 executable. Their paths (`rc_sw.bin`: a count, then `x y z` lines),
  stop files and cars are in `/XF/XF000004`, but the PS2 game has no
  traffic.

The game reads it at `0x2f3b30`; parsed by `street_assets::courtfx` and
played by `street-game/src/ambience.rs`.

## Front-end fonts (`/FE/XFONTSEN.BIN`)

A `c0 fb` archive of 12 `FNTS` fonts (reader: `street-assets/src/font.rs`).
Layout text elements name theirs in prop 13: 0 help lines, 1..2 option rows,
4 menu entries, 6 the script titles; 7 is capitals and digits only, 11 the
score digits and signs only.

| Offset | Contents |
|---|---|
| 0x00 | `FNTS`, two bytes, u16 |
| 0x0a | u16 glyph count |
| 0x14 | u32 glyph table offset |
| 0x18 | u32 kerning table offset (0 for none) |
| 0x1c | u32 texture offset |
| 0x24 | u32 line height |

- A glyph is 12 bytes: u16 character, u8 width, u8 height, u16 x, u16 y (in
  the texture), u8 advance, i8 x offset, i8 top offset from the line's top
  (negative for the script capitals that rise above it), u8.
- Kerning: u32 count, then 4-byte pairs: u16 left character, i8
  adjustment, u8 right character.
- Texture: u8 format (1: 4-bit, low nibble first; 2: 8-bit), byte, u16, u16
  width, u16 height, 8 bytes, then the pixels as coverage.

## Front-end layouts (`/FE/XLAY_FE.BIN`, `XLAY_GM.BIN`)

`c0 fb` archives of 61 and 12 layouts. A layout: u32 1, u16 element count,
u16 table offset, u32 0; per element a u16 property count and a u32 offset;
each property a u16 id and a u32 value. Prop 0 packs alignment (bits 0..3),
the widget kind (4..11) and the element id (12..); the one kind-0 element's
id is the screen class. Prop 3 is the position and prop 4 the size (two u16
each, on 640x480); prop 10 the colour (ARGB); text widgets carry their font
in prop 13 and text ids (`file << 8 | index`, the file by the game's table
at 0x3ec2f0) in props 14..20 or 56..63; titles (kind 35) add their shadow
colours in props 23 and 24. Kinds: 19 menu entry, 35 title, 5 help line, 10
navigation node (its links in 25..32; drawn as the star bullets), 9 the
cursor's track, 34 a button prompt, 3 an image set by code. See
`docs/TRACE.md` section 4.

## Front-end backdrops (`/FE/XAFEBG.BIN`)

105 entries, three per backdrop: backdrop `n` is entries `3n..3n+2` laid
side by side (128, 128 and 64 wide, 256 tall); the picture is the
top-left 320x240, shown doubled. Backdrop 17's first entry is one 512-wide
texture holding the title picture in its left 320 columns. A screen's
constructor picks its backdrop through the base screen constructor
`FUN_002d7720(screen, class, backdrop)`: class 3 (main menu), 5 (Play It)
and 6 use 18; 0x1d (choose player) 5; 0x21 (choose court) 4; 0x27 (set
challenge) 25; 0x28 (options) 28; 0x29 (choose region) 6; 0x36 (story menu)
2; 0x38 (map) 19; 0x14 (posters) 21; 0x3d (create player) 3; 0x37 (control
config) 10; 9 (credits intro) 31; -1 for none.

## Front-end widget sheets (`/FE/XACOMMON.BIN`, `/FE/XAFESCRN.BIN`)

Both hold several SSH sheets (`fe::read`). Image names repeat: read them
by position.

`XACOMMON.BIN`:

| Sheet, image | Contents |
|---|---|
| 0, 0 | list scroll arrows |
| 0, 1 | rating stars in five colours |
| 0, 2..4 | rules, corners, a panel frame |
| 0, 5 | the menu bullets (blue and white stars) and the dark panel |
| 0, 6 | blue header tabs, a checkbox and a tick |
| 1, 0 | the pad's buttons as icons |
| 1, 1 | a PS2 controller |
| 2, 0 | the slider: frame (4, 45, 175x19); a tick unlit (8, 8) and lit (8, 21), 8x10 |
| 3, 0..18 | the spinning ball |

`XAFESCRN.BIN`: sheet 0 holds frames, the Rewards categories' icons, the
create-player tab icons and trophies; sheet 2 (104 images) the Rewards
cards, indexed by a card's field 4 (see `XDBCARD` above and
`street-game/src/rewards.rs`). Its last image, Nelly and the St. Lunatics,
repeats the name `0061` of image 97 (Just Blaze).

## Match HUD (`/FE/HUD.SSH`, `/FE/OFFIND.SSH`, `XLAY_GM` blob 1)

The overlay layout (screen class 2) places the HUD on 640x480: the scores in
font 4 centred at (285, 53) and (350, 53); trick points in font 1, amber,
at (94, 71) and right of (541, 71); the shot clock in font 6 at (540, 400);
the court's name (text `overlays.txt` 35) centred at (320, 310) in font 6
and its place (36) at (320, 354) in font 4; a turbo label, meter and value
at (50, 449), (100, 449) and (150, 449); and image elements (kind 3) that
code fills from the atlas. Which part goes in which element is not traced;
`street-game/src/hud.rs` matches them by shape.

`HUD.SSH` is one 256x256 atlas. Its parts (x, y, w, h):

| Part | Where |
|---|---|
| GAMEBREAKER, cream / outlined | (10, 3, 108, 13) / (8, 22, 111, 17) |
| TURBO in blue, green, red, yellow | (136, 8 / 24 / 39 / 94, 62, 10) |
| turbo bars blue, yellow, red, green | (114, 58 / 74 / 112, 85, 12), (103, 163, 85, 13) |
| frames: square, score | (7, 55, 45, 45), (60, 58, 47, 34) |
| long panel, slanted rule | (7, 127, 174, 34), (8, 164, 91, 16) |
| meter fills cream, orange, yellow, blue, brown | (8, 183 / 198 / 213 / 227 / 241, 169, 12..13) |
| replay mark | (195, 194, 43, 37) |

`OFFIND.SSH` (256x128) holds the players' colours, 1P red, 2P yellow, 3P
blue, 4P green: numbered badges (2 + 41k, 1, 38, 40), then three rows of
arrows (up at y 44, diagonal at 67, right at 95).

## Nets

There is no net model. The game builds a cloth of 7 rings of 10 points
(`0x31af58`, `0x31b2d8`): ring radii 0.77, 0.5, 0.42, 0.38, 0.36, 0.33 and
0.33, hanging 0.1, 0.55, 0.83, 0.965, 1.1, 1.2 and 1.3 below the rim, with
the top ring fixed. It is textured with the court's `<court>n.ssh` (image
`net1`), else the shared `net.ssh`.

## The other games' GameCube discs (`gcdisc`, `refpack`, `gsh`, `eagl_gc`, `gc_character`)

For the player editor: every character of the original game (`GNSE69`, only on
GameCube here) and Street V3 (`G3VE69`; its GameCube version alone has
the Nintendo All-Stars, Mario, Luigi and Peach), read from the user's own
discs and assembled whole by `street_assets::gc_character` into the neutral
`character::Character` (meshes with joints and weights, one skeleton,
finished RGBA textures). `crates/street-assets/tests/gamecube_disc.rs` and
`gc_character_disc.rs` check them when `STREET_V3_GC` / `STREET_V1_GC` name
the images; `examples/gc_report.rs` prints the rosters, assembles everyone
(`--all`), writes OBJ/PNG and front/side renders (`--out`), poses them
(`--pose`, `--pose-v3`, `--pose-v1`) and compares skeletons (`--skeletons`).
All 226 V3 characters and all 175 the original game characters assemble with every
model and texture found.

- **Disc images** are opened through the `nod` crate: `.iso`/`.gcm` and
  Dolphin's `.rvz`, `.wia`, `.ciso`, `.gcz`. The file system is the
  GameCube's own; paths are lower case (`/xf/player.viv`). `main.dol` lies
  outside it (`GcDisc::main_dol`); a few tables are read from it.
- **Archives** are BIG (`BIGF`, and `BIG4` in V3) as on the PS2. V3's
  entries are mostly **RefPack**-packed (`10 fb`): see `refpack.rs`.
- **Database tables** (`.ndf` in V3, `.adf` in the original game) are Vol. 2's ADF
  layout big-endian (`db::Table`). Header byte `0x14` scales string
  offsets by `1 << n`: V3's `xdbagame.ndf` (232 KB of strings) uses 3,
  `xdbbacrt` 3, `xdbafe` 2, `xdbcrte` 1, everything else 0.
- **Sheets (`SHPG`)**: the SHPS layout with big-endian numbers and GX pixel
  formats: `0x16` RGBA8 (4×4 tiles, 64 bytes: AR pairs then GB pairs),
  `0x19` C8 (8×4 tiles) with a `0x32` RGB5A3 palette block, `0x1e` CMPR
  (8×8 tiles of four DXT1 blocks). Pixels start 16 bytes into the block;
  bytes 12..15 of the block header are the image's position (low 12 bits
  of x and y), which V3's small overlay layers use.

### Street V3

- **Players** (`/database/xdbplyr.ndf`, 240 records of 109 fields): 0 id,
  1 last name, 2 first name (localisation keys such as `Kt_Nene` lose their
  prefix), 4 jersey, 5 kind (0 league, 1 street legend, 2 legend, 3 guest:
  Nintendo and Beastie Boys, 4 generic baller, 5 create-a-player preset,
  -1 debug), 12 team id, 13 position, 14 speech/nickname, 18 height (in),
  19 weight (lb), 22 female, 23 kid, 30..89 appearance ids (pool B), 91..101
  the eleven ratings, 105/106 front-end portrait assets. Records without
  appearance ids (the `Random` entries) are not characters. 226 remain.
- **Teams** (`xdbteam.ndf`, 43 records): 0 id, 1 name (`Atlanta\nHawks` or a
  key: `KT_NINALL` is the Nintendo All-Stars), 2 city, 3 nickname, 5..10
  player ids, 15..22 uniform appearance ids.
- **Appearance recipes** (`xdbagame.ndf`, 4,410 records): id (pool B),
  slot, type, a comma-separated recipe, a flag. Player fields 30..69 are
  the home look, 70..74 what the away look changes, 75..89 a second look
  17 players have. `main.dol` 0x801fa8bc reads a recipe: `aref:N` inlines
  record N (`arefmale:`/`areffemale:` only for that sex), `add:N` adds
  record N as an item of its own, `mod:Name?skeletontype=..&shorts=true`
  a model (`name.o`), `tex:slot_sex_set_layer_variant?tattoo=true` a
  texture layer, `morph:NAME_value` (value / 1000), `haircolour:N`,
  `jtext:` jersey lettering (drawn from TrueType fonts at run time; not
  decoded), `removehat...`, `cref:`. Adding an item removes the items with
  the same slot and type, and any hair item for a hair item (0x801fda28).
  Models are grouped by item slot; adding one to a group hides its first,
  so the newest is drawn. Blink lids (slot `Blink`) are left out.
- **Texture layers** are grouped by the first four letters of their name
  (`hairhats_...` binds the model slot `hair`) and drawn in layer order
  (the second-last name part), each at its sheet position. Variants are
  rewritten as 0x801f6920 does: `skinfix` -> `skinNN`, `skinkidfix` ->
  `kidNN` for kids, `<x>hairfix` -> `<x><colour><a|b>` with `b` for skin
  tones above 6. The skin tone is the `NN` of the skin type's hands
  texture (`hand_m_master_00_skin06`); the hair colour is `haircolour:N`
  (default 1). Luigi's hands are Mario's `Mhands`, listed in his record.
- **Models** are the same EAGL ELF container as the PS2's (relocated pointer
  words little-endian) with a **big-endian payload**. `__Model:::` keeps the
  PS2 layout (word 39 group count, 40 group names, 51 draw list).
  - Stream tables: a pointer to `Coordinates`, `Normals` or `UV`, u32 entry
    count, then 16-byte entries (u32 batch, u16 count, u16 format, u32 byte
    size padded to four, pointer). Coordinates f32×3; normals i16×3 in 8.8
    fixed point (2.14 in a few batches, same format code), UVs i16×2 / 1024
    or f32×2. Every batch has all three, with equal counts.
  - Each batch is a GX display list: `0x98` strip, `0x90` triangles, `0xa0`
    fan, a u16 count, then per vertex the position/normal matrix slot (a
    multiple of 3), the texture matrix slot (always that + 30, which rules
    out a wrong layout), and position, normal and UV indices. Their widths
    are per array, not per batch: one byte up to 255 elements usually, but
    Mario's eyes have 16-bit UV indices into 148 UVs. A list ends at its
    first non-draw byte. Vertex arrays can start with `0xa3`; pointers to
    them are not lists.
  - The render record names its list with a pointer (often unaligned), then
    `1, &EAGLAnimationBuffer, n, &rows` and the arrays. **Skinning**: `n`
    blend rows of four big-endian floats; a float's low byte is a bone of
    the model's skeleton and the rest its weight (`3f80000e`: bone 14,
    weight 1). A vertex uses row `slot / 3`. Positions are the bind pose.
  - The next `__EAGL::TAR` reference names the texture slot
    (`SHAPENAME=head,1;...`). GX front faces are clockwise; `eagl_gc`
    turns them counter-clockwise.
- **Skeletons** are Vol. 2's layout big-endian: `(hash, 0, count, 0)` then
  112 bytes per bone (scale, parent, rotation, translation, mirror bone,
  inverse bind). `skeleton.o` (44 bones) is everyone's but the Nintendo
  stars': `skel_nintendo_mario.o` and `luigi_skel_nintendo.o` (15 bones,
  same rig), `skel_nintendo_peach.o` (44 bones whose locals are the stock
  ones but whose inverse binds are scaled and partly mirrored; the
  assembler makes her bones rigid at the bind pose, which leaves the
  vertices alone). Shorts use `skel_streetshorts`/`skel_longshorts` (same
  names, other cloth-bone offsets); parts are remapped to the main skeleton
  by name.
- **Morph targets** (`*_morph.o`: `BS_<model>_FatShape`, `Hard`, `Kid`,
  `Soft`, `Thin`, and the create-a-player face shapes) are not applied;
  the Nintendo stars' are empty. Generic ballers therefore share
  `Head_M_Master`'s face shape.
- **Animations** (`/xf/anims.viv`: `anibig`, `anibs2`, `anibfe`, `anibin`,
  `anibju`, `anidel`, `anibtrex`, `anifig`, ...) are Vol. 2's EAGL banks
  big-endian, with a 28-byte bank header (`0x1c, count, 0, 0, records,
  names`), no `_X` state records, and a rotation record's flags word giving
  the target rig's hash: `0xd423` the Nintendo skeleton (whose file starts
  `d4230096`; channel = bone), `0x1dc5` Vol. 2's rig (channel order
  `Root, LThigh, LLowLeg, LFoot, LToe, RThigh, ..., Neck, Head`; only
  `createplayer_stand`), and `0xf79b`/`0xc036`/others V3's body rig
  (`skeleton.o`'s order without the face, necklace, shorts and shirt
  bones: `Root, LowerTorso, MidTorso, UpperTorso, Neck, Head, RCollarBone,
  ..., LToe`). 2,016 of 2,096 animations decode.

### Skeleton families compared

- V3's 44-bone skeleton holds all 22 of Vol. 2's body bones under the same
  names and parents, at the same rest positions. Its thighs are turned 180°
  about their own z axis (so every leg bone's model-space frame is too),
  its collarbones tilt by 0.7° and 2.5°, and its face bones differ (`Jaw`,
  `Cheeks`, `BrowL`... against Vol. 2's lower-case `jaw`, `browL`,
  `lowerlipL`...). V3's own body animations are made for its rig; its
  `0x1dc5` animations and Vol. 2's are made for Vol. 2's, which is V3's
  with the thighs turned back (`gc_character::V3Rigs`).
- The Nintendo skeleton names 15 of those bones (no toes, no lower or mid
  torso, no collarbones, no head; arms hang from `UpperTorso`), with Mario's
  proportions.
- the original game's 43-bone skeleton has Vol. 2's names, order and parents,
  including `Root_Face` and the lower-case face bones, but other rest
  frames (the root's turn, the collarbones 92° apart in model space, a few
  degrees elsewhere) and about three times the size. Its own animations
  drive it channel for channel; Vol. 2's and V3's play on it through
  `retarget`.
- `retarget::Retarget` plays any of them on any other: bones are matched by
  name (and a few face aliases), and each takes its partner's rotation
  relative to its rest pose in model space, so the turned thighs and
  Mario's missing collarbones come out right. Checked by rendering Vol. 2,
  V3 body and Nintendo animations on V3 league players, Mario, Peach and league
  Street players.
- `canonical::rebind` goes further for the player editor and the game: it
  puts any of these characters on Vol. 2's skeleton itself (Vol. 2's names,
  parents and local rest rotations, the character's own bone positions,
  scaled so the hips stand 4.05 units over the soles), so Vol. 2's clips
  and clips baked onto Vol. 2's bones (`canonical::Baker`) play on it bone
  for bone. All four families' bind poses are T poses facing the same way
  (y up, toes +z), which is what makes keeping each vertex in place right.
  Root tracks are floor-relative in every game: V3's in Vol. 2's units
  (`anim::TRANSLATION_SCALE`; `createplayer_stand` stands its root 4.01
  high), Mario's rig 1.52 at `breathe_1`, the original game's raw (11.7), so a
  clip's root scales by the two rigs' hip heights. the original game's display
  lists wind some triangles against their normals; the rebind turns them.

### the original game

- **Players** (`/database/xdbplyr.adf`, 175 records of 69 fields): 0 id,
  1 last name, 2 first name, 3 team row (29 marks the street legends, 30
  the street crews), 5 position (0 C .. 4 PG), 6 jersey, 15 height, 16
  weight, 22..32 the eleven ratings, 34 build (0 small, 1 medium, 2 large,
  3 female), 35 model key, 36 skin tone (0..5), 37 shoe style, 38 body
  sheet variant, 39 own body sheet, 40/41 crew body and head kits.
  `main.dol` reads them through 0x8004990c with the field number.
- **Teams** (`xdbteam.adf`, 43 rows): name, city, nickname, six numbers,
  three-letter jersey code (`atl`). Rows 29 and up are the street teams,
  whose line-ups are not in the table.
- **Models** are an older EAGL: `__Model::`/`__BBOX::` with two colons,
  texture slots as data symbols `__TAR::tar_<name>_<n>`, the skeleton
  inside the model (`__Skeleton::Root`, Vol. 2's 43 bones, layout as
  above), render method `CompactLitTexture_Skin`. A render record holds,
  before its display-list pointer `a`: (count, pointer) for positions
  (i16×3, 8.8 fixed point, checked against `__BBOX::`), normals (i8×3) and
  UVs (i16×2 / 256), then the list's size; at `a + 8` the `__TAR` slot;
  before them `1, &EAGLAnimationBuffer, n, &rows` as in V3. Display lists
  have no texture matrix byte and 16-bit indices. Units: a player is about
  20 tall, hips at 11.
- **Assembly** (`main.dol` 0x80190784 models, 0x80191364 textures), from
  `/xbigf002.viv`..`/xbigf006.viv` by name:
  - league players: `head<key>.o`, the `small`/`medium`/`large` body morphed
    to the player (the whole player, drawn as the morph target), with the
    body file's skeleton; sheets `head<key>.gsh`, the body sheet
    (`body<0lw|1mw|2dw|3lb|4mb|5db><variant>.gsh` by skin tone, or
    `body<key>.gsh`), `jers<team>h.gsh` (`a` away; `c`/`d` alternates),
    shoes `<style + 1>.gsh` (`%04d`) and `mout.gsh`.
  - Street legends (team 29): `<key>.o`, sheet `<key>ho.gsh` (`aw` away).
  - Crews (team 30): from the kit table at 0x8025caa8 (32 bytes per kit:
    name `cpNN`, build, body sheet, tattoo set): `bodycp<NN>.o`,
    `headcp<NN><s|m|l|w>.o`, shoes `<s|m|l><style>.o`; sheets
    `headcp<NN>.gsh`, `clthcp<NN>.gsh`, `body<sheet>.gsh`, `<style>.gsh`.
  - Hands: `<build>_lhand_relax.o`, `_rhand_relax.o` (Magma, the alien and
    the yeti have their own; rows 155..159 are women).
  - Slots bind sheet images by name; `larm`/`rarm` take `arm`,
    `lleg`/`rleg` `leg`, `shoe` the shoe sheet's image. Accessories
    (headbands, pads, braces, glasses) and shadows are left out.
- **Animations** (`/anim/anibot.ani` in game, `anifront.ani` front end,
  `aniface.ani` faces) are not EAGL banks: a table of (frames, channels,
  offset, flags), then per animation one root translation `(x, y, z, w)`
  per frame and the rotations, channel `c` driving bone `c`. Flag 2 clear:
  raw quaternions frame by frame. Flag 2 set: the first frame's
  quaternions, then per frame `channels × 4` bytes, each indexing
  `main.dol`'s 256-step table (0x8025b668) to add to the running component.
  Flag 1 mirrors (0x800ccee8: bone pairs and component flips from the
  table at 0x8025bb6c, the root's x negated). `anibot.ani`'s 614 names are
  the pointer table at 0x80252980 (`RUN_1`, `RUN_2`, ...). All of
  `anibot` and `anifront` decode; `aniface` (flag 4) does not yet.

## HD texture packs for the GameCube Vol. 2 (`hires`)

For the player editor: Dolphin custom-texture packs made for the GameCube
Vol. 2 (`GNZE69`), such as the "Legendary Edition" roster mod's HD faces,
jerseys, shoes, logos, portraits and loading screens, shown on the PS2 data.
`crates/street-assets/tests/hires_disc.rs` checks it when `STREET_GC_VOL2`
names the GameCube image, `STREET_ISO` the PS2 image and `STREET_HIRES`
the pack's folder.

- **File names** are Dolphin's: `tex1_<w>x<h>_<texhash>[_<tluthash>]_<fmt>.png`
  (no `_m` mipmap mark or `$` wildcard in this pack). `<fmt>` is the GX
  format number (`6` RGBA8, `9` C8, `14` CMPR, ...); `<w>x<h>` the texture's
  own size, not the PNG's (replacements are often 2x, sometimes the same
  size with new art). Packs sort files into folders freely; only the name
  counts, so folders are searched recursively.
- **The hashes** (`TextureInfo::CalculateTextureName`) are XXH64 with seed 0:
  `texhash` over the base level's encoded bytes, `TexDecoder_GetTextureSizeInBytes`
  of them (whole tiles: 8x8 texels of 32 bytes for CMPR/C4/I4, 8x4 for
  C8/I8/IA4, 4x4 for the 16-bit formats, 4x4 of 64 bytes for RGBA8);
  `tluthash` (C4/C8/C14X2 only) over the palette entries from the lowest
  index the texture uses to the highest, two bytes each as stored
  (big-endian RGB5A3 here). Proved by reproducing every one of the
  Legendary Edition's 708 distinct names from the GameCube disc's sheets.
- **SHPG block types are the GX format + 0x10** (`0x16` RGBA8, `0x19` C8,
  `0x1e` CMPR); the palette block is `0x32`. The hashed bytes start 16
  bytes into the pixel block, the palette's 16 bytes into its block.
- **The GameCube disc keeps the PS2's layout**: the same files (lower
  case), the same `/xf/xf000002` and `/fe/*.bin` entries (the FE files are
  `c0 fb` archives on both), the same images in the same order. Named BIG
  archives (courts) sort their entries by name, so `seattlo.gsh` and
  `seattlo.ssh` sit at different entry numbers; they pair by name. Image
  names repeat within a sheet (`atl ` twice in a logo sheet), so images pair
  by position, checked by comparing 16x16 thumbnails (the pack's pairs
  differ by at most 0.004 in mean absolute RGBA, 0..1).
- **Loading screens** are cut differently: `/fe/xafeload.bin` entry `k` is
  three strips (128, 128 and 64 wide, 256 high: a 320x256 picture) on the
  GameCube and ten 128x256 tiles (5x2, 640x512) on the PS2. The pack's
  strips are joined into the whole picture. A picture smaller than the
  PS2's that shows the same thing (the Legendary Edition's 3, 12 and 13
  are the GameCube's half-size cards with small touches) is not used: it
  would only blur the PS2's. Its 5 is a new 640x512 card and is.
- **Front-end backdrops** (`/fe/xafebg.bin`, three strips per backdrop)
  pair strip by strip and are joined the same way.

The table `crates/street-assets/data/hires_gnze69.tsv` (made by
`cargo run -p street-assets --example hires_map -- <gc disc> <ps2 iso>
--pack <dir> --out <tsv>`) holds hashes and indices only: per row the
Dolphin key, `image` or `strip`, the PS2 file, entry and image index, name,
size and the XXH64 of its decoded RGBA pixels, the GameCube place, and the
match score. At run time `hires::Pack` finds a PS2 image's replacement by
that pixel hash, so callers need not know where an image came from;
`Pack::from_discs` pairs another pack from the two discs instead (about 15 s).

The Legendary Edition (714 PNGs, 708 distinct names, 1015 table rows) pairs
704 names: 715 rows of player sheets (`/xf/xf000002`: jerseys, faces,
shoes, headbands, shirts and shorts), 144 portraits (`xaports`), 136 logos
(`xateams`), 3 backdrop strips, 14 loading-screen strips (pictures 3, 5, 12
and 13 complete; 25 lacks a strip), one court image (Seattle), `xafescrn`
sheet 0's sprite sheet and `/xldscrn.bin` (street-game cuts the first
into pieces, which the pixel hash does not find, and does not draw the
second). Not paired: the three
Xbox 360 controller pictures (the GameCube pad's sheet in `xacommon`; the
PS2's holds the DualShock, laid out differently) and one GameCube-only
court picture (`indoor3` image 0 is `phil` on the GameCube, `fost` on the
PS2).

## Online protocol (`crates/street-net`)

Not a disc format: the UDP protocol of Streetball Revival's own online play
(ROADMAP section 5). Clients talk only to the server, never to each other.
Every machine, the server too, plays on its own disc's data (`street_net::disc`:
the pose bank, the gait records with the clips' hand flags, the staged Level 2
gamebreakers' plans and hand tracks, the player and team tables); none of it
crosses the network, only a SHA-256 digest of it, in the join.

**Datagrams.** Each starts with `0x53` (`'S'`, never `0xE5`, MyBaller's
sealed-datagram marker), the protocol version (23), and the message kind.
Fields follow in order: counts and ticks as LEB128 varints (`uv`; signed
ones zigzagged), ids and checksums as fixed little-endian integers, text as
a varint length and UTF-8 (names at most 32 bytes), and pads as the
simulation's 6-byte `Input` (stick x, z, aim x, z as `i8`, buttons `u16`).
A datagram is at most 1200 bytes; longer messages travel as `Part`s.

| Kind | Message | Direction | Fields |
|---|---|---|---|
| 1 | Join | C→S | match name, people (count; each name, team `0`/`1` or `0xFF` for any), setup flag `u8` and the setup if 1, the joiner's disc digest (32 bytes) |
| 2 | Ready | C→S | `bool` |
| 3 | Inputs | C→S | `need` (first confirmed tick the client lacks), `start` tick, local seats (length-prefixed bytes), tick count (at most 64), then per tick a changed-seats mask `u8` and the changed pads |
| 4 | Checksums | C→S | count; each tick (`uv`) and checksum (`u64`) |
| 5 | Ping | C→S | id `u32` (the client's clock in ms) |
| 6 | Leave | C→S | |
| 10 | Side | C→S | team `u8`: in the lobby, move this client's people to that side's free seats |
| 11 | Pick | C→S | the client's lobby pick (length-prefixed, at most 48 bytes; the game's own, see below) |
| 12 | Setup | C→S | in the lobby, from the match's creator only: the setup as everyone picked it |
| 16 | Welcome | S→C | the client's seats (bytes, 0 = 1P), the setup, the setup's version `u32` |
| 17 | Refused | S→C | `u8`: 0 version, 1 side full, 2 server busy, 3 invalid, 4 roster against the online rules, 5 roster mod the server lacks, 6 the joiner's disc data isn't the server's (its digest), 7 no match by that name and no setup to make one |
| 18 | Lobby | S→C | count; each seat, team, name, ready, pick (length-prefixed); then the setup's version `u32` |
| 19 | Start | S→C | tick |
| 20 | Frames | S→C | `first` tick, `ack` (first tick whose pads the server still wants from this client), `heard` (newest tick it has), `lead` (signed; `i16::MAX` unknown), frame count (at most 64), then per frame a present-seats mask `u8`, a changed-seats mask `u8`, and the changed pads |
| 21 | State | S→C | tick, the match snapshot (length-prefixed) |
| 22 | Pong | S→C | the ping's id, the server's next tick |
| 23 | Closed | S→C | `u8`: 0 game over, 1 timed out, 2 server shutting down |
| 32 | Part | both | transfer id `u16`, index `u8`, count `u8` (at most 64), then a slice of the whole message's bytes |

**Pad runs** (Inputs and Frames) are delta-coded: a seat's pad is written
only when it differs from the tick before; the first tick of a message is
coded against an empty pad, so each datagram reads on its own. A quiet frame
costs two bytes. A present seat with no pad written repeats its last one (an
empty pad if it was absent).

**Match setup**: court id (text), the two team ids (`u16` each), the roster
(count; a `u16` database record per world player), seed `u32`, the six seats'
teams (`u8` each; the default is home for 1P 3P 5P, away for 2P 4P 6P), the
world snapshot at tick 0 (length-prefixed), an opaque `extra` blob the game
may fill (length-prefixed), and the match's roster as data (length-prefixed;
empty for the disc's). The rules travel inside the world, the Replays option
too (the creator's). The world carries the match's own values of the disc's
data and none of its tables; the server and every client install their own
disc's under them (`World::install`) before playing it.

**World snapshot** (`street_sim::World::to_snapshot`): `SW`, a format
version (8), then every field of the world in declaration order: the tick,
the hoops, the players (team, nine ratings, position, heading, speed, gait,
action with its fields), the ball (position, velocity, state, shot, pass,
flight time), the rules, the score, both teams' trick-point state and combo
chains, possession, shot clock, phase, the last tick's events (including whether a steal intercepted a pass), the give 'n
go, the pick, each player's last turbo press (plus one, 0 for none), the
generator's six state words, native trick selections and staged actor clocks,
and each player's previous pad. Floats retain their exact bits for deterministic
rewrite save/restore. Variable staged geometry makes the current size depend on
the match configuration; the network world payload limit is 192KiB.

**Match snapshot** (`Match::to_snapshot`): the world snapshot
(length-prefixed), then per seat its team, the player it drives (`0xFF` for
none) and its last pad, then the replays (length-prefixed,
`street_sim::replay::Replays::encode`: the manager's ring of recorded updates,
each its world tick, the director's note and a script flag, the newest one's
recorded objects, the manager's state, speed, ramp, position, request, offer and
prompt, the director's notes, the gamebreaker task's step), the match's tick
(`uv`), the network's clock, which runs on while a replay holds the world's,
and the updates a staged Level 2's burst still holds the game (`uv`, at most
31: the match holds the 31 updates after the press, as the game alone holds
them until GB2's burst wipes).
The checksum the clients report folds the replays and the match's tick into
the world's.

**The lobby** (`street-game`'s `lobby.rs`, after Street V3's online
screens: `selsides`, `exh_selteam2`, `exh_selplayer2` on its disc). Everyone
joins first; Side moves a person to the other side's free seats (the server
welcomes them again with the new seats). Each client's Pick goes to everyone
in Lobby: the game writes a stage byte (0 choosing a side, 1 side set, 2
picking, 3 picked), a count, then per side it picked the side `u8`, the team
`u16` and three player records `u16`. Each side's first person (its lowest
seat) picks its team and players at the same time as the other side's; the
creator also picks for a side nobody plays, then the court and rules. With
every side picked, the creator sends Setup: the server checks it as it checks
a creation (its shape, the roster and the world against it, the disc's data),
replaces the match's setup, bumps the version, un-readies everyone and
welcomes them all again. A client whose Lobby shows a newer version than its
Welcome joins again for it every 250 ms until welcomed. Everyone loads the
match and readies; it starts as before.

**Exchange.** A client resends Join every 250 ms until it is welcomed or
refused, and pings every 500 ms (repeating its ready flag, side and pick in
the lobby, and a proposed setup every second until it is taken). The
server sends Lobby on any change and every second. Once everyone seated is
ready, the server sends Start and runs 60 ticks a second. Each tick it fixes
every occupied seat's pad (the one received for that tick, else the seat's
last), steps its own match, and sends every client a Frames from that
client's `need`. Clients send Inputs every tick, repeating every tick from
the server's `ack` on, and a checksum of each confirmed state at ticks that
are multiples of 30. The server keeps 600 ticks of frames and checksums; a
client whose checksum differs, or whose `need` is older than that, gets a
State (at most every 300 ms). `lead` is the worst margin, over the last 20 to
40 ticks, by which the client's pads arrived before their tick; clients aim
for 2. Anyone silent for 5 s is dropped, and their seats go to the computer.
A Join whose disc digest isn't the server's is refused (6), and one that names
no match and brings no setup (7); the server makes no match of its own. A
replay holds the world on every machine alike (the match steps the game's
replay manager, director and gamebreaker task after the world, and while a
replay plays it doesn't step the world), and a finished match stays up 120 s
from the end of its won game's replay.


World snapshot version8 adds the steal event's explicit pass-interception
origin. Network protocol version6 rejects older builds at handshake rather
than accepting a client that cannot read the corresponding snapshots. Story
save formats are independent of these transient match snapshots.


World snapshot version10 additionally owns the six native RNG state words,
per-player direct trick slot configurations and latched script identities.
GBCv5 uses lossless compression of full-precision hand positions/body-width
basis with 128KiB encoded and1MiB decoded bounds. Configuration checksums use
canonical uncompressed content. Network protocol7 rejects older peers and
permits255 fragments per message and192KiB world payloads; reassembly remains
bounded and timed out. An actual staged configuration117,604 bytes fits these
limits without geometry quantization. These transient formats are independent
of original console and persistent story save formats.


GBCv6 added native staged playback metadata with world snapshot11 and network
protocol8. The current stateful clock integration uses GBCv7, GBQv3, world
snapshot12 and network protocol9 to retain manager seeks, actor updates, blend
progress and motion-rate history separately. Earlier transient match layouts
are rejected. Encoded/inflated and network fragmentation bounds stay unchanged;
persistent clocks are integrated and validated; native timeline differences remain
recorded. Persistent per-user progress uses separate optional
records sections and never shares this network snapshot version.


Network protocol 12 and world snapshot 60 (2026-10-20): the referee holds the
disc. The join carries the joiner's disc digest, refusals 6 and 7 are new, and
the world snapshot no longer carries the disc's tables (the gait records and
hand flags, the staged plans and hand tracks, which version 59 sent with every
match's setup and full state): only each player's gait profile and the staged
players' model scales and width ratios. A match's setup is about 3 KB in 3
datagrams, where the staged tables (118,788 bytes compressed) and the gait's
(11,794) had made it over 130 KB. The match snapshot gains the replays and
the match's tick.
