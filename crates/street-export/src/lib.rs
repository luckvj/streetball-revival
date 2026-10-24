//! street-export: the player editor's export folder, made from the player's
//! own discs for the MyBaller launcher's builder. The contract it follows is
//! `docs/PLAYER_EDITOR.md`; nothing here ships game data.
//!
//! # Usage
//!
//! ```text
//! street-export characters --out <dir> --vol2 <iso> [--roster-db <dir>]
//!                          [--hires <dir>] [--v3 <disc>] [--v1 <disc>]
//!                          [--only vol2:0,vol2:158] [--all-anims] [--no-ui]
//! street-export save-info <street_save.dat>
//! street-export check-roster <roster.json> --vol2 <iso> [--roster-db <dir>]
//!                            [--save <street_save.dat>]
//! ```
//!
//! `characters` writes:
//!
//! - `catalog.json`: all 240 Vol. 2 records (league players, streetballers,
//!   legends, generic ballers and the six created-player kit records) with
//!   their named fields, all 101 raw fields, each one's effective move slots
//!   (6 lay-ups, 11 dunks, 11 tricks: the equipped `XDBMOVE` row, else the
//!   legends' own slot-1 script, else the native default, resolved by
//!   `street_assets::scripts::MoveScripts::slot_script` as the game does),
//!   the teams, the 93 moves with the clips each plays, every move script,
//!   an animation set per character (the match roles plus the walks and
//!   runs its gait records offer it, its own first), a clip index, and the
//!   Create A Baller options (builds, sizes, outfits and their prices, kit
//!   limits, accessory slots).
//! - `models/vol2/<record>.glb`: one skinned, textured character each, on
//!   the 43-bone Vol. 2 skeleton with the game's bone names. The armature
//!   carries the character's uniform scale (`model height * 1.2 / 78`, in
//!   metres: [`vol2::METRES_PER_UNIT`]) and a half turn so it faces +z; the
//!   joints carry the native width adjustment and head scale; the root's
//!   rest translation lifts the body onto the floor, as every animation's
//!   root track does. Hidden draw groups are left out; materials are
//!   double-sided, alpha-tested where the image has alpha.
//! - `anims/vol2/base.glb` (the match roles, the Create A Baller stand,
//!   celebrations, outros, every gait clip) and `anims/vol2/moves.glb`
//!   (every clip a move script plays): the skeleton and named clips, rotation
//!   tracks per bone and the root's translation, 30 frames per second.
//!   `<name>@mirror` is the game's mirrored (left-handed) twin of a clip.
//!   `--all-anims` also writes every bank whole.
//! - `portraits/vol2/<record>.png` (`XAPORTS`).
//! - `ui/` and `ui/manifest.json`: backdrops, fonts (atlas + metrics), the
//!   widget atlases with crops of the menus' pieces, team logos, layouts and
//!   strings, and Create A Baller swatches (see [`ui`]).
//!
//! `--roster-db` reads a roster mod's tables in place of the disc's (the
//! catalog's `roster` becomes `legendary`). `--hires` swaps in a Dolphin HD
//! pack's images through `street_assets::hires` ([`hires::Pack`]).
//! `--v3`/`--v1` add Street V3's and the original game's characters from the
//! player's GameCube discs ([`gamecube`]): `models/v3gc/<key>.glb` and
//! `models/v1gc/<key>.glb` on Vol. 2's skeleton (same joint names and rest
//! frames, the character's own proportions), so every game's clips play on
//! every model; their animation libraries baked onto that skeleton
//! (`anims/v3gc/base.glb`, `anims/v3gc/nintendo.glb`, `anims/v1gc/base.glb`,
//! `anims/v1gc/front.glb`); rendered head shots in `portraits/<game>/`; and
//! catalog characters, teams, animation sets and `games` entries.
//! `--only v3gc:mario` exports just that character (the libraries are
//! always written).
//!
//! `save-info` prints a street-rs save's users, created ballers
//! (`created:<slot>`), created teams and progress as JSON.
//!
//! `check-roster` says whether a `roster.json` may be played online
//! ([`check`], `street_assets::roster_rules`); the catalog's `rules` object
//! gives the same bounds for the builder to clamp to.
//!
//! # Size and time
//!
//! Measured on a 12-thread machine with a release build: a full export
//! (240 characters, both libraries, `ui/`, portraits) takes about 3 s and
//! writes about 204 MB in 1,345 files: models 164 MB (0.6..0.9 MB each,
//! their textures as PNG), libraries 6.4 MB (327 clips), `ui/` 23 MB (16 MB
//! of it kit swatches), portraits 3 MB, `catalog.json` 1.6 MB.
//! `--all-anims` adds 14 MB (all 646 clips by bank). A debug build is a
//! few times slower.

pub mod anims;
pub mod check;
pub mod export;
pub mod gamecube;
pub mod glb;
pub mod headshot;
pub mod hires;
pub mod model;
pub mod out;
pub mod picture;
pub mod save_info;
pub mod ui;
pub mod vol2;
