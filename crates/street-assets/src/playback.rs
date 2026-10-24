//! A move script played the way the actor's animation channel plays it,
//! for the display: the clip, its frame and the blend on each update of
//! the script's own clock.
//!
//! The animation VM (`FUN_00193440`) runs a script's opcodes back to back
//! inside one update until a blend (`8e`, `FUN_001978b0`), a wait (`8d`,
//! `FUN_00197298`), a play-out (`8c`, `FUN_00196f38`) or a free rate
//! (`ab`, `FUN_00197168`) installs a span; the channel's step
//! (`FUN_0019a658`, run once in the opcode's own update and once an update
//! after) carries the span: a blend holds the outgoing clip's frame and
//! mixes the incoming clip at its start frame by `counter / length`, then
//! makes it current; a wait moves the frame by `change / ticks` an update
//! and lands it exactly on its last; a play-out goes to the clip's last
//! frame at a frame each `x` updates; the free rate runs the clip for
//! good. Once a span's counter passes its length the opcodes run again in
//! that update. Game callbacks (`83 id`) are instant; `84 id` jumps when
//! the callback answers nonzero, `85` when zero. The channel keeps an
//! integer frame (`+0x5c`, the float frame truncated after every step)
//! that each wait starts from. Traced raw 2026-10-15 (docs/TRACE.md
//! "Shots, passes and catches on the script clock" and its display
//! section).
//!
//! The rewrite's own, labelled here: the airtime wait (`8d ee B fe`,
//! `FUN_0019bed8`'s updates until the actor lands) is one update (the
//! simulation's own jump lands the player at the script's traced end,
//! which counts that wait as nothing); the flight wait's ball state is
//! taken as a pass's whenever a flight is given; a blend's conditional
//! alternates are evaluated only for `0xf8` (the live left hand), the
//! others play the primary.

use crate::{
    Result,
    scripts::{ClipTarget, Executable, decode},
};

/// A blend's target record meaning the record already on the channel
/// (`-1000`, `FUN_001978b0`).
pub const CURRENT_RECORD: i32 = -1000;

/// The frame a wait reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitFrame {
    /// `8d ee B C`: frame B of the clip.
    Absolute(i32),
    /// `8d B C`: B frames on from the channel's integer frame.
    Relative(i32),
}

/// How long a wait lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitTicks {
    /// 1..127 updates.
    Fixed(u8),
    /// `0xfd`: the rest of the ball's flight (`ball+0x64 - ball+0x60`),
    /// at least a third of the frames to go.
    Flight,
    /// `0xfe`: until the actor lands (`FUN_0019bed8`).
    Airtime,
    /// `0xef D` or a count outside 1..127: not played here.
    Other,
}

/// One instruction of a script's path that the channel's clock sees.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// `8e`: blend over `frames` updates to a record at `start`. The
    /// primary target first, then one alternate per selector.
    Blend {
        frames: u8,
        start: i32,
        selectors: Vec<u8>,
        targets: Vec<ClipTarget>,
    },
    /// `8d`: reach a frame over some updates.
    Wait { to: WaitFrame, ticks: WaitTicks },
    /// `8c x`: the rest of the clip at a frame each `x` updates.
    PlayOut { per_frame: f32 },
    /// `ab x`: the clip on its own at a frame each `x` updates, for good.
    Rate { per_frame: f32 },
    /// `83 id`: a game callback, instant.
    Callback(u32),
    /// A byte 1..0x7f: a frame on, then that many updates asleep.
    Advance(u8),
    /// `81`: the script is over.
    End,
}

/// A script's opcodes along one path, in the order the VM runs them.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub address: u32,
    pub ops: Vec<Op>,
}

/// Which way a shot script's check goes: the plain path jumps at `85`
/// (callback 20 answered zero, the button held) and falls through `84`
/// (callback 17 answered zero, no fadeaway); the pump fake falls through
/// the `85` of callback 20. A dunk's hang on the rim takes callback 28's
/// other way (`FUN_00172400` answered: `85 28` falls through, `84 28`
/// jumps). A rebound or block jump's arm takes callback 47's or 48's
/// branch (`84 2f`, `84 30` answered: they jump).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Plain,
    Fake,
    Hang,
    Branch,
}

/// Compile the script at `address` along `path`: `82`, `9a` and `87`
/// jump, `85` jumps (`Path::Fake` falls through callback 20's), `84` and
/// the animation tests fall through, until `81` or a loop.
pub fn compile(data: &[u8], address: u32, path: Path) -> Result<Program> {
    let elf = Executable::new(data)?;
    let names: Vec<(Option<String>, bool)> = Vec::new();
    let mut ops = Vec::new();
    let mut pc = address;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..600 {
        if !seen.insert(pc) || elf.byte(pc).is_err() {
            break;
        }
        let op = elf.byte(pc)?;
        let ins = decode(&elf, &names, pc)?;
        match op {
            0x01..=0x7f => ops.push(Op::Advance(op)),
            0x81 => {
                ops.push(Op::End);
                return Ok(Program { address, ops });
            }
            0x83 => {
                if let Some(id) = ins.callback {
                    ops.push(Op::Callback(id));
                    // Callback 1 hands the channel to the locomotion
                    // (`FUN_001722c0`): the script ends there whether or
                    // not an `81` follows.
                    if id == 1 {
                        ops.push(Op::End);
                        return Ok(Program { address, ops });
                    }
                }
            }
            0x8c | 0xab => {
                let bytes = elf.bytes(pc + 1, 4)?;
                let per_frame = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                ops.push(if op == 0x8c {
                    Op::PlayOut { per_frame }
                } else {
                    Op::Rate { per_frame }
                });
            }
            0x8d => {
                let (to, count, special) = if elf.byte(pc + 1)? == 0xee {
                    (
                        WaitFrame::Absolute(i32::from(elf.byte(pc + 2)? as i8)),
                        elf.byte(pc + 3)?,
                        elf.byte(pc + 4).is_ok_and(|b| b == 0xef),
                    )
                } else {
                    (
                        WaitFrame::Relative(i32::from(elf.byte(pc + 1)? as i8)),
                        elf.byte(pc + 2)?,
                        elf.byte(pc + 3).is_ok_and(|b| b == 0xef),
                    )
                };
                let ticks = match count {
                    _ if special => WaitTicks::Other,
                    1..=127 => WaitTicks::Fixed(count),
                    0xfd => WaitTicks::Flight,
                    0xfe => WaitTicks::Airtime,
                    _ => WaitTicks::Other,
                };
                ops.push(Op::Wait { to, ticks });
            }
            0x8e => {
                if let Some(blend) = &ins.blend {
                    ops.push(Op::Blend {
                        frames: blend.frames.max(0) as u8,
                        start: blend.start_frame,
                        selectors: blend.selectors.clone(),
                        targets: blend.targets.clone(),
                    });
                }
            }
            _ => {}
        }
        pc = match op {
            0x82 | 0x9a | 0x87 => ins.branches[0],
            0x85 if path == Path::Fake && ins.callback == Some(20) => ins.next,
            0x85 if path == Path::Hang && ins.callback == Some(28) => ins.next,
            0x84 if path == Path::Hang && ins.callback == Some(28) => ins.branches[0],
            0x84 if path == Path::Branch && matches!(ins.callback, Some(47 | 48)) => {
                ins.branches[0]
            }
            0x85 => ins.branches[0],
            _ if !ins.fallthrough => break,
            _ => ins.next,
        };
    }
    ops.push(Op::End);
    Ok(Program { address, ops })
}

/// What a script's clock needs from the game: the clips' lengths, the
/// pass's flight and which blend alternate the actor takes.
pub trait Clock {
    /// Frames of an animation record, if the clip is known.
    fn frames(&self, record: i32) -> Option<i32>;
    /// The pass's nominal flight in updates (`ball+0x64`), while the ball
    /// is in a pass; `None` otherwise.
    fn flight(&self) -> Option<i32>;
    /// The actor's live hand is the left one (`+0x25c` = 2): a blend with
    /// the `0xf8` selector takes its alternate.
    fn left_hand(&self) -> bool;
    /// The script tick the actor lands on, while a jump the simulation
    /// knows the end of carries him; `None` otherwise.
    fn landing(&self) -> Option<i32> {
        None
    }
}

/// A clock with fixed answers.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedClock {
    /// Frames of every record.
    pub frames: Option<i32>,
    pub flight: Option<i32>,
    pub left_hand: bool,
}

impl Clock for FixedClock {
    fn frames(&self, _record: i32) -> Option<i32> {
        self.frames
    }
    fn flight(&self) -> Option<i32> {
        self.flight
    }
    fn left_hand(&self) -> bool {
        self.left_hand
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SpanKind {
    Blend,
    Wait,
    PlayOut,
    Free,
}

#[derive(Clone, Debug, PartialEq)]
struct Span {
    kind: SpanKind,
    /// `+0x6c` and `+0x70`.
    length: i32,
    counter: i32,
    /// `+0x68`: frames an update.
    rate: f32,
    /// `+0x60`.
    to_frame: i32,
    /// `+0x54`: the blend's incoming record.
    to_record: Option<i32>,
    /// The outgoing clip a blend holds: its record (`None` for the actor's
    /// own clip before the script's first blend) and frame.
    from: Option<(Option<i32>, f32)>,
}

/// A blend in progress: the outgoing record and frame it holds (`None`
/// for the actor's own clip before the script's first blend) and the
/// incoming clip's weight (`counter / length`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendState {
    pub from: Option<(Option<i32>, f32)>,
    pub weight: f32,
}

/// What the channel shows on one update.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPose {
    /// The record whose frame is shown: during a blend the incoming one.
    /// `None` before the script's first blend reaches a clip.
    pub record: Option<i32>,
    /// Its frame (`+0x64`; a blend's incoming clip sits at its start).
    pub frame: f32,
    /// A blend in progress.
    pub blend: Option<BlendState>,
    /// The script has ended (`81`): nothing holds the channel.
    pub done: bool,
}

/// An actor's animation channel running a [`Program`].
#[derive(Clone, Debug, PartialEq)]
pub struct Channel<'a> {
    program: &'a Program,
    pc: usize,
    /// `+0x4c`: updates until the opcodes run again.
    count: i32,
    /// `+0x50`.
    record: Option<i32>,
    /// `+0x64` and `+0x5c`.
    frame: f32,
    int_frame: i32,
    span: Option<Span>,
    /// Updates since the script started.
    tick: i32,
    done: bool,
    /// Callbacks run this update.
    callbacks: Vec<u32>,
}

enum After {
    Sleep,
    Run,
}

impl<'a> Channel<'a> {
    /// A channel the script is about to start on: the actor's own clip is
    /// on it (no record), the VM due in the first update (`FUN_0019c320`
    /// sets the count to 1 and runs it at once).
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            pc: 0,
            count: 1,
            record: None,
            frame: 0.0,
            int_frame: -1,
            span: None,
            tick: -1,
            done: false,
            callbacks: Vec::new(),
        }
    }

    /// Updates since the script started (the first `step` is update 0).
    pub fn tick(&self) -> i32 {
        self.tick
    }

    /// The callbacks the last `step` ran.
    pub fn callbacks(&self) -> &[u32] {
        &self.callbacks
    }

    /// One update: the count down, the span's step, then the opcodes until
    /// one installs a span (`FUN_00193440`).
    pub fn step(&mut self, clock: &dyn Clock) {
        self.tick += 1;
        self.callbacks.clear();
        if self.done {
            return;
        }
        self.count -= 1;
        if self.count > 0 {
            return;
        }
        if self.span.is_some()
            && let After::Sleep = self.advance(clock)
        {
            return;
        }
        self.run(clock);
    }

    /// `FUN_0019a658(-1)`: the span's step.
    fn advance(&mut self, clock: &dyn Clock) -> After {
        let Some(span) = &mut self.span else {
            return After::Run;
        };
        if span.kind == SpanKind::Free {
            // `ab`: the clip runs on at its rate, wrapping, for good.
            self.frame += span.rate;
            if let Some(n) = self.record.and_then(|r| clock.frames(r))
                && n > 0
                && self.frame >= n as f32
            {
                self.frame -= n as f32;
            }
            self.int_frame = self.frame as i32;
            self.count = 1;
            return After::Sleep;
        }
        span.counter += 1;
        if span.counter > span.length {
            match span.kind {
                SpanKind::Blend => {
                    self.record = span.to_record;
                    self.frame = span.to_frame as f32;
                }
                _ => self.frame = span.to_frame as f32,
            }
            self.int_frame = span.to_frame;
            self.span = None;
            self.count = 0;
            return After::Run;
        }
        if span.kind != SpanKind::Blend {
            self.frame += span.rate;
            if span.counter == span.length {
                self.frame = span.to_frame as f32;
            }
            self.int_frame = self.frame as i32;
        }
        self.count = 1;
        After::Sleep
    }

    fn last_frame(&self, clock: &dyn Clock, record: Option<i32>) -> Option<i32> {
        record.and_then(|r| clock.frames(r)).map(|n| (n - 1).max(0))
    }

    /// The opcodes, back to back, until one installs a span or the script
    /// ends.
    fn run(&mut self, clock: &dyn Clock) {
        loop {
            let Some(op) = self.program.ops.get(self.pc) else {
                self.done = true;
                return;
            };
            self.pc += 1;
            match op {
                Op::End => {
                    self.done = true;
                    return;
                }
                Op::Callback(id) => self.callbacks.push(*id),
                Op::Advance(n) => {
                    // `FUN_0019a658(n)`: a frame on at rate 1, asleep n.
                    self.frame += 1.0;
                    self.int_frame = self.frame as i32;
                    self.count = i32::from(*n);
                    return;
                }
                Op::Blend {
                    frames,
                    start,
                    selectors,
                    targets,
                } => {
                    let alternate = clock
                        .left_hand()
                        .then(|| selectors.iter().position(|&s| s == 0xf8))
                        .flatten()
                        .and_then(|k| targets.get(k + 1));
                    let target = alternate.or(targets.first());
                    let to_record = match target.map(|t| t.record) {
                        Some(CURRENT_RECORD) | None => self.record,
                        Some(r) => Some(r),
                    };
                    self.span = Some(Span {
                        kind: SpanKind::Blend,
                        length: i32::from(*frames),
                        counter: 0,
                        rate: 1.0 / f32::from(*frames).max(1.0),
                        to_frame: (*start).max(0),
                        to_record,
                        from: Some((self.record, self.frame)),
                    });
                    if let After::Sleep = self.advance(clock) {
                        return;
                    }
                }
                Op::Wait { to, ticks } => {
                    let last = self.last_frame(clock, self.record);
                    let mut to_frame = match *to {
                        WaitFrame::Absolute(b) => b,
                        WaitFrame::Relative(b) if self.int_frame < 0 => b,
                        WaitFrame::Relative(b) => self.int_frame + b,
                    };
                    if let Some(last) = last {
                        to_frame = to_frame.min(last);
                    }
                    let delta = if self.int_frame < 0 {
                        to_frame
                    } else {
                        to_frame - self.int_frame
                    };
                    let length = match *ticks {
                        WaitTicks::Fixed(n) => i32::from(n),
                        WaitTicks::Flight => match clock.flight() {
                            Some(flight) => (flight - self.tick).max(delta / 3),
                            None => delta / 2,
                        },
                        // `0x1973e8..0x197434`: the updates until the actor
                        // lands. A jump the sim does not report (the shots',
                        // which land at the script's traced end) counts as
                        // nothing, the rewrite's own.
                        WaitTicks::Airtime => clock.landing().map_or(1, |t| t - self.tick),
                        WaitTicks::Other => 1,
                    }
                    .max(1);
                    self.frame = self.int_frame.max(0) as f32;
                    self.span = Some(Span {
                        kind: SpanKind::Wait,
                        length,
                        counter: 0,
                        rate: delta as f32 / length as f32,
                        to_frame,
                        to_record: None,
                        from: None,
                    });
                    if let After::Sleep = self.advance(clock) {
                        return;
                    }
                }
                Op::PlayOut { per_frame } => {
                    let last = self
                        .last_frame(clock, self.record)
                        .unwrap_or(self.int_frame);
                    let from = self.int_frame.max(0);
                    let length = ((last - from) as f32 * per_frame).ceil().max(0.0) as i32;
                    self.frame = from as f32;
                    self.span = Some(Span {
                        kind: SpanKind::PlayOut,
                        length,
                        counter: 0,
                        rate: 1.0 / per_frame.max(1e-3),
                        to_frame: last.max(from),
                        to_record: None,
                        from: None,
                    });
                    if let After::Sleep = self.advance(clock) {
                        return;
                    }
                }
                Op::Rate { per_frame } => {
                    self.frame = self.int_frame.max(0) as f32;
                    self.span = Some(Span {
                        kind: SpanKind::Free,
                        length: -1,
                        counter: -1,
                        rate: 1.0 / per_frame.max(1e-3),
                        to_frame: -1,
                        to_record: None,
                        from: None,
                    });
                    self.advance(clock);
                    return;
                }
            }
        }
    }

    /// What the channel shows after the last `step`.
    pub fn pose(&self) -> ChannelPose {
        match &self.span {
            Some(span) if span.kind == SpanKind::Blend => ChannelPose {
                record: span.to_record,
                frame: span.to_frame as f32,
                blend: Some(BlendState {
                    from: span.from,
                    weight: span.counter as f32 / span.length.max(1) as f32,
                }),
                done: self.done,
            },
            _ => ChannelPose {
                record: self.record,
                frame: self.frame,
                blend: None,
                done: self.done,
            },
        }
    }
}

/// The channel's pose `tick` updates into the script (0: the update the
/// script started in), and the callbacks that update ran.
pub fn pose_at(program: &Program, tick: i32, clock: &dyn Clock) -> (ChannelPose, Vec<u32>) {
    let mut channel = Channel::new(program);
    for _ in 0..=tick.max(0) {
        channel.step(clock);
    }
    let callbacks = channel.callbacks().to_vec();
    (channel.pose(), callbacks)
}

/// The update on which `callback` runs along the program, if it does.
pub fn callback_tick(program: &Program, callback: u32, clock: &dyn Clock) -> Option<i32> {
    let mut channel = Channel::new(program);
    for _ in 0..1024 {
        channel.step(clock);
        if channel.callbacks().contains(&callback) {
            return Some(channel.tick());
        }
        if channel.pose().done {
            return None;
        }
    }
    None
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

    fn blend(frames: u8, start: u32, record: u32) -> Vec<u8> {
        let mut b = vec![0x8e, frames];
        b.extend(start.to_le_bytes());
        b.extend(record.to_le_bytes());
        b
    }

    fn callback(id: u32) -> Vec<u8> {
        let mut b = vec![0x83];
        b.extend(id.to_le_bytes());
        b.extend([0; 12]);
        b
    }

    /// `jumpshot_2hand_legkick_1`'s shape (`0x44e846`): a 6-frame blend to
    /// frame 0, frame 5 over 8, (the check) frame 10 over 16, frame 12
    /// over 4, callback 21, frame 14 over 4, the airtime to 21, 23 over
    /// 3, 25 over 4, the play-out at a frame each 2 updates, the end.
    fn legkick() -> Vec<u8> {
        let mut body = blend(6, 0, 360);
        body.extend([0x8d, 0xee, 5, 8]);
        body.extend([0x8d, 0xee, 10, 16]);
        body.extend([0x8d, 0xee, 12, 4]);
        body.extend(callback(21));
        body.extend([0x8d, 0xee, 14, 4]);
        body.extend([0x8d, 0xee, 21, 0xfe]);
        body.extend([0x8d, 0xee, 23, 3]);
        body.extend([0x8d, 0xee, 25, 4]);
        body.push(0x8c);
        body.extend(2.0f32.to_le_bytes());
        body.push(0x81);
        body
    }

    #[test]
    fn the_release_lands_on_the_walked_update_with_the_frame_the_waits_reach() {
        let data = executable(&legkick());
        let program = compile(&data, 0x1000, Path::Plain).unwrap();
        let clock = FixedClock {
            frames: Some(30),
            ..Default::default()
        };
        assert_eq!(callback_tick(&program, 21, &clock), Some(34));
        // The press update: the blend's first step, the incoming clip at
        // frame 0 weighted a sixth over the actor's own clip.
        let (p, _) = pose_at(&program, 0, &clock);
        assert_eq!(p.record, Some(360));
        assert_eq!(
            p.blend,
            Some(BlendState {
                from: Some((None, 0.0)),
                weight: 1.0 / 6.0
            })
        );
        let (p, _) = pose_at(&program, 5, &clock);
        assert_eq!(p.blend.map(|b| b.weight), Some(1.0));
        // Update 6: the clip is current at frame 0 and the first wait's
        // step has moved it 5/8 of a frame.
        let (p, _) = pose_at(&program, 6, &clock);
        assert_eq!(p.record, Some(360));
        assert_eq!(p.blend, None);
        assert!((p.frame - 5.0 / 8.0).abs() < 1e-5, "{}", p.frame);
        // Update 13: the wait's last step lands exactly on 5.
        let (p, _) = pose_at(&program, 13, &clock);
        assert_eq!(p.frame, 5.0);
        // The release update: callback 21 at frame 12, then the next
        // wait's first step shows 12.5.
        let (p, calls) = pose_at(&program, 34, &clock);
        assert_eq!(calls, [21]);
        assert_eq!(p.frame, 12.5);
        // The airtime is one update in the rewrite, so 21 shows at 38
        // and the play-out (4 frames at 2 updates each) runs 46..53.
        let (p, _) = pose_at(&program, 38, &clock);
        assert_eq!(p.frame, 21.0);
        let (p, _) = pose_at(&program, 45, &clock);
        assert_eq!(p.frame, 25.0);
        let (p, _) = pose_at(&program, 46, &clock);
        assert_eq!(p.frame, 25.5);
        let (p, _) = pose_at(&program, 53, &clock);
        assert_eq!(p.frame, 29.0);
        assert!(!p.done);
        let (p, _) = pose_at(&program, 54, &clock);
        assert!(p.done);
    }

    #[test]
    fn the_fake_path_falls_through_the_check_and_the_flight_wait_reads_the_pass() {
        // A check: `85 20 ... -> release` with a fake tail after it.
        let mut body = blend(6, 0, 360);
        let release = 0x1000 + body.len() as u32 + 21 + 3 + 1;
        body.push(0x85);
        body.extend(20u32.to_le_bytes());
        body.extend([0; 12]);
        body.extend(release.to_le_bytes());
        body.extend([0x8d, 9, 9]);
        body.push(0x81);
        body.extend(callback(21));
        body.push(0x81);
        let data = executable(&body);
        let plain = compile(&data, 0x1000, Path::Plain).unwrap();
        let fake = compile(&data, 0x1000, Path::Fake).unwrap();
        let clock = FixedClock {
            frames: Some(40),
            ..Default::default()
        };
        assert_eq!(callback_tick(&plain, 21, &clock), Some(6));
        assert_eq!(callback_tick(&fake, 21, &clock), None);
        let (p, _) = pose_at(&fake, 14, &clock);
        assert_eq!(p.frame, 9.0);
        assert!(!p.done);
        assert!(pose_at(&fake, 15, &clock).0.done);
        // A catch: blend 6 to frame 4, frame 10 over the flight's rest,
        // then the play-out at 1.5 (`catch_stand_1`).
        let mut body = blend(6, 4, 565);
        body.extend([0x8d, 0xee, 10, 0xfd]);
        body.push(0x8c);
        body.extend(1.5f32.to_le_bytes());
        body.push(0x81);
        let data = executable(&body);
        let catch = compile(&data, 0x1000, Path::Plain).unwrap();
        let clock = FixedClock {
            frames: Some(17),
            flight: Some(30),
            left_hand: false,
        };
        // Update 6: the wait over the 24 updates left; update 29 lands on
        // 10; the play-out's 9 updates end at 38.
        let (p, _) = pose_at(&catch, 6, &clock);
        assert!((p.frame - (4.0 + 6.0 / 24.0)).abs() < 1e-5);
        let (p, _) = pose_at(&catch, 29, &clock);
        assert_eq!(p.frame, 10.0);
        let (p, _) = pose_at(&catch, 38, &clock);
        assert_eq!(p.frame, 16.0);
        assert!(!p.done);
        assert!(pose_at(&catch, 39, &clock).0.done);
    }
}
