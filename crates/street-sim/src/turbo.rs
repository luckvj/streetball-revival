//! The turbo bars as the original keeps them (traced raw 2026-10-17,
//! docs/TRACE.md "The turbo bars, the juke press and the counter stance").
//!
//! The game holds six bars (game+0xb0, each `FUN_00190c68` filled to 1 at
//! every restart, `FUN_001406b8`). A player's is `FUN_00142098(game,
//! FUN_00156d98(actor), team)`: a person's controller has its own
//! (game+0xa0[controller], which `FUN_0013cf58` sets to [`COMPUTER_SLOT`]
//! of his side plus the people of that side before him), and every player
//! no person holds (`FUN_00156d98` 4) draws on his side's first,
//! [`COMPUTER_SLOT`] (`DAT_0043ded0`). So a computer side's three share
//! one bar, and a person's computer teammates share his.
//!
//! The bar fills and drains once an update, in one actor's update
//! (actor+0x2ac, `FUN_001587d8`, raw `0x1588f0..0x1589d0`): on a side a
//! person plays, each person's actor; on a computer side, while somebody
//! holds the ball (ball+0xf4), the holder on the side with it and, on the
//! other, the player at the holder's slot (`FUN_00140920`); while nobody
//! does, whoever it was. That actor drains [`crate::tuning::TURBO_DRAIN`]
//! when a turbo button is held in live play (`FUN_00140558`,
//! `FUN_00190c08`) and fills [`crate::tuning::TURBO_REFILL`] otherwise
//! (`FUN_00190c68`); the unlimited turbo cheat keeps every bar full.
//!
//! The moves that cost a bar (`FUN_00190c08`'s other callers) ask for it
//! first ([`World::spend_turbo`]): the juke press, 0.2 for any trick or
//! the clearout (`0x15c9e4`, `0x15c8c0`); the counter stance 0.2
//! (`0x15e094`); the turbo block 0.5 (`0x15d428`); the trick pass 0.2
//! (`0x14741c`).
//!
//! [`crate::Player::turbo`] is the bar the player draws on, as the HUD
//! reads it: a copy of his slot's, kept by [`World::sync_turbo`].

use crate::{BallState, Input, World};

/// Game+0xb0: the bars.
pub const SLOTS: usize = 6;

/// `DAT_0043ded0`: the bar of each side's computer players and its first
/// person.
pub const COMPUTER_SLOT: [usize; 2] = [3, 0];

/// The cost of a juke press, a counter stance and a clearout
/// (`0x3e4ccccd`, `FUN_0015c8e8`, `FUN_0015da28`, `FUN_0015c778`).
pub const MOVE: f32 = f32::from_bits(0x3e4c_cccd);

impl World {
    /// The bar `i` draws on (`FUN_00142098`'s index): his side's
    /// [`COMPUTER_SLOT`], plus, for a person, the people of his side
    /// before him.
    pub fn turbo_slot(&self, i: usize) -> usize {
        let Some(p) = self.players.get(i) else {
            return 0;
        };
        let team = p.team.min(1);
        let base = COMPUTER_SLOT[team];
        if !self.people.get(i).copied().unwrap_or(false) {
            return base;
        }
        let before = (0..i)
            .filter(|&j| self.players[j].team.min(1) == team && self.people[j])
            .count();
        base + before.min(2)
    }

    /// `i`'s bar.
    pub fn turbo_of(&self, i: usize) -> f32 {
        self.turbo[self.turbo_slot(i)]
    }

    /// Set `i`'s bar (and his teammates' who share it).
    pub fn set_turbo(&mut self, i: usize, value: f32) {
        let slot = self.turbo_slot(i);
        self.turbo[slot] = value;
        self.sync_turbo();
    }

    /// `FUN_00190c08` on `i`'s bar: `amount` off it, not below 0.
    pub(crate) fn drain_turbo(&mut self, i: usize, amount: f32) {
        let slot = self.turbo_slot(i);
        self.turbo[slot] = (self.turbo[slot] - amount).max(0.0);
        self.sync_turbo();
    }

    /// A move that costs `amount`: when `i`'s bar holds it
    /// (`FUN_00142098` at least `amount`), it is spent and the move goes
    /// ahead.
    pub(crate) fn spend_turbo(&mut self, i: usize, amount: f32) -> bool {
        if self.turbo_of(i) + 1e-6 < amount {
            return false;
        }
        self.drain_turbo(i, amount);
        true
    }

    /// Every player's [`crate::Player::turbo`] from his slot's bar.
    pub(crate) fn sync_turbo(&mut self) {
        for i in 0..self.players.len() {
            let v = self.turbo[self.turbo_slot(i)];
            self.players[i].turbo = v;
        }
    }

    /// Who a person controls this update (`FUN_00156d98` below 4):
    /// everyone without the computer's marker.
    pub(crate) fn set_people(&mut self, inputs: &[Input]) {
        for (i, p) in self.people.iter_mut().enumerate() {
            *p = !inputs.get(i).is_some_and(Input::is_computer);
        }
        self.sync_turbo();
    }

    /// Ball+0xf4: the holder, or the passer of a trick pass to himself
    /// in the air (`FUN_0013bf60`).
    fn turbo_holder(&self) -> Option<usize> {
        match (self.ball.state, self.ball.pass) {
            (BallState::Held(h), _) => Some(h),
            (BallState::Pass, Some(p))
                if p.from == p.to
                    && matches!(
                        p.via,
                        crate::ball::Via::Head { .. } | crate::ball::Via::Glass { .. }
                    ) =>
            {
                Some(p.from)
            }
            _ => None,
        }
    }

    /// Actor+0x2ac for `i` (`0x1588f0..0x158960`): whose update fills or
    /// drains his side's bar.
    pub(crate) fn update_turbo_owner(&mut self, i: usize) {
        let team = self.players[i].team.min(1);
        let people =
            (0..self.players.len()).any(|j| self.players[j].team.min(1) == team && self.people[j]);
        if people {
            self.turbo_owner[i] = self.people[i];
            return;
        }
        if self.turbo_holder().is_none() {
            return;
        }
        let held = |j: usize| self.ball.state == BallState::Held(j);
        self.turbo_owner[i] = if self.possession.min(1) == team {
            held(i)
        } else {
            // `FUN_00140920(game, team ^ 1, actor+0x280)`.
            let slot = (0..i)
                .filter(|&j| self.players[j].team.min(1) == team)
                .count();
            (0..self.players.len())
                .filter(|&j| self.players[j].team.min(1) != team)
                .nth(slot)
                .is_some_and(held)
        };
    }

    /// The owner's update of the bar (`0x158964..0x1589d0`): a turbo
    /// button held in live play drains it, anything else fills it.
    pub(crate) fn hold_turbo(&mut self, i: usize, input: &Input) {
        if !self.turbo_owner[i] {
            return;
        }
        let slot = self.turbo_slot(i);
        let bar = &mut self.turbo[slot];
        if input.turbo() && self.game_phase.live() {
            *bar = (*bar - crate::tuning::TURBO_DRAIN).max(0.0);
        } else {
            *bar = (*bar + crate::tuning::TURBO_REFILL).min(1.0);
        }
        self.sync_turbo();
    }

    /// `FUN_001406b8`: every bar full again at a restart.
    pub(crate) fn fill_turbo(&mut self) {
        self.turbo = [1.0; SLOTS];
        self.sync_turbo();
    }
}
