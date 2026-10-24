//! Static move-script inspection from the user's US PS2 executable.
//!
//! Uses the ELF load segments, the 632-entry registry at `0x3e7240`, and
//! instruction operand lengths from the animation VM's 48 handlers at
//! `0x3e8610`. Registry starts bound bodies: bytes in neighboring scripts
//! are never mistaken for this script's blend commands. Conditional branches
//! inside a body are inspected on both paths. External jumps are retained
//! separately. This is clip discovery, not execution of game callbacks.

use crate::{Error, Result, iso::Iso, moves::Kind};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek},
};

/// One animation operand, including VM sentinels such as -1000 and -2000.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipTarget {
    pub record: i32,
    pub name: Option<String>,
    /// The animation record's final flag requests mirroring.
    pub mirrored: bool,
}

/// A single `0x8e` blend instruction, with every conditional alternate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blend {
    pub address: u32,
    pub selectors: Vec<u8>,
    pub frames: i8,
    pub start_frame: i32,
    /// Primary target followed by one alternate per selector.
    pub targets: Vec<ClipTarget>,
}

#[derive(Clone, Debug)]
pub struct MoveScript {
    pub name: String,
    pub address: u32,
    pub state_mask: u32,
    pub legend: Option<u16>,
    pub blends: Vec<Blend>,
    /// Distinct named targets of animation assignments and blends, in
    /// instruction address order. Alternatives remain adjacent to primaries.
    pub clip_targets: Vec<String>,
    pub external_jumps: Vec<u32>,
    /// Game-callback ids (`0x83` opcode, `{id, a, b, c}` through
    /// `0x3e86d0`) in instruction address order, deduplicated.
    pub callbacks: Vec<u32>,
    /// The `{a, b, c}` words per callback id, same order as `callbacks`.
    pub callback_args: Vec<[u32; 3]>,
}

/// All 27 shots, 60 dunk ids, and 75 internal trick ids (the shop uses 0..72).
/// Native dunk ids0..59 all have family0 candidates; family2 rows are lay-ups.
#[derive(Clone, Debug)]
pub struct MoveScripts {
    pub shots: Vec<MoveScript>,
    pub dunks: Vec<Vec<MoveScript>>,
    pub tricks: Vec<MoveScript>,
    /// Four inline callback-32 finishers for ids 48..51, then its fallback.
    pub layup_finishes: Vec<MoveScript>,
    pub default_shots: Vec<u16>,
    pub default_dunks: Vec<u16>,
    pub default_tricks: Vec<u16>,
    /// Each dunk id's flag word (`0x43f5f0`, read by `FUN_00171de8` through
    /// the dunk id at `player+0x1e8`); bit 0x800 marks an advanced dunk.
    pub dunk_flags: Vec<u32>,
    row_scripts: Vec<u16>,
    /// The fixed `XDBMOVE` rows of the generic team's players
    /// ([`GENERIC_TEAM`], `FUN_001d9e60`): shots at `0x45fde0` (6), dunks at
    /// `0x45fdf0` (11), dribble tricks at `0x45fe08` (11), u16 each, 0xffff
    /// for none.
    generic_rows: [Vec<u16>; 3],
}

/// A record's field 3 (its team) that `0x13a1f8` tests (`FUN_001da4e0(rec,
/// 3)`): the created players take their own equipped rows, the generic
/// streetballers the fixed rows of `FUN_001d9e60`.
pub const CREATED_TEAM: i16 = 0x25;
pub const GENERIC_TEAM: i16 = 0x24;

/// The flag bit that makes a dunk advanced in the release callback.
pub const ADVANCED_DUNK: u32 = 0x800;

/// The animation VM clock started by a staged opcode-1 finisher.
#[derive(Clone, Debug)]
pub struct Finisher {
    pub animation: u16,
    pub address: u32,
    pub spans: Vec<FinisherSpan>,
    /// Callback 21 calls the actual ball shot release (FUN_001729f8).
    pub release_tick: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct FinisherSpan {
    pub start_tick: u32,
    pub duration: u32,
    pub from_frame: i32,
    pub to_frame: i32,
}

/// Registry lookup used by FUN_00334178. Each 20-byte row is
/// {animation, body}; `spans` reads the body's `8d ee` waits (reach
/// animation frame B over C ticks) and callback 21 sets `release_tick`
/// — ticks after the opcode-1 start, as the game's registering VM runs
/// them. Only fixed positive waits before callback 21 are kept.
/// A self-toss trick script's own timing, in native updates, read from its
/// bytecode: the opening blend (`8e f8 <updates>`), the wait before
/// callback 53 (`8d ee <frame> <updates>`), the callback's three words
/// (`83 35 00 00 00`, then flight, sample frame, bounce) and the closing wait (the last
/// `8d ee <frame> <updates>` before the script's `9c` end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelfTossTiming {
    pub blend_updates: u8,
    pub pre_callback_wait_updates: u8,
    pub callback: [u32; 3],
    pub followthrough_updates: u8,
}

/// Reads [`SelfTossTiming`] from the script at `address`, or `None` when
/// it does not open with a blend and reach callback 53.
pub fn self_toss_timing(data: &[u8], address: u32) -> Result<Option<SelfTossTiming>> {
    let elf = Executable::new(data)?;
    let code = elf.bytes(address, 128)?;
    let find = |from: usize, pattern: &[u8]| {
        code.get(from..)?
            .windows(pattern.len())
            .position(|w| w == pattern)
            .map(|at| from + at)
    };
    let Some(blend) = find(0, &[0x8e, 0xf8]) else {
        return Ok(None);
    };
    let Some(call) = find(blend, &[0x83, 0x35]) else {
        return Ok(None);
    };
    let Some(wait) = code[..call].windows(2).rposition(|w| w == [0x8d, 0xee]) else {
        return Ok(None);
    };
    let word = |at: usize| u32::from_le_bytes(code[at..at + 4].try_into().unwrap());
    // `83 35`, three zero bytes, then the three little-endian words.
    let callback = [word(call + 5), word(call + 9), word(call + 13)];
    // The closing wait is the last fixed wait before `9c 00 06 00 00 91`.
    let Some(end) = find(call + 17, &[0x9c, 0x00, 0x06, 0x00, 0x00, 0x91]) else {
        return Ok(None);
    };
    let Some(close) = code[..end].windows(2).rposition(|w| w == [0x8d, 0xee]) else {
        return Ok(None);
    };
    Ok(Some(SelfTossTiming {
        blend_updates: code[blend + 2],
        pre_callback_wait_updates: code[wait + 3],
        callback,
        followthrough_updates: code[close + 3],
    }))
}

pub fn finishers_from_executable(data: &[u8]) -> Result<Vec<Finisher>> {
    let elf = Executable::new(data)?;
    let mut registry = BTreeMap::new();
    for i in 0..632 {
        registry
            .entry(elf.word(0x3e7240 + i * 8)?)
            .or_insert(elf.string(elf.word(0x3e7244 + i * 8)?)?);
    }
    let names = (0..1144)
        .map(|i| {
            let p = elf.word(0x3de8a0 + i * 28)?;
            Ok((
                if p == 0 { None } else { Some(elf.string(p)?) },
                elf.word(0x3de8b8 + i * 28)? != 0,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut result = Vec::new();
    for (table, count) in [(0x415740, 22), (0x414df0, 119)] {
        for i in 0..count {
            let animation = elf.word(table + i * 20)?;
            if !(900..1144).contains(&animation)
                || result
                    .iter()
                    .any(|f: &Finisher| u32::from(f.animation) == animation)
            {
                continue;
            }
            let address = elf.word(table + i * 20 + 4)?;
            let end = registry
                .range((address + 1)..)
                .next()
                .map(|(&a, _)| a)
                .ok_or_else(|| Error::format("finisher lacks registry boundary"))?;
            let mut pending = BTreeSet::from([address]);
            let mut visited = BTreeMap::new();
            while let Some(pc) = pending.pop_first() {
                if visited.contains_key(&pc) || pc < address || pc >= end {
                    continue;
                }
                let instruction = decode(&elf, &names, pc)?;
                if instruction.next > end {
                    return Err(Error::format("finisher operand exceeds body"));
                }
                pending.extend(instruction.branches.iter().copied());
                if instruction.fallthrough && instruction.next < end {
                    pending.insert(instruction.next);
                }
                visited.insert(pc, instruction);
            }
            let mut spans = Vec::new();
            let (mut tick, mut frame) = (0, 0);
            let mut release_tick = None;
            for (&pc, instruction) in &visited {
                let opcode = elf.byte(pc)?;
                if opcode == 0x83 && elf.word(pc + 1)? == 21 {
                    release_tick = Some(tick);
                    break;
                }
                if opcode == 0x8e
                    && let Some(blend) = &instruction.blend
                {
                    frame = blend.start_frame;
                }
                if opcode == 0x8d {
                    if elf.byte(pc + 1)? != 0xee || elf.byte(pc + 3)? as i8 <= 0 {
                        break;
                    }
                    let to_frame = i32::from(elf.byte(pc + 2)? as i8);
                    let duration = u32::from(elf.byte(pc + 3)?);
                    spans.push(FinisherSpan {
                        start_tick: tick,
                        duration,
                        from_frame: frame,
                        to_frame,
                    });
                    tick += duration;
                    frame = to_frame;
                }
            }
            result.push(Finisher {
                animation: animation as u16,
                address,
                spans,
                release_tick,
            });
        }
    }
    Ok(result)
}

impl MoveScripts {
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        Self::from_executable(&iso.read("/SLUS_206.51")?)
    }

    /// Parse the original executable; malformed or unsupported layouts fail
    /// explicitly, rather than silently inventing clip targets.
    pub fn from_executable(data: &[u8]) -> Result<Self> {
        let elf = Executable::new(data)?;
        let mut registry = BTreeMap::new();
        for i in 0..632 {
            let name = elf.string(elf.word(0x3e7244 + i * 8)?)?;
            let address = elf.word(0x3e7240 + i * 8)?;
            registry.entry(address).or_insert(name);
        }
        let mut names = Vec::with_capacity(1144);
        for i in 0..1144 {
            let address = elf.word(0x3de8a0 + i * 28)?;
            names.push((
                if address == 0 {
                    None
                } else {
                    Some(elf.string(address)?)
                },
                elf.word(0x3de8b8 + i * 28)? != 0,
            ));
        }
        let inspect = |address, state_mask, legend| {
            inspect_script(&elf, &registry, &names, address, state_mask, legend)
        };
        let shots = (0..27)
            .map(|i| inspect(elf.word(0x3d8fd8 + i * 4)?, 0, None))
            .collect::<Result<Vec<_>>>()?;
        let layup_finishes = [0x447ed5, 0x447c1d, 0x447d0d, 0x447de5, 0x447bbe]
            .into_iter()
            .map(|a| inspect(a, 0, None))
            .collect::<Result<Vec<_>>>()?;
        let mut dunks = vec![Vec::new(); 60];
        // Native1695bc/169994:99 stride20 candidates from3d7b28.
        // {body, legend, selection mask, family, id}; family0 is dunk.
        // The preceding word belongs to the previous row, not this id.
        for address in (0x3d7b28..0x3d82e4).step_by(20) {
            if elf.word(address + 12)? != 0 {
                continue;
            }
            let id = elf.word(address + 16)? as usize;
            if id >= 60 {
                return Err(Error::format("dunk script id out of range"));
            }
            let op = elf.word(address + 4)?;
            let legend = if op == u32::MAX {
                None
            } else {
                Some(op as u16)
            };
            let script = inspect(elf.word(address)?, elf.word(address + 8)?, legend)?;
            if !dunks[id].iter().any(|s: &MoveScript| {
                s.address == script.address
                    && s.legend == script.legend
                    && s.state_mask == script.state_mask
            }) {
                dunks[id].push(script);
            }
        }
        // `0x3d7238`, stride 12, indexed by script id: {body, legend,
        // selection flags} (`FUN_0015f0f0`'s draw reads the flags at +8).
        // Reading from `0x3d7234` took the previous row's flags.
        let tricks = (0..75)
            .map(|i| {
                let address = 0x3d7238 + i * 12;
                let op = elf.word(address + 4)?;
                inspect(
                    elf.word(address)?,
                    elf.word(address + 8)?,
                    if op == u32::MAX {
                        None
                    } else {
                        Some(op as u16)
                    },
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let slots = |address: u32, count: u32| {
            (0..count)
                .map(|i| {
                    let id = elf.word(address + i * 4)?;
                    u16::try_from(id).map_err(|_| Error::format("move script id exceeds u16"))
                })
                .collect::<Result<Vec<_>>>()
        };
        let halves = |address: u32, count: u32| {
            (0..count)
                .map(|i| {
                    let b = elf.bytes(address + 2 * i, 2)?;
                    Ok(u16::from_le_bytes([b[0], b[1]]))
                })
                .collect::<Result<Vec<_>>>()
        };
        Ok(Self {
            shots,
            dunks,
            tricks,
            layup_finishes,
            default_shots: slots(0x43d6c0, 6)?,
            default_dunks: slots(0x43d5d0, 11)?,
            default_tricks: slots(0x43d498, 11)?,
            dunk_flags: (0..60)
                .map(|i| elf.word(0x43f5f0 + i * 4))
                .collect::<Result<Vec<_>>>()?,
            // XDBMOVE has 93 rows. The following zero is a table sentinel.
            row_scripts: slots(0x45f9c8, 93)?,
            generic_rows: [
                halves(0x45fde0, 6)?,
                halves(0x45fdf0, 11)?,
                halves(0x45fe08, 11)?,
            ],
        })
    }

    /// Lookup by XDBMOVE row index, **not** the database's film-room move id.
    pub fn row_script(&self, row: usize) -> Option<u16> {
        self.row_scripts.get(row).copied()
    }

    pub fn script(&self, kind: Kind, id: usize) -> Option<&MoveScript> {
        match kind {
            Kind::Shot => self.shots.get(id),
            Kind::Trick => self.tricks.get(id),
            Kind::Dunk => self.dunks.get(id).and_then(|rows| {
                rows.iter()
                    .find(|s| s.legend.is_none())
                    .or_else(|| rows.first())
            }),
        }
    }

    /// The directly named script, except the bbpass_1 setup script whose
    /// callback 32 starts an inline finishing script (FUN_001773b0).
    /// General game callbacks are not executed by this static inspector.
    pub fn playback_script(&self, kind: Kind, id: usize) -> Option<&MoveScript> {
        let script = self.script(kind, id)?;
        if kind == Kind::Dunk && script.address == 0x447b53 {
            let index = if (48..=51).contains(&id) { id - 48 } else { 4 };
            self.layup_finishes.get(index)
        } else {
            Some(script)
        }
    }

    /// The script id a move slot plays (0x13a1f8 / 0x1d8778): the equipped
    /// `XDBMOVE` row's script, else the record's own slot-1 override
    /// ([`legend_slot_script`]), else the native default for the slot.
    ///
    /// `team` is the record's field 3 when known. `0x13a1f8` reads the
    /// equipped rows only for [`CREATED_TEAM`] (with no legend override),
    /// the fixed `FUN_001d9e60` rows for [`GENERIC_TEAM`], and only the
    /// legends' slot-1 override for everyone else; a row of 0xffff, or one
    /// with no script, falls through to the slot's default. An equipped row
    /// on any other team (`roster.json`'s rows) is the rewrite's own
    /// extension and is honoured first, as is any row when `team` is unknown.
    pub fn slot_script(
        &self,
        kind: Kind,
        record: usize,
        team: Option<i16>,
        slot: usize,
        equipped_row: Option<usize>,
    ) -> Option<u16> {
        let category = match kind {
            Kind::Shot => 0,
            Kind::Dunk => 1,
            Kind::Trick => 2,
        };
        let row = match team {
            Some(GENERIC_TEAM) => self.generic_rows[category]
                .get(slot)
                .filter(|&&row| row != u16::MAX)
                .map(|&row| usize::from(row)),
            _ => equipped_row,
        };
        if let Some(id) = row.and_then(|row| self.row_script(row)) {
            return Some(id);
        }
        if slot == 1
            && !matches!(team, Some(CREATED_TEAM | GENERIC_TEAM))
            && let Some(id) = legend_slot_script(kind, record)
        {
            return Some(id);
        }
        match kind {
            Kind::Shot => self.default_shots.get(slot),
            Kind::Dunk => self.default_dunks.get(slot),
            Kind::Trick => self.default_tricks.get(slot),
        }
        .copied()
    }

    pub fn default_script(&self, kind: Kind, slot: usize) -> Option<&MoveScript> {
        let ids = match kind {
            Kind::Shot => &self.default_shots,
            Kind::Dunk => &self.default_dunks,
            Kind::Trick => &self.default_tricks,
        };
        self.playback_script(kind, *ids.get(slot)? as usize)
    }
}

/// The legends' own second dunk or trick (0x13a1f8): slot 1 of these
/// records plays this script id when nothing is equipped there.
pub fn legend_slot_script(kind: Kind, record: usize) -> Option<u16> {
    match (kind, record) {
        (Kind::Trick, 0x91) => Some(0x3c),
        (Kind::Trick, 0x93) => Some(0x12),
        (Kind::Trick, 0x94) => Some(0x44),
        (Kind::Trick, 0x96) => Some(10),
        (Kind::Trick, 0xa6) => Some(0x36),
        (Kind::Trick, 0xa9) => Some(0x17),
        (Kind::Dunk, 0x91) => Some(0x35),
        (Kind::Dunk, 0x92) => Some(0x38),
        (Kind::Dunk, 0x9e) => Some(0x1b),
        (Kind::Dunk, 0x9f) => Some(0x2c),
        (Kind::Dunk, 0xa7 | 0xa8) => Some(0x2e),
        (Kind::Dunk, 0xad) => Some(0x2b),
        (Kind::Dunk, 0xaf) => Some(0x1d),
        (Kind::Dunk, 0xb2) => Some(0x2f),
        (Kind::Dunk, 0xb5) => Some(0x2d),
        _ => None,
    }
}

/// Walks a move script from `address` along the path the animation VM
/// (`FUN_00193440`) takes when every game callback returns 0: `0x82`,
/// `0x9a` and `0x87` jump, `0x85` jumps (its callback false), `0x84` falls
/// through, `0x98`/`0x99` fall through. Opcodes run back to back inside
/// one update; a blend (`8e`, its byte of frames) and a fixed wait (`8d`,
/// its byte of ticks, 1..127) carry the ticks. `stop` sees each
/// instruction's address, opcode, decoding and tick and ends the walk
/// with a value.
fn walk_script(
    elf: &Executable<'_>,
    address: u32,
    mut stop: impl FnMut(u32, u8, &Instruction, u32) -> Option<u32>,
) -> Result<Option<u32>> {
    let names: Vec<(Option<String>, bool)> = Vec::new();
    let mut pc = address;
    let mut tick = 0;
    let mut seen = BTreeSet::new();
    for _ in 0..600 {
        if !seen.insert(pc) || elf.byte(pc).is_err() {
            return Ok(None);
        }
        let op = elf.byte(pc)?;
        let ins = decode(elf, &names, pc)?;
        if let Some(v) = stop(pc, op, &ins, tick) {
            return Ok(Some(v));
        }
        if let Some(blend) = &ins.blend {
            tick += u32::from(blend.frames.max(0) as u8);
        }
        if op == 0x8d {
            let ticks = if elf.byte(pc + 1)? == 0xee {
                elf.byte(pc + 3)?
            } else {
                elf.byte(pc + 2)?
            };
            if (1..128).contains(&ticks) {
                tick += u32::from(ticks);
            }
        }
        pc = match op {
            0x82 | 0x9a | 0x87 | 0x85 => ins.branches[0],
            _ if !ins.fallthrough => return Ok(None),
            _ => ins.next,
        };
    }
    Ok(None)
}

/// Ticks from a script's start to its game callback `callback` (`83 id`)
/// on the plain path ([`walk_script`]), or `None` when the path has none.
pub fn script_callback_tick(data: &[u8], address: u32, callback: u32) -> Result<Option<u32>> {
    let elf = Executable::new(data)?;
    walk_script(&elf, address, |_, op, ins, tick| {
        (op == 0x83 && ins.callback == Some(callback)).then_some(tick)
    })
}

/// Ticks from a shot script's start to its conditional callback 20 or 17
/// (`84`/`85`: the pump fake and the stick fadeaway are decided there).
pub fn script_check_tick(data: &[u8], address: u32) -> Result<Option<u32>> {
    let elf = Executable::new(data)?;
    walk_script(&elf, address, |_, op, ins, tick| {
        (matches!(op, 0x84 | 0x85) && matches!(ins.callback, Some(20) | Some(17))).then_some(tick)
    })
}

/// Ticks from a script's start to its final `81` on the plain path.
pub fn script_end_tick(data: &[u8], address: u32) -> Result<Option<u32>> {
    let elf = Executable::new(data)?;
    walk_script(&elf, address, |_, op, _, tick| (op == 0x81).then_some(tick))
}

/// A catch script's tail: ticks from the end of its flight wait (`8d ee B
/// fd`, which lasts the rest of the pass's flight) to the `9c` that clears
/// bit 9 (0x200); `None` when the script plays its clip out first (`8c`),
/// whose length is not in the executable.
pub fn script_catch_tail(data: &[u8], address: u32) -> Result<Option<u32>> {
    let elf = Executable::new(data)?;
    let mut after: Option<u32> = None;
    let mut clip_out = false;
    let tail = walk_script(&elf, address, |pc, op, _, tick| {
        if op == 0x8d
            && elf.byte(pc + 1).is_ok_and(|b| b == 0xee)
            && elf.byte(pc + 3).is_ok_and(|b| b == 0xfd)
        {
            after = Some(tick);
        }
        if after.is_some() && op == 0x8c {
            clip_out = true;
            return Some(0);
        }
        if let Some(t0) = after
            && op == 0x9c
            && elf.word(pc + 1).is_ok_and(|w| w & 0x200 != 0)
        {
            return Some(tick - t0);
        }
        None
    })?;
    Ok(if clip_out { None } else { tail })
}

/// A shot script's release gate on the held path (`86 19`: the VM re-runs
/// the opcode every update until callback 19, `FUN_00174c10`, returns
/// nonzero): the gate's tick, the script's own jump before it (`9f`'s
/// four-byte float, `a1`'s last four bytes, or game callback 18: the
/// vertical velocity in units an update, and its tick) and the record of
/// the clip on the channel at the gate (the last blend's target); `None`
/// when the path has no gate.
///
/// Callback 18 (`FUN_00172788`, raw `0x172788..0x1729f4`) is the side
/// fades' jump (`83 12 1`): when its first word is 1, or when the
/// channel's clip is one of the ten at [`CALLBACK_18_CLIPS`], it sets the
/// actor's upward velocity to 15 / fps ([`CALLBACK_18_SPEED`], `0x1727b4`;
/// 0.25 at 60 fps), a sideways drift of 7.2 / fps, and stores
/// `FUN_0019bed8(g, vy, y)` at actor+0x64 and 0 at +0x60 as `9f` does.
pub fn script_gate(data: &[u8], address: u32) -> Result<Option<ScriptGate>> {
    let elf = Executable::new(data)?;
    let speed = f32::from_bits(executable_constant(data, CALLBACK_18_SPEED)?) / 60.0;
    let clips = executable_words(data, CALLBACK_18_CLIPS, 10)?;
    let mut jump: Option<(u32, f32)> = None;
    let mut record: i32 = -1;
    let gate = walk_script(&elf, address, |pc, op, ins, tick| {
        if let Some(blend) = &ins.blend
            && let Some(target) = blend.targets.first()
        {
            record = target.record;
        }
        if jump.is_none() {
            if op == 0x9f {
                jump = elf.word(pc + 1).ok().map(|w| (tick, f32::from_bits(w)));
            } else if op == 0xa1 {
                jump = elf.word(pc + 10).ok().map(|w| (tick, f32::from_bits(w)));
            } else if op == 0x83
                && ins.callback == Some(18)
                && (ins.callback_args[0] == 1 || clips.contains(&(record as u32)))
            {
                jump = Some((tick, speed));
            }
        }
        (op == 0x86 && elf.word(pc + 1).is_ok_and(|w| w == 19)).then_some(tick)
    })?;
    Ok(gate.map(|tick| ScriptGate { tick, jump, record }))
}

/// The `lui` of callback 18's jump speed, 15.0 (`0x41700000`): the actor's
/// upward velocity is it over the frame rate (`FUN_00172788`, `0x1727b4`;
/// both of its branches divide the same 15).
pub const CALLBACK_18_SPEED: u32 = 0x1727b4;
/// The ten clips callback 18 jumps whatever its first word
/// (`0x3d91d8`, `FUN_00168e78(0x3d91d8, record, 10)` at `0x172850`).
pub const CALLBACK_18_CLIPS: u32 = 0x3d91d8;

/// What [`script_gate`] finds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptGate {
    pub tick: u32,
    /// The jump's tick and vertical velocity, when the script has its own
    /// (`9f`, `a1`, or callback 18 for the side fades).
    pub jump: Option<(u32, f32)>,
    pub record: i32,
}

/// Whether a shot script's check carries callback 17 (`84 17`, the stick
/// fadeaway) on the held path.
pub fn script_fades(data: &[u8], address: u32) -> Result<bool> {
    let elf = Executable::new(data)?;
    Ok(walk_script(&elf, address, |_, op, ins, _| {
        (op == 0x84 && ins.callback == Some(17)).then_some(1)
    })?
    .is_some())
}

/// Little-endian words at a virtual address of the shipped program, so a
/// test can hold a rewrite constant against the table it came from.
pub fn executable_words(data: &[u8], address: u32, count: usize) -> Result<Vec<u32>> {
    let elf = Executable::new(data)?;
    (0..count as u32)
        .map(|i| elf.word(address + i * 4))
        .collect()
}

/// The constant an instruction at `address` loads: a `lui` (with the `ori`
/// on the same register that follows it, if any), or an `addiu`/`ori` from
/// `$zero` (sign- or zero-extended as the opcode does). Anything else fails,
/// so a test cannot read a constant from the wrong instruction.
pub fn executable_constant(data: &[u8], address: u32) -> Result<u32> {
    let elf = Executable::new(data)?;
    let word = elf.word(address)?;
    let (op, rs, rt, imm) = (word >> 26, word >> 21 & 31, word >> 16 & 31, word & 0xffff);
    match op {
        0x0f => {
            let next = elf.word(address + 4)?;
            let ori = next >> 26 == 0x0d && next >> 21 & 31 == rt && next >> 16 & 31 == rt;
            Ok(imm << 16 | if ori { next & 0xffff } else { 0 })
        }
        0x09 if rs == 0 => Ok(imm as u16 as i16 as i32 as u32),
        0x0d if rs == 0 => Ok(imm),
        _ => Err(Error::format(format!(
            "no constant load at {address:#x} ({word:#010x})"
        ))),
    }
}

/// `length` bytes of the executable's image at `address`.
pub fn executable_bytes(data: &[u8], address: u32, length: usize) -> Result<Vec<u8>> {
    Ok(Executable::new(data)?.bytes(address, length)?.to_vec())
}

pub(crate) struct Executable<'a> {
    data: &'a [u8],
    segments: Vec<(u32, usize, usize)>,
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(
            offset
                ..offset
                    .checked_add(2)
                    .ok_or_else(|| Error::format("ELF offset overflow"))?,
        )
        .ok_or_else(|| Error::format("truncated ELF"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or_else(|| Error::format("ELF offset overflow"))?,
        )
        .ok_or_else(|| Error::format("truncated ELF"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
impl<'a> Executable<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Result<Self> {
        if data.get(..6) != Some(&b"\x7fELF\x01\x01"[..]) || u16_at(data, 18)? != 8 {
            return Err(Error::format(
                "expected little-endian ELF32 MIPS executable",
            ));
        }
        let table = u32_at(data, 28)? as usize;
        let stride = u16_at(data, 42)? as usize;
        let count = u16_at(data, 44)? as usize;
        if stride < 32 || count > 128 {
            return Err(Error::format("invalid ELF program headers"));
        }
        let mut segments = Vec::new();
        for i in 0..count {
            let at = table
                .checked_add(i * stride)
                .ok_or_else(|| Error::format("ELF header overflow"))?;
            if u32_at(data, at)? == 1 {
                let offset = u32_at(data, at + 4)? as usize;
                let address = u32_at(data, at + 8)?;
                let size = u32_at(data, at + 16)? as usize;
                if offset.checked_add(size).is_none_or(|end| end > data.len()) {
                    return Err(Error::format("ELF segment past end of file"));
                }
                segments.push((address, offset, size));
            }
        }
        if segments.is_empty() {
            return Err(Error::format("ELF has no load segments"));
        }
        Ok(Self { data, segments })
    }
    pub(crate) fn bytes(&self, address: u32, length: usize) -> Result<&[u8]> {
        for &(base, offset, size) in &self.segments {
            if let Some(delta) = address.checked_sub(base) {
                let delta = delta as usize;
                if delta.checked_add(length).is_some_and(|end| end <= size) {
                    return Ok(&self.data[offset + delta..offset + delta + length]);
                }
            }
        }
        Err(Error::format(format!(
            "unmapped executable address {address:#x}+{length}"
        )))
    }
    pub(crate) fn byte(&self, address: u32) -> Result<u8> {
        Ok(self.bytes(address, 1)?[0])
    }
    pub(crate) fn word(&self, address: u32) -> Result<u32> {
        u32_at(self.bytes(address, 4)?, 0)
    }
    pub(crate) fn string(&self, address: u32) -> Result<String> {
        let mut bytes = Vec::new();
        for i in 0..512 {
            let value = self.byte(
                address
                    .checked_add(i)
                    .ok_or_else(|| Error::format("string address overflow"))?,
            )?;
            if value == 0 {
                return String::from_utf8(bytes)
                    .map_err(|_| Error::format("invalid executable string"));
            }
            bytes.push(value);
        }
        Err(Error::format("unterminated executable string"))
    }
}

fn inspect_script(
    elf: &Executable<'_>,
    registry: &BTreeMap<u32, String>,
    names: &[(Option<String>, bool)],
    address: u32,
    state_mask: u32,
    legend: Option<u16>,
) -> Result<MoveScript> {
    let name = registry
        .get(&address)
        .ok_or_else(|| Error::format(format!("script {address:#x} absent from registry")))?
        .clone();
    let end = registry
        .range((address + 1)..)
        .next()
        .map(|(&a, _)| a)
        .ok_or_else(|| Error::format("script has no next registry boundary"))?;
    let mut pending = BTreeSet::from([address]);
    let mut visited = BTreeSet::new();
    let mut ops = BTreeMap::new();
    let mut external = BTreeSet::new();
    while let Some(pc) = pending.pop_first() {
        if !visited.insert(pc) {
            continue;
        }
        if pc < address || pc >= end {
            external.insert(pc);
            continue;
        }
        let decoded = decode(elf, names, pc)?;
        if decoded.next > end {
            return Err(Error::format(format!(
                "script {name}: operand crosses body boundary at {pc:#x}"
            )));
        }
        for &branch in &decoded.branches {
            pending.insert(branch);
        }
        if decoded.fallthrough && decoded.next < end {
            pending.insert(decoded.next);
        }
        ops.insert(pc, decoded);
        if visited.len() > 4096 {
            return Err(Error::format("script inspection instruction limit"));
        }
    }
    let mut blends = Vec::new();
    let mut clip_targets = Vec::new();
    let mut callbacks = Vec::new();
    let mut callback_args = Vec::new();
    for (_, op) in ops {
        for target in op.targets {
            if let Some(name) = target.name
                && !clip_targets.contains(&name)
            {
                clip_targets.push(name);
            }
        }
        if let Some(blend) = op.blend {
            blends.push(blend);
        }
        if let Some(id) = op.callback
            && !callbacks.contains(&id)
        {
            callbacks.push(id);
            callback_args.push(op.callback_args);
        }
    }
    Ok(MoveScript {
        name,
        address,
        state_mask,
        legend,
        blends,
        clip_targets,
        external_jumps: external.into_iter().collect(),
        callbacks,
        callback_args,
    })
}

pub(crate) struct Instruction {
    pub(crate) next: u32,
    pub(crate) fallthrough: bool,
    pub(crate) branches: Vec<u32>,
    pub(crate) blend: Option<Blend>,
    pub(crate) targets: Vec<ClipTarget>,
    pub(crate) callback: Option<u32>,
    callback_args: [u32; 3],
}
fn clip(names: &[(Option<String>, bool)], record: u32) -> ClipTarget {
    let target = names.get(record as usize);
    ClipTarget {
        record: record as i32,
        name: target.and_then(|(name, _)| name.clone()),
        mirrored: target.is_some_and(|(_, mirrored)| *mirrored),
    }
}

pub(crate) fn decode(
    elf: &Executable<'_>,
    names: &[(Option<String>, bool)],
    pc: u32,
) -> Result<Instruction> {
    let op = elf.byte(pc)?;
    let p = pc + 1;
    let mut n = 0;
    let mut result = Instruction {
        next: p,
        fallthrough: true,
        branches: Vec::new(),
        blend: None,
        targets: Vec::new(),
        callback: None,
        callback_args: [0; 3],
    };
    match op {
        0x00..=0x80 | 0x88 | 0x8f | 0x90 | 0x96 | 0x97 | 0xa7 => (),
        0x81 => result.fallthrough = false,
        0x82 => {
            n = 4;
            result.branches.push(elf.word(p)?);
            result.fallthrough = false;
        }
        0x83 => {
            n = 16;
            result.callback = Some(elf.word(p)?);
            result.callback_args = [elf.word(p + 4)?, elf.word(p + 8)?, elf.word(p + 12)?];
        }
        0x84 | 0x85 => {
            // A game callback whose answer decides the jump
            // (`FUN_00195b80`, `FUN_00195cd0`): the id and words first.
            n = 20;
            result.callback = Some(elf.word(p)?);
            result.callback_args = [elf.word(p + 4)?, elf.word(p + 8)?, elf.word(p + 12)?];
            result.branches.push(elf.word(p + 16)?);
        }
        0x87 => {
            n = 24;
            result
                .branches
                .extend([elf.word(p + 16)?, elf.word(p + 20)?]);
            result.fallthrough = false;
        }
        // `a0` (`FUN_001957d8`) reads the slide's float too.
        0x86 | 0x8c | 0x91 | 0x9b | 0x9c | 0xa0 | 0xa2 | 0xa4 | 0xaa | 0xab | 0xae => n = 4,
        0x8b | 0xa3 => n = 1,
        0x92 | 0x9d | 0xa6 | 0xad => n = 3,
        // `FUN_00194c40` reads one float (`addiu t1, a2, 4` past it).
        0x9f => n = 4,
        0xa1 | 0xac => n = 13,
        0xa8 => n = 9,
        0xa9 => {
            n = match elf.byte(p)? {
                0xef => 10,
                0xec => 18,
                _ => 17,
            }
        }
        0x9a => {
            let jump = (p as i64 + elf.byte(p)? as i8 as i64 + 1) as u32;
            result.branches.push(jump);
            result.fallthrough = false;
            n = 1;
        }
        0x98 | 0x99 => loop {
            let value = elf.word(p + n)?;
            n += 4;
            if value >= 1144 {
                result.branches.push(value);
                break;
            }
            if n > 512 {
                return Err(Error::format("unterminated animation-test operand"));
            }
        },
        0x89 => {
            let mut record = elf.word(p)?;
            n = 4;
            if record == (-7i32) as u32 {
                record = elf.word(p + n)?;
                n += 4;
            }
            if record == (-8i32) as u32 || record == (-14i32) as u32 {
                result.targets.push(clip(names, elf.word(p + n)?));
                result.targets.push(clip(names, elf.word(p + n + 4)?));
                n += 8;
            } else {
                result.targets.push(clip(names, record));
            }
        }
        0x8a => n = if elf.byte(p)? >= 128 { 3 } else { 1 },
        0x8d | 0xa5 => {
            n = if elf.byte(p)? == 0xee { 3 } else { 2 };
            if op == 0x8d && elf.byte(p + n - 1)? == 0xef {
                n += 1;
            }
        }
        0x93..=0x95 => {
            if elf.byte(p)? == 0xf1 {
                n += 1;
            }
            if matches!(elf.byte(p + n)?, 0xec | 0xf2 | 0xf3 | 0xf4 | 0xf7 | 0xf8) {
                n += 1;
            }
            n += 2;
        }
        0x8e => {
            let mut selectors = Vec::new();
            // The handler tests prefixes in this exact order, independently.
            for selector in [0xf2, 0xf8, 0xec, 0xf5, 0xf4, 0xf3] {
                if elf.byte(p + n)? == selector {
                    selectors.push(selector);
                    n += 1;
                }
            }
            let frames = elf.byte(p + n)? as i8;
            n += 1;
            let start_frame = elf.word(p + n)? as i32;
            result.targets.push(clip(names, elf.word(p + n + 4)?));
            n += 8;
            for _ in &selectors {
                result.targets.push(clip(names, elf.word(p + n)?));
                n += 4;
            }
            result.blend = Some(Blend {
                address: pc,
                selectors,
                frames,
                start_frame,
                targets: result.targets.clone(),
            });
        }
        0x9e => {
            let mut selector = elf.byte(p)?;
            n = 1;
            if selector == 0x83 {
                selector = elf.byte(p + n)?;
                n += 1;
            }
            if selector == 0x84 {
                selector = elf.byte(p + n)?;
                n += 1;
            }
            n += if selector >= 128 { 9 } else { 16 };
        }
        _ => {
            return Err(Error::format(format!(
                "illegal animation opcode {op:#x} at {pc:#x}"
            )));
        }
    }
    elf.bytes(p, n as usize)?;
    result.next = p
        .checked_add(n)
        .ok_or_else(|| Error::format("script address overflow"))?;
    Ok(result)
}

/// Shared two-turbo move slot masks, in slot order.
pub const DUNK_MASKS: [u32; 11] = [
    0x3c000, 0x1c000, 0x2c000, 0x34000, 0x38000, 0xc000, 0x14000, 0x24000, 0x18000, 0x28000,
    0x30000,
];
pub const DUNK_LEVELS: [u8; 11] = [4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2];
pub const SCRIPTS: usize = 27;
pub const DUNK_SCRIPTS: usize = 60;
pub const TRICK_SCRIPTS: usize = 75;

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
    fn blend_alternates_retain_mirroring_and_do_not_scan_next_script() {
        let mut body = vec![0x8e, 0xf2, 6];
        body.extend(0u32.to_le_bytes());
        body.extend(0u32.to_le_bytes());
        body.extend(1u32.to_le_bytes());
        body.push(0x81);
        let boundary = 0x1000 + body.len() as u32;
        body.extend([0x8e, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x81]);
        let data = executable(&body);
        let elf = Executable::new(&data).unwrap();
        let registry = BTreeMap::from([(0x1000, "a".into()), (boundary, "b".into())]);
        let names = vec![(Some("left".into()), false), (Some("left".into()), true)];
        let script = inspect_script(&elf, &registry, &names, 0x1000, 0, None).unwrap();
        assert_eq!(script.blends.len(), 1);
        assert_eq!(script.clip_targets, ["left"]);
        assert_eq!(script.blends[0].selectors, [0xf2]);
        assert_eq!(script.blends[0].frames, 6);
        assert!(!script.blends[0].targets[0].mirrored);
        assert!(script.blends[0].targets[1].mirrored);
    }

    #[test]
    fn conditionals_visit_both_paths_and_external_jump_stops_body() {
        let mut body = vec![0x84];
        body.extend([0; 16]);
        body.extend(0x101bu32.to_le_bytes());
        body.push(0x82);
        body.extend(0x2000u32.to_le_bytes());
        body.push(0x80);
        body.push(0x89);
        body.extend(0u32.to_le_bytes());
        body.push(0x81);
        let data = executable(&body);
        let elf = Executable::new(&data).unwrap();
        let registry = BTreeMap::from([
            (0x1000, "branch".into()),
            (0x1000 + body.len() as u32, "next".into()),
        ]);
        let script = inspect_script(
            &elf,
            &registry,
            &[(Some("target".into()), false)],
            0x1000,
            0,
            None,
        )
        .unwrap();
        assert_eq!(script.clip_targets, ["target"]);
        assert_eq!(script.external_jumps, [0x2000]);
    }

    #[test]
    fn original_dunk_candidates_match_every_native_tuple_and_correct_ids() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = crate::iso::Iso::open(path).unwrap();
        let data = iso.read("/SLUS_206.51").unwrap();
        let elf = Executable::new(&data).unwrap();
        let scripts = MoveScripts::from_executable(&data).unwrap();
        let mut tuples = Vec::new();
        let mut digest = 0xcbf29ce484222325u64;
        for index in 0..99 {
            let address = 0x3d7b28 + index * 20;
            let tuple: [u32; 5] =
                std::array::from_fn(|word| elf.word(address + word as u32 * 4).unwrap());
            for byte in tuple.into_iter().flat_map(u32::to_le_bytes) {
                digest = (digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
            if tuple[3] == 0 {
                assert!(
                    scripts.dunks[tuple[4] as usize]
                        .iter()
                        .any(|script| script.address == tuple[0]
                            && script.legend
                                == if tuple[1] == u32::MAX {
                                    None
                                } else {
                                    Some(tuple[1] as u16)
                                }
                            && script.state_mask == tuple[2]),
                    "native candidate{index}: {tuple:x?}"
                );
            }
            tuples.push(tuple);
        }
        // Original retained raw oracle: all99 complete tuples, including
        // family2 rows, rather than a parser-dependent count/digest.
        assert_eq!(digest, 0xd76faa2a3ad133a2);
        assert_eq!(tuples.iter().filter(|tuple| tuple[3] == 0).count(), 66);
        assert_eq!(tuples[0], [0x44960a, 0xaf, 0x3ff, 0, 29]);
        assert_eq!(tuples[6], [0x449f89, 0xb2, 0x1ff, 0, 47]);
        assert_eq!(tuples[38], [0x447b53, u32::MAX, 0x8ff, 0, 30]);
        for (index, id) in [(56, 48), (57, 49), (58, 50), (59, 51)] {
            assert_eq!(tuples[index], [0x447b53, u32::MAX, 0x8ff, 0, id]);
        }
        assert_eq!(tuples[67], [0x44a7b3, u32::MAX, 0x8ff, 0, 59]);
        assert_eq!(tuples[98], [0x447aec, u32::MAX, 0x100883, 0, 41]);
        assert!(scripts.dunks.iter().all(|rows| !rows.is_empty()));
    }

    #[test]
    fn slot_resolution_follows_0x13a1f8_switches_and_fixed_rows() {
        // The expected ids are decoded from 0x13a1f8's own instructions: the
        // jump tables of its two legend switches (`record - 0x91`, 25 trick
        // and 37 dunk cases) and the `addiu v1, zero, id` each case runs.
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let data = crate::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let scripts = MoveScripts::from_executable(&data).unwrap();
        let word = |a: u32| executable_words(&data, a, 1).unwrap()[0];
        // addiu v1, s3, -0x91: both switches index by `record - 0x91`.
        assert_eq!(word(0x13a298), 0x2663_ff6f);
        assert_eq!(word(0x13a3ac), 0x2663_ff6f);
        for (kind, table, cases, default) in [
            (Kind::Trick, 0x43d290u32, 25u32, 0x13a338u32),
            (Kind::Dunk, 0x43d300, 37, 0x13a464),
        ] {
            for case in 0..cases {
                let target = word(table + 4 * case);
                let expected = (target != default).then(|| {
                    // A `b` with the id in its delay slot, or the last case's
                    // `addiu` itself.
                    let at = if word(target) >> 26 == 0x04 {
                        target + 4
                    } else {
                        target
                    };
                    assert_eq!(word(at) >> 16, 0x2403, "{at:#x}");
                    executable_constant(&data, at).unwrap() as u16
                });
                let record = 0x91 + case as usize;
                assert_eq!(legend_slot_script(kind, record), expected, "{record:#x}");
                // Other teams' slot 1 plays it when nothing is equipped.
                assert_eq!(
                    scripts.slot_script(kind, record, Some(29), 1, None),
                    expected.or(scripts.slot_script(kind, 0, Some(0), 1, None))
                );
                assert_eq!(
                    scripts.slot_script(kind, record, Some(CREATED_TEAM), 1, None),
                    scripts.slot_script(kind, 0, Some(0), 1, None)
                );
            }
        }
        // FUN_001d9e60's fixed rows for the generic team, each through the
        // row-to-script table at 0x45f9c8, and the defaults (0x43d6c0
        // shots, 0x43d5d0 dunks, 0x43d498 tricks) for everyone else.
        for (kind, rows, defaults, count) in [
            (Kind::Shot, 0x45fde0u32, 0x43d6c0u32, 6u32),
            (Kind::Dunk, 0x45fdf0, 0x43d5d0, 11),
            (Kind::Trick, 0x45fe08, 0x43d498, 11),
        ] {
            for slot in 0..count {
                let half = word((rows + 2 * slot) & !3) >> (8 * ((rows + 2 * slot) & 2));
                let row = half & 0xffff;
                let default = word(defaults + 4 * slot);
                let expected = if row == 0xffff {
                    default
                } else {
                    word(0x45f9c8 + 4 * row)
                };
                let slot = slot as usize;
                assert_eq!(
                    scripts.slot_script(kind, 0, Some(GENERIC_TEAM), slot, Some(0)),
                    Some(expected as u16),
                    "{kind:?} {slot}"
                );
                assert_eq!(
                    scripts.slot_script(kind, 0, Some(CREATED_TEAM), slot, None),
                    Some(default as u16)
                );
            }
        }
    }

    #[test]
    fn truncated_and_non_mips_executables_are_errors() {
        assert!(MoveScripts::from_executable(&[]).is_err());
        assert!(MoveScripts::from_executable(&executable(&[0x81])).is_err());
        let mut data = executable(&[0x81]);
        data[18] = 3;
        assert!(Executable::new(&data).is_err());
    }
}
