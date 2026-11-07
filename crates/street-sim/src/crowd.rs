//! The crowd's calls inside the game's update. The sound listener's crowd
//! call (`FUN_001a75a8`, through `FUN_0030d618`) reaches `FUN_0030ca10`
//! at once, in the update that posted the listener's message: it picks
//! `trunc(groups × share)` distinct groups, each `(r + left) % left` of
//! the groups still free on the shared generator's whole 32-bit value
//! (`0x30ca88`), and gives them the call (a cheer, or for 0x29 a gasp).
//! The picks are the shared generator's draws in the middle of the
//! update, before the actors after the event draw theirs (docs/TRACE.md
//! "The court's effects, draw for draw": 40 of the 65 picks in the third
//! attract's log are followed by gameplay draws in the same update), so
//! the simulation makes them at the event and hands the renderer only
//! which groups they chose ([`World::crowd`]).
//!
//! Since every pick finds a free group (there are never more picks than
//! groups), a call's draws depend only on the court's group count and the
//! share; the groups' timers and nodes stay the renderer's.
//!
//! The calls, by the listener slots `street-game`'s `sfx.rs` maps:
//!
//! | Event | Listener | Call |
//! |---|---|---|
//! | a basket | slot 26 `FUN_001a3b10` (via `FUN_001a7708`) | 0x27 |
//! | a gamebreaker basket, also | `FUN_001a3b10` → `FUN_001a6b80` | 0x27 |
//! | a passout's second leg | slot 39 `FUN_001a5d50` | 0x25 |
//! | a block or goaltend | slot 16 `FUN_001a4ce8` | 0x27 |
//! | a held-ball steal or a counter | slot 15 `FUN_001a63e0` | 0x27 |
//! | a loose ball picked up | slot 17 `FUN_001a65c0` | 0x27 |
//! | a trick that fakes a defender out | slot 13 `FUN_001a5a78` | 0x29 |
//! | the tip-off's take | slot 22 `FUN_001a5d10` | 0x25 |
//!
//! Slot 76's floor contacts (`FUN_001a5348`, 0x29): the posed joint 10
//! falling to the floor in the actor's update ([`World::floor_contact`]).

use crate::{Event, Goal, World};

/// One call: cheer or gasp, and the groups it picked, each the index
/// among the groups still free when it was drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub cheer: bool,
    pub picks: Vec<u8>,
}

/// The court's crowd as the game's update sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crowd {
    /// The crowd's groups (none in the School and on a court without
    /// spectators): the highest seat group + 1.
    pub groups: u8,
    /// This update's calls, in the order the update made them.
    pub calls: Vec<Call>,
}

/// `FUN_001a75a8`'s share of the crowd: `FUN_001a7748`'s lead over the
/// points to win, half of it plus a half, at most all (21 when the goal
/// has no points).
pub fn fraction(lead: u32, to_win: u32) -> f32 {
    let to_win = if to_win > 0 { to_win } else { 21 };
    ((lead as f32 / to_win as f32) * 0.5 + 0.5).min(1.0)
}

impl World {
    /// The share of the crowd a call reaches ([`fraction`]): the leading
    /// score, by the goal's own points.
    pub fn crowd_fraction(&self) -> f32 {
        let (lead, to_win) = match self.rules.goal {
            Goal::Points(n) => (self.score[0].max(self.score[1]), n),
            Goal::TrickPoints(n) => (
                self.teams[0].trick_points.max(self.teams[1].trick_points),
                n,
            ),
            Goal::Gamebreaker(_) => (0, 0),
        };
        fraction(lead, to_win)
    }

    /// Slot 76's poster in `i`'s update (`FUN_00156f48`, class
    /// `DAT_003c6930`): his posed joint 10 falling from above 1.5 to 1.5 or
    /// below since his last update gasps the crowd (`FUN_001a5348`'s
    /// `FUN_001a75a8(.., 0x29)`), its picks the shared generator's in his
    /// update (the third attract match's blocker knocked out of the air
    /// lands at update 1004 and five groups gasp). Not under a staged
    /// sequence (the Script Manager's states 3..7).
    pub(crate) fn floor_contact(&mut self, i: usize) {
        if self.poses.get().is_none() || self.sequence.is_some() {
            self.players[i].floor_joint = None;
            return;
        }
        let now = self.actor_pose(i).map(|p| p.joints[10].position[1]);
        let before = std::mem::replace(&mut self.players[i].floor_joint, now);
        if let (Some(before), Some(now)) = (before, now)
            && before > 1.5
            && now <= 1.5
        {
            self.crowd_call(false);
        }
    }

    /// `FUN_0030ca10`: the call's picks, drawn now.
    pub fn crowd_call(&mut self, cheer: bool) {
        let groups = self.crowd.groups;
        if groups == 0 {
            return;
        }
        // cvt.w.s truncates.
        let count = (f32::from(groups) * self.crowd_fraction()) as i32;
        let mut left = u32::from(groups);
        let mut picks = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count.max(0) {
            if left == 0 {
                break;
            }
            let k = self.rng.next_u32().wrapping_add(left) % left;
            picks.push(k as u8);
            left -= 1;
        }
        self.crowd.calls.push(Call { cheer, picks });
    }

    /// `event` as the update posts it: into the tick's events, and the
    /// listener's crowd call for it, at once. The basket's call comes
    /// after its other listeners' draws ([`World::score_basket`]) and the
    /// tip's at the take, before its pass draws ([`World::tip`]'s take).
    pub(crate) fn post(&mut self, event: Event) {
        let calls = match event {
            Event::PassoutBounce { .. }
            | Event::Block { .. }
            | Event::Goaltend { .. }
            | Event::Steal {
                intercepted_pass: false,
                ..
            }
            | Event::Counter { .. }
            | Event::Rebound { .. } => 1,
            _ => 0,
        };
        let gasp = matches!(
            event,
            Event::Trick {
                fakeout: Some(_),
                ..
            }
        );
        self.events.push(event);
        for _ in 0..calls {
            self.crowd_call(true);
        }
        if gasp {
            self.crowd_call(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_draws_its_share_of_the_groups() {
        let mut w = World::new(
            crate::tests::hoops(),
            vec![crate::Player::new(0, [0.0, 0.0], 0.0)],
            crate::Rules::default(),
            7,
        );
        w.crowd.groups = 10;
        let before = w.rng.clone();
        w.crowd_call(true);
        // No score yet: half of ten.
        let mut g = before;
        let want: Vec<u8> = (0..5u32)
            .map(|i| (g.next_u32().wrapping_add(10 - i) % (10 - i)) as u8)
            .collect();
        assert_eq!(
            w.crowd.calls,
            vec![Call {
                cheer: true,
                picks: want
            }]
        );
        assert_eq!(w.rng.state(), g.state());
    }

    #[test]
    fn no_crowd_draws_nothing() {
        let mut w = World::new(
            crate::tests::hoops(),
            vec![crate::Player::new(0, [0.0, 0.0], 0.0)],
            crate::Rules::default(),
            7,
        );
        let before = w.rng.state();
        w.crowd_call(false);
        assert!(w.crowd.calls.is_empty());
        assert_eq!(w.rng.state(), before);
    }

    #[test]
    fn the_share_follows_the_lead() {
        assert_eq!(fraction(0, 21), 0.5);
        assert_eq!(fraction(21, 21), 1.0);
        assert_eq!(fraction(30, 21), 1.0);
        assert_eq!(fraction(7, 0), 7.0 / 21.0 * 0.5 + 0.5);
    }
}
