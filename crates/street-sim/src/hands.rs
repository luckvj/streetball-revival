//! The hands' shapes (`FUN_001719f0`, called from the actor's update at
//! `0x1573d0`; raw `0x1719f0..0x171dd8`): each update every actor picks one
//! of six poses for each hand, which his model draws (actor vt+0x54,
//! `FUN_0017c5a8`: actor+0xb8 the right, +0xbc the left, handed to the
//! model at +0x28c). The poses are the body models' hand groups in their
//! order: right 0 `ARfist`, 1 `ARfthru`, 2 `ARpoint`, 3 `ARrelax`, 4
//! `ARrhang`, 5 `ARstress`; left 6..11 the same `AL` groups.
//!
//! The function also walks a per-row table at `0x3e91a8` (4 rows of frame
//! ranges whose flags pick poses, `0x171c60..0x171d84`); on the disc that
//! address holds other data (script entries: counts and code pointers), so
//! no row ever matches and the walk changes nothing.

use crate::{Action, BallState, World, school::Court, school::bits};

/// The right hand's poses; the left's are these plus [`LEFT`].
pub const FIST: u8 = 0;
pub const FOLLOW_THROUGH: u8 = 1;
pub const POINT: u8 = 2;
pub const RELAX: u8 = 3;
pub const RIM_HANG: u8 = 4;
pub const STRESS: u8 = 5;
pub const LEFT: u8 = 6;

/// The pose groups by index (right 0..5, left 6..11).
pub const GROUPS: [&str; 12] = [
    "ARfist", "ARfthru", "ARpoint", "ARrelax", "ARrhang", "ARstress", "ALfist", "ALfthru",
    "ALpoint", "ALrelax", "ALrhang", "ALstress",
];

/// The frame flags' hand bits (channel +0x14): the right hand holds the
/// ball (4), the left (8).
const RIGHT_HAND: u32 = 4;
const LEFT_HAND: u32 = 8;

impl World {
    /// `FUN_001719f0`: player `i`'s right and left hand poses this update.
    ///
    /// Relaxed, unless: in a jump shot or lay-up (bit 3) the hand the
    /// frame names alone follows through; in a swipe, a block jump, a
    /// dive, a rebound jump, a catch or a post (bits 6, 12, 15-dive, 8, 9,
    /// 11), or with a pass flying to him, both stress; hanging on the rim
    /// (bit 14) off rows 847, 848, 921 and 922 the hand or hands the frame
    /// names hang (rows 996 and 997 hang the other); the locomotion's rows
    /// (under 40) and row 957 clench both; holding the ball below row 958
    /// relaxes the hand his dribble does not use (+0x25c); the pick stance
    /// (record 64) and the pick's clips clench both.
    pub fn hand_shapes(&self, i: usize) -> (u8, u8) {
        let (row, frame) = match self.channel(i) {
            Some(c) => (c.shown.record.unwrap_or(0), c.shown.frame),
            None => {
                let f = self.gait_frame(i);
                (f.record, f.time)
            }
        };
        let hand = self
            .poses
            .get()
            .map_or(0, |p| p.state(row, frame as i32) & (RIGHT_HAND | LEFT_HAND));
        let state = Court::bits(self, i);
        let p = &self.players[i];
        let (mut right, mut left) = (RELAX, RELAX + LEFT);
        if state & bits::SHOT != 0 && hand != RIGHT_HAND | LEFT_HAND {
            if hand == LEFT_HAND {
                left = FOLLOW_THROUGH + LEFT;
            } else if hand == RIGHT_HAND {
                right = FOLLOW_THROUGH;
            }
        }
        let catching = self.catch_row_playing(i).is_some_and(|c| c.catching);
        let receiving =
            self.ball.state == BallState::Pass && self.ball.pass.is_some_and(|q| q.to == i);
        if state & (bits::SWIPE | bits::BLOCK | bits::REBOUND) != 0
            || matches!(p.action, Action::Dive { .. })
            || catching
            || p.posting
            || receiving
        {
            right = STRESS;
            left = STRESS + LEFT;
        }
        let hanging = matches!(
            p.action,
            Action::Dunking {
                hang: Some(true),
                ..
            }
        ) && p.flight.flags & crate::jump::NO_GRAVITY != 0;
        if hanging && !matches!(row, 847 | 848 | 921 | 922) {
            match hand {
                h if h == RIGHT_HAND | LEFT_HAND => {
                    left = RIM_HANG + LEFT;
                    right = RIM_HANG;
                }
                LEFT_HAND => left = RIM_HANG + LEFT,
                RIGHT_HAND => right = RIM_HANG,
                _ => {}
            }
            if matches!(row, 996 | 997) {
                if left == RIM_HANG + LEFT {
                    right = RIM_HANG;
                    left = RELAX + LEFT;
                } else if right == RIM_HANG {
                    left = RIM_HANG + LEFT;
                    right = RELAX;
                }
            }
        }
        if row < 40 || row == 957 {
            right = FIST;
            left = FIST + LEFT;
        }
        if row < 958 && self.ball.state == BallState::Held(i) {
            if p.gait_channel.hand != 1 {
                right = RELAX;
            } else {
                left = RELAX + LEFT;
            }
        }
        // `FUN_0016eb18` (record 64), `FUN_0016eb80` (records 650..652).
        if row == 64 || matches!(row, 650..=652) {
            right = FIST;
            left = FIST + LEFT;
        }
        (right, left)
    }
}
