//! Readers for the original game (PS2, SLUS-20651) disc data.
//!
//! Everything here parses bytes read from the user's own disc image at runtime.
//! No game data is stored in this repository.
//!
//! - [`iso`]: ISO9660 file system (the disc image itself)
//! - [`big`]: BIG/VIV archives (`/XF/*.VIV`, `/XF/XF00000n`)
//! - [`locale`]: the `c0 fb` locale/text archives (`/LOCALE/*.EUS`)
//! - [`ssh`]: SHPS texture sheets, PS2 flavour (`*.ssh`)
//! - [`eagl`]: EAGL models, ELF relocatable objects carrying VIF/DMA geometry (`*.o`)
//! - [`court`]: one court's models and textures, loaded from the disc
//! - [`crowd`]: a court's crowd: the seating plan and the animated spectator models (`.e3b`)
//! - [`font`]: the front end's bitmap fonts (`FNTS`) and screen layouts (`XLAY`)
//! - [`courtfx`]: a court's effects script (sun, shadow colour, litter, traffic)
//! - [`player`]: player models and textures, assembled the way the game does
//! - [`morph`]: morph targets (players' faces on the shared bodies)
//! - [`anim`]: EAGL animation banks (`/ANIM/*.BIN`)
//! - [`clip`]: one decoded animation and its joints, sampled as the renderer and the match's tables sample them
//! - [`ee`]: the EE's single-precision arithmetic (chopped multiplies and adds)
//! - [`db`]: the game database (`/DATABASE/*.ADF`): players, teams, courts, and where each player's model is
//!
//! The other the original games, for the player editor (read from the user's
//! own GameCube discs the same way):
//!
//! - [`gcdisc`]: GameCube disc images (`.iso`, `.rvz`, `.ciso`, ...)
//! - [`refpack`]: RefPack compression (most of V3's archive entries)
//! - [`gsh`]: SHPG texture sheets, GameCube flavour (`*.gsh`)
//! - [`eagl_gc`]: EAGL models with GameCube (GX) geometry and skins
//! - [`hires`]: Dolphin HD texture packs for the GameCube Vol. 2, shown on the PS2 images
//! - [`retarget`]: driving one skeleton with another's animation
//! - [`gc_discs`]: the GameCube discs street-game was given, and their
//!   characters loaded for drawing
//! - [`canonical`]: any game's character on Vol. 2's skeleton, and other
//!   rigs' clips baked into Vol. 2's bone frames
//! - [`gc_character`]: whole V3 and the original game characters (rosters, recipes,
//!   layered textures, skeletons, animations) as a neutral
//!   [`character::Character`]

pub mod anim;
pub mod big;
mod bytes;
pub mod canonical;
pub mod character;
pub mod clip;
pub mod court;
pub mod courtfx;
pub mod crowd;
pub mod db;
pub mod eagl;
pub mod eagl_gc;
pub mod ee;
mod error;
pub mod fe;
pub mod font;
pub mod gait;
pub mod gc_character;
pub mod gc_discs;
mod gc_v1;
pub mod gcdisc;
pub mod gsh;
pub mod hires;
pub mod iso;
pub mod locale;
mod lzari;
pub mod morph;
pub mod moves;
pub mod ordinary;
pub mod overlays;
pub mod passes;
pub mod playback;
pub mod player;
pub mod pose;
pub mod refpack;
pub mod retarget;
pub mod roster_edits;
pub mod roster_rules;
pub mod roster_share;
pub mod save;
pub mod scripts;
pub mod ssh;
pub mod timeline;
mod vif;

pub use error::{Error, Result};
/// The animation-record names, indexed by the game's own record id.
pub use moves::MOVE_SLOTS;
