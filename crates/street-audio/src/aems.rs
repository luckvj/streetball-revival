//! AEMS: the sound scripts that decide what the game's effects play.
//!
//! The game never names a sound. It fires an *event* (an index into the
//! event map, `SFX/GAME_EVT.AEM` in a match, `SFX/FE_EVT.AEM` in the front
//! end) with a few parameters: where the sound is (an angle and a distance
//! from the camera) and what kind it is (a trick level, a player's voice...).
//! The map turns the event into *actions* on the *modules* of the loaded
//! banks (`.ABK`): start a new instance of a module, pulse one of its
//! triggers, or set one of its named inputs to a parameter. A module is a
//! small graph of nodes, a modular synthesizer: constants, random pickers,
//! envelopes, table lookups, arithmetic, and *players*, which choose a sound
//! from a list and drive a voice's pitch, volume, direction and effect send.
//! The graph runs every 10 ms ([`TICK_MS`]).
//!
//! This is a port of the game's runtime (`0x34a000..0x34f800` in
//! `SLUS_206.51`; `docs/FORMATS.md` has the file layouts and the addresses
//! of each node's code). It is pure: voices are started and steered through
//! a [`Mixer`] the caller supplies.

use std::collections::VecDeque;

use street_assets::{Error, Result};

/// How often modules run.
pub const TICK_MS: i32 = 10;

fn bad(what: &str) -> Error {
    Error::Format(format!("AEMS: {what}"))
}

/// Little-endian reads over a file, with bounds checks.
#[derive(Clone, Copy)]
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&self, at: usize) -> Result<u8> {
        self.0
            .get(at)
            .copied()
            .ok_or_else(|| bad("read past the end"))
    }
    fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes([self.u8(at)?, self.u8(at + 1)?]))
    }
    fn i16(&self, at: usize) -> Result<i16> {
        Ok(self.u16(at)? as i16)
    }
    fn u32(&self, at: usize) -> Result<u32> {
        let b = self
            .0
            .get(at..at + 4)
            .ok_or_else(|| bad("read past the end"))?;
        Ok(u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn i32(&self, at: usize) -> Result<i32> {
        Ok(self.u32(at)? as i32)
    }
    /// A `{u32 offset, i16 count}` list of u32 indices.
    fn list(&self, at: usize) -> Result<Vec<u32>> {
        let (start, n) = (self.u32(at)? as usize, self.i16(at + 4)?);
        (0..n.max(0) as usize)
            .map(|k| self.u32(start + 4 * k))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The event map (.AEM)

/// Sets a module input to an event parameter, clamped.
#[derive(Clone, Debug)]
pub struct ParamAction {
    pub min: i32,
    pub max: i32,
    pub target: Target,
}

/// A node of a module of a bank, by the ids and name hash the files use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub bank: u16,
    pub module: u16,
    pub node: u32,
}

#[derive(Clone, Debug, Default)]
pub struct EventDef {
    /// For each parameter, the actions it drives.
    pub params: Vec<Vec<u32>>,
    /// Start actions when the event fires.
    pub start: Vec<u32>,
    /// Start actions when the event is updated through its handle.
    pub update: Vec<u32>,
    /// Start actions when the event is released through its handle.
    pub release: Vec<u32>,
}

/// `.AEM` (`AE`, version 21): events to module actions.
#[derive(Clone, Debug)]
pub struct EventMap {
    pub param_actions: Vec<ParamAction>,
    pub start_actions: Vec<Target>,
    pub events: Vec<EventDef>,
    /// Global settings (volumes): the actions each one drives.
    pub globals: Vec<Vec<u32>>,
}

impl EventMap {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let b = Bytes(data);
        if data.get(..2) != Some(b"AE") {
            return Err(bad("not an AEM event map"));
        }
        let (np, ns, ne, ng) = (b.u16(8)?, b.u16(0xa)?, b.u16(0xc)?, b.u16(0xe)?);
        let (pa, sa, ev, gl) = (
            b.u32(0x10)? as usize,
            b.u32(0x14)? as usize,
            b.u32(0x18)? as usize,
            b.u32(0x1c)? as usize,
        );
        let param_actions = (0..np as usize)
            .map(|i| {
                let o = pa + 0x18 * i;
                Ok(ParamAction {
                    min: b.i32(o)?,
                    max: b.i32(o + 4)?,
                    target: Target {
                        bank: b.u16(o + 8)?,
                        module: b.u16(o + 0xa)?,
                        node: b.u32(o + 0xc)?,
                    },
                })
            })
            .collect::<Result<_>>()?;
        let start_actions = (0..ns as usize)
            .map(|i| {
                let o = sa + 0x10 * i;
                Ok(Target {
                    bank: b.u16(o)?,
                    module: b.u16(o + 2)?,
                    node: b.u32(o + 4)?,
                })
            })
            .collect::<Result<_>>()?;
        let events = (0..ne as usize)
            .map(|e| {
                let o = ev + 0x20 * e;
                let n = b.i16(o)?.max(0) as usize;
                let pp = b.u32(o + 4)? as usize;
                Ok(EventDef {
                    params: (0..n).map(|k| b.list(pp + 8 * k)).collect::<Result<_>>()?,
                    start: b.list(o + 8)?,
                    update: b.list(o + 0x10)?,
                    release: b.list(o + 0x18)?,
                })
            })
            .collect::<Result<_>>()?;
        let globals = (0..ng as usize)
            .map(|k| b.list(gl + 8 * k))
            .collect::<Result<_>>()?;
        Ok(EventMap {
            param_actions,
            start_actions,
            events,
            globals,
        })
    }
}

// ---------------------------------------------------------------------------
// The module banks (.ABK)

/// Node kinds (the byte at node + 4). The comments name the game's code.
pub mod kind {
    /// An input the event map sets (no code).
    pub const PARAM: u8 = 0x01;
    /// A trigger: a value above zero lasts one pass (`0x34c640`).
    pub const TRIGGER: u8 = 0x02;
    /// The start trigger, node 0 of every module.
    pub const START: u8 = 0x03;
    /// Plays a sound from a list (`0x34c8a8`, voices `0x34e8b0`, streams `0x34f4f0`).
    pub const PLAY: u8 = 0x04;
    pub const GATE: u8 = 0x05;
    pub const ENVELOPE: u8 = 0x06;
    pub const COUNTER: u8 = 0x07;
    pub const RANDOM: u8 = 0x08;
    pub const SELECT: u8 = 0x09;
    pub const DEMUX: u8 = 0x0a;
    pub const SUM: u8 = 0x0b;
    pub const SUB: u8 = 0x0c;
    pub const SCALE: u8 = 0x0d;
    pub const MUL: u8 = 0x0e;
    pub const DIV: u8 = 0x0f;
    pub const MOD: u8 = 0x10;
    pub const TABLE16: u8 = 0x11;
    pub const TIMER: u8 = 0x12;
    pub const DELAY: u8 = 0x13;
    pub const CONST: u8 = 0x14;
    pub const LFO: u8 = 0x15;
    pub const PASS: u8 = 0x16;
    pub const MAX: u8 = 0x17;
    pub const MIN: u8 = 0x18;
    pub const EDGE: u8 = 0x19;
    /// Ends the instance when its input rises.
    pub const END: u8 = 0x1c;
    /// Fires another event when its inputs change.
    pub const SEND: u8 = 0x1d;
    pub const LOOKUP: u8 = 0x1e;
    pub const RAMP: u8 = 0x1f;
    pub const ANY: u8 = 0x20;
}

/// A node's size in the file (`0x418694`, eight bytes per kind).
fn node_size(k: u8) -> usize {
    const SIZES: [u8; 0x21] = [
        0, 0x0c, 0x0c, 0x0c, 0x14, 0x1c, 0x18, 0x18, 0x1c, 0x10, 0x10, 0x14, 0x10, 0x14, 0x10,
        0x0c, 0x0c, 0x18, 0x10, 0x10, 0x10, 0x10, 0x0c, 0x10, 0x10, 0x14, 0, 0, 0x0c, 0x14, 0x20,
        0x0c, 0x0c,
    ];
    SIZES.get(k as usize).copied().unwrap_or(0) as usize
}

#[derive(Clone, Debug)]
pub struct Node {
    pub hash: u32,
    pub kind: u8,
    /// Input slots.
    pub inputs: u8,
    /// Per output, the (node, input slot) pairs it feeds.
    pub outputs: Vec<Vec<(u16, u16)>>,
    /// The node's own words after its header (at node + 0x0c).
    pub words: Vec<i32>,
    /// A node's table: sounds (play), values (lookups, edge), points
    /// (envelope, as ms/value pairs) or weights (random).
    pub table: Vec<i32>,
}

impl Node {
    fn w(&self, i: usize) -> i32 {
        self.words.get(i).copied().unwrap_or(0)
    }
    /// The 16-bit half `h` (0 low, 1 high) of word `i`.
    fn h(&self, i: usize, h: usize) -> i16 {
        (self.w(i) >> (16 * h)) as i16
    }
}

#[derive(Clone, Debug)]
pub struct Module {
    pub id: u16,
    /// Instances that may run at once.
    pub voices: usize,
    /// The event whose start creates instances; other events only steer them.
    pub owner: i32,
    pub nodes: Vec<Node>,
}

/// A stream a crowd bank plays from its `.AST`.
#[derive(Clone, Copy, Debug)]
pub struct StreamEntry {
    /// Byte offset of its `SCHl` in the `.AST`.
    pub offset: u32,
    pub looping: bool,
}

/// The scripts of an `.ABK` (`AB`, version 9).
#[derive(Clone, Debug)]
pub struct Program {
    pub version: u8,
    pub bank: u16,
    pub modules: Vec<Module>,
    pub streams: Vec<StreamEntry>,
}

impl Program {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let b = Bytes(data);
        if data.get(..2) != Some(b"AB") {
            return Err(bad("not an ABK bank"));
        }
        let version = b.u8(2)?;
        let count = b.i32(0xc)?.max(0) as usize;
        let at = b.u32(0x10)? as usize;
        let stride = if version >= 8 { 0x1c } else { 0x14 };
        let mut modules = Vec::with_capacity(count);
        for m in 0..count {
            let o = at + stride * m;
            let n = b.i16(o + 4)?.max(0) as usize;
            let mut p = b.u32(o + 0xc)? as usize;
            let mut nodes = Vec::with_capacity(n);
            for _ in 0..n {
                let k = b.u8(p + 4)?;
                let size = node_size(k);
                if size == 0 {
                    return Err(bad(&format!("node kind {k:#x}")));
                }
                let outs = b.u8(p + 6)? as usize;
                let io = b.u32(p + 8)? as usize;
                let outputs = (0..outs)
                    .map(|j| {
                        let (list, len) = (b.u32(io + 8 * j)? as usize, b.i16(io + 8 * j + 4)?);
                        (0..len.max(0) as usize)
                            .map(|c| Ok((b.u16(list + 4 * c)?, b.u16(list + 4 * c + 2)?)))
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<_>>()?;
                let words = (12..size)
                    .step_by(4)
                    .map(|w| b.i32(p + w))
                    .collect::<Result<Vec<_>>>()?;
                let mut node = Node {
                    hash: b.u32(p)?,
                    kind: k,
                    inputs: b.u8(p + 5)?,
                    outputs,
                    words,
                    table: Vec::new(),
                };
                node.table = table(&b, &node)?;
                nodes.push(node);
                p += size;
            }
            modules.push(Module {
                id: b.u16(o)?,
                voices: b.i16(o + 2)?.max(0) as usize,
                owner: b.i32(o + 8)?,
                nodes,
            });
        }
        // The stream table (`0x34f4f0`): twelve bytes per stream, its count
        // at 0x1a.
        let streams = match b.u32(0x34)? as usize {
            0 => Vec::new(),
            s => (0..b.i16(0x1a)?.max(0) as usize)
                .map(|k| {
                    Ok(StreamEntry {
                        offset: b.u32(s + 0xc * k + 4)?,
                        looping: b.u8(s + 0xc * k + 3)? & 1 != 0,
                    })
                })
                .collect::<Result<_>>()?,
        };
        Ok(Program {
            version,
            bank: b.u16(0x14)?,
            modules,
            streams,
        })
    }
}

/// Reads the table a node points at.
fn table(b: &Bytes, n: &Node) -> Result<Vec<i32>> {
    let at = |i: usize| n.w(i) as u32 as usize;
    Ok(match n.kind {
        kind::PLAY => {
            // A {offset, count} pair, each entry a u32 whose low half is the sound.
            let list = at(0);
            let (start, len) = (b.u32(list)? as usize, b.i16(list + 4)?);
            (0..len.max(0) as usize)
                .map(|k| b.u16(start + 4 * k).map(i32::from))
                .collect::<Result<_>>()?
        }
        kind::ENVELOPE => {
            let len = n.h(1, 1).max(0) as usize;
            (0..2 * len)
                .map(|k| b.i32(at(0) + 4 * k))
                .collect::<Result<_>>()?
        }
        kind::RANDOM if n.w(2) != 0 => {
            let len = (n.w(1) - n.w(0) + 1).max(0) as usize;
            (0..len)
                .map(|k| b.u8(at(2) + k).map(i32::from))
                .collect::<Result<_>>()?
        }
        kind::TABLE16 => {
            let len = n.h(1, 0).max(0) as usize;
            (0..len)
                .map(|k| b.i16(at(0) + 2 * k).map(i32::from))
                .collect::<Result<_>>()?
        }
        kind::EDGE => {
            let len = n.w(0).max(0) as usize;
            (0..len)
                .map(|k| b.i16(at(1) + 2 * k).map(i32::from))
                .collect::<Result<_>>()?
        }
        kind::LOOKUP => {
            let len = n.w(1).max(0) as usize;
            let size = n.w(4);
            (0..len)
                .map(|k| match size {
                    4 => b.i32(at(0) + 4 * k),
                    2 => b.i16(at(0) + 2 * k).map(i32::from),
                    _ => b.u8(at(0) + k).map(|v| i32::from(v as i8)),
                })
                .collect::<Result<_>>()?
        }
        _ => Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Voices

/// What a player node plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A sound of the bank's own `BNKl`.
    Bank(u16),
    /// A stream from the bank's `.AST`, by its byte offset.
    Stream { offset: u32, looping: bool },
}

/// A voice's settings, as the player node's inputs give them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VoiceParams {
    /// Playback rate, 0x1000 = the sound's own.
    pub pitch: u16,
    /// 0..0x7fff.
    pub volume: u16,
    /// Direction: 0 ahead, increasing to the left (0x4000 left, 0xc000
    /// right), as the game's event angles (`0x19ebe0`).
    pub azimuth: u16,
    /// How wide the sound is spread, -0x4000..0x3fff.
    pub spread: i16,
    /// Reverb send, 0..0x7fff.
    pub reverb: u16,
    /// A second volume (the bank's mix level), 0..0x7fff.
    pub level: u16,
}

impl VoiceParams {
    /// Overall gain, 0..1.
    pub fn gain(&self) -> f32 {
        f32::from(self.volume) / 32767.0 * f32::from(self.level) / 32767.0
    }

    /// Stereo pan, -1 left .. 1 right.
    pub fn pan(&self) -> f32 {
        -(f32::from(self.azimuth) / 65536.0 * std::f32::consts::TAU).sin()
    }

    /// Playback-rate multiplier.
    pub fn rate(&self) -> f32 {
        f32::from(self.pitch) / 4096.0
    }
}

/// A node of a live instance: its kind, inputs and state words.
pub type NodeView = (u8, Vec<i32>, [i32; 8]);

/// Plays what the scripts ask for. Ids are the mixer's own.
pub trait Mixer {
    /// Starts `sound` of the bank loaded in slot `bank`; `None` if it can't.
    fn start(&mut self, bank: usize, sound: Sound, params: VoiceParams) -> Option<u32>;
    fn update(&mut self, voice: u32, params: VoiceParams);
    fn stop(&mut self, voice: u32);
    /// Milliseconds played and left, or `None` once the voice has finished.
    fn position(&mut self, voice: u32) -> Option<(u32, u32)>;
}

// ---------------------------------------------------------------------------
// The runtime

/// A node's live state: its inputs, and the words the game keeps after the
/// node's pointers (`state[0]` at +0x10 is output 0, `state[1]` output 1...).
#[derive(Clone, Debug, Default)]
struct Cell {
    inputs: Vec<i32>,
    state: [i32; 8],
    /// Demux outputs; shuffle counts; edge history; delay line; sent values.
    extra: Vec<i32>,
    /// Ramp: the step and accumulator (`0x34d758`).
    step: i64,
    acc: i64,
    /// Player: the voice playing, its sound, the last trigger and the
    /// settings sent.
    voice: Option<u32>,
    sound: u16,
    trigger: i32,
    sent: VoiceParams,
}

impl Cell {
    fn output(&self, k: u8, j: usize, node: &Node) -> i32 {
        match k {
            kind::CONST => node.w(0),
            kind::DEMUX => self.extra.get(j).copied().unwrap_or(0),
            _ => self.state.get(j).copied().unwrap_or(0),
        }
    }
}

struct Instance {
    id: i32,
    cells: Vec<Cell>,
}

/// A loaded bank's runtime.
struct Slot {
    program: Program,
    /// Live instances per module.
    live: Vec<Vec<Instance>>,
}

/// Game-side handles to fired events, and the random generator.
pub struct Engine {
    map: EventMap,
    slots: Vec<Option<Slot>>,
    globals: Vec<i32>,
    serial: i32,
    rng: [u32; 6],
    /// Events nodes asked for (`kind::SEND`), fired after the pass.
    sends: VecDeque<Vec<i32>>,
}

/// Parameter actions resolved to (slot, module, node).
#[derive(Clone, Copy)]
struct Place {
    slot: usize,
    module: usize,
    node: usize,
}

impl Engine {
    pub fn new(map: EventMap, seed: u32) -> Self {
        let globals = vec![0; map.globals.len()];
        Engine {
            map,
            slots: Vec::new(),
            globals,
            serial: 0,
            rng: [seed, 0x2545_f491, 0x9e37_79b9, 0x6a09_e667, 0xbb67_ae85, 0],
            sends: VecDeque::new(),
        }
    }

    pub fn map(&self) -> &EventMap {
        &self.map
    }

    /// Loads a bank's scripts; returns its slot (the game has sixteen).
    pub fn load(&mut self, program: Program) -> usize {
        let slot = Slot {
            live: program.modules.iter().map(|_| Vec::new()).collect(),
            program,
        };
        match self.slots.iter().position(Option::is_none) {
            Some(i) => {
                self.slots[i] = Some(slot);
                i
            }
            None => {
                self.slots.push(Some(slot));
                self.slots.len() - 1
            }
        }
    }

    /// Unloads a slot, stopping its voices.
    pub fn unload(&mut self, slot: usize, mixer: &mut impl Mixer) {
        if let Some(Some(s)) = self.slots.get_mut(slot) {
            for live in &mut s.live {
                for inst in live.drain(..) {
                    stop_voices(&inst, mixer);
                }
            }
        }
        if let Some(s) = self.slots.get_mut(slot) {
            *s = None;
        }
    }

    /// The bank id loaded in each slot.
    pub fn bank(&self, slot: usize) -> Option<&Program> {
        self.slots.get(slot)?.as_ref().map(|s| &s.program)
    }

    fn place(&self, t: Target) -> Option<Place> {
        for (slot, s) in self.slots.iter().enumerate() {
            let Some(s) = s else { continue };
            if s.program.bank != t.bank {
                continue;
            }
            let (module, m) = s
                .program
                .modules
                .iter()
                .enumerate()
                .find(|(_, m)| m.id == t.module)?;
            let node = m.nodes.iter().position(|n| n.hash == t.node)?;
            return Some(Place { slot, module, node });
        }
        None
    }

    /// The game's random numbers (`0x355248`).
    fn random(&mut self) -> u32 {
        let r = &mut self.rng;
        let a = r[5].wrapping_add(r[4]);
        let c0 = u32::from(a < r[5] || a < r[4]);
        let b = a.wrapping_add(r[3]).wrapping_add(c0);
        let c = b.wrapping_add(r[2]).wrapping_add(u32::from(b < r[3]));
        r[5] = r[5].wrapping_add(1);
        let d = c.wrapping_add(r[1]).wrapping_add(u32::from(c < r[2]));
        r[0] = d.wrapping_add(r[0]).wrapping_add(u32::from(d < r[1]));
        (r[1], r[2], r[3], r[4]) = (d, c, b, a);
        // FUN_00355248 increments successive words through short-circuit
        // assignments when the counter wraps, even if an increment stops
        // the carry chain before it reaches the returned word.
        if r[5] == 0 {
            for i in (0..5).rev() {
                r[i] = r[i].wrapping_add(1);
                if r[i] != 0 {
                    break;
                }
            }
        }
        r[0]
    }

    /// Sets global `g` (volumes: see `docs/FORMATS.md`).
    pub fn set_global(&mut self, g: usize, value: i32) {
        if let Some(v) = self.globals.get_mut(g) {
            *v = value;
        }
        self.apply_global(g);
    }

    fn apply_global(&mut self, g: usize) {
        let Some(list) = self.map.globals.get(g).cloned() else {
            return;
        };
        let mut value = self.globals[g];
        for a in list {
            let Some(pa) = self.map.param_actions.get(a as usize).cloned() else {
                continue;
            };
            value = value.clamp(pa.min, pa.max.max(pa.min));
            if let Some(p) = self.place(pa.target) {
                self.write(p, None, value, true);
            }
        }
        self.globals[g] = value;
    }

    /// Writes `value` to a node's output (pulse or param) on the instances
    /// `only` names, or all of them.
    fn write(&mut self, p: Place, only: Option<i32>, value: i32, _param: bool) -> bool {
        let Some(Some(s)) = self.slots.get_mut(p.slot) else {
            return false;
        };
        let mut hit = false;
        for inst in &mut s.live[p.module] {
            if only.is_none_or(|id| inst.id == id)
                && let Some(c) = inst.cells.get_mut(p.node)
            {
                c.state[0] = value;
                hit = true;
            }
        }
        hit
    }

    /// Fires event `msg[0]` with parameters `msg[1..]`; returns its handle
    /// for [`update`](Self::update) and [`release`](Self::release).
    pub fn fire(&mut self, msg: &[i32], mixer: &mut impl Mixer) -> Option<i32> {
        let event = *msg.first()?;
        let def = self.map.events.get(usize::try_from(event).ok()?)?.clone();
        let serial = self.serial;
        self.serial = self.serial.wrapping_add(1).max(0);
        let started = self.starts(event, &def.start, serial, 0);
        let mut set = false;
        for (k, list) in def.params.iter().enumerate() {
            set |= self.params(event, list, msg.get(k + 1).copied().unwrap_or(0), serial);
        }
        for g in 0..self.globals.len() {
            self.apply_global(g);
        }
        self.run_sends(mixer);
        (started || set).then_some(serial)
    }

    /// Updates a fired event's parameters and pulses its update triggers.
    pub fn update(&mut self, handle: i32, msg: &[i32], mixer: &mut impl Mixer) {
        self.steer(handle, msg, false);
        self.run_sends(mixer);
    }

    /// Sends an event's release: parameters, then its release triggers.
    pub fn release(&mut self, handle: i32, msg: &[i32], mixer: &mut impl Mixer) {
        self.steer(handle, msg, true);
        self.run_sends(mixer);
    }

    fn steer(&mut self, handle: i32, msg: &[i32], release: bool) {
        let Some(&event) = msg.first() else { return };
        let Some(def) = usize::try_from(event)
            .ok()
            .and_then(|e| self.map.events.get(e))
            .cloned()
        else {
            return;
        };
        for (k, list) in def.params.iter().enumerate() {
            self.params(event, list, msg.get(k + 1).copied().unwrap_or(0), handle);
        }
        let list = if release { &def.release } else { &def.update };
        self.starts(event, list, handle, if release { 2 } else { 1 });
    }

    fn params(&mut self, event: i32, list: &[u32], mut value: i32, serial: i32) -> bool {
        let mut hit = false;
        for &a in list {
            let Some(pa) = self.map.param_actions.get(a as usize).cloned() else {
                continue;
            };
            value = value.clamp(pa.min, pa.max.max(pa.min));
            let Some(p) = self.place(pa.target) else {
                continue;
            };
            let owned = self.owner(p) == event;
            hit |= self.write(p, owned.then_some(serial), value, true);
        }
        hit
    }

    fn owner(&self, p: Place) -> i32 {
        self.slots[p.slot]
            .as_ref()
            .map_or(-1, |s| s.program.modules[p.module].owner)
    }

    /// The start actions of `0x34c038`: `mode` 0 fires, 1 updates, 2 releases.
    fn starts(&mut self, event: i32, list: &[u32], serial: i32, mode: u8) -> bool {
        let mut hit = false;
        for &a in list {
            let Some(t) = self.map.start_actions.get(a as usize).copied() else {
                continue;
            };
            let Some(p) = self.place(t) else { continue };
            if self.owner(p) != event {
                hit |= self.write(p, None, 1, false);
                continue;
            }
            if p.node == 0 && (mode != 0 || !self.create(p, serial)) {
                continue;
            }
            hit |= self.write(p, Some(serial), 1, false);
        }
        hit
    }

    /// A new instance of a module (`0x34d910`, `0x34e510`).
    fn create(&mut self, p: Place, id: i32) -> bool {
        let Some(Some(s)) = self.slots.get_mut(p.slot) else {
            return false;
        };
        let m = &s.program.modules[p.module];
        if s.live[p.module].len() >= m.voices {
            return false;
        }
        let mut cells: Vec<Cell> = m
            .nodes
            .iter()
            .map(|n| {
                let mut c = Cell {
                    inputs: vec![0; n.inputs as usize],
                    ..Cell::default()
                };
                match n.kind {
                    kind::DEMUX | kind::SEND => c.extra = vec![0; n.w(0).max(0) as usize],
                    kind::EDGE => c.extra = vec![0; n.w(0).max(0) as usize],
                    kind::RANDOM if n.w(2) == 0 && n.w(3) as u8 == 2 => {
                        c.extra = vec![0; (n.w(1) - n.w(0) + 1).max(0) as usize]
                    }
                    kind::DELAY => c.extra = vec![0; (n.w(0) / TICK_MS).max(0) as usize + 1],
                    kind::TIMER => c.state[1] = -1,
                    kind::COUNTER => c.state[0] = if n.w(2) as u8 == 0 { n.w(1) } else { n.w(0) },
                    _ => {}
                }
                c
            })
            .collect();
        // Constants feed their targets once.
        for n in m.nodes.iter().filter(|n| n.kind == kind::CONST) {
            for links in &n.outputs {
                for &(to, slot) in links {
                    if let Some(i) = cells
                        .get_mut(to as usize)
                        .and_then(|c| c.inputs.get_mut(slot as usize))
                    {
                        *i = n.w(0);
                    }
                }
            }
        }
        s.live[p.module].push(Instance { id, cells });
        true
    }

    /// Runs every module once (`0x34c370`); call every [`TICK_MS`].
    pub fn tick(&mut self, mixer: &mut impl Mixer) {
        for si in 0..self.slots.len() {
            let Some(mut slot) = self.slots[si].take() else {
                continue;
            };
            for (mi, m) in slot.program.modules.iter().enumerate() {
                let mut i = 0;
                while i < slot.live[mi].len() {
                    let done =
                        self.pass(si, &slot.program.streams, m, &mut slot.live[mi][i], mixer);
                    if done {
                        let inst = slot.live[mi].remove(i);
                        stop_voices(&inst, mixer);
                    } else {
                        i += 1;
                    }
                }
            }
            self.slots[si] = Some(slot);
        }
        self.run_sends(mixer);
    }

    fn run_sends(&mut self, mixer: &mut impl Mixer) {
        // A sent event may send more; the game's events are shallow, but cap it.
        for _ in 0..64 {
            let Some(msg) = self.sends.pop_front() else {
                break;
            };
            self.fire(&msg, mixer);
        }
    }

    /// One pass over an instance (`0x34c448`); true when it has ended.
    fn pass(
        &mut self,
        slot: usize,
        streams: &[StreamEntry],
        m: &Module,
        inst: &mut Instance,
        mixer: &mut impl Mixer,
    ) -> bool {
        for (ni, n) in m.nodes.iter().enumerate() {
            if n.kind == kind::CONST && n.inputs == 0 {
                continue;
            }
            let ended = self.step(slot, streams, n, &mut inst.cells[ni], mixer);
            if ended {
                return true;
            }
            for (j, links) in n.outputs.iter().enumerate() {
                let v = inst.cells[ni].output(n.kind, j, n);
                for &(to, s) in links {
                    if let Some(i) = inst
                        .cells
                        .get_mut(to as usize)
                        .and_then(|c| c.inputs.get_mut(s as usize))
                    {
                        *i = v;
                    }
                }
            }
        }
        // The last node is the end node.
        inst.cells
            .last()
            .is_some_and(|c| c.inputs.first().is_some_and(|&v| v > 0))
    }

    /// Runs one node; true if it ended the instance.
    fn step(
        &mut self,
        slot: usize,
        streams: &[StreamEntry],
        n: &Node,
        c: &mut Cell,
        mixer: &mut impl Mixer,
    ) -> bool {
        let inp = |c: &Cell, i: usize| c.inputs.get(i).copied().unwrap_or(0);
        let s = &mut c.state;
        match n.kind {
            kind::TRIGGER | kind::START => {
                if s[0] > 0 {
                    if s[1] == 0 {
                        s[1] = 1;
                    } else {
                        s[0] = 0;
                        s[1] = 0;
                    }
                }
            }
            kind::PLAY => play(slot, streams, n, c, mixer),
            kind::GATE => {
                let v = inp(c, 0);
                let s = &mut c.state;
                let count = if v < n.w(0) || v > n.w(1) {
                    s[1]
                } else if s[2] == 0 {
                    s[0] = 1;
                    s[2] = 1;
                    s[1] = 1;
                    return false;
                } else {
                    s[1]
                };
                if count < 1 {
                    if n.w(2) <= v && v <= n.w(3) && s[2] == 1 {
                        s[2] = 0;
                    } else {
                        s[0] = 0;
                    }
                } else {
                    s[1] = count - 1;
                }
            }
            kind::ENVELOPE => envelope(n, c),
            kind::COUNTER => {
                let (gate, v) = (inp(c, 0), inp(c, 1));
                let s = &mut c.state;
                if v < n.w(0) || v > n.w(1) {
                    if gate > 0 && s[1] == 0 {
                        if n.w(2) as u8 == 0 {
                            s[0] -= 1;
                            if s[0] < n.w(0) {
                                s[0] = n.w(1);
                            }
                        } else {
                            s[0] += 1;
                            if s[0] > n.w(1) {
                                s[0] = n.w(0);
                            }
                        }
                    }
                } else {
                    s[0] = v;
                }
                s[1] = gate;
            }
            kind::RANDOM => {
                let gate = inp(c, 0);
                // state[3] is the last gate.
                if gate > 0 && c.state[3] == 0 {
                    self.random_pick(n, c);
                }
                c.state[3] = gate;
            }
            kind::SELECT => {
                let k = inp(c, 0);
                c.state[0] = if k > 0 { inp(c, k as usize) } else { 0 };
            }
            kind::DEMUX => {
                let (k, v) = (inp(c, 0), inp(c, 1));
                for (i, o) in c.extra.iter_mut().enumerate() {
                    *o = if i as i32 == k - 1 { v } else { 0 };
                }
            }
            kind::SUM => {
                let sum = (0..n.w(0).max(0) as usize).fold(0i32, |a, i| a.wrapping_add(inp(c, i)));
                c.state[0] = sum.min(n.w(1));
            }
            kind::SUB => c.state[0] = (inp(c, 0) - inp(c, 1)).max(n.w(0)),
            kind::SCALE => {
                let mut v = inp(c, 0);
                for i in 1..n.w(1).max(1) as usize {
                    v = div64(i64::from(v) * i64::from(inp(c, i)), n.w(0));
                }
                c.state[0] = v;
            }
            kind::MUL => c.state[0] = inp(c, 0).wrapping_mul(inp(c, 1)).min(n.w(0)),
            kind::DIV => {
                let d = inp(c, 1);
                c.state[0] = if d > 0 { inp(c, 0) / d } else { 0 };
            }
            kind::MOD => {
                let d = inp(c, 1);
                c.state[0] = if d != 0 { inp(c, 0) % d } else { 0 };
            }
            kind::TABLE16 => {
                let (lo, hi, len) = (
                    i32::from(n.h(1, 1)),
                    i32::from(n.h(2, 0)),
                    i32::from(n.h(1, 0)),
                );
                let v = inp(c, 0).clamp(lo, hi.max(lo));
                let idx = if hi != lo {
                    (v - lo) * (len - 1) / (hi - lo)
                } else {
                    0
                };
                c.state[0] = n.table.get(idx.max(0) as usize).copied().unwrap_or(0);
            }
            kind::TIMER => timer(n, c),
            kind::DELAY => {
                let len = c.extra.len();
                if len > 0 {
                    let w = (c.state[2] as usize + 1) % len;
                    c.state[2] = w as i32;
                    c.extra[w] = i32::from(inp(c, 0) as i16);
                    let back = ((inp(c, 1) / TICK_MS).max(0) as usize).min(len - 1);
                    c.state[0] = c.extra[(w + len - back) % len];
                }
            }
            kind::LFO => lfo(n, c),
            kind::PASS => c.state[0] = inp(c, 0),
            kind::MAX => c.state[0] = (0..n.w(0).max(0) as usize).fold(0, |a, i| a.max(inp(c, i))),
            kind::MIN => {
                c.state[0] = (0..n.w(0).max(0) as usize).fold(i32::MAX, |a, i| a.min(inp(c, i)))
            }
            kind::EDGE => {
                for i in 0..c.extra.len() {
                    let v = inp(c, i);
                    if c.extra[i] < 1 && v > 0 {
                        c.state[0] = n.table.get(i).copied().unwrap_or(0);
                    }
                    c.extra[i] = i32::from(v as i16);
                }
            }
            kind::END => return inp(c, 0) > 0,
            kind::SEND => {
                let count = c.extra.len();
                let mut changed = false;
                for i in 0..count {
                    let v = inp(c, i);
                    changed |= c.extra[i] != v;
                    c.extra[i] = v;
                }
                if changed {
                    let mut msg = vec![n.w(1)];
                    msg.extend_from_slice(&c.extra);
                    self.sends.push_back(msg);
                }
            }
            kind::LOOKUP => {
                let (lo, hi) = (n.w(2), n.w(3));
                let v = inp(c, 0).clamp(lo, hi.max(lo));
                let idx = (v - lo) * n.w(1) / (hi - lo + 1).max(1);
                c.state[0] = n.table.get(idx.max(0) as usize).copied().unwrap_or(0);
            }
            kind::RAMP => ramp(c),
            kind::ANY => {
                if c.state[0] == 1 {
                    c.state[0] = 0;
                } else if n.inputs != 0 && (0..n.inputs as usize).any(|i| inp(c, i) > 0) {
                    c.state[0] = 1;
                }
            }
            _ => {}
        }
        false
    }

    /// The random node (`0x34ccd8`): weighted, uniform, or a shuffle that
    /// uses every value once before repeating.
    fn random_pick(&mut self, n: &Node, c: &mut Cell) {
        let (lo, hi) = (n.w(0), n.w(1));
        let span = (hi - lo + 1).max(1) as u32;
        match n.w(3) as u8 {
            0 => {
                let r = (self.random() % 100) as i32;
                let mut acc = 0;
                for (i, &w) in n.table.iter().enumerate() {
                    acc += w;
                    if r < acc {
                        c.state[0] = lo + i as i32;
                        break;
                    }
                }
            }
            1 => c.state[0] = (self.random() % span) as i32 + lo,
            2 if !c.extra.is_empty() => {
                // state[2] is the round; `extra` counts each value's uses. A
                // value is free while its count differs from the round
                // (`0x34ccd8`); when none is, the round moves on.
                let last = (hi - lo) as usize;
                let mut r = (self.random() % span) as usize;
                for _ in 0..=c.extra.len() + 1 {
                    let round = i32::from(c.state[2] as i16);
                    let pick = (r..=last.min(c.extra.len() - 1))
                        .chain(0..r.min(c.extra.len()))
                        .find(|&i| c.extra[i] != round);
                    if let Some(i) = pick {
                        c.extra[i] += 1;
                        c.state[0] = lo + i as i32;
                        return;
                    }
                    if c.state[0] == lo + r as i32 {
                        r += 1;
                    }
                    if hi < r as i32 {
                        r = lo.max(0) as usize;
                    }
                    c.state[2] = i32::from((c.state[2] as i16).wrapping_add(1));
                }
            }
            _ => {}
        }
    }

    /// The first live instance of a module: each node's kind, inputs and
    /// state words (for tools that trace a script).
    pub fn inspect(&self, slot: usize, module: usize) -> Option<Vec<NodeView>> {
        let s = self.slots.get(slot)?.as_ref()?;
        let inst = s.live.get(module)?.first()?;
        let nodes = &s.program.modules[module].nodes;
        Some(
            nodes
                .iter()
                .zip(&inst.cells)
                .map(|(n, c)| (n.kind, c.inputs.clone(), c.state))
                .collect(),
        )
    }

    /// Instances running, per (slot, module).
    pub fn live(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.slots.iter().enumerate().flat_map(|(si, s)| {
            s.iter().flat_map(move |s| {
                s.live
                    .iter()
                    .enumerate()
                    .map(move |(mi, l)| (si, mi, l.len()))
            })
        })
    }
}

/// The player node: the bank voice (`0x34e8b0`) or stream (`0x34f4f0`).
fn play(slot: usize, streams: &[StreamEntry], n: &Node, c: &mut Cell, mixer: &mut impl Mixer) {
    let inp = |c: &Cell, i: usize| c.inputs.get(i).copied().unwrap_or(0);
    let params = |c: &Cell| VoiceParams {
        pitch: inp(c, 0).clamp(0, 0xffff) as u16,
        volume: inp(c, 2).clamp(0, 0x7fff) as u16,
        azimuth: inp(c, 3) as u16,
        spread: inp(c, 4).clamp(-0x4000, 0x3fff) as i16,
        reverb: inp(c, 5).clamp(0, 0x7fff) as u16,
        level: if n.inputs > 10 {
            inp(c, 10).clamp(0, 0x7fff) as u16
        } else {
            0x7fff
        },
    };
    let trigger = inp(c, 7);
    if c.trigger != trigger {
        match trigger {
            1 if c.trigger == 0 => {
                let k = inp(c, 6).min(n.table.len() as i32 - 1).max(0) as usize;
                c.sound = n.table.get(k).copied().unwrap_or(0) as u16;
                let p = params(c);
                c.sent = p;
                c.voice = sound(streams, c.sound).and_then(|s| mixer.start(slot, s, p));
                c.state[0] = i32::from(c.voice.is_some());
                c.state[1] = if c.voice.is_some() { i32::MAX } else { 0 };
                c.state[2] = 0;
            }
            2 if c.trigger != 1 => {
                if let Some(i) = c.inputs.get_mut(7) {
                    *i = 0;
                }
            }
            0 => {
                if let Some(v) = c.voice.take() {
                    mixer.stop(v);
                }
                c.state[0] = 0;
                c.state[1] = 0;
            }
            _ => {}
        }
        c.trigger = inp(c, 7);
    }
    if c.trigger != 1 {
        return;
    }
    let Some(v) = c.voice else { return };
    match mixer.position(v) {
        None => {
            c.voice = None;
            c.state[0] = 0;
            c.state[1] = 0;
            c.state[2] = 0;
        }
        Some((played, left)) => {
            c.state[1] = left.min(i32::MAX as u32) as i32;
            c.state[2] = played.min(i32::MAX as u32) as i32;
            let p = params(c);
            if p != c.sent {
                c.sent = p;
                mixer.update(v, p);
            }
        }
    }
}

fn sound(streams: &[StreamEntry], id: u16) -> Option<Sound> {
    match id >> 14 {
        1 => Some(Sound::Bank(id & 0x3fff)),
        3 => {
            let e = streams.get((id & 0x3fff) as usize)?;
            Some(Sound::Stream {
                offset: e.offset,
                looping: e.looping,
            })
        }
        _ => None,
    }
}

fn stop_voices(inst: &Instance, mixer: &mut impl Mixer) {
    for c in &inst.cells {
        if let Some(v) = c.voice {
            mixer.stop(v);
        }
    }
}

fn div64(n: i64, d: i32) -> i32 {
    if d == 0 { 0 } else { (n / i64::from(d)) as i32 }
}

/// The envelope (`0x34ca68`): input 1 starts it and runs its points, 3
/// jumps to the last point (the release), 2 holds, 0 silences. It falls to
/// zero after its last running point.
fn envelope(n: &Node, c: &mut Cell) {
    let gate = c.inputs.first().copied().unwrap_or(0);
    let (running, count) = (i32::from(n.h(1, 0)), i32::from(n.h(1, 1)));
    let point = |i: i32| -> (i32, i32) {
        let i = i.max(0) as usize;
        (
            n.table.get(2 * i).copied().unwrap_or(1),
            n.table.get(2 * i + 1).copied().unwrap_or(0),
        )
    };
    // state: [0] out, [1] ms into the segment, [2] the segment, [3] the
    // value it started from; the last gate is kept in `step`.
    let prev = c.step as i32;
    let s = &mut c.state;
    let run = match gate {
        1 if prev == 0 => {
            s[1] = 0;
            s[2] = 0;
            s[0] = n.w(2);
            s[3] = n.w(2);
            false
        }
        3 if prev != 3 && s[2] < count - 1 => {
            s[1] = 0;
            s[2] = count - 1;
            s[0] = point(count - 2).1;
            s[3] = s[0];
            false
        }
        1 | 3 => true,
        2 => false,
        _ => {
            s[0] = 0;
            false
        }
    };
    if run {
        if s[2] < running {
            s[1] += TICK_MS;
            let (ms, to) = point(s[2]);
            if s[1] < ms {
                let frac = (i64::from(s[1]) << 32) / i64::from(ms.max(1));
                s[0] = s[3] + ((frac * i64::from(to - s[3])) >> 32) as i32;
            } else {
                s[1] = 0;
                s[3] = to;
                s[2] += 1;
                s[0] = if s[2] < running { to } else { 0 };
            }
        } else {
            s[0] = 0;
        }
    }
    c.step = i64::from(gate);
}

/// The timer (`0x34d300`): input 0 rising restarts it; output 0 pulses
/// once input 1 ms have passed (or the node's own limit), then it idles.
fn timer(n: &Node, c: &mut Cell) {
    let (gate, len) = (
        c.inputs.first().copied().unwrap_or(0),
        c.inputs.get(1).copied().unwrap_or(0),
    );
    // state: [0] out, [1] ms since the start (-1 idle), [2] the last gate.
    let s = &mut c.state;
    if gate > 0 && s[2] == 0 {
        s[1] = 0;
    }
    let t = s[1];
    let due = t >= len || t > n.w(0);
    if due && s[0] == 0 {
        s[0] = 1;
    } else {
        s[0] = 0;
        if len < t || n.w(0) < t {
            s[1] = -1;
        }
    }
    s[2] = gate;
    if s[1] >= 0 {
        s[1] += TICK_MS;
    }
}

/// The LFO (`0x34c678`): input 0 the period in ms, input 1 the depth.
fn lfo(n: &Node, c: &mut Cell) {
    let inp = |i: usize| c.inputs.get(i).copied().unwrap_or(0);
    if n.inputs == 2 {
        let period = inp(0).max(1);
        c.state[1] = ((TICK_MS << 16) / period).min(0xffff);
    }
    let depth = inp(1);
    let phase = (c.state[2] as u32 + (c.state[1] as u32 & 0xffff)) & 0xffff;
    c.state[2] = phase as i32;
    let (wave, uni) = (n.h(0, 0), n.h(0, 1) == 1);
    c.state[0] = match wave {
        0 => {
            let sin = ((phase as f64 / 65536.0 * std::f64::consts::TAU).sin() * 65536.0) as i64;
            let v = ((sin * i64::from(depth)) >> 16) as i32;
            if uni { (v + depth) >> 1 } else { v }
        }
        1 => {
            if phase > 0x7fff {
                depth
            } else if uni {
                0
            } else {
                -depth
            }
        }
        2 if uni => ((u64::from(phase) * depth as u64) >> 16) as i32,
        2 => (((u64::from(phase) * depth as u64) >> 15) as i32).wrapping_sub(depth),
        3 => {
            let tri = if phase < 0x8000 {
                phase
            } else {
                0x10000 - phase
            } as i64
                * i64::from(depth);
            if uni {
                (tri >> 15) as i32
            } else {
                ((tri >> 14) as i32) - depth
            }
        }
        _ => c.state[0],
    };
}

/// The ramp (`0x34d758`): glides output 0 to input 2 over input 0 ms, at a
/// speed input 1 scales (0x1000 = as set).
fn ramp(c: &mut Cell) {
    let inp = |c: &Cell, i: usize| c.inputs.get(i).copied().unwrap_or(0);
    let (time, speed, target) = (inp(c, 0), inp(c, 1), inp(c, 2));
    let out = c.state[0];
    if target == out {
        return;
    }
    // state: [1] the time set, [2] the start, [3] the target set.
    if target == c.state[3] && time == c.state[1] {
        c.acc += c.step.wrapping_mul(i64::from(speed));
        let v = (c.acc >> 36) as i32;
        c.state[0] = if target <= out {
            v.max(target)
        } else {
            v.min(target)
        };
        return;
    }
    c.state[2] = out;
    c.acc = i64::from(out) << 36;
    c.state[3] = target;
    c.state[1] = time;
    if time < 1 {
        c.state[0] = target;
    } else {
        c.step = ((i64::from(target - out) * i64::from(TICK_MS)) << 24) / i64::from(time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_sizes_cover_the_kinds_the_banks_use() {
        for k in [
            1u8, 2, 3, 4, 5, 6, 7, 8, 9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x10, 0x11, 0x12, 0x13, 0x14,
        ] {
            assert!(node_size(k) >= 12, "kind {k:#x}");
        }
        assert_eq!(node_size(kind::LOOKUP), 0x20);
        assert_eq!(node_size(0x1a), 0);
    }

    #[test]
    fn the_ramp_glides_to_its_target() {
        let mut c = Cell {
            inputs: vec![100, 0x1000, 1000],
            ..Cell::default()
        };
        ramp(&mut c);
        for _ in 0..5 {
            ramp(&mut c);
        }
        assert_eq!(c.state[0], 500, "halfway after 50 of 100 ms");
        for _ in 0..10 {
            ramp(&mut c);
        }
        assert_eq!(c.state[0], 1000);
    }

    #[test]
    fn the_random_number_generator_moves() {
        let mut e = Engine::new(
            EventMap {
                param_actions: vec![],
                start_actions: vec![],
                events: vec![],
                globals: vec![],
            },
            1,
        );
        let a: Vec<u32> = (0..4).map(|_| e.random()).collect();
        assert!(a.windows(2).all(|w| w[0] != w[1]));
        e.rng = [5, 4, 3, 2, 1, u32::MAX];
        assert_eq!(e.random(), 15);
        assert_eq!(e.rng, [15, 10, 6, 3, 1, 0]);
        assert_eq!(e.random(), 35);
    }
}
