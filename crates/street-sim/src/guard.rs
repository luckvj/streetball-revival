//! The defender's guarding gait: `FUN_00163d40`'s defensive mode (the
//! flag `DAT_003d78e0`) and the facing turn that follows it
//! (`FUN_00168ea8`), from the raw executable (`docs/TRACE.md` "The
//! defenders' gait on the original's locomotion").
//!
//! The original moves a player along his stick: the locomotion's moving
//! path (raw `0x167480..0x167dc0`) sets the velocity from the stick's angle
//! (actor+0x88, copied to the wanted facing +0xb0) at the playing gait
//! record's speed, and the facing (+0xa8) turns toward +0xb0 by at most 46
//! units an update afterwards (`FUN_00168ea8`, the actor's vtable +0x64).
//! A defender in the mode keeps moving along his stick, but his wanted
//! facing is the ball's holder, and he plays one of the five guarding
//! records (the shuffles to either side, the shuffle forward and back,
//! the jog backwards) by where the stick points from the holder.

/// `DAT_004b99c8` and `DAT_004b99cc`, both `(int)(fps × 0.6)` (raw
/// `0x178e2c..0x178e84`): the mode holds at least this many updates once
/// it has started, and stays off this long once it has ended.
pub const HOLD: i64 = 36;
/// `(int)(fps × 0.4)` (`0x167a4c`): after a jog backwards ends, the
/// guard shuffles back for this many updates before he jogs back again.
pub const JOG_REST: i64 = 24;
/// `(int)97.21083` (`0x167870`): the stick within this of the holder's
/// direction asks for the shuffle forward.
pub const FRONT: i32 = 97;
/// The holder within this distance (`FUN_0018d0c0`, `0x167384`).
pub const REACH: f32 = 15.0;
/// `DAT_003d78e4`: the guard within this many units of the holder's angle
/// about the rim (`FUN_00170840`).
pub const ANGLE: i32 = 100;
/// The holder's fold below this (`FUN_0018d290`): he does not face away
/// from the rim he attacks.
pub const FOLD: i32 = 0x1e4;
/// `FUN_00168ea8`'s turn of the facing toward the wanted facing, units
/// an update (0x2e; 34, 23 and 11 for clips and moves the walk never
/// plays).
pub const TURN: i32 = 46;
/// `FUN_00168ea8`'s turn for a posting holder (`0x41ba0b3f`, 23.26).
pub const POST_TURN: i32 = 23;
/// Its turn on the post defence's rows within 57 of the wanted facing
/// (`0x413e0b3f`, 11.88).
pub const STANCE_TURN: i32 = 11;

/// The guarding records' flags (`0x4b8790`'s records 0..6).
pub mod flags {
    /// Shuffle to the side, stick 97..415 units round from the holder.
    pub const SIDE_A: u32 = 0x2008;
    /// Shuffle to the other side, 609..927.
    pub const SIDE_B: u32 = 0x2010;
    /// Shuffle forward, the stick at the holder.
    pub const FRONT: u32 = 0x2020;
    /// Shuffle back, the stick away from him.
    pub const BACK: u32 = 0x2040;
    /// Jog backwards, the stick away from a holder who moves.
    pub const JOG_BACK: u32 = 0x2080;
    /// Every guarding record has it.
    pub const GUARD: u32 = 0x2000;
}

/// A player's mode state (actor+0x2e0, +0x2e4, +0x334, +0x338) and the
/// facings of his last updates (+0x2e8.., newest first: the seven
/// `FUN_00168a10` reads).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guard {
    /// The update the mode last started, or -1 (+0x2e4).
    pub entered: i64,
    /// The update the mode last ended, or -1 (+0x2e0).
    pub left: i64,
    /// The last update he jogged backwards (+0x334).
    pub jog_back: i64,
    /// The last update the locomotion ran without the jog backwards
    /// (+0x338).
    pub other: i64,
    /// His facing words after each of the last seven turns.
    pub history: [i32; 7],
}

impl Default for Guard {
    fn default() -> Self {
        Self {
            entered: -1,
            left: -1,
            jog_back: 0,
            other: 0,
            history: [0; 7],
        }
    }
}

/// What the locomotion asks of a guard this update: the record's flags and
/// the facing word toward the holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stance {
    pub flags: u32,
    pub toward: i32,
}

/// The mode's gates this update (`0x167220..0x1673f0`), from what the
/// caller measured: `eligible` the defending side's player in a phase that
/// is not a restart, no turbo held, someone holding the ball; `close` the
/// holder within [`REACH`], facing his rim, the guard in his own half
/// between the holder and that rim (`FUN_00170840(holder, guard, 100)`).
/// Updates the starts and ends and says whether the mode is on.
pub fn gate(g: &mut Guard, now: i64, eligible: bool, close: bool) -> bool {
    let on = if !eligible || (g.left != -1 && now - g.left < HOLD) {
        false
    } else if g.entered != -1 && now - g.entered < HOLD {
        true
    } else {
        close
    };
    if on {
        g.left = -1;
        if g.entered == -1 {
            g.entered = now;
        }
    } else {
        g.entered = -1;
        if g.left == -1 {
            g.left = now;
        }
    }
    on
}

/// The record the mode asks for (`0x16786c..0x167b38`) with the stick at
/// `stick` and the holder at `toward` (both words): `None` when the stick
/// points at a moving holder after the mode's first [`HOLD`] updates, which
/// ends the mode and plays the ordinary gait.
pub fn record(g: &Guard, now: i64, stick: i32, toward: i32, holder_still: bool) -> Option<u32> {
    let rel = (stick - toward) & 0x3ff;
    if !(FRONT < rel && rel < 0x400 - FRONT) {
        return (holder_still || now - g.entered < HOLD).then_some(flags::FRONT);
    }
    Some(if rel < 0x200 - FRONT {
        flags::SIDE_A
    } else if rel <= 0x200 + FRONT {
        // A jog backwards ended within `JOG_REST`: shuffle back.
        if holder_still || (g.jog_back < g.other && g.other - g.jog_back < JOG_REST) {
            flags::BACK
        } else {
            flags::JOG_BACK
        }
    } else {
        flags::SIDE_B
    })
}

impl crate::World {
    /// `FUN_00163d40`'s mode for player `i` this update, before his step:
    /// his stick out, a guard faces the holder on a guarding record
    /// (`None` for the ordinary gait). Updates his [`Guard`]; `start` is
    /// where everyone stood when the update began (the game's tables).
    pub(crate) fn guard_stance(
        &mut self,
        i: usize,
        input: &crate::Input,
        start: &[[f32; 2]],
    ) -> Option<Stance> {
        use crate::game_phase::phase;
        let now = self.tick as i64;
        let p = &self.players[i];
        if p.action != crate::Action::None {
            return None;
        }
        let [sx, sz] = input.stick();
        if !input.moves() {
            // The stand path forgets the mode (`0x168170`).
            let g = &mut self.players[i].guard;
            g.entered = -1;
            g.left = -1;
            g.other = now;
            return None;
        }
        let team = p.team;
        let holder = self.handler().filter(|&h| self.players[h].team != team);
        let ph = self.game_phase.phase;
        // Actor+0x194 is 2 (the side without the ball, `FUN_00156f48`),
        // not a restart, no turbo bit held, the ball held.
        let eligible = holder.is_some()
            && team != self.possession
            && !matches!(ph, phase::PREGAME | phase::TIP | phase::RESTART)
            && !input.turbo();
        let close = holder.is_some_and(|h| self.guards_him(i, h, start));
        let on = gate(&mut self.players[i].guard, now, eligible, close);
        let Some(h) = holder.filter(|_| on) else {
            self.players[i].guard.other = now;
            return None;
        };
        let toward = angle(start[i], start[h]);
        let stick = crate::timing::facing_word(crate::math::atan2(sx, sz));
        let still = self.holder_velocity(h) == [0.0; 2];
        let g = &mut self.players[i].guard;
        match record(g, now, stick, toward, still) {
            None => {
                // The stick at a moving holder after the mode's first
                // updates: it ends (`0x167968`).
                g.entered = -1;
                g.left = now;
                g.other = now;
                None
            }
            Some(flags) => {
                if flags == flags::JOG_BACK {
                    g.jog_back = now;
                } else {
                    g.other = now;
                }
                let toward = if g.entered == now {
                    toward
                } else {
                    steady(&g.history, toward)
                };
                Some(Stance { flags, toward })
            }
        }
    }

    /// The mode's places (`0x167220..0x1673f0`): the holder within
    /// [`REACH`] and not facing away from the rim he attacks
    /// (`FUN_0018d290`'s fold from his wanted facing), the guard in his own
    /// half and between him and that rim (`FUN_00170840(holder, guard,
    /// 100)`).
    fn guards_him(&self, i: usize, h: usize, start: &[[f32; 2]]) -> bool {
        placed(
            start[i],
            start[h],
            crate::timing::facing_word(self.players[h].wanted),
            [
                crate::restart::side(self.players[h].team),
                crate::restart::side(self.players[i].team),
                crate::restart::side(self.possession),
            ],
        )
    }

    /// The holder's velocity as the original keeps it (actor+0x40): a
    /// trick's blend coasts on the press's.
    fn holder_velocity(&self, h: usize) -> [f32; 2] {
        let p = &self.players[h];
        match p.action {
            crate::Action::Trick { play, .. } => play.drift,
            _ => p.velocity(),
        }
    }
}

/// The mode's places (`0x167220..0x1673f0`) for a guard at `me` and the
/// holder at `him` with his wanted facing `word`; `sides` the side bytes
/// (+1 attacks +x) of the holder's side, the guard's and the possession's.
/// The holder within [`REACH`] and not facing away from the rim he attacks
/// (`FUN_0018d290`'s fold under [`FOLD`]), the guard in his own half and
/// between the holder and the offence's rim (`FUN_00170840(holder, guard,
/// 100)`: within [`ANGLE`] of the holder's angle about it and no farther
/// from it, game+0x374).
pub fn placed(me: [f32; 2], him: [f32; 2], word: i32, sides: [f32; 3]) -> bool {
    let [holder, own, offence] = sides;
    let rim = [holder * 36.75, 0.0];
    let a = (word - units(him[0] - rim[0], him[1] - rim[1])) & 0x3ff;
    let fold = if a > 0x200 { 0x400 - a } else { a };
    if fold >= FOLD || crate::math::len(me[0] - him[0], me[1] - him[1]) > REACH {
        return false;
    }
    // His own half: away from the rim his side attacks.
    if (me[0] >= 0.0) == (own > 0.0) {
        return false;
    }
    let goal = [offence * 36.75, 0.0];
    let about = |q: [f32; 2]| units(q[0] - goal[0], q[1] - goal[1]);
    let to_goal = |q: [f32; 2]| crate::math::len(q[0] - goal[0], q[1] - goal[1]);
    turn(about(him), about(me)).abs() <= ANGLE && to_goal(me) <= to_goal(him)
}

/// The word that moves `from` toward `to` (`FUN_0016f508`, game+0x2cc).
pub fn toward(from: [f32; 2], to: [f32; 2]) -> i32 {
    angle(from, to)
}

/// The word of `(dx, dz)` (`FUN_0018ce78`'s `atan2` form, truncated).
fn units(dx: f32, dz: f32) -> i32 {
    crate::timing::angle_units(crate::math::atan2(dx, dz))
}

/// The word that moves `from` toward `to` (`FUN_0016f508`, game+0x2cc).
fn angle(from: [f32; 2], to: [f32; 2]) -> i32 {
    units(from[0] - to[0], from[1] - to[1])
}

/// `FUN_00168a10`: the facing toward the holder, unless it is a small turn
/// (under 58 units) against a facing that has changed direction more than
/// once over the last seven updates, which keeps the facing.
pub fn steady(history: &[i32; 7], toward: i32) -> i32 {
    let d = turn(toward, history[0]);
    if d == 0 || d.abs() >= 0x3a {
        return toward;
    }
    let mut sign = d.signum();
    let mut changes = 0;
    for k in 1..7 {
        let s = turn(history[k - 1], history[k]).signum();
        if s != sign {
            changes += 1;
            sign = s;
        }
    }
    if changes > 1 { history[0] } else { toward }
}

/// `FUN_0018cd78`: `a - b` wrapped to -512..=512.
pub fn turn(a: i32, b: i32) -> i32 {
    let d = a - b;
    if d.abs() > 0x200 {
        if d < 0 { d + 0x400 } else { d - 0x400 }
    } else {
        d
    }
}

/// `FUN_00168ea8`'s step: `from` turned toward `to` by at most `rate`.
pub fn turn_toward(from: i32, to: i32, rate: i32) -> i32 {
    let d = turn(to, from);
    if d.abs() > rate {
        (from + rate * d.signum()) & 0x3ff
    } else {
        to & 0x3ff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_holds_and_rests_for_its_updates() {
        let mut g = Guard::default();
        // Off from the first update: the rest starts.
        assert!(!gate(&mut g, 100, true, false));
        assert_eq!(g.left, 100);
        // Close, but within the rest.
        assert!(!gate(&mut g, 135, true, true));
        // Past it: on, and held whatever the gates say.
        assert!(gate(&mut g, 136, true, true));
        assert_eq!((g.entered, g.left), (136, -1));
        assert!(gate(&mut g, 171, true, false));
        assert!(!gate(&mut g, 172, true, false));
        // Not eligible (a turbo held): off at once.
        let mut g = Guard::default();
        assert!(gate(&mut g, 10, true, true));
        assert!(!gate(&mut g, 11, false, true));
    }

    #[test]
    fn the_stick_about_the_holder_picks_the_record() {
        let g = Guard {
            entered: 0,
            ..Guard::default()
        };
        let at = 300;
        // At him: forward while he stands or the mode is young.
        assert_eq!(record(&g, 100, at + 20, at, true), Some(flags::FRONT));
        assert_eq!(record(&g, 10, at - 20, at, false), Some(flags::FRONT));
        assert_eq!(record(&g, 100, at, at, false), None);
        assert_eq!(record(&g, 100, at + 98, at, false), Some(flags::SIDE_A));
        assert_eq!(record(&g, 100, at + 414, at, false), Some(flags::SIDE_A));
        assert_eq!(record(&g, 100, at + 512, at, true), Some(flags::BACK));
        assert_eq!(record(&g, 100, at + 512, at, false), Some(flags::JOG_BACK));
        assert_eq!(record(&g, 100, at + 610, at, false), Some(flags::SIDE_B));
        assert_eq!(record(&g, 100, at + 926, at, false), Some(flags::SIDE_B));
        // A jog backwards that ended 10 updates ago: shuffle back.
        let rested = Guard {
            jog_back: 90,
            other: 99,
            ..g
        };
        assert_eq!(record(&rested, 100, at + 512, at, false), Some(flags::BACK));
        let long = Guard {
            jog_back: 60,
            other: 99,
            ..g
        };
        assert_eq!(
            record(&long, 100, at + 512, at, false),
            Some(flags::JOG_BACK)
        );
    }

    #[test]
    fn a_wavering_facing_holds_for_small_turns() {
        // Turning steadily one way: the turn is taken.
        let steady_turn = [100, 90, 80, 70, 60, 50, 40];
        assert_eq!(steady(&steady_turn, 120), 120);
        // Wavering (the sign changes twice): small turns keep the facing,
        // big ones are taken.
        let waver = [100, 110, 100, 110, 100, 110, 100];
        assert_eq!(steady(&waver, 120), 100);
        assert_eq!(steady(&waver, 200), 200);
        // Across the wrap: 44 to go is reached; 144 back is turned 46.
        assert_eq!(turn_toward(1000, 20, 46), 20);
        assert_eq!(turn_toward(20, 900, 46), 998);
        assert_eq!(turn_toward(20, 30, 46), 30);
    }
}
