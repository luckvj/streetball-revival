//! The words the match HUD's pop-ups name moves by, read from the
//! executable: the overlay screen (`FUN_002b87c0`) indexes string-pointer
//! tables in `.data` by the scorer's own move ids.
//!
//! | Table | Count | Index |
//! |---|---|---|
//! | `0x3fb500` | 75 | the trick move, actor `+0x264` (the row of the trick table at `0x3d7234`, the simulation's trick script id) |
//! | `0x3fb630` | 60 | the dunk, actor `+0x1e8` (`FUN_0013a1f8`'s dunk id) |
//! | `0x3fb720` | 28 | the lay-up finish, actor `+0x26c` |
//! | `0x3fb790` | 4 | the passout special, the HUD's `+0x1e4` |
//! | `0x3fb7a0` | 4 | the alley-oop, `FUN_00171e28`'s style bits |

use std::io::{Read, Seek};

use crate::{Result, iso::Iso, scripts::Executable};

/// The pop-ups' move names, in the executable's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveNames {
    pub tricks: Vec<String>,
    pub dunks: Vec<String>,
    pub layups: Vec<String>,
    pub passouts: Vec<String>,
    pub alley_oops: Vec<String>,
}

impl MoveNames {
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        Self::from_executable(&iso.read("/SLUS_206.51")?)
    }

    pub fn from_executable(data: &[u8]) -> Result<Self> {
        let elf = Executable::new(data)?;
        let table = |address: u32, count: u32| {
            (0..count)
                .map(|k| elf.string(elf.word(address + 4 * k)?))
                .collect::<Result<Vec<_>>>()
        };
        Ok(Self {
            tricks: table(0x3fb500, 75)?,
            dunks: table(0x3fb630, 60)?,
            layups: table(0x3fb720, 28)?,
            passouts: table(0x3fb790, 4)?,
            alley_oops: table(0x3fb7a0, 4)?,
        })
    }
}
