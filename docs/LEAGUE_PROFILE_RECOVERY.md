# League Challenge user ownership (2026-10-13)

Evidence is static US PS2 code from the original `SLUS_206.51`, SHA-256
`9f2cf237da7bd78dada337a2939f3878c05ddbe39588570b5328154cbe637614`.
Ghidra project `street2ee` uses EE opcode substitutions; the control flow and
field stores were checked against the original ELF with `tools/mipsx.py`.
Retained decompilation and raw instruction lists live at
`%TEMP%/opencode/street-re/mode-league-profile-probe-20261013/`.

## Observed ownership and lifecycle

- `FUN_001ee168` / `FUN_001ee1f8` bind and initialize a complete 0xbc-byte
  controller user payload at object +0x18. The Challenge fields are +0x48
  packed flags, +0x4a selected team, +0x4c ordinary win mask and +0x50 regional
  boss bits. Their serialized offsets are +0x30, +0x32, +0x34 and +0x38.
- `FUN_001ee450` checks the name's first byte for empty or space, searches
  the persistent user table by name with `FUN_001ef1e8`, obtains a free slot
  only when needed and stores the complete payload through `FUN_001ebbc8`
  (raw call at 0x1ee514). Empty-name guests have a live payload and bypass
  this storage. The postgame flush `FUN_001dff80` calls this for each of the
  four live controller users (raw 0x1dffcc).
- The mode-4 selected-side arm of `FUN_001dbb20` calls `FUN_001ee3d8`
  (raw 0x1dbba4), writing the selected team to user +0x4a. `FUN_001ee548`
  counts that team as already beaten independently from the ordinary mask;
  valid league team ID zero is Atlanta. The asset API's old name `current_court`
  for serialized +0x32 is misleading: the native consumers establish a team.
- `FUN_001ee3e0` starts the selected region and sets the unbeaten bit6 only
  when pending bit7 is clear. `FUN_001ee790` marks an ordinary result and
  pending status; a loss clears bit6. `FUN_001ee9a0` marks a boss win,
  clears pending and sets the region to the -1 sentinel. The guest's live
  payload undergoes the same result operations, subject to the established
  selected-human-seat gate in `FUN_001ded40`.
- `FUN_001eeb18` clears ordinary and boss masks on Continue after completion;
  the lifetime trophy and rewards are in the shared game record.

## Rewrite changes

`Records.User` now owns a league payload containing ordinary and boss wins,
loss tracking, selected team, original packed flags and the rewrite lineup.
The active `Story` fields remain a live mode cache, bound when User Setup
continues. Starting and finishing a league match uses the same binding helpers
covered by the real-disc menu regression. Named payloads round-trip in an
appended `LEAGUEU` extension of the records section; guests do not enter it.
Deleting a selected name detaches its live cache.

Original saves import each user's masks, own team and packed flags separately;
the previous union across names is removed. The source save bytes remain
preserved by the existing importer. Earlier rewrite saves contain no Challenge
owner: their original shared masks/roster/loss fields remain an unassigned
snapshot, while every selected name starts with its own stored payload. No
name is guessed for that old snapshot. Shared RP, cards and trophy survive.

The existing rewrite player chooser permits arbitrary three-player lineups.
When all selected players have one database team, that team supplies the new
own-team ID; a mixed lineup keeps the existing custom-side ID37. This is an
explicit adaptation of the rewrite chooser, not evidence of an original
cross-team player chooser. Imported valid teams supply their disc roster.

## Validation and boundaries

Focused tests cover A to B to A, save/reload, guest isolation, earlier record
formats, deletion shifting user indices, native own-team ID zero, distinct
original users' ordinary/boss masks, retained packed flags, and production
User Setup binding through match preparation and the postgame helper. The
league setup/reward/import tests run with the actual supplied USA ISO.

No original runtime profile replay was captured. Resetting a guest payload
on each rewrite mode entry is an explicit policy; only the native guest
persistence exclusion is proved, and its precise live re-entry reset timing
has not been traced. The exact original screen sequence for replacing a team during a pending region, controller-multiplayer
league attribution beyond the already supported single home seat, and imported
custom-team-ID reconstruction remain unresolved. Other original per-user
systems (including Street School payloads) are outside this slice. This change
does not establish full native menu or mode parity.


Independent review also found that native1ee790/1ee9a0 commits the currently
selected ordinary team's automatic win into the region's stored mask after
every ordinary result and a boss win. Production now commits that bit under
the same condition, preserving it if the selected team later changes. Boss
loss excludes the update. The regression covers both ordinary outcomes, both
boss outcomes, team changes and named-profile save/reload.
