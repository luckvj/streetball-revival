//! The alley-oop's leap on its script. Traced raw 2026-10-17 (docs/TRACE.md
//! "The alley-oop's leap and the dive on their scripts").
//!
//! A player of the side with the ball who does not hold it and presses
//! shoot with a turbo held (`FUN_0015a060`, raw `0x15b218..0x15b2f8`: the
//! actor's words +0x7c & 0x80 and +0x78 & 0x2000) leaps when he can
//! (`FUN_0016c450`, [`World::can_leap`]): his move is an alley-oop row of
//! the dunk table (`FUN_001693b8(actor, 1)`, mask 0x100000, by where the
//! ball comes from against his line to the rim), or with two turbos held
//! one of the two scripts at `0x3d70d0` ([`crate::timing::LEAP_ROWS`]). The
//! row's script is the whole leap: `9b 0x8000` (state bit 15, the leap),
//! callback 4 (the wanted facing to the rim), a blend of eight updates,
//! `9e` (the aimed jump, 80 updates on), the contact window of kind 1
//! (`a6 01 02 50`, not ported), the long wait the lob is timed to, then a
//! dunk's tail: callback 21 (the slam, when he holds the ball), callback
//! 27, `9b 0x80` (bit 15 gone) and the landing. So the leap plays as a
//! [`Action::Dunking`] on its row ([`crate::jump`] flies it), bit 15 while
//! its script has not reached its `9b 0x80` ([`leaping`]).
//!
//! The lob to him (callback 38's `FUN_00162aa8`, [`World::lob_to_leaper`])
//! is timed to arrive at least five updates before his wait ends and aimed
//! at his hand as his clip will show it then; he catches it on the bone
//! rules of the side with the ball ([`crate::touch`]) and slams it at his
//! callback 21 as an alley-oop finish (the launcher's kind 3).

use crate::timing::{self, LEAP_ROWS, MOVE_ROWS};
use crate::{Action, BallState, World, distance};

/// Whether `row` of [`crate::Action::Dunking`] is a leap's: an alley-oop
/// row of the dunk table (mask 0x100000, rows 94..98) or a two-turbo leap
/// ([`LEAP_ROWS`], rows 99 and 100).
pub fn leap_row(row: u8) -> bool {
    timing::move_row(row).is_some_and(|r| r.mask & 0x100000 != 0)
}

/// The script tick at which a leap's `9b 0x80` replaces bit 15 (the dunk's
/// tail after callbacks 21 and 27), by leap row: the replay of its steps.
pub fn leap_end(row: u8) -> Option<i32> {
    static ENDS: std::sync::OnceLock<Vec<(u8, i32)>> = std::sync::OnceLock::new();
    let ends = ENDS.get_or_init(|| {
        (0..(MOVE_ROWS.len() + LEAP_ROWS.len()) as u8)
            .filter(|&r| leap_row(r))
            .filter_map(|r| {
                let steps = crate::held::move_row_steps(r)?;
                let mut run = crate::trick::Run::new(false, 0);
                for _ in 0..600 {
                    let ran = run.step(steps, 0.0);
                    if ran.set_bits & 0x80 != 0 {
                        return Some((r, i32::from(run.tick)));
                    }
                    if run.done {
                        break;
                    }
                }
                None
            })
            .collect()
    });
    ends.iter().find(|&&(r, _)| r == row).map(|&(_, t)| t)
}

/// State bit 15 (`FUN_0016e9e0`): `action` is a leap whose script has not
/// yet set its dunk's bits (the update's script tick is `tick - 1`).
pub fn leaping(action: &Action) -> bool {
    match *action {
        Action::Dunking { row, tick, .. } if leap_row(row) => {
            leap_end(row).is_some_and(|end| i32::from(tick) - 1 < end)
        }
        _ => false,
    }
}

/// `FUN_001692f8` (raw `0x1692f8..0x1693b4`): actor+0x320 for a leap's
/// script, `trunc(fps × 0.15)` for `allydunk_2` (`0x447900`), `trunc(fps
/// × 0.12)` for the other five; a person's pass picks a leaper only while
/// his meet is that far off at least (`FUN_00146278`, `0x146894`).
pub fn lead(script: u32) -> u8 {
    let fps = crate::TICK_HZ as f32;
    let k = if script == 0x447900 {
        f32::from_bits(0x3e19_999a)
    } else {
        f32::from_bits(0x3df5_c28f)
    };
    (fps * k) as u8
}

/// `FUN_0016ec80`: the post-up's clips (records 0x2c0..0x2c3, 0x2c8,
/// 0x2c9), which bar a dunk.
fn post_up_record(record: u16) -> bool {
    (0x2c0..0x2c4).contains(&record) || (0x2c8..0x2ca).contains(&record)
}

impl World {
    /// `FUN_00170840(a, b, width)`: `b` between `a` and the rim the side
    /// with the ball attacks (game+0x110, `FUN_001404e0`), within `width`
    /// units of `a`'s angle about it and no farther from it.
    pub(crate) fn in_cone(&self, a: usize, b: usize, width: i32) -> bool {
        if a == b {
            return false;
        }
        let width = if width == 0 { 0x100 } else { width };
        let rim_x = self.rim_spot(crate::target_hoop(self.possession))[0];
        let word = |j: usize| {
            let p = self.players[j].position;
            timing::angle_units(crate::math::atan2(p[0] - rim_x, p[1])) & 0x3ff
        };
        let (ua, ub) = (word(a), word(b));
        let d = timing::angle_diff(ua, ub).abs();
        let rim = |j: usize| distance(self.players[j].position, [rim_x, 0.0]);
        d <= width && rim(b) <= rim(a)
    }

    /// `FUN_0016c500(actor, flag)` (raw `0x16c500..0x16c904`), whether a
    /// dunk is on for `i`: 2 yes, 1 out of the dunking area
    /// (`FUN_0016c908`: 22.5 across, 23.75 from the rim), else 0. Open
    /// unless the nearest opponent (the game's distance table,
    /// `FUN_00170dc0`) stands within 4.5 of him and in the cone between him
    /// and the rim (`FUN_00170840`; 200 units wide, or for a dunker or
    /// leaper `(1 − Dunks / 99) × 200 + 100`), or he is on a post-up clip
    /// (`FUN_0016ec80`); a full meter opens it within 15 when he faces the
    /// rim. On within 15 when open, moving or `flag` 0, facing the rim
    /// within 350 units (`FUN_0018d290`, his wanted facing) and not behind
    /// the board (`FUN_0016f580(actor, 0)`); else on standing within 7.5.
    /// The special games' modes (vt+0xb4 3, 5, 0x15) are not modelled.
    pub(crate) fn dunk_on(&self, i: usize, flag: bool) -> i32 {
        let p = &self.players[i];
        let rim = self.rim_spot(crate::target_hoop(p.team));
        let d = distance(p.position, rim);
        if !(p.position[1].abs() < f32::from_bits(0x41b4_0000) && d < f32::from_bits(0x41be_0000)) {
            return 1;
        }
        let moving = if crate::jump::scripted(&p.action) {
            p.flight.velocity != [0.0; 3]
        } else {
            p.moving()
        };
        let at = self.floor_positions();
        let opponent = (0..self.players.len())
            .filter(|&j| self.players[j].team != p.team)
            .fold(None, |best: Option<(usize, f32)>, j| {
                let dj = distance(at[i], at[j]);
                match best {
                    Some((_, b)) if b <= dj => best,
                    _ => Some((j, dj)),
                }
            })
            .map(|(j, _)| j);
        let (facing, _) = timing::facing_angle(p.wanted, p.position, rim);
        let dunks = p.ratings.native(2);
        let width = if crate::block::holder_bits(&p.action) & 0x8080 == 0 {
            200
        } else {
            ((1.0 - dunks as f32 / 99.0) * 200.0 + 100.0).round() as i32
        };
        let mut open = true;
        if let Some(o) = opponent
            && self.in_cone(i, o, width)
            && distance(self.players[o].position, p.position) <= 4.5
        {
            open = false;
        }
        if self
            .channel(i)
            .and_then(|c| c.shown.record)
            .is_some_and(post_up_record)
        {
            open = false;
        }
        let range = f32::from_bits(0x4170_0000);
        let side = crate::restart::side(p.team);
        let behind = crate::jump::behind_board(p.position, side);
        let faces = (0..=0x15e).contains(&facing) || ((0x400 - 0x15e)..=0x400).contains(&facing);
        if self.teams[p.team].meter_full() && d <= range && !behind && faces {
            open = true;
        }
        if open && d <= range && (moving || !flag) && faces && !behind {
            return 2;
        }
        if !moving && d <= f32::from_bits(0x40f0_0000) {
            2
        } else {
            0
        }
    }

    /// `FUN_0016c450`: `i` can leap for an alley-oop: not in a jump shot, a
    /// dunk, a hang or a leap (state bits 3, 7, 14, 15), a dunk on for him
    /// ([`World::dunk_on`] with flag 0), the ball held or passed, on the
    /// floor and within 35 of the middle along the court.
    pub fn can_leap(&self, i: usize) -> bool {
        let p = &self.players[i];
        crate::block::holder_bits(&p.action) & 0xc088 == 0
            && self.dunk_on(i, false) == 2
            && matches!(self.ball.state, BallState::Held(_) | BallState::Pass)
            && p.height == 0.0
            && p.position[0].abs() < f32::from_bits(0x420c_0000)
    }

    /// `FUN_0015a060`'s shoot press off the ball (raw `0x15b218..0x15b2f8`),
    /// for `i` of the side with the ball who does not hold it: shoot pressed
    /// with a turbo held. A person who presses rebound (Triangle) with it
    /// while the ball is a shot, on the rim or coming off it does not leap.
    /// When he can leap ([`World::can_leap`]) he does: with two turbos held
    /// one of the two scripts at `0x3d70d0` (`rand() & 1`; dunk id 24, move
    /// kind 1), else an alley-oop row ([`World::leap_row_draw`]). True when
    /// the press went to a leap. (When he cannot, a person on the floor in
    /// live play plays the call-for-it gesture `0x44c95e`, not ported.)
    pub(crate) fn leap_press(&mut self, i: usize, input: &crate::Input, turbos: u32) -> bool {
        if turbos == 0 {
            return false;
        }
        if !input.is_computer()
            && input.held(crate::button::JUMP)
            && matches!(
                self.native_ball_state(),
                crate::game_phase::ball::SHOT
                    | crate::game_phase::ball::RIM
                    | crate::game_phase::ball::REBOUND
            )
        {
            return false;
        }
        if !self.can_leap(i) {
            return false;
        }
        let row = if turbos >= 2 {
            let k = (self.rng.next_u32() & 1) as usize;
            (MOVE_ROWS.len() + k) as u8
        } else {
            self.leap_row_draw(i)
        };
        self.start_leap(i, row);
        true
    }

    /// `FUN_001693b8(actor, 1)` (raw `0x1693b8..0x169c94`), the alley-oop
    /// row: the draw for the face (`r & 1`, then `FUN_00159190`'s own), the
    /// rows of the table with mask 0x100000 whose sector bit holds where
    /// the ball comes from against his line to the rim (`(FUN_0016f260 −
    /// FUN_0016f2b0 + 0x640) & 0x3ff >> 7`: 4 the ball from the rim's side,
    /// 0 from behind him), the live hand's bits, the distance bands, then
    /// `rand() % count`. No Dunks gates and no unlocks on this path; the
    /// rows cover every sector, so the jump shot fallbacks never play.
    pub(crate) fn leap_row_draw(&mut self, i: usize) -> u8 {
        // `FUN_00159190(actor, r & 1 ? 1 : 2, fps × 2)`: the face reaction
        // draws once more for its clip.
        self.rng.next_u32();
        self.face(i, 2 * crate::TICK_HZ as i32);
        let p = &self.players[i];
        let rim = self.rim_spot(crate::target_hoop(p.team));
        let to_rim = timing::angle_units(crate::math::atan2(
            p.position[0] - rim[0],
            p.position[1] - rim[1],
        ));
        let b = self.ball.position;
        let from_ball = timing::angle_units(crate::math::atan2(
            p.position[0] - b[0],
            p.position[1] - b[2],
        ));
        let sector = (((to_rim - from_ball + 0x640) & 0x3ff) >> 7) as u32;
        let d = distance(p.position, rim);
        let hand = p.gait_channel.hand;
        let rows: Vec<usize> = MOVE_ROWS
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                let m = r.mask;
                let band = if d >= 12.5 {
                    0xc00
                } else if d > 8.0 {
                    0xa00
                } else {
                    0x900
                };
                m & 0x100000 != 0
                    && m & (1 << sector) != 0
                    && !(m & 0x40000 != 0 && hand == 1)
                    && !(m & 0x20000 != 0 && hand == 2)
                    && m & band != 0
                    && r.legend <= 0
            })
            .map(|(k, _)| k)
            .collect();
        if rows.is_empty() {
            return timing::DUNK_FALLBACK_ROW as u8;
        }
        rows[(self.rng.next_u32() % rows.len() as u32) as usize] as u8
    }

    /// `i` leaps on `row`'s script: started at once (`FUN_0019c2f8`), his
    /// velocity his own until the script's steps change it.
    pub(crate) fn start_leap(&mut self, i: usize, row: u8) {
        let Some(r) = timing::move_row(row) else {
            return;
        };
        let hoop = crate::target_hoop(self.players[i].team);
        let spot = self.rim_side(i, hoop, 1.2);
        self.begin_move(i, row);
        self.players[i].start_dunk(
            hoop,
            spot,
            crate::player::DunkStyle::Normal,
            (row, r.release, r.end),
        );
    }

    /// `FUN_00162aa8(passer)` (raw `0x162aa8..0x1631c4`), callback 38's
    /// flight and point for a pass to a leaper: `max(table / S, 20)` (the
    /// game's distance table, as [`crate::passing::flight_updates`] reads
    /// it but not truncated), raised to the span his channel has left less
    /// five (`+0x6c − +0x70 − 5`) when that is longer; his clip posed at the
    /// frame it will show then (`+0x5c + rate × flight`, wrapped by the
    /// clip's length) at his place facing his wanted facing, the hand by
    /// the frame's hand flags (8 the right, 4 the left, else the midpoint),
    /// plus his velocity times the flight and half the gravity times its
    /// square; at least 12.5 high, or 8 when the span set the flight. The
    /// flight is stored truncated (`DAT_003d65a0`). Without the disc's
    /// animations his reach above his feet stands in for the hand.
    pub(crate) fn lob_to_leaper(&self, i: usize, to: usize, start: &[[f32; 2]]) -> (i32, [f32; 3]) {
        let at = |j: usize| start.get(j).copied().unwrap_or(self.players[j].position);
        let table = distance(at(i), at(to));
        let board = self.board_flight(i, self.players[to].position);
        let mut flight = board.map_or_else(
            || (table / crate::passing::PASS_SPEED).max(crate::passing::PASS_MIN_FLIGHT),
            |(_, flight)| flight,
        );
        let run = self.move_run(to);
        let mut floor = f32::from_bits(0x4148_0000);
        // A board route bypasses the ordinary lob's span extension
        // (0x162c68..0x162d1c).
        if board.is_none()
            && let Some(span) = run.as_ref().and_then(|r| r.span)
        {
            let left = f32::from(span.length) - f32::from(span.counter) - 5.0;
            if flight < left {
                flight = left;
                floor = 8.0;
            }
        }
        let p = &self.players[to];
        let mut hand = [p.position[0], p.height + p.ratings.reach, p.position[1]];
        if let (Some(poses), Some(run)) = (self.poses.get(), run.as_ref())
            && let Some(record) = run.record
        {
            let rate = run.span.map_or(0.0, |s| s.rate);
            let mut frame = f32::from(run.int_frame.max(0)) + rate * flight;
            if let Some(n) = poses.frames(record) {
                while frame >= n as f32 {
                    frame -= n as f32;
                }
            }
            let mut place = self.place(to);
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
        let v = p.flight.velocity;
        let g = crate::jump::gravity();
        let mut point = [
            hand[0] + flight * v[0],
            hand[1] + flight * v[1] + g * 0.5 * flight * flight,
            hand[2] + flight * v[2],
        ];
        if point[1] <= floor {
            point[1] = floor;
        }
        (flight as i32, point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trick::{Run, Step};
    use crate::{Player, Rules};

    /// A row's script clock as [`crate::timing::MoveRow`] keeps it: the
    /// tick its callback 21 runs.
    fn release(steps: &[Step]) -> i32 {
        let call21 = steps.iter().position(|s| *s == Step::Call(21)).unwrap();
        let mut run = Run::new(false, 0);
        for _ in 0..600 {
            let before = run.pc;
            run.step(steps, 0.0);
            if usize::from(before) <= call21 && usize::from(run.pc) > call21 {
                return i32::from(run.tick);
            }
        }
        -1
    }

    #[test]
    fn the_leaps_are_the_alley_oop_rows_and_the_two_turbo_scripts() {
        // Rows 94..98 carry the table's 0x100000; 99 and 100 are past it.
        let leaps: Vec<u8> = (0..=u8::MAX).filter(|&r| leap_row(r)).collect();
        assert_eq!(leaps, vec![94, 95, 96, 97, 98, 99, 100]);
        for r in leaps {
            let steps = crate::held::move_row_steps(r).unwrap();
            // `9b 0x8000` first, callback 21 at the row's release, then the
            // tail's `9b 0x80` within a few updates of it.
            assert!(steps.contains(&Step::Bits(0x8000)), "{r}");
            let row = timing::move_row(r).unwrap();
            assert_eq!(release(steps), i32::from(row.release), "{r}");
            let end = leap_end(r).unwrap();
            assert!(
                (2..=3).contains(&(end - i32::from(row.release))),
                "{r}: {end}"
            );
        }
        assert!(!leap_row(50) && !leap_row(timing::NO_MOVE_ROW));
        // `FUN_001692f8`'s leads.
        assert_eq!(lead(0x447900), 9);
        assert_eq!(lead(0x447969), 7);
    }

    fn court() -> World {
        let mut w = World::new(
            crate::tests::hoops(),
            vec![
                Player::new(0, [10.0, 0.0], std::f32::consts::FRAC_PI_2),
                Player::new(0, [30.0, 4.0], std::f32::consts::FRAC_PI_2),
                Player::new(1, [-30.0, 20.0], 0.0),
            ],
            Rules::default(),
            3,
        );
        w.phase = crate::Phase::Live;
        w.ball.hold(0);
        w
    }

    #[test]
    fn a_leap_holds_bit_15_until_its_dunk_tail() {
        let mut w = court();
        w.start_leap(1, 96);
        let end = leap_end(96).unwrap();
        for k in 0..=end + 2 {
            assert_eq!(w.players[1].leaping(), k <= end, "tick {k}");
            assert_eq!(
                crate::block::holder_bits(&w.players[1].action) == 0x8000,
                k <= end
            );
            w.step(&[crate::Input::default(); 3]);
        }
    }

    #[test]
    fn who_can_leap_is_fun_0016c450() {
        let mut w = court();
        // 6.1 from the rim, standing: on (within 7.5 whatever his facing).
        w.players[1].position = [31.0, 2.0];
        assert!(w.can_leap(1));
        // Out of the area across (22.5) or from the rim (23.75): no.
        w.players[1].position = [30.0, 23.0];
        assert_eq!(w.dunk_on(1, false), 1);
        assert!(!w.can_leap(1));
        // 12 out facing away from the rim and standing: no.
        w.players[1].position = [24.75, 0.0];
        w.players[1].wanted = -std::f32::consts::FRAC_PI_2;
        assert_eq!(w.dunk_on(1, false), 0);
        // Facing it: on; the ball loose: no.
        w.players[1].wanted = std::f32::consts::FRAC_PI_2;
        assert!(w.can_leap(1));
        w.ball.drop_loose();
        assert!(!w.can_leap(1));
    }

    #[test]
    fn the_alley_oop_row_follows_where_the_ball_comes_from() {
        // The ball from the rim's side (sector 4): rows 96 and 97; from
        // straight behind him (sector 0): row 98 alone.
        let mut w = court();
        let mut ahead = std::collections::BTreeSet::new();
        let mut behind = std::collections::BTreeSet::new();
        for _ in 0..40 {
            w.ball.position = [36.75, 4.0, 0.0];
            ahead.insert(w.leap_row_draw(1));
            w.ball.position = [16.5, 4.0, 12.0];
            behind.insert(w.leap_row_draw(1));
        }
        assert_eq!(ahead.into_iter().collect::<Vec<_>>(), vec![96, 97]);
        assert_eq!(behind.into_iter().collect::<Vec<_>>(), vec![98]);
    }

    #[test]
    fn the_lob_waits_for_the_leapers_span() {
        let mut w = court();
        w.start_leap(1, 94);
        // Into the long wait (`8d ee 0a 41`, 65 updates from tick 13).
        for _ in 0..20 {
            w.step(&[crate::Input::default(); 3]);
        }
        let run = w.move_run(1).unwrap();
        let span = run.span.unwrap();
        let left = i32::from(span.length) - i32::from(span.counter) - 5;
        // From 20 away the table's flight (26) is short of the span: the
        // span sets it, and the point's floor is 8.
        let at = w.floor_positions();
        let (n, point) = w.lob_to_leaper(0, 1, &at);
        assert_eq!(n, left);
        assert!(point[1] >= 8.0);
        // From farther than the span the table's flight stands.
        w.players[0].position = [-30.0, 0.0];
        let at = w.floor_positions();
        let (n, point) = w.lob_to_leaper(0, 1, &at);
        assert_eq!(
            n,
            crate::passing::flight_updates(crate::distance(at[0], at[1]))
        );
        assert!(point[1] >= 12.5);
    }
}
