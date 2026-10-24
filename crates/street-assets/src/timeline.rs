//! Staged actor, camera, ball and sound timelines from `/SCRIPTS/SCRIPTS.BIN`.
//!
//! The `ssb\x11` reader is `FUN_00336510`; the bank-name/offset table is
//! `0x416858` in SLUS-20651. A category is a record layout, not an event
//! identifier. In particular, `FUN_003355c8` dispatches category 1 by byte
//! 0x21 of its payload; only dispatch 2 reaches the AEMS listener.

use std::io::{Read, Seek};

use crate::{Error, Result, bytes, iso::Iso};

/// Executable animation record, with the game's own mirror flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub name: Option<String>,
    pub mirrored: bool,
}

#[derive(Clone, Debug)]
pub struct ScriptBank {
    pub scripts: Vec<Timeline>,
    pub animations: Vec<Animation>,
    pub finishers: Vec<crate::scripts::Finisher>,
    /// The 13 four-entry variant groups at 0x3db110: empty group 0,
    /// long shots 1..4, short shots 5..8, paired dunk sequences 9..12.
    pub gamebreakers: [[Option<usize>; 4]; 13],
    gamebreaker_regions: Vec<Vec<u8>>,
    /// Ordered row pairs/root-distance bits built by original 0032c0a0.
    staged_loop_distances: Vec<(u16, u16, u32)>,
}

impl ScriptBank {
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        Self::parse(
            &iso.read("/SCRIPTS/SCRIPTS.BIN")?,
            &iso.read("/SLUS_206.51")?,
        )
    }

    pub fn parse(data: &[u8], executable: &[u8]) -> Result<Self> {
        let elf = Elf::parse(executable)?;
        let mut scripts = parse_timelines(data)?;
        if scripts.len() != 70 {
            return Err(Error::format("expected 70 US PS2 staged scripts"));
        }
        for (i, script) in scripts.iter_mut().enumerate() {
            let entry = 0x416858 + i as u32 * 8;
            if elf.word(entry + 4)? != script.offset {
                return Err(Error::format(
                    "staged script offset disagrees with executable",
                ));
            }
            script.name = elf.string(elf.word(entry)?)?;
        }
        let animations = records(&elf)?;
        let mut gamebreakers = [[None; 4]; 13];
        for (family, group) in gamebreakers.iter_mut().enumerate() {
            for (variant, slot) in group.iter_mut().enumerate() {
                let offset = elf.word(0x3db110 + family as u32 * 32 + variant as u32 * 8)?;
                if offset != u32::MAX {
                    *slot = Some(scripts.iter().position(|s| s.offset == offset).ok_or_else(
                        || Error::format("gamebreaker points outside staged script bank"),
                    )?);
                }
            }
        }
        // FUN_00141298 expands one RLE row selected from this 320x206
        // location map. Several original rows leave a few far-edge columns
        // unwritten; retain that absence instead of making up a region.
        let mut gamebreaker_regions = Vec::with_capacity(206);
        let mut at = 0x3d2790;
        for _ in 0..206 {
            let mut row = Vec::new();
            loop {
                let pair = elf.at(at, 2)?;
                at += 2;
                if pair == [0, 0] {
                    break;
                }
                let count = usize::from(pair[0]);
                if pair[1] > 12 || row.len() + count > 320 {
                    return Err(Error::format("invalid gamebreaker region RLE"));
                }
                row.resize(row.len() + count, pair[1]);
            }
            gamebreaker_regions.push(row);
        }
        Ok(Self {
            scripts,
            animations,
            finishers: crate::scripts::finishers_from_executable(executable)?,
            gamebreakers,
            gamebreaker_regions,
            staged_loop_distances: staged_loop_distances(&elf)?,
        })
    }

    pub fn named(&self, name: &str) -> Option<&Timeline> {
        self.scripts.iter().find(|script| script.name == name)
    }

    /// Original position-to-sequence family lookup (`FUN_00141298`). A
    /// zero family uses the ordinary Gamebreaker camera rather than SSB.
    /// Court x is reflected for team 0 before mapping into the shipped RLE.
    pub fn gamebreaker_family(&self, position: [f32; 2], team: usize) -> Option<u8> {
        if !position.iter().all(|v| v.is_finite()) {
            return None;
        }
        let x = if team == 0 { -position[0] } else { position[0] };
        let col = (((-x + 42.0) / 84.0) * 320.0)
            .round_ties_even()
            .clamp(0.0, 319.0) as usize;
        let row = (((-position[1] + 26.5) / 53.0) * 206.0)
            .round_ties_even()
            .clamp(0.0, 205.0) as usize;
        self.gamebreaker_regions.get(row)?.get(col).copied()
    }

    pub fn gamebreaker_region_rows(&self) -> &[Vec<u8>] {
        &self.gamebreaker_regions
    }

    /// Mirror the animation record using the original adjacent-pair rule
    /// (`FUN_00192888`), rather than reflecting an arbitrary named clip.
    pub fn animation(&self, id: u16, mirror: bool) -> Option<&Animation> {
        self.animations.get(self.animation_id(id, mirror)?)
    }

    fn animation_id(&self, id: u16, mirror: bool) -> Option<usize> {
        let id = usize::from(id);
        let anim = self.animations.get(id)?;
        let id = if mirror && anim.mirrored {
            id.saturating_sub(1)
        } else if mirror && self.animations.get(id + 1).is_some_and(|a| a.mirrored) {
            id + 1
        } else {
            id
        };
        Some(id)
    }

    /// First matching row in the native staged table, including its actual
    /// adjacent mirrored row. Missing rows retain the configured rate.
    pub fn staged_loop_distance(&self, id: u16, mirror: bool) -> Option<u32> {
        let id = self.animation_id(id, mirror)? as u16;
        self.staged_loop_distances
            .iter()
            .find_map(|&(a, b, bits)| (id == a || id == b).then_some(bits))
    }

    pub fn staged_loop_rows(&self) -> &[(u16, u16, u32)] {
        &self.staged_loop_distances
    }
}

fn staged_loop_distances(elf: &Elf<'_>) -> Result<Vec<(u16, u16, u32)>> {
    // Constant propagation is bounded to the 108 original constructor calls.
    // 00191410 stores rowA/rowB and f12 at record +04/+08/+18.
    let mut registers = [None; 32];
    registers[0] = Some(0u32);
    let mut floats = [None; 32];
    let mut rows = Vec::new();
    for address in (0x0032_c124..0x0032_f0c0).step_by(4) {
        let word = elf.word(address)?;
        let op = word >> 26;
        let rs = ((word >> 21) & 31) as usize;
        let rt = ((word >> 16) & 31) as usize;
        let fs = ((word >> 11) & 31) as usize;
        let fd = ((word >> 6) & 31) as usize;
        match op {
            15 => registers[rt] = Some((word & 0xffff) << 16),
            9 | 13 => {
                registers[rt] = registers[rs].map(|value| {
                    if op == 13 {
                        value | (word & 0xffff)
                    } else {
                        value.wrapping_add((word as i16 as i32) as u32)
                    }
                })
            }
            0 if matches!(word & 63, 0x21 | 0x2d) && rt == 0 => registers[fs] = registers[rs],
            0x11 if rs == 4 => floats[fs] = registers[rt],
            0x11 if rs == 16 && word & 63 == 6 => floats[fd] = floats[fs],
            0x31 => floats[rt] = None,
            0x11 if rs == 16 && word & 63 <= 3 => floats[fd] = None,
            3 if (word & 0x03ff_ffff) << 2 == 0x0019_1410 => {
                let a = registers[6].ok_or_else(|| Error::format("unknown staged loop row A"))?;
                let b = registers[7].ok_or_else(|| Error::format("unknown staged loop row B"))?;
                let bits =
                    floats[12].ok_or_else(|| Error::format("unknown staged root distance"))?;
                let distance = f32::from_bits(bits);
                if a >= 1144 || b >= 1144 || !distance.is_finite() || distance <= 0.0 {
                    return Err(Error::format("invalid original staged loop constructor"));
                }
                rows.push((a as u16, b as u16, bits));
            }
            _ => {}
        }
    }
    if rows.len() != 108 {
        return Err(Error::format("expected 108 staged loop constructors"));
    }
    Ok(rows)
}

#[derive(Clone, Debug)]
pub struct Timeline {
    pub name: String,
    pub offset: u32,
    /// 0 = fixed team assignment, 1 = proximity assignment, 2 = shooter-first.
    pub kind: u32,
    pub frames: u32,
    /// Original record order is significant when several events share a frame.
    pub records: Vec<Record>,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub frame: u32,
    pub duration: u32,
    pub payload: Payload,
}

#[derive(Clone, Debug)]
pub enum Payload {
    Actor(Actor),
    /// Category 1 dispatches by byte 0x21, rather than by its opcode alone.
    Command(Command),
    Effect(Effect),
    Camera(Camera),
    Key(Key),
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub position: [f32; 3],
    pub slot: u16,
    pub heading: i16,
    pub animation: u16,
    pub alternate_animation: u16,
    /// Uninterpreted animation/movement selectors preserved from the disc.
    pub words: [u16; 7],
    /// Native finite-playback end fraction, payload +32, in 1/256 units.
    pub animation_end: u16,
    pub flags: [u8; 16],
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov: f32,
    pub heading: f32,
    pub target_actor: i16,
    pub flags: [u8; 6],
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub rgba: [u32; 4],
    pub target_actor: i16,
    pub flags: [u8; 6],
}

impl Effect {
    /// Field `k` as the effect player reads it (`FUN_00333530`): the
    /// position 0..2, the target 3..5, the colour 6..9 (whole numbers).
    pub fn field(&self, k: u8) -> Option<f32> {
        Some(match k {
            0..=2 => self.position[usize::from(k)],
            3..=5 => self.target[usize::from(k - 3)],
            6..=9 => self.rgba[usize::from(k - 6)] as i32 as f32,
            _ => return None,
        })
    }
}

/// The effect players a script manager keeps (record byte +0x2c).
pub const EFFECT_PLAYERS: usize = 4;

/// An effect player's screen layer: its blend (record byte +0x2d: 1 a
/// blend by its alpha, 2 an add, 3 a subtract; else not drawn) and colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectLayer {
    pub blend: u8,
    pub rgba: [i32; 4],
}

#[derive(Clone, Debug)]
pub struct Command {
    pub opcode: u16,
    /// 0 = ball, 1 = replay, 2 = AEMS, 3 = script control, 4 = effects.
    pub dispatch: u8,
    pub bytes: Vec<u8>,
}

/// The fields consumed by `FUN_00331630`/`FUN_001a17a8`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub event: u16,
    pub position: [f32; 3],
    pub intensity: i32,
    pub parameter: i32,
    pub extra: i32,
}

impl Command {
    pub fn sound(&self) -> Option<Sound> {
        if self.dispatch != 2 {
            return None;
        }
        // All parsed command layouts contain these fields. Avoid panicking
        // even if a caller constructs a shortened Command itself.
        let level = bytes::u32_le(&self.bytes, 16).ok()? as i32;
        Some(Sound {
            event: self.opcode,
            position: vector(&self.bytes, 20).ok()?,
            intensity: if level == 0 { 1000 } else { level },
            parameter: bytes::u32_le(&self.bytes, 8).ok()? as i32,
            extra: bytes::u32_le(&self.bytes, 12).ok()? as i32,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub value: f32,
    pub tangent: f32,
    /// Record index, including non-key records, rather than a curve index.
    pub previous: u16,
    pub field: u8,
    /// 0 = step, 1 = linear, 2 = cubic Hermite.
    pub interpolation: u8,
}

/// A sampled staged frame. Positions remain in the original court units.
#[derive(Clone, Debug)]
pub struct FrameState {
    pub actors: [Option<ActorFrame>; 7],
    pub camera: Option<Camera>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug)]
pub struct ActorFrame {
    pub actor: Actor,
    pub start_frame: u32,
    /// Elapsed staged frames; the animation's own rate comes from words[0].
    pub elapsed: f32,
}

/// A compact actor scalar curve resolved from the original key reference.
#[derive(Clone, Copy, Debug)]
pub struct ActorCurve {
    pub slot: u16,
    pub field: u8,
    pub start_frame: u32,
    pub end_frame: u32,
    pub start: f32,
    pub end: f32,
    pub start_tangent: f32,
    pub end_tangent: f32,
    pub interpolation: u8,
}

impl Key {
    /// Original interpolation (`FUN_00336f48`, `FUN_00337088/98`).
    /// Cubic slopes scale by the inclusive frame interval (`delta + 1`).
    pub fn interpolate(self, previous: f32, tangent: f32, delta: u32, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self.interpolation {
            0 => {
                if t >= 1.0 {
                    self.value
                } else {
                    previous
                }
            }
            1 => previous + t * (self.value - previous),
            2 => {
                let a = tangent * (delta as f32 + 1.0);
                let b = self.tangent * (delta as f32 + 1.0);
                (((a + b + 2.0 * (previous - self.value)) * t
                    + (3.0 * (self.value - previous) - b - 2.0 * a))
                    * t
                    + a)
                    * t
                    + previous
            }
            _ => previous,
        }
    }
}

impl Timeline {
    /// Compact resolved actor curves for a renderer-independent simulation
    /// plan. Heading values/tangents remain in the disc's degrees.
    pub fn actor_curves(&self) -> Vec<ActorCurve> {
        let mut curves = Vec::new();
        for (index, record) in self.records.iter().enumerate() {
            let Payload::Key(key) = record.payload else {
                continue;
            };
            let Some(root) = self.key_root(index) else {
                continue;
            };
            let Payload::Actor(actor) = &self.records[root].payload else {
                continue;
            };
            if key.field > 3 {
                continue;
            }
            let Some(previous) = self.records.get(usize::from(key.previous)) else {
                continue;
            };
            let (start, tangent) = match &previous.payload {
                Payload::Key(key) => (key.value, key.tangent),
                Payload::Actor(actor) => (
                    if key.field == 3 {
                        f32::from(actor.heading)
                    } else {
                        actor.position[usize::from(key.field)]
                    },
                    0.0,
                ),
                _ => continue,
            };
            curves.push(ActorCurve {
                slot: actor.slot,
                field: key.field,
                start_frame: previous.frame,
                end_frame: record.frame,
                start,
                end: key.value,
                start_tangent: tangent,
                end_tangent: key.tangent,
                interpolation: key.interpolation,
            });
        }
        curves
    }

    /// Resolve scalar curves through their original record references. The
    /// two source-less keys on the shipped disc are ignored by the original
    /// entity lookup and here. Animation selection stays an actor field;
    /// one-shot ball/audio dispatches are returned by `events_crossed`.
    pub fn sample(&self, frame: f32, mirrored: bool) -> FrameState {
        let frame = frame.max(0.0).min(self.frames.saturating_sub(1) as f32);
        let mut state = FrameState {
            actors: std::array::from_fn(|_| None),
            camera: None,
            effects: Vec::new(),
        };
        let mut actor_changed = [[f32::NEG_INFINITY; 4]; 7];
        for record in &self.records {
            if record.frame as f32 > frame {
                continue;
            }
            match &record.payload {
                Payload::Actor(actor) => {
                    if let Some(slot) = state.actors.get_mut(usize::from(actor.slot)) {
                        let mut next = actor.clone();
                        if let Some(previous) = slot {
                            if actor.flags[0] != 0 {
                                next.position = previous.actor.position;
                            }
                            if actor.flags[4] != 0 {
                                next.heading = previous.actor.heading;
                            }
                        }
                        if actor.flags[0] == 0 {
                            actor_changed[usize::from(actor.slot)][..3].fill(record.frame as f32);
                        }
                        if actor.flags[4] == 0 {
                            actor_changed[usize::from(actor.slot)][3] = record.frame as f32;
                        }
                        *slot = Some(ActorFrame {
                            actor: next,
                            start_frame: record.frame,
                            elapsed: frame - record.frame as f32,
                        });
                    }
                }
                Payload::Effect(effect) => state.effects.push(effect.clone()),
                _ => (),
            }
        }
        for (index, record) in self.records.iter().enumerate() {
            let Payload::Key(key) = record.payload else {
                continue;
            };
            let Some(previous) = self.records.get(usize::from(key.previous)) else {
                continue;
            };
            if previous.frame as f32 >= frame {
                continue;
            }
            let Some(root) = self.key_root(index) else {
                continue;
            };
            let Some(value) = self.key_value(index, frame) else {
                continue;
            };
            let update = frame.min(record.frame as f32);
            let Payload::Actor(actor) = &self.records[root].payload else {
                continue;
            };
            let Some(stamp) = actor_changed
                .get_mut(usize::from(actor.slot))
                .and_then(|fields| fields.get_mut(usize::from(key.field)))
            else {
                continue;
            };
            if update < *stamp {
                continue;
            }
            *stamp = update;
            if let Some(Some(sample)) = state.actors.get_mut(usize::from(actor.slot)) {
                match key.field {
                    0..=2 => sample.actor.position[usize::from(key.field)] = value,
                    3 => sample.actor.heading = value.round_ties_even() as i16,
                    _ => (),
                }
            }
        }
        if mirrored {
            for actor in state.actors.iter_mut().flatten() {
                actor.actor.position[0] = -actor.actor.position[0];
                actor.actor.heading = 360i16.saturating_sub(actor.actor.heading);
            }
        }
        // Default timeline playback enables camera 0.
        state.camera = self.camera(frame, mirrored, 0);
        state
    }

    /// Camera `id` at `frame` (payload byte 36; `FUN_00337718` enables one
    /// id at a time: 0 in play, the freeze's and the replay's own ids when
    /// they run). Only keys rooted at that camera's records move it.
    pub fn camera(&self, frame: f32, mirrored: bool, id: u8) -> Option<Camera> {
        let frame = frame.max(0.0).min(self.frames.saturating_sub(1) as f32);
        let mut camera: Option<Camera> = None;
        let mut changed = [f32::NEG_INFINITY; 8];
        let ours = |payload: &Payload| matches!(payload, Payload::Camera(c) if c.flags[2] == id);
        for record in &self.records {
            if record.frame as f32 > frame {
                continue;
            }
            let Payload::Camera(next) = &record.payload else {
                continue;
            };
            if next.flags[2] != id {
                continue;
            }
            let mut next = next.clone();
            if next.flags[0] != 0 {
                if let Some(previous) = &camera {
                    next.position = previous.position;
                }
            } else {
                changed[..3].fill(record.frame as f32);
            }
            changed[3..].fill(record.frame as f32);
            camera = Some(next);
        }
        let camera_ref = camera.as_mut()?;
        for (index, record) in self.records.iter().enumerate() {
            let Payload::Key(key) = record.payload else {
                continue;
            };
            let Some(previous) = self.records.get(usize::from(key.previous)) else {
                continue;
            };
            if previous.frame as f32 >= frame {
                continue;
            }
            let Some(root) = self.key_root(index) else {
                continue;
            };
            if !ours(&self.records[root].payload) {
                continue;
            }
            let Some(value) = self.key_value(index, frame) else {
                continue;
            };
            let update = frame.min(record.frame as f32);
            let Some(stamp) = changed.get_mut(usize::from(key.field)) else {
                continue;
            };
            if update < *stamp {
                continue;
            }
            *stamp = update;
            match key.field {
                0..=2 => camera_ref.position[usize::from(key.field)] = value,
                3..=5 => camera_ref.target[usize::from(key.field - 3)] = value,
                6 => camera_ref.fov = value,
                7 => camera_ref.heading = value,
                _ => (),
            }
        }
        if mirrored {
            camera_ref.position[0] = -camera_ref.position[0];
            camera_ref.target[0] = -camera_ref.target[0];
        }
        camera
    }

    fn key_root(&self, index: usize) -> Option<usize> {
        let mut index = index;
        for _ in 0..self.records.len() {
            match self.records.get(index)?.payload {
                Payload::Key(key) => index = usize::from(key.previous),
                _ => return Some(index),
            }
        }
        None
    }

    fn key_value(&self, index: usize, frame: f32) -> Option<f32> {
        let record = self.records.get(index)?;
        let Payload::Key(key) = record.payload else {
            return None;
        };
        let previous = self.records.get(usize::from(key.previous))?;
        let (value, tangent) = match &previous.payload {
            Payload::Key(previous) => (previous.value, previous.tangent),
            Payload::Actor(actor) => (
                match key.field {
                    0..=2 => actor.position[usize::from(key.field)],
                    3 => f32::from(actor.heading),
                    _ => return None,
                },
                0.0,
            ),
            Payload::Camera(camera) => (
                match key.field {
                    0..=2 => camera.position[usize::from(key.field)],
                    3..=5 => camera.target[usize::from(key.field - 3)],
                    6 => camera.fov,
                    7 => camera.heading,
                    _ => return None,
                },
                0.0,
            ),
            Payload::Effect(effect) => (effect.field(key.field)?, 0.0),
            _ => return None,
        };
        let delta = record.frame.saturating_sub(previous.frame);
        if delta == 0 {
            return Some(key.value);
        }
        Some(key.interpolate(
            value,
            tangent,
            delta,
            (frame - previous.frame as f32) / delta as f32,
        ))
    }

    /// The script's effect layers at `frame` (its effect players,
    /// `FUN_00333298`): by player (record byte +0x2c), the latest effect
    /// record at or before the frame, its fields set by the keys rooted at
    /// it as of the frame (`FUN_00333648`; the colour truncated to whole
    /// numbers, at most 255, `FUN_00333750`). The players live as long as
    /// the script; the caller passes the frame the manager has reached.
    pub fn effect_layers(&self, frame: f32) -> [Option<EffectLayer>; EFFECT_PLAYERS] {
        let mut roots = [None; EFFECT_PLAYERS];
        for (index, record) in self.records.iter().enumerate() {
            if let Payload::Effect(effect) = &record.payload
                && record.frame as f32 <= frame
                && let Some(slot) = roots.get_mut(usize::from(effect.flags[2]))
            {
                *slot = Some(index);
            }
        }
        roots.map(|root| {
            let root = root?;
            let Payload::Effect(effect) = &self.records[root].payload else {
                return None;
            };
            let mut fields: [f32; 10] = std::array::from_fn(|k| effect.field(k as u8).unwrap());
            let mut changed = [f32::NEG_INFINITY; 10];
            for (index, record) in self.records.iter().enumerate() {
                let Payload::Key(key) = record.payload else {
                    continue;
                };
                if self.key_root(index) != Some(root) {
                    continue;
                }
                let Some(previous) = self.records.get(usize::from(key.previous)) else {
                    continue;
                };
                if previous.frame as f32 >= frame {
                    continue;
                }
                let update = frame.min(record.frame as f32);
                let Some(stamp) = changed.get_mut(usize::from(key.field)) else {
                    continue;
                };
                if update < *stamp {
                    continue;
                }
                let Some(value) = self.key_value(index, update) else {
                    continue;
                };
                *stamp = update;
                fields[usize::from(key.field)] = value;
            }
            let colour = |k: usize| (fields[6 + k] as i32).min(255);
            Some(EffectLayer {
                blend: effect.flags[3],
                rgba: [colour(0), colour(1), colour(2), colour(3)],
            })
        })
    }

    /// One-shot dispatches crossing a frame boundary, including frame zero
    /// on the first call. A backwards seek replays no one-shot events.
    pub fn events_crossed(
        &self,
        previous: Option<u32>,
        frame: u32,
    ) -> impl Iterator<Item = &Record> {
        self.records.iter().filter(move |record| {
            matches!(record.payload, Payload::Command(_))
                && record.frame <= frame
                && previous.is_none_or(|old| old < record.frame)
        })
    }
}

fn vector(data: &[u8], at: usize) -> Result<[f32; 3]> {
    Ok([
        bytes::f32_le(data, at)?,
        bytes::f32_le(data, at + 4)?,
        bytes::f32_le(data, at + 8)?,
    ])
}

fn parse_timelines(data: &[u8]) -> Result<Vec<Timeline>> {
    let mut scripts = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let header = bytes::range(data, offset, 20)?;
        if &header[..4] != b"ssb\x11" {
            return Err(Error::format("unsupported staged script magic"));
        }
        let kind = bytes::u32_le(header, 4)?;
        let frames = bytes::u32_le(header, 8)?;
        let length = bytes::u32_le(header, 12)? as usize;
        let count = bytes::u32_le(header, 16)? as usize;
        if kind > 2 || frames == 0 || length < 20 || count > (length - 20) / 24 {
            return Err(Error::format("invalid staged script header"));
        }
        let script = bytes::range(data, offset, length)?;
        let mut records = Vec::with_capacity(count);
        let mut at = 20;
        for _ in 0..count {
            let category = bytes::u32_le(script, at)?;
            let frame = bytes::u32_le(script, at + 4)?;
            let duration = bytes::u32_le(script, at + 8)?;
            let size = match category {
                0 => 52,
                1 => 36,
                2 => 48,
                3 => 40,
                4 => 12,
                _ => return Err(Error::format("unknown staged script category")),
            };
            let data = bytes::range(script, at + 12, size)?;
            if frame >= frames {
                return Err(Error::format("staged event past last frame"));
            }
            let payload = match category {
                0 => Payload::Actor(Actor {
                    position: vector(data, 0)?,
                    slot: bytes::u16_le(data, 12)?,
                    heading: bytes::u16_le(data, 30)? as i16,
                    animation: bytes::u16_le(data, 18)?,
                    alternate_animation: bytes::u16_le(data, 22)?,
                    words: std::array::from_fn(|i| bytes::u16_le(data, 14 + i * 2).unwrap()),
                    animation_end: bytes::u16_le(data, 32)?,
                    flags: data[36..52].try_into().unwrap(),
                }),
                1 => Payload::Command(Command {
                    opcode: bytes::u16_le(data, 0)?,
                    dispatch: data[33],
                    bytes: data.to_vec(),
                }),
                2 => Payload::Effect(Effect {
                    position: vector(data, 0)?,
                    target: vector(data, 12)?,
                    rgba: std::array::from_fn(|i| bytes::u32_le(data, 24 + i * 4).unwrap()),
                    target_actor: bytes::u16_le(data, 40)? as i16,
                    flags: data[42..48].try_into().unwrap(),
                }),
                3 => Payload::Camera(Camera {
                    position: vector(data, 0)?,
                    target: vector(data, 12)?,
                    fov: bytes::f32_le(data, 24)?,
                    heading: bytes::f32_le(data, 28)?,
                    target_actor: bytes::u16_le(data, 32)? as i16,
                    flags: data[34..40].try_into().unwrap(),
                }),
                4 => Payload::Key(Key {
                    value: bytes::f32_le(data, 0)?,
                    tangent: bytes::f32_le(data, 4)?,
                    previous: bytes::u16_le(data, 8)?,
                    field: data[10],
                    interpolation: data[11],
                }),
                _ => unreachable!(),
            };
            records.push(Record {
                frame,
                duration,
                payload,
            });
            at += 12 + size;
        }
        if at != length {
            return Err(Error::format("staged script has trailing record bytes"));
        }
        for (index, record) in records.iter().enumerate() {
            if let Payload::Key(key) = record.payload {
                let previous = usize::from(key.previous);
                // Two shipped IntBoss06b keys carry the signed -1 source
                // sentinel; the original entity lookup drops these keys.
                if key.previous != u16::MAX
                    && (previous >= index
                        || key.interpolation > 2
                        || records[previous].frame > record.frame)
                {
                    return Err(Error::format("invalid staged keyframe reference"));
                }
            }
        }
        scripts.push(Timeline {
            name: String::new(),
            offset: offset as u32,
            kind,
            frames,
            records,
        });
        offset += length;
    }
    Ok(scripts)
}

/// The executable's 1144 animation records (`0x3de8a0`, 28 bytes each):
/// each clip's name and mirror flag, by record number.
pub fn animation_records(executable: &[u8]) -> Result<Vec<Animation>> {
    records(&Elf::parse(executable)?)
}

fn records(elf: &Elf) -> Result<Vec<Animation>> {
    (0..1144)
        .map(|i| {
            let entry = 0x3de8a0 + i * 28;
            let name = elf.word(entry)?;
            Ok(Animation {
                name: if name == 0 {
                    None
                } else {
                    Some(elf.string(name)?)
                },
                mirrored: elf.word(entry + 24)? != 0,
            })
        })
        .collect()
}

struct Elf<'a> {
    data: &'a [u8],
    segments: Vec<(u32, usize, usize)>,
}
impl<'a> Elf<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        if data.get(..6) != Some(b"\x7fELF\x01\x01") || bytes::u16_le(data, 18)? != 8 {
            return Err(Error::format("expected US PS2 ELF32 MIPS executable"));
        }
        let table = bytes::u32_le(data, 28)? as usize;
        let stride = bytes::u16_le(data, 42)? as usize;
        let count = bytes::u16_le(data, 44)? as usize;
        if stride < 32 || count > 128 {
            return Err(Error::format("invalid ELF program headers"));
        }
        let mut segments = Vec::new();
        for i in 0..count {
            let at = table
                .checked_add(i * stride)
                .ok_or_else(|| Error::format("ELF header overflow"))?;
            if bytes::u32_le(data, at)? == 1 {
                let offset = bytes::u32_le(data, at + 4)? as usize;
                let size = bytes::u32_le(data, at + 16)? as usize;
                bytes::range(data, offset, size)?;
                segments.push((bytes::u32_le(data, at + 8)?, offset, size));
            }
        }
        Ok(Self { data, segments })
    }
    fn at(&self, address: u32, length: usize) -> Result<&[u8]> {
        for &(base, offset, size) in &self.segments {
            if let Some(delta) = address.checked_sub(base) {
                let delta = delta as usize;
                if delta.checked_add(length).is_some_and(|end| end <= size) {
                    return bytes::range(self.data, offset + delta, length);
                }
            }
        }
        Err(Error::format("staged script table address outside ELF"))
    }
    fn word(&self, address: u32) -> Result<u32> {
        bytes::u32_le(self.at(address, 4)?, 0)
    }
    fn string(&self, address: u32) -> Result<String> {
        let mut name = String::new();
        for i in 0..256 {
            let at = address
                .checked_add(i)
                .ok_or_else(|| Error::format("ELF string overflow"))?;
            let value = self.at(at, 1)?[0];
            if value == 0 {
                return Ok(name);
            }
            if !(32..127).contains(&value) {
                return Err(Error::format("invalid staged script name"));
            }
            name.push(char::from(value));
        }
        Err(Error::format("unterminated staged script name"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_disambiguates_ball_and_aems_in_the_same_layout() {
        let mut command = Command {
            opcode: 0x21,
            dispatch: 0,
            bytes: vec![0; 36],
        };
        assert!(command.sound().is_none());
        command.dispatch = 2;
        command.bytes[8..12].copy_from_slice(&7u32.to_le_bytes());
        let sound = command.sound().unwrap();
        assert_eq!(sound.event, 0x21);
        assert_eq!(sound.intensity, 1000);
        assert_eq!(sound.parameter, 7);
    }

    #[test]
    fn cubic_uses_inclusive_frame_tangents_and_keeps_endpoint_values() {
        let key = Key {
            value: 10.0,
            tangent: -0.5,
            previous: 0,
            field: 0,
            interpolation: 2,
        };
        assert_eq!(key.interpolate(2.0, 1.0, 3, 0.0), 2.0);
        assert_eq!(key.interpolate(2.0, 1.0, 3, 1.0), 10.0);
        assert_eq!(key.interpolate(2.0, 1.0, 3, 0.5), 6.75);
    }

    #[test]
    fn malformed_lengths_unknown_categories_and_cyclic_keys_fail() {
        let mut script = b"ssb\x11".to_vec();
        for value in [1u32, 10, 44, 1, 4, 2, 1] {
            script.extend(value.to_le_bytes());
        }
        script.extend([0; 12]);
        assert!(parse_timelines(&script).is_err(), "self-referencing key");
        script[20..24].copy_from_slice(&5u32.to_le_bytes());
        assert!(parse_timelines(&script).is_err(), "unknown category");
        script[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_timelines(&script).is_err(), "outside buffer");
        assert!(parse_timelines(&script[..19]).is_err(), "truncated header");
    }
}
