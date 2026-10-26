//! The post-up's backdown: what `FUN_00163d40` (the locomotion) does with
//! a holder whose dribble walks him at his defender near the rim (raw
//! `0x167564..0x16780c` moving, `0x1685f8..0x1687e8` standing), and the
//! test that starts it (`FUN_00163b48`).
//!
//! A holder backs down on his own: 15 to 18 from the rim the offence
//! attacks, his stick toward it and his wanted facing toward his nearest
//! opponent (each within 250 units), that man up and 2.1 nearer the rim,
//! within 6 of him and no more than 2.5 off his line to the rim, outside
//! a restart and without turbo. He goes `into_backdown`, which sets state
//! bit 11 ([`crate::Player::posting`]); while posting the dribble's
//! records are the backdown's (request 0x2100: `BACKDOWN_3_R`, script
//! `dribble_backdown_1`), and the stick decides his way out: off his
//! defender's cone or with the man not nearer the rim he comes out the
//! back, a side push (beyond 128 units) spins or steps out by his live
//! hand, and a push straight back keeps backing down.

use crate::stand::{
    BACKDOWN_BACK, BACKDOWN_IN, BACKDOWN_SPIN, BACKDOWN_STEP, DROPSTEP_L, DROPSTEP_R, POST_CALL,
};
use crate::{Input, World, timing};

/// The backdown's gait request (`0x167594`, `0x16780c`): record 704/705
/// (`BACKDOWN_3_R`, `dribble_backdown_1`).
pub const REQUEST: u32 = 0x2100;

/// `into_backdown`'s steps from its `82` into the side dribble
/// (`0x443bad`: the `96`, stop, `91`, `9b 0x820`, the blend, the loop, its
/// end).
pub const INTO_DRIBBLE: std::ops::Range<u8> = 10..17;

/// The bump reaction's step where it runs into the post stance's body.
pub const BUMP_STANCE: u8 = 9;

/// The backdown's band from the rim (`0x163c44..0x163c7c`).
pub const NEAR: f32 = 15.0;
pub const FAR: f32 = 18.0;

/// `x` folded to a half turn's units (0..=0x200).
fn fold(x: i32) -> i32 {
    let x = x & 0x3ff;
    if x > 0x200 { 0x400 - x } else { x }
}

/// The stick's word (actor+0x88).
pub(crate) fn stick_word(input: &Input) -> i32 {
    let [sx, sz] = input.stick();
    timing::facing_word(crate::math::atan2(sx, sz))
}

impl World {
    /// `FUN_0016e930` (game+0x374): `i`'s distance to the rim the offence
    /// attacks.
    pub(crate) fn offence_rim_distance(&self, i: usize) -> f32 {
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        crate::distance(self.players[i].position, rim)
    }

    /// `FUN_0016ea20`: state bit 11, posting up.
    pub(crate) fn posting(&self, i: usize) -> bool {
        self.players[i].posting
    }

    /// The backdown's rows (`s3 - 704 < 4`): `BACKDOWN_3_R` and
    /// `BACKDOWN_SIDEDRIBBLE_R`.
    fn on_backdown_rows(&self, i: usize) -> bool {
        (704..708).contains(&self.channel_row(i))
    }

    /// `FUN_00163b48(actor, stick)`: whether `i`, holding the ball with his
    /// stick at `stick`, backs down (raw `0x163b48..0x163d38`).
    pub(crate) fn backs_down(&self, i: usize, stick: i32, input: &Input) -> bool {
        let Some(o) = self.nearest_opponent(i) else {
            return false;
        };
        if self.posting(i) {
            return true;
        }
        if self.down(o) {
            return false;
        }
        let p = &self.players[i];
        // `FUN_001404e0(game, actor+0x284)`: the rim he attacks.
        let rim = self.rim_spot(crate::target_hoop(p.team));
        // `FUN_0018d500(stick, me, rim)`: the stick from his line to it.
        let to_rim = crate::contact::angle(p.position, rim);
        if fold(stick - to_rim) >= 251 {
            return false;
        }
        // `FUN_0018d1f8(me, opponent)`: his wanted facing (+0xb0) from his
        // line to the man.
        let at = crate::contact::angle(p.position, self.players[o].position);
        if fold(timing::facing_word(p.wanted) - at) >= 251 {
            return false;
        }
        let (mine, his) = (self.offence_rim_distance(i), self.offence_rim_distance(o));
        if his >= mine - f32::from_bits(0x4006_6666) {
            return false;
        }
        if !(NEAR..=FAR).contains(&mine) {
            return false;
        }
        if crate::distance(p.position, self.players[o].position) > 6.0 {
            return false;
        }
        if self.game_phase.phase == crate::game_phase::phase::RESTART || input.turbos() != 0 {
            return false;
        }
        // `FUN_001379d8`: the man no more than 2.5 off his line to the rim.
        let me = p.position;
        let q = self.players[o].position;
        let l = [rim[0] - me[0], rim[1] - me[1]];
        let d = [q[0] - me[0], q[1] - me[1]];
        let ll = l[0] * l[0] + l[1] * l[1];
        if ll == 0.0 {
            return false;
        }
        let t = (d[0] * l[0] + d[1] * l[1]) / ll;
        let off = crate::math::len(d[0] - l[0] * t, d[1] - l[1] * t);
        off <= 2.5
    }

    /// The moving holder's backdown (`0x167564..0x16780c`), before the
    /// gait's record is chosen: `request` the family's, `stick` his
    /// stick's word. Starts a script and returns true when one takes the
    /// update; else may set `request` to [`REQUEST`].
    pub(crate) fn backdown_move(
        &mut self,
        i: usize,
        input: &Input,
        stick: i32,
        request: &mut u32,
    ) -> bool {
        if self.backs_down(i, stick, input) {
            if self.on_backdown_rows(i) {
                *request = REQUEST;
            } else if self.posting(i) {
                let a = timing::angle_diff(
                    (timing::facing_word(self.players[i].wanted) + 0x200) & 0x3ff,
                    stick,
                );
                if a.abs() <= 384 {
                    let script = if a < -128 {
                        DROPSTEP_L
                    } else if a > 128 {
                        DROPSTEP_R
                    } else {
                        BACKDOWN_IN
                    };
                    self.start_backdown(i, script);
                    return true;
                }
            } else {
                self.start_backdown(i, BACKDOWN_IN);
                return true;
            }
        }
        if !self.on_backdown_rows(i) {
            return false;
        }
        // On the backdown: the stick from his wanted facing.
        let s0 = timing::angle_diff(timing::facing_word(self.players[i].wanted), stick);
        let o = self.nearest_opponent(i);
        let held = o.is_some_and(|o| {
            s0.abs() < 385
                && self.guards_holder(o, 250)
                && self.offence_rim_distance(o) <= self.offence_rim_distance(i)
        });
        if !held {
            self.start_backdown(i, BACKDOWN_BACK);
            return true;
        }
        let hand = self.players[i].gait_channel.hand;
        if s0 < -128 {
            self.start_backdown(
                i,
                if hand == 1 {
                    BACKDOWN_SPIN
                } else {
                    BACKDOWN_STEP
                },
            );
            return true;
        }
        if s0 > 128 {
            self.start_backdown(
                i,
                if hand == 2 {
                    BACKDOWN_SPIN
                } else {
                    BACKDOWN_STEP
                },
            );
            return true;
        }
        *request = REQUEST;
        false
    }

    /// `FUN_0016fef8(actor, width)`: `o` between the holder and the rim
    /// the offence attacks, within `width` of the holder's line from it
    /// and nearer to it.
    pub(crate) fn guards_holder(&self, o: usize, width: i32) -> bool {
        let Some(h) = self.handler() else {
            return false;
        };
        if h == o {
            return false;
        }
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        let (hp, op) = (self.players[h].position, self.players[o].position);
        let d = fold(crate::contact::angle(hp, rim) - crate::contact::angle(op, rim));
        d <= width && crate::distance(op, rim) <= crate::distance(hp, rim)
    }

    /// Whether `i`'s channel plays a backdown move that holds the
    /// locomotion off (its `91` sets 0x2 without 0x10, `0x15ab58`,
    /// `0x15b084`): any but `into_backdown` once it runs into the side
    /// dribble (`0x443bad`, whose `91` clears it: [`INTO_DRIBBLE`]); the
    /// post call; the pick stance until its `91 0` (step 7).
    pub(crate) fn backdown_holds(&self, i: usize) -> bool {
        self.players[i].stand.is_some_and(|s| {
            (crate::stand::BACKDOWN_MOVES.contains(&s.script)
                && !(s.script == BACKDOWN_IN && INTO_DRIBBLE.contains(&s.run.pc)))
                || s.script == POST_CALL
                || (s.script == crate::stand::PICK_STANCE && s.run.pc < 7)
                || self.bumped(i)
        })
    }

    /// Whether `i` plays the bump reaction's clip (717), before it runs
    /// into the post stance: its `91` sets 0x2 (no locomotion) and 0x40
    /// (off balance, `FUN_0016ea70`).
    pub(crate) fn bumped(&self, i: usize) -> bool {
        use crate::stand::{BUMP_REACTION, COLLISION_CARRIER, COLLISION_DEFENDER};
        self.players[i].stand.is_some_and(|s| {
            (s.script == BUMP_REACTION && s.run.pc < BUMP_STANCE)
                || matches!(s.script, COLLISION_DEFENDER | COLLISION_CARRIER)
        })
    }

    /// Actor+0x78 & 0x2000: a turbo held this update (the contacts run
    /// after every pad was read).
    pub(crate) fn turbo_held(&self, i: usize) -> bool {
        self.previous_inputs.get(i).is_some_and(|x| x.turbos() > 0)
    }

    /// The turbo collision (`FUN_001343e8`, `0x135754..0x1358fc`): holder
    /// `b` off the backdown's walk, not passing, catching or in a trick
    /// (bits 4, 9, 10), facing his rim within 250 (`FUN_0018d290`), 15..18
    /// from it with defender `a` more than 2.1 nearer, both on the floor
    /// and `a` not off balance, outside a restart, moving with turbo held:
    /// `a` plays `collision_defender`, `b` `collision_ballcarrier` (both off
    /// balance and without a walk while they play), the contact's message
    /// goes out (kind 0; `b`'s AI hears `a`) and neither is pushed. True
    /// when it did.
    pub(crate) fn turbo_collision(
        &mut self,
        a: usize,
        b: usize,
        moving: [bool; 2],
        turbo: bool,
    ) -> bool {
        let p = &self.players[b];
        if matches!(
            p.action,
            crate::Action::Passing { .. } | crate::Action::Trick { .. }
        ) || self.catch_row_playing(b).is_some()
        {
            return false;
        }
        if self.down(a) || p.height != 0.0 || self.players[a].height != 0.0 {
            return false;
        }
        let rim = self.rim_spot(crate::target_hoop(p.team));
        let (_, fold) = timing::facing_angle(p.wanted, p.position, rim);
        if fold >= 251 {
            return false;
        }
        let (mine, his) = (self.offence_rim_distance(b), self.offence_rim_distance(a));
        if his >= mine - f32::from_bits(0x4006_6666) || !(NEAR..=FAR).contains(&mine) {
            return false;
        }
        if self.game_phase.phase == crate::game_phase::phase::RESTART || !moving[1] || !turbo {
            return false;
        }
        self.start_stand(a, crate::stand::COLLISION_DEFENDER);
        self.start_stand(b, crate::stand::COLLISION_CARRIER);
        self.post_contact(a, 0);
        if let Some(brain) = self.ai.brains.get_mut(b) {
            brain.screened_by = a as i32;
        }
        true
    }

    /// The post-up's bump (`FUN_001343e8`, `0x135644..0x135740`), with `b`
    /// holding the ball against defender `a`: `b` on the backdown's walk
    /// (704/705) and moving, `a` still, both on the floor and neither
    /// down, `b`'s count of contacts with `a` at half of `0x3d0f6c`'s 30 or
    /// more, each facing the other within 100 units (`FUN_0018d1f8`), and
    /// `b` the stronger (attribute 4): `a` plays `backdown_bump_reaction`,
    /// `b`'s count rests at -1, the contact's message goes out (kind 0;
    /// the defender's AI hears who bumped him) and the contact pushes
    /// neither. True when it did.
    pub(crate) fn backdown_bump(&mut self, a: usize, b: usize, moving: [bool; 2]) -> bool {
        if b >= 6 || self.down(a) || self.down(b) || !moving[1] || moving[0] {
            return false;
        }
        if self.players[a].height != 0.0 || self.players[b].height != 0.0 {
            return false;
        }
        if self.contacts.count[b] < crate::contact::SCREEN_CONTACTS / 2 {
            return false;
        }
        if !matches!(self.channel_row(b), 704 | 705) {
            return false;
        }
        let off = |x: usize, y: usize| {
            let d = (self.facing_units(x)
                - crate::contact::angle(self.players[x].position, self.players[y].position))
                & 0x3ff;
            if 0x200 < d { 0x400 - d } else { d }
        };
        if off(a, b) > 100 || off(b, a) > 100 {
            return false;
        }
        let power = |i: usize| self.players[i].ratings.native(4);
        if power(a) >= power(b) {
            return false;
        }
        self.contacts.count[b] = -1;
        self.start_stand(a, crate::stand::BUMP_REACTION);
        self.post_contact(b, 0);
        if let Some(brain) = self.ai.brains.get_mut(a) {
            brain.screened_by = b as i32;
        }
        true
    }

    /// The off-ball post call (`FUN_0015a060`, `0x15b544..0x15b5bc`): a
    /// man of the side with the ball, without it, holding the trick
    /// button (+0x78 & 0x800000), in the dunking area (`FUN_0016c908`),
    /// on the floor, outside a restart and neither in a rebound jump,
    /// catching nor posting (bits 8, 9, 11), plays `post_call_for_ball_2`,
    /// which posts (`9b 0x800`) and holds his walk (`91` 0x2); let go of
    /// the button on its rows 728/729, he stands ready and the bit clears
    /// (`0x15ad84..0x15adc0`).
    pub(crate) fn post_call(&mut self, i: usize, input: &Input) {
        let p = &self.players[i];
        if p.team != self.possession || self.ball.state == crate::BallState::Held(i) {
            return;
        }
        let held = input.held(crate::button::TRICK);
        if !held {
            if p.posting && matches!(self.channel_row(i), 728 | 729) {
                self.players[i].posting = false;
                self.start_stand(i, crate::stand::READY);
            }
            return;
        }
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        let inside = p.position[1].abs() < 22.5 && crate::distance(p.position, rim) < 23.75;
        if !matches!(p.action, crate::Action::None)
            || !inside
            || p.height != 0.0
            || self.game_phase.phase == crate::game_phase::phase::RESTART
            || p.posting
            || self.catch_row_playing(i).is_some()
            || self.players[i].stand.is_some_and(|s| s.script == POST_CALL)
        {
            return;
        }
        self.start_stand(i, POST_CALL);
    }

    /// `FUN_0016f260`: the way from `i` to the rim the offence attacks.
    pub(crate) fn toward_rim(&self, i: usize) -> i32 {
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        crate::contact::angle(self.players[i].position, rim)
    }

    /// `FUN_0016ec80`: `i`'s channel on the post-up's clips (records
    /// 704..707, 712, 713).
    pub(crate) fn on_post_up_clip(&self, i: usize) -> bool {
        matches!(self.channel_row(i), 704..=707 | 712 | 713)
    }

    /// `FUN_0016e8c0`: the man defender `i` guards (`FUN_0017f498(ai, 0)`,
    /// his side's defence strategy's matchup for his slot).
    pub(crate) fn man_of(&self, i: usize) -> Option<usize> {
        let team = self.players[i].team;
        let slot = (0..i).filter(|&j| self.players[j].team == team).count();
        let man = self.ai.coaches.get(team.min(1))?.defense.man[slot.min(2)];
        usize::try_from(man)
            .ok()
            .filter(|&j| j < self.players.len())
    }

    /// The moving path's first test (`0x167108..0x167198`): a defender on
    /// the post defence's rows whose nearest opponent is his man, posting
    /// or on the post-up's clips within 5, with his stick within 384 of
    /// his wanted facing, holds his stance: no locomotion this update.
    pub(crate) fn holds_post(&self, i: usize, stick: i32) -> bool {
        let Some(o) = self.nearest_opponent(i) else {
            return false;
        };
        self.man_of(i) == Some(o)
            && matches!(self.channel_row(i), 718 | 719)
            && (self.on_post_up_clip(o) || self.posting(o))
            && timing::angle_diff(stick, timing::facing_word(self.players[i].wanted)).abs() < 384
            && crate::distance(self.players[i].position, self.players[o].position) <= 5.0
    }

    /// Whether `i`'s locomotion is off this update: a backdown move under
    /// way ([`World::backdown_holds`]) or, with his stick out, the post
    /// defence's hold ([`World::holds_post`]).
    pub(crate) fn locomotion_held(&self, i: usize, input: &Input) -> bool {
        self.backdown_holds(i) || (input.moves() && self.holds_post(i, stick_word(input)))
    }

    /// The defence's update for a man on the post defence's rows
    /// (`FUN_0015da28`, `0x15db18..0x15dca4`): the holder not in a blend
    /// (0x200000), his nearest opponent and posting or on the post-up's
    /// clips: the defender keeps his distance from the holder and goes
    /// round him toward the holder's way to the rim, at most 14 units an
    /// update (`0x416b8e0f`).
    pub(crate) fn post_slide(&mut self, i: usize) {
        if !matches!(self.channel_row(i), 718 | 719) {
            return;
        }
        let Some(h) = self.handler() else {
            return;
        };
        let blending = self.players[h]
            .stand
            .is_some_and(|s| s.run.blend_counter().is_some());
        if blending
            || self.nearest_opponent(i) != Some(h)
            || !(self.posting(h) || self.on_post_up_clip(h))
        {
            return;
        }
        let (hp, me) = (self.players[h].position, self.players[i].position);
        let rim = self.toward_rim(h);
        let at = crate::contact::angle(hp, me);
        let d = timing::angle_diff(rim, at);
        let step = f32::from_bits(0x416b_8e0f) as i32;
        let word = if d > step {
            (at + step) & 0x3ff
        } else if d < -step {
            (at - step) & 0x3ff
        } else {
            rim
        };
        let r = crate::distance(hp, me);
        self.players[i].position = [
            hp[0] - r * crate::input::native_sin_1024(word),
            hp[1] - r * crate::input::native_cos_1024(word),
        ];
    }

    /// `FUN_0019c2f8` with a backdown script, played on his channel as a
    /// stand his stick does not stop ([`crate::stand::BACKDOWN_MOVES`]).
    fn start_backdown(&mut self, i: usize, script: u8) {
        self.start_stand(i, script);
    }
}
