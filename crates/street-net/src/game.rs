//! What every machine in a match simulates: the [`World`] plus the seats,
//! the people at the controls (1P..6P), stepped one [`Frame`] of seat pads
//! at a time.
//!
//! Who a seat drives is part of the simulated state, because it depends on
//! the game. The world moves a person's controller where the original
//! moves it ([`street_sim::control`]: the switch on defence, the hand-off at
//! a pass's release and at a take, the give 'n go), as the offline game's
//! `street-game/src/controls.rs` follows it:
//!
//! - The pads go in as sent, the pass button included: the world turns a
//!   pass press on defence into the switch itself.
//! - Alone on a team, a seat drives the player the world has its pad on
//!   ([`World::person_on`]); several seats on a team each keep their own,
//!   as the original keeps a side two or three pads play.
//! - A seat that joins takes the lowest-numbered teammate nobody holds; a
//!   seat that leaves hands its player back to the computer.
//! - Every player no seat drives gets [`street_sim::ai::input`].
//!
//! The match also runs the game's replay manager, director and gamebreaker
//! task ([`street_sim::replay`]) as a match played alone runs them: a
//! basket's instant replay, the gamebreaker's two slowed replays, a
//! person's replay and cancel presses. A replay holds the gameplay as the
//! original's does (`FUN_0033d380(0x401b2)` suspends its task groups): the
//! world isn't stepped and its clock (the AI's dice, the director's notes)
//! stands still, while the match's own tick, the network's, runs on with
//! the frames. Every machine steps the same replays on the same frames, so
//! every machine holds the same updates; each shows its own drawing of
//! them.

use street_sim::replay::{Press, Replays};
use street_sim::{Input, World, ai};

/// The most people in one match: three a side.
pub const MAX_SEATS: usize = 6;

/// The updates a staged Level 2 holds the game after its own: its task
/// suspends the game's (`FUN_00144df0`, groups 0x401d2) until GB2's burst
/// begins its wipe (`FUN_002bb818`), the banner's 32nd update counting the
/// one it shows on (`street-game`'s `banner.rs`, which checks this).
pub const LEVEL2_BURST_HOLD: u16 = 31;

/// One tick of input for the whole match: each seat's pad, or `None` for a
/// seat nobody occupies (its player, if any, goes to the computer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Frame {
    pub seats: [Option<Input>; MAX_SEATS],
}

/// One seat's state in the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seat {
    /// The side this seat plays for (fixed by the match setup).
    pub team: u8,
    /// The player it drives now, or `None` while the seat is empty.
    pub player: Option<u8>,
    /// The pad as sent last tick, for telling fresh presses.
    pub pad: Input,
}

/// The simulated state of a match.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub world: World,
    pub seats: [Seat; MAX_SEATS],
    /// The replay manager, director and gamebreaker task.
    pub replays: Replays,
    /// The frames stepped since the match began, counted from the world's
    /// tick then: the network's clock, which runs on while a replay holds
    /// the world's.
    pub tick: u64,
    /// Updates a staged Level 2's burst still holds the game
    /// ([`LEVEL2_BURST_HOLD`]).
    pub burst_hold: u16,
}

impl Match {
    /// Check the world and seat ownership before using an untrusted snapshot.
    pub fn validate(&self) -> Result<(), crate::wire::DecodeError> {
        use crate::wire::DecodeError;
        self.world
            .validate()
            .map_err(|_| DecodeError::Bad("world"))?;
        let mut held = Vec::new();
        for seat in &self.seats {
            if seat.team >= 2 {
                return Err(DecodeError::Bad("seat team"));
            }
            if let Some(index) = seat.player {
                let player = self
                    .world
                    .players
                    .get(usize::from(index))
                    .ok_or(DecodeError::Bad("seat player"))?;
                if player.team != usize::from(seat.team) || held.contains(&index) {
                    return Err(DecodeError::Bad("seat ownership"));
                }
                held.push(index);
            }
        }
        self.replays
            .validate(self.world.players.len())
            .map_err(|_| DecodeError::Bad("replays"))?;
        if self.tick < self.world.tick {
            return Err(DecodeError::Bad("match tick"));
        }
        Ok(())
    }
    /// A match on `world` with seat `k` on side `seat_teams[k]`, every seat
    /// empty.
    pub fn new(world: World, seat_teams: [u8; MAX_SEATS]) -> Self {
        Self {
            tick: world.tick,
            world,
            seats: seat_teams.map(|team| Seat {
                team,
                ..Default::default()
            }),
            replays: Replays::default(),
            burst_hold: 0,
        }
    }

    /// The frame about to be stepped (the network's tick).
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The seat driving `player`, if any.
    pub fn seat_of(&self, player: usize) -> Option<usize> {
        self.seats
            .iter()
            .position(|s| s.player.map(usize::from) == Some(player))
    }

    /// Advance one tick: settle who each seat drives, then step the world
    /// with the seats' pads and the computer's inputs for everyone else,
    /// unless a replay holds it, and the replays after (as
    /// `street-game`'s `tape` does played alone). While a replay holds the
    /// world its events are none. A staged Level 2 holds it too, as a match
    /// alone does for GB2's burst (`game.rs`'s `step_sim`).
    pub fn step(&mut self, frame: &Frame) {
        // Each person's fresh presses, against the pad of the tick before.
        let presses: Vec<Press> = (0..MAX_SEATS)
            .filter_map(|k| {
                let pad = frame.seats[k]?;
                let team = usize::from(self.seats[k].team);
                Some(Press {
                    team,
                    buttons: pad.buttons & !self.seats[k].pad.buttons,
                    other_human: self.world.teams.get(1 - team).is_some_and(|t| t.human),
                })
            })
            .collect();
        let inputs = self.inputs(frame);
        let burst = self.burst_hold > 0;
        self.burst_hold = self.burst_hold.saturating_sub(1);
        let stepped = !self.replays.holds_world() && !burst;
        if stepped {
            self.world.step(&inputs);
        } else {
            self.world.events.clear();
        }
        let mut events = std::mem::take(&mut self.world.events);
        let option = self.world.replays;
        self.replays.update(
            &mut self.world,
            &mut events,
            stepped,
            &presses,
            option,
            false,
        );
        // The press that stages a Level 2 shows GB2; its burst holds the
        // updates after until its wipe.
        if stepped
            && self.world.staged_start.is_some()
            && events
                .iter()
                .any(|e| matches!(e, street_sim::Event::Gamebreaker { level: 2.., .. }))
        {
            self.burst_hold = LEVEL2_BURST_HOLD;
        }
        self.world.events = events;
        self.tick += 1;
    }

    /// Settle the seats for `frame` and return every player's input.
    fn inputs(&mut self, frame: &Frame) -> Vec<Input> {
        let w = &self.world;
        for k in 0..MAX_SEATS {
            let Some(raw) = frame.seats[k] else {
                self.seats[k].player = None;
                self.seats[k].pad = Input::default();
                continue;
            };
            let seat = &mut self.seats[k];
            seat.pad = raw;
            if seat.player.is_none() {
                let team = usize::from(seat.team);
                let free = (0..w.players.len()).find(|&j| {
                    w.team_of(j) == team
                        && !self
                            .seats
                            .iter()
                            .any(|s| s.player.map(usize::from) == Some(j))
                });
                self.seats[k].player = free.map(|j| j as u8);
            }
        }
        // Alone on a team, the seat goes where the world moved its pad.
        for k in 0..MAX_SEATS {
            if self.seats[k].player.is_none() {
                continue;
            }
            let team = self.seats[k].team;
            let alone = self
                .seats
                .iter()
                .filter(|s| s.player.is_some() && s.team == team)
                .count()
                == 1;
            if let (true, Some(p)) = (alone, w.person_on(usize::from(team))) {
                self.seats[k].player = Some(p as u8);
            }
        }
        (0..w.players.len())
            .map(|i| match self.seat_of(i) {
                Some(k) => self.seats[k].pad,
                None => ai::input(w, i),
            })
            .collect()
    }

    /// The world's checksum with the seats, the replays and the match's
    /// tick folded in (FNV-1a).
    pub fn checksum(&self) -> u64 {
        let mut hash = self.world.checksum();
        let mut feed = |b: u8| {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        };
        for s in &self.seats {
            feed(s.team);
            feed(s.player.unwrap_or(u8::MAX));
            s.pad.to_bytes().into_iter().for_each(&mut feed);
        }
        self.replays.encode().into_iter().for_each(&mut feed);
        self.tick.to_le_bytes().into_iter().for_each(&mut feed);
        self.burst_hold
            .to_le_bytes()
            .into_iter()
            .for_each(&mut feed);
        hash
    }

    /// The whole match as bytes: the world's snapshot, then each seat's
    /// team, player (255 for none) and last pad, the replays and the
    /// match's tick.
    pub fn to_snapshot(&self) -> Vec<u8> {
        let mut w = crate::wire::Writer::default();
        w.bytes(&self.world.to_snapshot());
        for s in &self.seats {
            w.u8(s.team);
            w.u8(s.player.unwrap_or(u8::MAX));
            w.input(&s.pad);
        }
        w.bytes(&self.replays.encode());
        w.uv(self.tick);
        w.uv(u64::from(self.burst_hold));
        w.0
    }

    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, crate::wire::DecodeError> {
        use crate::wire::DecodeError;
        let mut r = crate::wire::Reader::new(bytes);
        let world =
            World::from_snapshot(r.bytes(1 << 16)?).map_err(|_| DecodeError::Bad("world"))?;
        let mut seats = [Seat::default(); MAX_SEATS];
        for s in &mut seats {
            s.team = r.u8()?;
            s.player = match r.u8()? {
                u8::MAX => None,
                p if usize::from(p) < world.players.len() => Some(p),
                _ => return Err(DecodeError::Bad("seat player")),
            };
            s.pad = r.input()?;
        }
        let replays =
            Replays::decode(r.bytes(1 << 14)?).map_err(|_| DecodeError::Bad("replays"))?;
        let tick = r.uv()?;
        let burst_hold = u16::try_from(r.uv()?)
            .ok()
            .filter(|&h| h <= LEVEL2_BURST_HOLD)
            .ok_or(DecodeError::Bad("burst hold"))?;
        if !r.done() {
            return Err(DecodeError::Bad("match snapshot length"));
        }
        let state = Self {
            world,
            seats,
            replays,
            tick,
            burst_hold,
        };
        state.validate()?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MatchSetup;
    use street_sim::{BallState, button, math};

    fn pad(stick: [f32; 2], buttons: u16) -> Option<Input> {
        Some(Input::moving(stick).with(buttons))
    }

    #[test]
    fn seats_take_free_players_and_give_them_back() {
        let setup = MatchSetup::standard(Default::default(), 1);
        let mut m = Match::new(setup.world, setup.seat_teams);
        let mut f = Frame::default();
        f.seats[0] = pad([0.0, 0.0], 0);
        f.seats[1] = pad([0.0, 0.0], 0);
        f.seats[2] = pad([0.0, 0.0], 0);
        m.step(&f);
        // Seats 0 and 2 play for team 0 (players 0..3), seat 1 for team 1.
        let p0 = m.seats[0].player.unwrap();
        let p2 = m.seats[2].player.unwrap();
        assert_ne!(p0, p2);
        assert_eq!(m.world.team_of(p0.into()), 0);
        assert_eq!(m.world.team_of(m.seats[1].player.unwrap().into()), 1);
        f.seats[2] = None;
        m.step(&f);
        assert_eq!(m.seats[2].player, None);
        assert_eq!(m.seat_of(p2.into()), None);
    }

    #[test]
    fn same_frames_same_match() {
        let setup = MatchSetup::standard(Default::default(), 9);
        let mut a = Match::new(setup.world.clone(), setup.seat_teams);
        let mut b = a.clone();
        for t in 0..2000u32 {
            let mut f = Frame::default();
            let angle = t as f32 * 0.01;
            f.seats[0] = pad(
                [math::sin(angle), math::cos(angle)],
                if t % 40 < 3 { button::PASS } else { 0 },
            );
            f.seats[1] = pad([0.5, 0.0], if t % 90 < 2 { button::SHOOT } else { 0 });
            a.step(&f);
            b.step(&f);
        }
        assert_eq!(a.checksum(), b.checksum());
        let back = Match::from_snapshot(&a.to_snapshot()).unwrap();
        assert_eq!(back, a);
        assert_eq!(back.checksum(), a.checksum());
    }

    #[test]
    fn snapshots_reject_bad_world_and_cross_team_or_duplicate_seats() {
        let setup = MatchSetup::standard(Default::default(), 9);
        let base = Match::new(setup.world, setup.seat_teams);
        let mut bad = base.clone();
        bad.world.ball.state = BallState::Held(99);
        assert!(Match::from_snapshot(&bad.to_snapshot()).is_err());
        let mut bad = base.clone();
        bad.seats[0].team = 2;
        assert!(Match::from_snapshot(&bad.to_snapshot()).is_err());
        let mut bad = base.clone();
        bad.seats[0].player = Some(4); // opponent, even though in range
        assert!(Match::from_snapshot(&bad.to_snapshot()).is_err());
        let mut bad = base;
        bad.seats[0].player = Some(0);
        bad.seats[2].player = Some(0); // same player driven twice
        assert!(Match::from_snapshot(&bad.to_snapshot()).is_err());
    }
}
