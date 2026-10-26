//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
//! The trick moves' and the defenders' reactions' scripts and tables
//! ([`street_sim::trick`]) read again from the executable and the disc's
//! animations.
use street_assets::{
    iso::Iso,
    playback::{self, Clock, Path},
    pose::PoseData,
    scripts::{executable_constant, executable_words},
};
use street_sim::trick::{self, Run, Step};

struct Disc {
    elf: Vec<u8>,
    data: PoseData,
    /// The channel's record and rate a script's `8e`/`8c` read when the
    /// code that starts it wrote them (record -2000, start -2001, a
    /// negative rate): the celebrations' ([`Disc::steps_preset`]).
    preset: std::cell::Cell<Option<(u32, f32)>>,
}

impl Disc {
    fn load() -> Option<Self> {
        let path = std::env::var_os("STREET_ISO")?;
        let mut iso = Iso::open(path).unwrap();
        let elf = iso.read("/SLUS_206.51").unwrap();
        let data = PoseData::read(&mut iso).unwrap();
        Some(Self {
            elf,
            data,
            preset: std::cell::Cell::new(None),
        })
    }

    fn word(&self, address: u32) -> u32 {
        executable_words(&self.elf, address, 1).unwrap()[0]
    }

    fn byte(&self, address: u32) -> u8 {
        (self.word(address & !3) >> ((address & 3) * 8)) as u8
    }

    fn signed(&self, address: u32) -> i32 {
        i32::from(self.byte(address) as i8)
    }

    fn unaligned(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|k| self.byte(address + k as u32)))
    }

    /// Frames in `record`'s clip.
    fn frames(&self, record: u32) -> Option<i32> {
        let (clip, _) = (*self.data.records.get(record as usize)?)?;
        Some(self.data.clips[usize::from(clip)].rotations.len() as i32)
    }

    /// `record`'s root at `frame` (at scale 1, a mirrored record's x
    /// negated), interpolated as `Poses::root` does.
    fn root(&self, record: u16, frame: f32) -> [f32; 3] {
        let (clip, mirrored) = self.data.records[usize::from(record)].unwrap();
        let root = &self.data.clips[usize::from(clip)].root;
        if root.is_empty() {
            return [0.0; 3];
        }
        let n = root.len();
        let t = frame.clamp(0.0, (n - 1) as f32);
        let f0 = (t as usize).min(n - 1);
        let f1 = (f0 + 1).min(n - 1);
        let u = t - f0 as f32;
        let mut p: [f32; 3] =
            std::array::from_fn(|k| root[f0][k] + (root[f1][k] - root[f0][k]) * u);
        if mirrored {
            p[0] = -p[0];
        }
        p
    }

    /// The script at `address` along its plain path (`82` and `85` jump,
    /// `84` and the animation tests fall through), the clock's steps with
    /// the play-outs' updates from the clips' frames and the waits' frames
    /// kept within their clips.
    fn steps(&self, address: u32) -> Vec<Step> {
        self.steps_on(address, Paths::default())
    }

    /// A stand script of `FUN_00163d40`'s stand path ([`street_sim::stand`])
    /// as steps: its loops and the jumps between scripts kept
    /// ([`Step::Jump`], [`Step::Roll`] for callback 3's `85`), `ab` as
    /// [`Step::Loop`], the planted feet (`95`) and their release (`96`),
    /// `98`'s choice between two plantings, `91` as [`Step::Flags`] (no
    /// stand sets 0x1000, so every stand's root carries its player);
    /// callback 11's `84` and `ae` change nothing a stand plays and are
    /// passed.
    fn stand_steps(&self, entry: u32) -> Vec<Step> {
        let mut out: Vec<Step> = Vec::new();
        let mut at = std::collections::BTreeMap::new();
        let mut fixups: Vec<(usize, u32)> = Vec::new();
        let mut pending = vec![entry];
        while let Some(start) = pending.pop() {
            if at.contains_key(&start) {
                continue;
            }
            let (mut pc, mut record, mut frame) = (start, 0u32, -1i32);
            loop {
                if at.contains_key(&pc) {
                    out.push(Step::Jump(0));
                    fixups.push((out.len() - 1, pc));
                    break;
                }
                at.insert(pc, out.len());
                let op = self.byte(pc);
                let p = pc + 1;
                pc = match op {
                    0xa7 => p,
                    0x96 => {
                        out.push(Step::Unplant);
                        p
                    }
                    0x91 => {
                        out.push(Step::Flags(self.unaligned(p)));
                        p + 4
                    }
                    0xae => p + 4,
                    0x93..=0x95 => {
                        let mut n = 0;
                        let keep = self.byte(p) == 0xf1;
                        if keep {
                            n += 1;
                        }
                        let mut pick = 0;
                        if matches!(self.byte(p + n), 0xec | 0xf2 | 0xf3 | 0xf4 | 0xf7 | 0xf8) {
                            pick = self.byte(p + n);
                            n += 1;
                        }
                        out.push(Step::Plant(street_sim::jump::Plant {
                            hands: op == 0x93,
                            keep,
                            pick,
                            bones: [self.byte(p + n), self.byte(p + n + 1)],
                        }));
                        p + n + 2
                    }
                    // `98 r.. -> T`: a jump on the record the channel plays.
                    0x98 => {
                        let mut n = 0;
                        let mut records = [0u16; 12];
                        let mut k = 0;
                        while self.unaligned(p + n) < 1144 {
                            let v = self.unaligned(p + n);
                            if v != 0 {
                                assert!(k < records.len(), "{pc:#x}: more records tested");
                                records[k] = v as u16;
                                k += 1;
                            }
                            n += 4;
                        }
                        let target = self.unaligned(p + n);
                        out.push(Step::OnRecord { records, to: 0 });
                        fixups.push((out.len() - 1, target));
                        pending.push(target);
                        p + n + 4
                    }
                    0x84 if self.unaligned(p) == 22 => {
                        let target = self.unaligned(p + 16);
                        out.push(Step::RightHand(0));
                        fixups.push((out.len() - 1, target));
                        pending.push(target);
                        p + 20
                    }
                    0x84 if matches!(self.unaligned(p), 10 | 45 | 49) => {
                        let target = self.unaligned(p + 16);
                        out.push(Step::Asked(0));
                        fixups.push((out.len() - 1, target));
                        pending.push(target);
                        p + 20
                    }
                    0x84 => {
                        assert_eq!(self.unaligned(p), 11, "{pc:#x}: the test");
                        p + 20
                    }
                    0x85 => {
                        assert_eq!(self.unaligned(p), 3, "{pc:#x}: the roll");
                        let target = self.unaligned(p + 16);
                        out.push(Step::Roll {
                            n: self.unaligned(p + 4) as u8,
                            to: 0,
                        });
                        fixups.push((out.len() - 1, target));
                        pending.push(target);
                        p + 20
                    }
                    0x83 if self.unaligned(p) == 4 => {
                        out.push(Step::Want(self.unaligned(p + 4) as i32 as i16));
                        p + 16
                    }
                    0xab => {
                        let frames = self.frames(record).unwrap();
                        out.push(Step::Loop {
                            per_frame: f32::from_bits(self.unaligned(p)),
                            frames: frames as u8,
                        });
                        p + 4
                    }
                    0x82 => {
                        let target = self.unaligned(p);
                        out.push(Step::Jump(0));
                        fixups.push((out.len() - 1, target));
                        pending.push(target);
                        break;
                    }
                    _ => {
                        // The held reading for `8e`'s selectors (`f2`:
                        // a left-hander's alternate).
                        let held = Paths {
                            held: true,
                            fake: false,
                            jumps: false,
                        };
                        match self.step_at(pc, held, &mut record, &mut frame, &mut out) {
                            Some(next) => next,
                            None => break,
                        }
                    }
                };
            }
        }
        for (k, target) in fixups {
            let to = at[&target] as u8;
            match &mut out[k] {
                Step::Jump(t)
                | Step::Roll { to: t, .. }
                | Step::RightHand(t)
                | Step::Asked(t)
                | Step::OnRecord { to: t, .. } => *t = to,
                _ => unreachable!(),
            }
        }
        out
    }

    /// [`Disc::steps`] for a script whose channel the starting code set up:
    /// `record` at frame 0 played at a frame each `rate` updates
    /// (`FUN_00170e70`: channel+0x28, +0x24 and +0x3c).
    fn steps_preset(&self, address: u32, record: u32, rate: f32) -> Vec<Step> {
        self.preset.set(Some((record, rate)));
        let steps = self.steps(address);
        self.preset.set(None);
        steps
    }

    /// [`Disc::steps`] with the held scripts' readings ([`Paths`]).
    fn steps_on(&self, address: u32, paths: Paths) -> Vec<Step> {
        let mut out = Vec::new();
        let (mut pc, mut record, mut frame) = (address, 0u32, -1i32);
        for _ in 0..300 {
            if paths.jumps && self.byte(pc) == 0x98 {
                pc = self.either(pc, paths, &mut record, &mut frame, &mut out);
                continue;
            }
            if paths.jumps && matches!(self.byte(pc), 0x84 | 0x85) && self.unaligned(pc + 1) == 28 {
                match self.hang(pc, paths, &mut record, &mut frame, &mut out) {
                    Some(next) => pc = next,
                    None => return out,
                }
                continue;
            }
            match self.step_at(pc, paths, &mut record, &mut frame, &mut out) {
                Some(next) => pc = next,
                None => return out,
            }
        }
        panic!("{address:#x}: no end");
    }

    /// Callback 28's `84`/`85 28 -> T` (`FUN_00172400`: the dunker's hang
    /// on the rim): a [`Step::Hang`] over the hang's path and the other's
    /// (`85` falls through into the hang, `84` jumps to it), each to its
    /// `End` or to where they meet, which goes on from the returned pc.
    fn hang(
        &self,
        pc: u32,
        paths: Paths,
        record: &mut u32,
        frame: &mut i32,
        out: &mut Vec<Step>,
    ) -> Option<u32> {
        let p = pc + 1;
        let target = self.unaligned(p + 16);
        let (mut rh, mut fh) = (*record, *frame);
        let (mut ro, mut fo) = (*record, *frame);
        let (mut hang, mut other) = (Vec::new(), Vec::new());
        let next = if self.byte(pc) == 0x85 {
            self.until(p + 20, Some(target), paths, &mut rh, &mut fh, &mut hang);
            Some(target)
        } else {
            let join = self.until(p + 20, None, paths, &mut ro, &mut fo, &mut other);
            let met = self.until(target, join, paths, &mut rh, &mut fh, &mut hang);
            assert!(join.is_some(), "{pc:#x}: the other path ends");
            assert!(met.is_none() || met == join, "{pc:#x}: the paths part");
            join
        };
        out.push(Step::Hang {
            hang: hang.len() as u8,
            other: other.len() as u8,
        });
        out.extend(hang);
        out.extend(other);
        *record = ro;
        *frame = fo;
        next
    }

    /// The steps from `pc` up to an `82 J` (J returned, not followed),
    /// `join` itself, or an `End` (kept; none returned); `98` pairs as
    /// [`Step::Either`]s.
    fn until(
        &self,
        mut pc: u32,
        join: Option<u32>,
        paths: Paths,
        record: &mut u32,
        frame: &mut i32,
        out: &mut Vec<Step>,
    ) -> Option<u32> {
        for _ in 0..80 {
            if Some(pc) == join {
                return Some(pc);
            }
            if self.byte(pc) == 0x82 {
                return Some(self.unaligned(pc + 1));
            }
            if self.byte(pc) == 0x98 {
                pc = self.either(pc, paths, record, frame, out);
                continue;
            }
            pc = self.step_at(pc, paths, record, frame, out)?;
        }
        panic!("{pc:#x}: a hang path without an end");
    }

    /// A `98` record test over its pair of branches (`X..; 82 L; T: X'..;
    /// L:`): a [`Step::Either`] and both branches.
    fn either(
        &self,
        pc: u32,
        paths: Paths,
        record: &mut u32,
        frame: &mut i32,
        out: &mut Vec<Step>,
    ) -> u32 {
        let p = pc + 1;
        let mut n = 0;
        let mut named = Vec::new();
        let target = loop {
            let v = self.unaligned(p + n);
            n += 4;
            if v >= 1144 {
                break v;
            }
            if v != 0 {
                named.push(v);
            }
        };
        assert_eq!(named.len(), 1, "{pc:#x}: the records tested");
        let (mut ra, mut fa) = (*record, *frame);
        let mut plain = Vec::new();
        let mut q = p + n;
        let join = loop {
            if self.byte(q) == 0x82 {
                break self.unaligned(q + 1);
            }
            q = self
                .step_at(q, paths, &mut ra, &mut fa, &mut plain)
                .expect("a branch ends");
        };
        let (mut rb, mut fb) = (*record, *frame);
        let mut alternate = Vec::new();
        let mut q = target;
        while q != join {
            q = self
                .step_at(q, paths, &mut rb, &mut fb, &mut alternate)
                .expect("a branch ends");
        }
        // Branches holding only what the steps leave out (the glass aim
        // `a9`) are no branch.
        if !(plain.is_empty() && alternate.is_empty()) {
            out.push(Step::Either {
                record: named[0] as u16,
                plain: plain.len() as u8,
                alternate: alternate.len() as u8,
            });
        }
        out.extend(plain);
        out.extend(alternate);
        *record = ra;
        *frame = fa;
        join
    }

    /// The jumps' own opcodes as the held tables carry them
    /// ([`street_sim::jump`]), or none for another.
    fn jump_op(&self, pc: u32) -> Option<(Vec<Step>, u32)> {
        let p = pc + 1;
        let float = |a: u32| f32::from_bits(self.unaligned(a));
        Some(match self.byte(pc) {
            0x91 => (vec![Step::Flags(self.unaligned(p))], p + 4),
            0x9f => (vec![Step::Rise(float(p))], p + 4),
            0xaa => (vec![Step::Carry(float(p))], p + 4),
            0x96 => (vec![Step::Unplant], p),
            0x83 if self.unaligned(p) == 4 => {
                (vec![Step::Want(self.unaligned(p + 4) as i16)], p + 16)
            }
            0xa1 if self.byte(p) == 0xf0 => (
                vec![Step::Toward {
                    turn: float(p + 1) as i16,
                    speed: float(p + 5),
                    rise: float(p + 9),
                }],
                p + 13,
            ),
            // The dive's hop (`FUN_00194d58`'s 0x85 branch).
            0xa1 if self.byte(p) == 0x85 => (
                vec![Step::Dive {
                    speed: float(p + 5),
                    rise: float(p + 9),
                }],
                p + 13,
            ),
            0xa0 => (vec![Step::Slide(float(p))], p + 4),
            0x93..=0x95 => {
                let mut n = 0;
                let keep = self.byte(p) == 0xf1;
                if keep {
                    n += 1;
                }
                let mut pick = 0;
                if matches!(self.byte(p + n), 0xec | 0xf2 | 0xf3 | 0xf4 | 0xf7 | 0xf8) {
                    pick = self.byte(p + n);
                    n += 1;
                }
                (
                    vec![Step::Plant(street_sim::jump::Plant {
                        hands: self.byte(pc) == 0x93,
                        keep,
                        pick,
                        bones: [self.byte(p + n), self.byte(p + n + 1)],
                    })],
                    p + n + 2,
                )
            }
            0xac => (
                vec![Step::Reach(street_sim::jump::Reach {
                    cap: self.byte(p) == 1,
                    updates: self.unaligned(p + 1) as u8,
                    frame: self.unaligned(p + 5) as u8,
                    ahead: float(p + 9),
                })],
                p + 13,
            ),
            0x9e => {
                // `FUN_001988b0`'s operands: 0x85/0x86 a one-hand pick, else
                // 0x83 the second bones and 0x84 the midpoint; a selector of
                // 0x80 or more the channel's record and the direction byte,
                // below it the direction itself and two records.
                let (mut second, mut mid, mut side) = (false, false, 0i8);
                let mut n = 0;
                let mut sel = self.byte(p);
                n += 1;
                match sel {
                    0x85 => side = 1,
                    0x86 => side = -1,
                    _ => {
                        if sel == 0x83 {
                            second = true;
                            sel = self.byte(p + n);
                            n += 1;
                        }
                        if sel == 0x84 {
                            mid = true;
                            sel = self.byte(p + n);
                            n += 1;
                        }
                    }
                }
                let (direction, updates, frame, records, next) = if sel >= 0x80 {
                    (
                        self.byte(p + n),
                        self.unaligned(p + n + 1),
                        self.unaligned(p + n + 5),
                        [trick::CURRENT; 2],
                        p + n + 9,
                    )
                } else {
                    (
                        sel,
                        self.unaligned(p + n),
                        self.unaligned(p + n + 4),
                        [
                            self.unaligned(p + n + 8) as u16,
                            self.unaligned(p + n + 12) as u16,
                        ],
                        p + n + 16,
                    )
                };
                (
                    vec![Step::Aim(street_sim::jump::Aim {
                        second,
                        mid,
                        side,
                        direction,
                        updates: updates as u8,
                        frame: frame as u8,
                        records,
                    })],
                    next,
                )
            }
            _ => return None,
        })
    }

    /// One instruction at `pc` into `out`: the next pc (`82` and `85`
    /// followed), or none at the end.
    fn step_at(
        &self,
        pc: u32,
        paths: Paths,
        record: &mut u32,
        frame: &mut i32,
        out: &mut Vec<Step>,
    ) -> Option<u32> {
        let op = self.byte(pc);
        let p = pc + 1;
        if paths.jumps
            && let Some((steps, next)) = self.jump_op(pc)
        {
            out.extend(steps);
            return Some(next);
        }
        let next = match op {
            0x00 | 0x80 | 0x88 | 0x8f | 0x90 | 0x96 | 0xa7 => p,
            // `a0` (`FUN_001957d8`): the slide's float.
            0xa0 => p + 4,
            0x81 => {
                out.push(Step::End);
                return None;
            }
            0x82 => self.unaligned(p),
            0x83 => {
                out.push(Step::Call(self.unaligned(p) as u8));
                // Callback 1 hands the channel to the locomotion
                // (`FUN_001722c0`): a plain script ends there whether or
                // not an `81` follows.
                if self.unaligned(p) == 1 && !paths.held {
                    out.push(Step::End);
                    return None;
                }
                p + 16
            }
            0x84 => {
                // `84` jumps when its callback answers: the heave's fake
                // (`84 14`) leaves for its own path.
                out.push(Step::Test(self.unaligned(p) as u8));
                if paths.fake && self.unaligned(p) == 20 {
                    self.unaligned(p + 16)
                } else {
                    p + 20
                }
            }
            0x85 => {
                out.push(Step::Test(self.unaligned(p) as u8));
                if paths.fake && self.unaligned(p) == 20 {
                    p + 20
                } else {
                    self.unaligned(p + 16)
                }
            }
            0x9b | 0x9c => {
                let w = self.unaligned(p);
                out.push(if op == 0x9b {
                    Step::Bits(w)
                } else {
                    Step::Clear(w)
                });
                p + 4
            }
            0x86 | 0x91 | 0xa2 | 0xa4 | 0xaa | 0xae => p + 4,
            0x9f
            | 0x8b
            | 0xa3
            | 0x92
            | 0x9d
            | 0xad
            | 0xa8
            | 0xac
            | 0xa9
            | 0x93..=0x95
            | 0x98
            | 0x99
            | 0x9e
                if paths.held =>
            {
                match op {
                    0x9f => p + 4,
                    0x8b | 0xa3 => p + 1,
                    0x92 | 0x9d | 0xad => p + 3,
                    0xa8 => p + 9,
                    0xac => p + 13,
                    0xa9 => {
                        p + match self.byte(p) {
                            0xef => 10,
                            0xec => 18,
                            _ => 17,
                        }
                    }
                    0x93..=0x95 => {
                        let mut n = 0;
                        if self.byte(p) == 0xf1 {
                            n += 1;
                        }
                        if matches!(self.byte(p + n), 0xec | 0xf2 | 0xf3 | 0xf4 | 0xf7 | 0xf8) {
                            n += 1;
                        }
                        p + n + 2
                    }
                    0x98 | 0x99 => {
                        let mut n = 0;
                        while self.unaligned(p + n) < 1144 {
                            n += 4;
                        }
                        p + n + 4
                    }
                    _ => {
                        // `9e`: its selectors, then 9 or 16 bytes.
                        let mut sel = self.byte(p);
                        let mut n = 1;
                        for prefix in [0x83, 0x84] {
                            if sel == prefix {
                                sel = self.byte(p + n);
                                n += 1;
                            }
                        }
                        p + n + if sel >= 128 { 9 } else { 16 }
                    }
                }
            }
            0x92 | 0x9d | 0xad => p + 3,
            0xa6 => {
                if self.byte(p) == 3 {
                    out.push(Step::Window {
                        row: self.byte(p + 1),
                        n: self.byte(p + 2),
                    });
                } else if self.byte(p) == 2 {
                    out.push(Step::Shove {
                        row: self.byte(p + 1),
                        n: self.byte(p + 2),
                    });
                } else if self.byte(p) == 1 && paths.held {
                    out.push(Step::Knock {
                        row: self.byte(p + 1),
                        n: self.byte(p + 2),
                    });
                } else {
                    assert!(paths.held, "{pc:#x}: contact kind");
                }
                p + 3
            }
            0x97 => {
                out.push(Step::Stop);
                p
            }
            0xa1 => {
                if self.byte(p) == 0x81 {
                    out.push(Step::Hop {
                        turn: f32::from_bits(self.unaligned(p + 1)) as i16,
                        speed: f32::from_bits(self.unaligned(p + 5)),
                        rise: f32::from_bits(self.unaligned(p + 9)),
                    });
                } else {
                    assert!(paths.held, "{pc:#x}: the hop");
                }
                p + 13
            }
            0x8c => {
                let per_frame = match (f32::from_bits(self.unaligned(p)), self.preset.get()) {
                    (f, Some((_, rate))) if f < 0.0 => rate,
                    (f, _) => f,
                };
                let last = self.frames(*record).unwrap() - 1;
                let n = ((last - (*frame).max(0)) as f32 * per_frame)
                    .ceil()
                    .max(0.0);
                out.push(Step::PlayOut {
                    per_frame,
                    to: last as u8,
                    n: n as u16,
                });
                *frame = last;
                p + 4
            }
            0x8d => {
                let last = self.frames(*record).unwrap() - 1;
                let (step, next, to) = if self.byte(p) == 0xee {
                    let to = self.signed(p + 1).min(last);
                    match self.byte(p + 2) {
                        0xef => (Step::Flight { to: to as u8 }, p + 4, to),
                        // The rest of the pass's flight (the catch rows).
                        0xfd => (Step::Flight { to: to as u8 }, p + 3, to),
                        0xfe => (Step::Land { to: to as u8 }, p + 3, to),
                        n => (Step::Wait { to: to as u8, n }, p + 3, to),
                    }
                } else {
                    let to = (*frame + self.signed(p)).min(last);
                    (
                        Step::Wait {
                            to: to as u8,
                            n: self.byte(p + 1),
                        },
                        p + 2,
                        to,
                    )
                };
                out.push(step);
                *frame = to;
                next
            }
            0x8e => {
                let mut n = 0;
                let mut selectors = Vec::new();
                for s in [0xf2, 0xf8, 0xec, 0xf5, 0xf4, 0xf3] {
                    if self.byte(p + n) == s {
                        selectors.push(s);
                        n += 1;
                    }
                }
                let frames = self.byte(p + n);
                let start = self.unaligned(p + n + 1) as i32;
                let primary = self.unaligned(p + n + 5);
                let alternates: Vec<u32> = (0..selectors.len())
                    .map(|k| self.unaligned(p + n + 9 + 4 * k as u32))
                    .collect();
                let preset = |r: u32| match self.preset.get() {
                    Some((record, _)) if r == 0xffff_f830 => record,
                    _ => r,
                };
                let (primary, alternates): (u32, Vec<u32>) = (
                    preset(primary),
                    alternates.into_iter().map(preset).collect(),
                );
                let current = |r: u32| {
                    if r == 0xffff_fc18 && paths.held {
                        trick::CURRENT
                    } else {
                        r as u16
                    }
                };
                let picked = selectors
                    .iter()
                    .position(|&s| matches!(s, 0xf2 | 0xec))
                    .filter(|_| paths.held);
                if let Some(k) = picked {
                    out.push(Step::Pick {
                        n: frames,
                        start: start.max(0) as u8,
                        records: [current(primary), current(alternates[k])],
                        by: if selectors[k] == 0xf2 {
                            trick::Select::Handed
                        } else {
                            trick::Select::Side
                        },
                    });
                } else {
                    let left = selectors
                        .iter()
                        .position(|&s| s == 0xf8)
                        .map_or(primary, |k| alternates[k]);
                    out.push(Step::Blend {
                        n: frames,
                        start: start.max(0) as u8,
                        records: [current(primary), current(left)],
                    });
                }
                if primary != 0xffff_fc18 {
                    *record = primary;
                }
                *frame = start.max(0);
                p + n + 9 + 4 * selectors.len() as u32
            }
            op => panic!("{pc:#x}: opcode {op:#x}"),
        };
        Some(next)
    }

    /// A script's root change from its first blend's frame to its last
    /// wait's (each clip's own, summed, for a script of two clips).
    fn net(&self, steps: &[Step], left: bool) -> [f32; 2] {
        let mut total = [0.0f32; 2];
        let mut clip: Option<(u16, f32, f32)> = None;
        let close = |c: Option<(u16, f32, f32)>, total: &mut [f32; 2]| {
            if let Some((record, from, to)) = c {
                let (a, b) = (self.root(record, from), self.root(record, to));
                total[0] += b[0] - a[0];
                total[1] += b[2] - a[2];
            }
        };
        for s in steps {
            match *s {
                Step::Blend { start, records, .. } => {
                    close(clip, &mut total);
                    let f = f32::from(start);
                    clip = Some((records[usize::from(left)], f, f));
                }
                Step::Wait { to, .. }
                | Step::PlayOut { to, .. }
                | Step::Land { to }
                | Step::Flight { to } => {
                    if let Some(c) = &mut clip {
                        c.2 = f32::from(to);
                    }
                }
                _ => {}
            }
        }
        close(clip, &mut total);
        total
    }
}

/// How [`Disc::steps_on`] reads a script: the held scripts' (the shots,
/// dunks, lay-ups and passes, [`street_sim::held`]) skip the jumps', the
/// planted foot's and the animation tests' opcodes, keep `8e f2` and `8e
/// ec` as picks and `-1000` as the current record; the pump fake's path
/// falls through callback 20's `85`.
#[derive(Clone, Copy, Default)]
struct Paths {
    held: bool,
    fake: bool,
    /// The jumps' own opcodes as steps and the `98` pairs as
    /// [`Step::Either`]s (the shots', dunks', lay-ups' and passes' tables).
    jumps: bool,
}

/// The clock the display's channel gets: the disc's frames, the hop's
/// landing, the live hand.
struct DiscClock<'a> {
    disc: &'a Disc,
    landing: Option<i32>,
    left: bool,
}

impl Clock for DiscClock<'_> {
    fn frames(&self, record: i32) -> Option<i32> {
        self.disc.frames(record as u32)
    }
    fn flight(&self) -> Option<i32> {
        None
    }
    fn left_hand(&self) -> bool {
        self.left
    }
    fn landing(&self) -> Option<i32> {
        self.landing
    }
}

/// The sim's clock against the display's (`playback`, the VM's model the
/// shots, passes and catches were checked with) update by update: the
/// record, the frame, the blend and the end. Up to the self-toss's flight
/// (whose length the display does not know).
fn same_clock(disc: &Disc, address: u32, steps: &[Step], left: bool) {
    let flight = steps.iter().any(|s| matches!(s, Step::Flight { .. }));
    let mut run = Run::new(left, 40);
    let program = playback::compile(&disc.elf, address, Path::Plain).unwrap();
    let mut channel = playback::Channel::new(&program);
    for _ in 0..512 {
        run.step(steps, 0.0);
        let clock = DiscClock {
            disc,
            landing: (run.landing >= 0).then_some(i32::from(run.landing)),
            left,
        };
        channel.step(&clock);
        let (ours, theirs) = (run.shown(), channel.pose());
        if flight && channel.callbacks().contains(&53) {
            return;
        }
        assert_eq!(run.done, theirs.done, "{address:#x} tick {}", run.tick);
        if run.done {
            return;
        }
        assert_eq!(
            ours.record.map(i32::from),
            theirs.record,
            "{address:#x} tick {}",
            run.tick
        );
        assert_eq!(ours.blend.is_some(), theirs.blend.is_some(), "{address:#x}");
        if let (Some(a), Some(b)) = (ours.blend, theirs.blend) {
            assert!(
                (a - b.weight).abs() < 1e-5,
                "{address:#x} tick {}",
                run.tick
            );
        }
        assert!(
            (ours.frame - theirs.frame).abs() < 1e-4,
            "{address:#x} tick {}: {} {}",
            run.tick,
            ours.frame,
            theirs.frame
        );
    }
    panic!("{address:#x}: no end");
}

#[test]
fn the_catch_rows_scripts_are_the_executables() {
    // `0x3d75c8`'s rows (+0 the script, +0xc the record, +0x10 the catch
    // frame): each script along its plain path is `touch::CATCH_SCRIPTS`'
    // row, and its flight wait (`8d ee F fd`) ends on the row's frame.
    use street_sim::timing::CATCH_ROWS;
    use street_sim::touch::CATCH_SCRIPTS;
    let Some(disc) = Disc::load() else {
        return;
    };
    assert_eq!(CATCH_SCRIPTS.len(), CATCH_ROWS.len());
    for (k, row) in CATCH_ROWS.iter().enumerate() {
        assert_eq!(
            disc.steps(row.script),
            CATCH_SCRIPTS[k],
            "row {k} {}",
            row.name
        );
        let to = CATCH_SCRIPTS[k].iter().find_map(|s| match *s {
            Step::Flight { to } => Some(to),
            _ => None,
        });
        assert_eq!(to, Some(row.frame), "row {k} {}", row.name);
        assert!(
            CATCH_SCRIPTS[k].iter().any(|s| matches!(
                s,
                Step::Blend { records, .. } if records[0] == row.record
            )),
            "row {k} {}",
            row.name
        );
    }
}

#[test]
fn the_trick_scripts_are_the_executables() {
    let Some(disc) = Disc::load() else {
        return;
    };
    for (id, s) in trick::SCRIPTS.iter().enumerate().take(75) {
        let row = 0x3d7238 + 12 * id as u32;
        assert_eq!(s.address, disc.word(row), "trick {id}");
        assert_eq!(u32::from(s.flags), disc.word(row + 8), "trick {id} flags");
        assert_eq!(disc.steps(s.address), s.steps, "trick {id}");
        for left in [false, true] {
            let net = disc.net(s.steps, left);
            let table = s.root[usize::from(left)];
            assert!(
                (net[0] - table[0]).abs() < 1e-4 && (net[1] - table[1]).abs() < 1e-4,
                "trick {id} root {net:?} {table:?}"
            );
            if id != 74 {
                same_clock(&disc, s.address, s.steps, left);
            }
        }
    }
}

#[test]
fn the_celebrations_are_the_executables() {
    // The boot's constructor fills `0x3d9078` and `0x3d9138` in order, each
    // row an immediate before its `jal 0x30fa50` (the frame rate the rates
    // scale by); `FUN_00170e70` draws `r % 17` (`0x170ea4`) and starts
    // `0x44cbbf` (`0x170f80`), `FUN_00171160` starts `0x44cc03`
    // (`0x171464`, `0x171624`); the hold's celebration waits fps × 2
    // updates (`DAT_004b79f4`, boot-written: the constructor's).
    let Some(disc) = Disc::load() else {
        return;
    };
    let call = 0x0c00_0000 | (0x30fa50 >> 2);
    let mut rows = Vec::new();
    for at in (0x17bc00..0x17c504).step_by(4) {
        if disc.word(at) != call {
            continue;
        }
        // `sw rt, 0(sp)` (in the delay slot or before), then rt's
        // `addiu rt, zero, row` before that.
        let Some(store) = (0..=6)
            .map(|k| at + 4 - 4 * k)
            .find(|&a| disc.word(a) & 0xffe0_ffff == 0xafa0_0000)
        else {
            continue;
        };
        let rt = (disc.word(store) >> 16) & 31;
        if let Some(w) = (1..=6)
            .map(|k| disc.word(store - 4 * k))
            .find(|w| w >> 16 == 0x2400 | rt)
        {
            rows.push((w & 0xffff) as u16);
        }
    }
    let celebrate: Vec<u16> = trick::CELEBRATE_ROWS.iter().map(|r| r.0).collect();
    let deject: Vec<u16> = trick::DEJECT_ROWS.iter().map(|r| r.0).collect();
    assert_eq!(rows[..17], celebrate[..]);
    assert_eq!(rows[rows.len() - 4..], deject[..]);
    assert_eq!(disc.word(0x170ea4), 0x2403_0011);
    let address = |hi: u32, lo: u32| {
        ((disc.word(hi) & 0xffff) << 16).wrapping_add(disc.word(lo) as u16 as i16 as i32 as u32)
    };
    assert_eq!(address(0x170f80, 0x170f8c), 0x44cbbf);
    assert_eq!(address(0x171464, 0x171624), 0x44cc03);
    let base = usize::from(trick::CELEBRATE_FIRST);
    assert!(
        trick::REACTIONS[base..base + 17]
            .iter()
            .all(|r| r.address == 0x44cbbf)
    );
    let base = usize::from(trick::DEJECT_FIRST);
    assert_eq!(trick::REACTIONS.len(), base + 4);
    assert!(
        trick::REACTIONS[base..]
            .iter()
            .all(|r| r.address == 0x44cc03)
    );
}

#[test]
fn the_head_looks_lists_are_the_executables() {
    // `0x3e7060`'s rows (filled by the boot) point at lists in the
    // executable; the teammate's look draws (`0x159af0`, `r & 2`); the
    // kinds' switch (`0x43f000`) sends kind 6 there.
    use street_sim::look::{SITUATION_LISTS, SITUATIONS, TEAMMATE};
    let Some(disc) = Disc::load() else {
        return;
    };
    for (s, lists) in SITUATIONS.iter().zip(SITUATION_LISTS) {
        let read = |at: u32, n: usize| -> Vec<u8> {
            (0..n).map(|k| disc.word(at + 4 * k as u32) as u8).collect()
        };
        assert_eq!(read(lists[0], s.targets.len()), s.targets);
        assert_eq!(read(lists[1], s.events.len()), s.events);
        assert_eq!(read(lists[2], s.next.len()), s.next);
    }
    assert_eq!(disc.word(0x43f000 + 4 * u32::from(TEAMMATE)), 0x159af0);
    assert_eq!(disc.word(0x159af0), 0x0c00_0000 | (0x3a16b8 >> 2));
    assert_eq!(disc.word(0x159af8), 0x3042_0002);
}

#[test]
fn the_stand_scripts_are_the_executables() {
    // `FUN_00163d40`'s stand path: each stand script read again from the
    // executable (the flair's with its row and rate), its lists of
    // scripts and rows, the scripts it names in place and its constants.
    use street_sim::stand::{self, STAND_ADDRESSES, STAND_SCRIPTS};
    let Some(disc) = Disc::load() else {
        return;
    };
    for (k, &address) in STAND_ADDRESSES.iter().enumerate() {
        assert_eq!(STAND_SCRIPTS[k].address, address);
        assert_eq!(
            disc.stand_steps(address),
            STAND_SCRIPTS[k].steps,
            "{address:#x}"
        );
    }
    let first = usize::from(stand::FLAIR_FIRST);
    assert_eq!(STAND_SCRIPTS.len(), first + stand::FLAIR_ROWS.len());
    for (k, &(row, rate)) in stand::FLAIR_ROWS.iter().enumerate() {
        disc.preset.set(Some((u32::from(row), rate)));
        let steps = disc.stand_steps(0x457271);
        disc.preset.set(None);
        assert_eq!(STAND_SCRIPTS[first + k].address, 0x457271);
        assert_eq!(steps, STAND_SCRIPTS[first + k].steps, "flair {k}");
    }
    let words =
        |at: u32, n: usize| -> Vec<u32> { (0..n).map(|k| disc.word(at + 4 * k as u32)).collect() };
    let rows =
        |at: u32, n: usize| -> Vec<u16> { words(at, n).into_iter().map(|w| w as u16).collect() };
    let scripts = |list: &[u8]| -> Vec<u32> {
        list.iter()
            .map(|&k| STAND_ADDRESSES[usize::from(k)])
            .collect()
    };
    assert_eq!(words(0x3d7938, 15), scripts(&stand::OFFENCE_STANDS));
    assert_eq!(rows(0x3d78f0, 18), stand::OFFENCE_ROWS);
    assert_eq!(words(0x3d79d0, 4), scripts(&stand::DEFENCE_STANDS));
    assert_eq!(words(0x3d7a40, 5), scripts(&stand::POST_STANDS));
    assert_eq!(rows(0x3d7a20, 8), stand::POST_ROWS);
    assert_eq!(rows(0x3d7978, 22), stand::DEFENCE_ROWS);
    assert_eq!(rows(0x43f0a0, 7), stand::FLAIR_PLAYING);
    assert_eq!(words(0x3d7a08, 6), scripts(&stand::HELD_STANDS));
    assert_eq!(rows(0x3d79e0, 9), stand::HELD_ROWS);
    let dribble: Vec<u8> = (0..11).map(|k| stand::DRIBBLE_FIRST + k).collect();
    assert_eq!(words(0x3d7ad0, 11), scripts(&dribble));
    assert_eq!(rows(0x3d7a58, 29), stand::DRIBBLE_ROWS);
    // The scripts named in place (`lui a1, 0x45 / 0x44; addiu a1, lo`).
    let address = |hi: u32, lo: u32| {
        ((disc.word(hi) & 0xffff) << 16).wrapping_add(disc.word(lo) as u16 as i16 as i32 as u32)
    };
    // The box-out's contact clips (`0x135054..0x1350f0`, `0x135264..`).
    for (hi, lo, k) in [
        (0x135054, 0x13505c, stand::BOXER_BEHIND),
        (0x135080, 0x135088, stand::BOXED_BEHIND[0]),
        (0x13508c, 0x135090, stand::BOXED_BEHIND[1]),
        (0x1350b8, 0x1350c0, stand::BOXER_SIDE[0]),
        (0x1350b4, 0x1350c4, stand::BOXER_SIDE[1]),
        (0x1350e4, 0x1350ec, stand::BOXED_SIDE[0]),
        (0x1350e0, 0x1350f0, stand::BOXED_SIDE[1]),
        (0x135258, 0x135264, stand::BOXER_BEHIND),
    ] {
        assert_eq!(address(hi, lo), STAND_ADDRESSES[usize::from(k)], "{hi:#x}");
    }
    for (hi, lo, k) in [
        (0x1682a4, 0x1682ac, stand::BOX_OUT),
        (0x168170, 0x16819c, stand::READY),
        (0x1686e4, 0x1686ec, stand::CRADLE),
        (0x16876c, 0x168774, stand::PROTECT),
        (0x168908, 0x168910, stand::BACKDOWN),
        // The actor's pick stance and its let go (`FUN_0015a060`).
        (0x15b83c, 0x15b844, stand::PICK_STANCE),
        (0x15adf8, 0x15ae00, stand::READY),
    ] {
        assert_eq!(address(hi, lo), STAND_ADDRESSES[usize::from(k)], "{hi:#x}");
    }
    // The flair's rows, as the boot's constructor fills `0x3d9100`.
    let call = 0x0c00_0000 | (0x30fa50 >> 2);
    let mut flair = Vec::new();
    for at in (0x17c1d0..0x17c3c0).step_by(4) {
        if disc.word(at) != call {
            continue;
        }
        if let Some(w) = (1..=3)
            .map(|k| disc.word(at - 4 * k))
            .find(|w| w >> 21 == 0x09 << 5 && w & 0xffff >= 40)
        {
            flair.push((w & 0xffff) as u16);
        }
    }
    let ours: Vec<u16> = stand::FLAIR_ROWS.iter().map(|r| r.0).collect();
    assert_eq!(flair, ours);
    // `r % 15`, `r % 6`, `r % 11`, 30 handles, 14 from the rim, folds
    // under 301 and 200, the post defence's fps × 0.12.
    assert_eq!(disc.word(0x1682dc), 0x2403_000f);
    assert_eq!(disc.word(0x16879c), 0x2403_0006);
    assert_eq!(disc.word(0x168944), 0x2403_000b);
    assert_eq!(disc.word(0x168930), 0x2862_001e);
    assert_eq!(disc.word(0x168250), 0x3c01_4160);
    assert_eq!(disc.word(0x1683a8), 0x2863_012d);
    assert_eq!(disc.word(0x1683c8), 0x2402_00c8);
    assert_eq!(disc.word(0x16844c), 0x3c01_3df5);
    assert_eq!(
        (60.0f32 * f32::from_bits(0x3df5_c28f)) as u16,
        stand::POST_HOLD
    );
}

#[test]
fn the_dunks_knocks_are_the_executables() {
    // `FUN_001366d8`: the floor lists (`0x3d0f58`, by quarter), the air
    // knocks (`lui a1, 0x45; addiu a1, a1, lo` at 0x136ecc and 0x136f18),
    // row 2's body capsules, and the duel's bands (`slti` 21, 51, 101,
    // 151, 201).
    let Some(disc) = Disc::load() else {
        return;
    };
    for (quarter, list) in trick::KNOCK_FLOOR.iter().enumerate() {
        let at = disc.word(0x3d0f58 + 4 * quarter as u32);
        let mut scripts = Vec::new();
        for k in 0.. {
            match disc.word(at + 4 * k) {
                0 => break,
                s => scripts.push(s),
            }
        }
        let named: Vec<u32> = list
            .iter()
            .map(|&k| trick::REACTIONS[usize::from(k)].address)
            .collect();
        assert_eq!(named, scripts, "quarter {quarter}");
    }
    let lo = |a: u32| 0x45_0000u32.wrapping_add(disc.word(a) as u16 as i16 as i32 as u32);
    assert_eq!(
        trick::REACTIONS[usize::from(trick::KNOCK_BEHIND)].address,
        lo(0x136ecc)
    );
    assert_eq!(
        trick::REACTIONS[usize::from(trick::KNOCK_FRONT)].address,
        lo(0x136f18)
    );
    for (row, body) in [(2u32, &trick::KNOCK_BODY[..]), (4, &trick::SHOVE_BODY[..])] {
        let row = 0x3d0d90 + row * 12;
        let (at, count) = (disc.word(row), disc.word(row + 4));
        assert_eq!(count as usize, body.len());
        for (k, &(a, b, r, reach)) in body.iter().enumerate() {
            let w = executable_words(&disc.elf, at + 16 * k as u32, 4).unwrap();
            assert_eq!((w[0] as usize, w[1] as usize), (a, b));
            assert_eq!((f32::from_bits(w[2]), f32::from_bits(w[3])), (r, reach));
        }
    }
    for (a, n) in [
        (0x136b50, 21),
        (0x136b58, 51),
        (0x136bd8, 101),
        (0x136c64, 151),
        (0x136cd0, 201),
    ] {
        assert_eq!(disc.word(a), 0x2a62_0000 | n, "{a:#x}");
    }
}

#[test]
fn the_freestyles_are_the_executables() {
    // `FUN_0015c8e8`: each row's script (`addiu a1, a1, lo` after `lui a1,
    // 0x45`), its id (+0x264) and the opponent's reaction.
    let Some(disc) = Disc::load() else {
        return;
    };
    let lo = |a: u32| 0x45_0000u32.wrapping_add(disc.word(a) as u16 as i16 as i32 as u32);
    let scripts = [
        [0x15cb94, 0x15cbc0, 0x15cbe4],
        [0x15cc94, 0x15ccf4, 0x15cd4c],
    ];
    let reactions = [
        [0x15cc20, 0x15cc20, 0x15cc20],
        [0x15ccd0, 0x15cd30, 0x15cd88],
    ];
    let ids = [
        [0x15cbac, 0x15cbd8, 0x15cbf8],
        [0x15cca8, 0x15cd08, 0x15cd60],
    ];
    for row in 0..2 {
        for k in 0..3 {
            let (script, reaction) = trick::FREESTYLES[row][k];
            assert_eq!(
                trick::SCRIPTS[usize::from(script)].address,
                lo(scripts[row][k]),
                "row {row} {k}"
            );
            assert_eq!(
                trick::REACTIONS[usize::from(reaction)].address,
                lo(reactions[row][k]),
                "row {row} {k} reaction"
            );
            let id = disc.word(ids[row][k]) & 0xffff;
            // The last keeps id 45 though it plays script 58.
            let ours = if (row, k) == (1, 2) {
                45
            } else {
                u32::from(script)
            };
            assert_eq!(id, ours, "row {row} {k} id");
        }
    }
    // The bounds: 4.75 (`lui 0x4098`) and 8 (`lui 0x4100`).
    assert_eq!(disc.word(0x15cb34), 0x3c01_4098);
    assert_eq!(disc.word(0x15cc34), 0x3c01_4100);
}

#[test]
fn the_clearout_is_the_executables() {
    let Some(disc) = Disc::load() else {
        return;
    };
    // `FUN_0015c778`: the script (`lui a1, 0x45; addiu a1, a1, 0x6d02`),
    // the 0.2 (`0x3e4ccccd`).
    assert_eq!(disc.word(0x15c888), 0x3c05_0045);
    assert_eq!(disc.word(0x15c894), 0x24a5_6d02);
    assert_eq!(disc.word(0x15c854) & 0xffff, 0x3e4c);
    let s = &trick::SCRIPTS[usize::from(trick::CLEAROUT)];
    assert_eq!(s.address, 0x456d02);
    assert_eq!(s.mask, 0, "never drawn");
    assert_eq!(disc.steps(s.address), s.steps);
    let net = disc.net(s.steps, false);
    assert!((net[0] - s.root[0][0]).abs() < 1e-4 && (net[1] - s.root[0][1]).abs() < 1e-4);
    same_clock(&disc, s.address, s.steps, false);
    // `FUN_00136290`'s lists, quarter 2 of `0x43d0b0` and `0x43d0c0`.
    let list = |table: u32| {
        let at = disc.word(table + 8);
        let mut out = Vec::new();
        for k in 0.. {
            match disc.word(at + 4 * k) {
                0 => break,
                s => out.push(s),
            }
        }
        out
    };
    let named = |l: &[u8]| -> Vec<u32> {
        l.iter()
            .map(|&k| trick::REACTIONS[usize::from(k)].address)
            .collect()
    };
    assert_eq!(list(0x43d0b0), named(&trick::CLEAROUT_BRACED));
    assert_eq!(list(0x43d0c0), named(&trick::CLEAROUT_STUMBLE));
    let stances: Vec<u16> = (0..3).map(|k| disc.word(0x43d0d0 + 4 * k) as u16).collect();
    assert_eq!(stances, trick::CLEAROUT_STANCES);
    // `0x3d0d90` row 1: the arms.
    let row = 0x3d0d90 + 12;
    let (at, count) = (disc.word(row), disc.word(row + 4));
    assert_eq!(count as usize, trick::CLEAROUT_ARMS.len());
    for (k, &(a, b, r, reach)) in trick::CLEAROUT_ARMS.iter().enumerate() {
        let w = executable_words(&disc.elf, at + 16 * k as u32, 4).unwrap();
        assert_eq!((w[0] as usize, w[1] as usize), (a, b));
        assert_eq!((f32::from_bits(w[2]), f32::from_bits(w[3])), (r, reach));
    }
}

#[test]
fn the_reaction_scripts_and_lists_are_the_executables() {
    let Some(disc) = Disc::load() else {
        return;
    };
    for (k, r) in trick::REACTIONS.iter().enumerate() {
        // The celebrations and the win's reactions: their row and rate
        // on the shared script.
        let celebrate = usize::from(trick::CELEBRATE_FIRST);
        let deject = usize::from(trick::DEJECT_FIRST);
        let steps = if k >= deject {
            let (row, _, rate) = trick::DEJECT_ROWS[k - deject];
            disc.steps_preset(r.address, u32::from(row), rate)
        } else if k >= celebrate {
            let (row, rate) = trick::CELEBRATE_ROWS[k - celebrate];
            disc.steps_preset(r.address, u32::from(row), rate)
        } else {
            disc.steps(r.address)
        };
        assert_eq!(steps, r.steps, "reaction {k} {:#x}", r.address);
        let net = disc.net(r.steps, false);
        assert!(
            (net[0] - r.root[0]).abs() < 1e-4 && (net[1] - r.root[1]).abs() < 1e-4,
            "reaction {:#x} root {net:?} {:?}",
            r.address,
            r.root
        );
        if k < celebrate {
            same_clock(&disc, r.address, r.steps, false);
        }
    }
    // `FUN_001372d8` indexes the lists by `FUN_0018d9f0`'s quarter (its
    // third output): four zero-ended lists at each table.
    for (table, ours) in [(0x3d0f18, &trick::HELD), (0x3d0f48, &trick::FAKED)] {
        for (quarter, list) in ours.iter().enumerate() {
            let at = disc.word(table + 4 * quarter as u32);
            let mut scripts = Vec::new();
            for k in 0.. {
                match disc.word(at + 4 * k) {
                    0 => break,
                    s => scripts.push(s),
                }
            }
            let named: Vec<u32> = list
                .iter()
                .map(|&k| trick::REACTIONS[usize::from(k)].address)
                .collect();
            assert_eq!(named, scripts, "{table:#x} quarter {quarter}");
        }
    }
    // The step-through (`0x137778`: lui 0x45, addiu -0x3caa).
    assert_eq!(disc.word(0x137778), 0x3c05_0045);
    assert_eq!(disc.word(0x137780), 0x24a5_c356);
    assert_eq!(
        trick::REACTIONS[usize::from(trick::STEP_THROUGH)].address,
        0x44c356
    );
    // The screen (`0x135400..0x135640`): the screener's `0x3d0fb0[r % 3]`
    // (`lui s0, 0x3d; addiu s0, s0, 0xfb0`), the defender's heavy and
    // light tables (`addiu v1, v1, 0xf28` / `0xf38`) by the quarter.
    assert_eq!(disc.word(0x1354bc), 0x3c10_003d);
    assert_eq!(disc.word(0x1354c4), 0x2610_0fb0);
    assert_eq!(disc.word(0x135544), 0x2463_0f28);
    assert_eq!(disc.word(0x1355a4), 0x2463_0f38);
    let screener: Vec<u32> = (0..3).map(|k| disc.word(0x3d0fb0 + 4 * k)).collect();
    let named = |l: &[u8]| -> Vec<u32> {
        l.iter()
            .map(|&k| trick::REACTIONS[usize::from(k)].address)
            .collect()
    };
    assert_eq!(screener, named(&trick::SCREENER));
    // A person's pick (`FUN_0015a060`, `0x15b68c`: `0x43f038`, the same
    // three scripts) and a computer's (`0x43f028`, `trick::PICKS`).
    let person: Vec<u32> = (0..3).map(|k| disc.word(0x43f038 + 4 * k)).collect();
    assert_eq!(person, named(&trick::SCREENER));
    let computer: Vec<u32> = (0..3).map(|k| disc.word(0x43f028 + 4 * k)).collect();
    assert_eq!(computer, named(&trick::PICKS));
    for (table, ours) in [
        (0x3d0f28, &trick::SCREENED_HEAVY),
        (0x3d0f38, &trick::SCREENED_LIGHT),
    ] {
        for (quarter, list) in ours.iter().enumerate() {
            let at = disc.word(table + 4 * quarter as u32);
            let mut scripts = Vec::new();
            for k in 0.. {
                match disc.word(at + 4 * k) {
                    0 => break,
                    s => scripts.push(s),
                }
            }
            assert_eq!(named(list), scripts, "{table:#x} quarter {quarter}");
        }
    }
}

#[test]
fn the_trick_press_reads_the_ordinary_row_and_the_slot_table() {
    let Some(disc) = Disc::load() else {
        return;
    };
    // `FUN_00139850`: the ordinary players' row of 75 masks.
    let words = |a: u32, n: usize| executable_words(&disc.elf, a, n).unwrap();
    let mut mask = [0u8; 75];
    for s in words(0x43d498, 75) {
        mask[s as usize] = 0;
    }
    mask[0x4a] = 3;
    for s in [0x23, 0xb, 0x2d, 0x32, 0x33, 0x3a] {
        mask[s] = 0;
    }
    for k in 0..24 {
        mask[disc.word(0x43d5a4 - 4 * k) as usize] = 0;
    }
    for s in words(0x43d4c4, 17) {
        mask[s as usize] = 2;
    }
    for s in words(0x43d508, 16) {
        mask[s as usize] = 1;
    }
    let slots = disc.word(0x3d17ac);
    for (k, s) in words(0x43d498, 11).into_iter().enumerate() {
        assert_eq!(u32::from(trick::SLOTS[k]), s, "slot {k}");
        mask[s as usize] = 1 << disc.word(slots + 8 * k as u32 + 4);
        assert_eq!(
            disc.word(slots + 8 * k as u32 + 4),
            u32::from(street_sim::player::TRICK_SLOT_LEVELS[k])
        );
    }
    for (id, s) in trick::SCRIPTS.iter().enumerate().take(75) {
        assert_eq!(s.mask, mask[id], "trick {id} mask");
    }
}

#[test]
fn the_fakeout_reads_its_odds_capsules_and_records() {
    let Some(disc) = Disc::load() else {
        return;
    };
    let words = |a: u32, n: usize| executable_words(&disc.elf, a, n).unwrap();
    // `0x1375d8..0x13763c`: lo + trunc((hi - lo) × Handles / 99).
    let (lo, hi) = (words(0x43d0e0, 5), words(0x43d0f8, 5));
    for (k, &(l, h)) in trick::ODDS.iter().enumerate() {
        assert_eq!((l as u32, h as u32), (lo[k], hi[k]));
    }
    assert_eq!(
        executable_constant(&disc.elf, 0x137628).unwrap(),
        0x42c6_0000
    );
    // `0x13766c`: addiu v0, s0, 10 for a person on the first difficulty
    // (`0x137658`: bnez past it when `FUN_00172220` finds the computer).
    assert_eq!(disc.word(0x13766c), 0x2602_0000 | trick::PERSON_ODDS as u32);
    assert_eq!(disc.word(0x137658), 0x1440_0007);
    // The kind-3 row the trick scripts' `a6` name (`0x3d0d90` row 3): the
    // handler's four capsules.
    let row = 0x3d0d90 + 3 * 12;
    let (at, count) = (disc.word(row), disc.word(row + 4));
    assert_eq!(count as usize, trick::HANDLER.len());
    for (k, &(a, b, r, reach)) in trick::HANDLER.iter().enumerate() {
        let w = words(at + 16 * k as u32, 4);
        assert_eq!((w[0] as usize, w[1] as usize), (a, b));
        assert_eq!((f32::from_bits(w[2]), f32::from_bits(w[3])), (r, reach));
    }
    // `FUN_001392b8`'s three lists of the other's capsules.
    for (g, list) in trick::DEFENDER.iter().enumerate() {
        let (at, count) = (
            disc.word(0x3d1180 + 8 * g as u32),
            disc.word(0x3d1184 + 8 * g as u32),
        );
        assert_eq!(count as usize, list.len());
        for (k, &(a, b, r, reach)) in list.iter().enumerate() {
            let w = words(at + 16 * k as u32, 4);
            assert_eq!((w[0] as usize, w[1] as usize), (a, b));
            assert_eq!((f32::from_bits(w[2]), f32::from_bits(w[3])), (r, reach));
        }
    }
    // `FUN_001372d8`'s records: those that never fake (`0x13750c..`), the
    // pair that closes the window (`0x1374fc`), the step-through's
    // (`0x137744..`).
    let never = [
        0x13750c, 0x137514, 0x13751c, 0x137524, 0x13752c, 0x137534, 0x13753c, 0x137544, 0x13754c,
        0x137554,
    ]
    .map(|a| executable_constant(&disc.elf, a).unwrap() as u16);
    assert_eq!(disc.word(0x1374fc) & 0xffff, (-0x2b0i32 as u32) & 0xffff);
    let mut all: Vec<u16> = never.to_vec();
    all.extend([0x2b0, 0x2b1]);
    let mut ours = trick::NO_FAKE.to_vec();
    all.sort_unstable();
    ours.sort_unstable();
    assert_eq!(all, ours);
    assert_eq!(disc.word(0x137744) & 0xffff, (-0xbci32 as u32) & 0xffff);
    let through = [0x137754, 0x13775c, 0x137764, 0x13776c]
        .map(|a| executable_constant(&disc.elf, a).unwrap() as u16);
    assert_eq!(
        trick::STEP_THROUGH_RECORDS,
        [0xbc, 0xbd, through[0], through[1], through[2], through[3]]
    );
    // `FUN_00139408`: the capsules only within 6 (`lui 0x40c0`).
    let six = (0x139408..0x139560)
        .step_by(4)
        .any(|a| disc.word(a) & 0xffff_0000 == 0x3c01_0000 && disc.word(a) & 0xffff == 0x40c0);
    assert!(six);
    assert_eq!(trick::REACH, 6.0);
}

/// A step as `held_tables.rs` spells it.
fn spelled(s: &Step) -> String {
    let f = |v: f32| format!("f32::from_bits({:#x})", v.to_bits());
    let r = |v: u16| {
        if v == trick::CURRENT {
            "CURRENT".to_string()
        } else {
            v.to_string()
        }
    };
    match *s {
        Step::Bits(w) => format!("Bits({w:#x})"),
        Step::Clear(w) => format!("Clear({w:#x})"),
        Step::Blend { n, start, records } => format!(
            "Blend {{ n: {n}, start: {start}, records: [{}, {}] }}",
            r(records[0]),
            r(records[1])
        ),
        Step::Pick {
            n,
            start,
            records,
            by,
        } => format!(
            "Pick {{ n: {n}, start: {start}, records: [{}, {}], by: {by:?} }}",
            r(records[0]),
            r(records[1])
        ),
        Step::Stop => "Stop".into(),
        Step::Window { row, n } => format!("Window {{ row: {row}, n: {n} }}"),
        Step::Shove { row, n } => format!("Shove {{ row: {row}, n: {n} }}"),
        Step::Knock { row, n } => format!("Knock {{ row: {row}, n: {n} }}"),
        Step::Wait { to, n } => format!("Wait {{ to: {to}, n: {n} }}"),
        Step::PlayOut { per_frame, to, n } => {
            format!(
                "PlayOut {{ per_frame: {}, to: {to}, n: {n} }}",
                f(per_frame)
            )
        }
        Step::Land { to } => format!("Land {{ to: {to} }}"),
        Step::Flight { to } => format!("Flight {{ to: {to} }}"),
        Step::Hop { turn, speed, rise } => format!(
            "Hop {{ turn: {turn}, speed: {}, rise: {} }}",
            f(speed),
            f(rise)
        ),
        Step::Call(c) => format!("Call({c})"),
        Step::Test(c) => format!("Test({c})"),
        Step::End => "End".into(),
        Step::Flags(w) => format!("Flags({w:#x})"),
        Step::Rise(v) => format!("Rise({})", f(v)),
        Step::Toward { turn, speed, rise } => format!(
            "Toward {{ turn: {turn}, speed: {}, rise: {} }}",
            f(speed),
            f(rise)
        ),
        Step::Dive { speed, rise } => format!("Dive {{ speed: {}, rise: {} }}", f(speed), f(rise)),
        Step::Slide(v) => format!("Slide({})", f(v)),
        Step::Aim(a) => format!(
            "Aim(jump::Aim {{ second: {}, mid: {}, side: {}, direction: {}, updates: {}, frame: {}, records: [{}, {}] }})",
            a.second,
            a.mid,
            a.side,
            a.direction,
            a.updates,
            a.frame,
            r(a.records[0]),
            r(a.records[1])
        ),
        Step::Reach(a) => format!(
            "Reach(jump::Reach {{ cap: {}, updates: {}, frame: {}, ahead: {} }})",
            a.cap,
            a.updates,
            a.frame,
            f(a.ahead)
        ),
        Step::Carry(v) => format!("Carry({})", f(v)),
        Step::Want(w) => format!("Want({w})"),
        Step::Plant(p) => format!(
            "Plant(jump::Plant {{ hands: {}, keep: {}, pick: {:#x}, bones: [{}, {}] }})",
            p.hands, p.keep, p.pick, p.bones[0], p.bones[1]
        ),
        Step::Unplant => "Unplant".into(),
        Step::OnRecord { records, to } => format!("OnRecord {{ records: {records:?}, to: {to} }}"),
        Step::Either {
            record,
            plain,
            alternate,
        } => format!("Either {{ record: {record}, plain: {plain}, alternate: {alternate} }}"),
        Step::Hang { hang, other } => format!("Hang {{ hang: {hang}, other: {other} }}"),
        Step::Loop { per_frame, frames } => {
            format!("Loop {{ per_frame: {}, frames: {frames} }}", f(per_frame))
        }
        Step::Jump(to) => format!("Jump({to})"),
        Step::Roll { n, to } => format!("Roll {{ n: {n}, to: {to} }}"),
        Step::RightHand(to) => format!("RightHand({to})"),
        Step::Asked(to) => format!("Asked({to})"),
    }
}

/// Prints a plain-path script's steps and its clip's root change (run
/// with `--ignored --nocapture`, `STEPS=0x..,0x..`).
#[test]
#[ignore]
fn print_plain_steps() {
    let Some(disc) = Disc::load() else {
        return;
    };
    let list = std::env::var("STEPS").unwrap_or_default();
    for item in list.split(',').filter(|s| !s.is_empty()) {
        let address = u32::from_str_radix(item.trim_start_matches("0x"), 16).unwrap();
        let steps = disc.steps(address);
        println!(
            "// {address:#x} root {:?} {:?}",
            disc.net(&steps, false),
            disc.net(&steps, true)
        );
        println!("const S{address:X}: &[Step] = &[");
        for s in &steps {
            println!("    {},", spelled(s));
        }
        println!("];");
    }
}

/// Prints [`trick::REACTIONS`]' celebration and win-reaction entries
/// (run with `--ignored --nocapture`).
#[test]
#[ignore]
fn print_celebration_reactions() {
    let Some(disc) = Disc::load() else {
        return;
    };
    let rows = trick::CELEBRATE_ROWS
        .iter()
        .map(|&(r, rate)| (0x44cbbf, r, rate))
        .chain(
            trick::DEJECT_ROWS
                .iter()
                .map(|&(r, _, rate)| (0x44cc03, r, rate)),
        );
    for (address, record, rate) in rows {
        let steps = disc.steps_preset(address, u32::from(record), rate);
        let root = disc.net(&steps, false);
        println!("    Reaction {{");
        println!("        address: {address:#x},");
        println!(
            "        root: [f32::from_bits({:#010x}), f32::from_bits({:#010x})],",
            root[0].to_bits(),
            root[1].to_bits()
        );
        println!("        steps: &[");
        for s in &steps {
            println!("            {},", spelled(s));
        }
        println!("        ],");
        println!("    }},");
    }
}

/// Prints `stand_tables.rs`' scripts (run with `--ignored --nocapture`).
#[test]
#[ignore]
fn print_stand_scripts() {
    let Some(disc) = Disc::load() else {
        return;
    };
    let flair = street_sim::stand::FLAIR_ROWS.iter().map(|&(row, rate)| {
        disc.preset.set(Some((u32::from(row), rate)));
        let steps = disc.stand_steps(0x457271);
        disc.preset.set(None);
        (0x457271, steps)
    });
    let rows: Vec<(u32, Vec<Step>)> = street_sim::stand::STAND_ADDRESSES
        .iter()
        .map(|&a| (a, disc.stand_steps(a)))
        .chain(flair)
        .collect();
    for (address, steps) in rows {
        println!("    StandScript {{");
        println!("        address: {address:#x},");
        println!("        steps: &[");
        for s in &steps {
            println!("            {},", spelled(s));
        }
        println!("        ],");
        println!("    }},");
    }
}

/// Prints `held_tables.rs`'s consts for scripts not yet in it (run with
/// `--ignored --nocapture` when a row is added).
#[test]
#[ignore]
fn print_held_steps() {
    let Some(disc) = Disc::load() else {
        return;
    };
    let held = Paths {
        held: true,
        fake: false,
        jumps: true,
    };
    let list = std::env::var("STEPS").unwrap_or_default();
    for item in list.split(',').filter(|s| !s.is_empty()) {
        let (address, fake) = match item.strip_suffix("_fake") {
            Some(a) => (
                u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap(),
                true,
            ),
            None => (
                u32::from_str_radix(item.trim_start_matches("0x"), 16).unwrap(),
                false,
            ),
        };
        let steps = disc.steps_on(address, Paths { fake, ..held });
        let release = street_assets::scripts::script_callback_tick(&disc.elf, address, 21).unwrap();
        let end = street_assets::scripts::script_end_tick(&disc.elf, address).unwrap();
        println!("// release {release:?} end {end:?}");
        println!(
            "const S{:X}{}: &[Step] = &[",
            address,
            if fake { "_FAKE" } else { "" }
        );
        for s in &steps {
            println!("    {},", spelled(s));
        }
        println!("];");
    }
}

#[test]
fn the_held_moves_scripts_are_the_executables() {
    // The shot scripts along the held path and the pump fake's, the dunk
    // and lay-up rows' and the pass rows' scripts, and the scoop's
    // (`0x45748a`): each read again from the executable as
    // `held_tables.rs` was generated.
    use street_sim::held::{
        DIVE_STEPS, EXTRA_STEPS, FAKE_STEPS, LEAP_STEPS, MOVE_STEPS, PASS_STEPS, SHOT_STEPS,
    };
    use street_sim::timing::{
        DIVE_SCRIPTS, EXTRA_ROWS, LEAP_ROWS, MOVE_ROWS, PASS_ROWS, SHOT_SCRIPTS,
    };
    let Some(disc) = Disc::load() else {
        return;
    };
    let held = Paths {
        held: true,
        fake: false,
        jumps: true,
    };
    for (k, s) in SHOT_SCRIPTS.iter().enumerate() {
        assert_eq!(
            disc.steps_on(s.script, held),
            SHOT_STEPS[k],
            "shot {k} {}",
            s.name
        );
        if s.check > 0 {
            let fake = Paths { fake: true, ..held };
            assert_eq!(
                disc.steps_on(s.script, fake),
                FAKE_STEPS[k],
                "fake {k} {}",
                s.name
            );
        } else {
            assert!(FAKE_STEPS[k].is_empty(), "fake {k}");
        }
    }
    for (k, r) in MOVE_ROWS.iter().enumerate() {
        assert_eq!(
            disc.steps_on(r.script, held),
            MOVE_STEPS[k],
            "move {k} {}",
            r.name
        );
    }
    for (k, r) in PASS_ROWS.iter().enumerate() {
        assert_eq!(disc.steps_on(r.script, held), PASS_STEPS[k], "pass {k}");
    }
    // The two-turbo leaps (`0x3d70d0`) and the dives.
    for (k, r) in LEAP_ROWS.iter().enumerate() {
        assert_eq!(disc.word(0x3d70d0 + 4 * k as u32), r.script, "leap {k}");
        assert_eq!(disc.steps_on(r.script, held), LEAP_STEPS[k], "leap {k}");
    }
    for (k, &a) in DIVE_SCRIPTS.iter().enumerate() {
        assert_eq!(disc.steps_on(a, held), DIVE_STEPS[k], "dive {k}");
    }
    // `layup_3_bboard` and `FUN_0016b1e8`'s ten mid-air lay-ups.
    for (k, r) in EXTRA_ROWS.iter().enumerate() {
        assert_eq!(disc.steps_on(r.script, held), EXTRA_STEPS[k], "{}", r.name);
    }
    for (k, r) in street_sim::timing::PUTBACK_ROWS.iter().enumerate() {
        let steps = street_sim::held::PUTBACK_STEPS[k];
        assert_eq!(disc.steps_on(r.script, held), steps, "{}", r.name);
        let release =
            street_assets::scripts::script_callback_tick(&disc.elf, r.script, 21).unwrap();
        let end = street_assets::scripts::script_end_tick(&disc.elf, r.script).unwrap();
        assert_eq!(
            (release, end),
            (Some(u32::from(r.release)), Some(u32::from(r.end))),
            "{}",
            r.name
        );
    }
    // The staged Level 2 dunks' finishing bodies (`0x415740`'s rows).
    for (k, r) in street_sim::timing::FINISH_ROWS.iter().enumerate() {
        let steps = street_sim::held::FINISH_STEPS[k];
        assert_eq!(disc.steps_on(r.script, held), steps, "{}", r.name);
        let release =
            street_assets::scripts::script_callback_tick(&disc.elf, r.script, 21).unwrap();
        let end = street_assets::scripts::script_end_tick(&disc.elf, r.script).unwrap();
        assert_eq!(
            (release, end),
            (Some(u32::from(r.release)), Some(u32::from(r.end))),
            "{}",
            r.name
        );
    }
    assert_eq!(
        disc.steps_on(
            0x45748a,
            Paths {
                jumps: false,
                ..held
            }
        ),
        street_sim::touch::SCOOP_STEPS,
        "the scoop"
    );
}

#[test]
fn the_counter_scripts_are_the_executables() {
    // The counter stance's two scripts (`0x3d7228`, the press's `lui s0,
    // 0x3d; addiu s0, s0, 0x7228` at `0x15e01c`), the take (`0x3d0fa0`,
    // `0x134e80`), the take after a trick that never fakes (`0x45795d`,
    // `0x134df8`/`0x134e54`), and the handler's and the missed defender's
    // stumbles (`0x44bf61` at `0x134bdc`/`0x134bf0`, `0x44bf94` at
    // `0x134ee0`/`0x134ee8`); the odds' 0.8 (`0x134b68`) and the press's
    // 0.2 (`0x15dfe8`).
    use street_sim::counter;
    let Some(disc) = Disc::load() else {
        return;
    };
    assert_eq!(disc.word(0x15e01c), 0x3c10_003d);
    assert_eq!(disc.word(0x15e028), 0x2610_7228);
    for k in 0..2 {
        assert_eq!(
            disc.word(counter::STANCE_TABLE + 4 * k),
            counter::SCRIPTS[k as usize].address
        );
    }
    assert_eq!(disc.word(0x134e80), 0x3c03_003d);
    assert_eq!(disc.word(0x134e88), 0x8c65_0fa0);
    assert_eq!(
        disc.word(counter::TAKE_TABLE),
        counter::SCRIPTS[usize::from(counter::TAKE)].address
    );
    assert_eq!(disc.word(0x134df8), 0x3c05_0045);
    assert_eq!(disc.word(0x134e54), 0x24a5_795d);
    assert_eq!(
        counter::SCRIPTS[usize::from(counter::TAKE_NO_FAKE)].address,
        0x45795d
    );
    assert_eq!(disc.word(0x134bdc), 0x3c05_0045);
    assert_eq!(disc.word(0x134bf0), 0x24a5_bf61);
    assert_eq!(disc.word(0x134ee0), 0x3c05_0045);
    assert_eq!(disc.word(0x134ee8), 0x24a5_bf94);
    assert_eq!(counter::reaction_addresses(), [0x44bf61, 0x44bf94]);
    assert_eq!(
        [disc.word(0x134b68), disc.word(0x134b6c)],
        [0x3c01_3f4c, 0x3421_cccd]
    );
    assert_eq!(
        [disc.word(0x15dfe8), disc.word(0x15dfec)],
        [0x3c01_3e4c, 0x3421_cccd]
    );
    let held = Paths {
        held: true,
        ..Default::default()
    };
    for (k, s) in counter::SCRIPTS.iter().enumerate() {
        assert_eq!(
            disc.steps_on(s.address, held),
            s.steps,
            "counter script {k}"
        );
        for left in [false, true] {
            // The take's pick by the hand plays its alternate for a
            // left-hander: the root as a blend of that record.
            let steps: Vec<Step> = s
                .steps
                .iter()
                .map(|&step| match step {
                    Step::Pick {
                        n, start, records, ..
                    } => Step::Blend {
                        n,
                        start,
                        records: [records[usize::from(left)]; 2],
                    },
                    other => other,
                })
                .collect();
            let net = disc.net(&steps, false);
            let table = s.root[usize::from(left)];
            assert!(
                (net[0] - table[0]).abs() < 1e-4 && (net[1] - table[1]).abs() < 1e-4,
                "counter script {k} root {net:?} {table:?}"
            );
        }
        same_clock(&disc, s.address, s.steps, false);
    }
}
