//! The stand path of the locomotion (`FUN_00163d40`, raw `0x1680ac..
//! 0x1689b0`): what a player on the floor plays while his stick is under
//! 16, or while he holds the ball and may not move (his dribble picked
//! up, actor+0x1a0 −1) or the referee tosses it (ball state 7)
//! (docs/TRACE.md "The stand path").
//!
//! It starts a stand script on his channel (`FUN_0019c2f8`) whenever the
//! row the channel plays is none of the stands his place in the play
//! allows, so a stop blends into its stand, which then loops (`ab`) until
//! he moves again or a move takes the channel.

/// The stand scripts, as [`STAND_SCRIPTS`] holds them.
pub const STAND_ADDRESSES: [u32; 51] = [
    0x4570f5, 0x457139, 0x4571a8, 0x457202, 0x4570bb, 0x4570d8, 0x4572ae, 0x45731b, 0x456f41,
    0x457412, 0x45742f, 0x4573ce, 0x4573f0, 0x457450, 0x45746d, 0x443bad, 0x443bcf, 0x443bf6,
    0x443c1d, 0x443c44, 0x443c6b, 0x443c92, 0x443cb9, 0x443ce0, 0x443d74, 0x443d07, 0x443d2e,
    0x456f6f, 0x456f9d, 0x456fcb, 0x456ff9, 0x45702c, 0x45705f, 0x45708d, 0x44392a, 0x443980,
    0x4439cd, 0x443a1a, 0x4438d7, 0x4438a7, 0x457377, 0x456d38, 0x456f0e, 0x456de8, 0x456e39,
    0x456e8a, 0x456ed6, 0x456d96, 0x44c31c, 0x44c741, 0x44c6f8,
];

/// The defence's flair (`FUN_00170fc8`'s `r % 7` of `0x3d9100`, which the
/// boot's constructor fills at `0x17c1e8..0x17c3a8`): each row and the
/// updates its clip shows a frame for, played by script `0x457271`, which
/// then goes on into the face-up stand `0x4572ae`.
pub const FLAIR_ROWS: [(u16, f32); 7] = [
    (950, 4.0),
    (951, 4.0),
    (952, 4.0),
    (953, 4.0),
    (954, 4.0),
    (955, 2.0),
    (956, 2.0),
];

use crate::trick::{Run, Select::Handed, Select::Side, Step, Step::*};
use crate::{Action, BallState, Input, World, jump, timing};

/// One stand script (as [`crate::trick::Reaction`]s are, without the root
/// a stand's loop has no use for).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandScript {
    pub address: u32,
    pub steps: &'static [Step],
}

include!("stand_tables.rs");

/// [`STAND_SCRIPTS`]' indices.
pub const BREATHE: u8 = 0;
pub const WARMUP: u8 = 1;
pub const KICK: u8 = 2;
pub const STRETCH: u8 = 3;
pub const READY: u8 = 4;
pub const RELAX: u8 = 5;
pub const FACE_UP: u8 = 6;
pub const ANTICIPATE: u8 = 7;
pub const BOX_OUT: u8 = 8;
pub const CRADLE: u8 = 9;
pub const PROTECT: u8 = 10;
pub const BACKDOWN: u8 = 15;
pub const DRIBBLE_FIRST: u8 = 16;
/// The box-out's contact (`FUN_001343e8`, `0x134f4c..0x135344`): the
/// boxer's behind him and to either side (by the boxed man's eighth about
/// his facing: 6 or more, below 6), and the boxed man's behind (`r & 1`: 1,
/// 0) and to either side (the same eighths).
pub const BOXER_BEHIND: u8 = 27;
pub const BOXER_SIDE: [u8; 2] = [28, 29];
pub const BOXED_BEHIND: [u8; 2] = [30, 31];
pub const BOXED_SIDE: [u8; 2] = [33, 32];
/// The backdown's (`FUN_00163d40`, [`crate::backdown`]): `into_backdown`,
/// `from_backdown_back`, `backdown_spinout`, `backdown_stepout`,
/// `juke_dropstep_l` and `_r`.
pub const BACKDOWN_IN: u8 = 34;
pub const BACKDOWN_BACK: u8 = 35;
pub const BACKDOWN_SPIN: u8 = 36;
pub const BACKDOWN_STEP: u8 = 37;
pub const DROPSTEP_L: u8 = 38;
pub const DROPSTEP_R: u8 = 39;
/// The backdown's scripts, which the stick does not stop.
pub const BACKDOWN_MOVES: std::ops::RangeInclusive<u8> = BACKDOWN_IN..=DROPSTEP_R;
/// `defense_stand_postup_1` (`0x457377`, `0x3d79d0`'s third): the
/// defender's stance against a post-up (`FUN_00163a38`).
pub const POST_STANCE: u8 = 40;
/// A posting holder who picked up his dribble: `post_face_rim_r_1`
/// (`0x456d38`) off his defender's cone, else `r % 5` of `0x3d7a40`
/// (`post_stand_wb_1`, `post_fake_1`, `_2`, `post_push_1`,
/// `post_look_to_pass`), kept while the row is one of `0x3d7a20`'s 8.
pub const POST_FACE_RIM: u8 = 41;
pub const POST_STANDS: [u8; 5] = [42, 43, 44, 45, 46];
pub const POST_ROWS: [u16; 8] = [727, 726, 724, 725, 723, 722, 732, 733];
/// `post_call_for_ball_2` (`0x456d96`): a man off the ball posting for it
/// ([`World::post_call`]), rows 728/729 by callback 10.
pub const POST_CALL: u8 = 47;
/// `backdown_bump_reaction` (`0x44c31c`): a defender bumped by a
/// stronger posting holder ([`World::backdown_bump`]).
pub const BUMP_REACTION: u8 = 48;
/// `collision_defender` (`0x44c741`) and `collision_ballcarrier`
/// (`0x44c6f8`): a holder's turbo collision ([`World::turbo_collision`]).
pub const COLLISION_DEFENDER: u8 = 49;
pub const COLLISION_CARRIER: u8 = 50;
pub const FLAIR_FIRST: u8 = 51;

/// The offence's stands off the ball (`0x3d7938`, `r % 15`) and the rows
/// that count as one already (`0x3d78f0`).
pub const OFFENCE_STANDS: [u8; 15] = [
    BREATHE, WARMUP, KICK, STRETCH, READY, READY, READY, READY, READY, RELAX, RELAX, RELAX, RELAX,
    RELAX, RELAX,
];
pub const OFFENCE_ROWS: [u16; 18] = [
    65, 50, 52, 53, 68, 67, 729, 728, 54, 55, 61, 60, 57, 56, 59, 58, 62, 63,
];

/// The defence's stands (`0x3d79d0`'s first four, the third the stance
/// against a post-up) and the rows that count as one (`0x3d7978`).
pub const DEFENCE_STANDS: [u8; 4] = [BREATHE, FACE_UP, POST_STANCE, ANTICIPATE];
pub const DEFENCE_ROWS: [u16; 22] = [
    65, 69, 719, 718, 70, 54, 55, 61, 60, 57, 56, 59, 58, 62, 63, 950, 951, 952, 953, 954, 955, 956,
];

/// The flair's own rows (`0x43f0a0`).
pub const FLAIR_PLAYING: [u16; 7] = [950, 951, 952, 953, 954, 955, 956];

/// The holder's stands while he does not dribble (`0x3d7a08`, `r % 6`)
/// and the rows that count as one (`0x3d79e0`).
pub const HELD_STANDS: [u8; 6] = [11, 12, CRADLE, PROTECT, 13, 14];
pub const HELD_ROWS: [u16; 9] = [665, 656, 657, 659, 660, 662, 661, 664, 663];

/// The dribbler's stands (`0x3d7ad0`: `r % 11`, or `r & 1` of the first
/// two below 30 handles) and the rows that count as one (`0x3d7a58`).
pub const DRIBBLE_ROWS: [u16; 29] = [
    706, 707, 75, 74, 77, 76, 667, 666, 669, 668, 670, 671, 673, 672, 674, 675, 680, 681, 677, 676,
    678, 679, 141, 176, 177, 174, 175, 46, 47,
];

/// Updates a defender may stand on the post defence's rows before his
/// stand starts again (`(int)(fps × 0.12)`, `0x16844c..0x168490`).
pub const POST_HOLD: u16 = 7;

/// A stand script under way on a player's channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standing {
    /// Its index in [`STAND_SCRIPTS`].
    pub script: u8,
    pub run: Run,
    /// The clip he played as it started (record and frame), held through
    /// its first blend.
    pub from: Option<(u16, f32)>,
    /// The planted feet (channel flag 0x80000, `95`): the bones and their
    /// point on the floor, held until a `96` lets go.
    pub plant: Option<([u8; 2], [f32; 2])>,
}

impl Standing {
    pub fn steps(&self) -> &'static [Step] {
        STAND_SCRIPTS[usize::from(self.script)].steps
    }
}

/// `x` folded to a half turn's units (0..=0x200).
fn fold(x: i32) -> i32 {
    let x = x & 0x3ff;
    if x > 0x200 { 0x400 - x } else { x }
}

impl World {
    /// The row `i`'s channel plays (`FUN_00163d40`'s `s3`: +0x50, or +0x54
    /// while a blend runs): a stand's, else his gait record's.
    pub fn channel_row(&self, i: usize) -> u16 {
        let p = &self.players[i];
        if let Some(r) = p.stand.and_then(|s| s.run.shown().record) {
            return r;
        }
        let c = &p.gait_channel;
        if c.record.is_some() {
            c.record_row()
        } else {
            c.row
        }
    }

    /// Whether `i` holds the ball and may not move (`0x167020..0x167050`:
    /// a dribble picked up, actor+0x1a0 −1): the stand path runs whatever
    /// his stick asks, which only turns him.
    pub fn held_still(&self, i: usize) -> bool {
        self.ball.state == BallState::Held(i) && self.players[i].dribble == -1
    }

    /// The stand path (`FUN_00163d40`, `0x1680ac..0x1689b0`) for `i`, on the
    /// floor with nothing else on his channel and his stick under 16 (or
    /// [`World::held_still`]); `input` his pad. Otherwise any stand stops.
    pub(crate) fn stand_path(&mut self, i: usize, input: &Input) {
        let p = &self.players[i];
        let free = matches!(p.action, Action::None)
            && p.height == 0.0
            && self.catch_row_playing(i).is_none()
            && !(self.ball.state == BallState::Pass && self.ball.pass.is_some_and(|q| q.to == i));
        if self.locomotion_held(i, input) {
            return;
        }
        if !free || (input.moves() && !self.held_still(i)) {
            let p = &mut self.players[i];
            p.stand = None;
            p.boxing = false;
            p.boxed = false;
            return;
        }
        let row = self.channel_row(i);
        if self.ball.state == BallState::Held(i) {
            self.holder_stand(i, input, row);
            return;
        }
        // Actor+0x194: the jump ball's role (0) stands as it is.
        if self.opening_toss != crate::OpeningToss::None {
            return;
        }
        let me = self.players[i].position;
        let rim = self.rim_spot(crate::target_hoop(self.possession));
        let ball = self.ball.position;
        let ball_dist = crate::math::len(ball[0] - rim[0], ball[2] - rim[1]);
        let my_dist = crate::distance(me, rim);
        // A box-out (bit 17) ends once the ball is farther from the rim
        // than he is, or once it is no shot, rim or rebound.
        let state = self.native_ball_state();
        let shot_up = matches!(state, 0 | 5 | 6);
        if self.players[i].boxing && (my_dist < ball_dist || !shot_up) {
            self.players[i].boxing = false;
            self.start_stand(i, READY);
            // The man he held (+0x234) is let go, back to his stand if he
            // was still boxed (`0x1681c0..0x168214`).
            if let Some(k) = self.players[i].partner.take().map(usize::from) {
                self.players[k].boxer = None;
                if self.players[k].boxed {
                    self.start_stand(k, READY);
                    self.players[k].boxed = false;
                }
            }
        }
        // No holder and a shot up, the rim within 14 and the ball nearer
        // it than he is: he boxes out (`0x168218..0x1682c0`).
        if self.handler().is_none()
            && shot_up
            && my_dist <= 14.0
            && ball_dist <= my_dist
            && !self.players[i].boxing
            && !self.players[i].boxed
        {
            self.start_stand(i, BOX_OUT);
            return;
        }
        if self.players[i].team == self.possession {
            // The offence off the ball: a draw every update.
            let k = (self.rng.next_u32() % 15) as usize;
            if !OFFENCE_ROWS.contains(&row) {
                self.start_stand(i, OFFENCE_STANDS[k]);
            }
            return;
        }
        // The defence: `r & 1` every update for the breathing or the
        // anticipation; the face-up while the holder faces his rim
        // (within 301) and he faces the holder (within 200).
        let mut pick = 3 * (self.rng.next_u32() & 1) as usize;
        let holder = self.handler();
        if let Some(h) = holder {
            let hp = self.players[h].position;
            let his_rim = self.rim_spot(crate::target_hoop(self.players[h].team));
            let toward =
                timing::angle_units(crate::math::atan2(hp[0] - his_rim[0], hp[1] - his_rim[1]));
            if fold(timing::facing_word(self.players[h].wanted) - toward) < 301 {
                let off = fold(self.facing_units(i) - crate::contact::angle(me, hp));
                if off <= 200 {
                    pick = 1;
                }
            }
        }
        // The post defence's rows (`FUN_0016ebe8`, 718 and 719) count up;
        // against a post-up ([`World::post_stance`]) he takes the stance
        // facing his nearest opponent and the count starts again
        // (`0x1683d4..0x168448`).
        let post = matches!(row, 718 | 719);
        if self.post_stance(i) {
            if !post {
                self.start_stand(i, POST_STANCE);
                pick = 2;
                if let Some(o) = self.nearest_opponent(i) {
                    let to = crate::contact::angle(me, self.players[o].position);
                    self.players[i].wanted = crate::restart::heading_radians(to);
                }
            }
            self.players[i].stand_count = 0;
        } else {
            let p = &mut self.players[i];
            p.stand_count = if post {
                p.stand_count.saturating_add(1)
            } else {
                0
            };
        }
        let p = &self.players[i];
        if p.stand_count >= POST_HOLD || !DEFENCE_ROWS.contains(&row) {
            self.start_stand(i, DEFENCE_STANDS[pick]);
        }
        // The flair (`0x1684dc..0x1685f0`): the holder's nearest opponent,
        // still, in a restart, not already flaring.
        if let Some(h) = holder
            && self.nearest_opponent(h) == Some(i)
            && !self.players[i].moving()
            && self.game_phase.phase == crate::game_phase::phase::RESTART
            && !FLAIR_PLAYING.contains(&row)
        {
            let k = (self.rng.next_u32() % FLAIR_ROWS.len() as u32) as u8;
            self.start_stand(i, FLAIR_FIRST + k);
        }
    }

    /// The holder's stand (`0x1685f8..0x1689ac`).
    fn holder_stand(&mut self, i: usize, input: &Input, row: u16) {
        // Posting (bit 11) and dribbling: out the back unless his nearest
        // opponent stands in his cone (250) nearer the rim (`0x1687e8..
        // 0x16885c`).
        if self.players[i].dribble == 1 && self.posting(i) {
            let held = self.nearest_opponent(i).is_some_and(|o| {
                self.in_cone(i, o, 250)
                    && self.offence_rim_distance(i) > self.offence_rim_distance(o)
            });
            if !held {
                self.start_stand(i, BACKDOWN_BACK);
                return;
            }
        }
        // Posting with his dribble picked up: the post's stands while the
        // man is in his cone, else he faces the rim (`0x168608..0x1686b0`).
        if self.players[i].dribble != 1 && self.posting(i) {
            let cone = self
                .nearest_opponent(i)
                .is_some_and(|o| self.in_cone(i, o, 0));
            if cone {
                if !POST_ROWS.contains(&row) {
                    let k = (self.rng.next_u32() % POST_STANDS.len() as u32) as usize;
                    self.start_stand(i, POST_STANDS[k]);
                }
            } else if !matches!(row, 720 | 721) {
                self.start_stand(i, POST_FACE_RIM);
            }
            return;
        }
        if self.players[i].dribble == 1 {
            if DRIBBLE_ROWS.contains(&row) {
                return;
            }
            if matches!(row, 704 | 705) {
                self.start_stand(i, BACKDOWN);
                return;
            }
            let k = if self.players[i].ratings.native(7) >= 30 {
                self.rng.next_u32() % 11
            } else {
                self.rng.next_u32() & 1
            };
            self.start_stand(i, DRIBBLE_FIRST + k as u8);
            return;
        }
        let restart = self.game_phase.phase == crate::game_phase::phase::RESTART;
        if restart && !matches!(row, 665 | 656) {
            // The inbounder cradles it; once all six are ready a draw goes
            // unused.
            if self.restart_ready() {
                self.rng.next_u32();
            }
            self.start_stand(i, CRADLE);
        } else if !HELD_ROWS.contains(&row) {
            if self.players[i].flight.flags & crate::jump::PLANTED != 0 {
                self.start_stand(i, PROTECT);
            } else {
                let k = (self.rng.next_u32() % 6) as usize;
                self.start_stand(i, HELD_STANDS[k]);
            }
        }
        // His stick turns him (actor+0xb0 = +0x88).
        if input.strength() != 0 {
            let [sx, sz] = input.stick();
            self.players[i].wanted = crate::math::atan2(sx, sz);
        }
    }

    /// Whether `x` may be boxed out (`0x134fa0..0x135014`): on the floor
    /// and not rising, nobody's box-out or boxed man already, not in a
    /// rebound jump (bit 8) or a block jump (bit 12), with nothing but his
    /// stand on his channel.
    fn boxable(&self, x: usize) -> bool {
        let p = &self.players[x];
        matches!(p.action, Action::None)
            && p.height == 0.0
            && p.rise == 0.0
            && !p.boxing
            && !p.boxed
    }

    /// The box-out's contact (`FUN_001343e8`, `0x134f4c..0x135344`) for a
    /// touching pair, `a` the original's `iVar21`: a boxer (bit 17) holding
    /// nobody yet (+0x234) meeting a man he may box ([`World::boxable`])
    /// takes him instead of the push. `a` boxing: with the man in front of
    /// him (`FUN_0018d9f0`'s eighths 3..5) the push goes on; behind him (its
    /// quarter 0) `a` plays `BOX_BEHIND_4` and the man one of the boxed
    /// front clips by `r & 1`; to either side the side clips (the eighth
    /// below 6 or not). Else `b` boxing with `a` behind him the same
    /// behind. Then the faces (the boxer's 2, the boxed man's 3 or 4 by
    /// `r & 1`, fps × 3) and the contact's message (`0x3c6910`): the boxed
    /// man's AI hears the boxer. True when it took the pair.
    pub(crate) fn box_out_contact(&mut self, a: usize, b: usize) -> bool {
        let holds = |w: &World, x: usize| w.players[x].boxing && w.players[x].partner.is_none();
        let (boxer, boxed, clips) = if holds(self, a) && self.boxable(b) {
            let eighth = self.octant(a, b);
            if (3..=5).contains(&eighth) {
                return false;
            }
            if self.quarter(a, b) == 0 {
                let r = self.boxed_draw();
                (a, b, (BOXER_BEHIND, BOXED_BEHIND[usize::from(r == 0)]))
            } else {
                let k = usize::from(eighth < 6);
                (a, b, (BOXER_SIDE[k], BOXED_SIDE[k]))
            }
        } else if holds(self, b) && self.boxable(a) {
            if self.quarter(b, a) != 0 {
                return false;
            }
            let r = self.boxed_draw();
            (b, a, (BOXER_BEHIND, BOXED_BEHIND[usize::from(r == 0)]))
        } else {
            return false;
        };
        self.players[a].partner = None;
        self.players[b].boxer = None;
        self.start_stand(boxer, clips.0);
        self.start_stand(boxed, clips.1);
        self.players[boxer].partner = Some(boxed as u8);
        self.players[boxed].boxer = Some(boxer as u8);
        let fps3 = 3 * crate::TICK_HZ as i32;
        self.face(boxer, fps3);
        self.rng.next_u32();
        self.face(boxed, fps3);
        if let Some(brain) = self.ai.brains.get_mut(boxed) {
            brain.screened_by = boxer as i32;
        }
        true
    }

    /// The boxed man's clip behind the boxer, `r & 1` (`0x135070`).
    fn boxed_draw(&mut self) -> u32 {
        self.rng.next_u32() & 1
    }

    /// `FUN_0019c2f8` with stand `script` on `i`'s channel: its first update
    /// is the channel's step that follows ([`World::stand_tick`]).
    /// `FUN_0019c320`'s channel-0 start (`0x19c3cc..0x19c490`): a boxed
    /// man (bit 18) lets his boxer go, who plays `standboxout` again; a
    /// boxer (bit 17) lets his man go, who stands ready if he was still
    /// boxed; both bits clear (the new script's `9b` sets its own).
    pub(crate) fn release_box_out(&mut self, i: usize) {
        if self.players[i].boxed {
            if let Some(b) = self.players[i].boxer.take().map(usize::from) {
                self.players[b].partner = None;
                self.start_stand(b, BOX_OUT);
            }
            self.players[i].boxed = false;
        } else if self.players[i].boxing {
            if let Some(k) = self.players[i].partner.take().map(usize::from) {
                self.players[k].boxer = None;
                if self.players[k].boxed {
                    self.start_stand(k, READY);
                    self.players[k].boxed = false;
                }
            }
            self.players[i].boxing = false;
        }
    }

    pub(crate) fn start_stand(&mut self, i: usize, script: u8) {
        self.release_box_out(i);
        let from = self.poses.get().map(|_| {
            let f = self.gait_frame(i);
            (f.record, f.time)
        });
        let left = self.players[i].gait_channel.hand == 2;
        let handed = self.players[i].left_handed;
        let mut run = Run::new(left, 0).picking(handed, false);
        run.asked = self.asked(i, script);
        run.before = Some(from.map_or(self.players[i].gait_channel.row, |(record, _)| record));
        // The channel's flags outlive the script: a plant holds into the
        // next stand until its `96`.
        let plant = self.players[i].stand.and_then(|s| s.plant);
        self.players[i].stand = Some(Standing {
            script,
            run,
            from,
            plant,
        });
    }

    /// The answer to `script`'s [`Step::Asked`] test as it starts: the
    /// backdown's turn (callback 45, `FUN_00177750`: his facing more than
    /// 256 off his way to the rim, the side it is off to; else the right
    /// hand) or the post defence's side (callback 49, `FUN_00177fa0`, also
    /// for the bump reaction, which runs into the stance; the
    /// holder's right hand when the holder is his nearest opponent; else
    /// by his own place, `z >= 0` on the side `x < 0`).
    fn asked(&self, i: usize, script: u8) -> bool {
        let p = &self.players[i];
        match script {
            BACKDOWN_IN => {
                let d = timing::angle_diff(self.facing_units(i), self.toward_rim(i));
                if d.abs() > 256 {
                    d < 0
                } else {
                    p.gait_channel.hand == 1
                }
            }
            // Callback 10 (`FUN_001757e8`): his nearest opponent on his
            // wanted facing's one side.
            POST_CALL => self.nearest_opponent(i).is_some_and(|o| {
                let at = crate::contact::angle(p.position, self.players[o].position);
                (timing::facing_word(p.wanted) - at) & 0x3ff < 0x200
            }),
            POST_STANCE | BUMP_REACTION => match self.handler() {
                Some(h) if self.nearest_opponent(i) == Some(h) => {
                    self.players[h].gait_channel.hand == 1
                }
                _ => {
                    let [x, z] = p.position;
                    if z >= 0.0 { x < 0.0 } else { x >= 0.0 }
                }
            },
            _ => false,
        }
    }

    /// `FUN_00163a38`: whether defender `i` takes the stance against a
    /// post-up: his nearest opponent posting (bit 11) or on the post-up's
    /// clips (`FUN_0016ec80`), `i` within 199 units of that man's way to
    /// the rim as seen from him, within 5 of him (7 on the post defence's
    /// rows) and in his cone (`FUN_00170840(man, i, 0)`).
    pub(crate) fn post_stance(&self, i: usize) -> bool {
        let Some(o) = self.nearest_opponent(i) else {
            return false;
        };
        if !(self.posting(o) || self.on_post_up_clip(o)) {
            return false;
        }
        let me = self.players[i].position;
        let from = self.players[o].position;
        let off = timing::angle_diff(crate::contact::angle(from, me), self.toward_rim(o)).abs();
        let reach = if matches!(self.channel_row(i), 718 | 719) {
            7.0
        } else {
            5.0
        };
        off <= f32::from_bits(0x4347_9c4d) as i32
            && crate::distance(me, from) <= reach
            && self.in_cone(o, i, 0)
    }

    /// One update of `i`'s stand script (the channel's step): its rolls
    /// draw from the C library's generator (callback 3); callbacks 4 and 5
    /// turn the wanted facing to his rim and to the ball, callback 7 sets
    /// the facing from the pelvis; its first update's state word (`9b`)
    /// sets or clears the box-out (bit 17). Its `95` plants a foot (the
    /// bones `jump::Plant` picks, on the model as the update before left
    /// it) and its `96` lets go; while planted, the model posed, he moves
    /// by the planted point less the bones' midpoint (`FUN_00156f48`,
    /// `0x157800..0x157954`). That carries the stop: the stand path sets
    /// no velocity, and a stand that plants (breathe_1, the defence's
    /// anticipation, protectball) swings him over the foot through its
    /// blend from the gait's frame, where the off-ball offence's stands
    /// plant nothing and he stops.
    pub(crate) fn stand_tick(&mut self, i: usize) {
        let Some(mut s) = self.players[i].stand else {
            return;
        };
        let poses = self.poses.arc();
        // The model as the channel shows it: before the first blend, the
        // clip the script started on.
        let posed = |w: &World, s: &Standing| {
            let p = poses.as_ref()?;
            let (body, place) = (w.players[i].body(), w.place(i));
            crate::trick::pose(p, s.run.shown(), s.from, body, place).or_else(|| {
                let (record, time) = s.from?;
                let frame = crate::pose::Frame {
                    record,
                    time,
                    looping: true,
                };
                p.pose(frame, body, place)
            })
        };
        let prev = posed(self, &s);
        let before = s.run.shown();
        let steps = s.steps();
        let height = self.players[i].height;
        let libc = &mut self.libc;
        let ran = s.run.step_rolling(steps, height, &mut || {
            *libc = libc.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *libc & 0x7fff_ffff
        });
        if s.run.tick == 0 {
            self.players[i].boxing = ran.set_bits & 0x20000 != 0;
            self.players[i].boxed = ran.set_bits & 0x40000 != 0;
            self.players[i].posting = ran.set_bits & 0x800 != 0;
        } else if ran.set_bits & 0x800 != 0 {
            // `into_backdown` runs into the side dribble's `9b 0x820`.
            self.players[i].posting = true;
        }
        if ran.toward_ball {
            let (p, b) = (&self.players[i], self.ball.position);
            let word = timing::angle_units(crate::math::atan2(
                p.position[0] - b[0],
                p.position[1] - b[2],
            ));
            self.players[i].wanted = crate::restart::heading_radians(word);
        }
        let mut unplant = false;
        for step in ran.ops.iter() {
            match step {
                Step::Want(w) => {
                    let p = &self.players[i];
                    let rim = self.rim_spot(crate::target_hoop(p.team));
                    let to = timing::angle_units(crate::math::atan2(
                        p.position[0] - rim[0],
                        p.position[1] - rim[1],
                    ));
                    self.players[i].wanted = crate::restart::heading_radians(to + i32::from(w));
                }
                Step::Plant(plant) if !plant.hands => {
                    if let Some(pose) = prev.as_ref() {
                        let held = s.plant.map(|(bones, _)| bones);
                        let record = s.run.shown().record;
                        let (bones, at) = self.plant_point(i, plant, pose, record, held);
                        s.plant = Some((bones, [at[0], at[2]]));
                    }
                    unplant = false;
                }
                Step::Unplant => unplant = true,
                _ => {}
            }
        }
        // `96` lets go at the motion step.
        if unplant {
            s.plant = None;
        }
        // `FUN_0019a658`: no stand's `91` sets 0x1000, so the clip's root
        // change carries him (times the height scale, turned by his
        // facing): the idle clips sway him on the spot (clip 68, `READY`,
        // about 0.5 across in the attract captures' restarts).
        if let Some(poses) = poses.as_deref() {
            let step = crate::trick::root_step(poses, before, s.run.shown());
            let p = &mut self.players[i];
            let m = crate::trick::root_motion(step, p.body(), timing::facing_word(p.heading));
            p.position[0] += m[0];
            p.position[1] += m[1];
            p.clamp_to_court();
        }
        if let Some((bones, at)) = s.plant
            && let Some(pose) = posed(self, &s)
        {
            let [a, b] =
                bones.map(|k| pose.joints[usize::from(k).min(crate::pose::BONES - 1)].position);
            let p = &mut self.players[i];
            p.position[0] += at[0] - (a[0] + b[0]) * 0.5;
            p.position[1] += at[1] - (a[2] + b[2]) * 0.5;
        }
        if ran.face
            && let Some(poses) = self.poses.get()
            && let Some(pose) = crate::trick::pose(
                poses,
                s.run.shown(),
                s.from,
                self.players[i].body(),
                self.place(i),
            )
        {
            self.players[i].heading =
                crate::restart::heading_radians(crate::trick::pelvis_word(&pose));
        }
        self.players[i].stand = (!s.run.done).then_some(s);
    }
}
