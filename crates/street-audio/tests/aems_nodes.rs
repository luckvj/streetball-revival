//! Synthetic tests for the AEMS sound-script engine (`aems.rs`).
//!
//! Small `.ABK` and `.AEM` blobs built here drive events through modules
//! without the disc. [`AbkBuilder`] lays out a module bank, [`AemBuilder`]
//! an event map, and [`FakeMixer`] records what the scripts ask to play.

use std::collections::HashSet;

use street_audio::aems::{Engine, EventMap, Mixer, Program, Sound, VoiceParams, kind};

fn set_u16(buf: &mut [u8], at: usize, v: u16) {
    buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn set_i16(buf: &mut [u8], at: usize, v: i16) {
    buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn set_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn set_i32(buf: &mut [u8], at: usize, v: i32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// A node's table: the sounds a PLAY node picks from, or an ENVELOPE's
/// (ms, value) points.
enum Table {
    None,
    Sounds(Vec<u16>),
    Points(Vec<i32>),
}

struct NodeBuild {
    hash: u32,
    kind: u8,
    inputs: u8,
    /// Per output, the (node, input) pairs it feeds.
    outputs: Vec<Vec<(u16, u16)>>,
    /// Words after the table-offset word (`PLAY`, `ENVELOPE`), else all words.
    words: Vec<i32>,
    table: Table,
}

impl NodeBuild {
    fn plain(hash: u32, kind: u8, inputs: u8) -> Self {
        NodeBuild {
            hash,
            kind,
            inputs,
            outputs: Vec::new(),
            words: Vec::new(),
            table: Table::None,
        }
    }

    fn constant(hash: u32, value: i32, to: Vec<(u16, u16)>) -> Self {
        NodeBuild {
            hash,
            kind: kind::CONST,
            inputs: 0,
            outputs: vec![to],
            words: vec![value],
            table: Table::None,
        }
    }
}

struct ModuleBuild {
    id: u16,
    voices: i16,
    owner: i32,
    nodes: Vec<NodeBuild>,
}

/// File size of the node kinds used here (the same table `aems.rs` reads).
fn node_size(kind: u8) -> usize {
    match kind {
        kind::PARAM | kind::TRIGGER | kind::START | kind::END => 0x0c,
        kind::CONST => 0x10,
        kind::PLAY => 0x14,
        kind::ENVELOPE => 0x18,
        kind::RANDOM => 0x1c,
        _ => panic!("test node kind {kind:#x} has no size"),
    }
}

/// A module bank (`.ABK`, version 9): a header, the module table, the
/// nodes, then the connection lists and node tables they point at.
struct AbkBuilder {
    bank: u16,
    modules: Vec<ModuleBuild>,
}

impl AbkBuilder {
    fn build(self) -> Vec<u8> {
        let mut buf = vec![0u8; 0x38];
        buf[0] = b'A';
        buf[1] = b'B';
        buf[2] = 9;
        set_i32(&mut buf, 0xc, self.modules.len() as i32);
        set_u32(&mut buf, 0x10, 0x38);
        set_u16(&mut buf, 0x14, self.bank);
        // Stream count (0x1a) and stream table (0x34) stay zero: no streams.
        let mut at = 0x38 + 0x1c * self.modules.len();
        let mut node_at = Vec::new();
        for m in &self.modules {
            let mut offs = Vec::new();
            for n in &m.nodes {
                offs.push(at);
                at += node_size(n.kind);
            }
            node_at.push(offs);
        }
        // Connection lists and tables, appended after the nodes. Every
        // offset below is absolute: `base` plus the position in `aux`.
        let base = at;
        let mut aux = Vec::new();
        let mut fix = Vec::new();
        for m in &self.modules {
            let mut row = Vec::new();
            for n in &m.nodes {
                let io = if n.outputs.is_empty() {
                    None
                } else {
                    let io = base + aux.len();
                    aux.resize(aux.len() + 8 * n.outputs.len(), 0);
                    for (j, links) in n.outputs.iter().enumerate() {
                        let list = (base + aux.len()) as u32;
                        for &(node, input) in links {
                            aux.extend_from_slice(&node.to_le_bytes());
                            aux.extend_from_slice(&input.to_le_bytes());
                        }
                        set_u32(&mut aux, io - base + 8 * j, list);
                        set_i16(&mut aux, io - base + 8 * j + 4, links.len() as i16);
                    }
                    Some(io)
                };
                let list = match &n.table {
                    Table::None => None,
                    Table::Sounds(sounds) => {
                        let list = base + aux.len();
                        aux.resize(aux.len() + 8, 0);
                        let data = (base + aux.len()) as u32;
                        for &s in sounds {
                            aux.extend_from_slice(&s.to_le_bytes());
                            aux.extend_from_slice(&0u16.to_le_bytes());
                        }
                        set_u32(&mut aux, list - base, data);
                        set_i16(&mut aux, list - base + 4, sounds.len() as i16);
                        Some(list)
                    }
                    Table::Points(points) => {
                        let data = base + aux.len();
                        for &p in points {
                            aux.extend_from_slice(&p.to_le_bytes());
                        }
                        Some(data)
                    }
                };
                row.push((io, list));
            }
            fix.push(row);
        }
        buf.resize(at, 0);
        for (mi, m) in self.modules.iter().enumerate() {
            let o = 0x38 + 0x1c * mi;
            set_u16(&mut buf, o, m.id);
            set_i16(&mut buf, o + 2, m.voices);
            set_i16(&mut buf, o + 4, m.nodes.len() as i16);
            set_i32(&mut buf, o + 8, m.owner);
            set_u32(&mut buf, o + 0xc, node_at[mi][0] as u32);
            for (ni, n) in m.nodes.iter().enumerate() {
                let p = node_at[mi][ni];
                let (io, list) = fix[mi][ni];
                set_u32(&mut buf, p, n.hash);
                buf[p + 4] = n.kind;
                buf[p + 5] = n.inputs;
                buf[p + 6] = n.outputs.len() as u8;
                set_u32(&mut buf, p + 8, io.unwrap_or(0) as u32);
                let words = match &n.table {
                    Table::None => n.words.clone(),
                    Table::Sounds(_) | Table::Points(_) => {
                        let mut w = vec![list.unwrap() as i32];
                        w.extend_from_slice(&n.words);
                        w
                    }
                };
                assert_eq!(words.len(), (node_size(n.kind) - 12) / 4);
                for (k, &w) in words.iter().enumerate() {
                    set_i32(&mut buf, p + 12 + 4 * k, w);
                }
            }
        }
        buf.extend_from_slice(&aux);
        buf
    }
}

struct EventBuild {
    params: Vec<Vec<u32>>,
    start: Vec<u32>,
    update: Vec<u32>,
    release: Vec<u32>,
}

/// An event map (`.AEM`): the header, the action tables and events, then
/// the index lists they point at.
struct AemBuilder {
    /// (min, max, bank, module, node hash) per parameter action.
    params: Vec<(i32, i32, u16, u16, u32)>,
    /// (bank, module, node hash) per start action.
    starts: Vec<(u16, u16, u32)>,
    events: Vec<EventBuild>,
    /// Parameter-action indices per global.
    globals: Vec<Vec<u32>>,
}

impl AemBuilder {
    fn build(self) -> Vec<u8> {
        let mut fixed = vec![0u8; 0x20];
        fixed[0] = b'A';
        fixed[1] = b'E';
        fixed[2] = 0x15;
        set_u32(&mut fixed, 4, 1);
        set_u16(&mut fixed, 8, self.params.len() as u16);
        set_u16(&mut fixed, 0xa, self.starts.len() as u16);
        set_u16(&mut fixed, 0xc, self.events.len() as u16);
        set_u16(&mut fixed, 0xe, self.globals.len() as u16);
        let pa = 0x20;
        let sa = pa + 0x18 * self.params.len();
        let ev = sa + 0x10 * self.starts.len();
        set_u32(&mut fixed, 0x10, pa as u32);
        set_u32(&mut fixed, 0x14, sa as u32);
        set_u32(&mut fixed, 0x18, ev as u32);
        fixed.resize(ev + 0x20 * self.events.len(), 0);
        for (i, &(min, max, bank, module, node)) in self.params.iter().enumerate() {
            let o = pa + 0x18 * i;
            set_i32(&mut fixed, o, min);
            set_i32(&mut fixed, o + 4, max);
            set_u16(&mut fixed, o + 8, bank);
            set_u16(&mut fixed, o + 0xa, module);
            set_u32(&mut fixed, o + 0xc, node);
        }
        for (i, &(bank, module, node)) in self.starts.iter().enumerate() {
            let o = sa + 0x10 * i;
            set_u16(&mut fixed, o, bank);
            set_u16(&mut fixed, o + 2, module);
            set_u32(&mut fixed, o + 4, node);
        }
        // Index lists, appended after the events; every offset is absolute.
        let mut aux = Vec::new();
        let array = |aux: &mut Vec<u8>, base: usize, items: &[u32]| -> (u32, i16) {
            let off = base + aux.len();
            for &a in items {
                aux.extend_from_slice(&a.to_le_bytes());
            }
            (off as u32, items.len() as i16)
        };
        let base = fixed.len();
        for (e, event) in self.events.iter().enumerate() {
            let o = ev + 0x20 * e;
            set_i16(&mut fixed, o, event.params.len() as i16);
            // The per-parameter directory lives in aux too.
            let pp = base + aux.len();
            aux.resize(aux.len() + 8 * event.params.len(), 0);
            for (k, items) in event.params.iter().enumerate() {
                let (off, n) = array(&mut aux, base, items);
                set_u32(&mut aux, pp - base + 8 * k, off);
                set_i16(&mut aux, pp - base + 8 * k + 4, n);
            }
            set_u32(&mut fixed, o + 4, pp as u32);
            for (dir, items) in [
                (o + 8, &event.start),
                (o + 0x10, &event.update),
                (o + 0x18, &event.release),
            ] {
                let (off, n) = array(&mut aux, base, items);
                set_u32(&mut fixed, dir, off);
                set_i16(&mut fixed, dir + 4, n);
            }
        }
        let gl = base + aux.len();
        set_u32(&mut fixed, 0x1c, gl as u32);
        aux.resize(aux.len() + 8 * self.globals.len(), 0);
        for (k, items) in self.globals.iter().enumerate() {
            let (off, n) = array(&mut aux, base, items);
            set_u32(&mut aux, gl - base + 8 * k, off);
            set_i16(&mut aux, gl - base + 8 * k + 4, n);
        }
        fixed.extend_from_slice(&aux);
        fixed
    }
}

/// Records what the scripts ask to play. Every voice stays playing until
/// it is stopped, reporting a fixed position meanwhile.
#[derive(Default)]
struct FakeMixer {
    starts: Vec<(usize, Sound, VoiceParams)>,
    updates: Vec<(u32, VoiceParams)>,
    stops: Vec<u32>,
    live: HashSet<u32>,
    next: u32,
}

impl Mixer for FakeMixer {
    fn start(&mut self, bank: usize, sound: Sound, params: VoiceParams) -> Option<u32> {
        self.next += 1;
        let id = self.next;
        self.live.insert(id);
        self.starts.push((bank, sound, params));
        Some(id)
    }

    fn update(&mut self, voice: u32, params: VoiceParams) {
        self.updates.push((voice, params));
    }

    fn stop(&mut self, voice: u32) {
        self.live.remove(&voice);
        self.stops.push(voice);
    }

    fn position(&mut self, voice: u32) -> Option<(u32, u32)> {
        self.live.contains(&voice).then_some((50, 1000))
    }
}

/// Instances running over all slots and modules.
fn live_count(engine: &Engine) -> usize {
    engine.live().map(|(_, _, n)| n).sum()
}

const BANK: u16 = 5;
const MODULE: u16 = 10;

const H_START: u32 = 0x101;
const H_PITCH: u32 = 0x102;
const H_VOL: u32 = 0x103;
const H_INDEX: u32 = 0x104;
const H_TRIG: u32 = 0x105;
const H_PLAY: u32 = 0x106;
const H_END: u32 = 0x107;
const H_PARAM: u32 = 0x108;
const H_RANDOM: u32 = 0x109;
const H_ENV: u32 = 0x10a;

/// A module that plays bank sound `sound` with fixed settings: node 0 is
/// START, constants feed the PLAY node, and an unfed END node closes it.
fn play_module(owner: i32, sound: u16) -> ModuleBuild {
    const PLAY: u16 = 5;
    ModuleBuild {
        id: MODULE,
        voices: 4,
        owner,
        nodes: vec![
            NodeBuild::plain(H_START, kind::START, 0),
            NodeBuild::constant(H_PITCH, 0x1000, vec![(PLAY, 0)]),
            NodeBuild::constant(H_VOL, 0x7fff, vec![(PLAY, 2)]),
            NodeBuild::constant(H_INDEX, 0, vec![(PLAY, 6)]),
            NodeBuild::constant(H_TRIG, 1, vec![(PLAY, 7)]),
            NodeBuild {
                hash: H_PLAY,
                kind: kind::PLAY,
                inputs: 8,
                outputs: Vec::new(),
                words: vec![0],
                table: Table::Sounds(vec![sound]),
            },
            NodeBuild::plain(H_END, kind::END, 1),
        ],
    }
}

fn engine_for(abk: Vec<u8>, aem: Vec<u8>) -> (Engine, FakeMixer) {
    let map = EventMap::parse(&aem).expect("event map parses");
    let program = Program::parse(&abk).expect("bank parses");
    let mut engine = Engine::new(map, 7);
    engine.load(program);
    (engine, FakeMixer::default())
}

// One event whose start creates a module instance: constants feed the
// PLAY node, which starts bank sound 3 at full pitch and volume.
#[test]
fn start_action_plays_a_bank_sound() {
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![play_module(0, 0x4003)],
    }
    .build();
    let aem = AemBuilder {
        params: Vec::new(),
        starts: vec![(BANK, MODULE, H_START)],
        events: vec![EventBuild {
            params: Vec::new(),
            start: vec![0],
            update: Vec::new(),
            release: Vec::new(),
        }],
        globals: Vec::new(),
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    assert!(engine.fire(&[0], &mut mix).is_some());
    engine.tick(&mut mix);
    assert_eq!(live_count(&engine), 1);
    assert_eq!(
        mix.starts.as_slice(),
        &[(
            0,
            Sound::Bank(3),
            VoiceParams {
                pitch: 0x1000,
                volume: 0x7fff,
                azimuth: 0,
                spread: 0,
                reverb: 0,
                level: 0x7fff,
            },
        )],
    );
}

// An END node fed by a constant 1 removes the instance after one tick,
// stopping the voice it started.
#[test]
fn end_node_fed_by_one_kills_the_instance() {
    const PLAY: u16 = 5;
    const END: u16 = 6;
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![ModuleBuild {
            id: MODULE,
            voices: 4,
            owner: 0,
            nodes: vec![
                NodeBuild::plain(H_START, kind::START, 0),
                NodeBuild::constant(H_PITCH, 0x1000, vec![(PLAY, 0)]),
                NodeBuild::constant(H_VOL, 0x7fff, vec![(PLAY, 2)]),
                NodeBuild::constant(H_INDEX, 0, vec![(PLAY, 6)]),
                NodeBuild::constant(H_TRIG, 1, vec![(PLAY, 7), (END, 0)]),
                NodeBuild {
                    hash: H_PLAY,
                    kind: kind::PLAY,
                    inputs: 8,
                    outputs: Vec::new(),
                    words: vec![0],
                    table: Table::Sounds(vec![0x4000]),
                },
                NodeBuild::plain(H_END, kind::END, 1),
            ],
        }],
    }
    .build();
    let aem = AemBuilder {
        params: Vec::new(),
        starts: vec![(BANK, MODULE, H_START)],
        events: vec![EventBuild {
            params: Vec::new(),
            start: vec![0],
            update: Vec::new(),
            release: Vec::new(),
        }],
        globals: Vec::new(),
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    assert!(engine.fire(&[0], &mut mix).is_some());
    engine.tick(&mut mix);
    assert_eq!(live_count(&engine), 0);
    assert_eq!(mix.starts.len(), 1, "the voice started first");
    assert_eq!(mix.stops.len(), 1, "then it was stopped");
}

// A parameter action clamps the event's value before it reaches a PARAM node.
#[test]
fn parameter_actions_clamp() {
    for (value, want) in [(100, 20), (-100, 10), (15, 15)] {
        let abk = AbkBuilder {
            bank: BANK,
            modules: vec![ModuleBuild {
                id: MODULE,
                voices: 4,
                owner: 0,
                nodes: vec![
                    NodeBuild::plain(H_START, kind::START, 0),
                    NodeBuild::plain(H_PARAM, kind::PARAM, 0),
                ],
            }],
        }
        .build();
        let aem = AemBuilder {
            params: vec![(10, 20, BANK, MODULE, H_PARAM)],
            starts: vec![(BANK, MODULE, H_START)],
            events: vec![EventBuild {
                params: vec![vec![0]],
                start: vec![0],
                update: Vec::new(),
                release: Vec::new(),
            }],
            globals: Vec::new(),
        }
        .build();
        let (mut engine, mut mix) = engine_for(abk, aem);
        assert!(engine.fire(&[0, value], &mut mix).is_some());
        let view = engine.inspect(0, 0).expect("an instance runs");
        assert_eq!(view[1].0, kind::PARAM);
        assert_eq!(view[1].2[0], want, "event value {value}");
    }
}

// A RANDOM node in shuffle mode gives every value once before repeating.
#[test]
fn shuffle_random_covers_its_range() {
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![ModuleBuild {
            id: MODULE,
            voices: 4,
            owner: 0,
            nodes: vec![
                NodeBuild::plain(H_START, kind::START, 0),
                NodeBuild {
                    hash: H_TRIG,
                    kind: kind::TRIGGER,
                    inputs: 0,
                    outputs: vec![vec![(2, 0)]],
                    words: Vec::new(),
                    table: Table::None,
                },
                NodeBuild {
                    hash: H_RANDOM,
                    kind: kind::RANDOM,
                    inputs: 1,
                    outputs: Vec::new(),
                    words: vec![5, 8, 0, 2],
                    table: Table::None,
                },
                NodeBuild::plain(H_END, kind::END, 1),
            ],
        }],
    }
    .build();
    let aem = AemBuilder {
        params: Vec::new(),
        starts: vec![(BANK, MODULE, H_START), (BANK, MODULE, H_TRIG)],
        events: vec![EventBuild {
            params: Vec::new(),
            start: vec![0],
            update: vec![1],
            release: Vec::new(),
        }],
        globals: Vec::new(),
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    let handle = engine.fire(&[0], &mut mix).expect("an instance starts");
    engine.tick(&mut mix);
    engine.tick(&mut mix);
    let mut picks = Vec::new();
    for _ in 0..4 {
        // Each update pulses the trigger; two ticks settle it high, then low.
        engine.update(handle, &[0], &mut mix);
        engine.tick(&mut mix);
        engine.tick(&mut mix);
        let view = engine.inspect(0, 0).expect("the instance runs");
        picks.push(view[2].2[0]);
    }
    picks.sort_unstable();
    assert_eq!(picks, vec![5, 6, 7, 8]);
}

// An ENVELOPE through (100 ms, 1000), (100 ms, 0) rises halfway after
// 50 ms of ticks, then falls back.
#[test]
fn envelope_rises_then_falls() {
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![ModuleBuild {
            id: MODULE,
            voices: 4,
            owner: 0,
            nodes: vec![
                NodeBuild::plain(H_START, kind::START, 0),
                NodeBuild::constant(H_TRIG, 1, vec![(2, 0)]),
                NodeBuild {
                    hash: H_ENV,
                    kind: kind::ENVELOPE,
                    inputs: 1,
                    outputs: Vec::new(),
                    words: vec![(2 << 16) | 2, 0],
                    table: Table::Points(vec![100, 1000, 100, 0]),
                },
                NodeBuild::plain(H_END, kind::END, 1),
            ],
        }],
    }
    .build();
    let aem = AemBuilder {
        params: Vec::new(),
        starts: vec![(BANK, MODULE, H_START)],
        events: vec![EventBuild {
            params: Vec::new(),
            start: vec![0],
            update: Vec::new(),
            release: Vec::new(),
        }],
        globals: Vec::new(),
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    assert!(engine.fire(&[0], &mut mix).is_some());
    engine.tick(&mut mix);
    for _ in 0..5 {
        engine.tick(&mut mix);
    }
    let view = engine.inspect(0, 0).expect("the instance runs");
    let peak = view[2].2[0];
    assert!((peak - 500).abs() < 60, "about 500 after 50 ms, got {peak}");
    for _ in 0..11 {
        engine.tick(&mut mix);
    }
    let view = engine.inspect(0, 0).expect("the instance runs");
    assert!(view[2].2[0] < peak, "it falls again");
}

// A global drives a PARAM node through the map's globals list, clamped.
#[test]
fn globals_drive_params() {
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![ModuleBuild {
            id: MODULE,
            voices: 4,
            owner: 0,
            nodes: vec![
                NodeBuild::plain(H_START, kind::START, 0),
                NodeBuild::plain(H_PARAM, kind::PARAM, 0),
            ],
        }],
    }
    .build();
    let aem = AemBuilder {
        params: vec![(0, 100, BANK, MODULE, H_PARAM)],
        starts: vec![(BANK, MODULE, H_START)],
        events: vec![EventBuild {
            params: Vec::new(),
            start: vec![0],
            update: Vec::new(),
            release: Vec::new(),
        }],
        globals: vec![vec![0]],
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    assert!(engine.fire(&[0], &mut mix).is_some());
    engine.set_global(0, 1000);
    let view = engine.inspect(0, 0).expect("the instance runs");
    assert_eq!(view[1].2[0], 100);
    engine.set_global(0, -50);
    let view = engine.inspect(0, 0).expect("the instance runs");
    assert_eq!(view[1].2[0], 0);
}

// A module owned by another event is never created by an event that
// names it; firing that event only pulses its live instances.
#[test]
fn foreign_modules_are_pulsed_not_created() {
    let abk = AbkBuilder {
        bank: BANK,
        modules: vec![ModuleBuild {
            id: MODULE,
            voices: 4,
            owner: 1,
            nodes: vec![
                NodeBuild::plain(H_START, kind::START, 0),
                NodeBuild::plain(H_TRIG, kind::TRIGGER, 0),
                NodeBuild::plain(H_END, kind::END, 1),
            ],
        }],
    }
    .build();
    let aem = AemBuilder {
        params: Vec::new(),
        starts: vec![(BANK, MODULE, H_START), (BANK, MODULE, H_TRIG)],
        events: vec![
            EventBuild {
                params: Vec::new(),
                start: vec![1],
                update: Vec::new(),
                release: Vec::new(),
            },
            EventBuild {
                params: Vec::new(),
                start: vec![0],
                update: Vec::new(),
                release: Vec::new(),
            },
        ],
        globals: Vec::new(),
    }
    .build();
    let (mut engine, mut mix) = engine_for(abk, aem);
    engine.fire(&[0], &mut mix);
    assert_eq!(live_count(&engine), 0, "no instance without its owner");
    engine.fire(&[1], &mut mix);
    assert_eq!(live_count(&engine), 1);
    engine.fire(&[0], &mut mix);
    assert_eq!(live_count(&engine), 1, "still one instance");
    let view = engine.inspect(0, 0).expect("the instance runs");
    assert_eq!(view[1].0, kind::TRIGGER);
    assert_eq!(view[1].2[0], 1, "the trigger was pulsed");
}
