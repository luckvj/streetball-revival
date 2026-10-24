//! The dive for a loose ball on its script. Traced raw 2026-10-17
//! (docs/TRACE.md "The alley-oop's leap and the dive on their scripts").
//!
//! The press is the pad's 0x1000000: a person's when he presses a turbo
//! button again within `trunc(fps / 6)` updates of letting go of the same
//! one (`FUN_001308e8` against the release time `FUN_00130a28` keeps, per
//! button; [`World::dive_tap`]), the computer's from "loose ball"
//! (`FUN_0010e0a0`, raw `0x10e504..0x10e7dc`, [`crate::ai`]). Both sides'
//! updates take it (`FUN_0015a060` raw `0x15b390..0x15b494` for the side
//! with the ball, `FUN_0015da28` `0x15de6c..0x15df5c` for the other): not
//! in a jump shot, a dunk, a rebound or block jump, a catch row or a leap
//! (state bits 0x9388), the ball loose (state 1), on the floor, and not in
//! the game's phase 6. The near script when the ball 40 updates on
//! (`FUN_001537d0(ball, fps × 2 / 3)`) lies within his reach on the floor
//! (`6.5 ×` his height scale on the side with the ball, his height in feet
//! (actor+0x260) on the other), else the far one ([`DIVE_SCRIPTS`]).
//!
//! The scripts set no state bits (`9b 0`): callback 5 (the wanted facing to
//! the ball), a blend of six to `diveforball_notrans` (71) or
//! `DIVEFORBALL_SUCCESSFUL_03_8_lowcomp` (72 from frame 7), `a1 85` (the
//! hop, [`dive_hop`]), the landing, the far one's slide (`a0 0.025`), the
//! getting up, callbacks 7 and 1. Each update of a dive (`FUN_0016eab0`:
//! channel 0 on record 71 or 72) his side's update tries his catch first
//! ([`World::dive_catch`]).

use crate::timing::{self, DIVE_SCRIPTS};
use crate::{Action, BallState, Phase, World, distance};

/// The press's bit on a computer's pad (`0x10e780`), [`crate::button::DIVE`]
/// in the sim's input.
pub const PAD_BIT: u32 = 0x100_0000;

/// `trunc(fps × 0.16667)` (`0x3e2aaaad`): the updates within which a turbo
/// pressed again after its release is a dive (`FUN_001308e8`).
pub fn tap_window() -> u64 {
    (crate::TICK_HZ as f32 * f32::from_bits(0x3e2a_aaad)) as u64
}

/// `a1 85 a v w` (`FUN_00194d58`, raw `0x194f0c..0x195378`, then
/// `0x195718..0x195784`): `w` (`rise`) and `v` (`speed`) scaled to the
/// frame rate; `t` the updates a rise of `w` from his height plus the
/// model's root (`root`) takes to come down to the ball's catch height (its
/// diameter and 1 above the floor), `(−w − √|w² − 2 g h|) / g`; the ball
/// then (its place plus `t` times its velocity, `ball` and `velocity` in
/// units an update); his wanted facing turned to it; the speed held to the
/// distance to it less his reach (his height scale × 6.5, × 0.4 on the far
/// clip (record 71), else × 0.1) over `t`. Returns the velocity and the
/// facing word.
#[allow(clippy::too_many_arguments)]
pub fn dive_hop(
    speed: f32,
    rise: f32,
    at: [f32; 3],
    root: f32,
    ball: [f32; 3],
    velocity: [f32; 3],
    radius: f32,
    scale: f32,
    far: bool,
) -> ([f32; 3], i32) {
    let fps = crate::TICK_HZ as f32;
    let w = rise * (60.0 / fps);
    let mut v = speed * (60.0 / fps);
    let g = crate::jump::gravity();
    let h = at[1] + root - (radius + radius + 1.0);
    // The EE's `sqrt.s` of a negative takes its magnitude.
    let t = (-w - (w * w - 2.0 * g * h).abs().sqrt()) / g;
    let to = [ball[0] + t * velocity[0], ball[2] + t * velocity[2]];
    let word = timing::angle_units(crate::math::atan2(at[0] - to[0], at[2] - to[1]));
    let d = distance([at[0], at[2]], to);
    let reach = scale
        * 6.5
        * if far {
            f32::from_bits(0x3ecc_cccd)
        } else {
            f32::from_bits(0x3dcc_cccd)
        };
    let held = (d - reach) / t;
    if held < v {
        v = held;
    }
    (
        [
            -v * crate::input::native_sin_1024(word),
            w,
            -v * crate::input::native_cos_1024(word),
        ],
        word,
    )
}

impl World {
    /// A person's dive press (`FUN_001308e8`, `FUN_00130a28`): a turbo
    /// button pressed within [`tap_window`] updates of its own release.
    /// Keeps each button's release; true on the press that dives.
    pub(crate) fn dive_tap(&mut self, i: usize, pressed: u16, released: u16) -> bool {
        let turbos = [
            crate::button::L1,
            crate::button::R1,
            crate::button::L2,
            crate::button::R2,
        ];
        let mut dive = false;
        for (k, b) in turbos.into_iter().enumerate() {
            if pressed & b != 0 {
                let last = self.turbo_releases[i][k];
                if self
                    .tick
                    .checked_sub(last)
                    .is_some_and(|d| d < tap_window())
                {
                    dive = true;
                }
            }
            if released & b != 0 {
                self.turbo_releases[i][k] = self.tick;
            }
        }
        dive
    }

    /// The dive press for `i` ([`crate`]'s module doc): true when he dives.
    pub(crate) fn dive_press(&mut self, i: usize) -> bool {
        let p = &self.players[i];
        if crate::block::holder_bits(&p.action) & 0x9388 != 0
            || self.native_ball_state() != crate::game_phase::ball::LOOSE
            || p.height != 0.0
            || self.game_phase.phase == 6
        {
            return false;
        }
        let ahead = crate::rebound::ball_path(self, (crate::TICK_HZ * 2 / 3) as usize)
            .last()
            .map_or(self.ball.position, |&(b, _)| b);
        let reach = if p.team == self.possession {
            6.5 * p.body().height
        } else {
            p.height_ft()
        };
        let near = distance([ahead[0], ahead[2]], p.position) <= reach;
        let script = u8::from(near);
        self.begin_script(i, DIVE_SCRIPTS[usize::from(script)]);
        self.players[i].start_dive(script);
        self.events.push(crate::Event::Dive { player: i });
        true
    }

    /// `FUN_0015c598` (raw `0x15c598..0x15c768`), the dive's catch, each
    /// update of `i`'s dive before his other tests: the ball's last move
    /// against his limbs (`FUN_00137db8`). Both bones below 9 (the legs):
    /// a free ball is knocked away (`FUN_00137ed8` at 0.15, his cooldown
    /// started), a held one left. Else a free ball is his (his side has it,
    /// `FUN_00169138`; slot 24's dive, the dribble 0); a held one, while
    /// he is off the floor and its holder is not diving too, is his on
    /// `rand() % 100 < 11`, else he is barred for `fps / 2` updates
    /// (+0x1a8). True when the test met the ball. Without the disc's
    /// animations the rewrite's reach stands in ([`World::catcher`]).
    pub(crate) fn dive_catch(&mut self, i: usize) -> bool {
        let Some(poses) = self.poses.arc() else {
            return false;
        };
        if !matches!(self.players[i].action, Action::Dive { .. })
            || self.phase != Phase::Live
            || self.players[i].barred > 0
        {
            return false;
        }
        let Some(pose) = self.actor_pose(i) else {
            return false;
        };
        let to = self.ball.position;
        let from = std::array::from_fn(|k| to[k] - self.ball.moved[k]);
        let Some(hit) = crate::pose::sweep(
            &pose,
            from,
            to,
            self.ball.radius,
            poses.reach(self.players[i].body()),
        ) else {
            return false;
        };
        let holder = match self.ball.state {
            BallState::Held(h) => Some(h),
            _ => None,
        };
        if hit.bones.0 < 9 && hit.bones.1 < 9 {
            if holder.is_none() {
                self.body_knock(i, hit);
            }
            return true;
        }
        match holder {
            None => {
                if self.catching.is_some_and(|c| c.player == i) {
                    self.catching = None;
                }
                self.take_ball(i);
                self.events.push(crate::Event::Pickup { player: i });
            }
            Some(h) => {
                if self.players[i].height <= 0.0
                    || matches!(self.players[h].action, Action::Dive { .. })
                {
                    return true;
                }
                if self.rng.next_u32() % 100 < 11 {
                    self.take_ball(i);
                    self.events.push(crate::Event::Steal {
                        player: i,
                        from: h,
                        intercepted_pass: false,
                    });
                } else {
                    self.players[i].barred = (crate::TICK_HZ / 2) as u16;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Input, Player, Rules, button};

    #[test]
    fn the_hop_heads_for_where_the_ball_comes_down_short_of_it() {
        // A ball rolling along +z, 6 away on -x: he turns to its place t
        // updates on and goes no faster than reaching it less his reach.
        let g = crate::jump::gravity();
        let (radius, scale, root) = (0.4, 1.2, 3.0);
        let ball = [-6.0, radius, 0.0];
        let velocity = [0.0, 0.0, 0.05];
        let (v, word) = dive_hop(
            0.5,
            0.1,
            [0.0, 0.0, 0.0],
            root,
            ball,
            velocity,
            radius,
            scale,
            true,
        );
        let h = root - (2.0 * radius + 1.0);
        let t = (-0.1 - (0.01 - 2.0 * g * h).sqrt()) / g;
        let to = [ball[0], ball[2] + t * velocity[2]];
        let reach = scale * 6.5 * f32::from_bits(0x3ecc_cccd);
        let speed = ((to[0] * to[0] + to[1] * to[1]).sqrt() - reach) / t;
        assert!((v[1] - 0.1).abs() < 1e-6);
        assert!(((v[0] * v[0] + v[2] * v[2]).sqrt() - speed.min(0.5)).abs() < 1e-4);
        // Toward the ball: along -x, a little toward +z.
        assert!(v[0] < 0.0 && v[2] > 0.0, "{v:?}");
        assert_eq!(
            word,
            timing::angle_units(crate::math::atan2(-to[0], -to[1]))
        );
    }

    #[test]
    fn a_turbo_pressed_again_soon_after_its_release_dives() {
        // `FUN_001308e8`: within `trunc(fps / 6)` = 10 updates of letting
        // go of the same button.
        assert_eq!(tap_window(), 10);
        let mut w = World::new(
            crate::tests::hoops(),
            vec![Player::new(0, [0.0, 0.0], 0.0)],
            Rules::default(),
            1,
        );
        let r1 = button::R1;
        assert!(!w.dive_tap(0, r1, 0), "a first press");
        w.tick += 1;
        assert!(!w.dive_tap(0, 0, r1), "the release");
        w.tick += 9;
        assert!(w.dive_tap(0, r1, 0), "pressed 9 after its release");
        w.tick += 1;
        assert!(!w.dive_tap(0, 0, r1));
        w.tick += 10;
        assert!(!w.dive_tap(0, r1, 0), "10 after: too late");
        // Another button's release does not count.
        w.tick += 1;
        assert!(!w.dive_tap(0, 0, r1));
        w.tick += 2;
        assert!(!w.dive_tap(0, button::L1, 0));
        let _ = Input::default();
    }
}
