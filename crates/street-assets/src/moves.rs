//! Default move slots for the US PS2 release (SLUS-20651).
//!
//! These ids name script-table entries, not `XDBMOVE` rows or move ids.
//! `0x43d6c0` holds the six shots, `0x43d5d0` the eleven dunks, and
//! `0x43d498` the eleven tricks. Purchased move **row indices** are mapped
//! through `0x45f9c8`; see [`crate::scripts::MoveScripts::row_script`].

/// Two-turbo shot masks at `0x43d460`.
pub const TURBO_SHOTS: [u32; 6] = [0xc000, 0x14000, 0x24000, 0x18000, 0x28000, 0x30000];

/// The database's move kinds. The game's set accessor uses a different order:
/// dunk = 0, trick = 1, shot = 2.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Shot,
    Dunk,
    Trick,
}

pub const MOVE_SLOTS: [usize; 3] = [6, 11, 11];
pub const SHOTS_SET: [u16; 6] = [4, 7, 5, 6, 3, 1];
pub const DUNKS_SET: [u16; 11] = [48, 21, 0, 35, 58, 20, 23, 2, 15, 37, 39];
pub const TRICKS_SET: [u16; 11] = [20, 17, 19, 22, 12, 16, 56, 57, 63, 66, 69];
