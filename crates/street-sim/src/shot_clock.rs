//! The shot clock as the game runs it: `FUN_0013f1f0` (the game's
//! vt+0x104, each update after the phase's own update) and its refill
//! `FUN_0013f188` (vt+0xfc). See `docs/TRACE.md` "The shot clock".
//!
//! - The clock refills (`FUN_0013f188`: the current time to the full one,
//!   slot 50, the three waits cleared) when the side with the ball
//!   (game+0x110) is not the one it last saw (`DAT_003d30c4`) — a take, a
//!   restart for the other side — when a shot in the air (ball state 5)
//!   starts down, unless a pass's wait runs, and when the ball meets the
//!   rim or the collar at the end the side with the ball attacks (slot 30,
//!   the game's `FUN_00141d48`), which also stops the clock (game+0x18c)
//!   until someone holds the ball.
//! - It runs one update's time in live play (`FUN_00140558`) unless
//!   stopped, run out, or a gamebreaker is in play (`FUN_00140ff8`).
//! - Run out (slot 49): with nobody holding the ball it waits (+0x198)
//!   for a holder or a loose ball or rebound below 12; with the holder in
//!   a shot, a dunk, a lay-up or a leap (bits 3, 7, 13, 15) it waits
//!   (+0x1a4) for the shot (a refill — the rim, the shot falling — ends the
//!   wait without a violation); otherwise the violation (slot 52), and the
//!   restart for the other side (vt+0x14c, `FUN_001406b8`), or, with the
//!   holder in a pass (bit 4), a wait (+0x19c) for his pass to end.
//!
//! Not modelled: the singleton's +0x74 (never set in play) and slot 48's
//! per-second post (the HUD and the crowd read the clock themselves).

use crate::{Action, Event, World, game_phase::ball};

/// The words the clock keeps beside its time ([`World::shot_clock`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotClock {
    /// `DAT_003d30c4`: the side with the ball the clock last saw, −1 for
    /// none yet.
    pub side: i32,
    /// Game+0x18c: stopped by the rim until someone holds the ball.
    pub stopped: bool,
    /// Game+0x198: run out with nobody holding the ball.
    pub loose: bool,
    /// Game+0x19c: run out while the holder passes.
    pub pass: bool,
    /// Game+0x1a4: run out while the holder shoots.
    pub shot: bool,
}

impl Default for ShotClock {
    fn default() -> Self {
        Self {
            side: -1,
            stopped: false,
            loose: false,
            pass: false,
            shot: false,
        }
    }
}

impl World {
    /// `FUN_0013f188`: the clock full again, its waits cleared.
    pub fn refill_shot_clock(&mut self) {
        self.shot_clock = self.rules.shot_clock.unwrap_or(0.0);
        self.clock.loose = false;
        self.clock.pass = false;
        self.clock.shot = false;
    }

    /// Slot 30 (`FUN_00141d48`): the ball met the rim or the collar. At
    /// the end the side with the ball attacks, the clock stops until the
    /// next hold and, unless a pass's wait runs, is full again.
    pub(crate) fn rim_touched(&mut self) {
        let side = crate::restart::side(self.possession.min(1));
        let x = self.ball.position[0];
        if (side > 0.0 && x > 0.0) || (side < 0.0 && x < 0.0) {
            self.clock.stopped = true;
            if !self.clock.pass {
                self.refill_shot_clock();
            }
        }
    }

    /// The holder's state bits the clock tests: a shot, a dunk, a lay-up or
    /// a leap (3, 7, 13, 15).
    fn clock_shooting(&self, h: usize) -> bool {
        crate::block::holder_bits(&self.players[h].action) & 0xa088 != 0
    }

    /// Bit 4 (`FUN_00154680`): the holder throws a pass.
    fn clock_passing(&self, h: usize) -> bool {
        matches!(self.players[h].action, Action::Passing { .. })
    }

    /// The violation (slot 52): the side that ran it out loses 12,500 from
    /// a meter without a hold (`FUN_00144260`).
    fn shot_clock_violation(&mut self) {
        let team = self.possession.min(1);
        self.events.push(Event::ShotClockViolation { team });
        if self.teams[team].meter_hold == 0 {
            self.fill_meter(team, -12_500.0);
        }
    }

    /// `FUN_0013f1f0`, once an update.
    pub(crate) fn step_shot_clock(&mut self) {
        if self.rules.shot_clock.is_none() || self.sequence.is_some() {
            return;
        }
        let holder = self.handler();
        let state = self.native_ball_state();
        if self.clock.stopped && holder.is_some() {
            self.clock.stopped = false;
        }
        if !self.clock.pass && state == ball::SHOT && self.ball.velocity[1] <= 0.0 {
            self.refill_shot_clock();
        }
        let other = 1 - self.possession.min(1);
        if self.clock.loose {
            if holder.is_none()
                && (12.0 <= self.ball.position[1] || !matches!(state, ball::REBOUND | ball::LOOSE))
            {
                return;
            }
            self.shot_clock_violation();
            self.clock.loose = false;
            self.start_restart(other);
            return;
        }
        if self.clock.pass {
            match holder {
                None if !matches!(state, ball::LOOSE | ball::REBOUND) => return,
                Some(h) if self.clock_shooting(h) || self.clock_passing(h) => return,
                _ => {}
            }
            self.clock.pass = false;
            self.start_restart(other);
            return;
        }
        if self.clock.shot {
            let over = match holder {
                None => state != ball::SHOT,
                Some(h) => !self.clock_shooting(h),
            };
            if !over {
                return;
            }
            self.shot_clock_violation();
            self.clock.shot = false;
            if holder.is_some_and(|h| self.clock_passing(h)) {
                self.clock.pass = true;
            } else {
                self.start_restart(other);
            }
            return;
        }
        let side = self.possession as i32;
        if side != self.clock.side {
            self.refill_shot_clock();
            self.clock.side = side;
        }
        let gamebreaker = self.teams.iter().any(|t| t.meter_gamebreaker);
        if !self.game_phase.live()
            || self.phase != crate::Phase::Live
            || self.clock.stopped
            || self.shot_clock == 0.0
            || gamebreaker
        {
            return;
        }
        self.shot_clock = (self.shot_clock - crate::TICK_SECONDS).max(0.0);
        if self.shot_clock > 0.0 {
            return;
        }
        // Slot 49: run out.
        match holder {
            None => self.clock.loose = true,
            Some(h) if self.clock_shooting(h) => self.clock.shot = true,
            Some(h) => {
                self.shot_clock_violation();
                if self.clock_passing(h) {
                    self.clock.pass = true;
                } else {
                    self.start_restart(other);
                }
            }
        }
    }
}
