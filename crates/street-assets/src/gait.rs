//! Ordinary gait data constructed by the recovered US executable.
//!
//! The table is not present in the ELF: `163d40` builds 108 forty-byte
//! records through `191410` on its first call. This reader evaluates only
//! that straight-line initializer, with native time base `+28 = 60.0`.
//! It does not execute actor scripts or select a runtime gait.
//!
//! Truncating binary32 multiplication matches all 1,080 retained original
//! table words. Nearest and truncating division produce identical table
//! words for these inputs, so the original division mode remains unproved.
//! Both division paths are checked; no other time base is supported.

use crate::{Error, Result, iso::Iso, scripts::Executable};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek};

/// The exact recovered `SLUS_206.51` program. Other or patched programs fail.
pub const SOURCE_SHA256: [u8; 32] = [
    0x9f, 0x2c, 0xf2, 0x37, 0xda, 0x7b, 0xd7, 0x8d, 0xad, 0xa3, 0x37, 0xa2, 0x93, 0x9f, 0x38, 0x78,
    0xc0, 0x5d, 0xdb, 0xe3, 0x95, 0x88, 0x57, 0x0b, 0x53, 0x28, 0x15, 0x4c, 0xbe, 0x63, 0x76, 0x14,
];
pub const TIME_BASE_HZ: u32 = 60;
pub const RECORD_COUNT: usize = 108;
pub const TABLE_ADDRESS: u32 = 0x4b8790;
const INIT_START: u32 = 0x163e20;
const INIT_END: u32 = 0x166f60;
const RECORD_CONSTRUCTOR: u32 = 0x191410;
const TIME_GETTER: u32 = 0x30fa50;
const TIME_OBJECT: u32 = 0x4da218;
const CLASS_TABLE: u32 = 0x43f0e8;
const CLASS_COUNT: usize = 144;
const PHASE_TABLE: u32 = 0x3d9290;
const PHASE_COUNT: usize = 158;

/// One native record, retaining the actual ten words and their producer.
#[derive(Clone, Debug, PartialEq)]
pub struct GaitRecord {
    pub address: u32,
    pub constructor_call: u32,
    pub script: u32,
    pub rows: [u16; 2],
    pub category_mask: u32,
    pub player_restriction: i32,
    pub minimum_handles: i32,
    pub root_distance: f32,
    pub minimum_motion: f32,
    pub maximum_motion: f32,
    pub flags: u32,
}

impl GaitRecord {
    pub fn words(&self) -> [u32; 10] {
        [
            self.script,
            u32::from(self.rows[0]),
            u32::from(self.rows[1]),
            self.category_mask,
            self.player_restriction as u32,
            self.minimum_handles as u32,
            self.root_distance.to_bits(),
            self.minimum_motion.to_bits(),
            self.maximum_motion.to_bits(),
            self.flags,
        ]
    }
}

/// Callback 52 searches these ordered `{row, flags}` pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowClass {
    pub row: u16,
    pub flags: u32,
}

/// Immutable, source-derived ordinary metadata, not mutable actor state.
#[derive(Clone, Debug, PartialEq)]
pub struct GaitData {
    pub records: Vec<GaitRecord>,
    pub row_classes: Vec<RowClass>,
    /// Callback 15's ordered membership table; duplicates are preserved.
    pub phase_rows: Vec<u16>,
}

impl GaitData {
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        Self::from_executable(&iso.read("/SLUS_206.51")?)
    }

    pub fn from_executable(data: &[u8]) -> Result<Self> {
        let identity: [u8; 32] = Sha256::digest(data).into();
        if identity != SOURCE_SHA256 {
            return Err(Error::format(
                "ordinary gait data requires the recovered US SLUS_206.51 executable",
            ));
        }
        let elf = Executable::new(data)?;
        validate_record_constructor(&elf)?;
        let records = construct(&elf, Rounding::Chop, Rounding::Nearest)?;
        let alternative = construct(&elf, Rounding::Chop, Rounding::Chop)?;
        if records
            .iter()
            .zip(&alternative)
            .any(|(a, b)| a.words() != b.words())
        {
            return Err(Error::format(
                "ordinary gait division choices differ outside the recovered 60 Hz domain",
            ));
        }
        let row_classes = (0..CLASS_COUNT)
            .map(|i| {
                Ok(RowClass {
                    row: row(elf.word(CLASS_TABLE + i as u32 * 8)?)?,
                    flags: elf.word(CLASS_TABLE + i as u32 * 8 + 4)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let phase_rows = (0..PHASE_COUNT)
            .map(|i| row(elf.word(PHASE_TABLE + i as u32 * 4)?))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            records,
            row_classes,
            phase_rows,
        })
    }

    /// The native search uses the first matching pair, without merging flags.
    pub fn row_flags(&self, row: u16) -> Option<u32> {
        self.row_classes
            .iter()
            .find(|entry| entry.row == row)
            .map(|entry| entry.flags)
    }

    pub fn phase_member(&self, row: u16) -> bool {
        self.phase_rows.contains(&row)
    }
}

fn row(value: u32) -> Result<u16> {
    if value < 1144 {
        Ok(value as u16)
    } else {
        Err(Error::format("ordinary gait animation row is out of range"))
    }
}

fn store(op: u32, base: u32, value: u32, offset: u32) -> u32 {
    op << 26 | base << 21 | value << 16 | offset
}

/// Verify the callee's actual stores, return value and JR delay-slot store.
fn validate_record_constructor(elf: &Executable<'_>) -> Result<()> {
    let expected = [
        store(0x2b, 4, 5, 0),
        4 << 21 | 2 << 11 | 0x2d, // move v0,a0
        store(0x2b, 4, 6, 4),
        store(0x2b, 4, 7, 8),
        store(0x2b, 4, 8, 12),
        store(0x2b, 4, 9, 16),
        store(0x2b, 4, 10, 20),
        store(0x39, 4, 12, 24),
        store(0x39, 4, 13, 28),
        store(0x39, 4, 14, 32),
        31 << 21 | 8, // jr ra
        store(0x2b, 4, 11, 36),
    ];
    for (i, word) in expected.into_iter().enumerate() {
        if elf.word(RECORD_CONSTRUCTOR + i as u32 * 4)? != word {
            return Err(Error::format("ordinary gait record constructor differs"));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Rounding {
    Nearest,
    Chop,
}

fn rounded(value: f64, mode: Rounding) -> Result<u32> {
    let nearest = value as f32;
    if !value.is_finite() || !nearest.is_finite() {
        return Err(Error::format("nonfinite ordinary gait arithmetic"));
    }
    let bits = nearest.to_bits();
    Ok(
        if matches!(mode, Rounding::Chop) && f64::from(nearest).abs() > value.abs() {
            bits - 1
        } else {
            bits
        },
    )
}

struct Constructor {
    integers: [Option<u32>; 32],
    floats: [Option<u32>; 32],
    multiply: Rounding,
    divide: Rounding,
}

impl Constructor {
    fn integer(&self, register: usize) -> Result<u32> {
        self.integers[register]
            .ok_or_else(|| Error::format("ordinary gait constructor reads an undefined register"))
    }

    fn float(&self, register: usize) -> Result<f64> {
        self.floats[register]
            .map(|bits| f64::from(f32::from_bits(bits)))
            .ok_or_else(|| Error::format("ordinary gait constructor reads an undefined float"))
    }

    /// Only the original initializer's straight-line instruction forms.
    /// JAL and its delay slot are handled separately; no branches are accepted.
    fn instruction(&mut self, address: u32, word: u32) -> Result<()> {
        let op = word >> 26;
        let rs = ((word >> 21) & 31) as usize;
        let rt = ((word >> 16) & 31) as usize;
        let rd = ((word >> 11) & 31) as usize;
        let fd = ((word >> 6) & 31) as usize;
        let immediate = word & 0xffff;
        let signed = i32::from(immediate as i16) as u32;
        match op {
            0 if word == 0 => {}
            0 if word & 63 == 0x2d && rt == 0 && word & 0x7c0 == 0 => {
                self.integers[rd] = Some(self.integer(rs)?);
            }
            0x0f if rs == 0 => self.integers[rt] = Some(immediate << 16),
            0x09 => self.integers[rt] = Some(self.integer(rs)?.wrapping_add(signed)),
            0x0d => self.integers[rt] = Some(self.integer(rs)? | immediate),
            0x11 if rs == 4 && word & 0x7ff == 0 => {
                self.floats[rd] = Some(self.integer(rt)?);
            }
            0x11 if rs == 16 && word & 63 == 6 && rt == 0 => {
                self.floats[fd] = self.floats[rd];
                self.float(fd)?;
            }
            0x11 if rs == 16 && matches!(word & 63, 2 | 3) => {
                let (left, right) = (self.float(rd)?, self.float(rt)?);
                let (value, mode) = if word & 63 == 2 {
                    (left * right, self.multiply)
                } else {
                    if right == 0.0 {
                        return Err(Error::format("ordinary gait constructor divides by zero"));
                    }
                    (left / right, self.divide)
                };
                self.floats[fd] = Some(rounded(value, mode)?);
            }
            0x31 if self.integer(rs)?.wrapping_add(signed) == TIME_OBJECT + 0x28 => {
                self.floats[rt] = Some((TIME_BASE_HZ as f32).to_bits());
            }
            0x2b if address == INIT_END - 4
                && self.integer(rs)?.wrapping_add(signed) == 0x3d78e8
                && self.integer(rt)? == 1 => {}
            _ => {
                return Err(Error::format(format!(
                    "unsupported ordinary gait constructor instruction at {address:#x}"
                )));
            }
        }
        // The source never writes zero, but preserve the register's invariant.
        self.integers[0] = Some(0);
        Ok(())
    }

    fn record(&self, index: usize, call: u32) -> Result<GaitRecord> {
        let address = self.integer(4)?;
        if address != TABLE_ADDRESS + index as u32 * 40 {
            return Err(Error::format(
                "ordinary gait record address sequence differs",
            ));
        }
        let float = |register: usize| {
            self.floats[register]
                .map(f32::from_bits)
                .ok_or_else(|| Error::format("ordinary gait record reads an undefined float"))
        };
        Ok(GaitRecord {
            address,
            constructor_call: call,
            script: self.integer(5)?,
            rows: [row(self.integer(6)?)?, row(self.integer(7)?)?],
            category_mask: self.integer(8)?,
            player_restriction: self.integer(9)? as i32,
            minimum_handles: self.integer(10)? as i32,
            root_distance: float(12)?,
            minimum_motion: float(13)?,
            maximum_motion: float(14)?,
            flags: self.integer(11)?,
        })
    }
}

fn construct(
    elf: &Executable<'_>,
    multiply: Rounding,
    divide: Rounding,
) -> Result<Vec<GaitRecord>> {
    let mut constructor = Constructor {
        integers: [None; 32],
        floats: [None; 32],
        multiply,
        divide,
    };
    constructor.integers[0] = Some(0);
    // Entry 163e1c copies the 3d0000 global base into s5.
    constructor.integers[21] = Some(0x3d0000);
    let mut records = Vec::with_capacity(RECORD_COUNT);
    let mut pc = INIT_START;
    while pc < INIT_END {
        let word = elf.word(pc)?;
        if word >> 26 == 3 {
            if pc + 4 >= INIT_END {
                return Err(Error::format(
                    "ordinary gait constructor lacks a delay slot",
                ));
            }
            let target = ((pc + 4) & 0xf0000000) | (word & 0x3ffffff) << 2;
            constructor.instruction(pc + 4, elf.word(pc + 4)?)?;
            match target {
                TIME_GETTER => constructor.integers[2] = Some(TIME_OBJECT),
                RECORD_CONSTRUCTOR if records.len() < RECORD_COUNT => {
                    records.push(constructor.record(records.len(), pc)?);
                    constructor.integers[2] = Some(constructor.integer(4)?);
                }
                _ => return Err(Error::format("unsupported ordinary gait constructor call")),
            }
            pc += 8;
        } else {
            constructor.instruction(pc, word)?;
            pc += 4;
        }
    }
    if records.len() != RECORD_COUNT {
        return Err(Error::format(
            "ordinary gait constructor record count differs",
        ));
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executable() -> Option<Vec<u8>> {
        let path = std::env::var_os("STREET_ISO")?;
        Some(Iso::open(path).unwrap().read("/SLUS_206.51").unwrap())
    }

    fn replace(data: &mut [u8], address: u32, value: u32) {
        // Keep the mutation helper independent of the production address map.
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let table = word(28) as usize;
        let stride = u16::from_le_bytes(data[42..44].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(data[44..46].try_into().unwrap()) as usize;
        let offset = (0..count)
            .find_map(|i| {
                let at = table + i * stride;
                let delta = address.checked_sub(word(at + 8))?;
                (word(at) == 1 && delta + 4 <= word(at + 16))
                    .then(|| word(at + 4) as usize + delta as usize)
            })
            .unwrap();
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn native_initializer_distinguishes_multiply_but_not_divide_rounding() {
        let Some(data) = executable() else { return };
        let elf = Executable::new(&data).unwrap();
        let truncated = construct(&elf, Rounding::Chop, Rounding::Nearest).unwrap();
        let all_truncated = construct(&elf, Rounding::Chop, Rounding::Chop).unwrap();
        let nearest = construct(&elf, Rounding::Nearest, Rounding::Nearest).unwrap();
        assert_eq!(truncated, all_truncated);
        let differences: Vec<_> = truncated
            .iter()
            .zip(nearest)
            .flat_map(|(a, b)| a.words().into_iter().zip(b.words()))
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(differences.len(), 93);
        assert!(differences.iter().all(|(a, b)| a.abs_diff(*b) == 1));
    }

    #[test]
    fn native_initializer_rejects_unknown_calls_stores_and_instruction_forms() {
        let Some(data) = executable() else { return };
        let mut changed = data.clone();
        // The ten-field callee's last word is the JR delay-slot store.
        replace(
            &mut changed,
            RECORD_CONSTRUCTOR + 44,
            store(0x2b, 4, 11, 32),
        );
        assert!(validate_record_constructor(&Executable::new(&changed).unwrap()).is_err());
        for (address, instruction) in [
            (INIT_START, 0x1000_0000),                  // unsupported branch
            (0x166dd4, (3 << 26) | (0x191414 >> 2)),    // unproved leaf
            (INIT_END - 4, store(0x2b, 21, 2, 0x78ec)), // different final store
        ] {
            changed.clone_from(&data);
            replace(&mut changed, address, instruction);
            assert!(
                construct(
                    &Executable::new(&changed).unwrap(),
                    Rounding::Chop,
                    Rounding::Nearest
                )
                .is_err()
            );
        }
    }

    #[test]
    fn constructor_rejects_undefined_registers_and_other_time_inputs() {
        let mut state = Constructor {
            integers: [None; 32],
            floats: [None; 32],
            multiply: Rounding::Chop,
            divide: Rounding::Nearest,
        };
        state.integers[0] = Some(0);
        // No caller-owned registers are supplied as arbitrary initializer seeds.
        assert!(
            state
                .instruction(INIT_START, 4 << 21 | 2 << 11 | 0x2d)
                .is_err()
        );
        state.integers[2] = Some(TIME_OBJECT);
        assert!(
            state
                .instruction(INIT_START, store(0x31, 2, 0, 0x2c))
                .is_err()
        );
        state
            .instruction(INIT_START, store(0x31, 2, 0, 0x28))
            .unwrap();
        assert_eq!(state.floats[0], Some(60f32.to_bits()));
        state.floats[1] = Some(0f32.to_bits());
        assert!(
            state
                .instruction(
                    INIT_START,
                    (0x11 << 26) | (16 << 21) | (1 << 16) | (2 << 6) | 3
                )
                .is_err()
        );
        assert!(state.record(0, INIT_START).is_err());
    }
}
