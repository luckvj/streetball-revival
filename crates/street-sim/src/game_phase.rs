//! The game's phase (game+0xf4), as the original keeps it.
//!
//! the original game's game object holds the phase of play and changes it
//! in two ways (docs/TRACE.md, "The game's phase machine"):
//!
//! - at once, in the listeners the ball's moments are posted to
//!   (`FUN_00190d90` calls every listener in turn): the ball given to a
//!   player (`FUN_0014cb28` posts class `0x3c6600`, the game listener's
//!   slot 24, `FUN_001415c0`), a shot leaving the hands (the launcher
//!   posts `0x3c6530`, slot 11, `0x141558`), a basket (slot 26,
//!   `FUN_00141818`) and the restart it starts (`FUN_001406b8`);
//! - once an update, in the game's update `FUN_0013d868`: the end of the
//!   game, the side holding the ball (`FUN_001405f0`), then the phase's
//!   own update `FUN_0013e4c8`. It runs after the players and the ball
//!   have moved (the captures show a loose ball, a catch, a launch and a
//!   crossing change the phase in the same update), and before the hoop
//!   posts a basket (the game-winning basket shows phase 9 one update
//!   later).
//!
//! [`World::step`](crate::World::step) feeds it in that order. Every one of
//! the six game classes (vtables `0x43b490`, `0x43dcf8`, `0x43e9d0`,
//! `0x440240`, `0x440ce8`, `0x441680`) shares these methods; vt+0x6c
//! (`0x143060`) returns 0 in all of them, so phase 8 is never reached in
//! Vol. 2 and a won game goes to 9.

/// game+0xf4's values.
pub mod phase {
    /// Constructed (`FUN_0013cf58`), the intro playing.
    pub const PREGAME: i32 = 0;
    /// The referee's toss (`FUN_00141fd8`).
    pub const TIP: i32 = 1;
    /// A loose ball.
    pub const LOOSE: i32 = 2;
    /// A shot or lay-up in the air and its rebound.
    pub const SHOT: i32 = 3;
    /// The ball in the possessing side's back court: bringing it up.
    pub const BACK_COURT: i32 = 4;
    /// The ball in its front court: the half-court offence.
    pub const FRONT_COURT: i32 = 5;
    /// A restart (`FUN_001406b8`).
    pub const RESTART: i32 = 6;
    /// Over by vt+0x6c, which no Vol. 2 game class returns.
    pub const OVER: i32 = 8;
    /// Won (`FUN_0013d868`): the game's target reached.
    pub const WON: i32 = 9;
}

/// The ball's state as the phase reads it (ball+0x180).
pub mod ball {
    /// Coming off the rim or the glass from a shot.
    pub const REBOUND: i32 = 0;
    /// Loose: let go, knocked away, through the net.
    pub const LOOSE: i32 = 1;
    /// Held (`FUN_0014cb28`).
    pub const HELD: i32 = 3;
    /// Passed (`FUN_0014bff8`, from the press).
    pub const PASS: i32 = 4;
    /// Shot (`FUN_0014aaf8`) or dunked.
    pub const SHOT: i32 = 5;
    /// On the rim.
    pub const RIM: i32 = 6;
    /// The referee's toss (`FUN_0014c858`).
    pub const TOSS: i32 = 7;
}

/// Phase 9's countdown (game+0x448 = `DAT_004b79fc`, written by the boot
/// reset at `0x142f84..0x142fa4` as fps × 2.5).
pub const WON_UPDATES: i32 = 150;

/// The restart's hold after a gamebreaker's basket that does not win
/// (game+0x104 = fps × 2 + `DAT_004b79f4`, which `0x142f50..0x142f60`
/// writes as fps × 2: `FUN_00141818` at `0x141a14..0x141a38`). The restart
/// cannot end while it counts (`FUN_00140840`).
pub const GAMEBREAKER_HOLD: i32 = 240;

/// How far into the restart's hold the basket's scorer is told to
/// celebrate (game+0x464 against `DAT_004b79f4`, fps × 2:
/// `0x13da28..0x13da50`).
pub const CELEBRATE_AT: i32 = 120;

/// A falling rebound below this height is a loose ball (`FUN_0013e4c8`:
/// ball state 0 and ball+0x34 < 6.0).
pub const REBOUND_FLOOR: f32 = 6.0;

/// The game object's phase and the fields its transitions keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamePhase {
    /// game+0xf4 ([`phase`]).
    pub phase: i32,
    /// game+0x110: the side the held phases belong to, -1 none yet
    /// (`FUN_0013cf58`).
    pub possession: i32,
    /// game+0x440: updates of phases 2 and 3 in a row.
    pub loose: i32,
    /// game+0x104: the restart's hold ([`GAMEBREAKER_HOLD`]).
    pub hold: i32,
    /// game+0x464: the hold's updates, counted with it and cleared where
    /// it is set ([`CELEBRATE_AT`]).
    pub since: i32,
    /// game+0x448: phase 9's countdown ([`WON_UPDATES`]).
    pub countdown: i32,
}

impl Default for GamePhase {
    /// `FUN_0013cf58`: phase 0, nobody's possession.
    fn default() -> Self {
        Self {
            phase: phase::PREGAME,
            possession: -1,
            loose: 0,
            hold: 0,
            since: 0,
            countdown: 0,
        }
    }
}

/// What one update of the phase reads of the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The ball's state ([`ball`]).
    pub ball: i32,
    /// The ball's x and height (ball+0x30, +0x34).
    pub ball_x: f32,
    pub ball_y: f32,
    /// The holder's side and x (ball+0xf4's actor, +0x284 and +0x30).
    pub holder: Option<(usize, f32)>,
    /// game+0x98 by side: 1 for the side attacking +x, -1 for -x.
    pub sides: [i32; 2],
    /// `FUN_00140840`'s six ready flags (game+0x404..0x418).
    pub ready: bool,
    /// game+0x4a4: someone has brought the ball up since the last restart.
    pub brought_up: bool,
    /// vt+0xbc: the game is won.
    pub won: bool,
    /// vt+0x164 (`FUN_0013ecd8`): the ball is settled: not passed, shot or
    /// on the rim, and its holder not in a shot, dunk, lay-up or pass.
    pub settled: bool,
    /// The script manager runs a script (game+0x424 +0x50 in 3..7).
    pub scripted: bool,
}

/// What an update changed that the rest of the game acts on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Phase 6 is over: phase 4, every player the controls' again
    /// (`FUN_00140880(0)`), and slot 28 (`0x3c6640`) posted.
    pub restart_over: bool,
    /// [`CELEBRATE_AT`] updates into the hold: the basket's scorer is
    /// told to celebrate (actor+0x1c0 and +0x1c4).
    pub celebrate: bool,
    /// Phase 9 began (`0x13dbb0..0x13dee8`: the scorer's celebration and
    /// the loser's reaction).
    pub won: bool,
    /// Phase 9's countdown ran (case 9, `0x13e890..0x13eb38`: a
    /// celebration that waited).
    pub won_update: bool,
    /// game+0x4a4 set (`FUN_0013e4c8` case 4).
    pub brought_up: bool,
    /// The ball crossed into the front court (slot 23, `0x3c65f0`).
    pub crossed: bool,
}

impl GamePhase {
    /// A game that starts with a restart (the rewrite's dead ball): phase
    /// 6 with `team` to inbound.
    pub fn restarting(team: usize) -> Self {
        let mut g = Self::default();
        g.start_restart(team);
        g
    }

    /// `FUN_00140558`: play is on (`0x3d30d0`: phases 2..5; game+0x19c,
    /// the shot clock's wait for a shot to finish, is the rewrite's never).
    pub fn live(&self) -> bool {
        (phase::LOOSE..=phase::FRONT_COURT).contains(&self.phase)
    }

    /// `FUN_00141fd8` (vt+0x15c), reached from case 0 once the intro is
    /// over (game+0x9c 4): phase 1, the referee holding the ball. The
    /// match's constructor (`FUN_0013cf58`) left game+0x110 at -1.
    pub fn start_tip(&mut self) {
        *self = Self {
            phase: phase::TIP,
            ..Self::default()
        };
    }

    /// `FUN_001406b8` (vt+0x14c): phase 6 with `team` to inbound. The
    /// rest of the restart (the ready flags, the controls, the ball the
    /// other side holds, the turbo) is the world's.
    pub fn start_restart(&mut self, team: usize) {
        self.phase = phase::RESTART;
        self.possession = team.min(1) as i32;
    }

    /// The rewrite's own end of a restart without the six walking to their
    /// spots (its timed dead ball, its formation restart): phase 4 as
    /// `FUN_0013e4c8` case 6 sets it.
    pub fn end_restart(&mut self) {
        if self.phase == phase::RESTART {
            self.phase = phase::BACK_COURT;
        }
    }

    /// The rewrite's own: play resumed without the transition that leads
    /// there (the world made live from outside, [`crate::World::step`]'s
    /// sync): `team` has the ball, and the phase is the one
    /// `FUN_0013e4c8` would have left for a ball in state `ball` at `x`
    /// (the holder's, else the ball's).
    pub fn resume(&mut self, team: usize, ball: i32, x: f32, sides: [i32; 2]) {
        self.possession = team.min(1) as i32;
        self.loose = 0;
        self.phase = match ball {
            ball::SHOT | ball::RIM | ball::REBOUND => phase::SHOT,
            ball::LOOSE => phase::LOOSE,
            ball::TOSS => phase::TIP,
            _ => phase::BACK_COURT,
        };
        if self.phase == phase::BACK_COURT {
            self.court(x, sides, false);
        }
    }

    /// Slot 11 (`0x141558`): a shot or lay-up leaves the hands through
    /// the launcher (`FUN_0014aaf8`): phase 3, unless the game is over or
    /// a script runs. A dunk posts slot 12 first, which the game ignores,
    /// then this one.
    pub fn launched(&mut self, scripted: bool) {
        if !scripted && !matches!(self.phase, phase::OVER | phase::WON) {
            self.phase = phase::SHOT;
        }
    }

    /// Slot 24 (`FUN_001415c0`): the ball given to a player of `team`
    /// standing at `x`. In phase 1 (the tip) the game goes to phase 4;
    /// then the side holding it takes possession (`FUN_001405f0`).
    pub fn gained(&mut self, team: usize, x: f32, sides: [i32; 2], scripted: bool) {
        if scripted {
            return;
        }
        if self.phase == phase::TIP {
            // game+0x1a8 = the other side; the referee lets go (vt+0x3c).
            self.phase = phase::BACK_COURT;
        }
        self.possess(team, x, sides, scripted);
    }

    /// `FUN_001405f0` (vt+0x13c): outside a restart, the holder's side
    /// has the ball (`FUN_00140690`, vt+0x144); a new side starts its
    /// phase where the holder stands (`FUN_0013ee10`).
    fn possess(&mut self, team: usize, x: f32, sides: [i32; 2], scripted: bool) {
        if self.phase == phase::RESTART {
            return;
        }
        let team = team.min(1) as i32;
        if team != self.possession {
            self.possession = team;
            self.court(x, sides, scripted);
        }
    }

    /// `FUN_0013ee10`: phase 5 when `x` (the holder's, else the ball's) is
    /// in the half the side with the ball attacks, else 4. x = 0 counts as
    /// +x. Not before anyone has the ball, outside play's phases or while
    /// a script runs.
    fn court(&mut self, x: f32, sides: [i32; 2], scripted: bool) {
        let Ok(team) = usize::try_from(self.possession) else {
            return;
        };
        if matches!(self.phase, phase::PREGAME | phase::OVER | phase::WON) || scripted {
            return;
        }
        let side = sides[team.min(1)];
        let front = if 0.0 <= x { side == 1 } else { side == -1 };
        self.phase = if front {
            phase::FRONT_COURT
        } else {
            phase::BACK_COURT
        };
    }

    /// One update: `FUN_0013d868`'s phase parts in its order.
    pub fn update(&mut self, v: &View) -> Changes {
        let mut changes = Changes::default();
        // game+0x104 counts down outside the script manager's states and
        // game+0x464 up (`0x13d9a0..0x13d9f0`); still holding at
        // [`CELEBRATE_AT`] updates in, the scorer is told to celebrate.
        if 0 < self.hold {
            if !v.scripted {
                self.since += 1;
                self.hold -= 1;
            }
            if self.hold != 0 && self.since == CELEBRATE_AT {
                changes.celebrate = true;
            }
        }
        // The end of the game (`0x13db98..0x13dbdc`): won (vt+0xbc), no
        // script running and the ball settled (vt+0x164): phase 9.
        if self.phase != phase::WON && v.won && !v.scripted && v.settled {
            self.phase = phase::WON;
            self.countdown = WON_UPDATES;
            changes.won = true;
        }
        // vt+0x13c: the holder's side.
        if let Some((team, x)) = v.holder {
            self.possess(team, x, v.sides, v.scripted);
        }
        self.step(v, &mut changes);
        changes
    }

    /// `FUN_0013e4c8`.
    fn step(&mut self, v: &View, changes: &mut Changes) {
        // A loose ball in play, or a rebound fallen below 6.
        if self.phase != phase::LOOSE
            && self.live()
            && (v.ball == ball::LOOSE || (v.ball == ball::REBOUND && v.ball_y < REBOUND_FLOOR))
        {
            self.phase = phase::LOOSE;
        }
        match self.phase {
            // The tip: the toss in the air frees the players (the world's
            // [`crate::OpeningToss`]); a toss nobody tipped lands loose.
            phase::TIP if v.ball == ball::LOOSE => self.phase = phase::LOOSE,
            phase::LOOSE | phase::SHOT => {
                if let Some((_, x)) = v.holder {
                    self.court(x, v.sides, v.scripted);
                }
            }
            phase::BACK_COURT => {
                // game+0x98 + game+0x110 as a signed byte: -1, else +1.
                let side = usize::try_from(self.possession)
                    .map_or(1, |t| if v.sides[t.min(1)] < 0 { -1 } else { 1 });
                let front = if 0.0 <= v.ball_x {
                    side == 1
                } else {
                    side == -1
                };
                if front {
                    self.phase = phase::FRONT_COURT;
                    changes.crossed = true;
                    changes.brought_up = !v.brought_up;
                } else if !v.brought_up && v.holder.is_some() {
                    changes.brought_up = true;
                }
            }
            phase::RESTART if v.ready && self.hold == 0 && v.holder.is_none() => {
                self.phase = phase::BACK_COURT;
                changes.restart_over = true;
            }
            phase::WON if 0 < self.countdown => {
                self.countdown -= 1;
                changes.won_update = true;
            }
            _ => {}
        }
        if (phase::LOOSE..=phase::SHOT).contains(&self.phase) {
            self.loose += 1;
        } else {
            self.loose = 0;
        }
    }

    /// For [`crate::World::validate`].
    pub fn valid(&self) -> bool {
        matches!(self.phase, 0..=6 | 8 | 9)
            && (-1..=1).contains(&self.possession)
            && 0 <= self.loose
            && (0..=GAMEBREAKER_HOLD).contains(&self.hold)
            && 0 <= self.since
            && (0..=WON_UPDATES).contains(&self.countdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sides as the original has them: team 0 attacks +x, team 1 -x.
    const SIDES: [i32; 2] = [1, -1];

    fn view(ball: i32) -> View {
        View {
            ball,
            ball_x: 0.0,
            ball_y: 0.0,
            holder: None,
            sides: SIDES,
            ready: false,
            brought_up: false,
            won: false,
            settled: true,
            scripted: false,
        }
    }

    fn held(team: usize, x: f32) -> View {
        View {
            ball_x: x,
            ball_y: 4.0,
            holder: Some((team, x)),
            ..view(ball::HELD)
        }
    }

    fn at(phase: i32, possession: i32) -> GamePhase {
        GamePhase {
            phase,
            possession,
            ..GamePhase::default()
        }
    }

    #[test]
    fn the_tip_goes_to_the_takers_back_or_front_court() {
        // Slot 24 in phase 1: phase 4, then `FUN_0013ee10` by the
        // taker's x for the side that now has it.
        let mut g = GamePhase::default();
        g.start_tip();
        assert_eq!((g.phase, g.possession), (phase::TIP, -1));
        g.gained(0, -3.25, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::BACK_COURT, 0));
        let mut g = at(phase::TIP, -1);
        g.gained(1, -3.25, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::FRONT_COURT, 1));
        // A toss nobody tips lands loose.
        let mut g = at(phase::TIP, -1);
        g.update(&view(ball::TOSS));
        assert_eq!(g.phase, phase::TIP);
        g.update(&view(ball::LOOSE));
        assert_eq!(g.phase, phase::LOOSE);
    }

    #[test]
    fn a_steal_or_interception_brings_the_ball_up_from_the_back_court() {
        // Team 0 attacks +x: team 1 steals at x = 20, team 0's front
        // court and team 1's back court: phase 4 for team 1.
        let mut g = at(phase::FRONT_COURT, 0);
        g.gained(1, 20.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::BACK_COURT, 1));
        // Taken in the new side's own front court: phase 5.
        let mut g = at(phase::FRONT_COURT, 0);
        g.gained(1, -5.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::FRONT_COURT, 1));
        // A teammate's catch changes nothing.
        let mut g = at(phase::FRONT_COURT, 0);
        g.gained(0, -20.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::FRONT_COURT, 0));
        // Not while a script runs.
        let mut g = at(phase::FRONT_COURT, 0);
        g.gained(1, 20.0, SIDES, true);
        assert_eq!((g.phase, g.possession), (phase::FRONT_COURT, 0));
    }

    #[test]
    fn the_back_court_ends_when_the_ball_crosses_half_court() {
        let mut g = at(phase::BACK_COURT, 1);
        // Held in the back court: game+0x4a4 is set.
        let c = g.update(&held(1, 10.0));
        assert_eq!(g.phase, phase::BACK_COURT);
        assert!(c.brought_up && !c.crossed);
        let c = g.update(&View {
            brought_up: true,
            ..held(1, 1.0)
        });
        assert_eq!(c, Changes::default());
        // The ball (not the holder) over the line: phase 5.
        let c = g.update(&View {
            ball_x: -0.5,
            brought_up: true,
            ..held(1, 0.5)
        });
        assert_eq!(g.phase, phase::FRONT_COURT);
        assert!(c.crossed && !c.brought_up);
        // A pass across half court too, setting game+0x4a4.
        let mut g = at(phase::BACK_COURT, 0);
        let c = g.update(&View {
            ball_x: 2.0,
            ..view(ball::PASS)
        });
        assert_eq!(g.phase, phase::FRONT_COURT);
        assert!(c.crossed && c.brought_up);
    }

    #[test]
    fn a_shot_launch_then_a_rebound_or_a_block() {
        let mut g = at(phase::FRONT_COURT, 0);
        g.launched(false);
        assert_eq!(g.phase, phase::SHOT);
        // Off the rim, above 6: still phase 3.
        g.update(&View {
            ball_y: 9.0,
            ..view(ball::REBOUND)
        });
        assert_eq!((g.phase, g.loose), (phase::SHOT, 1));
        // Below 6: a loose ball.
        g.update(&View {
            ball_y: 5.9,
            ..view(ball::REBOUND)
        });
        assert_eq!((g.phase, g.loose), (phase::LOOSE, 2));
        // A rebound caught in the air: the catcher's court.
        let mut g = at(phase::SHOT, 0);
        g.gained(1, -30.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::FRONT_COURT, 1));
        let mut g = at(phase::SHOT, 0);
        g.update(&View {
            ball_y: 9.0,
            ..held(0, 30.0)
        });
        assert_eq!(g.phase, phase::FRONT_COURT);
        // A block knocks the shot loose (state 1): phase 2 at once.
        let mut g = at(phase::SHOT, 0);
        g.update(&View {
            ball_y: 15.0,
            ..view(ball::LOOSE)
        });
        assert_eq!(g.phase, phase::LOOSE);
        // No launch once the game is won, or under a script.
        let mut g = at(phase::WON, 0);
        g.launched(false);
        assert_eq!(g.phase, phase::WON);
        let mut g = at(phase::FRONT_COURT, 0);
        g.launched(true);
        assert_eq!(g.phase, phase::FRONT_COURT);
    }

    #[test]
    fn a_loose_ball_and_its_pickup() {
        // Knocked loose or a pass gone loose in either court: phase 2.
        for p in [phase::BACK_COURT, phase::FRONT_COURT] {
            let mut g = at(p, 0);
            g.update(&view(ball::LOOSE));
            assert_eq!(g.phase, phase::LOOSE);
        }
        // Picked up by the same side: phase by where it is.
        let mut g = at(phase::LOOSE, 0);
        g.update(&held(0, 20.0));
        assert_eq!((g.phase, g.loose), (phase::FRONT_COURT, 0));
        let mut g = at(phase::LOOSE, 0);
        g.update(&held(0, -20.0));
        assert_eq!(g.phase, phase::BACK_COURT);
        // By the other side: possession and its court.
        let mut g = at(phase::LOOSE, 0);
        g.gained(1, 20.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::BACK_COURT, 1));
    }

    #[test]
    fn a_restart_ends_at_the_inbound_pass_once_all_six_are_ready() {
        let mut g = at(phase::FRONT_COURT, 0);
        g.start_restart(1);
        assert_eq!((g.phase, g.possession), (phase::RESTART, 1));
        // The other side's catch does not take possession in phase 6.
        g.gained(0, 0.0, SIDES, false);
        assert_eq!((g.phase, g.possession), (phase::RESTART, 1));
        // Ready but the inbounder still holds it; then not ready.
        g.update(&View {
            ready: true,
            ..held(1, 30.0)
        });
        assert_eq!(g.phase, phase::RESTART);
        g.update(&view(ball::PASS));
        assert_eq!(g.phase, phase::RESTART);
        let c = g.update(&View {
            ready: true,
            ball_x: 30.0,
            ..view(ball::PASS)
        });
        assert_eq!(g.phase, phase::BACK_COURT);
        assert!(c.restart_over);
    }

    #[test]
    fn a_gamebreakers_basket_holds_the_restart() {
        let mut g = at(phase::FRONT_COURT, 0);
        g.start_restart(1);
        g.hold = GAMEBREAKER_HOLD;
        let ready = View {
            ready: true,
            ball_x: 30.0,
            ..view(ball::PASS)
        };
        for _ in 0..GAMEBREAKER_HOLD - 1 {
            g.update(&ready);
            assert_eq!(g.phase, phase::RESTART);
        }
        // The hold does not count under a script.
        g.update(&View {
            scripted: true,
            ..ready
        });
        assert_eq!((g.phase, g.hold), (phase::RESTART, 1));
        g.update(&ready);
        assert_eq!((g.phase, g.hold), (phase::BACK_COURT, 0));
    }

    #[test]
    fn the_holds_scorer_is_told_to_celebrate_120_updates_in() {
        // `0x13d9a0..0x13da50`: game+0x464 counts with the hold (not
        // under a script) and at fps × 2 the scorer is told, once.
        let mut g = at(phase::FRONT_COURT, 0);
        g.start_restart(1);
        g.hold = GAMEBREAKER_HOLD;
        let waiting = view(ball::PASS);
        let mut told = Vec::new();
        for k in 1..GAMEBREAKER_HOLD + 10 {
            let scripted = k == 50;
            let c = g.update(&View {
                scripted,
                ..waiting
            });
            if c.celebrate {
                told.push(k);
            }
        }
        assert_eq!(told, [CELEBRATE_AT + 1]);
        assert_eq!((g.hold, g.since), (0, GAMEBREAKER_HOLD));
    }

    #[test]
    fn a_won_game_goes_to_phase_9_once_the_ball_settles() {
        let mut g = at(phase::SHOT, 0);
        g.update(&View {
            won: true,
            settled: false,
            ball_y: 12.0,
            ..view(ball::SHOT)
        });
        assert_eq!(g.phase, phase::SHOT);
        // Under a script the game is not ended, though the loose ball
        // still ends phase 3 (the top of `FUN_0013e4c8` is not gated).
        g.update(&View {
            won: true,
            scripted: true,
            ..view(ball::LOOSE)
        });
        assert_eq!(g.phase, phase::LOOSE);
        g.update(&View {
            won: true,
            ..view(ball::LOOSE)
        });
        assert_eq!((g.phase, g.countdown), (phase::WON, WON_UPDATES - 1));
        assert!(!g.live());
        for _ in 0..WON_UPDATES + 5 {
            g.update(&View {
                won: true,
                ..view(ball::LOOSE)
            });
        }
        assert_eq!((g.phase, g.countdown), (phase::WON, 0));
    }

    #[test]
    fn only_phases_2_to_5_are_live() {
        for p in 0..=9 {
            assert_eq!(at(p, 0).live(), (2..=5).contains(&p), "{p}");
        }
    }
}
