//! The pass's receiver on his way to the ball. Traced raw 2026-10-18
//! (docs/TRACE.md "The computer's offence").
//!
//! The pass start (`FUN_0015f930`, `0x15fd14..0x15fe4c`) keeps the
//! receiver's spot (`DAT_003d70c0`/`c4`, [`crate::passing::PassAim`]) and
//! sets `DAT_003d70b4` ([`SpotRun`]) when he gets there within the
//! window, whether or not the pass then leads him. The release
//! (`FUN_001611d8`) clears it (`0x161350`) and decides again
//! ([`World::release_point`]): a pass to the spot sets it; a led pass whose
//! straight run would leave the court (±40.5 by ±25) turns him at once
//! toward the point `FUN_001636c0` gives (along the edge), or, heading
//! straight at the edge, runs him to the spot; past both edges he stops
//! where he is. With it set no catch row is parked (`0x162420..0x162450`).
//!
//! His own update (`FUN_0015a060`, `0x15a4a8..0x15a700` while the passer
//! still holds the ball, `0x15a708..0x15ab50` while it flies) keeps a
//! moving receiver off the court's edges (`FUN_0016f668`, [`edge_turn`])
//! and, with the spot run set, stops him once he is within 1 of the spot
//! while moving (2 standing, his velocity the zero vector; the distance
//! counts his height), low enough: he faces the ball and plays
//! `catch_stand_1` (`0x44319a`) in the flight, `breathe_1` (`0x4570f5`, the
//! stand he holds until his catch row) before the release.

use crate::passing::PassAim;
use crate::timing::{self, angle_diff};
use crate::{BallState, World, distance, math};

/// `DAT_003d70b4` with the spot `DAT_003d70c0`/`c4`: the receiver runs to
/// the spot, and his update stops him there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpotRun {
    /// 0 none; 1 while he runs to [`SpotRun::spot`] (the update he gets
    /// there counts it to 2, and stopping him sets it back to 0).
    pub count: u8,
    pub spot: [f32; 2],
    /// The receiver who stopped at the spot before the release
    /// (`breathe_1`, `ab`: its stand plays until a script replaces it).
    pub waiting: Option<usize>,
    /// The run the pass start gave the receiver (`FUN_0016c970`: his
    /// requested heading +0x88 toward the spot and intensity +0x90), which
    /// steers him in place of his stick while the pass is his (the pad
    /// builder `FUN_001587d8` leaves both alone while ball+0x180 is 4 and
    /// ball+0x100 is him, `0x158adc..0x158af8`): player, word, intensity
    /// (0..64).
    pub steer: Option<(usize, i32, i32)>,
}

/// `FUN_0016c970`'s intensity for a run of `d` to the spot (`0x16cc8c..
/// 0x16ccfc`): none within 1.5, 39 within 3, 59 within 17.5, else 64.
pub fn run_intensity(d: f32) -> i32 {
    if d < 1.5 {
        0
    } else if d < 3.0 {
        0x27
    } else if d < 17.5 {
        0x3b
    } else {
        0x40
    }
}

/// `catch_stand_1`'s row of [`timing::CATCH_ROWS`], which the spot run
/// plays at the spot.
pub const STAND_ROW: usize = 0;

/// `breathe_1`'s clip (record 65) and `catch_stand_1`'s (565), which the
/// stops leave playing (`0x15a6d0`, `0x15a944`).
const BREATHE_RECORD: u16 = 0x41;
const STAND_RECORD: u16 = 0x235;

/// `FUN_0016f668(actor, &word)`: what a moving receiver's next step does at
/// the court's edges, `word` his wanted facing (+0xb0). Past +z 25 or -z
/// 25 (his place plus his velocity), heading out within 0x57 of straight
/// at the edge he stops facing it; otherwise, heading out at all, he turns
/// along it (0x100 or 0x300). Then the same for ±40.5 along x (0x300 and
/// 0x100 straight out; 0x200 and 0 along). The answer: 0 untouched, 1
/// turned (the caller runs him along the new word at his speed), 2 stopped
/// (his speed zeroed); the word is the one he wants.
pub fn edge_turn(position: [f32; 2], velocity: [f32; 2], word: i32) -> (u8, i32) {
    let [x, z] = position;
    let (nx, nz) = (x + velocity[0], z + velocity[1]);
    let mut t = 0;
    let mut w = word & 0x3ff;
    // `0x16f66c..0x16f7cc`: along z.
    if 25.0 < nz && 256 < w && w < 768 {
        if (w - 0x200).abs() < 0x57 {
            w = 0x200;
            t = 2;
        } else {
            w = if w < 0x200 { 0x100 } else { 0x300 };
            t = 1;
        }
    } else if nz < -25.0 && (0x301..0x400).contains(&w) || nz < -25.0 && w < 0x100 {
        let from_zero = (((w + 0x200) & 0x3ff) - 0x200).abs();
        if from_zero < 0x57 {
            w = 0;
            t = 2;
        } else {
            w = if w < 0x200 { 0x100 } else { 0x300 };
            t = 1;
        }
    }
    // `0x16f7d0..0x16f8f8`: along x.
    if 40.5 < nx {
        if (0x201..0x3ff).contains(&w) {
            if (w - 0x300).abs() < 0x57 {
                return (2, 0x300);
            }
            w = if w < 0x300 { 0x200 } else { 0 };
            t = 1;
        }
    } else if nx < -40.5 && 0 < w && w < 0x200 {
        if (w - 0x100).abs() < 0x57 {
            return (2, 0x100);
        }
        w = if w < 0x100 { 0 } else { 0x200 };
        t = 1;
    }
    (t, w)
}

impl PassAim {
    /// The pass start's test (`0x15fdc4`): the receiver gets to the spot
    /// within `fps / 2 - trunc(fps × 0.12)` updates.
    pub fn runs(&self) -> bool {
        let fps = crate::TICK_HZ as f32;
        let margin = (fps * f32::from_bits(0x3df5_c28f)) as i32 as f32;
        let window = (fps * 0.5) as i32 as f32;
        self.reach + margin <= window
    }
}

impl World {
    /// The pass start's spot (`0x15fd3c`) and its spot run (`0x15fdf8`):
    /// set when the receiver gets there within the window, else left as
    /// it was.
    ///
    /// `FUN_0016c970` (the pass start's `0x15fcf8`, for a receiver on the
    /// floor who is not leaping, posting or the passer) also points his
    /// run at the spot ([`SpotRun::steer`]); its gait step then
    /// (`FUN_00163d40(actor, 0, 1)`) is his next walk here.
    pub(crate) fn spot_at_press(&mut self, i: usize, to: usize, aim: &PassAim) {
        self.spot_run.spot = aim.spot;
        if aim.runs() {
            self.spot_run.count = 1;
        }
        let r = &self.players[to];
        self.spot_run.steer = (to != i && !r.leaping() && r.height <= 0.0).then(|| {
            let p = r.position;
            let word = timing::angle_units(math::atan2(p[0] - aim.spot[0], p[1] - aim.spot[1]));
            (to, word, run_intensity(distance(p, aim.spot)))
        });
    }

    /// The stick receiver `i` runs on while a pass is his
    /// ([`SpotRun::steer`]): pressed (the windup) or flying.
    pub(crate) fn receiver_stick(&mut self, i: usize) -> Option<[i8; 2]> {
        let (who, word, intensity) = self.spot_run.steer?;
        if who != i {
            return None;
        }
        let his = match self.ball.state {
            BallState::Held(_) => self.windup.is_some_and(|w| w.to == i),
            BallState::Pass => self.ball.pass.is_some_and(|p| p.to == i),
            _ => false,
        };
        if !his {
            self.spot_run.steer = None;
            return None;
        }
        let s = intensity.min(64) as f32 / 64.0;
        let d = [
            -crate::input::native_sin_1024(word) * s,
            -crate::input::native_cos_1024(word) * s,
        ];
        Some(crate::Input::moving(d).stick)
    }

    /// The receiver's velocity in units an update: his coast on a catch
    /// row ([`crate::touch::Catching::drift`]), else his walk.
    fn receiver_velocity(&self, i: usize) -> [f32; 2] {
        match self.catch_row_playing(i) {
            Some(c) => c.drift,
            None => self.players[i]
                .velocity()
                .map(|v| v / crate::TICK_HZ as f32),
        }
    }

    /// `FUN_0016f2b0`: the word from `i` to the ball.
    fn word_to_ball(&self, i: usize) -> i32 {
        let p = self.players[i].position;
        let b = self.ball.position;
        timing::angle_units(math::atan2(p[0] - b[0], p[1] - b[2]))
    }

    /// Face `word` at once (actor+0xa8 and +0xb0).
    fn face_word(&mut self, i: usize, word: i32) {
        let h = crate::restart::heading_radians(word);
        let p = &mut self.players[i];
        p.heading = h;
        p.wanted = h;
    }

    /// The release's point for a pass to a teammate on the floor
    /// (`FUN_001611d8`, `0x16189c..0x161f1c` and `0x162320..0x16236c`),
    /// and the spot run it leaves. `d` is the distance the flight was
    /// timed on (the table's, led; the passer's to the spot, else).
    pub(crate) fn release_point(&mut self, to: usize, aim: &PassAim, n: i32, d: f32) -> [f32; 2] {
        self.spot_run.count = 0;
        let pos = self.players[to].position;
        if !aim.lead {
            // `0x161ee0..0x161f1c`.
            self.spot_run.count = 1;
            if d <= 1.5 {
                self.spot_run.spot = pos;
                return pos;
            }
            return aim.spot;
        }
        let mut point = self.lead_spot(to, n);
        let v = self.players[to]
            .velocity()
            .map(|c| c / crate::TICK_HZ as f32);
        let speed = math::len(v[0], v[1]);
        let straight = [pos[0] + n as f32 * v[0], pos[1] + n as f32 * v[1]];
        let (mut x_edge, mut z_edge) = (false, false);
        if speed != 0.0 {
            x_edge = 40.5 <= straight[0].abs();
            z_edge = 25.0 <= straight[1].abs();
        }
        let mut edges = u8::from(x_edge) + u8::from(z_edge);
        let mut f23 = -1.0f32;
        let wanted = timing::facing_word(self.players[to].wanted);
        let off = |a: i32| angle_diff(a, wanted).abs() < 0x57;
        if edges == 2 {
            // `0x1619b8..0x161a00`: he stops where he is, facing the ball.
            edges = 0;
            self.spot_run.count = 1;
            self.spot_run.spot = pos;
            point = pos;
            let w = self.word_to_ball(to);
            self.face_word(to, w);
        } else {
            let spot = self.spot_run.spot;
            if x_edge {
                f23 = (spot[0].abs() - pos[0].abs()) / v[0].abs();
                if off(0x300) || off(0x100) {
                    self.spot_run.count = 1;
                    point = spot;
                }
            }
            if z_edge {
                f23 = (spot[1].abs() - pos[1].abs()) / v[1].abs();
                if off(0x200) || off(0) {
                    self.spot_run.count = 1;
                    point = spot;
                }
            }
        }
        if edges != 0 {
            // `0x161bd4..0x161ed8`: straight at the point at his speed.
            let word = timing::angle_units(math::atan2(pos[0] - point[0], pos[1] - point[1]));
            if speed != 0.0 {
                self.face_word(to, word);
                self.players[to].course = self.players[to].heading;
            }
            if distance(pos, point) <= 1.5 {
                self.spot_run.count = 1;
            }
        }
        // `0x162320..0x16236c`.
        if f23 <= 1.5 && self.spot_run.count != 0 && edges < 2 {
            point = pos;
        }
        point
    }

    /// Receiver `i`'s update while a pass to him is pressed or flies
    /// (`0x15a4a8..0x15a700`, `0x15a708..0x15ab50`), before his parked row
    /// is looked at: the edges, then the spot run's stop. True when he
    /// stopped at the spot in the flight (his `catch_stand_1` is set up for
    /// [`World::catching_start`]).
    pub(crate) fn receiver_run(&mut self, i: usize) -> bool {
        let Some(pass) = self
            .ball
            .pass
            .filter(|_| self.ball.state == BallState::Pass)
        else {
            return self.receiver_waits(i);
        };
        // A posting receiver (bit 11) keeps his post (`0x15a768`).
        if pass.to != i || pass.from == i || self.players[i].leaping() || self.posting(i) {
            return false;
        }
        if self.ball.flown_ticks() > self.ball.flight_ticks() {
            return false;
        }
        let stopped = self.edges_and_spot(i, 4.0, false);
        if !stopped {
            return false;
        }
        if self.catch_row_playing(i).is_some_and(|c| c.catching) {
            return false;
        }
        let w = self.word_to_ball(i);
        self.halt(i, w);
        self.spot_run.count = 0;
        if self.shown_record(i) == Some(STAND_RECORD) {
            return false;
        }
        // `0x15a950..0x15a96c`: `catch_stand_1` at once (parked with his
        // count at the flight left, so `World::catching_start` starts it
        // this update).
        self.start_catching(i, STAND_ROW);
        if let Some(c) = self.catching.as_mut() {
            c.parked = true;
        }
        let left = self.ball.flight_ticks() - self.ball.flown_ticks();
        self.players[i].received_flight = left.clamp(0, i32::from(u16::MAX));
        if let Some(p) = self.ball.pass.as_mut() {
            p.catch = STAND_ROW as u8;
        }
        true
    }

    /// The windup's half (`0x15a4a8..0x15a700`): the passer still holds the
    /// ball. Stopped at the spot the receiver plays `breathe_1` and stands
    /// until a catch row takes him.
    fn receiver_waits(&mut self, i: usize) -> bool {
        // A posting receiver (bit 11) keeps his post (`0x15a4ec`).
        if !self.windup.is_some_and(|w| w.to == i && w.from != i)
            || self.players[i].leaping()
            || self.posting(i)
        {
            return false;
        }
        if !self.edges_and_spot(i, 0.0, true) {
            return false;
        }
        if self.catch_row_playing(i).is_some_and(|c| c.catching) {
            return false;
        }
        let word = self.word_to_ball(i);
        self.halt(i, word);
        self.spot_run.count = 0;
        if self.shown_record(i) != Some(BREATHE_RECORD) {
            self.spot_run.waiting = Some(i);
        }
        false
    }

    /// The record channel 0 shows (`s5` in `FUN_0015a060`).
    fn shown_record(&self, i: usize) -> Option<u16> {
        self.channel(i).and_then(|c| c.shown.record)
    }

    /// The edges (`FUN_0016f668`) for a moving receiver and the spot run's
    /// count: true when he is to stop (at an edge, or at the spot). `floor`
    /// is the height below which he counts: under 4 in the flight
    /// (`0x15a884`), on the floor before the release (`0x15a618`).
    fn edges_and_spot(&mut self, i: usize, floor: f32, on_floor: bool) -> bool {
        let v = self.receiver_velocity(i);
        let moving = v != [0.0; 2];
        let mut t = 0;
        if moving {
            let p = &self.players[i];
            let (turn, word) = edge_turn(p.position, v, timing::facing_word(p.wanted));
            t = turn;
            match turn {
                1 => {
                    // `0x15a79c..0x15a7e8`: along the word at his speed.
                    let h = crate::restart::heading_radians(word);
                    let speed = math::len(v[0], v[1]);
                    let p = &mut self.players[i];
                    p.wanted = h;
                    p.course = h;
                    if let Some(c) = self
                        .catching
                        .as_mut()
                        .filter(|c| c.player == i && !c.parked)
                    {
                        c.drift = [math::sin(h) * speed, math::cos(h) * speed];
                    }
                }
                2 => {
                    let h = crate::restart::heading_radians(word);
                    let p = &mut self.players[i];
                    p.wanted = h;
                    p.speed = 0.0;
                }
                _ => {}
            }
        }
        if self.spot_run.count == 1 {
            // `0x15a5fc..0x15a610`, `0x15a86c..0x15a880`: 1, or 2 when his
            // velocity (+0x40) is the zero vector; `FUN_0018cdb0` from his
            // place, height and all, to the spot on the floor.
            let radius = if moving { 1.0 } else { 2.0 };
            let h = self.players[i].height;
            let low = if on_floor { h <= 0.0 } else { h < floor };
            let d = distance(self.players[i].position, self.spot_run.spot);
            if low && math::len(d, h) <= radius {
                self.spot_run.count += 1;
            }
        }
        t >= 2 || self.spot_run.count >= 2
    }

    /// Stopped (+0x190, the velocity and +0x90/+0x94 zeroed), wanting
    /// `word` (+0xb0).
    fn halt(&mut self, i: usize, word: i32) {
        let p = &mut self.players[i];
        p.speed = 0.0;
        p.wanted = crate::restart::heading_radians(word);
        if let Some(c) = self.catching.as_mut().filter(|c| c.player == i) {
            c.drift = [0.0; 2];
        }
    }

    /// The receiver standing at the spot in `breathe_1` until a catch row
    /// takes him: the stick no longer moves him.
    pub(crate) fn receiver_holds(&mut self, i: usize) -> bool {
        let Some(w) = self.spot_run.waiting.filter(|&w| w == i) else {
            return false;
        };
        let pass_on = match self.ball.state {
            BallState::Held(_) => self.windup.is_some_and(|wu| wu.to == w),
            BallState::Pass => self.ball.pass.is_some_and(|p| p.to == w),
            _ => false,
        };
        if !pass_on || self.catch_row_playing(i).is_some() {
            self.spot_run.waiting = None;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Player, Rules};
    use std::f32::consts::FRAC_PI_4;

    #[test]
    fn the_run_to_the_spot_eases_off_near_it() {
        // `FUN_0016c970` (`0x16cc8c..0x16ccfc`).
        assert_eq!(run_intensity(1.4), 0);
        assert_eq!(run_intensity(1.5), 0x27);
        assert_eq!(run_intensity(2.9), 0x27);
        assert_eq!(run_intensity(3.0), 0x3b);
        assert_eq!(run_intensity(17.4), 0x3b);
        assert_eq!(run_intensity(17.5), 0x40);
    }

    #[test]
    fn a_moving_receiver_turns_along_the_courts_edge_or_stops_at_it() {
        // `FUN_0016f668`: inside the court nothing changes.
        assert_eq!(edge_turn([0.0, 0.0], [0.4, 0.4], 0x80), (0, 0x80));
        // Over +z 25: straight out (0x200) he stops facing it; at a slant
        // he turns along it.
        assert_eq!(edge_turn([0.0, 24.8], [0.0, 0.5], 0x200), (2, 0x200));
        assert_eq!(edge_turn([0.0, 24.8], [0.3, 0.4], 0x180), (1, 0x100));
        assert_eq!(edge_turn([0.0, 24.8], [-0.3, 0.4], 0x280), (1, 0x300));
        // Under -z 25, the same about word 0.
        assert_eq!(edge_turn([0.0, -24.8], [0.0, -0.5], 0x3f0), (2, 0));
        assert_eq!(edge_turn([0.0, -24.8], [0.3, -0.4], 0x380), (1, 0x300));
        // Over +x 40.5 about 0x300, under -x 40.5 about 0x100.
        assert_eq!(edge_turn([40.3, 0.0], [0.5, 0.0], 0x300), (2, 0x300));
        assert_eq!(edge_turn([40.3, 0.0], [0.4, 0.3], 0x280), (1, 0x200));
        assert_eq!(edge_turn([-40.3, 0.0], [-0.5, 0.0], 0x100), (2, 0x100));
        assert_eq!(edge_turn([-40.3, 0.0], [-0.4, -0.3], 0x80), (1, 0));
        // In the corner, turned along the side straight at the end: he
        // stops.
        assert_eq!(edge_turn([40.3, 24.8], [0.5, 0.5], 0x280), (2, 0x300));
    }

    #[test]
    fn a_spot_run_needs_the_receiver_there_within_23_updates() {
        // `0x15fdc4`: `fps / 2 - trunc(fps × 0.12)`, 30 - 7 at 60.
        let aim = |reach| PassAim {
            spot: [0.0; 2],
            lead: false,
            reach,
        };
        assert!(aim(0.0).runs());
        assert!(aim(23.0).runs());
        assert!(!aim(23.5).runs());
    }

    /// Passer 0 at -6 and receiver 1 at `at`, heading `heading` at `speed`
    /// (units a second).
    fn two(at: [f32; 2], heading: f32, speed: f32) -> World {
        let mut w = World::new(
            crate::tests::hoops(),
            vec![
                Player::new(0, [-6.0, 0.0], 0.0),
                Player::new(0, at, heading),
            ],
            Rules::default(),
            1,
        );
        w.players[1].speed = speed;
        w.ball.position = [-6.0, 4.0, 0.0];
        w
    }

    #[test]
    fn a_spot_pass_goes_to_the_spot_and_runs_him_there() {
        // `0x161ee0..0x161f1c`.
        let aim = PassAim {
            spot: [14.0, 3.0],
            lead: false,
            reach: 5.0,
        };
        let mut w = two([10.0, 0.0], 0.0, 0.0);
        assert_eq!(w.release_point(1, &aim, 30, 20.0), [14.0, 3.0]);
        assert_eq!(w.spot_run.count, 1);
        // Timed on 1.5 or less: where he stands, the spot moved to him.
        let mut w = two([10.0, 0.0], 0.0, 0.0);
        assert_eq!(w.release_point(1, &aim, 30, 1.0), [10.0, 0.0]);
        assert_eq!(w.spot_run.spot, [10.0, 0.0]);
    }

    #[test]
    fn a_led_pass_that_would_carry_him_off_the_court_runs_him_to_the_spot_or_stops_him() {
        // `0x1619b8..0x161ed8`: heading for the corner, past both edges in
        // the flight, he stops where he is, facing the ball.
        let lead = |spot| PassAim {
            spot,
            lead: true,
            reach: 0.0,
        };
        let mut w = two([38.0, 22.0], FRAC_PI_4, 12.0);
        w.spot_run.spot = [30.0, 10.0];
        assert_eq!(
            w.release_point(1, &lead([30.0, 10.0]), 30, 30.0),
            [38.0, 22.0]
        );
        assert_eq!(w.spot_run.count, 1);
        let p = &w.players[1];
        let f = p.forward();
        let to = [
            w.ball.position[0] - p.position[0],
            w.ball.position[2] - p.position[1],
        ];
        let n = crate::math::len(to[0], to[1]);
        assert!((f[0] * to[0] + f[1] * to[1]) / n > 0.99, "{f:?}");
        // Straight at the side (+z), past it in the flight: to the spot,
        // facing it.
        let mut w = two([0.0, 22.0], 0.0, 12.0);
        w.spot_run.spot = [0.0, 24.0];
        assert_eq!(
            w.release_point(1, &lead([0.0, 24.0]), 30, 30.0),
            [0.0, 24.0]
        );
        assert_eq!(w.spot_run.count, 1);
        assert!(w.players[1].forward()[1] > 0.99);
        // Inside the court: led, no run.
        let mut w = two([0.0, 0.0], 0.0, 12.0);
        let at = w.release_point(1, &lead([0.0, 5.0]), 30, 30.0);
        assert_eq!(w.spot_run.count, 0);
        assert!((at[1] - 6.0).abs() < 0.01, "{at:?}");
    }
}
