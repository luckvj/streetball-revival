//! The advanced dunk (docs/TRACE.md "Advanced Dunks"): the four-turbo
//! slot's advanced id ([`crate::timing::ADVANCED_DUNKS`]) from 8.5 or more
//! out (`FUN_001693b8`, `0x169628..0x16965c`) starts `bbpass_1`
//! (`0x447b53`) on the upper body and returns, with no row drawn, while
//! channel 0 runs on (his walk goes on). The script sets state bits 0x90,
//! blends three updates and waits seven; at its tick 10:
//!
//! - callback 31 (`FUN_00173ec0` with 20, 0): the ball leaves him
//!   (`FUN_0014bff8`), flies 20 updates to the board's point `(±(38 − r),
//!   15 − r, 0)`, r the ball's radius, and he is barred `trunc(fps × 0.34)`
//!   = 20 updates (+0x1a8); the zone flag (`DAT_003d9210`) is set only for
//!   a ball let go 23..32.5 across and within 8 of the middle (the ball's
//!   place, not his);
//! - callback 32 (`FUN_001773b0`): the finish by his slot-0 dunk id
//!   ([`crate::timing::advanced_finish`]) on channel 0, a leap.
//!
//! The finish's own callback 31 (`FUN_001631c8` with 45, 1, 20): with the
//! zone flag and the ball still his toss, a second leg to his hand as his
//! clip will show it at frame 20, plus his velocity times the 45 updates
//! and half the gravity times their square, in 45 updates. He catches it
//! on the leaper's bone rules and slams it at callback 21. Without the
//! second leg the toss ends at the board and the ball comes off it loose.
//! The School's Advanced Dunks capture (the re-made set, school-3 and -6)
//! holds a success from 9.4 out at x -29.6: the toss's 20 updates, the
//! second leg's 45, the catch 40 updates on at 7.4 up, the slam.

use crate::ball::{NO_CATCH, Pass, Via};
use crate::{BallState, World, target_hoop, timing};

/// The advanced dunk under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Advanced {
    pub player: usize,
    /// `bbpass_1`'s tick: updates since the press.
    pub tick: u8,
    /// `DAT_003d9210`: the toss came from 23..32.5 across, within 8 of the
    /// middle.
    pub zone: bool,
    /// Callback 32 ran: the finish leaps.
    pub finishing: bool,
    /// The ball's offset from him in his own frame as the toss clip's blend
    /// began (`FUN_00193300`), eased from over its three updates.
    pub anchor: Option<[f32; 3]>,
    /// The leg back the finish's callback 31 queued: his hand's point and
    /// the world's tick it must arrive by (flown from the board once the
    /// toss gets there).
    pub back: Option<([f32; 3], u64)>,
}

/// `trunc(fps × 0.34)`: the updates the toss bars him (`0x3eae147b`).
pub fn toss_bar() -> u16 {
    (crate::TICK_HZ as f32 * f32::from_bits(0x3eae_147b)) as u16
}

impl World {
    /// The press's advanced dunk: `bbpass_1` starts.
    pub(crate) fn start_advanced(&mut self, i: usize) {
        self.advanced = Some(Advanced {
            player: i,
            tick: 0,
            zone: false,
            finishing: false,
            anchor: None,
            back: None,
        });
    }

    /// The toss on the upper body, each update of his: at its tick 10 the
    /// toss (callback 31) and the finish (callback 32).
    pub(crate) fn advanced_tick(&mut self, i: usize) {
        let Some(mut a) = self.advanced.filter(|a| a.player == i && !a.finishing) else {
            return;
        };
        if self.ball.state != BallState::Held(i) {
            self.advanced = None;
            return;
        }
        a.tick += 1;
        if a.tick < timing::ADVANCED_TOSS_TICK {
            self.advanced = Some(a);
            return;
        }
        let p = &self.players[i];
        let hoop = target_hoop(p.team);
        let side = if self.rim_spot(hoop)[0] >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let r = self.ball.radius;
        let point = [side * (38.0 - r), 15.0 - r, 0.0];
        // The ball leaves the toss clip's hand at the wait's frame 9 (its
        // blend's records 490, or 489 on the live left hand).
        let record = if self.players[i].gait_channel.hand == 2 {
            489
        } else {
            490
        };
        let from = self.hand_on(i, record, 9.0).unwrap_or(self.ball.position);
        // The zone is the ball's place as he lets it go (ball+0x30 into
        // `0x3d6590`, `0x173fc0`): across by the hoop's side, and z.
        let [x, _, z] = from;
        a.zone = (23.0..=32.5).contains(&(side * x).abs()) && z.abs() <= 8.0;
        self.ball.throw(
            from,
            point,
            20.0 / crate::TICK_HZ as f32,
            Pass {
                from: i,
                to: i,
                tried: u8::MAX,
                via: Via::Glass { hoop },
                catch: NO_CATCH,
                since: self.tick,
            },
        );
        self.players[i].barred = toss_bar();
        // Callback 32: the finish by his slot-0 dunk.
        let id = self.players[i]
            .shooter
            .map_or(timing::DUNK_SLOTS[0], |s| s.dunks[0]);
        let row = timing::advanced_finish(id);
        if let Some(r) = timing::move_row(row) {
            let spot = self.rim_side(i, hoop, 1.2);
            self.begin_move(i, row);
            self.players[i].start_dunk(
                hoop,
                spot,
                crate::player::DunkStyle::Advanced,
                (row, r.release, r.end),
            );
        }
        a.finishing = true;
        self.advanced = Some(a);
    }

    /// The finish's callback 31 (`FUN_001631c8` with 45, 1, 20): from the
    /// zone with the toss still flying, the leg back to his hand, due 45
    /// updates on; the toss flies on to the board first.
    pub(crate) fn advanced_second_leg(&mut self, i: usize, velocity: [f32; 3]) {
        let Some(mut a) = self.advanced.filter(|a| a.player == i && a.finishing) else {
            return;
        };
        let tossing = self.ball.state == BallState::Pass
            && self.ball.pass.is_some_and(|p| p.from == i && p.to == i);
        if !a.zone || !tossing {
            self.advanced = None;
            return;
        }
        let flight = 45.0;
        let point = self.hand_ahead(i, 20.0, flight, velocity);
        a.back = Some((point, self.tick + flight as u64));
        self.advanced = Some(a);
    }

    /// The toss at the board (`FUN_00148378`'s leg end): with the leg back
    /// queued the ball flies to his hand in the updates left, else none
    /// here. True when it handled the leg.
    pub(crate) fn advanced_leg(&mut self, pass: Pass) -> bool {
        let Some((hand, due)) = self
            .advanced
            .filter(|a| a.player == pass.from)
            .and_then(|a| a.back)
        else {
            return false;
        };
        self.advanced = None;
        let left = due.saturating_sub(self.tick).max(1);
        let from = self.ball.position;
        self.ball.throw(
            from,
            hand,
            left as f32 / crate::TICK_HZ as f32,
            Pass {
                via: Via::Direct,
                ..pass
            },
        );
        true
    }

    /// `FUN_0014e060` while the toss clip plays on his upper body: the
    /// ball in its hand (records 490, or 489 on the live left hand), held
    /// at the clip's frame 2 through the three-update blend, eased from
    /// the offset stored as it began, then at frame `tick − 1` through the
    /// wait. None when no toss is under way or without the disc's
    /// animations.
    pub(crate) fn advanced_ball(
        &mut self,
        h: usize,
        before: Option<(usize, [f32; 3])>,
    ) -> Option<[f32; 3]> {
        // The press's own update still shows his dribble (the clip starts
        // on his next).
        let mut a = self
            .advanced
            .filter(|a| a.player == h && !a.finishing && a.tick > 0)?;
        let poses = self.poses.arc()?;
        let p = &self.players[h];
        let record = if p.gait_channel.hand == 2 { 489 } else { 490 };
        let tick = f32::from(a.tick);
        let frame = if tick <= 3.0 { 2.0 } else { tick - 1.0 };
        let place = self.place(h);
        let at = crate::pose::Frame {
            record,
            time: frame,
            looping: false,
        };
        let pose = poses.pose(at, p.body(), place)?;
        let state = poses.state(record, frame as i32);
        let point = crate::dribble::in_hand(
            &pose,
            record,
            frame as i32,
            state,
            p.body(),
            self.ball.radius,
        );
        if a.tick == 1 {
            a.anchor = before.filter(|b| b.0 == h).map(|b| b.1);
            self.advanced = Some(a);
        }
        Some(match a.anchor.filter(|_| a.tick < 3) {
            Some(local) => {
                let q = crate::held::yaw(place.word);
                let o = crate::pose::rotate(q, local);
                let f = tick / 3.0;
                std::array::from_fn(|k| (place.position[k] + o[k]) * (1.0 - f) + point[k] * f)
            }
            None => point,
        })
    }

    /// `i`'s dunk id (actor+0x1e8) for his dunk row: the row's, but an
    /// advanced finish's is his slot-0 dunk id (callback 32,
    /// `0x1773f4..0x177410`, whatever finish it picked). None off a dunk.
    pub fn dunk_id(&self, i: usize) -> Option<u8> {
        let p = self.players.get(i)?;
        let crate::Action::Dunking { row, .. } = p.action else {
            return None;
        };
        if (timing::ADVANCED_FIRST..timing::ADVANCED_FIRST + timing::ADVANCED_ROWS.len() as u8)
            .contains(&row)
        {
            return Some(p.shooter.map_or(timing::DUNK_SLOTS[0], |s| s.dunks[0]));
        }
        timing::move_row(row).map(|r| r.id)
    }

    /// `i`'s hand on `record` at `frame`, posed at his place facing his
    /// facing; none without the disc's animations.
    pub(crate) fn hand_on(&self, i: usize, record: u16, frame: f32) -> Option<[f32; 3]> {
        let poses = self.poses.get()?;
        let p = &self.players[i];
        let at = crate::pose::Frame {
            record,
            time: frame,
            looping: false,
        };
        let pose = poses.pose(at, p.body(), self.place(i))?;
        let state = poses.state(record, frame as i32);
        let right = pose.joints[crate::pose::RIGHT_HAND].position;
        let left = pose.joints[crate::pose::LEFT_HAND].position;
        Some(match state & 0xc {
            8 => right,
            4 => left,
            _ => std::array::from_fn(|k| (right[k] + left[k]) * 0.5),
        })
    }

    /// `i`'s hand as his scripted clip will show it at `frame` (wrapped by
    /// the clip's length), posed at his place facing his wanted facing, the
    /// hand by the frame's flags (8 the right, 4 the left, else the
    /// midpoint), plus his velocity times `flight` and half the gravity
    /// times its square (`velocity`, units an update, as the aim just set
    /// it). Without the disc's animations his reach above his feet.
    pub(crate) fn hand_ahead(
        &self,
        i: usize,
        frame: f32,
        flight: f32,
        velocity: [f32; 3],
    ) -> [f32; 3] {
        let p = &self.players[i];
        let mut hand = [p.position[0], p.height + p.ratings.reach, p.position[1]];
        if let (Some(poses), Some(run)) = (self.poses.get(), self.move_run(i))
            && let Some(record) = run.record
        {
            let mut frame = frame;
            if let Some(n) = poses.frames(record) {
                while frame >= n as f32 {
                    frame -= n as f32;
                }
            }
            let mut place = self.place(i);
            place.word = timing::facing_word(p.wanted);
            let at = crate::pose::Frame {
                record,
                time: frame,
                looping: false,
            };
            if let Some(pose) = poses.pose(at, p.body(), place) {
                let state = poses.state(record, frame as i32);
                let right = pose.joints[crate::pose::RIGHT_HAND].position;
                let left = pose.joints[crate::pose::LEFT_HAND].position;
                hand = match state & 0xc {
                    8 => right,
                    4 => left,
                    _ => std::array::from_fn(|k| (right[k] + left[k]) * 0.5),
                };
            }
        }
        let v = velocity;
        let g = crate::jump::gravity();
        [
            hand[0] + flight * v[0],
            hand[1] + flight * v[1] + g * 0.5 * flight * flight,
            hand[2] + flight * v[2],
        ]
    }
}
