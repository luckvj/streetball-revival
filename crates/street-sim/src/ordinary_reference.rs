//! Test-only original-word executor, independent of the numeric kernel.
//! It executes admitted MIPS scalar/leaf fragments, not a whole EE actor.
//! Declared helper outputs: singleton time at60Hz, resolved owner/heading,
//! decoded row states. Pose/root/handles and overwritten temporary frame-count
//! results are excluded. No captured actor values are an initializer.

use super::*;

const ACTOR: u32 = 0x1000_0000;
const CHANNEL: u32 = 0x1000_1000;
const TIME: u32 = 0x1000_2000;
const POSITION: u32 = 0x1000_3000;
const STACK: u32 = 0x1001_0000;
const RETURN: u32 = 0xffff_fffc;

pub(super) struct Machine<'a> {
    elf: &'a [u8],
    memory: BTreeMap<u32, u8>,
    gpr: [u128; 32],
    fpr: [u32; 32],
    condition: bool,
    heading: i32,
    initial: Projection,
    states: &'a [u32],
}

enum Flow {
    Jump(u32),
    Hook(u32),
    Annul,
}

impl<'a> Machine<'a> {
    pub(super) fn new(elf: &'a [u8], state: &Projection, states: &'a [u32]) -> Self {
        let mut machine = Self {
            elf,
            memory: BTreeMap::new(),
            gpr: [0; 32],
            fpr: [0; 32],
            condition: false,
            heading: 0,
            initial: state.clone(),
            states,
        };
        let actor = &state.actor;
        for (base, words) in [
            (0x30, actor.position),
            (0x40, actor.physical),
            (0x2c0, actor.placement),
        ] {
            for (i, word) in words.into_iter().enumerate() {
                machine.put(ACTOR + base + i as u32 * 4, word);
            }
        }
        for (offset, word) in [
            (0x60, actor.elapsed as u32),
            (0x64, actor.flight as u32),
            (0xa4, actor.flags),
            (0xa8, actor.heading as u32),
            (0xb0, actor.heading_b0 as u32),
            (0x198, actor.state_198 as u32),
            (0x1a0, actor.state_1a0 as u32),
            (0x1a4, actor.state_1a4 as u32),
            (0x1a8, actor.state_1a8 as u32),
            (0x1b8, actor.state_1b8 as u32),
            (0x1d4, actor.state_1d4 as u32),
            (0x2b4, actor.state_2b4 as u32),
            (0x2d0, actor.placement_heading as u32),
        ] {
            machine.put(ACTOR + offset, word);
        }
        let channel = &state.primary;
        for (offset, word) in [
            (0x10, channel.pc),
            (0x48, channel.flags),
            (0x50, channel.row as u32),
            (0x54, channel.alternate as u32),
            (0x58, channel.link as u32),
            (0x5c, channel.integer as u32),
            (0x60, channel.target as u32),
            (0x64, channel.phase),
            (0x3c, channel.configured_step),
            (0xb0, channel.held_offset),
        ] {
            machine.put(CHANNEL + offset, word);
        }
        for (offset, value) in [
            (0x4c, channel.wait.map(|v| v as u32)),
            (0x68, channel.step),
            (0x6c, channel.duration.map(|v| v as u32)),
            (0x70, channel.counter.map(|v| v as u32)),
        ] {
            if let Some(value) = value {
                machine.put(CHANNEL + offset, value);
            }
        }
        if let Some(flags) = channel.state_flags {
            machine.put(CHANNEL + 0x14, flags[0]);
            machine.put(CHANNEL + 0x18, flags[1]);
        }
        machine.put(TIME + 0x28, 60.0_f32.to_bits());
        // 193130 initializes the native animation handle sentinel; actual
        // handle ownership/buffer allocation is outside this projection.
        machine.put(CHANNEL + 0xb4, u32::MAX);
        for (opcode, handler) in [
            (0x89, 0x196318),
            (0x91, 0x195858),
            (0x96, 0x194780),
            (0x97, 0x1947a0),
            (0x9b, 0x1958b0),
            (0xa7, 0x193710),
            (0xab, 0x197168),
        ] {
            assert_eq!(machine.word(0x3e8610 + (opcode & 0x7f) * 4), handler);
        }
        machine
    }

    fn source(&self, address: u32) -> u8 {
        let word = |at: usize| u32::from_le_bytes(self.elf[at..at + 4].try_into().unwrap());
        let table = word(28) as usize;
        let stride = u16::from_le_bytes(self.elf[42..44].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(self.elf[44..46].try_into().unwrap()) as usize;
        for i in 0..count {
            let at = table + i * stride;
            if word(at) == 1
                && let Some(delta) = address.checked_sub(word(at + 8))
                && delta < word(at + 16)
            {
                return self.elf[(word(at + 4) + delta) as usize];
            }
        }
        panic!("unmapped reference read {address:#x}");
    }

    fn byte(&self, address: u32) -> u8 {
        self.memory
            .get(&address)
            .copied()
            .unwrap_or_else(|| self.source(address))
    }

    fn word(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.byte(address + i as u32)))
    }

    fn put(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.memory.insert(address + i as u32, byte);
        }
    }

    fn reg(&self, index: usize) -> u32 {
        self.gpr[index] as u32
    }
    fn set(&mut self, index: usize, value: u32) {
        if index != 0 {
            self.gpr[index] = value as i32 as i128 as u128;
        }
    }

    fn call(&mut self, address: u32) {
        self.set(31, RETURN);
        self.run(address, RETURN);
    }

    fn run(&mut self, mut pc: u32, end: u32) {
        for _ in 0..4096 {
            if pc == end {
                return;
            }
            match self.step(pc) {
                None => pc += 4,
                Some(Flow::Annul) => pc += 8,
                Some(flow) => {
                    assert!(
                        self.step(pc + 4).is_none(),
                        "reference branch in delay slot"
                    );
                    pc = match flow {
                        Flow::Jump(target) => target,
                        Flow::Hook(target) => {
                            match target {
                                0x30fa50 => self.set(2, TIME),
                                0x18ce78 => self.set(2, self.heading as u32),
                                0x19c2e0 => self.set(2, CHANNEL),
                                _ => panic!("undeclared reference helper {target:#x}"),
                            }
                            pc + 8
                        }
                        Flow::Annul => unreachable!(),
                    };
                }
            }
        }
        panic!("reference instruction budget at {pc:#x}");
    }

    fn step(&mut self, pc: u32) -> Option<Flow> {
        let w = self.word(pc);
        let (op, rs, rt, rd, shift, funct) = (
            w >> 26,
            (w >> 21 & 31) as usize,
            (w >> 16 & 31) as usize,
            (w >> 11 & 31) as usize,
            w >> 6 & 31,
            w & 63,
        );
        let immediate = w as u16;
        let address = self.reg(rs).wrapping_add(immediate as i16 as u32);
        let branch = pc
            .wrapping_add(4)
            .wrapping_add((immediate as i16 as i32 * 4) as u32);
        match op {
            0 => match funct {
                0 => self.set(rd, self.reg(rt) << shift),
                8 => return Some(Flow::Jump(self.reg(rs))),
                0x21 | 0x2d => self.set(rd, self.reg(rs).wrapping_add(self.reg(rt))),
                0x24 => self.set(rd, self.reg(rs) & self.reg(rt)),
                0x25 => self.set(rd, self.reg(rs) | self.reg(rt)),
                0x27 => self.set(rd, !(self.reg(rs) | self.reg(rt))),
                0x0b => {
                    if self.reg(rt) != 0 {
                        self.set(rd, self.reg(rs));
                    }
                }
                _ => panic!("reference special {w:#x} at {pc:#x}"),
            },
            3 => {
                self.set(31, pc + 8);
                let target = (pc & 0xf000_0000) | (w & 0x03ff_ffff) << 2;
                return Some(if [0x30fa50, 0x18ce78, 0x19c2e0].contains(&target) {
                    Flow::Hook(target)
                } else {
                    Flow::Jump(target)
                });
            }
            4 | 5 | 6 | 0x14 | 0x15 => {
                let take = match op {
                    4 | 0x14 => self.reg(rs) == self.reg(rt),
                    5 | 0x15 => self.reg(rs) != self.reg(rt),
                    6 => (self.reg(rs) as i32) <= 0,
                    _ => unreachable!(),
                };
                return if take {
                    Some(Flow::Jump(branch))
                } else if op >= 0x14 {
                    Some(Flow::Annul)
                } else {
                    None
                };
            }
            9 => self.set(rt, address),
            0xc => self.set(rt, self.reg(rs) & u32::from(immediate)),
            0xd => self.set(rt, self.reg(rs) | u32::from(immediate)),
            0xf => self.set(rt, u32::from(immediate) << 16),
            0x20 => self.set(rt, self.byte(address) as i8 as i32 as u32),
            0x23 => self.set(rt, self.word(address)),
            0x24 => self.set(rt, u32::from(self.byte(address))),
            0x2b => self.put(address, self.reg(rt)),
            0x31 => self.fpr[rt] = self.word(address),
            0x39 => self.put(address, self.fpr[rt]),
            0x1e => {
                self.gpr[rt] =
                    u128::from_le_bytes(std::array::from_fn(|i| self.byte(address + i as u32)))
            }
            0x1f => {
                for (i, byte) in self.gpr[rt].to_le_bytes().into_iter().enumerate() {
                    self.memory.insert(address + i as u32, byte);
                }
            }
            0x11 => match rs {
                0 => self.set(rt, self.fpr[rd]),
                4 => self.fpr[rd] = self.reg(rt),
                8 => {
                    let take = self.condition == (rt & 1 != 0);
                    return if take {
                        Some(Flow::Jump(branch))
                    } else if rt & 2 != 0 {
                        Some(Flow::Annul)
                    } else {
                        None
                    };
                }
                16 => {
                    let destination = shift as usize;
                    let (a, b) = (f32::from_bits(self.fpr[rd]), f32::from_bits(self.fpr[rt]));
                    if funct == 0x32 || funct == 0x34 {
                        self.condition = if funct == 0x32 { a == b } else { a < b };
                        return None;
                    }
                    self.fpr[destination] = match funct {
                        0 if a >= 0.0 && b >= 0.0 => exact_positive_add(self.fpr[rd], self.fpr[rt]),
                        0 => chop(f64::from(a) + f64::from(b)),
                        1 => chop(f64::from(a) - f64::from(b)),
                        2 => chop(f64::from(a) * f64::from(b)),
                        3 => (a / b).to_bits(),
                        4 => b.sqrt().to_bits(), // EE sqrt usesft, original19bf14
                        6 => self.fpr[rd],
                        7 => self.fpr[rd] ^ 0x8000_0000,
                        0x24 => a as i32 as u32,
                        _ => panic!("reference float {w:#x} at {pc:#x}"),
                    };
                }
                20 if funct == 0x20 => {
                    self.fpr[shift as usize] = (self.fpr[rd] as i32 as f32).to_bits()
                }
                _ => panic!("reference cop1 {w:#x} at {pc:#x}"),
            },
            _ => panic!("reference opcode {w:#x} at {pc:#x}"),
        }
        None
    }

    pub(super) fn place(&mut self, position: [u32; 4], heading: i32) -> Projection {
        self.heading = heading;
        for (i, word) in position.into_iter().enumerate() {
            self.put(POSITION + i as u32 * 4, word);
        }
        self.set(4, ACTOR);
        self.set(5, POSITION);
        self.set(6, POSITION);
        self.set(29, STACK);
        self.set(31, RETURN);
        self.run(0x1704e0, 0x170544);
        // 170544..550 dead physical store is overwritten by97 before use.
        // Setter relation branches are known false after source A4 reset.
        self.put(CHANNEL + 0x10, 0x4574e2);
        self.put(CHANNEL + 0x48, self.word(CHANNEL + 0x48) & 0xffdf_0000);
        self.put(CHANNEL + 0x4c, 0);
        loop {
            let pc = self.word(CHANNEL + 0x10);
            let opcode = self.byte(pc);
            self.put(CHANNEL + 0x10, pc + 1);
            self.set(4, ACTOR);
            self.set(5, CHANNEL);
            match opcode {
                0xa7 => {
                    self.set(2, ACTOR);
                    self.run(0x193750, 0x193754);
                    self.run(0x193768, 0x19376c);
                }
                0x96 => self.call(0x194780),
                0x97 => self.call(0x1947a0),
                0x91 => self.call(0x195858),
                0x9b => self.call(0x1958b0),
                0x89 => {
                    let row = self.word(pc + 1);
                    assert_eq!(row, 65);
                    self.put(CHANNEL + 0x10, pc + 5);
                    self.set(17, row);
                    self.set(18, CHANNEL);
                    self.fpr[20] = (-1.0_f32).to_bits();
                    self.run(0x196594, 0x196598);
                    self.set(2, self.states[0]);
                    self.run(0x1965ac, 0x1965d8);
                    self.run(0x196608, 0x196628);
                    self.run(0x19662c, 0x196630);
                }
                0xab => {
                    self.set(16, CHANNEL);
                    self.run(0x197188, 0x1971d4);
                    // Native frame-count result in60 is unconditionally
                    // overwritten at197210, so exclude that getter/call.
                    self.run(0x1971f8, 0x197274);
                    self.clock();
                    break;
                }
                _ => panic!("unadmitted original entry opcode {opcode:#x}"),
            }
        }
        self.put(CHANNEL + 0x4c, 2); // outside-update setter tail
        self.set(16, ACTOR);
        self.set(17, ACTOR + 0xc4);
        self.run(0x17055c, 0x1705e4);
        self.projection()
    }

    pub(super) fn update(&mut self) -> Projection {
        //193440 scalar gate; no actor/action/facial update is claimed.
        if self.word(CHANNEL + 0x48) & 1 == 0 {
            let wait = self.word(CHANNEL + 0x4c).wrapping_sub(1);
            self.put(CHANNEL + 0x4c, wait);
            if wait == 0 {
                self.clock();
            }
        }
        self.projection()
    }

    fn clock(&mut self) {
        self.set(18, CHANNEL);
        self.set(3, self.word(CHANNEL + 0x48));
        self.run(0x19a8c8, 0x19a9d0); // ADD in branch delay, then store64
        let phase = f32::from_bits(self.word(CHANNEL + 0x64));
        assert!(phase >= 0.0 && phase < (self.states.len() - 1) as f32);
        self.set(16, u32::MAX);
        self.run(0x19aca4, 0x19acd4); // wait and CVT/store5c delay slot
        let integer = self.word(CHANNEL + 0x5c) as usize;
        // Declared metadata getter result, not an emulator of animation banks.
        self.put(CHANNEL + 0x14, self.states[integer]);
        self.put(CHANNEL + 0x18, self.states[integer]);
        if self.word(CHANNEL + 0x48) & 0x20000 != 0 {
            assert_eq!(self.word(CHANNEL + 0x54), u32::MAX);
            self.set(4, self.word(CHANNEL + 0x48));
            self.run(0x19b618, 0x19b630); // reached non-blend tail clear
        }
    }

    fn projection(&self) -> Projection {
        let mut result = self.initial.clone();
        let actor = &mut result.actor;
        actor.position = std::array::from_fn(|i| self.word(ACTOR + 0x30 + i as u32 * 4));
        actor.physical = std::array::from_fn(|i| self.word(ACTOR + 0x40 + i as u32 * 4));
        actor.placement = std::array::from_fn(|i| self.word(ACTOR + 0x2c0 + i as u32 * 4));
        actor.elapsed = self.word(ACTOR + 0x60) as i32;
        actor.flight = self.word(ACTOR + 0x64) as i32;
        actor.flags = self.word(ACTOR + 0xa4);
        actor.heading = self.word(ACTOR + 0xa8) as i32;
        actor.heading_b0 = self.word(ACTOR + 0xb0) as i32;
        actor.state_198 = self.word(ACTOR + 0x198) as i32;
        actor.state_1a0 = self.word(ACTOR + 0x1a0) as i32;
        actor.state_1a4 = self.word(ACTOR + 0x1a4) as i32;
        actor.state_1a8 = self.word(ACTOR + 0x1a8) as i32;
        actor.state_1b8 = self.word(ACTOR + 0x1b8) as i32;
        actor.state_1d4 = self.word(ACTOR + 0x1d4) as i32;
        actor.state_2b4 = self.word(ACTOR + 0x2b4) as i32;
        actor.placement_heading = self.word(ACTOR + 0x2d0) as i32;
        let channel = &mut result.primary;
        channel.pc = self.word(CHANNEL + 0x10);
        channel.wait = Some(self.word(CHANNEL + 0x4c) as i32);
        channel.flags = self.word(CHANNEL + 0x48);
        channel.state_flags = Some([self.word(CHANNEL + 0x14), self.word(CHANNEL + 0x18)]);
        channel.row = self.word(CHANNEL + 0x50) as i32;
        channel.alternate = self.word(CHANNEL + 0x54) as i32;
        channel.link = self.word(CHANNEL + 0x58) as i32;
        channel.integer = self.word(CHANNEL + 0x5c) as i32;
        channel.target = self.word(CHANNEL + 0x60) as i32;
        channel.phase = self.word(CHANNEL + 0x64);
        channel.configured_step = self.word(CHANNEL + 0x3c);
        channel.step = Some(self.word(CHANNEL + 0x68));
        channel.duration = Some(self.word(CHANNEL + 0x6c) as i32);
        channel.counter = Some(self.word(CHANNEL + 0x70) as i32);
        channel.held_offset = self.word(CHANNEL + 0xb0);
        result
    }
}

fn chop(exact: f64) -> u32 {
    let nearest = exact as f32;
    if nearest.is_finite() && f64::from(nearest.abs()) > exact.abs() {
        nearest.to_bits() - 1
    } else {
        nearest.to_bits()
    }
}

/// Independent integer-significand positive ADD oracle, not kernel's f64
/// sum/cast implementation. Inputs in these cases are finite normals or0.
fn exact_positive_add(a: u32, b: u32) -> u32 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    assert_eq!((a | b) & 0x8000_0000, 0);
    let (a, b) = if a >= b { (a, b) } else { (b, a) };
    let (mut exponent, small_exponent) = ((a >> 23) & 255, (b >> 23) & 255);
    let difference = exponent - small_exponent;
    assert!(exponent > 0 && exponent < 255 && small_exponent > 0);
    if difference > 31 {
        return a;
    }
    let mut significand = (u64::from((a & 0x7f_ffff) | 0x80_0000) << difference)
        + u64::from((b & 0x7f_ffff) | 0x80_0000);
    significand >>= difference;
    if significand >= 0x100_0000 {
        significand >>= 1;
        exponent += 1;
    }
    (exponent << 23) | (significand as u32 & 0x7f_ffff)
}
