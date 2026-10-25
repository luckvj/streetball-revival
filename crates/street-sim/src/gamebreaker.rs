//! Deterministic playback metadata for the original staged Gamebreakers.
//!
//! Disc parsing stays outside the simulation. Match setup supplies the
//! sampled actor poses and original ball instructions; a server therefore
//! plays the same timeline without loading animation banks or a renderer.
//!
//! Sides: the scripts are authored attacking -x, the basket team 1
//! attacks (its region map, unreflected, puts the dunk families by the -x
//! rim). Team 0, attacking +x, plays them mirrored across x. Headings: a script's heading is the model's own
//! turn (heading 0 faces -z), half a turn from the simulation's (heading 0
//! faces +z); [`script_heading`] converts.

use crate::Player;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorPose {
    pub position: [f32; 3],
    /// Original script heading converted to radians by match setup.
    pub heading: f32,
    pub animation: u16,
    pub animation_frame: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BallCue {
    pub frame: u32,
    pub duration: u32,
    /// The original 36-byte command record, including the dispatch byte.
    /// Retaining it avoids discarding fields used by the staged-ball physics.
    pub data: [u8; 36],
}

impl BallCue {
    pub fn opcode(self) -> u16 {
        u16::from_le_bytes([self.data[0], self.data[1]])
    }
    pub fn source(self) -> i16 {
        i16::from_le_bytes([self.data[2], self.data[3]])
    }
    pub fn target(self) -> i16 {
        i16::from_le_bytes([self.data[4], self.data[5]])
    }
    pub fn word(self, offset: usize) -> Option<i32> {
        Some(i32::from_le_bytes(
            self.data
                .get(offset..offset.checked_add(4)?)?
                .try_into()
                .ok()?,
        ))
    }
    pub fn destination(self) -> [f32; 2] {
        [
            f32::from_bits(self.word(20).unwrap() as u32),
            f32::from_bits(self.word(24).unwrap() as u32),
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequencePlan {
    /// Index into the original 70-entry bank.
    pub script: usize,
    /// Original actor binding mode: fixed (0), nearest (1), shooter-first (2).
    pub kind: u32,
    pub frame_count: u32,
    pub actors: Vec<ActorTrack>,
    pub curves: Vec<Curve>,
    pub ball_cues: Vec<BallCue>,
    /// The script's own control and replay commands.
    pub controls: Controls,
    /// Fixed animation-VM spans started by staged opcode 1.
    pub finishes: Vec<FinishTrack>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishTrack {
    pub slot: u8,
    pub start_frame: u32,
    pub animation: u16,
    pub spans: Vec<FinishSpan>,
    /// The actor's state word at the body's callback 21 (its last `9b`
    /// before it): bit 7 or 15 sends the release down the launcher's dunk
    /// path.
    pub release_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishSpan {
    pub start_tick: u32,
    pub duration: u32,
    pub from_frame: f32,
    pub to_frame: f32,
}

/// A script's control commands (`FUN_003355c8`'s listeners 0, 2 and 4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// Script control 0 (`FUN_00333238`): the script stops at this frame
    /// (manager +0x8c, then `FUN_00335f48`), short of its last frame.
    pub stop: Option<u32>,
    /// Replay command 2 (`FUN_00334d60` → `FUN_00336340`): the world stops
    /// at this frame for a while (the task groups 0x382 suspended) while a
    /// camera of the script's own moves.
    pub hold: Option<Hold>,
    /// Replay command 0 (`FUN_00334b30`), at the end of its span: the span
    /// plays again slowed, from a camera of its own, as the game's replay
    /// ([`crate::Event::StagedRerun`], `crate::replay`).
    pub rerun: Option<Rerun>,
}

/// The freeze: from `frame`, `duration` frames on camera `camera`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold {
    pub frame: u32,
    pub duration: u32,
    pub camera: u8,
    /// The command's word 12 is 1: the freeze suspends the task groups
    /// 0x1382, not 0x382 (`FUN_00336340`), so the replay manager and the
    /// gamebreaker task (group 0x1000) wait out the freeze too and its
    /// updates stay out of the replay ring. Fifteen of the disc's sixteen
    /// set it; `GamSR1`'s freeze is recorded.
    pub holds_replays: bool,
}

/// The slowed replay of frames `from..to`, at `speed` percent (25..150,
/// else 100), on camera `camera`. The disc's sixteen ask for it once (the
/// command's word 12 is 0: `FUN_00336188` counts word 12 + 1 replays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rerun {
    pub from: u32,
    pub to: u32,
    pub speed: u8,
    pub camera: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorTrack {
    pub frame: u32,
    pub duration: u32,
    pub slot: u16,
    pub pose: ActorPose,
    /// Animation frames per staged frame, from the original speed/256.
    pub animation_speed: f32,
    /// Resolved original staged actor VM inputs. Synthetic plans omit them.
    pub playback: Option<NativePlayback>,
    pub keep_position: bool,
    pub keep_heading: bool,
}

/// Inputs consumed by 0032b458/0032b810, resolved against the loaded clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePlayback {
    pub frames: u16,
    pub speed_word: u16,
    pub end_fraction: u16,
    pub flags: [u8; 16],
    pub initial: bool,
    /// Native first-match root-distance bits for original/mirrored rows.
    pub loop_distances: [Option<u32>; 2],
}

/// The live channel clock and its last evaluated target pose can differ:
/// 0032b810 can seek +64 before the next pose evaluation, and completion
/// changes +64 without evaluating another pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePhase {
    pub channel_frame: f32,
    pub display_frame: f32,
    pub looping: bool,
    /// During the six-update blend, +64 still belongs to the previous row.
    /// Only the new row's fixed target phase is known here; endpoint consumers
    /// retain their earlier approximation until that transition completes.
    pub blending: bool,
}

// EE scalar add/sub/multiply chop their result; divide rounds to nearest.
// Values here are bounded positive normal clip/rate inputs.
fn phase_chop(value: f64) -> f32 {
    let rounded = value as f32;
    if rounded.is_finite() && f64::from(rounded.abs()) > value.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

// The retained PCSX2 2.8.2 target masks the smaller add/sub operand to
// preserve one guard bit before chopping. Result-only chopping differs
// during signed cancellation. This helper is scoped to staged positions;
// it is not a change to the phase clock or a complete hardware float model.
fn position_add_sub(a: f32, b: f32, subtract: bool) -> f32 {
    let (mut ab, mut bb) = (a.to_bits(), b.to_bits());
    let difference = ((ab >> 23) & 255) as i32 - ((bb >> 23) & 255) as i32;
    if difference > 0 {
        bb &= if difference >= 25 {
            0x8000_0000
        } else {
            u32::MAX << (difference - 1)
        };
    } else if difference < 0 {
        ab &= if difference <= -25 {
            0x8000_0000
        } else {
            u32::MAX << (-difference - 1)
        };
    }
    let (a, b) = (f64::from(f32::from_bits(ab)), f64::from(f32::from_bits(bb)));
    phase_chop(if subtract { a - b } else { a + b })
}

fn position_add(a: f32, b: f32) -> f32 {
    position_add_sub(a, b, false)
}

fn position_sub(a: f32, b: f32) -> f32 {
    position_add_sub(a, b, true)
}

fn position_mul(a: f32, b: f32) -> f32 {
    phase_chop(f64::from(a) * f64::from(b))
}

/// Inputs read by the staged loop motion-rate producer (0032c0a0).
#[derive(Clone, Copy, Debug)]
pub struct NativeMotion {
    pub position: [f32; 3],
    pub previous: [f32; 3],
    /// Actor Y at the manager's rate check, before the actor update clamps it.
    pub height: f32,
    pub scale: f32,
    pub last_tick: u32,
    /// The manager observes this before the subsequent actor VM update.
    pub blending: bool,
}

impl NativeMotion {
    /// None preserves the channel's prior configured/current rate.
    pub fn step(self, root_distance: f32) -> Option<f32> {
        if self.height != 0.0 || self.last_tick as i32 <= 0 || self.blending {
            return None;
        }
        let dx = phase_chop(f64::from(self.position[0]) - f64::from(self.previous[0]));
        let dz = phase_chop(f64::from(self.position[2]) - f64::from(self.previous[2]));
        let x2 = phase_chop(f64::from(dx) * f64::from(dx));
        let z2 = phase_chop(f64::from(dz) * f64::from(dz));
        let distance = phase_chop(f64::from(x2) + f64::from(z2)).sqrt();
        if distance == 0.0 {
            return None;
        }
        let period = root_distance / distance;
        let period = phase_chop(f64::from(period) * f64::from(self.scale));
        let step = 1.0 / period;
        (step.is_finite() && step > 0.0).then_some(step)
    }
}

#[derive(Clone, Copy)]
struct PlaybackClock {
    frame: f32,
    sampled: f32,
    counter: u32,
    end: f32,
    active: bool,
}

impl NativePlayback {
    pub fn motion_step(self, mirrored: bool, motion: NativeMotion) -> Option<f32> {
        if !self.looping() {
            return None;
        }
        motion.step(f32::from_bits(self.loop_distances[usize::from(mirrored)]?))
    }
    pub fn looping(self) -> bool {
        self.flags[6] != 0
    }

    pub fn blend_ticks(self) -> u32 {
        if self.initial || self.flags[7] != 0 {
            0
        } else {
            6
        }
    }

    /// 60 Hz configured step, preserving 0032b4c4..0032b578's operation order.
    pub fn step(self) -> f32 {
        let mut period = 256.0 / f32::from(self.speed_word);
        if !self.initial && !self.looping() {
            let last = f32::from(self.frames - 1);
            period = phase_chop(f64::from(last) * f64::from(period));
            period = phase_chop(f64::from(period) - 6.0) / last;
        }
        1.0 / period
    }

    fn end_frame(self) -> f32 {
        let last = f32::from(self.frames - 1);
        if self.end_fraction < 256 {
            (((u32::from(self.frames) * u32::from(self.end_fraction)) >> 8) as f32 - 1.0).min(last)
        } else {
            last
        }
    }

    fn manager_seek(self, clock: &mut PlaybackClock) {
        if self.flags[10] != 0 {
            clock.frame = ((u32::from(self.frames) * u32::from(self.flags[10])) >> 8) as f32;
        }
        clock.end = self.end_frame();
    }

    fn advance(self, clock: &mut PlaybackClock, duration: u32, step: f32, first: bool) {
        if !clock.active {
            return;
        }
        let last = f32::from(self.frames - 1);
        if self.looping() {
            clock.frame = phase_chop(f64::from(clock.frame) + f64::from(step));
            if clock.frame >= f32::from(self.frames) {
                clock.frame = phase_chop(f64::from(clock.frame) - f64::from(self.frames));
            }
        } else {
            // A freshly selected row starts at -1. Subunit finite playback
            // shows key zero without incrementing the duration counter.
            if first && step < 1.0 {
                clock.frame = 0.0;
            } else {
                clock.counter += 1;
                if clock.counter > duration {
                    if clock.end >= 0.0 {
                        clock.frame = clock.end;
                    }
                    clock.active = false;
                    return;
                }
                clock.frame = phase_chop(f64::from(clock.frame) + f64::from(step));
                if clock.counter == duration && clock.end >= 0.0 {
                    clock.frame = clock.end;
                }
            }
            if clock.frame > last {
                clock.frame = last;
                clock.counter = duration;
            }
            clock.frame = clock.frame.max(0.0);
        }
        clock.sampled = clock.frame;
    }

    fn tick(self, clock: &mut PlaybackClock, tick: u32, duration: u32, step: f32) {
        if !self.initial && !self.looping() && tick == 7 {
            self.manager_seek(clock);
        }
        let blend = self.blend_ticks();
        if blend != 0 && tick < blend {
            return;
        }
        if blend != 0 && tick == blend {
            clock.frame = if self.looping() {
                0.0
            } else {
                ((u32::from(self.frames - 1) * u32::from(self.flags[10])) >> 8) as f32
            };
            clock.counter = 0;
        }
        self.advance(clock, duration, step, false);
    }

    /// Fixed-rate, uninterrupted native integer script ticks: setup evaluates
    /// once, then +4c=2 skips
    /// the same-tick actor update (0019c628). Seeks between script ticks
    /// interpolate adjacent clocks for presentation; no runtime parity is
    /// asserted for that replay/presentation interpolation. Runtime gait rate
    /// overrides and extra actor updates during state7 resume need additional
    /// history and are not reconstructed from source-record elapsed time.
    pub fn phase_at(self, elapsed: f32, duration: u32) -> NativePhase {
        let elapsed = elapsed.clamp(0.0, 10_000.0);
        let ticks = elapsed as u32;
        let blend = self.blend_ticks();
        let duration = if self.initial || self.looping() {
            duration
        } else {
            duration.saturating_sub(6)
        };
        let step = self.step();
        let start = if blend != 0 && !self.looping() {
            ((u32::from(self.frames - 1) * u32::from(self.flags[10])) >> 8) as f32
        } else if self.looping() {
            0.0
        } else {
            -1.0
        };
        let mut clock = PlaybackClock {
            frame: start,
            sampled: start.max(0.0),
            counter: 0,
            end: f32::from(self.frames - 1),
            active: true,
        };
        if blend == 0 {
            self.advance(&mut clock, duration, step, !self.looping());
        }
        if self.initial && !self.looping() {
            self.manager_seek(&mut clock);
        }
        for tick in 1..=ticks {
            self.tick(&mut clock, tick, duration, step);
        }
        let fraction = elapsed.fract();
        if fraction != 0.0 && ticks < 10_000 {
            let mut next = clock;
            self.tick(&mut next, ticks + 1, duration, step);
            let interpolate = |a: f32, mut b: f32| {
                if self.looping() && b < a {
                    b += f32::from(self.frames);
                }
                let value = a + (b - a) * fraction;
                if self.looping() {
                    value.rem_euclid(f32::from(self.frames))
                } else {
                    value
                }
            };
            clock.frame = interpolate(clock.frame, next.frame);
            clock.sampled = interpolate(clock.sampled, next.sampled);
        }
        NativePhase {
            channel_frame: clock.frame,
            display_frame: clock.sampled,
            looping: self.looping(),
            blending: ticks < blend,
        }
    }
}

/// Mutable staged VM clock. Manager seeks/rate writes and actor updates are
/// separate events; the manager's script tick cannot reconstruct their history.
/// Float words are retained exactly by the snapshot codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeClock {
    pub track: u16,
    pub current_track: u16,
    pub frame_bits: u32,
    pub sampled_bits: u32,
    pub step_bits: u32,
    pub configured_step_bits: u32,
    pub end_bits: u32,
    pub target_bits: u32,
    pub counter: u32,
    pub blend_updates: u8,
    pub seek_left: u8,
    pub active: bool,
    pub last_tick: u32,
    pub position_bits: [u32; 3],
    pub previous_bits: [u32; 3],
}

impl NativeClock {
    fn start(index: usize, actor: &ActorTrack, previous: Option<Self>, position: [f32; 3]) -> Self {
        let playback = actor.playback.expect("resolved native track");
        let phase = playback.phase_at(0.0, actor.duration);
        let step = playback.step();
        let blending = playback.blend_ticks() != 0 && previous.is_some();
        let previous = previous.filter(|_| blending);
        Self {
            track: index as u16,
            current_track: previous.map_or(index as u16, |clock| clock.current_track),
            frame_bits: previous.map_or(phase.channel_frame.to_bits(), |clock| clock.frame_bits),
            sampled_bits: previous
                .map_or(phase.display_frame.to_bits(), |clock| clock.sampled_bits),
            step_bits: if blending {
                (1.0f32 / 6.0).to_bits()
            } else {
                step.to_bits()
            },
            configured_step_bits: step.to_bits(),
            end_bits: if playback.initial && !playback.looping() {
                playback.end_frame()
            } else {
                f32::from(playback.frames - 1)
            }
            .to_bits(),
            target_bits: if playback.looping() {
                0.0
            } else {
                ((u32::from(playback.frames - 1) * u32::from(playback.flags[10])) >> 8) as f32
            }
            .to_bits(),
            counter: if blending {
                1
            } else if playback.looping() {
                u32::MAX
            } else {
                u32::from(step >= 1.0)
            },
            blend_updates: u8::from(blending),
            seek_left: if !playback.initial && !playback.looping() {
                7
            } else {
                0
            },
            active: true,
            last_tick: actor.frame,
            position_bits: position.map(f32::to_bits),
            previous_bits: position.map(f32::to_bits),
        }
    }

    fn clock(self) -> PlaybackClock {
        PlaybackClock {
            frame: f32::from_bits(self.frame_bits),
            sampled: f32::from_bits(self.sampled_bits),
            counter: self.counter,
            end: f32::from_bits(self.end_bits),
            active: self.active,
        }
    }

    fn store(&mut self, clock: PlaybackClock) {
        self.frame_bits = clock.frame.to_bits();
        self.sampled_bits = clock.sampled.to_bits();
        self.counter = clock.counter;
        self.end_bits = clock.end.to_bits();
        self.active = clock.active;
    }

    fn actor_update(&mut self, actor: &ActorTrack) {
        let playback = actor.playback.expect("resolved clock track");
        let mut clock = self.clock();
        if self.blend_updates != 0 {
            self.blend_updates += 1;
            self.counter = u32::from(self.blend_updates);
            if self.blend_updates <= 6 {
                return;
            }
            self.blend_updates = 0;
            self.current_track = self.track;
            clock.frame = f32::from_bits(self.target_bits);
            clock.counter = if playback.looping() { u32::MAX } else { 0 };
            self.step_bits = self.configured_step_bits;
        }
        playback.advance(
            &mut clock,
            if playback.initial || playback.looping() {
                actor.duration
            } else {
                actor.duration.saturating_sub(6)
            },
            f32::from_bits(self.step_bits),
            false,
        );
        self.store(clock);
    }

    pub fn phase(self, playback: NativePlayback) -> NativePhase {
        NativePhase {
            channel_frame: f32::from_bits(self.frame_bits),
            display_frame: if self.blend_updates != 0 {
                f32::from_bits(self.target_bits)
            } else {
                f32::from_bits(self.sampled_bits)
            },
            looping: playback.looping(),
            blending: self.blend_updates != 0,
        }
    }

    pub(crate) fn valid(self) -> bool {
        self.track < 1024
            && self.current_track < 1024
            && self.last_tick <= 10_000
            && self.blend_updates <= 6
            && self.seek_left <= 7
            && (self.counter == u32::MAX || self.counter <= 10_001)
            && [self.frame_bits, self.sampled_bits, self.target_bits]
                .into_iter()
                .all(|b| {
                    let v = f32::from_bits(b);
                    v.is_finite() && (0.0..=2048.0).contains(&v)
                })
            && [self.step_bits, self.configured_step_bits]
                .into_iter()
                .all(|b| {
                    let v = f32::from_bits(b);
                    v.is_finite() && v > 0.0 && v <= 2048.0
                })
            && {
                let v = f32::from_bits(self.end_bits);
                v.is_finite() && (-1.0..=2048.0).contains(&v)
            }
            && self
                .position_bits
                .into_iter()
                .chain(self.previous_bits)
                .all(|b| bounded(f32::from_bits(b)))
    }

    fn encode(self, out: &mut Vec<u8>) {
        out.extend(self.track.to_le_bytes());
        out.extend(self.current_track.to_le_bytes());
        for word in [
            self.frame_bits,
            self.sampled_bits,
            self.step_bits,
            self.configured_step_bits,
            self.end_bits,
            self.target_bits,
            self.counter,
            self.last_tick,
        ]
        .into_iter()
        .chain(self.position_bits)
        .chain(self.previous_bits)
        {
            put32(out, word);
        }
        out.extend([self.blend_updates, self.seek_left, u8::from(self.active)]);
    }

    fn decode(input: &mut Input<'_>) -> Result<Self, String> {
        let track = input.u16()?;
        let current_track = input.u16()?;
        let frame_bits = input.u32()?;
        let sampled_bits = input.u32()?;
        let step_bits = input.u32()?;
        let configured_step_bits = input.u32()?;
        let end_bits = input.u32()?;
        let target_bits = input.u32()?;
        let counter = input.u32()?;
        let last_tick = input.u32()?;
        let mut position_bits = [0; 3];
        let mut previous_bits = [0; 3];
        for word in &mut position_bits {
            *word = input.u32()?;
        }
        for word in &mut previous_bits {
            *word = input.u32()?;
        }
        let blend_updates = input.u8()?;
        let seek_left = input.u8()?;
        let active = match input.u8()? {
            0 => false,
            1 => true,
            _ => return Err("invalid native clock active flag".into()),
        };
        let clock = Self {
            track,
            current_track,
            frame_bits,
            sampled_bits,
            step_bits,
            configured_step_bits,
            end_bits,
            target_bits,
            counter,
            blend_updates,
            seek_left,
            active,
            last_tick,
            position_bits,
            previous_bits,
        };
        if !clock.valid() {
            return Err("invalid native staged clock".into());
        }
        Ok(clock)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub slot: u8,
    /// 0..2 = position, 3 = heading in radians.
    pub field: u8,
    pub start_frame: u32,
    pub end_frame: u32,
    pub start: f32,
    pub end: f32,
    pub start_tangent: f32,
    pub end_tangent: f32,
    pub interpolation: u8,
}

impl Curve {
    /// Position-only 00337088/00337098 arithmetic for resolved staged
    /// records. The retained caller skips start>=frame; the lower clamp
    /// remains the adapter's existing policy rather than native authority.
    fn sample_native_position(self, frame: f32) -> f32 {
        let delta = self.end_frame.saturating_sub(self.start_frame);
        if self.field > 2 || delta == 0 || !matches!(self.interpolation, 1 | 2) {
            return self.sample(frame);
        }
        let t = (position_sub(frame, self.start_frame as f32) / delta as f32).clamp(0.0, 1.0);
        if self.interpolation == 1 {
            return position_add(
                self.start,
                position_mul(t, position_sub(self.end, self.start)),
            );
        }
        let forward = position_sub(self.end, self.start);
        let backward = position_sub(self.start, self.end);
        let a = position_mul(self.start_tangent, (delta + 1) as f32);
        let b = position_mul(self.end_tangent, (delta + 1) as f32);
        let triple = position_mul(forward, 3.0);
        let doubled = position_add(backward, backward);
        let ac = position_add(position_add(a, b), doubled);
        let bc = position_sub(position_sub(triple, b), position_add(a, a));
        position_add(
            position_mul(
                position_add(position_mul(position_add(position_mul(ac, t), bc), t), a),
                t,
            ),
            self.start,
        )
    }

    pub fn sample(self, frame: f32) -> f32 {
        let delta = self.end_frame.saturating_sub(self.start_frame);
        if delta == 0 {
            return self.end;
        }
        let t = ((frame - self.start_frame as f32) / delta as f32).clamp(0.0, 1.0);
        match self.interpolation {
            0 => {
                if t >= 1.0 {
                    self.end
                } else {
                    self.start
                }
            }
            1 => self.start + t * (self.end - self.start),
            2 => {
                let a = self.start_tangent * (delta as f32 + 1.0);
                let b = self.end_tangent * (delta as f32 + 1.0);
                (((a + b + 2.0 * (self.start - self.end)) * t
                    + (3.0 * (self.end - self.start) - b - 2.0 * a))
                    * t
                    + a)
                    * t
                    + self.start
            }
            _ => self.start,
        }
    }
}

impl SequencePlan {
    /// The frames the script plays: up to its stop command, else all.
    pub fn length(&self) -> u32 {
        self.controls
            .stop
            .map_or(self.frame_count, |s| (s + 1).min(self.frame_count))
    }

    /// The camera on screen at `frame`, `held` frames into the freeze: its
    /// id and the frame of its track.
    pub fn camera_at(&self, frame: f32, held: f32) -> (u8, f32) {
        match self.controls.hold {
            Some(h) if held > 0.0 && frame as u32 == h.frame => (h.camera, h.frame as f32 + held),
            _ => (0, frame),
        }
    }

    pub fn initial_positions(&self) -> [Option<[f32; 2]>; 7] {
        let frame = self.pose_at(0.0);
        std::array::from_fn(|slot| {
            let pose = frame[slot]?;
            Some([pose.position[0], pose.position[2]])
        })
    }

    pub fn pose_at(&self, frame: f32) -> [Option<ActorPose>; 7] {
        self.pose_with_animation(frame, true)
    }

    /// Manager scalar positions do not reconstruct immutable animation
    /// clocks; the mutable VM clocks advance separately in constant time.
    pub fn spatial_pose_at(&self, frame: f32) -> [Option<ActorPose>; 7] {
        self.pose_with_animation(frame, false)
    }

    fn pose_with_animation(&self, frame: f32, animation: bool) -> [Option<ActorPose>; 7] {
        let mut result = self.authored_pose_at(frame, animation);
        // Ball opcode 3 dispatches to the source player's +0x134 method
        // (0032f8f0 -> 00334088), not to the ball's float setters. It
        // launches that actor vertically and restores script control after
        // the command's duration. Its x/z velocities are zero meanwhile.
        for cue in &self.ball_cues {
            if cue.opcode() != 3 || cue.target() == 6 || cue.duration == 0 {
                continue;
            }
            let elapsed = frame - cue.frame as f32;
            if elapsed < 0.0 || elapsed > cue.duration as f32 {
                continue;
            }
            let Ok(slot @ 0..=5) = usize::try_from(cue.source()) else {
                continue;
            };
            // Launch physics and its starting position retain the existing
            // fallback until that separate producer is paired to native data.
            let (Some(start), Some(pose)) = (
                self.authored_pose_with_position_policy(cue.frame as f32, animation, false)[slot],
                &mut result[slot],
            ) else {
                continue;
            };
            pose.position = start.position;
            // 00334088's constants and singleton +0x28 (60 Hz). The
            // player's 00157480..001574f4 step adds velocity and half an
            // acceleration to position, then acceleration to velocity.
            // Therefore the same parabola also samples replay subframes.
            let acceleration = f32::from_bits(0x4200_9be3) * f32::from_bits(0xbfa1_3ba4)
                / (crate::TICK_HZ * crate::TICK_HZ) as f32;
            let duration = cue.duration as f32;
            let velocity =
                -(0.5 * acceleration * duration * duration + start.position[1]) / duration;
            pose.position[1] =
                (start.position[1] + velocity * elapsed + 0.5 * acceleration * elapsed * elapsed)
                    .max(0.0);
        }
        result
    }

    fn authored_pose_at(&self, frame: f32, animation: bool) -> [Option<ActorPose>; 7] {
        self.authored_pose_with_position_policy(frame, animation, true)
    }

    fn authored_pose_with_position_policy(
        &self,
        frame: f32,
        animation: bool,
        native_positions: bool,
    ) -> [Option<ActorPose>; 7] {
        let mut result: [Option<ActorPose>; 7] = [None; 7];
        let mut changed = [[f32::NEG_INFINITY; 4]; 7];
        let mut native = [false; 7];
        for track in &self.actors {
            if track.frame as f32 > frame {
                continue;
            }
            let Some(slot) = result.get_mut(usize::from(track.slot)) else {
                continue;
            };
            native[usize::from(track.slot)] = native_positions && track.playback.is_some();
            let mut pose = track.pose;
            pose.animation_frame = if !animation {
                pose.animation_frame
            } else {
                match track.playback {
                    Some(playback) => {
                        let phase = playback.phase_at(frame - track.frame as f32, track.duration);
                        if phase.blending {
                            pose.animation_frame
                                + (frame - track.frame as f32) * track.animation_speed
                        } else {
                            phase.channel_frame
                        }
                    }
                    None => {
                        pose.animation_frame + (frame - track.frame as f32) * track.animation_speed
                    }
                }
            };
            if let Some(previous) = slot {
                if track.keep_position {
                    pose.position = previous.position;
                }
                if track.keep_heading {
                    pose.heading = previous.heading;
                }
            }
            if !track.keep_position {
                changed[usize::from(track.slot)][..3].fill(track.frame as f32);
            }
            if !track.keep_heading {
                changed[usize::from(track.slot)][3] = track.frame as f32;
            }
            *slot = Some(pose);
        }
        for curve in &self.curves {
            if curve.start_frame as f32 >= frame {
                continue;
            }
            let update = frame.min(curve.end_frame as f32);
            let Some(stamp) = changed
                .get_mut(usize::from(curve.slot))
                .and_then(|fields| fields.get_mut(usize::from(curve.field)))
            else {
                continue;
            };
            if update < *stamp {
                continue;
            }
            *stamp = update;
            if let Some(Some(pose)) = result.get_mut(usize::from(curve.slot)) {
                let value = if native[usize::from(curve.slot)] && curve.field <= 2 {
                    curve.sample_native_position(frame)
                } else {
                    curve.sample(frame)
                };
                match curve.field {
                    0..=2 => pose.position[usize::from(curve.field)] = value,
                    3 => pose.heading = value.to_degrees().round_ties_even().to_radians(),
                    _ => (),
                }
            }
        }
        for finish in &self.finishes {
            if frame < finish.start_frame as f32 {
                continue;
            }
            let elapsed = frame - finish.start_frame as f32;
            let Some(span) = finish.spans.iter().find(|span| {
                elapsed >= span.start_tick as f32
                    && elapsed <= (span.start_tick + span.duration) as f32
            }) else {
                continue;
            };
            if let Some(Some(pose)) = result.get_mut(usize::from(finish.slot)) {
                pose.animation = finish.animation;
                let t = (elapsed - span.start_tick as f32) / span.duration.max(1) as f32;
                pose.animation_frame = span.from_frame + (span.to_frame - span.from_frame) * t;
            }
        }
        result
    }

    /// Display sampling uses the last evaluated VM pose, which can differ
    /// from the +64 channel clock used by prospective endpoint evaluation.
    pub fn display_phase_at(&self, slot: usize, frame: f32) -> Option<NativePhase> {
        let track = self
            .actors
            .iter()
            .rfind(|track| usize::from(track.slot) == slot && track.frame as f32 <= frame)?;
        let playback = track.playback?;
        for finish in self
            .finishes
            .iter()
            .filter(|finish| usize::from(finish.slot) == slot && finish.start_frame as f32 <= frame)
        {
            let elapsed = frame - finish.start_frame as f32;
            if finish.spans.iter().any(|span| {
                elapsed >= span.start_tick as f32
                    && elapsed <= (span.start_tick + span.duration) as f32
            }) {
                let phase = self.pose_at(frame)[slot]?.animation_frame;
                return Some(NativePhase {
                    channel_frame: phase,
                    display_frame: phase,
                    looping: false,
                    blending: false,
                });
            }
        }
        Some(playback.phase_at(frame - track.frame as f32, track.duration))
    }

    pub fn runtime_display_phase(&self, sequence: &Sequence, slot: usize) -> Option<NativePhase> {
        let frame = sequence.pose_frame() as f32;
        if self.finishes.iter().any(|finish| {
            usize::from(finish.slot) == slot
                && frame >= finish.start_frame as f32
                && finish.spans.iter().any(|span| {
                    let elapsed = frame - finish.start_frame as f32;
                    elapsed >= span.start_tick as f32
                        && elapsed <= (span.start_tick + span.duration) as f32
                })
        }) {
            return self.display_phase_at(slot, frame);
        }
        let clock = sequence.native_clocks.get(slot).copied().flatten()?;
        Some(clock.phase(self.actors.get(usize::from(clock.track))?.playback?))
    }

    /// Apply the current frame without changing the clock. Ball commands
    /// are handed to the caller and never treated as an inferred basket.
    pub fn apply<'a>(
        &'a self,
        sequence: &Sequence,
        players: &mut [Player],
    ) -> impl Iterator<Item = &'a BallCue> {
        if sequence.frame < self.frame_count {
            let frame = self.pose_at(sequence.frame as f32);
            for (slot, pose) in frame.iter().enumerate() {
                let (Some(pose), Some(player)) = (pose, sequence.actors[slot]) else {
                    continue;
                };
                let Some(player) = players.get_mut(player) else {
                    continue;
                };
                player.position = [
                    if sequence.mirrored() {
                        -pose.position[0]
                    } else {
                        pose.position[0]
                    },
                    pose.position[2],
                ];
                player.height = pose.position[1].max(0.0);
                player.heading = script_heading(pose.heading, sequence.mirrored());
                player.speed = 0.0;
            }
        }
        let frame = sequence.frame;
        self.ball_cues.iter().filter(move |cue| cue.frame == frame)
    }

    /// Where the ball is at `frame`, from the script's ball commands
    /// (dispatch 0): opcode 1 puts it in an actor's hands, opcode 0 throws
    /// it from one end to the other over the command's frames (an actor, or
    /// with -1 a point: the rim, or with byte 16 = 2 a toss up high), opcode
    /// 2 shoots it at the rim. Opcode 3 launches an actor vertically in
    /// [`Self::pose_at`] without moving the ball. Before the first command
    /// the holder (role 0) has it.
    pub fn ball_at(&self, frame: u32) -> BallPlace {
        let mut place = BallPlace::Held(0);
        for cue in &self.ball_cues {
            if cue.frame > frame {
                continue;
            }
            let end = cue.frame.saturating_add(cue.duration);
            let t = if frame >= end {
                1.0
            } else {
                (frame - cue.frame) as f32 / cue.duration.max(1) as f32
            };
            let slot = |v: i16| u8::try_from(v).ok();
            let toss = cue.data[16] == 2;
            let point = |actor: Option<u8>, other: Option<u8>| match actor {
                Some(a) => BallEnd::Actor(a),
                None if toss => BallEnd::Above(other.unwrap_or(0)),
                None => BallEnd::Rim,
            };
            place = match cue.opcode() {
                1 => match slot(cue.source()) {
                    Some(a) => BallPlace::Held(a),
                    None => place,
                },
                0 => {
                    let (from, to) = (slot(cue.source()), slot(cue.target()));
                    if t >= 1.0 {
                        match to {
                            Some(a) => BallPlace::Held(a),
                            None => BallPlace::Flight {
                                from: point(to, from),
                                to: point(to, from),
                                t: 1.0,
                            },
                        }
                    } else {
                        BallPlace::Flight {
                            from: point(from, to),
                            to: point(to, from),
                            t,
                        }
                    }
                }
                2 => BallPlace::Flight {
                    from: slot(cue.source()).map_or(BallEnd::Rim, BallEnd::Actor),
                    to: BallEnd::Rim,
                    t,
                },
                _ => place,
            };
        }
        place
    }

    /// The roles the ball passes through, which must all be bound.
    pub fn ball_roles(&self) -> impl Iterator<Item = u8> + '_ {
        self.ball_cues
            .iter()
            .filter(|cue| cue.opcode() <= 2)
            .flat_map(|cue| [cue.source(), cue.target()])
            .filter_map(|v| u8::try_from(v).ok())
            .chain(std::iter::once(0))
    }
}

/// One end of a staged throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BallEnd {
    /// An actor's hands (a role).
    Actor(u8),
    /// The basket being attacked.
    Rim,
    /// High over an actor (a toss to themselves).
    Above(u8),
}

/// Where a staged sequence has the ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BallPlace {
    Held(u8),
    Flight { from: BallEnd, to: BallEnd, t: f32 },
}

/// A script heading (radians, the model's own turn) as the simulation's
/// heading, mirrored across x for the side attacking +x (team 0).
pub fn script_heading(heading: f32, mirrored: bool) -> f32 {
    let h = if mirrored { -heading } else { heading };
    let h = h - std::f32::consts::PI;
    (h + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionRun {
    pub length: u16,
    pub family: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionMap {
    pub rows: Vec<Vec<RegionRun>>,
}

impl RegionMap {
    pub fn from_rows(rows: &[Vec<u8>]) -> Self {
        Self {
            rows: rows
                .iter()
                .map(|row| {
                    let mut runs: Vec<RegionRun> = Vec::new();
                    for &family in row {
                        if let Some(last) = runs.last_mut()
                            && last.family == family
                            && last.length < u16::MAX
                        {
                            last.length += 1;
                        } else {
                            runs.push(RegionRun { length: 1, family });
                        }
                    }
                    runs
                })
                .collect(),
        }
    }

    pub fn family(&self, position: [f32; 2], team: usize) -> Option<u8> {
        if !position.iter().all(|v| v.is_finite()) {
            return None;
        }
        // The rows are laid out for the side attacking -x: the original
        // negates x for an actor of team 0 (actor+0x284 == 0, raw
        // FUN_00141298), which attacks +x.
        let x = if team == 0 { -position[0] } else { position[0] };
        // `cvt.w.s`, which truncates on the EE (`0x141328`), then the
        // clamps.
        let column = ((((-x + 42.0) / 84.0) * 320.0) as i32).clamp(0, 319) as usize;
        let row = ((((-position[1] + 26.5) / 53.0) * 206.0) as i32).clamp(0, 205) as usize;
        let mut start = 0;
        for run in self.rows.get(row)? {
            start += usize::from(run.length);
            if column < start {
                return Some(run.family);
            }
        }
        None
    }
}

/// The most an encoded staged config ([`Config::encode`]) may be.
pub const MAX_CONFIG_BYTES: usize = 128 * 1024;
/// Bound inflation before allocating its output, independently of wire size.
const MAX_CONFIG_PAYLOAD_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub plans: Vec<SequencePlan>,
    /// Entries refer to `SequencePlan.script`, rather than the plan vector's order.
    pub families: [[Option<usize>; 4]; 13],
    pub regions: RegionMap,
    /// Animated attachment centres in local model units, retaining clip precision.
    pub hands: Vec<HandTrack>,
    /// Model scale per roster player; an empty list uses unit model scale.
    pub player_scales: Vec<f32>,
    /// Native width/height per roster player, clamped by the native narrow-body
    /// branch to one. Size options affect `player_scales`, leaving this unchanged.
    pub player_width_ratios: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandTrack {
    pub animation: u16,
    /// Native animation frames, after the raw state chooses right/left/centre.
    pub positions: Vec<[f32; 3]>,
    /// Additional local displacement for one unit of width/height above one.
    /// Joint transforms are affine in the four native width-adjusted offsets,
    /// so this basis covers each player's geometry without duplicating tracks.
    /// Retain full floats so width adjustment adds no second quantization step.
    /// An empty basis denotes a uniform-body-only synthetic track.
    pub width_deltas: Vec<[f32; 3]>,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.hands.len() > 256
            || self.player_scales.len() > 6
            || self.player_width_ratios.len() > 6
            || (!self.player_width_ratios.is_empty()
                && self.player_width_ratios.len() != self.player_scales.len())
            || self
                .player_width_ratios
                .iter()
                .any(|&ratio| !ratio.is_finite() || !(1.0..=10.0).contains(&ratio))
            || self
                .player_scales
                .iter()
                .any(|&s| !s.is_finite() || !(0.1..=10.0).contains(&s))
        {
            return Err("invalid staged hand tracks or model scales".into());
        }
        let mut hand_ids = [false; 1144];
        for hand in &self.hands {
            if hand.animation >= 1144
                || hand_ids[usize::from(hand.animation)]
                || hand.positions.is_empty()
                || hand.positions.len() > 2048
                || hand.positions.iter().flatten().any(|&v| !bounded(v))
                || (!hand.width_deltas.is_empty()
                    && hand.width_deltas.len() != hand.positions.len())
                || (self.player_width_ratios.iter().any(|&r| r > 1.0)
                    && hand.width_deltas.is_empty())
                || hand.width_deltas.iter().flatten().any(|&v| !bounded(v))
            {
                return Err("invalid staged hand animation".into());
            }
            hand_ids[usize::from(hand.animation)] = true;
        }
        if self.plans.len() > 16
            || (!self.regions.rows.is_empty() && self.regions.rows.len() != 206)
        {
            return Err("invalid staged plan or region row count".into());
        }
        for row in &self.regions.rows {
            if row.iter().any(|r| r.length == 0 || r.family > 12)
                || row.iter().map(|r| usize::from(r.length)).sum::<usize>() > 320
            {
                return Err("invalid staged region row".into());
            }
        }
        let mut seen = [false; 70];
        for plan in &self.plans {
            if plan.script >= 70
                || seen[plan.script]
                || plan.kind > 2
                || plan.frame_count == 0
                || plan.frame_count > 10_000
                || plan.actors.len() > 1024
                || plan.curves.len() > 2048
                || plan.ball_cues.len() > 512
            {
                return Err("invalid staged script metadata".into());
            }
            seen[plan.script] = true;
            for actor in &plan.actors {
                if actor.slot > 6
                    || actor.frame >= plan.frame_count
                    || actor.duration == 0
                    || actor.pose.animation >= 1144
                    || !actor.pose.position.iter().all(|v| bounded(*v))
                    || !bounded(actor.pose.heading)
                    || !bounded(actor.pose.animation_frame)
                    || !bounded(actor.animation_speed)
                    || actor.animation_speed < 0.0
                {
                    return Err("invalid staged actor track".into());
                }
                if let Some(playback) = actor.playback
                    && (!(2..=2048).contains(&playback.frames)
                        || playback.speed_word == 0
                        || playback.speed_word > i16::MAX as u16
                        || playback.end_fraction > i16::MAX as u16
                        || playback.flags[1] == 0
                        || playback.flags[9] != 0
                        || playback.loop_distances.iter().flatten().any(|&bits| {
                            let value = f32::from_bits(bits);
                            !value.is_finite() || value <= 0.0 || value > 100.0
                        })
                        || actor.duration > 10_000
                        || (!playback.initial && !playback.looping() && actor.duration <= 6)
                        || !playback.step().is_finite()
                        || playback.step() <= 0.0
                        || playback.step() > f32::from(playback.frames))
                {
                    return Err("invalid or unsupported staged native playback".into());
                }
            }
            for curve in &plan.curves {
                if curve.slot > 6
                    || curve.field > 3
                    || curve.interpolation > 2
                    || curve.end_frame >= plan.frame_count
                    || curve.start_frame > curve.end_frame
                    || ![
                        curve.start,
                        curve.end,
                        curve.start_tangent,
                        curve.end_tangent,
                    ]
                    .iter()
                    .all(|v| bounded(*v))
                {
                    return Err("invalid staged scalar curve".into());
                }
            }
            let c = plan.controls;
            if c.stop.is_some_and(|s| s >= plan.frame_count)
                || c.hold.is_some_and(|h| {
                    h.frame >= plan.frame_count || !(1..=600).contains(&h.duration) || h.camera > 9
                })
                || c.rerun.is_some_and(|r| {
                    r.from >= r.to
                        || r.to > plan.frame_count
                        || !(25..=150).contains(&r.speed)
                        || r.camera > 9
                })
            {
                return Err("invalid staged controls".into());
            }
            for cue in &plan.ball_cues {
                if cue.frame >= plan.frame_count
                    || cue.duration == 0
                    || cue.data[33] != 0
                    || cue.opcode() > 5
                    || !(-1..=6).contains(&cue.source())
                    || !(-1..=6).contains(&cue.target())
                    || !cue.destination().iter().all(|v| bounded(*v))
                {
                    return Err("invalid staged ball cue".into());
                }
            }
            if plan.finishes.len() > 64 {
                return Err("too many staged finishers".into());
            }
            for finish in &plan.finishes {
                if finish.slot > 6
                    || finish.start_frame >= plan.frame_count
                    || finish.animation >= 1144
                    || finish.spans.len() > 64
                {
                    return Err("invalid staged finisher".into());
                }
                for span in &finish.spans {
                    if span.duration == 0
                        || span.duration > 10_000
                        || span.start_tick > 10_000
                        || !bounded(span.from_frame)
                        || !bounded(span.to_frame)
                    {
                        return Err("invalid staged finisher clock".into());
                    }
                }
            }
        }
        for script in self.families.iter().flatten().flatten() {
            if *script >= 70 || !seen[*script] {
                return Err("staged variant references missing plan".into());
            }
        }
        Ok(())
    }

    /// Compact immutable match metadata; the scene's clips/cameras/sound
    /// bytes do not enter network snapshots.
    pub fn encode(&self) -> Vec<u8> {
        let payload = self.encode_payload();
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&payload[4..], 6);
        let mut out = b"GBC\x08".to_vec();
        put32(&mut out, (payload.len() - 4) as u32);
        out.extend(compressed);
        out
    }

    /// The config as bytes in canonical scalar order (uncompressed), for
    /// the digest the machines in a match compare (`street-net`'s `disc`).
    pub fn canonical(&self) -> Vec<u8> {
        self.encode_payload()
    }

    fn encode_payload(&self) -> Vec<u8> {
        let mut out = b"GBC\x08".to_vec();
        put32(&mut out, self.plans.len() as u32);
        for plan in &self.plans {
            put32(&mut out, plan.script as u32);
            put32(&mut out, plan.kind);
            put32(&mut out, plan.frame_count);
            put32(&mut out, plan.actors.len() as u32);
            for actor in &plan.actors {
                put32(&mut out, actor.frame);
                put32(&mut out, actor.duration);
                out.extend(actor.slot.to_le_bytes());
                for value in actor.pose.position {
                    put_float(&mut out, value);
                }
                put_float(&mut out, actor.pose.heading);
                out.extend(actor.pose.animation.to_le_bytes());
                put_float(&mut out, actor.pose.animation_frame);
                put_float(&mut out, actor.animation_speed);
                out.push(u8::from(actor.keep_position) | (u8::from(actor.keep_heading) << 1));
                out.push(u8::from(actor.playback.is_some()));
                if let Some(playback) = actor.playback {
                    out.extend(playback.frames.to_le_bytes());
                    out.extend(playback.speed_word.to_le_bytes());
                    out.extend(playback.end_fraction.to_le_bytes());
                    out.extend(playback.flags);
                    out.push(u8::from(playback.initial));
                    for distance in playback.loop_distances {
                        put32(&mut out, distance.unwrap_or(0));
                    }
                }
            }
            put32(&mut out, plan.curves.len() as u32);
            for curve in &plan.curves {
                out.extend([curve.slot, curve.field, curve.interpolation]);
                put32(&mut out, curve.start_frame);
                put32(&mut out, curve.end_frame);
                for value in [
                    curve.start,
                    curve.end,
                    curve.start_tangent,
                    curve.end_tangent,
                ] {
                    put_float(&mut out, value);
                }
            }
            put32(&mut out, plan.ball_cues.len() as u32);
            for cue in &plan.ball_cues {
                put32(&mut out, cue.frame);
                put32(&mut out, cue.duration);
                out.extend(cue.data);
            }
            let c = plan.controls;
            put32(&mut out, c.stop.unwrap_or(u32::MAX));
            match c.hold {
                Some(h) => {
                    put32(&mut out, h.frame);
                    put32(&mut out, h.duration);
                    out.push(h.camera);
                    out.push(u8::from(h.holds_replays));
                }
                None => {
                    put32(&mut out, u32::MAX);
                    put32(&mut out, 0);
                    out.extend([0, 0]);
                }
            }
            match c.rerun {
                Some(r) => {
                    put32(&mut out, r.from);
                    put32(&mut out, r.to);
                    out.extend([r.speed, r.camera]);
                }
                None => {
                    put32(&mut out, u32::MAX);
                    put32(&mut out, 0);
                    out.extend([0, 0]);
                }
            }
            put32(&mut out, plan.finishes.len() as u32);
            for finish in &plan.finishes {
                out.push(finish.slot);
                put32(&mut out, finish.start_frame);
                out.extend(finish.animation.to_le_bytes());
                put32(&mut out, finish.release_bits);
                put32(&mut out, finish.spans.len() as u32);
                for span in &finish.spans {
                    put32(&mut out, span.start_tick);
                    put32(&mut out, span.duration);
                    put_float(&mut out, span.from_frame);
                    put_float(&mut out, span.to_frame);
                }
            }
        }
        for family in &self.families {
            for script in family {
                put32(&mut out, script.map_or(u32::MAX, |i| i as u32));
            }
        }
        put32(&mut out, self.regions.rows.len() as u32);
        for row in &self.regions.rows {
            put32(&mut out, row.len() as u32);
            for run in row {
                out.extend(run.length.to_le_bytes());
                out.push(run.family);
            }
        }
        put32(&mut out, self.player_scales.len() as u32);
        for &scale in &self.player_scales {
            put_float(&mut out, scale);
        }
        put32(&mut out, self.player_width_ratios.len() as u32);
        for &ratio in &self.player_width_ratios {
            put_float(&mut out, ratio);
        }
        put32(&mut out, self.hands.len() as u32);
        for hand in &self.hands {
            out.extend(hand.animation.to_le_bytes());
            put32(&mut out, hand.positions.len() as u32);
            put_points(&mut out, &hand.positions);
            put32(&mut out, hand.width_deltas.len() as u32);
            put_points(&mut out, &hand.width_deltas);
        }
        out
    }

    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() > MAX_CONFIG_BYTES {
            return Err("staged configuration exceeds 128 KiB".into());
        }
        let mut header = Input::new(data, b"GBC\x08")?;
        let decoded_size = header.u32()? as usize;
        if decoded_size > MAX_CONFIG_PAYLOAD_BYTES {
            return Err("inflated staged configuration exceeds 1 MiB".into());
        }
        let compressed = &data[8..];
        let mut payload = vec![0; decoded_size];
        use miniz_oxide::inflate::{
            TINFLStatus,
            core::{DecompressorOxide, decompress, inflate_flags},
        };
        let (status, consumed, written) = decompress(
            &mut DecompressorOxide::default(),
            compressed,
            &mut payload,
            0,
            inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
                | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
        );
        if status != TINFLStatus::Done || consumed != compressed.len() || written != decoded_size {
            return Err("invalid compressed staged configuration".into());
        }
        let mut input = Input {
            data: &payload,
            at: 0,
        };
        let count = input.count(16)?;
        let mut plans = Vec::with_capacity(count);
        for _ in 0..count {
            let script = input.u32()? as usize;
            let kind = input.u32()?;
            let frame_count = input.u32()?;
            let mut actors = Vec::new();
            for _ in 0..input.count(1024)? {
                let frame = input.u32()?;
                let duration = input.u32()?;
                let slot = input.u16()?;
                let position = [input.float()?, input.float()?, input.float()?];
                let heading = input.float()?;
                let animation = input.u16()?;
                let animation_frame = input.float()?;
                let animation_speed = input.float()?;
                let flags = input.u8()?;
                if flags > 3 {
                    return Err("invalid staged actor flags".into());
                }
                let playback = match input.u8()? {
                    0 => None,
                    1 => {
                        let frames = input.u16()?;
                        let speed_word = input.u16()?;
                        let end_fraction = input.u16()?;
                        let flags = input.take(16)?.try_into().unwrap();
                        let initial = match input.u8()? {
                            0 => false,
                            1 => true,
                            _ => return Err("invalid staged initial playback flag".into()),
                        };
                        let mut loop_distances = [None; 2];
                        for distance in &mut loop_distances {
                            *distance = Some(input.u32()?).filter(|&bits| bits != 0);
                        }
                        Some(NativePlayback {
                            frames,
                            speed_word,
                            end_fraction,
                            flags,
                            initial,
                            loop_distances,
                        })
                    }
                    _ => return Err("invalid staged playback presence flag".into()),
                };
                actors.push(ActorTrack {
                    frame,
                    duration,
                    slot,
                    pose: ActorPose {
                        position,
                        heading,
                        animation,
                        animation_frame,
                    },
                    animation_speed,
                    playback,
                    keep_position: flags & 1 != 0,
                    keep_heading: flags & 2 != 0,
                });
            }
            let mut curves = Vec::new();
            for _ in 0..input.count(2048)? {
                curves.push(Curve {
                    slot: input.u8()?,
                    field: input.u8()?,
                    interpolation: input.u8()?,
                    start_frame: input.u32()?,
                    end_frame: input.u32()?,
                    start: input.float()?,
                    end: input.float()?,
                    start_tangent: input.float()?,
                    end_tangent: input.float()?,
                });
            }
            let mut ball_cues = Vec::new();
            for _ in 0..input.count(512)? {
                ball_cues.push(BallCue {
                    frame: input.u32()?,
                    duration: input.u32()?,
                    data: input.take(36)?.try_into().unwrap(),
                });
            }
            let stop = Some(input.u32()?).filter(|&s| s != u32::MAX);
            let (frame, duration, camera) = (input.u32()?, input.u32()?, input.u8()?);
            let holds_replays = match input.u8()? {
                0 => false,
                1 => true,
                _ => return Err("invalid staged freeze".into()),
            };
            let hold = (frame != u32::MAX).then_some(Hold {
                frame,
                duration,
                camera,
                holds_replays,
            });
            let (from, to, speed, camera) = (input.u32()?, input.u32()?, input.u8()?, input.u8()?);
            let rerun = (from != u32::MAX).then_some(Rerun {
                from,
                to,
                speed,
                camera,
            });
            let mut finishes = Vec::new();
            for _ in 0..input.count(64)? {
                let slot = input.u8()?;
                let start_frame = input.u32()?;
                let animation = input.u16()?;
                let release_bits = input.u32()?;
                let mut spans = Vec::new();
                for _ in 0..input.count(64)? {
                    spans.push(FinishSpan {
                        start_tick: input.u32()?,
                        duration: input.u32()?,
                        from_frame: input.float()?,
                        to_frame: input.float()?,
                    });
                }
                finishes.push(FinishTrack {
                    slot,
                    start_frame,
                    animation,
                    spans,
                    release_bits,
                });
            }
            plans.push(SequencePlan {
                script,
                kind,
                frame_count,
                actors,
                curves,
                ball_cues,
                controls: Controls { stop, hold, rerun },
                finishes,
            });
        }
        let mut families = [[None; 4]; 13];
        for family in &mut families {
            for slot in family {
                let value = input.u32()?;
                if value != u32::MAX {
                    *slot = Some(value as usize);
                }
            }
        }
        let rows_count = input.count(206)?;
        let mut rows = Vec::with_capacity(rows_count);
        for _ in 0..rows_count {
            let mut row = Vec::new();
            for _ in 0..input.count(320)? {
                row.push(RegionRun {
                    length: input.u16()?,
                    family: input.u8()?,
                });
            }
            rows.push(row);
        }
        let mut player_scales = Vec::new();
        for _ in 0..input.count(6)? {
            player_scales.push(input.float()?);
        }
        let mut player_width_ratios = Vec::new();
        for _ in 0..input.count(6)? {
            player_width_ratios.push(input.float()?);
        }
        let mut hands = Vec::new();
        for _ in 0..input.count(256)? {
            let animation = input.u16()?;
            let count = input.count(2048)?;
            let positions = input.points(count)?;
            let count = input.count(2048)?;
            let width_deltas = input.points(count)?;
            hands.push(HandTrack {
                animation,
                positions,
                width_deltas,
            });
        }
        input.done()?;
        let config = Self {
            plans,
            families,
            regions: RegionMap { rows },
            hands,
            player_scales,
            player_width_ratios,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn plan(&self, script: usize) -> Option<&SequencePlan> {
        self.plans.iter().find(|plan| plan.script == script)
    }

    /// The actual animation attachment sampled the way FUN_00330f98 does:
    /// truncated native animation frame modulo the clip length, then model
    /// heading and player scale. Timeline reflection applies to local x too.
    /// Role `role`'s pose at `frame` with the clip and clip frame he shows:
    /// the plan's, or his native clock's outside a finishing span.
    pub fn clip_pose(
        &self,
        plan: &SequencePlan,
        sequence: &Sequence,
        role: u8,
        frame: f32,
    ) -> Option<ActorPose> {
        let mut pose = plan
            .pose_at(frame)
            .get(usize::from(role))
            .copied()
            .flatten()?;
        if let Some(clock) = sequence
            .native_clocks
            .get(usize::from(role))
            .copied()
            .flatten()
            && !plan.finishes.iter().any(|finish| {
                finish.slot == role
                    && frame >= finish.start_frame as f32
                    && finish.spans.iter().any(|span| {
                        let elapsed = frame - finish.start_frame as f32;
                        elapsed >= span.start_tick as f32
                            && elapsed <= (span.start_tick + span.duration) as f32
                    })
            })
        {
            let actor = plan.actors.get(usize::from(clock.current_track))?;
            pose.animation = actor.pose.animation;
            pose.animation_frame = f32::from_bits(clock.frame_bits)
                + f32::from_bits(clock.step_bits) * (frame - clock.last_tick as f32).max(0.0);
        }
        Some(pose)
    }

    pub fn hand_position(
        &self,
        plan: &SequencePlan,
        sequence: &Sequence,
        role: u8,
        frame: f32,
    ) -> Option<[f32; 3]> {
        let pose = self.clip_pose(plan, sequence, role, frame)?;
        let player = sequence.actors.get(usize::from(role)).copied().flatten()?;
        let track = self
            .hands
            .iter()
            .find(|track| track.animation == pose.animation)?;
        // 00331104 uses CVT.W.S; the EE conversion truncates toward zero.
        let index = pose.animation_frame.trunc().max(0.0) as usize % track.positions.len();
        let scale = self.player_scales.get(player).copied().unwrap_or(1.0);
        let width = self.player_width_ratios.get(player).copied().unwrap_or(1.0) - 1.0;
        let delta = track.width_deltas.get(index).copied().unwrap_or([0.0; 3]);
        let mut local: [f32; 3] = std::array::from_fn(|axis| {
            (track.positions[index][axis] + delta[axis] * width) * scale
        });
        let mut position = pose.position;
        let mut heading = pose.heading;
        if sequence.mirrored() {
            position[0] = -position[0];
            local[0] = -local[0];
            heading = -heading;
        }
        let (sin, cos) = (libm::sinf(heading), libm::cosf(heading));
        Some([
            position[0] + local[0] * cos + local[2] * sin,
            position[1] + local[1],
            position[2] - local[0] * sin + local[2] * cos,
        ])
    }

    pub fn start(
        &self,
        players: &[Player],
        shooter: usize,
        level: u8,
        variety: &mut Variety,
        random: u32,
    ) -> Option<Sequence> {
        let player = players.get(shooter)?;
        if level < 2 {
            return None;
        }
        let family = usize::from(self.regions.family(player.position, player.team)?);
        if family == 0 {
            return None;
        }
        let script = variety.choose(family, self.families.get(family)?, random)?;
        let plan = self.plan(script)?;
        if plan.frame_count == 0 {
            return None;
        }
        Some(Sequence {
            script,
            shooter,
            team: player.team,
            level,
            frame: 0,
            held: 0,
            actors: bind(players, shooter, plan.kind, plan.initial_positions()),
            native_clocks: [None; 7],
            resume_pending: false,
        })
    }
}

/// Each family exhausts its eligible variants before clearing its used flags
/// (`FUN_001851f0`). This is mutable match state and belongs in snapshots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Variety {
    pub used: [u8; 13],
}

impl Variety {
    pub fn encode(self) -> Vec<u8> {
        let mut data = b"GBV\x01".to_vec();
        data.extend(self.used);
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self, String> {
        let mut input = Input::new(data, b"GBV\x01")?;
        let used: [u8; 13] = input.take(13)?.try_into().unwrap();
        if used.iter().any(|&flags| flags > 15) {
            return Err("invalid staged variant flags".into());
        }
        input.done()?;
        Ok(Self { used })
    }
    pub fn choose(
        &mut self,
        family: usize,
        choices: &[Option<usize>; 4],
        random: u32,
    ) -> Option<usize> {
        let used = self.used.get_mut(family)?;
        let eligible = choices.iter().enumerate().fold(0u8, |mask, (i, slot)| {
            mask | if slot.is_some() { 1 << i } else { 0 }
        });
        if eligible == 0 {
            return None;
        }
        if *used & eligible == eligible {
            *used = 0;
        }
        let available = eligible & !*used;
        let mut pick = random % available.count_ones();
        for (i, &slot) in choices.iter().enumerate() {
            if available & (1 << i) == 0 {
                continue;
            }
            if pick == 0 {
                *used |= 1 << i;
                return slot;
            }
            pick -= 1;
        }
        None
    }
}

/// Playback clock and player-role binding. Immutable disc plans live in Config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequence {
    pub script: usize,
    pub shooter: usize,
    pub team: usize,
    pub level: u8,
    pub frame: u32,
    /// Frames into the script's freeze (0 when not frozen).
    pub held: u32,
    pub actors: [Option<usize>; 7],
    pub native_clocks: [Option<NativeClock>; 7],
    /// State7 expiry has enabled actor tasks; forward manager work resumes
    /// on the following world tick.
    pub resume_pending: bool,
}

impl Sequence {
    pub fn pose_frame(self) -> u32 {
        self.native_clocks
            .iter()
            .flatten()
            .map(|clock| clock.last_tick)
            .max()
            .unwrap_or(self.frame)
    }

    /// Manager work precedes the actor VM. Newly started scripts evaluated
    /// immediately in setup and skip their actor update on this same tick.
    pub fn update_native_clocks(&mut self, plan: &SequencePlan, scales: &[f32]) {
        let positions = plan.spatial_pose_at(self.frame as f32);
        for (role, pose) in positions.into_iter().enumerate() {
            let Some((index, actor)) =
                plan.actors.iter().enumerate().rfind(|(_, actor)| {
                    usize::from(actor.slot) == role && actor.frame <= self.frame
                })
            else {
                continue;
            };
            let Some(playback) = actor.playback else {
                self.native_clocks[role] = None;
                continue;
            };
            let Some(pose) = pose else {
                continue;
            };
            let old = self.native_clocks[role];
            if old.is_some_and(|clock| clock.last_tick == self.frame) {
                continue;
            }
            let started = old.is_none_or(|clock| usize::from(clock.track) != index);
            let mut clock = if started {
                NativeClock::start(index, actor, old, pose.position)
            } else {
                old.unwrap()
            };
            if started
                && clock.blend_updates != 0
                && let Some(previous) = plan
                    .actors
                    .get(usize::from(clock.current_track))
                    .and_then(|actor| actor.playback)
            {
                clock.frame_bits = f32::from_bits(clock.frame_bits)
                    .min(f32::from(previous.frames - 1))
                    .to_bits();
            }
            clock.previous_bits =
                old.map_or(pose.position.map(f32::to_bits), |old| old.position_bits);
            clock.position_bits = pose.position.map(f32::to_bits);
            clock.last_tick = self.frame;
            if !started && clock.seek_left != 0 {
                clock.seek_left -= 1;
                if clock.seek_left == 0 {
                    let mut state = clock.clock();
                    playback.manager_seek(&mut state);
                    clock.store(state);
                }
            }
            let scale = self.actors[role]
                .and_then(|player| scales.get(player))
                .copied()
                .unwrap_or(1.0);
            if let Some(step) = playback.motion_step(
                self.mirrored(),
                NativeMotion {
                    position: pose.position,
                    previous: clock.previous_bits.map(f32::from_bits),
                    // 0032bae8 writes signed proxy Y before 0032f2e8
                    // calls 0032c0a0. The actor clamps it later.
                    height: pose.position[1],
                    scale,
                    last_tick: self.frame,
                    blending: clock.blend_updates != 0,
                },
            ) {
                clock.step_bits = step.to_bits();
                clock.configured_step_bits = step.to_bits();
            }
            if !started {
                clock.actor_update(actor);
            }
            self.native_clocks[role] = Some(clock);
        }
    }

    /// State7 expiry enables actor tasks before the next forward manager tick.
    pub fn resume_native_actors(&mut self, plan: &SequencePlan) {
        for clock in self.native_clocks.iter_mut().flatten() {
            if let Some(actor) = plan.actors.get(usize::from(clock.track)) {
                clock.actor_update(actor);
            }
        }
    }

    pub fn encode(self) -> Vec<u8> {
        let mut data = b"GBQ\x03".to_vec();
        put32(&mut data, self.script as u32);
        put32(&mut data, self.frame);
        put32(&mut data, self.held);
        put32(&mut data, self.shooter as u32);
        put32(&mut data, self.team as u32);
        data.push(self.level);
        data.push(u8::from(self.resume_pending));
        for actor in self.actors {
            put32(&mut data, actor.map_or(u32::MAX, |i| i as u32));
        }
        for clock in self.native_clocks {
            data.push(u8::from(clock.is_some()));
            if let Some(clock) = clock {
                clock.encode(&mut data);
            }
        }
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self, String> {
        let mut input = Input::new(data, b"GBQ\x03")?;
        let script = input.u32()? as usize;
        let frame = input.u32()?;
        let held = input.u32()?;
        let shooter = input.u32()? as usize;
        let team = input.u32()? as usize;
        let level = input.u8()?;
        let resume_pending = match input.u8()? {
            0 => false,
            1 => true,
            _ => return Err("invalid native resume flag".into()),
        };
        let mut actors = [None; 7];
        for actor in &mut actors {
            let value = input.u32()?;
            if value != u32::MAX {
                *actor = Some(value as usize);
            }
        }
        let mut native_clocks = [None; 7];
        for clock in &mut native_clocks {
            *clock = match input.u8()? {
                0 => None,
                1 => Some(NativeClock::decode(&mut input)?),
                _ => return Err("invalid native clock presence flag".into()),
            };
        }
        input.done()?;
        if script >= 70
            || frame > 10_000
            || held > 600
            || shooter >= 6
            || team > 1
            || level != 2
            || actors.iter().flatten().any(|&i| i >= 6)
        {
            return Err("invalid staged playback state".into());
        }
        let mut assigned = [false; 6];
        for &actor in actors.iter().flatten() {
            if assigned[actor] {
                return Err("duplicate staged actor binding".into());
            }
            assigned[actor] = true;
        }
        Ok(Self {
            script,
            shooter,
            team,
            level,
            frame,
            held,
            actors,
            native_clocks,
            resume_pending,
        })
    }
    /// The scripts are authored attacking -x: the side attacking +x
    /// (team 0, [`crate::restart::side`]) plays its sequences mirrored
    /// across x.
    pub fn mirrored(self) -> bool {
        crate::restart::side(self.team) > 0.0
    }

    /// Advance after applying the current frame; returns true while more
    /// original frames remain. Completion itself has no scoring side effect.
    pub fn advance(&mut self, frame_count: usize) -> bool {
        self.frame = self.frame.saturating_add(1);
        (self.frame as usize) < frame_count
    }
}

fn put32(data: &mut Vec<u8>, value: u32) {
    data.extend(value.to_le_bytes());
}

fn bounded(value: f32) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.0
}
// Lossless byte-plane storage puts the shared exponent and zero low bits
// beside each other before deflate. No native scalar or quaternion is rounded.
fn put_points(data: &mut Vec<u8>, points: &[[f32; 3]]) {
    for byte in 0..4 {
        for axis in 0..3 {
            for point in points {
                data.push(point[axis].to_bits().to_le_bytes()[byte]);
            }
        }
    }
}

fn put_float(data: &mut Vec<u8>, value: f32) {
    put32(data, value.to_bits());
}

struct Input<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Input<'a> {
    fn new(data: &'a [u8], magic: &[u8; 4]) -> Result<Self, String> {
        if data.get(..4) != Some(magic) {
            return Err("unsupported staged metadata version".into());
        }
        Ok(Self { data, at: 4 })
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(length)
            .ok_or("staged metadata offset overflow")?;
        let bytes = self
            .data
            .get(self.at..end)
            .ok_or("truncated staged metadata")?;
        self.at = end;
        Ok(bytes)
    }
    fn points(&mut self, count: usize) -> Result<Vec<[f32; 3]>, String> {
        // Check the complete bounded block before allocating any track.
        let bytes = self.take(count * 12)?;
        let mut points = vec![[0.0; 3]; count];
        for (frame, point) in points.iter_mut().enumerate() {
            for (axis, value) in point.iter_mut().enumerate() {
                *value = f32::from_bits(u32::from_le_bytes(std::array::from_fn(|byte| {
                    bytes[(byte * 3 + axis) * count + frame]
                })));
            }
        }
        Ok(points)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn float(&mut self) -> Result<f32, String> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err("non-finite staged metadata".into());
        }
        Ok(value)
    }
    fn count(&mut self, maximum: usize) -> Result<usize, String> {
        let value = self.u32()? as usize;
        if value > maximum {
            return Err("staged metadata count exceeds limit".into());
        }
        Ok(value)
    }
    fn done(&self) -> Result<(), String> {
        if self.at != self.data.len() {
            return Err("trailing staged metadata".into());
        }
        Ok(())
    }
}

/// Original actor assignment (`FUN_00337b08`): the holder is role 0;
/// nearest mode assigns roles 1..5 in order, greedily within the right team.
pub fn bind(
    players: &[Player],
    shooter: usize,
    kind: u32,
    starts: [Option<[f32; 2]>; 7],
) -> [Option<usize>; 7] {
    let mut result = [None; 7];
    let Some(holder) = players.get(shooter) else {
        return result;
    };
    let team = holder.team;
    if kind == 0 {
        for side in 0..2 {
            for (slot, (index, _)) in players
                .iter()
                .enumerate()
                .filter(|(_, p)| p.team == if side == 0 { team } else { 1 - team.min(1) })
                .take(3)
                .enumerate()
            {
                result[side * 3 + slot] = Some(index);
            }
        }
        return result;
    }
    result[0] = Some(shooter);
    for slot in 1..6 {
        let required_team = if slot < 3 { team } else { 1 - team.min(1) };
        let start = match starts[slot] {
            Some(start) => start,
            None if kind == 2 => [0.0, 0.0],
            None => continue,
        };
        // The scripts are authored attacking -x ([`Sequence::mirrored`]).
        let mirrored = crate::restart::side(team) > 0.0;
        let start = [if mirrored { -start[0] } else { start[0] }, start[1]];
        let mut best = None;
        let mut distance = f32::INFINITY;
        for (index, player) in players.iter().enumerate() {
            if player.team != required_team || result.contains(&Some(index)) {
                continue;
            }
            let dx = start[0] - player.position[0];
            let dz = start[1] - player.position[1];
            let candidate = dx * dx + dz * dz;
            if best.is_none() || (kind == 1 && candidate < distance) {
                best = Some(index);
                distance = candidate;
            }
        }
        result[slot] = best;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_position_add_sub_preserves_guard_bit_and_signed_cancellation() {
        // Original 00337088/00337098 operand domains, with boundaries from
        // the retained emulator's FPU_ADD_SUB exponent-mask branches.
        for (a, b, subtract, expected) in [
            (0x3f80_0000, 0x3f00_0001, false, 0x3fc0_0000),
            (0x3f80_0000, 0x3f00_0001, true, 0x3eff_fffe),
            (0x3f00_0001, 0x3f80_0000, true, 0xbeff_fffe),
            (0x3f80_0000, 0x33ff_ffff, false, 0x3f80_0000),
            (0x3f80_0000, 0x33ff_ffff, true, 0x3f7f_ffff),
            (0x33ff_ffff, 0x3f80_0000, true, 0xbf7f_ffff),
            (0x3f80_0000, 0x337f_ffff, true, 0x3f80_0000),
            (0x337f_ffff, 0x3f80_0000, true, 0xbf80_0000),
            (0xbf80_0000, 0x33ff_ffff, false, 0xbf7f_ffff),
            (0xbf80_0000, 0x3f7f_ffff, false, 0xb380_0000),
            (0x3f80_0000, 0xbf80_0000, false, 0x0000_0000),
            (0x8000_0000, 0x8000_0000, false, 0x8000_0000),
            (0x8000_0000, 0x0000_0000, true, 0x8000_0000),
            (0x0000_0000, 0x8000_0000, true, 0x0000_0000),
        ] {
            assert_eq!(
                position_add_sub(f32::from_bits(a), f32::from_bits(b), subtract).to_bits(),
                expected,
                "operands {a:08x} {b:08x}, subtract={subtract}"
            );
        }
        assert_eq!(
            position_mul(78.0, f32::from_bits(0x3f99_999a)).to_bits(),
            0x42bb_3333
        );
    }

    fn original_intro31_z_curve() -> Curve {
        // Original SCRIPTS.BIN script31, scalar record18. Slot is remapped
        // from role1 to the single role in this routing fixture.
        Curve {
            slot: 0,
            field: 2,
            start_frame: 445,
            end_frame: 600,
            start: f32::from_bits(0xc0b5_78d5),
            end: f32::from_bits(0x3f67_ef9e),
            start_tangent: f32::from_bits(0x3c23_d70a),
            end_tangent: 0.0,
            interpolation: 1,
        }
    }

    #[test]
    fn original_staged_scalar_tuples_preserve_linear_and_cubic_instruction_order() {
        let linear = original_intro31_z_curve();
        assert_eq!(linear.sample(468.0).to_bits(), 0xc096_3dea);
        assert_eq!(linear.sample_native_position(468.0).to_bits(), 0xc096_3deb);
        // Script31 record13 retains a negative starting tangent. Cancellation
        // here distinguishes original grouping from result-only chopping.
        let cubic = Curve {
            field: 1,
            start_frame: 377,
            end_frame: 385,
            start: 0.0,
            end: 1.5,
            start_tangent: f32::from_bits(0xbd03_126f),
            end_tangent: 0.0,
            interpolation: 2,
            ..linear
        };
        assert_eq!(cubic.sample(378.0).to_bits(), 0x3d17_1a9f);
        assert_eq!(cubic.sample_native_position(378.0).to_bits(), 0x3d17_1aa0);
        for curve in [
            Curve {
                interpolation: 0,
                ..linear
            },
            Curve {
                end_frame: 445,
                ..linear
            },
            Curve { field: 3, ..cubic },
        ] {
            assert_eq!(
                curve.sample_native_position(468.0).to_bits(),
                curve.sample(468.0).to_bits()
            );
        }
        assert_eq!(
            linear.sample_native_position(444.0).to_bits(),
            linear.start.to_bits()
        );
    }

    #[test]
    fn native_position_routing_preserves_heading_legacy_records_and_actor_launch() {
        let mut plan = plan();
        plan.frame_count = 674;
        plan.actors[0].duration = 674;
        plan.curves = vec![original_intro31_z_curve()];
        let legacy = plan.spatial_pose_at(468.0)[0].unwrap();
        assert_eq!(legacy.position[2].to_bits(), 0xc096_3dea);
        plan.actors[0].playback = Some(playback(10, 128, true, false));
        assert_eq!(
            plan.spatial_pose_at(468.0)[0].unwrap().position[2].to_bits(),
            0xc096_3deb
        );

        // Original script16 heading record22 uses degrees in the source;
        // the plan's existing radians conversion and integer-degree setter
        // retain their generic scalar path even when playback is resolved.
        let heading = Curve {
            slot: 0,
            field: 3,
            start_frame: 210,
            end_frame: 250,
            start: 360.0 * (std::f32::consts::PI / 180.0),
            end: 255.0 * (std::f32::consts::PI / 180.0),
            start_tangent: 0.0,
            end_tangent: -9.0 * (std::f32::consts::PI / 180.0),
            interpolation: 1,
        };
        plan.curves.push(heading);
        assert_eq!(
            plan.spatial_pose_at(236.0)[0].unwrap().heading.to_bits(),
            heading
                .sample(236.0)
                .to_degrees()
                .round_ties_even()
                .to_radians()
                .to_bits()
        );
        plan.actors[0].playback = None;
        let legacy_heading = plan.spatial_pose_at(236.0)[0].unwrap().heading;
        plan.actors[0].playback = Some(playback(10, 128, true, false));
        assert_eq!(
            plan.spatial_pose_at(236.0)[0].unwrap().heading.to_bits(),
            legacy_heading.to_bits()
        );

        let mut data = [0; 36];
        data[..2].copy_from_slice(&3u16.to_le_bytes());
        data[4..6].copy_from_slice(&(-1i16).to_le_bytes());
        plan.ball_cues.push(BallCue {
            frame: 468,
            duration: 60,
            data,
        });
        for frame in [468.0, 469.0, 498.0, 528.0] {
            let resolved = plan.spatial_pose_at(frame)[0].unwrap();
            plan.actors[0].playback = None;
            let legacy = plan.spatial_pose_at(frame)[0].unwrap();
            assert_eq!(resolved.position, legacy.position, "launch frame {frame}");
            assert_eq!(resolved.position[2].to_bits(), 0xc096_3dea);
            plan.actors[0].playback = Some(playback(10, 128, true, false));
        }
        assert_ne!(plan.spatial_pose_at(529.0)[0].unwrap().position[2], {
            plan.actors[0].playback = None;
            plan.spatial_pose_at(529.0)[0].unwrap().position[2]
        });
    }

    #[test]
    fn native_loop_motion_preserves_prior_rates_outside_the_writer_domain() {
        let motion = NativeMotion {
            position: [1.0, 0.0, 2.0],
            previous: [0.0; 3],
            height: 0.0,
            scale: 1.0,
            last_tick: 1,
            blending: false,
        };
        assert!(motion.step(0.5).is_some());
        assert_eq!(
            NativeMotion {
                position: motion.previous,
                ..motion
            }
            .step(0.5),
            None
        );
        assert_eq!(
            NativeMotion {
                height: 0.001,
                ..motion
            }
            .step(0.5),
            None
        );
        assert_eq!(
            NativeMotion {
                last_tick: 0,
                ..motion
            }
            .step(0.5),
            None
        );
        assert_eq!(
            NativeMotion {
                last_tick: u32::MAX,
                ..motion
            }
            .step(0.5),
            None
        );
        assert_eq!(
            NativeMotion {
                blending: true,
                ..motion
            }
            .step(0.5),
            None
        );
        let looping = playback(40, 128, true, true);
        assert_eq!(looping.motion_step(false, motion), None);
    }

    fn playback(frames: u16, speed_word: u16, initial: bool, looping: bool) -> NativePlayback {
        let mut flags = [0; 16];
        flags[1] = 1;
        flags[6] = u8::from(looping);
        NativePlayback {
            frames,
            speed_word,
            end_fraction: 256,
            flags,
            initial,
            loop_distances: [None; 2],
        }
    }

    #[test]
    fn unresolved_replacement_clears_the_previous_native_clock() {
        let mut plan = plan();
        plan.actors[0].playback = Some(playback(10, 128, true, false));
        let mut replacement = plan.actors[0];
        replacement.frame = 2;
        replacement.duration = 6;
        replacement.playback = None;
        plan.actors.push(replacement);
        let mut sequence = Sequence {
            script: 0,
            shooter: 0,
            team: 0,
            level: 2,
            frame: 0,
            held: 0,
            actors: [Some(0), None, None, None, None, None, None],
            native_clocks: [None; 7],
            resume_pending: false,
        };
        sequence.update_native_clocks(&plan, &[1.0]);
        assert!(sequence.native_clocks[0].is_some());
        sequence.frame = 2;
        sequence.update_native_clocks(&plan, &[1.0]);
        assert!(sequence.native_clocks[0].is_none());
        assert!(plan.runtime_display_phase(&sequence, 0).is_none());
        assert_eq!(plan.pose_at(3.0)[0].unwrap().animation_frame, 0.5);
        assert_eq!(Sequence::decode(&sequence.encode()).unwrap(), sequence);
    }

    #[test]
    fn native_staged_startup_skip_and_loop_terminal_are_distinct() {
        // Raw 19c628 skips the actor update on setup's tick. AB evaluates
        // zero+step immediately; 8C's subunit -1 path evaluates zero instead.
        let finite = playback(10, 128, true, false);
        assert_eq!(finite.phase_at(0.0, 100).display_frame, 0.0);
        assert_eq!(finite.phase_at(1.0, 100).display_frame, 0.5);
        assert_eq!(finite.phase_at(18.0, 100).display_frame, 9.0);
        assert_eq!(finite.phase_at(20.0, 100).display_frame, 9.0);
        let fast = playback(10, 384, true, false);
        assert_eq!(fast.phase_at(0.0, 100).display_frame, 0.5);
        assert_eq!(fast.phase_at(1.0, 100).display_frame, 2.0);
        let looping = playback(4, 128, true, true);
        assert_eq!(looping.phase_at(0.0, 100).display_frame, 0.5);
        assert_eq!(looping.phase_at(6.0, 100).display_frame, 3.5);
        assert_eq!(looping.phase_at(7.0, 100).display_frame, 0.0);
        assert_eq!(looping.phase_at(8.0, 100).display_frame, 0.5);
        assert!(looping.phase_at(6.0, 100).looping);
    }

    #[test]
    fn native_staged_blend_target_and_delayed_manager_seek_use_different_counts() {
        let mut finite = playback(10, 128, false, false);
        finite.flags[10] = 128;
        finite.end_fraction = 128;
        // 32b500 corrects period by six updates: (9*2-6)/9 -> step .75.
        assert_eq!(finite.step(), 0.75);
        // 32b6b8's target uses N-1. 32b810's tick-seven seek uses N.
        for tick in 0..6 {
            assert_eq!(finite.phase_at(tick as f32, 24).display_frame, 4.0);
        }
        assert_eq!(finite.phase_at(6.0, 24).display_frame, 4.75);
        assert_eq!(finite.phase_at(7.0, 24).display_frame, 5.75);
        // With duration12, the supplied end is evaluated before natural
        // clip exhaustion. Duration24 exhausts first and keeps its last pose.
        assert_eq!(finite.phase_at(11.0, 12).display_frame, 4.0);
        assert_eq!(finite.phase_at(12.0, 12).display_frame, 4.0);
        assert_eq!(
            finite.phase_at(24.0, 24),
            NativePhase {
                channel_frame: 4.0,
                display_frame: 9.0,
                looping: false,
                blending: false,
            }
        );
        finite.flags[7] = 1;
        assert_eq!(finite.blend_ticks(), 0);
        assert_eq!(finite.phase_at(0.0, 24).display_frame, 0.0);
        assert_eq!(finite.phase_at(6.0, 24).display_frame, 4.5);
        assert_eq!(finite.phase_at(7.0, 24).display_frame, 5.75);
        let looping = playback(4, 128, false, true);
        for tick in 0..6 {
            assert_eq!(looping.phase_at(tick as f32, 24).display_frame, 0.0);
        }
        assert_eq!(looping.phase_at(6.0, 24).display_frame, 0.5);
        assert_eq!(looping.phase_at(7.0, 24).display_frame, 1.0);
    }

    #[test]
    fn native_staged_seek_and_completion_do_not_repose_the_skeleton() {
        let mut finite = playback(10, 128, true, false);
        finite.flags[10] = 128;
        assert_eq!(
            finite.phase_at(0.0, 100),
            NativePhase {
                channel_frame: 5.0,
                display_frame: 0.0,
                looping: false,
                blending: false,
            }
        );
        assert_eq!(finite.phase_at(1.0, 100).display_frame, 5.5);
        finite.flags[10] = 0;
        finite.end_fraction = 128;
        // Natural clip completion comes before duration100. The final pose
        // stays at9; 19a888 changes +64 to end4 and returns without sampling.
        assert_eq!(
            finite.phase_at(20.0, 100),
            NativePhase {
                channel_frame: 4.0,
                display_frame: 9.0,
                looping: false,
                blending: false,
            }
        );
        assert_eq!(
            finite.phase_at(10_001.0, 100),
            finite.phase_at(10_000.0, 100)
        );
    }

    #[test]
    fn native_staged_playback_metadata_roundtrips_and_changes_the_checksum() {
        let mut config = Config {
            plans: vec![plan()],
            ..Config::default()
        };
        config.plans[0].actors[0].playback = Some(playback(10, 128, true, false));
        let encoded = config.encode();
        assert_eq!(&encoded[..4], b"GBC\x08");
        assert_eq!(Config::decode(&encoded).unwrap(), config);
        let original = config.canonical();
        config.plans[0].actors[0].playback.as_mut().unwrap().flags[8] = 1;
        assert_ne!(config.canonical(), original);
        config.plans[0].actors[0]
            .playback
            .as_mut()
            .unwrap()
            .speed_word = 0;
        assert!(config.validate().is_err());
        let playback = config.plans[0].actors[0].playback.as_mut().unwrap();
        playback.speed_word = 128;
        playback.end_fraction = 0x8000;
        assert!(
            config.validate().is_err(),
            "native LH negative end fractions are outside this bridge"
        );
    }

    fn plan() -> SequencePlan {
        SequencePlan {
            script: 0,
            kind: 1,
            frame_count: 8,
            actors: vec![ActorTrack {
                frame: 0,
                duration: 8,
                slot: 0,
                pose: ActorPose {
                    position: [2.0, 0.0, 1.0],
                    heading: 0.0,
                    animation: 7,
                    animation_frame: 0.0,
                },
                animation_speed: 0.5,
                playback: None,
                keep_position: false,
                keep_heading: false,
            }],
            curves: vec![Curve {
                slot: 0,
                field: 0,
                start_frame: 0,
                end_frame: 4,
                start: 2.0,
                end: 10.0,
                start_tangent: 0.0,
                end_tangent: 0.0,
                interpolation: 1,
            }],
            ball_cues: vec![],
            controls: Controls::default(),
            finishes: Vec::new(),
        }
    }

    #[test]
    fn compact_codec_rejects_truncation_nonfinite_values_and_missing_references() {
        let mut config = Config {
            plans: vec![plan()],
            ..Config::default()
        };
        config.families[9][0] = Some(0);
        let bytes = config.encode();
        assert_eq!(Config::decode(&bytes).unwrap(), config);
        for end in 0..bytes.len() {
            assert!(Config::decode(&bytes[..end]).is_err());
        }
        config.plans[0].actors[0].pose.position[1] = f32::NAN;
        assert!(Config::decode(&config.encode()).is_err());
        config.plans[0].actors[0].pose.position[1] = 0.0;
        config.families[9][0] = Some(69);
        assert!(Config::decode(&config.encode()).is_err());
        assert!(Config::decode(&vec![0; 128 * 1024 + 1]).is_err());
    }

    #[test]
    fn endpoint_frame_conversion_truncates_before_wrapping() {
        let mut plan = plan();
        plan.curves.clear();
        plan.actors[0].pose.position = [0.0; 3];
        plan.actors[0].animation_speed = 1.0;
        let config = Config {
            hands: vec![HandTrack {
                animation: 7,
                positions: vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
                width_deltas: Vec::new(),
            }],
            ..Config::default()
        };
        // Team 1 attacks -x and plays the script as authored.
        let sequence = Sequence {
            script: 0,
            shooter: 0,
            team: 1,
            level: 2,
            frame: 0,
            held: 0,
            actors: [Some(0), None, None, None, None, None, None],
            native_clocks: [None; 7],
            resume_pending: false,
        };
        assert_eq!(
            config.hand_position(&plan, &sequence, 0, 0.75),
            Some([1.0, 2.0, 3.0])
        );
        assert_eq!(
            config.hand_position(&plan, &sequence, 0, 1.75),
            Some([4.0, 5.0, 6.0])
        );
        assert_eq!(
            config.hand_position(&plan, &sequence, 0, 2.75),
            Some([1.0, 2.0, 3.0])
        );
    }

    #[test]
    fn hand_geometry_roundtrip_uses_bound_players_width_and_outer_size() {
        let mut config = Config {
            hands: vec![HandTrack {
                animation: 7,
                positions: vec![[1.0, 2.0, 3.0]],
                width_deltas: vec![[0.25, -0.5, 0.125]],
            }],
            player_scales: vec![1.0, 0.5, 2.0],
            player_width_ratios: vec![1.0, 1.5, 1.0],
            ..Config::default()
        };
        config.validate().unwrap();
        let mut precision = config.clone();
        precision.hands[0].positions.push([-0.0, 2.345_67, 1e-7]);
        precision.hands[0]
            .width_deltas
            .push([0.123_456_7, -1e-8, -0.0]);
        let decoded = Config::decode(&precision.encode()).unwrap();
        for (actual, expected) in decoded.hands[0]
            .positions
            .iter()
            .flatten()
            .chain(decoded.hands[0].width_deltas.iter().flatten())
            .zip(
                precision.hands[0]
                    .positions
                    .iter()
                    .flatten()
                    .chain(precision.hands[0].width_deltas.iter().flatten()),
            )
        {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        config = Config::decode(&config.encode()).unwrap();
        // Team 1 plays the script as authored, team 0 (attacking +x)
        // mirrored.
        let mut sequence = Sequence {
            script: 0,
            shooter: 1,
            team: 1,
            level: 2,
            frame: 0,
            held: 0,
            actors: [Some(1), None, None, None, None, None, None],
            native_clocks: [None; 7],
            resume_pending: false,
        };
        assert_eq!(
            config.hand_position(&plan(), &sequence, 0, 0.0),
            Some([2.5625, 0.875, 2.53125])
        );
        sequence.team = 0;
        assert_eq!(
            config.hand_position(&plan(), &sequence, 0, 0.0),
            Some([-2.5625, 0.875, 2.53125])
        );
        // Identical width ratio with a different outer height/size multiplies
        // the complete local point, rather than modifying the width ratio.
        config.player_scales[1] = 1.0;
        sequence.team = 1;
        assert_eq!(
            config.hand_position(&plan(), &sequence, 0, 0.0),
            Some([3.125, 1.75, 4.0625])
        );
        for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0, 10.1] {
            config.player_width_ratios[1] = invalid;
            assert!(config.validate().is_err());
        }
        config.player_width_ratios[1] = 1.5;
        config.hands[0].width_deltas.clear();
        assert!(
            config.validate().is_err(),
            "wide players require the geometry basis"
        );
    }

    #[test]
    fn hand_codec_rejects_invalid_geometry_trailing_streams_and_inflation_overflow() {
        let mut config = Config {
            hands: vec![HandTrack {
                animation: 7,
                positions: vec![[1.0, 2.0, 3.0]],
                width_deltas: vec![[0.25, -0.5, 0.125]],
            }],
            ..Config::default()
        };
        let encoded = config.encode();
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(Config::decode(&trailing).is_err());
        let mut oversized = encoded;
        oversized[4..8].copy_from_slice(&((MAX_CONFIG_PAYLOAD_BYTES + 1) as u32).to_le_bytes());
        assert!(Config::decode(&oversized).is_err());
        let mut bomb = b"GBC\x08".to_vec();
        put32(&mut bomb, MAX_CONFIG_PAYLOAD_BYTES as u32);
        bomb.extend(miniz_oxide::deflate::compress_to_vec_zlib(
            &vec![0; MAX_CONFIG_PAYLOAD_BYTES + 1],
            6,
        ));
        assert!(bomb.len() < MAX_CONFIG_BYTES);
        assert!(Config::decode(&bomb).is_err());
        config.hands[0].width_deltas.push([0.0; 3]);
        assert!(Config::decode(&config.encode()).is_err());
        config.hands[0].width_deltas.pop();
        config.hands[0].width_deltas[0][1] = f32::NAN;
        assert!(Config::decode(&config.encode()).is_err());
        config.hands[0].width_deltas[0][1] = 0.0;
        config.hands[0].positions[0][0] = f32::INFINITY;
        assert!(Config::decode(&config.encode()).is_err());
    }

    #[test]
    fn curves_keep_moving_across_animation_replacement_and_yield_to_new_positions() {
        let mut plan = plan();
        let mut replacement = plan.actors[0];
        replacement.frame = 2;
        replacement.pose.animation = 9;
        replacement.pose.position[0] = 99.0;
        replacement.keep_position = true;
        plan.actors.push(replacement);
        let pose = plan.pose_at(3.0)[0].unwrap();
        assert_eq!(pose.position[0], 8.0);
        assert_eq!(pose.animation, 9);
        assert_eq!(pose.animation_frame, 0.5);
        replacement.frame = 6;
        replacement.keep_position = false;
        plan.actors.push(replacement);
        assert_eq!(plan.pose_at(7.0)[0].unwrap().position[0], 99.0);
    }

    #[test]
    fn actor_launch_matches_the_original_velocity_and_half_gravity_step() {
        let mut plan = plan();
        plan.frame_count = 80;
        plan.actors[0].pose.position[1] = 4.0;
        let mut data = [0; 36];
        data[..2].copy_from_slice(&3u16.to_le_bytes());
        data[4..6].copy_from_slice(&(-1i16).to_le_bytes());
        plan.ball_cues.push(BallCue {
            frame: 2,
            duration: 60,
            data,
        });
        let acceleration = -40.5 / 3600.0;
        let mut height = 4.0;
        let mut velocity = -(0.5 * acceleration * 60.0 * 60.0 + height) / 60.0;
        for tick in 0..=60 {
            let pose = plan.pose_at(2.0 + tick as f32)[0].unwrap();
            assert!((pose.position[1] - height).abs() < 0.0001, "tick {tick}");
            assert_eq!(pose.position[0], 6.0);
            assert_eq!(pose.position[2], 1.0);
            height = (height + velocity + 0.5 * acceleration).max(0.0);
            velocity += acceleration;
        }
        assert!((plan.pose_at(32.0)[0].unwrap().position[1] - 7.0625).abs() < 0.0001);
        assert!(plan.pose_at(62.0)[0].unwrap().position[1] < 0.0001);
        assert_eq!(plan.pose_at(63.0)[0].unwrap().position, [10.0, 4.0, 1.0]);
        assert_eq!(plan.ball_at(32), BallPlace::Held(0));
        // The ball listener ignores commands with either actor sentinel 6.
        plan.ball_cues[0].data[4..6].copy_from_slice(&6i16.to_le_bytes());
        assert_eq!(plan.pose_at(32.0)[0].unwrap().position, [10.0, 4.0, 1.0]);
    }

    #[test]
    fn mutable_playback_and_variety_roundtrip_without_aliasing_roles() {
        let sequence = Sequence {
            script: 1,
            shooter: 0,
            team: 0,
            level: 2,
            frame: 47,
            held: 0,
            actors: [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None],
            native_clocks: [None; 7],
            resume_pending: false,
        };
        assert_eq!(Sequence::decode(&sequence.encode()).unwrap(), sequence);
        let mut invalid = sequence;
        invalid.actors[1] = Some(0);
        assert!(Sequence::decode(&invalid.encode()).is_err());
        let variety = Variety { used: [3; 13] };
        assert_eq!(Variety::decode(&variety.encode()).unwrap(), variety);
        assert!(Variety::decode(&Variety { used: [16; 13] }.encode()).is_err());
    }

    #[test]
    fn variants_do_not_repeat_until_the_family_is_exhausted() {
        let mut variety = Variety::default();
        let choices = [Some(10), Some(11), None, Some(13)];
        assert_eq!(variety.choose(9, &choices, 1), Some(11));
        assert_eq!(variety.choose(9, &choices, 1), Some(13));
        assert_eq!(variety.choose(9, &choices, 1), Some(10));
        assert_eq!(variety.choose(9, &choices, 1), Some(11));
        assert_eq!(variety.choose(0, &[None; 4], 7), None);
    }

    #[test]
    fn nearest_roles_stay_with_their_team_and_keep_the_holder_at_zero() {
        let players: Vec<_> = [
            (0, [-30.0, 0.0]),
            (0, [-10.0, 0.0]),
            (0, [-20.0, 0.0]),
            (1, [-32.0, 0.0]),
            (1, [-12.0, 0.0]),
            (1, [-22.0, 0.0]),
        ]
        .into_iter()
        .map(|(team, position)| Player::new(team, position, 0.0))
        .collect();
        // The script's own starts (authored attacking -x): team 0, which
        // attacks +x, plays them mirrored.
        let starts = [
            Some([30.0, 0.0]),
            Some([20.0, 0.0]),
            Some([10.0, 0.0]),
            Some([22.0, 0.0]),
            Some([12.0, 0.0]),
            Some([32.0, 0.0]),
            None,
        ];
        assert_eq!(
            bind(&players, 0, 1, starts),
            [Some(0), Some(2), Some(1), Some(5), Some(4), Some(3), None]
        );
    }

    #[test]
    fn compact_region_rows_preserve_missing_columns_and_team_reflection() {
        let map = RegionMap::from_rows(&vec![
            {
                let mut row = vec![1; 160];
                row.extend([2; 155]);
                row
            };
            206
        ]);
        assert_eq!(map.rows[0].len(), 2);
        // Team 0 (attacking +x) reads the map reflected, team 1 as laid
        // out (FUN_00141298).
        assert_eq!(map.family([20.0, 0.0], 1), Some(1));
        assert_eq!(map.family([20.0, 0.0], 0), Some(2));
        assert_eq!(map.family([42.0, 0.0], 0), None);
        assert_eq!(map.family([-42.0, 0.0], 1), None);
    }
}
