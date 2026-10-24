//! The head's look (`FUN_00159380`, called late in the actor's update,
//! `0x157d9c`; docs/TRACE.md "The head's look").
//!
//! Each actor keeps a situation (actor+0x20c) that the play's events move
//! along a table (`0x3e7060`, eight rows the boot fills: the targets the
//! situation may look at, the events it answers and the situation each
//! leads to), a look target (+0x210) drawn among the situation's by their
//! weights and its time (+0x214, drawn between the target's bounds, −1
//! for good). The rewrite poses no head from it: what it keeps is the
//! look's place in the shared generator, two draws a new look (three for a
//! teammate's, whose slot is drawn).

use crate::{BallState, World};

/// One situation of `0x3e7060`: the targets it looks at (row +4, count
/// +0), the events it answers (+0xc, count +8) and where each leads (+0x10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Situation {
    pub targets: &'static [u8],
    pub events: &'static [u8],
    pub next: &'static [u8],
}

/// The situations as the boot fills `0x3e7060` (save state 1's RAM), their
/// lists in the executable at `0x454720..0x4548b8`.
pub const SITUATIONS: [Situation; 8] = [
    Situation {
        targets: &[0],
        events: &[7, 3, 8, 9],
        next: &[2, 5, 1, 6],
    },
    Situation {
        targets: &[1, 2, 3],
        events: &[0, 2, 7, 9, 3],
        next: &[0, 4, 2, 6, 5],
    },
    Situation {
        targets: &[4, 5, 6, 7],
        events: &[0, 1, 3, 9, 2, 8],
        next: &[0, 3, 5, 6, 4, 1],
    },
    Situation {
        targets: &[8],
        events: &[0, 8, 9],
        next: &[0, 1, 6],
    },
    Situation {
        targets: &[9],
        events: &[0, 7, 3, 9, 8],
        next: &[0, 2, 5, 6, 1],
    },
    Situation {
        targets: &[10, 11],
        events: &[0, 9, 1, 4, 8, 2],
        next: &[0, 6, 3, 2, 1, 4],
    },
    Situation {
        targets: &[12, 13, 14],
        events: &[0, 7, 8, 5],
        next: &[0, 2, 1, 7],
    },
    Situation {
        targets: &[15],
        events: &[0, 7, 8, 6],
        next: &[0, 2, 1, 6],
    },
];

/// The lists' addresses in the executable (`0x3e7060`'s pointers), for the
/// disc test: each situation's targets, events and next situations.
pub const SITUATION_LISTS: [[u32; 3]; 8] = [
    [0x454720, 0x454728, 0x454738],
    [0x454748, 0x454758, 0x454770],
    [0x454788, 0x454798, 0x4547b0],
    [0x4547c8, 0x4547d0, 0x4547e0],
    [0x4547f0, 0x4547f8, 0x454810],
    [0x454828, 0x454830, 0x454848],
    [0x454860, 0x454870, 0x454880],
    [0x454890, 0x454898, 0x4548a8],
];

/// One look target of `0x3e7100` (20 bytes): what it looks at (+0, the
/// switch at `0x43f000`), its weight (+8) and its time's bounds (+0xc,
/// +0x10; −1 for good).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub kind: u8,
    pub weight: f32,
    pub span: [i32; 2],
}

const TIMED: [i32; 2] = [60, 80];
const HELD: [i32; 2] = [-1, -1];

/// The targets as the boot fills `0x3e7100` (save state 1's RAM: every
/// weight 1, the timed ones fps to fps × 4/3).
pub const TARGETS: [Target; 16] = [
    Target {
        kind: 4,
        weight: 1.0,
        span: HELD,
    },
    Target {
        kind: 0,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 2,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 4,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 0,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 2,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 9,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 6,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 1,
        weight: 1.0,
        span: HELD,
    },
    Target {
        kind: 5,
        weight: 1.0,
        span: HELD,
    },
    Target {
        kind: 0,
        weight: 1.0,
        span: HELD,
    },
    Target {
        kind: 7,
        weight: 1.0,
        span: HELD,
    },
    Target {
        kind: 3,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 8,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 4,
        weight: 1.0,
        span: TIMED,
    },
    Target {
        kind: 5,
        weight: 1.0,
        span: HELD,
    },
];

/// The target kind that looks at a teammate whose slot it draws
/// (`0x159af0`: his own ± 1 by `r & 2`, wrapping).
pub const TEAMMATE: u8 = 6;

/// The state bits the look's events read (actor+0xa4 against +0x208): a
/// shot, a lay-up, a dunk or a leap; a swipe, the counter stance or a block
/// jump.
const MOVES: u32 = 0x8 | 0x2000 | 0x80 | 0x8000;
const REACHES: u32 = 0x40 | 0x10000 | 0x1000;

/// An actor's look (actor+0x200..+0x214).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// +0x20c.
    pub situation: u8,
    /// +0x210, of [`TARGETS`].
    pub target: u8,
    /// +0x214: updates the target has left, −1 for good.
    pub timer: i32,
    /// +0x200: the ball's state as the last update left it.
    pub last_state: i32,
    /// +0x204: who held the ball then.
    pub last_holder: Option<u8>,
    /// +0x208: the state bits as his last update left them.
    pub last_bits: u32,
}

impl Default for Look {
    /// The game's phases 0, 8 and 9 (`0x1593fc..0x15946c`).
    fn default() -> Self {
        Self {
            situation: 0,
            target: 0,
            timer: -1,
            last_state: 1,
            last_holder: None,
            last_bits: 0,
        }
    }
}

/// The look's event for state bits `bits` against the last update's
/// `last` (`0x159558..0x159700`): a move begun 3, a reach begun 5, a reach
/// over 6, a move over 4.
fn bits_event(bits: u32, last: u32) -> Option<u8> {
    let began = bits & !last;
    let ended = last & !bits;
    if began & MOVES != 0 {
        Some(3)
    } else if began & REACHES != 0 {
        Some(5)
    } else if ended & REACHES != 0 {
        Some(6)
    } else if ended & MOVES != 0 {
        Some(4)
    } else {
        None
    }
}

impl World {
    /// The state bits the look reads ([`crate::school::bits`], and bit 16
    /// for the counter stance).
    fn look_bits(&self, i: usize) -> u32 {
        let stance = matches!(
            self.players[i].action,
            crate::Action::Stealing {
                stance: Some(_),
                ..
            }
        );
        crate::school::Court::bits(self, i) | if stance { 0x10000 } else { 0 }
    }

    /// `FUN_00159380` for `i`; then actor+0x208 takes his bits
    /// (`0x157e7c`).
    pub(crate) fn head_look(&mut self, i: usize) {
        use crate::game_phase::phase;
        let bits = self.look_bits(i);
        if matches!(
            self.game_phase.phase,
            phase::PREGAME | phase::OVER | phase::WON
        ) {
            self.players[i].look = Look {
                last_bits: bits,
                ..Look::default()
            };
            return;
        }
        let mut l = self.players[i].look;
        let state = self.native_ball_state();
        let holder = self.handler();
        let me = &self.players[i];
        let mut event = None;
        if holder.is_none() && state != 4 && matches!(l.last_state, 3 | 4) {
            // The ball let go: event 0 (`0x159480..0x1594a4`).
            event = Some(0);
        } else if let Some(h) = holder.filter(|&h| Some(h as u8) != l.last_holder) {
            // A new holder: himself 3 in a move, else 7; a teammate 8, an
            // opponent 9 (`0x1594b4..0x159514`).
            event = Some(if h == i {
                if bits & MOVES != 0 { 3 } else { 7 }
            } else if self.players[h].team == me.team {
                8
            } else {
                9
            });
        } else if state == 4 && l.last_state != 4 {
            // A pass let go: to him 2, from him 1 (`0x159518..0x159548`).
            if let (BallState::Pass, Some(p)) = (self.ball.state, self.ball.pass) {
                if p.to == i {
                    event = Some(2);
                } else if p.from == i {
                    event = Some(1);
                }
            }
        }
        if event.is_none() {
            event = bits_event(bits, l.last_bits);
        }
        l.last_state = state;
        l.last_holder = holder.map(|h| h as u8);
        // The situation's answer to the event (`0x159724..0x159768`).
        let mut changed = false;
        if let Some(e) = event {
            let s = &SITUATIONS[usize::from(l.situation)];
            if let Some(k) = s.events.iter().position(|&x| x == e) {
                l.situation = s.next[k];
                changed = true;
            }
        }
        let pick = if changed {
            true
        } else if l.timer == -1 {
            false
        } else {
            l.timer -= 1;
            l.timer == 0
        };
        if pick {
            // The weighted draw among the situation's targets, the one
            // looked at left out when only the time ran out, chosen from
            // the last (`0x15978c..0x159920`).
            let s = &SITUATIONS[usize::from(l.situation)];
            let eligible = |t: u8| changed || t != l.target;
            let total: f32 = s
                .targets
                .iter()
                .filter(|&&t| eligible(t))
                .map(|&t| TARGETS[usize::from(t)].weight)
                .sum();
            let unit = |r: u32| (r & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
            let goal = total * unit(self.rng.next_u32());
            let mut left = total;
            let mut k = s.targets.len() as i32 - 1;
            while k >= 0 {
                let t = s.targets[k as usize];
                if eligible(t) {
                    left -= TARGETS[usize::from(t)].weight;
                    if left <= goal {
                        break;
                    }
                }
                k -= 1;
            }
            l.target = s.targets[k.max(0) as usize];
            // Its time (`0x159924..0x1599c0`).
            let [lo, hi] = TARGETS[usize::from(l.target)].span;
            let u = unit(self.rng.next_u32());
            l.timer = lo + (u * (hi - lo) as f32) as i32;
            if TARGETS[usize::from(l.target)].kind == TEAMMATE {
                self.rng.next_u32();
            }
        }
        l.last_bits = bits;
        self.players[i].look = l;
    }
}

#[cfg(test)]
impl World {
    /// Every look settled where the play stands (no event and no time
    /// running), for tests that count another move's draws.
    pub(crate) fn settle_looks(&mut self) {
        let (state, holder) = (self.native_ball_state(), self.handler());
        for i in 0..self.players.len() {
            let bits = self.look_bits(i);
            let l = &mut self.players[i].look;
            l.last_state = state;
            l.last_holder = holder.map(|h| h as u8);
            l.last_bits = bits;
            l.timer = -1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_begun_and_over_and_a_reach_are_the_looks_events() {
        assert_eq!(bits_event(0x8, 0), Some(3));
        assert_eq!(bits_event(0, 0x80), Some(4));
        assert_eq!(bits_event(0x40, 0), Some(5));
        assert_eq!(bits_event(0, 0x1000), Some(6));
        assert_eq!(bits_event(0x10, 0), None);
        // A move begun counts before a reach over.
        assert_eq!(bits_event(0x8, 0x40), Some(3));
    }
}
