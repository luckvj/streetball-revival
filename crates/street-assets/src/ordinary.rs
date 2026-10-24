//! Immutable ordinary actor script prerequisites from the recovered US ELF.
//!
//! Roots are the actual idle/placement scripts and the constructed ordinary
//! gait records. The static closure follows original branches and fallthrough,
//! including jumps into the middle of a registered body. Callback identities
//! are retained without evaluating them or inventing their dynamic targets.
//! This module does not initialize an actor or execute the animation VM.

use crate::{Error, Result, gait, iso::Iso, scripts::Executable};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek},
};

pub const IDLE_SCRIPT: u32 = 0x4574fa;
pub const PLACEMENT_SCRIPT: u32 = 0x4574e2;
const REGISTRY: u32 = 0x3e7240;
const REGISTRY_COUNT: u32 = 632;
const CALLBACK_TABLE: u32 = 0x3e86d0;
const ANIMATION_COUNT: u32 = 1144;

/// The native idle row/rate pair written at `table_address` by 178ca8..178e10.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdleRecord {
    pub table_address: u32,
    pub row_store: u32,
    pub rate_store: u32,
    pub row: u16,
    pub rate: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Idle,
    Placement,
    Gait(u16),
}

/// Source references are kept even when several records use the same script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub address: u32,
}

/// Exact executable bytes in one registry-bounded body, including unreachable
/// bytes. Only `instructions` defines the admitted static instruction starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The callback's original table identity and operands, without execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Callback {
    pub id: u32,
    pub arguments: [u32; 3],
    pub native_function: u32,
}

/// Static operand bounds and possible control flow. Conditional callback
/// outcomes remain unresolved; both original paths are retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub address: u32,
    pub opcode: u8,
    pub next: u32,
    pub fallthrough: bool,
    pub branches: Vec<u32>,
    pub callback: Option<Callback>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrdinaryPrograms {
    pub idle: [IdleRecord; 5],
    pub entries: Vec<Entry>,
    pub bodies: Vec<Body>,
    pub instructions: Vec<Instruction>,
    /// Literal/variable row candidates. Resolve their names, mirror flags,
    /// frames and states through the existing animation registry/decoder.
    pub animation_rows: Vec<u16>,
}

impl OrdinaryPrograms {
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        Self::from_executable(&iso.read("/SLUS_206.51")?)
    }

    pub fn from_executable(data: &[u8]) -> Result<Self> {
        // This also verifies exact supported source identity before any read.
        let gait = gait::GaitData::from_executable(data)?;
        let elf = Executable::new(data)?;
        let idle = idle_records(&elf)?;
        let mut entries = vec![
            Entry {
                kind: EntryKind::Idle,
                address: IDLE_SCRIPT,
            },
            Entry {
                kind: EntryKind::Placement,
                address: PLACEMENT_SCRIPT,
            },
        ];
        entries.extend(gait.records.iter().enumerate().map(|(i, record)| Entry {
            kind: EntryKind::Gait(i as u16),
            address: record.script,
        }));
        let mut rows: BTreeSet<u16> = gait.records.iter().flat_map(|r| r.rows).collect();
        rows.extend(idle.iter().map(|r| r.row));
        let registry = registry(&elf)?;
        let (bodies, instructions) = closure(&elf, &registry, &entries, &mut rows)?;
        Ok(Self {
            idle,
            entries,
            bodies,
            instructions,
            animation_rows: rows.into_iter().collect(),
        })
    }

    pub fn instruction_at(&self, address: u32) -> Option<&Instruction> {
        self.instructions
            .binary_search_by_key(&address, |i| i.address)
            .ok()
            .map(|i| &self.instructions[i])
    }

    pub fn body_containing(&self, address: u32) -> Option<&Body> {
        self.bodies
            .iter()
            .find(|body| body.start <= address && address < body.end)
    }
}

fn registry(elf: &Executable<'_>) -> Result<BTreeMap<u32, String>> {
    let mut records = BTreeMap::new();
    for i in 0..REGISTRY_COUNT {
        let address = elf.word(REGISTRY + i * 8)?;
        let name = elf.string(elf.word(REGISTRY + i * 8 + 4)?)?;
        records.entry(address).or_insert(name);
    }
    Ok(records)
}

fn closure(
    elf: &Executable<'_>,
    registry: &BTreeMap<u32, String>,
    entries: &[Entry],
    rows: &mut BTreeSet<u16>,
) -> Result<(Vec<Body>, Vec<Instruction>)> {
    let mut work: BTreeSet<_> = entries.iter().map(|entry| entry.address).collect();
    for entry in entries {
        if !registry.contains_key(&entry.address) {
            return Err(Error::format(
                "ordinary root is absent from the source registry",
            ));
        }
    }
    let mut instructions: BTreeMap<u32, Instruction> = BTreeMap::new();
    let mut selected = BTreeSet::new();
    while let Some(pc) = work.pop_first() {
        if instructions.contains_key(&pc) {
            continue;
        }
        let (&start, _) = registry
            .range(..=pc)
            .next_back()
            .ok_or_else(|| Error::format("ordinary branch precedes the source registry"))?;
        let (&end, _) = registry
            .range((start + 1)..)
            .next()
            .ok_or_else(|| Error::format("ordinary body has no next registry boundary"))?;
        let decoded = crate::scripts::decode(elf, &[], pc)?;
        if decoded.next > end {
            return Err(Error::format(
                "ordinary operand crosses a source body boundary",
            ));
        }
        if instructions
            .range(..pc)
            .next_back()
            .is_some_and(|(_, i)| i.next > pc)
            || instructions
                .range(pc..)
                .next()
                .is_some_and(|(&next, _)| next < decoded.next)
        {
            return Err(Error::format(
                "ordinary branch overlaps an instruction operand",
            ));
        }
        let opcode = elf.byte(pc)?;
        let callback = if matches!(opcode, 0x83..=0x85 | 0x87) {
            let id = elf.word(pc + 1)?;
            let at = id
                .checked_mul(4)
                .and_then(|offset| CALLBACK_TABLE.checked_add(offset))
                .ok_or_else(|| Error::format("ordinary callback table address overflow"))?;
            Some(Callback {
                id,
                arguments: [elf.word(pc + 5)?, elf.word(pc + 9)?, elf.word(pc + 13)?],
                native_function: elf.word(at)?,
            })
        } else {
            None
        };
        for target in decoded.targets {
            if target.record >= 0 {
                if target.record as u32 >= ANIMATION_COUNT {
                    return Err(Error::format(
                        "ordinary literal animation row is out of range",
                    ));
                }
                rows.insert(target.record as u16);
            }
        }
        work.extend(decoded.branches.iter().copied());
        if decoded.fallthrough {
            work.insert(decoded.next);
        }
        selected.insert(start);
        instructions.insert(
            pc,
            Instruction {
                address: pc,
                opcode,
                next: decoded.next,
                fallthrough: decoded.fallthrough,
                branches: decoded.branches,
                callback,
            },
        );
        if instructions.len() > 16_384 {
            return Err(Error::format("ordinary static instruction limit"));
        }
    }
    let bodies = selected
        .into_iter()
        .map(|start| {
            let (&end, _) = registry.range((start + 1)..).next().unwrap();
            Ok(Body {
                start,
                end,
                name: registry[&start].clone(),
                bytes: elf.bytes(start, (end - start) as usize)?.to_vec(),
            })
        })
        .collect::<Result<_>>()?;
    Ok((bodies, instructions.into_values().collect()))
}

fn word(op: u32, base: u32, value: u32, immediate: u32) -> u32 {
    op << 26 | base << 21 | value << 16 | immediate
}

fn expect(elf: &Executable<'_>, pc: u32, expected: u32) -> Result<()> {
    if elf.word(pc)? != expected {
        return Err(Error::format(format!(
            "ordinary idle initializer differs at {pc:#x}"
        )));
    }
    Ok(())
}

fn float_literal(elf: &Executable<'_>, pc: u32, register: u32) -> Result<f32> {
    let load = elf.word(pc)?;
    if load >> 26 != 15 || load >> 21 & 31 != 0 {
        return Err(Error::format(
            "ordinary idle float producer is not a literal LUI",
        ));
    }
    let integer = load >> 16 & 31;
    expect(
        elf,
        pc + 4,
        0x11 << 26 | 4 << 21 | integer << 16 | register << 11,
    )?;
    let value = f32::from_bits((load & 0xffff) << 16);
    if !value.is_finite() || value <= 0.0 {
        return Err(Error::format("ordinary idle rate literal is invalid"));
    }
    Ok(value)
}

/// Read source literals and validate their specific FP/store/copy producers.
/// No stack, heap, general memory interpreter or captured row is supplied.
fn idle_records(elf: &Executable<'_>) -> Result<[IdleRecord; 5]> {
    let divisor = float_literal(elf, 0x178b9c, 22)?;
    let common_rate = float_literal(elf, 0x178a64, 23)?;
    if divisor != gait::TIME_BASE_HZ as f32 {
        return Err(Error::format(
            "ordinary idle requires the recovered 60 Hz time base",
        ));
    }
    let base_load = elf.word(0x178cbc)?;
    if base_load >> 26 != 15 || base_load >> 16 & 31 != 2 || base_load >> 21 & 31 != 0 {
        return Err(Error::format("ordinary idle table base producer differs"));
    }
    let base_low = elf.word(0x178cd0)?;
    if base_low >> 26 != 9 || base_low >> 21 & 31 != 2 || base_low >> 16 & 31 != 9 {
        return Err(Error::format("ordinary idle table base offset differs"));
    }
    let base = ((base_load & 0xffff) << 16).wrapping_add(i32::from(base_low as i16) as u32);
    // The cursor starts at the source table, advances once before this block,
    // then advances between each later eight-byte record.
    expect(elf, 0x178bc0, word(15, 0, 5, base_load & 0xffff))?;
    expect(elf, 0x178bc8, word(9, 5, 5, base_low & 0xffff))?;
    expect(elf, 0x178bd0, word(0x2b, 29, 5, 0x914))?;
    expect(elf, 0x178bf4, word(0x23, 29, 6, 0x914))?;
    expect(elf, 0x178c04, word(9, 6, 6, 8))?;
    expect(elf, 0x178c10, word(0x2b, 29, 6, 0x914))?;
    for (load, advance, save, register) in [
        (0x178d34, 0x178d40, 0x178d48, 3),
        (0x178d88, 0x178d94, 0x178d9c, 4),
        (0x178dd4, 0x178de0, 0x178de8, 4),
    ] {
        expect(elf, load, word(0x23, 29, register, 0x914))?;
        expect(elf, advance, word(9, register, register, 8))?;
        expect(elf, save, word(0x2b, 29, register, 0x914))?;
    }
    for (pc, register) in [(0x178d10, 3), (0x178d64, 7), (0x178da8, 7), (0x178df0, 5)] {
        expect(elf, pc, word(0x23, 29, register, 0x914))?;
    }
    // Each tuple identifies source producers, not copied record values.
    let sources = [
        (
            0x178ca8, 0x178cb0, 0x178cac, 0x178cb4, 0x178cc4, 0x178cc8, 0x178ccc, None, 23,
            0x178cd4, 0x178cdc, 0x178ce0, 3, 9,
        ),
        (
            0x178cec,
            0x178cf4,
            0x178cf0,
            0x178cf8,
            0x178d0c,
            0x178d18,
            0x178d1c,
            Some(0x178d00),
            1,
            0x178d20,
            0x178d28,
            0x178d30,
            7,
            3,
        ),
        (
            0x178d38,
            0x178d3c,
            0x178d44,
            0x178d4c,
            0x178d60,
            0x178d6c,
            0x178d70,
            Some(0x178d54),
            1,
            0x178d74,
            0x178d7c,
            0x178d84,
            8,
            7,
        ),
        (
            0x178d8c, 0x178d90, 0x178d98, 0x178da0, 0x178db0, 0x178db8, 0x178dbc, None, 23,
            0x178dc0, 0x178dc8, 0x178dd0, 8, 7,
        ),
        (
            0x178dd8, 0x178ddc, 0x178de4, 0x178dec, 0x178df4, 0x178df8, 0x178dfc, None, 23,
            0x178e00, 0x178e08, 0x178e10, 6, 5,
        ),
    ];
    let mut records = Vec::new();
    for (
        i,
        (
            literal,
            row_store,
            getter,
            time_load,
            divide,
            multiply,
            rate_store,
            rate_literal,
            rate_register,
            copy_load,
            copy_high,
            copy_low,
            copy_register,
            destination,
        ),
    ) in sources.into_iter().enumerate()
    {
        let row_load = elf.word(literal)?;
        if row_load >> 26 != 9 || row_load >> 21 & 31 != 0 {
            return Err(Error::format("ordinary idle row producer differs"));
        }
        let row = i32::from(row_load as i16);
        if !(0..ANIMATION_COUNT as i32).contains(&row) {
            return Err(Error::format("ordinary idle row is out of range"));
        }
        expect(elf, row_store, word(0x2b, 29, row_load >> 16 & 31, 0))?;
        expect(elf, getter, 3 << 26 | 0x30fa50 >> 2)?;
        expect(elf, time_load, word(0x31, 2, 0, 0x28))?;
        expect(elf, divide, 0x11 << 26 | 16 << 21 | 22 << 16 | 3)?;
        expect(
            elf,
            multiply,
            0x11 << 26 | 16 << 21 | rate_register << 16 | 2,
        )?;
        expect(elf, rate_store, word(0x39, 29, 0, 4))?;
        expect(elf, copy_load, word(0x1a, 29, copy_register, 7))?;
        expect(elf, copy_load + 4, word(0x1b, 29, copy_register, 0))?;
        expect(elf, copy_high, word(0x2c, destination, copy_register, 7))?;
        expect(elf, copy_low, word(0x2d, destination, copy_register, 0))?;
        let factor = rate_literal
            .map(|pc| float_literal(elf, pc, rate_register))
            .transpose()?
            .unwrap_or(common_rate);
        let divided = f64::from(gait::TIME_BASE_HZ) / f64::from(divisor);
        let product = divided * f64::from(factor);
        if f64::from(divided as f32) != divided || f64::from(product as f32) != product {
            return Err(Error::format(
                "ordinary idle rate requires an unproved rounding choice",
            ));
        }
        records.push(IdleRecord {
            table_address: base + i as u32 * 8,
            row_store,
            rate_store,
            row: row as u16,
            rate: product as f32,
        });
    }
    Ok(records.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executable(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0; 84 + payload.len()];
        data[..6].copy_from_slice(b"\x7fELF\x01\x01");
        data[18..20].copy_from_slice(&8u16.to_le_bytes());
        data[28..32].copy_from_slice(&52u32.to_le_bytes());
        data[42..44].copy_from_slice(&32u16.to_le_bytes());
        data[44..46].copy_from_slice(&1u16.to_le_bytes());
        data[52..56].copy_from_slice(&1u32.to_le_bytes());
        data[56..60].copy_from_slice(&84u32.to_le_bytes());
        data[60..64].copy_from_slice(&0x1000u32.to_le_bytes());
        data[68..72].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        data[84..].copy_from_slice(payload);
        data
    }

    #[test]
    fn closure_follows_real_fallthrough_across_a_registry_label() {
        let data = executable(&[1, 0x81]);
        let registry = BTreeMap::from([
            (0x1000, "first".into()),
            (0x1001, "second".into()),
            (0x1002, "end".into()),
        ]);
        let roots = [Entry {
            kind: EntryKind::Idle,
            address: 0x1000,
        }];
        let (bodies, instructions) = closure(
            &Executable::new(&data).unwrap(),
            &registry,
            &roots,
            &mut BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(bodies.len(), 2);
        assert_eq!(instructions.len(), 2);
        assert!(instructions[0].fallthrough);
        assert_eq!(instructions[0].next, instructions[1].address);
        assert!(!instructions[1].fallthrough);
    }

    #[test]
    fn closure_rejects_operand_entry_and_operand_crossing_body_boundary() {
        let roots = [Entry {
            kind: EntryKind::Idle,
            address: 0x1000,
        }];
        for (target, end) in [(0x1002u32, 0x1005), (0x1000u32, 0x1004)] {
            let mut payload = vec![0x82];
            payload.extend(target.to_le_bytes());
            let data = executable(&payload);
            let registry = BTreeMap::from([(0x1000, "first".into()), (end, "end".into())]);
            assert!(
                closure(
                    &Executable::new(&data).unwrap(),
                    &registry,
                    &roots,
                    &mut BTreeSet::new()
                )
                .is_err()
            );
        }
    }
}
