//! Passout identity and the backboard special's row, toss and two legs.
//! Traced raw 2026-10-18 (docs/TRACE.md "The computer's offence").
//!
//! The press keeps the passout (`FUN_00142068`: game+0x140..+0x160, the
//! board's point and kind 1; the computer's from `FUN_00183318`'s draws,
//! a person's from `FUN_00146278`'s). The pass start (`FUN_0015f930`)
//! then takes, for a pass to oneself, only the rows with flag 0x200000
//! (83..90: `passout_overshoulder`, `_close`, `_travelling`, `_spin_1`,
//! `_spin_2`, `_bounce`), banded by the way the ball goes: the passer a
//! quarter second ahead to the board and the board to his led place, at
//! least 25 off the board (`0x160610..0x16067c`). The row's script tosses
//! the ball at its callback 53 (`FUN_001741a8`, [`PASSOUT_TOSSES`]), sets
//! bit 9 and waits for the ball's flight (`8d ee F ef 00`).
//!
//! The toss aims the leg back at the passer's hand as his clip will show
//! it at the callback's frame (`DAT_003d6590`) and times both legs
//! (`0x17481c..0x174964`): the whole flight `trunc(max(d1 / S + d2 /
//! (0.75 S), 20) × 0.8696)` and the first `trunc(max(d1 / S × 0.8696, 2))`,
//! `d1` the ball to the board, `d2` the board to the hand, `S` a pass's
//! speed. The ball flies the first leg (`FUN_001488e8`, kind 1); at its
//! end (`FUN_00148378`) a point below 13.25 is a miss (the legs end and
//! the ball bounces off the board), else the second leg flies to the hand
//! in the whole flight's updates left. The passer's side is barred the
//! whole flight (`0x174b1c..`, +0x1a8), so he takes the ball at its end:
//! the third attract capture's three caught on the update the flight ends.

use crate::passing::{PASS_MIN_FLIGHT, PASS_SPEED, PassAim, PassStyle, Windup};
use crate::pose::{self, Frame};
use crate::timing::PASS_ROWS;
use crate::{Action, BallState, Pass, World, ball::Via, distance};

/// Game+0x160, retained through the return leg for FUN_00143a88's value
/// and repeat key. The head route's flight is still the rewrite's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Board = 1,
    Head = 2,
}

/// The passout kept from the press to the release (game+0x140..+0x160) and
/// the leg back once tossed (`FUN_00148378`'s second leg).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Passout {
    pub passer: usize,
    pub kind: Kind,
    /// Game+0x148: whose head the pass goes off, absent for the board.
    pub defender: Option<usize>,
    /// The special's waypoint (game+0x150).
    pub point: [f32; 3],
    /// The leg back: the hand's point and the whole flight (the ball's
    /// timer, `FUN_0014c3a8`), set at the toss and spent at the board.
    pub back: Option<([f32; 3], i32)>,
    /// Released; a self-pass holds its script through the flight's wait.
    pub tossed: bool,
    /// The updates of his script left after the flight (its last wait),
    /// counting once the ball is no longer a pass to him.
    pub tail: Option<u8>,
}

/// A passout row's toss (`83 35`, callback 53): the row of
/// [`PASS_ROWS`], its script's tick at the callback (the press its tick
/// 0), the callback's frame (its second word, the clip's frame the hand is
/// posed at), its bounce (the third word, 1) and the updates its script
/// plays after the flight's wait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassoutToss {
    pub row: u8,
    pub tick: u8,
    pub frame: u8,
    pub bounce: bool,
    pub tail: u8,
}

/// Rows 83..90 (the scripts at `0x455e3d`, `0x456031`, `0x4560ae`,
/// `0x45612b`, `0x4561a8`, `0x455eba`; `tests/script_timing_disc.rs`).
pub const PASSOUT_TOSSES: [PassoutToss; 8] = [
    // passout_overshoulder
    PassoutToss {
        row: 83,
        tick: 20,
        frame: 21,
        bounce: false,
        tail: 6,
    },
    PassoutToss {
        row: 84,
        tick: 20,
        frame: 21,
        bounce: false,
        tail: 6,
    },
    // passout_close
    PassoutToss {
        row: 85,
        tick: 14,
        frame: 15,
        bounce: false,
        tail: 2,
    },
    // passout_travelling
    PassoutToss {
        row: 86,
        tick: 32,
        frame: 24,
        bounce: false,
        tail: 2,
    },
    // passout_overshoulder
    PassoutToss {
        row: 87,
        tick: 20,
        frame: 21,
        bounce: false,
        tail: 6,
    },
    // passout_spin_1
    PassoutToss {
        row: 88,
        tick: 15,
        frame: 31,
        bounce: false,
        tail: 12,
    },
    // passout_spin_2
    PassoutToss {
        row: 89,
        tick: 21,
        frame: 34,
        bounce: false,
        tail: 15,
    },
    // passout_bounce
    PassoutToss {
        row: 90,
        tick: 24,
        frame: 54,
        bounce: true,
        tail: 12,
    },
];

/// The board leg's ratio of the speed (`0x3f5e9bd4`, 1 / 1.15).
const BOARD_RATIO: f32 = f32::from_bits(0x3f5e_9bd4);
/// The head passout's (`0x1748f0`, `0x173bd8`: kind 2's 1.0526316).
const HEAD_RATIO: f32 = f32::from_bits(0x3f86_bca2);
/// The head bone the head passout aims at and tests (`0x146ab4`,
/// `0x18337c`: bone 21).
pub const HEAD_BONE: usize = 21;

/// Below this the board's point is a miss (`0x41540000`).
pub const BOARD_LOW: f32 = 13.25;

/// The row's toss, for a passout row to oneself.
pub fn toss(row: u8) -> Option<PassoutToss> {
    PASSOUT_TOSSES.iter().copied().find(|t| t.row == row)
}

impl World {
    /// The passout rows (`0x160a04..0x160bb4`): flag 0x200000
    /// to oneself, 0x400000 to a teammate, banded by the two floor legs
    /// from the passer 15 updates ahead through the point to the catch
    /// spot (`0x160610..0x16065c`, on the press's board estimate,
    /// [`crate::timing::pass_reach`]). Only board kind +/-1 floors that
    /// distance at 25 (`0x16065c`). None of them is airborne, standing or
    /// running only, so the passer's motion keeps none out.
    pub(crate) fn passout_rows(
        &self,
        i: usize,
        to: usize,
        point: [f32; 3],
        aim: &PassAim,
    ) -> Vec<usize> {
        let at = [point[0], point[2]];
        let ratio = match self.passout.map(|po| po.kind) {
            Some(Kind::Head) => HEAD_RATIO,
            _ => BOARD_RATIO,
        };
        let v = crate::timing::passer_velocity(self, i, self.players[i].moving());
        let reach = crate::timing::pass_reach(self, i, to, aim, v, Some((at, ratio)));
        let d = distance(reach.ahead, at) + distance(at, reach.catch);
        let d = if self.passout.is_some_and(|po| po.kind == Kind::Head) {
            d
        } else {
            d.max(25.0)
        };
        PASS_ROWS
            .iter()
            .enumerate()
            .filter(|(_, r)| r.flags & if i == to { 0x200000 } else { 0x400000 } != 0)
            .filter(|(_, r)| !(r.min_dist > 0.0 && d < r.min_dist))
            .filter(|(_, r)| !(r.max_dist > 0.0 && r.max_dist < d))
            .map(|(k, _)| k)
            .collect()
    }

    /// The press of a board pass: its row drawn, its script started, the
    /// ball held until callback 53 to oneself or 38 to a teammate.
    pub(crate) fn passout_press(&mut self, i: usize, to: usize, point: [f32; 3]) {
        self.special_pass_press(i, to, point, None);
    }

    /// Both kinds use FUN_0015f930's passout rows; the release is the
    /// row's callback 53 to oneself or 38 to a teammate.
    pub(crate) fn special_pass_press(
        &mut self,
        i: usize,
        to: usize,
        point: [f32; 3],
        defender: Option<usize>,
    ) {
        // The pass start stops a grounded passout before its row filter
        // measures the passer ahead (0x15facc..0x15faec).
        if self.players[i].height == 0.0 {
            self.players[i].speed = 0.0;
        }
        self.passout = Some(Passout {
            passer: i,
            kind: if defender.is_some() {
                Kind::Head
            } else {
                Kind::Board
            },
            defender,
            point,
            back: None,
            tossed: false,
            tail: None,
        });
        let aim = if i == to {
            PassAim {
                spot: self.players[i].position,
                lead: true,
                reach: 0.0,
            }
        } else {
            self.pass_aim(i, to)
        };
        let rows = self.passout_rows(i, to, point, &aim);
        let row = if rows.is_empty() {
            crate::timing::DEFAULT_PASS_ROW
        } else {
            rows[(self.rng.next_u32() % rows.len() as u32) as usize]
        };
        let style = if defender.is_some() {
            PassStyle::Heezay
        } else {
            PassStyle::Glass
        };
        self.players[i].pass_kind = style.release().kind;
        self.begin_script(i, PASS_ROWS[row].script);
        self.players[i].action = Action::Passing {
            elapsed: 0.0,
            row: row as u8,
            tick: 0,
        };
        let left = toss(row as u8).map_or(PASS_ROWS[row].release, |t| t.tick);
        if i != to {
            self.spot_at_press(i, to, &aim);
        }
        self.windup = Some(Windup {
            from: i,
            to,
            style,
            left: left.max(1),
            row: row as u8,
            aim,
        });
        self.fresh_take = false;
        self.after_throw(i, to, style);
    }

    /// Callback 38's board route, over the floor (raw 0x161430..0x1614e4
    /// and 0x162c68..0x162d14). The catch's flight uses the passer's live
    /// place to the board, then the board to the receiver/spot at 0.75 S.
    /// The route's distance also leads a moving receiver.
    pub(crate) fn board_flight(&self, i: usize, spot: [f32; 2]) -> Option<(f32, f32)> {
        let po = self.passout.filter(|po| po.passer == i && !po.tossed)?;
        let at = [po.point[0], po.point[2]];
        let first = distance(self.players[i].position, at);
        let second = distance(at, spot);
        // Kind 2's ratio for the head (`0x1614b8..0x1614e4`).
        let ratio = match po.kind {
            Kind::Board => BOARD_RATIO,
            Kind::Head => HEAD_RATIO,
        };
        let flight =
            (first / PASS_SPEED + second / (PASS_SPEED * 0.75)).max(PASS_MIN_FLIGHT) * ratio;
        Some((first + second, flight))
    }

    /// Callback 38 queues both legs at release (0x173a40..0x173dc8).
    /// The already drawn catch supplies the whole flight and target;
    /// its 3D geometry divides that flight between the two legs.
    pub(crate) fn board_release(
        &mut self,
        from: [f32; 3],
        target: [f32; 3],
        total: i32,
        mut pass: Pass,
    ) {
        let Some(po) = self.passout.filter(|po| po.passer == pass.from) else {
            return;
        };
        // A head passout to a teammate: its point again at the release
        // and the head's ratio (callback 38, `0x173930..0x173c64`).
        let (point, ratio) = match po.defender {
            Some(d) if po.kind == Kind::Head => (self.head_point(d), HEAD_RATIO),
            _ => (po.point, BOARD_RATIO),
        };
        if po.kind == Kind::Board && BOARD_LOW < point[1] {
            self.board_look(pass.from, pass.to);
        }
        let Some(po) = self.passout.as_mut() else {
            return;
        };
        let len =
            |a: [f32; 3], b: [f32; 3]| crate::math::len3(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        let d1 = len(point, from);
        let d2 = len(target, point);
        let whole = (d1 / PASS_SPEED + d2 / (PASS_SPEED * 0.75)).max(PASS_MIN_FLIGHT);
        let first = (total as f32 * (d1 / PASS_SPEED * ratio).max(2.0) / (whole * ratio)) as i32;
        po.point = point;
        po.back = Some((target, total));
        po.tossed = true;
        pass.via = match po.defender {
            Some(defender) if po.kind == Kind::Head => Via::Head { defender },
            _ => Via::Glass {
                hoop: crate::target_hoop(self.players[pass.from].team),
            },
        };
        self.ball
            .throw(from, point, first as f32 / crate::TICK_HZ as f32, pass);
    }

    /// The passer's hand as his clip shows it at the toss's frame
    /// (`FUN_001741a8`'s `DAT_003d6590`, raw `0x174490..0x174644`): posed
    /// at that frame where he stands, moved by the clip's root change from
    /// the frame shown to it (root(F) less the model's root now, +0xe0;
    /// none while the channel blends, flag 0x200000), facing his facing
    /// word, the hand the frame holds out (8 the right, else the left) with
    /// the ball's radius off the palm (`FUN_0014da20`). Without the disc's
    /// animations the rewrite's own: 4 above his place.
    fn toss_hand(&self, i: usize, frame: u8) -> [f32; 3] {
        let p = &self.players[i];
        let own = [p.position[0], p.height + 4.0, p.position[1]];
        let (Some(poses), Some(record)) = (
            self.poses.get(),
            self.channel(i).and_then(|c| c.shown.record),
        ) else {
            return own;
        };
        let at = Frame {
            record,
            time: f32::from(frame),
            looping: false,
        };
        let word = crate::timing::facing_word(p.heading);
        let body = p.body();
        let shown = self.channel(i).map(|c| c.shown);
        let now = shown
            .and_then(|s| {
                s.blend.is_none().then(|| {
                    poses.root(Frame {
                        record,
                        time: s.frame,
                        looping: false,
                    })
                })
            })
            .flatten();
        let root = match now {
            Some(now) => {
                let f = poses.root(at).unwrap_or(now);
                [f[0] - now[0], 0.0, f[2] - now[2]]
            }
            None => [0.0; 3],
        };
        let theta = word as f32 * std::f32::consts::TAU / 1024.0;
        let yaw = [
            0.0,
            crate::math::sin(theta * 0.5),
            0.0,
            crate::math::cos(theta * 0.5),
        ];
        let off = pose::rotate(yaw, [root[0] * body.height, 0.0, root[2] * body.height]);
        let place = pose::Place {
            position: [p.position[0] + off[0], p.height, p.position[1] + off[2]],
            word,
        };
        let Some(posed) = poses.pose(at, body, place) else {
            return own;
        };
        let hand = if poses.state(record, i32::from(frame)) & 8 != 0 {
            1
        } else {
            2
        };
        pose::hand_point(
            &posed,
            i32::from(record),
            i32::from(frame),
            hand,
            body,
            Some(self.ball.radius),
        )
    }

    /// Callback 53 with a passout to oneself (`FUN_001741a8`,
    /// `0x174698..0x174b7c`): the legs timed, the first thrown at the
    /// board, the passer's side barred the whole flight.
    pub(crate) fn passout_toss(&mut self, w: Windup) {
        let Some(passout) = self.passout else {
            return;
        };
        let i = w.from;
        let frame = toss(w.row).map_or(0, |t| t.frame);
        let hand = self.toss_hand(i, frame);
        if let Some(defender) = passout.defender {
            self.throw_head(i, defender, self.ball.position, hand);
            return;
        }
        let from = self.ball.position;
        let point = passout.point;
        if BOARD_LOW < point[1] {
            self.board_look(i, i);
        }
        let len =
            |a: [f32; 3], b: [f32; 3]| crate::math::len3(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        let (d1, d2) = (len(point, from), len(hand, point));
        let whole = (d1 / PASS_SPEED + d2 / (PASS_SPEED * 0.75)).max(PASS_MIN_FLIGHT);
        let total = (whole * BOARD_RATIO) as i32;
        let first = ((d1 / PASS_SPEED * BOARD_RATIO).max(2.0)) as i32;
        self.passout = Some(Passout {
            passer: i,
            kind: Kind::Board,
            defender: None,
            point,
            back: Some((hand, total)),
            tossed: true,
            tail: None,
        });
        let hoop = crate::target_hoop(self.players[i].team);
        self.ball.throw(
            from,
            point,
            first as f32 / crate::TICK_HZ as f32,
            Pass {
                from: i,
                to: i,
                tried: u8::MAX,
                via: Via::Glass { hoop },
                catch: crate::ball::NO_CATCH,
                since: self.tick,
            },
        );
        // `0x174b1c..0x174b6c`: his side barred the whole flight (the
        // receiver who is not the passer would be free; here he is both).
        let team = self.players[i].team;
        let n = total.clamp(0, i32::from(u16::MAX)) as u16;
        for p in self.players.iter_mut().filter(|p| p.team == team) {
            p.barred = n;
        }
    }

    /// The board passout's look (callback 53, `0x1749b4..0x174ad4`, to
    /// oneself; callback 38, `0x173c70..0x173d94`, to a teammate): a board
    /// point above [`BOARD_LOW`] turns the defender of `mark` (the
    /// passer, or the receiver; `FUN_0016e8f8`) to look behind him
    /// (`react_fakepasslookbehind_l`) when he stands on the floor, not in
    /// the counter stance, down, swiping or in a block jump, between
    /// `mark` and the rim (`FUN_00170840`, 256 wide), and `r % 100` comes
    /// in at most `40 + Handles / 99 × 35` (passer `i`'s).
    fn board_look(&mut self, i: usize, mark: usize) {
        let Some(d) = self.holder_defender(mark) else {
            return;
        };
        let q = &self.players[d];
        let busy = match q.action {
            Action::Stealing { .. } | Action::Jumping { .. } => true,
            _ => self.down(d),
        };
        if q.height != 0.0 || busy || !self.in_cone(mark, d, 0) {
            return;
        }
        let handles = self.players[i].ratings.native(7) as f32;
        let r = self.rng.next_u32() % 100;
        let odds = (handles / f32::from_bits(0x42c6_0000) * 35.0) as u32 + 40;
        if r <= odds {
            self.react(d, crate::trick::BOARD_LOOK);
        }
    }

    /// The head passout's point at the release (callback 53 `0x1746c4..
    /// 0x1747b0`, callback 38 alike): the defender's head bone, out along
    /// its local x and y by half his height scale (`model+0x40`) and the
    /// ball's radius. Without the disc's animations the rewrite's own: his
    /// reach less 2.2 above his place.
    pub(crate) fn head_point(&self, d: usize) -> [f32; 3] {
        let q = &self.players[d];
        let plain = [q.position[0], q.ratings.reach - 2.2, q.position[1]];
        let Some(pose) = self.actor_pose(d) else {
            return plain;
        };
        let bone = pose.joints[HEAD_BONE];
        let v = pose::rotate(bone.rotation, [1.0, 1.0, 0.0]);
        let n = pose::length(v);
        if n <= 0.0 {
            return bone.position;
        }
        let k = (q.body().height * 0.5 + self.ball.radius) / n;
        std::array::from_fn(|c| bone.position[c] + v[c] * k)
    }

    /// The head passout's legs at the release (`FUN_00148a08` then
    /// `FUN_001488e8`): the first to [`World::head_point`] and the second
    /// to `target`, timed as the board's with the head's ratio: the whole
    /// `trunc(max(d1 / S + d2 / (0.75 S), 20) × 1.0526)`, the first
    /// `trunc(max(d1 / S × 1.0526, 2))`.
    pub(crate) fn head_legs(
        &self,
        d: usize,
        from: [f32; 3],
        target: [f32; 3],
    ) -> ([f32; 3], i32, i32) {
        let point = self.head_point(d);
        let len =
            |a: [f32; 3], b: [f32; 3]| crate::math::len3(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        let (d1, d2) = (len(point, from), len(target, point));
        let whole = (d1 / PASS_SPEED + d2 / (PASS_SPEED * 0.75)).max(PASS_MIN_FLIGHT);
        let total = (whole * HEAD_RATIO) as i32;
        let first = (d1 / PASS_SPEED * HEAD_RATIO).max(2.0) as i32;
        (point, first, total)
    }

    /// The end of a head passout's first leg (`FUN_00148378` kind 2,
    /// `0x14845c..0x148638`): the ball within the passer's `Handles × 4 /
    /// 99` of the defender's head bone and no more than its radius and
    /// 0.75 above it meets him: in the counter stance he takes it
    /// (`PASSOUT_DEFENDER_REACTION_5_R`, his dribble at 0, his face `r &
    /// 1`), else he keeps his facing as his wanted facing, reacts by `r &
    /// 1` and the leg back flies to its target in what is left of the
    /// whole flight. Missed, the legs end and the ball flies on free.
    pub(crate) fn head_leg(&mut self, pass: Pass, d: usize) {
        let back = self.passout.as_mut().and_then(|po| po.back.take());
        let b = self.ball.position;
        let bone = self
            .actor_pose(d)
            .map_or_else(|| self.head_point(d), |p| p.joints[HEAD_BONE].position);
        let handles = self.players[pass.from].ratings.native(7) as f32;
        let reach = handles * 4.0 / f32::from_bits(0x42c6_0000);
        let off = crate::math::len3(b[0] - bone[0], b[1] - bone[1], b[2] - bone[2]);
        let Some((target, total)) =
            back.filter(|_| off <= reach && b[1] - bone[1] <= self.ball.radius + 0.75)
        else {
            self.ball.drop_loose();
            return;
        };
        if matches!(
            self.players[d].action,
            Action::Stealing { counter: true, .. }
        ) {
            self.intercept(d, pass);
            self.players[d].dribble = 0;
            self.players[d].action = Action::None;
            self.react(d, crate::trick::HEAD_CATCH);
            self.rng.next_u32();
            self.face(d, 2 * crate::TICK_HZ as i32);
            return;
        }
        self.players[d].wanted = self.players[d].heading;
        let r = self.rng.next_u32() & 1;
        self.react(d, crate::trick::HEAD_HIT[usize::from(r == 0)]);
        let left = (total - self.ball.flight_ticks()).max(1);
        self.ball.throw(
            b,
            target,
            left as f32 / crate::TICK_HZ as f32,
            Pass {
                via: Via::Direct,
                ..pass
            },
        );
    }

    /// The end of a passout's first leg (`FUN_00148378`, kind 1): a point
    /// below [`BOARD_LOW`] ends the legs and the ball bounces off the
    /// board; else the leg back flies to the hand in what is left of the
    /// whole flight. True when it handled the leg.
    pub(crate) fn passout_leg(&mut self, pass: Pass) -> bool {
        let Some(Passout {
            point,
            back: Some((hand, total)),
            ..
        }) = self.passout
        else {
            return false;
        };
        if self.passout.is_none_or(|po| po.passer != pass.from) {
            return false;
        }
        let flown = self.ball.flight_ticks();
        if let Some(po) = self.passout.as_mut() {
            po.back = None;
        }
        if point[1] < BOARD_LOW {
            self.ball.drop_loose();
            return true;
        }
        let left = (total - flown).max(1);
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

    /// The passer of a passout to himself waits for its flight on his
    /// row's script with bit 9 set (`9b 0x200` after callback 53).
    pub(crate) fn passout_catching(&self, i: usize) -> bool {
        self.passout
            .is_some_and(|po| po.passer == i && po.tossed && po.tail.is_none())
            && self.ball.state == BallState::Pass
            && self.ball.pass.is_some_and(|p| p.from == i && p.to == i)
    }

    /// The passer's row after its toss, each update: it holds him through
    /// the flight and its last wait, then his gait comes back (callback
    /// 1); a press whose windup was dropped ends at once.
    pub(crate) fn passout_tick(&mut self, i: usize) {
        let Some(mut po) = self.passout.filter(|po| po.passer == i) else {
            return;
        };
        if self.windup.is_some_and(|w| w.from == i) {
            return;
        }
        let row = match self.players[i].action {
            Action::Passing { row, .. } if toss(row).is_some() => Some(row),
            _ => None,
        };
        let flying = self.ball.state == BallState::Pass
            && self.ball.pass.is_some_and(|p| p.from == i && p.to == i);
        // A teammate's callback-38 row has an ordinary follow-through,
        // but its queued second leg must survive until the ball is caught.
        if po.tossed
            && self.ball.state == BallState::Pass
            && self.ball.pass.is_some_and(|p| p.from == i && p.to != i)
        {
            return;
        }
        if !po.tossed || row.is_none() {
            self.passout = None;
            if row.is_some() && !flying {
                self.players[i].action = Action::None;
            }
            return;
        }
        if flying {
            return;
        }
        let left = po
            .tail
            .unwrap_or_else(|| row.and_then(toss).map_or(0, |t| t.tail));
        if left == 0 {
            self.passout = None;
            self.players[i].action = Action::None;
            return;
        }
        po.tail = Some(left - 1);
        self.passout = Some(po);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Input, Phase, Player, Rules};

    #[test]
    fn the_tosses_are_the_passout_rows_to_oneself() {
        // Flag 0x200000 (the pass start's filter for a pass to oneself).
        let rows: Vec<u8> = PASS_ROWS
            .iter()
            .enumerate()
            .filter(|(_, r)| r.flags & 0x200000 != 0)
            .map(|(k, _)| k as u8)
            .collect();
        let tossed: Vec<u8> = PASSOUT_TOSSES.iter().map(|t| t.row).collect();
        assert_eq!(rows, tossed);
        assert!((BOARD_RATIO - 1.0 / 1.15).abs() < 1e-6);
    }

    /// Player 0 inside the board's wedge with the ball, a defender far
    /// away; team 0 attacks +x.
    fn at_the_line(seed: u32) -> World {
        let mut w = World::new(
            crate::tests::hoops(),
            vec![
                Player::new(0, [24.0, 1.0], std::f32::consts::FRAC_PI_2),
                Player::new(1, [-20.0, 0.0], 0.0),
            ],
            Rules::default(),
            seed,
        );
        w.phase = Phase::Live;
        w.ball.hold(0);
        w
    }

    fn idle(w: &mut World) {
        w.step(&[Input::default(), Input::default()]);
    }

    #[test]
    fn a_passout_to_himself_tosses_at_its_callback_53_and_comes_back_off_the_board() {
        for seed in 0..4 {
            let mut w = at_the_line(seed);
            w.passout_press(0, 0, [37.53, 14.5, 0.0]);
            let wu = w.windup.expect("the ball in his hands");
            let t = toss(wu.row).expect("a passout row");
            assert_eq!(wu.left, t.tick);
            for k in 1..t.tick {
                assert_eq!(w.validate(), Ok(()), "update {k}");
                idle(&mut w);
                assert_eq!(w.ball.state, BallState::Held(0), "update {k}");
            }
            idle(&mut w);
            assert_eq!(w.ball.state, BallState::Pass, "tossed at {}", t.tick);
            let (_, total) = w.passout.and_then(|po| po.back).expect("the leg back");
            let first = w.ball.flight_ticks();
            assert!(2 <= first && first < total, "{first} {total}");
            assert!((20.0 * BOARD_RATIO) as i32 <= total);
            // Bit 9 while it flies; his side barred the whole flight.
            assert!(w.passout_catching(0));
            assert!(i32::from(w.players[0].barred) >= total - 1);
            // The two legs take the whole flight; he takes it at its end.
            let mut n = 0;
            while w.ball.state == BallState::Pass && n < 200 {
                // Valid, and carried by a snapshot, through both legs.
                assert_eq!(w.validate(), Ok(()), "seed {seed}");
                let back = World::from_snapshot(&w.to_snapshot()).unwrap();
                assert_eq!((back.passout, back.windup), (w.passout, w.windup));
                idle(&mut w);
                n += 1;
            }
            assert_eq!(w.ball.state, BallState::Held(0), "seed {seed}");
            assert!((total - 1..=total + 1).contains(&n), "{n} against {total}");
            // His row plays its tail out, then his gait.
            assert!(matches!(w.players[0].action, Action::Passing { .. }));
            for _ in 0..=t.tail {
                idle(&mut w);
            }
            assert!(w.passout.is_none());
        }
    }

    #[test]
    fn a_board_point_below_13_25_misses_off_the_board() {
        // `FUN_00148378`: the first leg's end below `0x41540000` (wide of
        // the rim, which would knock it loose on the way).
        let mut w = at_the_line(2);
        w.passout_press(0, 0, [37.53, 12.9, 2.5]);
        let mut n = 0;
        while w.ball.state != BallState::Pass && n < 60 {
            idle(&mut w);
            n += 1;
        }
        assert_eq!(w.ball.state, BallState::Pass);
        let mut n = 0;
        while w.ball.state == BallState::Pass && n < 60 {
            idle(&mut w);
            n += 1;
        }
        // At the board, loose, the leg back spent.
        assert_eq!(w.ball.state, BallState::Loose);
        assert!(w.ball.position[0] > 37.0, "{:?}", w.ball.position);
        assert!(w.passout.is_some_and(|po| po.back.is_none()));
    }
}
