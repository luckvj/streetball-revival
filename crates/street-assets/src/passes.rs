//! The pass table at `0x3d65e8` in `SLUS_206.51`: 93 records of 28 bytes,
//! kept word for word. `0x160f28..0x160f7c` indexes it with stride 0x1c,
//! tests flag bit 14 (stored to `0x3d65a4`) and bit 8, and copies `+0x18`
//! to the passer's `+0x268`. Which row a pass picks is not recovered yet,
//! so nothing here names a row for a pass style.

use crate::{Result, scripts::Executable};

/// The table's address in the executable.
pub const PASS_TABLE: u32 = 0x3d65e8;
/// Records in the table.
pub const PASS_RECORDS: usize = 93;
/// Bytes per record (`addiu $v0, $zero, 0x1c` at `0x160f28`).
pub const PASS_STRIDE: u32 = 0x1c;

/// One record as the executable holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassRecord {
    /// +0x00: a move-script address.
    pub script: u32,
    /// +0x04 and +0x08: -1 where unused (their consumers are untraced).
    pub word_04: i32,
    pub word_08: i32,
    /// +0x0c and +0x10: floats (-1.0 where unused; their consumers are
    /// untraced, so they are not named speed or height here).
    pub float_0c: f32,
    pub float_10: f32,
    /// +0x14: flags; bit 14 and bit 8 are tested at `0x160f50..0x160f84`.
    pub flags: u32,
    /// +0x18: copied to the passer's +0x268 at `0x160f78`.
    pub kind: u32,
}

/// Reads the whole table from the executable's bytes.
pub fn pass_records(data: &[u8]) -> Result<Vec<PassRecord>> {
    let elf = Executable::new(data)?;
    (0..PASS_RECORDS as u32)
        .map(|row| {
            let at = PASS_TABLE + row * PASS_STRIDE;
            let w = |k: u32| elf.word(at + k * 4);
            Ok(PassRecord {
                script: w(0)?,
                word_04: w(1)? as i32,
                word_08: w(2)? as i32,
                float_0c: f32::from_bits(w(3)?),
                float_10: f32::from_bits(w(4)?),
                flags: w(5)?,
                kind: w(6)?,
            })
        })
        .collect()
}
