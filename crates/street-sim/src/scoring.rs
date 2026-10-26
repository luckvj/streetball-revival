//! The game's scoring messages, as its listeners take them: each message
//! (posted with `FUN_00190d90`; the message's own vtable picks the
//! listener's entry, the "slot") runs the combo manager's dispatcher
//! (game+0xd0, vtable `0x43c208`), then the two meters' listeners (game
//! +0xc8/+0xcc, vtable `0x43df50`), then the statistics' (game+0xd4,
//! vtable `0x440578`). The combo dispatcher goes first: it pushes the ring,
//! runs the matcher where it does and writes the values (the trick's
//! +0x24, the passout's +0x5c) the other listeners then read, and every
//! meter and statistics listener adds the matcher's total (`+0x150`, which
//! lives one update, [`combo::Manager::update`]) to its own value. See
//! `docs/TRACE.md` "The combo ring, the meter's listeners and the
//! statistics".
//!
//! | Slot | Posted by | Ring (`0x43c208`) | Meter (`0x43df50`) | Statistics (`0x440578`) |
//! |---|---|---|---|---|
//! | 5 | callback 20, a pump fake (`FUN_00172348`) | arms 13 (`+0x160`) | arms 73 (`+0x4c`) | arms 73 (`+0xa0c`) |
//! | 9 | a pass thrown (`FUN_0014bff8`) | the thrower in the air (`+0x154`) | | |
//! | 13 | a trick move lands (`0x157e58`) | class 10, 11 fakeout (`FUN_001438b8`) | total + the value; the victim 0 | 11, 0x16/0x17 |
//! | 15 | the take: a steal, a trick counter (`FUN_0014cb28`) | class 20, 21 counter; matcher | total + 1,000/3,000; the victim −1,000/−3,000 | 5, 11, 0x1d/0x26 |
//! | 16 | a block (`FUN_0014c408`, the take) | class 19; matcher | total + 2,500; the shooter −2,500 | 6, 11, 0x1c, the blocker's attempt |
//! | 17 | the take: a rebound | clears, class 16 | total + 500 | 4, 11, 0x24 |
//! | 19 | the take: a pass caught | classes 8/9/14 by the throw, 17 a passout special | the passout's 3,000 | 11 + the passout's value |
//! | 24 | every take | class 15, a dive | total + 500 for a dive; the repeat ring per possession | 11 + 500 for a dive |
//! | 26 | a basket (`FUN_001515a8`) | classes 0..7; matcher; the ring goes next update | total + the basket's value | 0..3, 7, 8, 11..13 |
//! | 71 | a cancel | class 18 | (the pockets) | 11 + 5,000 |
//! | 73 | a defender jumps at a pump fake (`FUN_0013f950`) | class 13 | total + 500 | 11 + 500 |
//! | 74 | a contact (`FUN_001343e8`): kind 3, a screen for the play's picker | class 12 | total + 500 | 11 + 500 |

use crate::{Action, Event, World, combo, distance, game_phase::ball, stats, target_hoop};

/// `0x43e1f8`: the meter listeners' (and statistics' `0x440838`) values by
/// kind: 2/3 long and short shots, 4 a lay-up, 5 a dunk, 6 an alley-oop,
/// 7 a block, 8 a steal, 13 a tip slam, 14 a game-winning dunk, 16 a trick
/// counter.
pub const METER_VALUES: [i32; 20] = [
    500, 1_000, 750, 250, 1_000, 1_500, 3_000, 2_500, 1_000, 25_000, 500, 500, 500, 2_000, 25_000,
    500, 3_000, 3_000, 5_000, 0,
];

/// `0x43e248` (and the statistics' `0x440888`): what the other side's
/// meter loses by kind: a block 2,500, a steal 1,000, a trick counter
/// 3,000, nothing else.
pub const METER_DRAINS: [i32; 20] = [
    0, 0, 0, 0, 0, 0, 0, 2_500, 1_000, 0, 0, 0, 0, 0, 0, 0, 3_000, 0, 0, 0,
];

/// `0x43df38` (`FUN_001439c8`): a dunk's value by its finish
/// (`actor+0x24c`).
pub const DUNK_FINISH: [i32; 5] = [1_000, 2_000, 3_000, 5_000, 7_500];

/// `0x440520` (`FUN_00187200`): the side statistic each ring class's value
/// goes to when a combo pays.
pub const CLASS_STATS: [usize; 22] = [
    0x1a, 0x1b, 0x19, 0x18, 0x27, 0x22, 0x23, 0x1e, 0x27, 0x27, 0x16, 0x17, 0x1f, 0x20, 0x27, 0x21,
    0x24, 0x25, 0x27, 0x1c, 0x1d, 0x26,
];

/// `DAT_004b7168` = `0x4b7150[6]` and `DAT_004ba470` (25,000 in RAM): a
/// game-winning dunk's own 25,000 on top of a gamebreaker's value.
pub const WINNER_BONUS: i32 = 25_000;

/// `DAT_004b84ac`: the updates a held ball keeps its last passer as the
/// assister (fps × 2, 120 in RAM).
pub const ASSIST_UPDATES: u16 = 2 * crate::TICK_HZ as u16;

/// What the take (`FUN_0014cb28`) reads before the ball is the taker's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Take {
    pub taker: usize,
    /// The ball's state (ball+0x180).
    pub state: i32,
    /// Its last holder let go of it (+0xfc): a pass's thrower, a shot's
    /// shooter.
    pub previous: Option<usize>,
    /// The side with the ball (game+0x110).
    pub possession: usize,
    /// The taker's catch is a dive's (`FUN_0016eab0`).
    pub diving: bool,
    /// Ball+0x1ac: a passout special's pass (a pass to oneself).
    pub passout: bool,
    /// The holder is in a shot, a dunk, a lay-up or a leap (bits 3, 7,
    /// 13, 15): his ball taken is a block.
    pub shooting: bool,
    /// A loose ball's pickup by the side without it is a steal.
    pub loose_steal: bool,
}

/// A basket as the hoop posts it (slot 26, `0x3c6620`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasketPost {
    /// +4: the scoring side's last holder.
    pub shooter: usize,
    /// +8: the assister, the ball's +0x10c when on the scoring side.
    pub assister: Option<usize>,
    /// +0xc: the shot's base, 2 or 3 (beyond the arc).
    pub base: i32,
    /// +0x10: the ball's kind (+0x184): 2 a jump shot, 3 an alley-oop's
    /// finish, 4 a dunk, 5 a lay-up.
    pub kind: i32,
    /// Ball+0x178: the gamebreaker's level, 0 for none.
    pub gamebreaker: u8,
    /// The game's vt+0x7c (`FUN_0012e890`): the basket wins the game.
    pub wins: bool,
    /// Actor+0x1e4 == 1: a tip slam.
    pub tip_slam: bool,
    /// Actor+0x24c: a dunk's finish, 0..4.
    pub finish: i32,
    /// Actor+0x270 (`FUN_00171e28`): an alley-oop's style bits.
    pub style: i32,
}

impl World {
    /// The game's count (game+0x128) as this update's listeners read it:
    /// the game's update moves it on before the actors post anything.
    pub(crate) fn scoring_now(&self) -> i32 {
        self.tick as i32 + 1
    }

    /// The script manager in states 3..7: a staged sequence.
    fn scripted(&self) -> bool {
        self.sequence.is_some()
    }

    /// The holder's state bits the combo manager tests: 3 a jump shot, 7 a
    /// dunk, 10 a trick move, 13 a lay-up, 15 an alley-oop's leap.
    pub(crate) fn holder_state(&self) -> Option<u32> {
        let h = self.handler()?;
        let action = &self.players[h].action;
        let trick = if action.juking() { 0x400 } else { 0 };
        Some(crate::block::holder_bits(action) | trick)
    }

    /// `FUN_00131560`, at the top of each update.
    pub(crate) fn combo_update(&mut self) {
        let scene = combo::Scene {
            tick: self.tick as i32,
            ball: self.native_ball_state(),
            holder: self.holder_state(),
            staged: self.scripted(),
        };
        self.combo.update(scene);
    }

    /// `FUN_00186fa8`: whether the statistics take `stat` now — not before
    /// the game starts nor once it is won (phases 0 and 9), and in a staged
    /// sequence only the list it keeps.
    fn stats_open(&self, stat: usize) -> bool {
        if matches!(self.game_phase.phase, 0 | 9) {
            return false;
        }
        !self.scripted() || matches!(stat, 0..=2 | 7..=0xd | 0x18..=0x1b | 0x1e | 0x22 | 0x23)
    }

    /// `FUN_00186ef8` for an actor and his side (scope 0 and 1).
    fn stat(&mut self, player: usize, stat: usize, amount: i32) {
        if self.stats_open(stat) {
            let team = self.players[player].team;
            self.stats.add(player, team, stat, amount);
        }
    }

    /// `FUN_00186ef8` for a side (scope 1, or the side-only 0x10..0x27).
    fn side_stat(&mut self, team: usize, stat: usize, amount: i32) {
        if self.stats_open(stat) {
            self.stats.add_team(team, stat, amount);
        }
    }

    /// Stat 11 for an actor and his side, which the side's trick points
    /// (and the Trick Point game, `FUN_0018c788`) follow. Reaching the
    /// target wins nothing by itself: the trick-point game is won at a
    /// basket ([`World::basket_would_win`]).
    pub(crate) fn trick_points(&mut self, player: usize, amount: i32) {
        if amount == 0 || !self.stats_open(stats::TRICK_POINTS) {
            return;
        }
        let team = self.players[player].team;
        self.stats.add(player, team, stats::TRICK_POINTS, amount);
        let t = &mut self.teams[team];
        t.trick_points = (t.trick_points as i64 + i64::from(amount)).max(0) as u32;
    }

    /// `FUN_00187200`: a combo that paid this update credits each walked
    /// event's value to its class's side statistic (a game-winning dunk
    /// with a gamebreaker splits off its 25,000).
    fn combo_breakdown(&mut self, team: usize, gamebreaker: bool) {
        if self.combo.row < 0 {
            return;
        }
        let kinds = combo::PATTERNS[self.combo.row as usize];
        let walked = kinds.iter().filter(|&&k| k != -1).count();
        for k in 6 - walked..6 {
            let slot = self.combo.ring.slots[k];
            let value = slot.paid_value();
            if slot.class == 6 && gamebreaker {
                self.side_stat(team, 0x23, WINNER_BONUS);
                self.side_stat(team, 0x1e, value - WINNER_BONUS);
            } else if let Some(&stat) = CLASS_STATS.get(slot.class as usize) {
                self.side_stat(team, stat, value);
            }
        }
    }

    /// The meter's fill for `team` with the matcher's total.
    fn fill_with_total(&mut self, team: usize, value: i32) {
        let amount = self.combo.total.max(0) + value;
        self.fill_meter(team, amount as f32);
    }

    /// The combo's payout as the HUD shows it (`Event::Combo`).
    fn combo_paid(&mut self, team: usize, payout: Option<combo::Payout>) {
        let Some(payout) = payout else {
            return;
        };
        if payout.total <= 0 {
            return;
        }
        let walked = self.combo.ring.walked(&payout);
        let newest = self.combo.ring.slots[5].paid_value();
        let combo = std::mem::take(&mut self.teams[team].combo);
        self.events.push(Event::Combo {
            team,
            points: combo.points * combo.multiplier.max(1),
            multiplier: combo.multiplier,
            walked,
            paid: payout.total.max(0) as u32,
            newest,
        });
    }

    /// Push `class` for `team` (`FUN_00131738`, then `FUN_00131788`).
    fn ring_push(&mut self, team: usize, class: i32, extra: i32, value: i32) {
        let now = self.scoring_now();
        let staged = self.scripted();
        self.combo.own(team);
        self.combo
            .push(combo::Slot::new(class, extra, now, value), staged);
    }

    /// Slot 5: a pump fake (callback 20). It arms the pump fake's points.
    pub(crate) fn post_pump_fake(&mut self) {
        self.combo.pump_fake = true;
        for t in &mut self.teams {
            t.pump_fake = true;
        }
        self.stats.pump_fake = true;
    }

    /// Slot 9 (`FUN_00131d88`): a pass leaves `passer`'s hands.
    pub(crate) fn post_pass_thrown(&mut self, passer: usize) {
        self.combo.air = if self.players[passer].height > 0.0 {
            if self.combo.selected { 2 } else { 1 }
        } else {
            0
        };
    }

    /// Slot 13: `actor`'s trick move landed (`faked`: actor+0x218). The
    /// ring takes class 10, or 11 for a fakeout, with the move's value
    /// class and `FUN_001438b8`'s value; the mover's meter that value with
    /// the total; his trick points the value. Returns the value.
    pub(crate) fn post_trick(
        &mut self,
        actor: usize,
        value_class: usize,
        script: i32,
        faked: bool,
    ) -> u32 {
        let team = self.players[actor].team;
        let value = self.trick_value(team, value_class, script, faked);
        let class = if faked { 11 } else { 10 };
        self.ring_push(team, class, value_class as i32, value);
        self.fill_with_total(team, value);
        if self.rules.tricks {
            self.trick_points(actor, value);
            self.side_stat(team, if faked { 0x17 } else { 0x16 }, value);
            self.side_stat(1 - team, 0x10, 0);
        }
        value.max(0) as u32
    }

    /// `FUN_001438b8`: the ladder `0x43df20` by the value class, halved
    /// unless faked (actor+0x218); classes 2..4 count their script in the
    /// side's last four (team+0x2c) and halve per repeat, round up, at
    /// least 1, and push it.
    fn trick_value(&mut self, team: usize, value_class: usize, script: i32, faked: bool) -> i32 {
        let base = crate::street::points::TRICK[value_class.min(4)] as i32;
        let base = if faked { base } else { base / 2 };
        if value_class < 2 {
            return base;
        }
        self.repeat_discount(team, script, base)
    }

    /// The repeat ring's count and push (`FUN_001438b8`, `FUN_00143a88`):
    /// `value × 0.5^n` for the `n` copies of `key` in the side's last four,
    /// rounded up, at least 1.
    fn repeat_discount(&mut self, team: usize, key: i32, value: i32) -> i32 {
        let t = &mut self.teams[team];
        let repeats = t.recent_moves.iter().filter(|&&k| k == key).count() as u32;
        t.recent_moves.rotate_right(1);
        t.recent_moves[0] = key;
        let scaled = value as f32 * 0.5f32.powi(repeats as i32);
        let mut out = scaled as i32;
        if (out as f32) < scaled {
            out += 1;
        }
        out.max(1)
    }

    /// Slot 15: `taker` took the ball from the other side (a steal; a
    /// trick counter when he is in the counter stance, `FUN_0016e9b0`). The
    /// ring takes class 20 or 21 and runs the matcher; the taker's meter
    /// gets the total and 1,000 or 3,000, the other side's loses as much;
    /// the taker gets a steal and the trick points.
    pub(crate) fn post_steal(&mut self, taker: usize) {
        let team = self.players[taker].team;
        let counter = matches!(
            self.players[taker].action,
            Action::Stealing { counter: true, .. }
        );
        let kind = if counter { 16 } else { 8 };
        let now = self.scoring_now();
        self.ring_push(team, if counter { 21 } else { 20 }, -1, -1);
        self.combo.pushed = true;
        let payout = self.combo.evaluate(now, if counter { 21 } else { 20 });
        self.fill_with_total(team, METER_VALUES[kind]);
        self.fill_meter(1 - team, -(METER_DRAINS[kind] as f32));
        for t in &mut self.teams {
            t.paid = true;
        }
        self.stat(taker, stats::STEALS, 1);
        if self.rules.tricks {
            self.trick_points(taker, self.combo.total.max(0) + METER_VALUES[kind]);
            self.side_stat(team, if counter { 0x26 } else { 0x1d }, METER_VALUES[kind]);
            self.combo_breakdown(team, false);
            self.side_stat(1 - team, 0x10, METER_DRAINS[kind]);
        }
        self.stats.paid = true;
        self.combo_paid(team, payout);
    }

    /// Slot 16: `blocker` blocked a shot. The ring takes class 19 and runs
    /// the matcher; the blocker's meter gets the total and 2,500, the
    /// shooter's loses 2,500; the blocker gets the block and the trick
    /// points, and a shot tried when the ball's holder is in a shot, a dunk,
    /// a lay-up or a leap (`FUN_00188868`'s tail).
    pub(crate) fn post_block(&mut self, blocker: usize, shooting: bool) {
        let team = self.players[blocker].team;
        let now = self.scoring_now();
        self.ring_push(team, 19, -1, -1);
        self.combo.pushed = true;
        let payout = self.combo.evaluate(now, 19);
        self.fill_with_total(team, METER_VALUES[7]);
        self.fill_meter(1 - team, -(METER_DRAINS[7] as f32));
        for t in &mut self.teams {
            t.paid = true;
        }
        self.stat(blocker, stats::BLOCKS, 1);
        if self.rules.tricks {
            self.trick_points(blocker, self.combo.total.max(0) + METER_VALUES[7]);
            self.side_stat(team, 0x1c, METER_VALUES[7]);
            self.combo_breakdown(team, false);
            self.side_stat(1 - team, 0x10, METER_DRAINS[7]);
        }
        self.stats.paid = true;
        if shooting {
            self.stat(blocker, stats::ATTEMPTS, 1);
        }
        self.combo_paid(team, payout);
    }

    /// Slot 17: `taker` took a rebound. The ring is cleared and takes class
    /// 16; his meter gets the total and 500; he gets the rebound and the
    /// trick points.
    pub(crate) fn post_rebound(&mut self, taker: usize) {
        let team = self.players[taker].team;
        self.combo.ring.clear();
        self.ring_push(team, 16, -1, -1);
        self.combo.pushed = true;
        self.fill_with_total(team, 500);
        for t in &mut self.teams {
            t.paid = true;
        }
        self.stat(taker, stats::REBOUNDS, 1);
        self.events.push(Event::Rebound { player: taker });
        if self.rules.tricks {
            self.trick_points(taker, self.combo.total.max(0) + 500);
            self.side_stat(team, 0x24, 500);
            self.combo_breakdown(team, false);
            self.side_stat(1 - team, 0x10, 0);
        }
        self.stats.paid = true;
    }

    /// Slot 19: `receiver` caught `passer`'s pass (`passout`: ball+0x1ac, a
    /// passout special). The first pass the manager hears is skipped
    /// (+0x16c). A passout pushes class 17 with `FUN_00143a88`'s value;
    /// a pass from the floor pushes class 9 only for a kick pass (pass kind
    /// 5), one from the air class 8, or 14 to a receiver in a dunk or a
    /// leap when +0x164 is set. A receiver in an alley-oop leap whose move
    /// has flag 0x800 counts nothing. The passout's 3,000 fills the
    /// receiver's meter; its value is the receiver's trick points.
    pub(crate) fn post_pass_caught(&mut self, passer: usize, receiver: usize, passout: bool) {
        let team = self.players[receiver].team;
        let leaping_0x800 = self.players[receiver].leaping() && self.advanced_move(receiver);
        let mut passout_value = 0;
        if self.combo.ready && !leaping_0x800 {
            let pushed = if passout {
                passout_value = self.passout_value(team, passer);
                let extra = i32::from(passout_value != 3_000);
                Some((17, extra, passout_value))
            } else {
                match self.combo.air {
                    0 if self.players[passer].pass_kind == 5 => Some((9, -1, -1)),
                    0 => None,
                    2 if self.players[receiver].leaping()
                        || matches!(self.players[receiver].action, Action::Dunking { .. }) =>
                    {
                        Some((14, -1, -1))
                    }
                    _ => Some((8, -1, -1)),
                }
            };
            if let Some((class, extra, value)) = pushed {
                self.ring_push(team, class, extra, value);
                self.combo.pushed = true;
            }
        }
        self.combo.ready = true;
        if passout && !leaping_0x800 {
            self.fill_meter(team, 3_000.0);
            self.teams[team].paid = true;
            if self.rules.tricks {
                self.trick_points(receiver, passout_value);
                self.side_stat(team, 0x25, passout_value);
                self.side_stat(1 - team, 0x25, 0x7465);
            }
        }
    }

    /// `FUN_00143a88`: a passout special's value, 3,000 (5,000 when the
    /// passout's state `FUN_00142038` is 2), discounted by
    /// its key −3 (−2) in the side's repeat ring like a trick's.
    fn passout_value(&mut self, team: usize, passer: usize) -> i32 {
        if self.passout.is_some_and(|po| {
            po.passer == passer && po.tossed && po.kind == crate::passout::Kind::Head
        }) {
            self.repeat_discount(team, -2, 5_000)
        } else {
            self.repeat_discount(team, -3, 3_000)
        }
    }

    /// The receiver's move has flag 0x800 (`FUN_00171de8`): the advanced
    /// dunk or leap of the slot his turbos pick.
    pub fn advanced_move(&self, i: usize) -> bool {
        let p = &self.players[i];
        p.native_dunks.is_some_and(|mask| {
            crate::player::NativeTricks::slot(p.action_buttons)
                .is_some_and(|slot| mask & (1 << slot) != 0)
        })
    }

    /// Slot 24: every take, `holder` now holding the ball (`diving`: his
    /// catch is a dive's, `FUN_0016eab0`). With nothing pushed since the
    /// last take the ring takes a dive's class 15; the meter's and the
    /// statistics' dive 500 likewise only without a steal, block, rebound
    /// or passout this take. The meter's repeat ring is cleared when the
    /// possession has changed since its last look.
    pub(crate) fn post_catch(&mut self, holder: usize, diving: bool) {
        let team = self.players[holder].team;
        if !self.combo.pushed && diving {
            self.ring_push(team, 15, -1, -1);
        }
        let bits = crate::block::holder_bits(&self.players[holder].action);
        self.combo.selected = bits & 0x8080 != 0;
        self.combo.pushed = false;
        for side in 0..2 {
            if !self.teams[side].paid && diving && side == team {
                self.fill_with_total(side, 500);
            }
            self.teams[side].paid = false;
            let possession = self.possession as i32;
            let t = &mut self.teams[side];
            if t.possession_seen != possession {
                t.possession_seen = possession;
                t.recent_moves = [-1; 4];
            }
        }
        if !self.stats.paid && diving && self.rules.tricks {
            self.trick_points(holder, 500);
            self.side_stat(team, 0x21, 500);
            self.side_stat(1 - team, 0x10, 500);
        }
        self.stats.paid = false;
    }

    /// Slot 26: a basket (`FUN_001322d0`, `FUN_001442b8`, `FUN_00187c10`).
    pub(crate) fn post_basket(&mut self, post: BasketPost) {
        let team = self.players[post.shooter].team;
        let game_type = self.rules.game_type();
        let wide = post.kind == 3 || post.kind == 4;
        // The ring: its class and value.
        let mut class = -1;
        let mut value = -1;
        let mut extra = -1;
        if post.wins && wide && game_type != 1 {
            class = 6;
            // `FUN_001f6d98` (the same slot): the winning dunk tears the
            // rim off on the ball's side.
            self.rims[crate::rim::post_side(self.ball.position[0])].tear();
        }
        if post.gamebreaker != 0 {
            value = gamebreaker_value(post.gamebreaker, post.kind, post.base);
            if class == 6 {
                value += WINNER_BONUS;
            } else {
                class = 7;
            }
        }
        if class == -1 {
            match post.kind {
                3 => {
                    class = 1;
                    value = 3_000
                        + if post.style & 1 != 0 { 1_000 } else { 0 }
                        + if post.style & 2 != 0 { 2_000 } else { 0 };
                    extra = (post.style >> 1) & 1;
                }
                4 if post.tip_slam => class = 5,
                4 => {
                    class = 0;
                    value = DUNK_FINISH[post.finish.clamp(0, 4) as usize];
                    extra = post.finish;
                }
                5 => class = 2,
                _ => class = if post.base == 3 { 3 } else { 4 },
            }
        } else if class == 7 {
            extra = if wide { 0 } else { 4 } + i32::from(post.gamebreaker == 2);
        }
        let now = self.scoring_now();
        self.ring_push(team, class, extra, value);
        let payout = self.combo.evaluate(now, class);
        self.combo.basket = true;
        // The meters (`FUN_001442b8`): in a dunks-only game only dunks and
        // alley-oops; a gamebreaker's basket scales the scorer's next fills
        // instead; a winning basket fills nothing.
        let counted = game_type != 2 || wide;
        // The meter's own kind for the basket (and the statistics').
        let meter_kind = match post.kind {
            3 => 6,
            4 if post.tip_slam => 13,
            4 => 5,
            5 => 4,
            _ if post.base == 3 => 2,
            _ => 3,
        };
        let own_value = match meter_kind {
            5 => DUNK_FINISH[post.finish.clamp(0, 4) as usize],
            6 => value,
            k => METER_VALUES[k],
        };
        if counted {
            if post.gamebreaker != 0 {
                let t = &mut self.teams[team];
                t.meter_scale *= t.meter_sequence_fill;
            } else if !post.wins {
                let amount = self.combo.total.max(0) + own_value;
                if amount != 0 {
                    self.fill_meter(team, amount as f32);
                }
            }
        }
        // The statistics' trick points (`FUN_00187c10`), after its counts
        // ([`World::score_basket`] keeps those).
        if counted && self.rules.tricks && self.stats_open(stats::TRICK_POINTS) {
            let mut kind: i32 = -1;
            let mut bonus = 0;
            let mut split = false;
            if post.gamebreaker != 0 {
                kind = 9;
            }
            if post.wins && wide && game_type != 1 {
                if kind == 9 {
                    split = true;
                    bonus = WINNER_BONUS;
                } else {
                    kind = 14;
                }
            }
            if kind == -1 {
                kind = meter_kind as i32;
            }
            let own = match kind {
                9 => value - if split { WINNER_BONUS } else { 0 },
                5 => DUNK_FINISH[post.finish.clamp(0, 4) as usize],
                6 => own_value,
                k => METER_VALUES[k as usize],
            };
            let paid = bonus + self.combo.total.max(0) + own;
            if paid != 0 {
                self.trick_points(post.shooter, paid);
                if split {
                    self.side_stat(team, 0x1e, own);
                    self.side_stat(team, 0x23, WINNER_BONUS);
                } else {
                    self.side_stat(team, kind as usize + 0x16, paid - self.combo.total.max(0));
                }
                self.combo_breakdown(team, post.gamebreaker != 0);
            }
            self.side_stat(1 - team, 0x10, METER_DRAINS[kind as usize]);
        }
        if counted && let Some(assister) = post.assister {
            self.stat(assister, stats::ASSISTS, 1);
        }
        self.combo_paid(team, payout);
    }

    /// Slot 71: `team` cancelled the other side's pocketed gamebreaker.
    pub(crate) fn post_cancel(&mut self, team: usize, actor: usize) {
        self.ring_push(team, 18, 0, -1);
        if self.stats_open(stats::TRICK_POINTS) {
            self.trick_points(actor, 5_000);
        }
    }

    /// Slot 73: defender `defender` jumped at a pump fake: the holder's
    /// side (the other side of the defender's) gets the pump fake's 500.
    pub(crate) fn post_pump_fake_points(&mut self, defender: usize) {
        self.events.push(Event::PumpFakeBite { player: defender });
        let team = 1 - self.players[defender].team;
        if self.combo.pump_fake {
            self.combo.pump_fake = false;
            self.ring_push(team, 13, -1, -1);
        }
        if self.teams[team].pump_fake {
            self.fill_with_total(team, 500);
        }
        for t in &mut self.teams {
            t.pump_fake = false;
        }
        if self.stats.pump_fake {
            self.stats.pump_fake = false;
            if let Some(h) = self.handler()
                && self.rules.tricks
            {
                let side = self.players[h].team;
                self.trick_points(h, 500);
                self.side_stat(side, 0x20, 500);
                self.side_stat(1 - side, 0x10, 0);
            }
        }
    }

    /// Slot 74: a contact between `a` and `b` of `kind` (3: a screen set by
    /// the holder's side's picker, `FUN_001342d0`): the screener's side
    /// gets a pick's 500 and the ring class 12.
    pub(crate) fn post_contact(&mut self, a: usize, kind: i32) {
        if kind != 3 {
            return;
        }
        let team = self.players[a].team;
        self.ring_push(team, 12, -1, -1);
        self.fill_with_total(team, 500);
        if self.rules.tricks {
            self.trick_points(a, 500);
            self.side_stat(team, 0x20, 500);
            self.side_stat(1 - team, 0x10, 0);
        }
    }

    /// `FUN_00142768`, the game's slot 6 (posted at callback 35's end, a
    /// block jump's take-off): a jumper within 18 of a holder in a jump
    /// shot or a pump fake (+0x1d0) is watched (game+0x44c).
    pub(crate) fn watch_jumper(&mut self, jumper: usize) {
        let Some(h) = self.handler() else {
            return;
        };
        let near = distance(self.players[jumper].position, self.players[h].position) < 18.0;
        let faking = matches!(self.players[h].action, Action::PumpFake { .. });
        let shooting = crate::block::holder_bits(&self.players[h].action) & 0x8 != 0;
        if near
            && (shooting || faking)
            && let Some(flag) = self.pump_fake_watch.get_mut(jumper)
        {
            *flag = true;
        }
    }

    /// `FUN_0013f950`, the game's update after the actors': a watched
    /// jumper still in his block jump (bit 12) when the holder, on the
    /// floor, is out of his jump shot — in a pump fake, a pass (after more
    /// than `fps × 0.1` updates of the shot, game+0x498) or neither —
    /// posts slot 73 once; the watch ends `fps × 0.2` updates after a shot
    /// that never left the floor (game+0x494), and at once when nobody
    /// holds the ball or the holder is in the air.
    pub(crate) fn step_pump_fake_watch(&mut self) {
        let fps = crate::TICK_HZ as f32;
        let holder = self.handler();
        let now = self.scoring_now();
        let bits = |w: &World, h: usize| crate::block::holder_bits(&w.players[h].action);
        let passing = |w: &World, h: usize| matches!(w.players[h].action, Action::Passing { .. });
        let faking = |w: &World, h: usize| matches!(w.players[h].action, Action::PumpFake { .. });
        let check = match holder {
            Some(h) if self.players[h].height == 0.0 => {
                bits(self, h) & 0x8 != 0
                    || passing(self, h)
                    || faking(self, h)
                    || now - self.pump_fake_left < ((fps * 0.2) as i32)
            }
            _ => false,
        };
        if check && let Some(h) = holder {
            for j in 0..self.pump_fake_watch.len() {
                if !self.pump_fake_watch[j] {
                    continue;
                }
                if !matches!(
                    self.players[j].action,
                    Action::Jumping { block: Some(_), .. }
                ) {
                    self.pump_fake_watch[j] = false;
                    continue;
                }
                if !faking(self, h) && !passing(self, h) && bits(self, h) & 0x8 != 0 {
                    continue;
                }
                self.pump_fake_watch[j] = false;
                if !passing(self, h) || ((fps * 0.1) as i32) < self.pump_fake_shot {
                    self.post_pump_fake_points(j);
                }
            }
        } else {
            self.pump_fake_watch.iter_mut().for_each(|f| *f = false);
        }
        match holder {
            None => self.pump_fake_shot = 0,
            Some(h) if bits(self, h) & 0x8 != 0 => {
                self.pump_fake_shot += 1;
                self.pump_fake_left = if self.players[h].height == 0.0 {
                    now
                } else {
                    0
                };
            }
            Some(h) if passing(self, h) => {}
            Some(_) => self.pump_fake_shot = 0,
        }
    }

    /// `FUN_0016e8f8`: the defender guarding holder `h` — the other side's
    /// whose kept man (`FUN_0017f498(ai, 2)`) he is, else the one in his
    /// slot (`FUN_0017f238`).
    pub(crate) fn holder_defender(&self, h: usize) -> Option<usize> {
        let other = 1 - self.players[h].team.min(1);
        let squad: Vec<usize> = (0..self.players.len())
            .filter(|&j| self.players[j].team == other)
            .collect();
        let team = self.players[h].team;
        let slot = (0..h).filter(|&j| self.players[j].team == team).count();
        let second = self.ai.coaches.get(other).map(|c| c.defense.second);
        (0..3)
            .find(|&k| second.is_some_and(|s| s[k] == h as i32))
            .and_then(|k| squad.get(k).copied())
            .or_else(|| squad.get(slot.min(2)).copied())
    }

    /// `FUN_001342d0`: a screen's kind, 3 when the screened `defender` is
    /// the holder's own and the holder stands 15..33.75 from the rim his
    /// side attacks, the defender 12..33.75 and the two within 15 of each
    /// other (game+0x374, the distance table); else 2.
    pub(crate) fn screen_kind(&self, defender: usize) -> i32 {
        let Some(h) = self.handler() else {
            return 2;
        };
        if self.holder_defender(h) != Some(defender) {
            return 2;
        }
        let rim = self.rim_spot(target_hoop(self.players[h].team));
        let to_rim = |i: usize| distance(self.players[i].position, rim);
        let (held, guard) = (to_rim(h), to_rim(defender));
        let apart = distance(self.players[h].position, self.players[defender].position);
        if 15.0 < held && held < 33.75 && 12.0 < guard && guard < 33.75 && apart <= 15.0 {
            3
        } else {
            2
        }
    }

    /// `FUN_0014cb28`'s messages for a take: the ball's state and the
    /// sides decide a pass caught (19) or an interception (15), a rebound
    /// (17), a shot taken from its shooter (16), a steal (15); every take
    /// then posts 24.
    pub(crate) fn post_take(&mut self, take: Take) {
        let Take {
            taker,
            state,
            previous,
            possession,
            diving,
            passout,
            shooting,
            loose_steal,
        } = take;
        let live = self.game_phase.live();
        let own = self.players[taker].team == possession;
        // The assist window (+0x10c, +0x158): a teammate's take keeps the
        // last holder.
        self.assist = match previous {
            Some(p) if own && p != taker => Some((p, ASSIST_UPDATES)),
            _ => None,
        };
        if state == ball::PASS {
            if own {
                if let Some(passer) = previous {
                    self.post_pass_caught(passer, taker, passout);
                }
            } else if live {
                self.post_steal(taker);
            }
        } else if live {
            match state {
                ball::RIM | ball::REBOUND => self.post_rebound(taker),
                _ if own => {}
                ball::SHOT => self.post_block(taker, shooting),
                ball::HELD if shooting => self.post_block(taker, true),
                ball::HELD => self.post_steal(taker),
                ball::LOOSE if loose_steal => self.post_steal(taker),
                _ => {}
            }
        }
        self.post_catch(taker, diving);
    }

    /// A loose ball taken by the side without it is a steal unless it came
    /// off that side's own last holder (`0x14cfd0..0x14d05c`: ball+0x1a4
    /// or ball+0xf8 the other side's last holder).
    pub(crate) fn loose_steal(&self, taker: usize) -> bool {
        let other = 1 - self.players[taker].team.min(1);
        let shooter = self.ball.shooter;
        !(shooter.is_some() && shooter == self.ball.touch[other])
    }
}

/// `FUN_001439e8`: a gamebreaker basket's value — Level 1 25,000, or
/// 50,000 for a dunk or alley-oop; Level 2 50,000 from beyond the arc,
/// 100,000 inside it, 250,000 for a dunk or alley-oop.
pub fn gamebreaker_value(level: u8, kind: i32, base: i32) -> i32 {
    let wide = kind == 3 || kind == 4;
    match (level, wide) {
        (2, true) => 250_000,
        (2, false) if base == 2 => 100_000,
        (2, false) => 50_000,
        (_, true) => 50_000,
        _ => 25_000,
    }
}
