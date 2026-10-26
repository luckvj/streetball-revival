//! Who each person drives, and the moments it moves (traced raw
//! 2026-10-18, docs/TRACE.md "Who the person controls, and the stick").
//!
//! The original keeps a controller word for each actor (game+0x24 + actor
//! × 4, `FUN_00156d98`): the pad (0..3) that drives him, or 4 for the
//! computer. The simulation reads the words from its inputs (a person's
//! player is one whose input is not [`Input::computer`],
//! [`World::people`]) and moves them at the original's moments within the
//! update; a client puts each person's pad on the player [`World::people`]
//! names for his side after the step ([`World::person_on`]).
//!
//! Only a side exactly one pad plays moves (game+0x9a, counted by
//! `FUN_0013cf58`, whose second loop starts that pad on the side's first
//! player): the game's construction (`FUN_0013c4e8`, `FUN_0013d388`) sets
//! team+0xc8 to `game+0x9a != 1`, and every move below waits while
//! team+0xc8 is set, so on a side two or three people play each keeps his
//! own player and Cross never switches. On a side one pad plays, team+0xc8
//! is the give 'n go ([`Control::hold`]).
//!
//! - **The action tap** (`FUN_0012f250`, the "action tap" process,
//!   priority 2, before the game's update): the pass message (`FUN_0012f2c8`
//!   case 9) on defence (`FUN_0012fd18`: game+0x110 not the pad's side), or
//!   in a loose ball or a shot (phases 2 and 3), is no pass press; outside
//!   the intro and the toss (phases 0 and 1), with the side not held, it is
//!   the switch (`FUN_0012fd60`, [`World::switch_target`]).
//! - **The game's update** (`FUN_0013d868`, `0x13e004..0x13e0d4`): outside
//!   phase 0, with somebody's possession and that side not held, a holder
//!   (ball+0xf4) the computer drives takes the side's pad.
//! - **The take** (`FUN_001415c0`, slot 24: every catch, steal, pickup,
//!   rebound and the restart's hand-over): the new holder, if the computer
//!   drives him and his side is not held, takes its pad.
//! - **The release** (`FUN_00173570`, callback 38, `0x173dd0..0x173e84`):
//!   the receiver (actor+0x74), if the computer drives him and the passer's
//!   side is not held, takes the passer's word and the passer goes to the
//!   computer. So the person's control leaves the passer as the ball does,
//!   not at the catch: in the played captures 43 of the person's 112
//!   changes come in a pass's flight, every one in its release's update.
//!
//! What a person's actor reads (`FUN_001587d8`): his pad, except while he
//! is a pass's receiver (ball state 4 and ball+0x100, from the press to the
//! catch), the computer walks him (actor+0x298) or he stands outside ±40.5
//! by ±25 (`FUN_0016f530`), when his AI's frame drives him
//! ([`World::pad_off`]). And a receiver's stick words (actor+0x84..+0x98)
//! keep what they were at the press for the whole of state 4
//! ([`Control::frozen`]; ported for the person's receivers): in the
//! captures his angle and strength hold in 18,169 of 18,202 such updates
//! (others change in a quarter).

use crate::{BallState, Input, World, button, game_phase, target_hoop};

/// A side's switch score for a fallen teammate (`FUN_0016ea70`, the clip's
/// flag 0x40, `0x1307a4..0x1307b0`).
const FALLEN: f32 = 3.0;
/// The switch takes the least score under this (`0x1307e4`).
const UNDER: f32 = 200.0;
/// A teammate off the screen scores this more (`FUN_00171920` false,
/// `0x1307c0`).
const OFF_SCREEN: f32 = 20.0;
/// A shot higher than this scores the defenders by the rim (`0x130678`).
const HIGH_SHOT: f32 = 8.0;

/// What the game keeps of the people's control between updates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Control {
    /// Team+0xc8 by side, on a side one pad plays: a give 'n go under way.
    /// The holder's right-stick pass sets it (`FUN_0015a060`,
    /// `0x15b900..0x15b940`); it clears when the person presses the right
    /// stick again off the ball (`FUN_001587d8`), when one of the side
    /// shoots (callback 21, `0x1734fc..0x17352c`) or the computer on it
    /// passes (callback 38, `0x173e44..0x173e80`), and when the side is
    /// without the ball (the team's update `FUN_0018a708`). While it is set
    /// the person keeps the passer, and Cross off the ball calls for the
    /// ball back.
    pub hold: [bool; 2],
    /// The pad the side's one person played, as the last update left it:
    /// the pad record's own last held word, against which the action tap
    /// takes the presses (`FUN_0012f250`: pressed = held and not before),
    /// whichever player the pad drives.
    pub pad_before: [Input; 2],
    /// A pass's receiver's stick as the press found it, which his pad
    /// update leaves alone until the ball is his ([`Frozen`]).
    pub frozen: Option<Frozen>,
    /// The game's tables as the last update built them (`FUN_0013f748`,
    /// `FUN_0013fbc8`, in the game's update before the actors move), which
    /// the action tap's switch reads before this update builds its own.
    pub tables: Option<Tables>,
}

/// A receiver's stick words held through a pass (`FUN_001587d8` skips
/// actor+0x84..+0x98 while the ball is in state 4 and he is ball+0x100).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frozen {
    pub player: usize,
    pub stick: [i8; 2],
    pub aim: [i8; 2],
}

/// What the game's tables were built from: the players' places on the
/// floor and the ball's place.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub places: Vec<[f32; 2]>,
    pub ball: [f32; 3],
}

impl World {
    fn side_of(&self, i: usize) -> usize {
        self.players[i].team.min(1)
    }

    /// Game+0x9a for `side`: the pads that play it this update.
    pub fn pads_on(&self, side: usize) -> usize {
        (0..self.players.len())
            .filter(|&j| self.side_of(j) == side && self.people.get(j).copied().unwrap_or(false))
            .count()
    }

    /// Team+0xc8: control on `side` stays where it is (a side without
    /// exactly one pad, or a give 'n go under way).
    pub fn control_held(&self, side: usize) -> bool {
        self.pads_on(side) != 1 || self.control.hold[side.min(1)]
    }

    /// The player the one person on `side` drives, when exactly one plays
    /// it: what a client puts that person's pad on for the next update.
    pub fn person_on(&self, side: usize) -> Option<usize> {
        if self.pads_on(side) != 1 {
            return None;
        }
        (0..self.players.len()).find(|&j| self.side_of(j) == side && self.people[j])
    }

    /// The edges `i`'s presses are taken against this update: on a side one
    /// pad plays, the pad's own last word for its person
    /// ([`Control::pad_before`]); otherwise his last input.
    pub(crate) fn pad_before(&self, i: usize) -> Input {
        let side = self.side_of(i);
        if self.people.get(i).copied().unwrap_or(false) && self.pads_on(side) == 1 {
            self.control.pad_before[side]
        } else {
            self.previous_inputs[i]
        }
    }

    /// Ball+0x100 while the ball is in state 4 (`FUN_0014bff8` at the
    /// press, so through the windup and the flight): the pass's receiver.
    pub fn pass_receiver(&self) -> Option<usize> {
        if let Some(w) = self.windup
            && self.ball.state == BallState::Held(w.from)
        {
            return Some(w.to);
        }
        match (self.ball.state, self.ball.pass) {
            (BallState::Pass, Some(p)) => Some(p.to),
            _ => None,
        }
    }

    /// Ball+0xf4: the holder, or a trick pass's passer while it is in the
    /// air (`FUN_0013bf60`).
    pub(crate) fn ball_holder(&self) -> Option<usize> {
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

    /// `FUN_001587d8`'s pad for a person's actor `i`: his AI's frame
    /// (`FUN_0013a970`) instead of his pad while he is a pass's receiver or
    /// stands outside ±40.5 by ±25 (`FUN_0016f530`; on the disc's court, as
    /// the court's bound [`World::contain`] is). (The restarts, where the
    /// computer walks him, are [`World::restarting`]'s.)
    pub fn pad_off(&self, i: usize) -> bool {
        if !self.people.get(i).copied().unwrap_or(false) {
            return false;
        }
        let [x, z] = self.players[i].position;
        let outside = x.abs() > 40.5 || z.abs() > 25.0;
        self.pass_receiver() == Some(i) || (outside && self.poses.get().is_some())
    }

    /// `FUN_0012fd18` and the pass message: Cross on `side` is no pass on
    /// defence (game+0x110 not the side) or in a loose ball or a shot.
    fn cross_is_switch(&self, side: usize) -> bool {
        use game_phase::phase;
        self.game_phase.possession != side as i32
            || matches!(self.game_phase.phase, phase::LOOSE | phase::SHOT)
    }

    /// The controller word of `from` goes to `to`, and `from` to the
    /// computer.
    fn move_control(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        self.people[to] = self.people[from];
        self.people[from] = false;
        self.sync_turbo();
    }

    /// The action tap: each person's pass message, as `FUN_0012f2c8` case 9
    /// takes it, before the game's update. `inputs` are this update's, one a
    /// player; a switch moves the person's pad to the teammate it picks.
    pub(crate) fn action_tap(&mut self, inputs: &mut [Input]) {
        use game_phase::phase;
        let n = self.players.len().min(inputs.len());
        for side in 0..2 {
            if !self.cross_is_switch(side) {
                continue;
            }
            for i in 0..n {
                if self.side_of(i) != side || !self.people[i] {
                    continue;
                }
                let pressed = inputs[i].pressed_since(&self.pad_before(i));
                // The pad's pass bit stays clear (no `FUN_00146228`).
                inputs[i].buttons &= !button::PASS;
                if pressed & button::PASS == 0
                    || matches!(self.game_phase.phase, phase::PREGAME | phase::TIP)
                    || self.control_held(side)
                {
                    continue;
                }
                if let Some(j) = self.switch_target_seen(i, inputs[i].off_screen) {
                    inputs[j] = inputs[i];
                    inputs[i] = Input::computer();
                    self.move_control(i, j);
                }
                // A side that can switch has one person.
                break;
            }
        }
    }

    /// `FUN_0012fd60` (raw `0x130634..0x130864`): the teammate a person's
    /// switch from `current` goes to. The per-pad direction pick (the
    /// options word's bit 12 + pad, `FUN_001e0bc0()`+0x138, 0 by default and
    /// on no options row) and the cycle (`def_switch_cycle`,
    /// `DAT_003d01c8`, 0 as registered at `0x1310c8` and in no `CSCRIPTS`
    /// file) are off, so every switch is the scored pick: each teammate the
    /// computer drives scores his distance (the game's tables as the last
    /// update built them) to a pass's receiver while one flies (ball state 4
    /// with no holder, ball+0x100; `FUN_0018d0c0`); else to the rim the
    /// offence attacks (game+0x374) when a shot is up that is his side's,
    /// falling or above 8, or the holder is in a dunk or a lay-up (bits 7,
    /// 13), or `current` is his side's nearest the ball (game+0x3a4); else
    /// to the ball (game+0x3d4). A fallen teammate adds 3; one off the
    /// screen adds 20 (`FUN_00171920`, the active camera's projection:
    /// [`Input::off_screen`], the person's client's camera, in
    /// [`World::switch_target_seen`]; none here). The least under 200
    /// wins, the first of equals. The side's nearest ranks by `(int)(a -
    /// b)` in the C library's sort (`FUN_003b3868`, `LAB_0013f728`), so
    /// distances within a unit keep actor order.
    pub fn switch_target(&self, current: usize) -> Option<usize> {
        self.switch_target_seen(current, 0)
    }

    /// [`World::switch_target`] with the actors `off_screen` (bit k for
    /// actor k, [`Input::off_screen`]) scoring 20 more.
    pub fn switch_target_seen(&self, current: usize, off_screen: u8) -> Option<usize> {
        use game_phase::ball;
        let n = self.players.len();
        let side = self.side_of(current);
        let (places, at) = match &self.control.tables {
            Some(t) if t.places.len() == n => (t.places.clone(), t.ball),
            _ => (self.floor_positions(), self.ball.position),
        };
        let state = self.native_ball_state();
        let mut by_rim = state == ball::SHOT
            && (self.game_phase.possession == side as i32
                || self.ball.velocity[1] < 0.0
                || self.ball.position[1] > HIGH_SHOT);
        let mut receiver = None;
        match self.ball_holder() {
            None if state == ball::PASS => receiver = self.ball.pass.map(|p| p.to),
            None => {}
            Some(h) => {
                let bits = self.actor_state_bits(h);
                by_rim |= bits & (1 << 7 | 1 << 13) != 0;
            }
        }
        let squad: Vec<usize> = (0..n).filter(|&j| self.side_of(j) == side).collect();
        let to_ball = |j: usize| crate::distance(places[j], [at[0], at[2]]);
        // Game+0x3a4: every player by his distance to the ball, sorted as
        // the game's tables are ([`crate::ai::native_sort`]), the side's
        // first.
        let mut all: Vec<usize> = (0..n).collect();
        crate::ai::native_sort(&mut all, to_ball);
        let nearest = all.into_iter().find(|&j| self.side_of(j) == side);
        let rim = self.rim_spot(target_hoop(self.possession.min(1)));
        let score = |j: usize| {
            let mut s = match receiver {
                Some(r) => crate::distance(places[j], places[r]),
                None if by_rim || nearest == Some(current) => crate::distance(places[j], rim),
                None => to_ball(j),
            };
            if matches!(
                self.players[j].action,
                crate::Action::Stumbling { .. } | crate::Action::Dive { .. }
            ) {
                s += FALLEN;
            }
            if j < 8 && off_screen >> j & 1 != 0 {
                s += OFF_SCREEN;
            }
            s
        };
        let mut best: Option<(usize, f32)> = None;
        for &j in &squad {
            if self.people[j] {
                continue;
            }
            let s = score(j);
            if s < best.map_or(UNDER, |(_, b)| b) {
                best = Some((j, s));
            }
        }
        best.map(|(j, _)| j)
    }

    /// The game's update's hand-off (`FUN_0013d868`): outside phase 0, a
    /// holder the computer drives on the side with the ball, not held,
    /// takes that side's pad. (The original does not ask the holder's side;
    /// every captured hand-off had the holder on the side with the ball,
    /// which the simulation requires.)
    pub(crate) fn game_hand_off(&mut self, inputs: &mut [Input]) {
        let p = self.game_phase.possession;
        if self.game_phase.phase == game_phase::phase::PREGAME || p < 0 {
            return;
        }
        let side = p as usize;
        if side > 1 || self.control_held(side) {
            return;
        }
        let Some(h) = self.ball_holder() else {
            return;
        };
        if self.people[h] || self.side_of(h) != side {
            return;
        }
        let Some(from) = self.person_on(side) else {
            return;
        };
        if let (Some(&pad), true) = (inputs.get(from), h < inputs.len()) {
            inputs[h] = pad;
            inputs[from] = Input::computer();
        }
        self.move_control(from, h);
    }

    /// The take's hand-off (`FUN_001415c0`, slot 24): `h` has just been
    /// given the ball; if the computer drives him and his side is not
    /// held, he takes its pad.
    pub(crate) fn take_hand_off(&mut self, h: usize) {
        let side = self.side_of(h);
        if self.people[h] || self.control_held(side) {
            return;
        }
        if let Some(from) = self.person_on(side) {
            self.move_control(from, h);
        }
    }

    /// The release's hand-off (`FUN_00173570`, callback 38): the receiver
    /// the computer drives takes the passer's word, the passer goes to the
    /// computer, unless the passer's side is held; a computer's pass on a
    /// side one pad plays ends its give 'n go.
    pub(crate) fn release_hand_off(&mut self, from: usize, to: usize) {
        let side = self.side_of(from);
        if !self.control_held(side) && !self.people[to] {
            self.move_control(from, to);
        }
        if !self.people[from] && self.pads_on(side) == 1 {
            self.control.hold[side] = false;
        }
    }

    /// Callback 21 (`0x1734fc..0x17352c`): a shot's release on `i`'s side,
    /// one pad playing it, ends its give 'n go.
    pub(crate) fn shot_ends_hold(&mut self, i: usize) {
        let side = self.side_of(i);
        if self.pads_on(side) == 1 {
            self.control.hold[side] = false;
        }
    }

    /// `FUN_0015a060` (`0x15b900..0x15b940`): the holder `i`'s right-stick
    /// pass on a side one pad plays starts a give 'n go.
    pub(crate) fn give_and_go_press(&mut self, i: usize) {
        let side = self.side_of(i);
        if self.people[i] && self.pads_on(side) == 1 {
            self.control.hold[side] = true;
        }
    }

    /// `FUN_001587d8`'s give 'n go for a person's actor `i` without the
    /// ball, this update's `input` against `before`, while his side is
    /// held (team+0xc8) and has the ball: Circle without a turbo asks the
    /// computer holder to shoot (`0x158e14..0x158f04`), Cross to pass back
    /// (with two turbos held, a trick pass; `0x158f50..0x159098`), each
    /// [`World::call_for_ball`]'s node; otherwise the right stick pushed
    /// home again, on a side one pad plays, ends the hold, and the game's
    /// next update hands him the holder.
    pub(crate) fn off_ball_call(
        &mut self,
        i: usize,
        input: &Input,
        before: &Input,
    ) -> Vec<crate::ai::Call> {
        let side = self.side_of(i);
        let pressed = input.pressed_since(before);
        let held = self.control_held(side)
            && self.ball.state != BallState::Held(i)
            && self.ball_holder().is_some()
            && self.game_phase.possession == side as i32;
        let mut calls = Vec::new();
        if held && pressed & button::SHOOT != 0 && input.turbos() == 0 {
            calls.push(crate::ai::Call::Shoot);
        }
        if held && pressed & button::PASS != 0 {
            calls.push(if input.turbos() >= 2 {
                crate::ai::Call::TrickPass
            } else {
                crate::ai::Call::Pass
            });
        }
        if !calls.is_empty() {
            return calls;
        }
        if self.pads_on(side) == 1
            && self.control.hold[side]
            && crate::passing::aim_pressed(input, before)
        {
            self.control.hold[side] = false;
        }
        calls
    }

    /// The team's update (`FUN_0018a708`): a side one pad plays and without
    /// the ball (game+0x110) ends its give 'n go.
    pub(crate) fn hold_without_ball(&mut self) {
        for side in 0..2 {
            if self.game_phase.possession != side as i32 && self.pads_on(side) == 1 {
                self.control.hold[side] = false;
            }
        }
    }

    /// Keep the receiver's stick as his last update left it at the press
    /// (state 4 begun), to the catch ([`Frozen`], applied to a person's:
    /// [`World::frozen_input`]).
    pub(crate) fn freeze_receiver(&mut self) {
        let Some(r) = self.pass_receiver() else {
            self.control.frozen = None;
            return;
        };
        if self.control.frozen.is_some_and(|f| f.player == r) {
            return;
        }
        let last = self.previous_inputs.get(r).copied().unwrap_or_default();
        self.control.frozen = Some(Frozen {
            player: r,
            stick: last.stick,
            aim: last.aim,
        });
    }

    /// `input` for player `i` with a frozen receiver's stick words, a
    /// person's (the press's, though he became the person's at the release).
    /// The original keeps every receiver's, the computer's too (the attract
    /// captures' receivers move along the stick they had at the press); the
    /// simulation's computer receivers still steer through the pass, which
    /// its own leads and court bound lean on (docs/TRACE.md "Who the person
    /// controls, and the stick": with theirs held as well, the first
    /// attract match's passes loose at their timer went from 8.0 to 12.1 a
    /// game against the original's 1).
    pub(crate) fn frozen_input(&self, i: usize, mut input: Input) -> Input {
        if let Some(f) = self.control.frozen
            && f.player == i
            && self.people[i]
            && self.pass_receiver() == Some(i)
        {
            input.stick = f.stick;
            input.aim = f.aim;
        }
        input
    }
}
