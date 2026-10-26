//! What the AI reads about the court: the game's per-frame tables
//! (`FUN_0013f748` and its neighbours) and the actor-state questions the
//! behaviours ask, answered from the world.
//!
//! Angles are the original's 10-bit stick angles: angle `a` moves along
//! `(-sin a, -cos a)` (`FUN_00167d88`), 1024 to the turn; distances are on
//! the floor.

use super::Ctx;
use crate::{Action, BallState, input::native_cos_1024, input::native_sin_1024};

/// The original's `x · 162.97466 + 0.5` (radians to 10-bit units),
/// converted by `cvt.w.s` (truncation), masked to the turn.
pub(super) fn units(radians: f32) -> i32 {
    ((radians * 162.974_66 + 0.5) as i32) & 0x3ff
}

/// The stick angle that moves from `from` toward `to` (`FUN_0018ce78`).
pub(super) fn angle_toward(from: [f32; 2], to: [f32; 2]) -> i32 {
    units(crate::math::atan2(from[0] - to[0], from[1] - to[1]))
}

/// The signed difference `a - b` wrapped to -512..=512 (`FUN_0018cd78`).
pub(super) fn turn(a: i32, b: i32) -> i32 {
    let d = a - b;
    if d.abs() > 0x200 {
        if d < 0 { d + 0x400 } else { d - 0x400 }
    } else {
        d
    }
}

/// The direction an angle moves, `(-sin a, -cos a)`.
pub(super) fn direction(a: i32) -> [f32; 2] {
    [-native_sin_1024(a), -native_cos_1024(a)]
}

pub(super) fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    crate::math::len(a[0] - b[0], a[1] - b[1])
}

impl Ctx<'_> {
    pub fn pos(&self, i: usize) -> [f32; 2] {
        self.w.players[i].position
    }

    pub fn team(&self, i: usize) -> usize {
        self.w.players[i].team.min(1)
    }

    /// The side a team attacks, -1 or 1 (`FUN_001404c0`, game+0x98):
    /// the sign of its target rim's x.
    pub fn side(&self, team: usize) -> i32 {
        let x = self.w.hoops[crate::target_hoop(team)].rim[0];
        if x < 0.0 { -1 } else { 1 }
    }

    /// The rim spot `team` attacks (`FUN_00140518` into ai+0xa0; the
    /// original's (side × 36.75, 0)).
    pub fn goal(&self, team: usize) -> [f32; 2] {
        self.w.rim_spot(crate::target_hoop(team))
    }

    /// The team on offence (game+0x110).
    pub fn offence(&self) -> usize {
        self.w.possession.min(1)
    }

    /// `FUN_0016e930` (game+0x374): the distance to the rim the offence
    /// attacks.
    pub fn hoop_dist(&self, i: usize) -> f32 {
        dist(self.pos(i), self.goal(self.offence()))
    }

    /// `FUN_0017dd30` (game+0x35c, filled by `FUN_0013fed0`): the angle
    /// from that rim toward the player, so steering along it backs away
    /// from the rim.
    pub fn hoop_angle(&self, i: usize) -> i32 {
        angle_toward(self.goal(self.offence()), self.pos(i))
    }

    /// `FUN_0017da80`: the stick angle from the player toward the
    /// offence's rim.
    pub fn angle_to_hoop(&self, i: usize) -> i32 {
        angle_toward(self.pos(i), self.goal(self.offence()))
    }

    /// game+0x1ac (`FUN_0018cde0`): the distance between two players.
    pub fn between(&self, a: usize, b: usize) -> f32 {
        dist(self.pos(a), self.pos(b))
    }

    /// game+0x2cc (`FUN_0017de28`): the angle from `a` toward `b`.
    pub fn angle_between(&self, a: usize, b: usize) -> i32 {
        angle_toward(self.pos(a), self.pos(b))
    }

    /// game+0x3d4 (`FUN_0018d160`): the distance to the ball on the floor.
    pub fn ball_dist(&self, i: usize) -> f32 {
        let b = self.w.ball.position;
        dist(self.pos(i), [b[0], b[2]])
    }

    /// The facing as a stick angle (actor+0xa8): the sim's heading faces
    /// `(sin h, cos h)`, half a turn from the stick angle that moves that
    /// way.
    pub fn facing(&self, i: usize) -> i32 {
        units(self.w.players[i].heading + std::f32::consts::PI)
    }

    /// The other players by distance, nearest first (game+0x23c: the five
    /// others in actor order sorted by [`native_sort`], `0x13fd00..0x13fd60`).
    pub fn by_distance(&self, i: usize) -> Vec<usize> {
        let mut others: Vec<usize> = (0..self.w.players.len()).filter(|&j| j != i).collect();
        native_sort(&mut others, |j| self.between(i, j));
        others
    }

    /// `FUN_0017f2e0(i, n, skip)`: the `n`th nearest opponent, skipping
    /// fallen ones when `skip` (animation flag 0x40, `FUN_0016ea70`).
    pub fn nth_opponent(&self, i: usize, n: usize, skip: bool) -> Option<usize> {
        let team = self.team(i);
        self.by_distance(i)
            .into_iter()
            .filter(|&j| self.team(j) != team && !(skip && self.fallen(j)))
            .nth(n)
    }

    /// `FUN_0017f3d8(i, n)`: the `n`th nearest teammate.
    pub fn nth_teammate(&self, i: usize, n: usize) -> Option<usize> {
        let team = self.team(i);
        self.by_distance(i)
            .into_iter()
            .filter(|&j| self.team(j) == team)
            .nth(n)
    }

    /// A team's players in slot order (`AITeam+0x50`).
    pub fn squad(&self, team: usize) -> Vec<usize> {
        (0..self.w.players.len())
            .filter(|&j| self.team(j) == team)
            .collect()
    }

    /// The player's slot within its team (actor+0x280).
    pub fn slot(&self, i: usize) -> usize {
        let team = self.team(i);
        (0..i).filter(|&j| self.team(j) == team).count()
    }

    /// `FUN_0017dfe0(i, 0)`: where the player stands around the rim its
    /// team attacks. Within 16 of it is 5 (z ≤ 0 after mirroring the court
    /// to the +x rim) or 6; elsewhere the angle about the rim in fifths of
    /// a half turn from the baseline (`FUN_0017e0c8`): 0..4, beyond the
    /// baseline 0.
    pub fn zone(&self, i: usize) -> i32 {
        self.zone_at(i, self.pos(i))
    }

    /// `FUN_0017dfe0(i, 1)` and its kind: the zone of a point, for the
    /// rim player `i`'s side attacks.
    pub fn zone_at(&self, i: usize, p: [f32; 2]) -> i32 {
        let side = self.side(self.team(i)) as f32;
        // The original multiplies the whole position by the side, turning
        // the court half a turn so the rim is at +36.75.
        let (x, z) = (p[0] * side, p[1] * side);
        if (x - 36.75) * (x - 36.75) + z * z < 256.0 {
            return if 0.0 < z { 6 } else { 5 };
        }
        self.sector_at(i, p)
    }

    /// `FUN_0017e0c8`: the angle about the rim in fifths of a half turn
    /// from the baseline, 0..4 (beyond the baseline 0). The bounds are
    /// 768.498, 102.9, 205.3, 307.7 and 410.1 truncated
    /// (`0x17e370..0x17e424`).
    pub fn sector(&self, i: usize) -> i32 {
        self.sector_at(i, self.pos(i))
    }

    fn sector_at(&self, i: usize, p: [f32; 2]) -> i32 {
        let side = self.side(self.team(i)) as f32;
        let (x, z) = (p[0] * side, p[1] * side);
        let a = units(crate::math::atan2(-(x - 36.75), -z));
        if !(102..=768).contains(&a) {
            0
        } else if a < 205 {
            1
        } else if a < 307 {
            2
        } else if a < 410 {
            3
        } else {
            4
        }
    }

    /// `FUN_00140f*`'s shot clock as the AI reads it (game vt+0x5c =
    /// `FUN_00131378`): the whole seconds left, mod 60. With the clock off
    /// the sim reads its last setting, or 24.
    pub fn shot_clock(&self) -> i32 {
        let t = match self.w.rules.shot_clock {
            Some(_) => self.w.shot_clock,
            None => 24.0,
        };
        (t as i32).rem_euclid(60)
    }

    pub fn has_ball(&self, i: usize) -> bool {
        self.w.ball.state == BallState::Held(i)
    }

    /// The player holding the ball, or the pass's receiver while a pass
    /// is in the air (`FUN_0014c3b8`).
    pub fn ball_owner(&self) -> Option<usize> {
        match self.w.ball.state {
            BallState::Held(h) => Some(h),
            BallState::Pass => self.w.ball.pass.map(|p| p.to),
            _ => None,
        }
    }

    /// Feet off the floor (actor y).
    pub fn airborne(&self, i: usize) -> bool {
        self.w.players[i].height > 0.0
    }

    // Actor state bits (+0xa4) as the sim's actions show them. The
    // original's bits come from its animation scripts; which action sets
    // which is read from the captures' launches (ball state 3 to 5: the
    // attract captures' 30 jump shots from 15 or more out carry bit 3
    // alone, their 29 dunks bit 7, their 8 alley-oop finishes bit 15 and
    // their lay-ups bits 3 and 13; the played captures' 70, 23, 6 and 3
    // alike).

    /// Bit 3 (`FUN_0016e990`): a jump shot, or a lay-up (with bit 13).
    pub fn shooting(&self, i: usize) -> bool {
        matches!(
            self.w.players[i].action,
            Action::Shooting { .. } | Action::Layup { .. } | Action::Dunking { layup: true, .. }
        )
    }

    /// Bit 7 (`FUN_0016e9d0`, the AI's `FUN_00181670`): a dunk.
    pub fn dunking(&self, i: usize) -> bool {
        matches!(
            self.w.players[i].action,
            Action::Dunking { layup: false, .. }
        )
    }

    /// Bit 13 (`FUN_0016ea40`, the AI's `FUN_00181690`): a lay-up (a dunk
    /// that turns into one too).
    pub fn laying_up(&self, i: usize) -> bool {
        matches!(
            self.w.players[i].action,
            Action::Layup { .. } | Action::Dunking { layup: true, .. }
        )
    }

    /// Bit 4 (`FUN_00154680`): throwing a pass (the pass animation, from
    /// the press through the follow-through).
    pub fn throwing(&self, i: usize) -> bool {
        matches!(self.w.players[i].action, Action::Passing { .. })
    }

    /// Bit 9 (`FUN_0016ea00`, the AI's `FUN_001816d0`): catching a pass.
    /// The catch row's script (`FUN_001611d8`) sets it at the release and
    /// clears it after its flight wait and tail: on the receiver from the
    /// release, through the flight, and for [`crate::timing::catch_hold`]
    /// updates after the catch ([`crate::World::catch`]) unless a move of
    /// his replaces the row.
    pub fn catching(&self, i: usize) -> bool {
        match self.w.ball.state {
            // A row parked for its own count sets it when its script starts.
            BallState::Pass => {
                self.w.ball.pass.is_some_and(|p| p.to == i)
                    && !self.w.catching.is_some_and(|c| c.player == i && c.parked)
            }
            _ => self.w.catch_holds(i),
        }
    }

    /// Bit 10 (`FUN_0016ea10`): a trick move.
    pub fn tricking(&self, i: usize) -> bool {
        self.w.players[i].action.juking()
    }

    /// Bit 12 (`FUN_0016e9c0`): a block jump (a jump started while the
    /// ball was held, passed, tossed, or shot and not yet on the hoop).
    pub fn blocking(&self, i: usize) -> bool {
        matches!(self.w.players[i].action, Action::Jumping { .. })
    }

    /// Bit 8 (`FUN_0016e9f0`): a rebound jump (`crate::rebound`).
    pub fn rebounding(&self, i: usize) -> bool {
        matches!(self.w.players[i].action, Action::ReboundJump { .. })
    }

    /// Animation flag 0x40 (`FUN_0016ea70`) and bit 16 (`FUN_0016e9b0`):
    /// off balance or on the floor.
    /// Bit 16 (`FUN_0016e9b0`, the AI's `FUN_00181828`): the counter
    /// stance or the take after it ([`crate::counter`]).
    pub fn countering(&self, i: usize) -> bool {
        matches!(
            self.w.players[i].action,
            Action::Stealing { counter: true, .. }
        )
    }

    pub fn fallen(&self, i: usize) -> bool {
        matches!(
            self.w.players[i].action,
            Action::Stumbling { .. } | Action::Dive { .. }
        )
    }

    /// `FUN_001817e0` → `FUN_0016eb80`: channel 0 plays a pick's clip,
    /// records 650..652 (the record a blend goes to, flag 0x200000, else
    /// the one it plays): the screener's pick ([`crate::trick::PICKS`],
    /// [`crate::trick::SCREENER`]).
    pub fn picking(&self, i: usize) -> bool {
        let Action::Stumbling {
            reaction: Some(r), ..
        } = self.w.players[i].action
        else {
            return false;
        };
        let record = match r.run.span {
            Some(s) if s.blend => s.to_record,
            _ => r.run.record,
        };
        record.is_some_and(|k| (650..=652).contains(&k))
    }

    /// Bit 15 (`FUN_0016e9e0`, the AI's `FUN_00181770`): an alley-oop's
    /// leap, to the finish.
    pub fn leaping(&self, i: usize) -> bool {
        self.w.players[i].leaping()
    }

    /// `FUN_0017dfa0`: the turbo left, 0..1.
    pub fn turbo(&self, i: usize) -> f32 {
        self.w.players[i].turbo
    }

    /// The meter full (team+0xc == 50000).
    pub fn meter_full(&self, team: usize) -> bool {
        self.w.rules.gamebreakers && self.w.teams[team].meter_full()
    }

    /// `FUN_00181938(j, 1)`: in a rebound jump, or (a computer player)
    /// asking for one this frame.
    pub fn rebounding_or_asking(&self, j: usize) -> bool {
        self.rebounding(j)
            || (!self.human_player(j)
                && self
                    .pads
                    .get(j)
                    .is_some_and(|&p| p & super::pad::bits::REBOUND != 0))
    }

    /// `FUN_0018b038`: how many of the side are up for a rebound.
    pub fn rebounders(&self, team: usize) -> usize {
        self.squad(team)
            .into_iter()
            .filter(|&j| self.rebounding_or_asking(j))
            .count()
    }

    /// ball+0x154 as the rebound code reads it, taken as how far out the
    /// shot was taken. The sim's shot does not keep it; the rewrite reads
    /// the shooter's distance from that rim when asked (its own reading).
    pub fn shot_distance(&self) -> f32 {
        match self.w.ball.shot {
            Some(s) => dist(self.pos(s.shooter), self.w.rim_spot(s.hoop)),
            None => 0.0,
        }
    }

    /// AIPhysPlayer+0xb0: who screened the defender last, if anyone.
    pub fn screener_of(&self, d: usize) -> Option<usize> {
        self.screened
            .get(d)
            .and_then(|&s| usize::try_from(s).ok())
            .filter(|&s| s < self.w.players.len())
    }

    /// AIPhysPlayer+0x60: the side a screener sets his pick for.
    pub fn pick_side(&self, j: usize) -> i32 {
        self.pick_sides.get(j).copied().unwrap_or(0)
    }

    /// `FUN_0013abf8`: the class of the stick's strength (actor+0x90):
    /// over 59 3, 40.. 2, over 15 1, else 0. The rewrite reads the
    /// player's last stick ([`crate::Input::strength`]).
    pub fn strength_class(&self, j: usize) -> i32 {
        let s = self
            .w
            .previous_inputs
            .get(j)
            .map_or(0, crate::Input::strength);
        if 0x3b < s {
            3
        } else if s < 0x28 {
            i32::from(0xf < s)
        } else {
            2
        }
    }

    /// `FUN_00181710` (`FUN_0016ec80`): the actor's channel is on the
    /// post-up's clips (records 704..707, 712, 713).
    pub fn posting_up(&self, j: usize) -> bool {
        self.w.on_post_up_clip(j)
    }

    /// `FUN_001816f0` (`FUN_0016ea20`): state bit 11, posting (the post
    /// call's `9b 0x800`, the backdown's dribble).
    pub fn posting(&self, j: usize) -> bool {
        self.w.posting(j)
    }

    /// `FUN_00181730` (`FUN_0016ebe8`): the actor's channel is on the post
    /// defence's records (716..719, 398, 399).
    pub fn post_defending(&self, j: usize) -> bool {
        matches!(self.w.channel_row(j), 716..=719 | 398 | 399)
    }

    /// The player's front behaviour is "set pick" (type 1).
    pub fn setting_pick(&self, j: usize) -> bool {
        self.fronts.get(j) == Some(&1)
    }

    /// `FUN_00180c20`: team+0x94 for the player's slot.
    pub fn open_frames(&self, j: usize) -> i32 {
        self.open[self.team(j)][self.slot(j).min(2)]
    }

    /// `FUN_0017eed8`: without the ball, moving within ±341 of the line
    /// to a holder within 10.
    pub fn closing_on_ball(&self, i: usize) -> bool {
        if self.has_ball(i) {
            return false;
        }
        let Some(h) = self.w.handler() else {
            return false;
        };
        if self.between(i, h) > 10.0 || !self.w.players[i].moving() {
            return false;
        }
        turn(self.facing(i), self.angle_between(i, h)).abs() <= 341
    }

    /// `FUN_0013c018`: with the ball's owner on the player's side, the
    /// third man (`0x3d1d20`).
    pub fn third_man(&self, i: usize) -> Option<usize> {
        let o = self.ball_owner()?;
        if self.team(o) != self.team(i) {
            return None;
        }
        const THIRD: [usize; 9] = [0, 2, 1, 2, 1, 0, 1, 0, 2];
        let k = THIRD[self.slot(i).min(2) * 3 + self.slot(o).min(2)];
        self.squad(self.team(i)).get(k).copied()
    }

    /// `FUN_0013ab38`: where a player is headed: a computer player's
    /// front behaviour's target, else where he stands.
    pub fn heading_for(&self, j: usize) -> [f32; 2] {
        self.targets
            .get(j)
            .copied()
            .flatten()
            .filter(|_| !self.human_player(j))
            .unwrap_or_else(|| self.pos(j))
    }

    /// `FUN_0017dfe0(j, 1)`: the zone of where the player is headed.
    pub fn zone_ahead(&self, j: usize) -> i32 {
        self.zone_at(j, self.heading_for(j))
    }

    /// `FUN_0017e0c8(j, 1)`: the sector of where the player is headed.
    pub fn sector_ahead(&self, j: usize) -> i32 {
        self.sector_at(j, self.heading_for(j))
    }

    /// `FUN_001d7d00(FUN_00181be8(j), k)`: a rating, 0..99, by the
    /// original's index (0 shots, 1 threes, 2 dunks, 3 rebounds, 4 power,
    /// 5 blocks, 6 steals, 7 handles, 8 speed, 9 the overall;
    /// [`crate::Ratings::native`]).
    pub fn rating(&self, j: usize, k: usize) -> i32 {
        self.w.players[j].ratings.native(k)
    }

    /// `FUN_001819a8`: the player's height in feet (attribute 8 / 12).
    pub fn height_ft(&self, j: usize) -> f32 {
        self.w.players[j].height_ft()
    }

    /// The side's player with a role (AI+0x58; `FUN_0017d640`), first by
    /// slot.
    pub fn with_role(&self, team: usize, role: u8, roles: &[u8]) -> Option<usize> {
        self.squad(team)
            .into_iter()
            .find(|&j| roles.get(j) == Some(&role))
    }

    /// `FUN_0018abd0(team, j, k)`: the `k`th (0 the lower) of the other
    /// two slots of `j`'s side (`0x3dce68`).
    pub fn other_of(&self, j: usize, k: usize) -> Option<usize> {
        const OTHERS: [usize; 6] = [1, 2, 0, 2, 0, 1];
        let s = OTHERS[self.slot(j).min(2) * 2 + k.min(1)];
        self.squad(self.team(j)).get(s).copied()
    }

    /// `FUN_0017d6c8(a, b)`: the third man of a side, by the slots of the
    /// other two (`0x3d9f60`).
    pub fn third_of(&self, a: usize, b: usize) -> Option<usize> {
        const THIRD: [usize; 9] = [0, 2, 1, 2, 1, 0, 1, 0, 2];
        let s = THIRD[self.slot(a).min(2) * 3 + self.slot(b).min(2)];
        self.squad(self.team(a)).get(s).copied()
    }

    /// `FUN_0017e0c8`'s neighbour `FUN_0017e548`: inside the court's
    /// playing area (|z| ≤ 27.5, |x| ≤ 43).
    pub fn in_court(&self, j: usize) -> bool {
        let p = self.pos(j);
        p[1].abs() <= 27.5 && p[0].abs() <= 43.0
    }

    /// `side × x`: how far toward the rim the player's side attacks.
    pub fn up_court(&self, j: usize) -> f32 {
        self.side(self.team(j)) as f32 * self.pos(j)[0]
    }

    /// `side × z`, the player's lateral place mirrored by his side.
    pub fn across(&self, j: usize) -> f32 {
        self.side(self.team(j)) as f32 * self.pos(j)[1]
    }

    /// `FUN_0017e4a0(j, 0)`: the lane a player runs in: across the court
    /// (mirrored by the side) below −8.8333 0, above 8.8333 2, else 1.
    pub fn lane(&self, j: usize) -> i32 {
        let z = self.side(self.team(j)) as f32 * self.pos(j)[1];
        if z <= -8.833_333 {
            0
        } else if z < 8.833_333 {
            1
        } else {
            2
        }
    }

    /// `FUN_0013b860`: the handler is passing: in a pass, or (a computer
    /// player) asking for one this frame or about to ("pass" and type
    /// 0x17 at the front).
    pub fn passing_now(&self, h: usize, b: &super::Brain) -> bool {
        if self.throwing(h) {
            return true;
        }
        if self.human_player(h) || !self.has_ball(h) {
            return false;
        }
        b.pad.buttons & super::pad::bits::PASS != 0 || matches!(b.front_kind(), 8 | 0x17)
    }
}

/// The game's tables' sort (`FUN_003b3868`, the C library's `qsort`, with
/// `LAB_0013f728`'s comparator `(int)(a - b)` on the distances): under
/// seven entries it is an insertion sort (`0x3b38cc`), moving an entry
/// back past each one the comparator puts after it. Distances within a
/// unit of each other compare equal, so they keep the order the table was
/// built in.
pub(crate) fn native_sort(items: &mut [usize], key: impl Fn(usize) -> f32) {
    debug_assert!(items.len() < 7);
    for m in 1..items.len() {
        let mut l = m;
        while l > 0 && (key(items[l - 1]) - key(items[l])) as i32 > 0 {
            items.swap(l - 1, l);
            l -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_move_where_the_original_moves() {
        // Toward -z is angle 0, toward -x a quarter turn (`FUN_00167d88`).
        assert_eq!(angle_toward([0.0, 0.0], [0.0, -5.0]), 0);
        assert_eq!(angle_toward([0.0, 0.0], [-5.0, 0.0]), 256);
        // -256.0 + 0.5 truncates to -255: 769, as the original computes it.
        assert_eq!(angle_toward([0.0, 0.0], [5.0, 0.0]), 769);
        let d = direction(256);
        assert!((d[0] + 1.0).abs() < 1e-6 && d[1].abs() < 1e-6);
        assert_eq!(turn(10, 1000), 34);
        assert_eq!(turn(1000, 10), -34);
    }
}
