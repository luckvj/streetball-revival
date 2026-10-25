//! Shooting and the moves around it, as Street School teaches them
//! (`XLOC_GM`, `school.txt`):
//!
//! - Dunk: "While moving towards the basket, hold any Turbo and press Shoot";
//!   with all the turbos an advanced dunk ("requires a player with a high
//!   Dunks rating, or else you risk missing the dunk").
//! - Dunk to Lay-up: "hold any Turbo and press Shoot to dunk, then press Shoot
//!   again to lay-up the ball", which is harder to block but worth fewer trick
//!   points. A drive without turbo ends in a lay-up.
//! - Pump fake: "quickly tap Shoot". A shot needs the button held a moment.
//! - Fadeaway: "first move away from the hoop, and then press Shoot."
//! - Turbo block: "Stand in front of the shooter or near the hoop for a
//!   goaltend. Then hold any two Turbos and press Block."
//! - Dive for ball: "double tap any Turbo when the ball is loose."
//! - Pick: "stand near the Long Range Arc and press Strategy", and a teammate
//!   clears out the defender guarding the ball carrier.
//! - Putbacks: a rebound caught in the air near the rim, shoot to slam it.

use crate::{
    Action, Ball, BallState, Basket, Difficulty, Event, Goal, Input, Shot, TICK_SECONDS, World,
    button, distance, player::DunkStyle, street, target_hoop, tuning,
};

impl World {
    /// The shoot button with the ball (`FUN_0015a060`, raw
    /// `0x15b0f8..0x15b204`): a gamebreaker with two turbos held (the
    /// held word's 0x40000, `FUN_00159cf0`; the sim's own start), else
    /// `FUN_0016b4b8` (raw `0x16b4b8..0x16c44c`), which asks nothing of the
    /// turbo bar:
    ///
    /// - a dunk on (`FUN_0016c500(actor, 1)` 2, [`World::dunk_on`]) short
    ///   of the rim's line (|x| ≤ 36.5), or past it from beside the rim
    ///   (sectors 2..6 seen from it) within 31 units of the rim's line
    ///   (`FUN_0016b3e8`) and 2 or more out: the dunk and lay-up rows
    ///   ([`crate::timing::press_move`], `FUN_001693b8(actor, 0)`), a
    ///   turbo held or not;
    /// - a dunk on past the rim's line otherwise, or none on behind the
    ///   board (`FUN_0016f580(actor, 0)`): the shots from behind it
    ///   ([`World::behind_board_shot`]);
    /// - else a jump shot ([`crate::timing::shot_script`]; out of the
    ///   dunking area, `FUN_0016c500` 1, a runner facing the rim).
    pub(crate) fn shoot_button(&mut self, i: usize, input: &Input) {
        let team = self.players[i].team;
        let hoop = target_hoop(team);
        if input.turbos() >= 2 && self.start_gamebreaker(i, hoop) {
            return;
        }
        // game+0x94 goes once `FUN_0016b4b8` has run (`0x15b204`).
        self.fresh_take = false;
        let rim = self.rim_spot(hoop);
        let buttons = self.players[i].action_buttons;
        let window = self.dunk_on(i, true);
        let p = &self.players[i];
        let side = crate::restart::side(team.min(1));
        if window == 2 {
            let dunk = p.position[0].abs() <= 36.5 || {
                let word = if rim[0] >= 0.0 { 0x100 } else { 0x300 };
                let s = crate::timing::sector(word, rim, p.position);
                (2..=6).contains(&s)
                    && on_rim_line(crate::contact::angle(rim, p.position), side)
                    && distance(p.position, rim) >= 2.0
            };
            if dunk {
                self.press_dunk(i, hoop, buttons);
            } else {
                self.behind_board_shot(i, hoop);
            }
            return;
        }
        if crate::jump::behind_board(p.position, side) {
            self.behind_board_shot(i, hoop);
            return;
        }
        // The shot script (`FUN_0016b4b8`, `timing::shot_script`); a
        // fadeaway is decided at the script's check tick from the stick
        // (`check_pump_fake`), not at the press.
        let long = self.is_long_range(self.players[i].position, hoop);
        let script = crate::timing::shot_script(self, i, buttons, long, 0, window == 1);
        self.begin_script(i, crate::timing::SHOT_SCRIPTS[script].script);
        self.players[i].start_shot(hoop, rim, 0, false, script);
    }

    /// `FUN_001693b8(actor, 0)`'s move started ([`crate::timing::press_move`]):
    /// a dunk row as a dunk, a lay-up row as a lay-up, or the jump shot it
    /// falls back on. A dunk is advanced (its odds) only for the four-turbo
    /// slot's stand-in.
    fn press_dunk(&mut self, i: usize, hoop: usize, buttons: u16) {
        match crate::timing::press_move(self, i, buttons) {
            crate::timing::PressMove::Row { row, advanced } => {
                let r = &crate::timing::MOVE_ROWS[usize::from(row)];
                let clock = (row, r.release, r.end);
                self.begin_move(i, row);
                if r.family == 0 {
                    let style = if advanced {
                        DunkStyle::Advanced
                    } else {
                        DunkStyle::Normal
                    };
                    let spot = self.rim_side(i, hoop, 1.2);
                    self.players[i].start_dunk(hoop, spot, style, clock);
                    self.events.push(Event::Dunk { player: i, hoop });
                } else {
                    let spot = self.rim_side(i, hoop, 1.8);
                    self.players[i].start_layup(hoop, spot, clock);
                    self.events.push(Event::Layup { player: i });
                }
            }
            crate::timing::PressMove::Shot(script) => {
                let rim = self.rim_spot(hoop);
                self.begin_script(i, crate::timing::SHOT_SCRIPTS[script].script);
                self.players[i].start_shot(hoop, rim, 0, false, script);
            }
        }
    }

    /// `FUN_0016b4b8`'s shots from behind the board (`0x16c128..0x16c290`
    /// past the rim's line, `0x16b6a4..0x16b7a0` under it): more than 3.5
    /// off the middle a side fade by the side of the court and the live
    /// hand (+0x25c), else `layup_3_bboard` (`0x446416`, lay-up kind 5 at
    /// +0x26c, [`crate::timing::BOARD_LAYUP_ROW`]).
    pub(crate) fn behind_board_shot(&mut self, i: usize, hoop: usize) {
        use crate::timing::{SIDEFADE_LEFT_L, SIDEFADE_LEFT_R, SIDEFADE_RIGHT_L, SIDEFADE_RIGHT_R};
        let rim = self.rim_spot(hoop);
        let p = &self.players[i];
        let z = p.position[1];
        let hand = p.gait_channel.hand;
        if z.abs() > 3.5 {
            let near = (rim[0] < 0.0) == (z >= 0.0);
            let script = match (near, hand) {
                (true, 1) => SIDEFADE_RIGHT_R,
                (true, _) => SIDEFADE_RIGHT_L,
                (false, 2) => SIDEFADE_LEFT_L,
                (false, _) => SIDEFADE_LEFT_R,
            };
            self.begin_script(i, crate::timing::SHOT_SCRIPTS[script].script);
            self.players[i].start_shot(hoop, rim, 0, false, script);
            return;
        }
        let row = crate::timing::BOARD_LAYUP_ROW;
        let r = crate::timing::move_row(row).expect("the board's lay-up row");
        let spot = self.rim_side(i, hoop, 1.8);
        self.begin_move(i, row);
        self.players[i].start_layup(hoop, spot, (row, r.release, r.end));
        self.events.push(Event::Layup { player: i });
    }

    /// The holder's shoot press in the air (`FUN_0015a060`, raw
    /// `0x15aedc..0x15b074`): in a dunk (state bit 7) more than 1.5 from
    /// the rim (game+0x374) and 0.5 up, or in a rebound jump (bit 8, with
    /// neither bit 13 nor 3) in the half his side attacks more than 1 up (a
    /// computer's only within 33 of the middle), `FUN_0016b1e8` turns it
    /// into a lay-up in the air ([`World::midair_layup`]): School's "Dunk
    /// to Lay-up" and the tip-in. (The original's putback, two turbos and
    /// the rebound press, is `FUN_00169c98`'s, not ported: the press plays
    /// the rebound jump.)
    pub(crate) fn busy_buttons(&mut self, i: usize, pressed: u16, input: &Input) {
        if pressed & button::SHOOT == 0 || self.ball.state != BallState::Held(i) {
            return;
        }
        let p = &self.players[i];
        let hoop = target_hoop(p.team);
        let d = distance(p.position, self.rim_spot(hoop));
        let rebound = matches!(p.action, Action::ReboundJump { .. });
        let bits = crate::block::holder_bits(&p.action) | if rebound { 0x100 } else { 0 };
        let go = if bits & 0x80 != 0 {
            d > 1.5 && p.height > 0.5
        } else if bits & 0x100 != 0 && bits & 0x2008 == 0 {
            let side = crate::restart::side(p.team.min(1));
            let front = (p.position[0] >= 0.0) == (side > 0.0);
            front && p.height > 1.0 && (!input.is_computer() || p.position[0].abs() <= 33.0)
        } else {
            false
        };
        if go {
            self.midair_layup(i, hoop);
        }
    }

    /// `FUN_0016b1e8` (raw `0x16b1e8..0x16b3dc`): unless the ball is a
    /// gamebreaker's (ball+0x178), within 10 of the rim (game+0x374) the
    /// script of `0x3d8f60` `rand() % 6` picks within 5.25 (else `rand() %
    /// 10`) starts at once ([`crate::timing::MIDAIR_ROWS`]; actor+0x1e4 5,
    /// the row's lay-up kind at +0x26c); of the first six, a shooter beside
    /// the rim (the half 1 or 3 of `FUN_0018d768` from it) with the hand on
    /// that side turns his wanted facing a quarter off the line to the rim
    /// (`FUN_00175248`). His flight goes on: the script is flown from where
    /// he is ([`crate::jump`]).
    pub(crate) fn midair_layup(&mut self, i: usize, hoop: usize) {
        if self.gamebreaker_held(i) {
            return;
        }
        let rim = self.rim_spot(hoop);
        let p = &self.players[i];
        let d = distance(p.position, rim);
        if d > 10.0 {
            return;
        }
        let n = if d <= 5.25 { 6 } else { 10 };
        let k = self.rng.next_u32() % n;
        if k < 6 {
            let word = if rim[0] >= 0.0 { 0x100 } else { 0x300 };
            let a = crate::timing::angle_units(crate::math::atan2(
                rim[0] - p.position[0],
                rim[1] - p.position[1],
            ));
            let half = (((word - a + 0x200) & 0x3ff) + 0x80) & 0x3ff;
            let half = half >> 8;
            let turn = match (half, p.gait_channel.hand) {
                (1, 1) => Some(-0x100),
                (3, 2) => Some(0x100),
                _ => None,
            };
            if let Some(turn) = turn {
                let to_rim = crate::contact::angle(p.position, rim);
                let word = (to_rim + turn) & 0x3ff;
                self.players[i].wanted = crate::restart::heading_radians(word);
            }
        }
        let row = crate::timing::MIDAIR_ROWS + k as u8;
        let r = crate::timing::move_row(row).expect("a mid-air row");
        let spot = self.rim_side(i, hoop, 1.8);
        // Out of a rebound jump his rise goes on as the script's flight.
        if let Action::ReboundJump { vy, drift, up, .. } = self.players[i].action {
            self.players[i].flight =
                crate::jump::Flight::new([drift[0], if up { vy } else { 0.0 }, drift[1]]);
        }
        self.begin_move(i, row);
        self.players[i].start_midair_layup(hoop, spot, (row, r.release, r.end));
        self.events.push(Event::Layup { player: i });
    }

    /// The shot script's check tick (callbacks 17 and 20, `FUN_001726c0`
    /// and `FUN_00172348`): with the stick pushed away from the basket the
    /// script jumps into the fadeaway (the scripts whose check carries
    /// callback 17); else a shoot button no longer held is a pump fake.
    /// A computer player holds the button through the shot (the attract
    /// captures' built word keeps 0x80), so only a person fakes. The
    /// played captures: the fake flag rises `check` updates after the
    /// press (12 and 14 for the stepback and `legkick_1`, 8 for the
    /// heave) and the fade's script runs from that update.
    pub(crate) fn check_pump_fake(&mut self, i: usize, input: &Input) {
        let Action::Shooting {
            gamebreaker: 0,
            tick,
            check,
            script,
            stall,
            ..
        } = self.players[i].action
        else {
            return;
        };
        if check == 0 || i32::from(tick) - 1 - i32::from(stall) != i32::from(check) {
            return;
        }
        if crate::timing::SHOT_SCRIPTS[usize::from(script)].fades
            && crate::timing::fades_away(self, i, input)
        {
            let fade = &crate::timing::SHOT_SCRIPTS[crate::timing::FADEAWAY];
            if let Action::Shooting {
                fade: fading,
                script,
                release,
                end,
                gate,
                latest,
                ..
            } = &mut self.players[i].action
            {
                *fading = true;
                *script = crate::timing::FADEAWAY as u8;
                *release = check + fade.release;
                *end = check + fade.end;
                *gate = check + fade.gate;
                *latest = check + fade.latest;
            }
            return;
        }
        if !input.held(button::SHOOT) {
            // The script falls through its `85` to its end
            // ([`crate::held::fake_end`]). Callback 20
            // (`FUN_00172348`) reads the held word (actor+0x78) with no
            // test of whose controller it is: a computer that lets go of
            // its button fakes too. A fake out of a dribble picks it up
            // (`0x1723a0..0x1723b0`: actor+0x1a0 1 to −1).
            if self.players[i].dribble == 1 {
                self.players[i].dribble = -1;
            }
            self.players[i].action = Action::PumpFake {
                elapsed: 0.0,
                script,
                tick,
            };
            self.players[i].height = 0.0;
            // Slot 5 (callback 20): it arms the pump fake's points.
            self.post_pump_fake();
            self.events.push(Event::PumpFake { player: i });
        }
    }

    /// The lay-up leaves the hand: a short, soft shot off the glass, made
    /// more often than a jump shot and harder to block.
    pub(crate) fn release_layup(&mut self, i: usize, hoop: usize) {
        if self.ball.state != BallState::Held(i) {
            return;
        }
        // Callback 21 launches the ball from the lay-up clip's hand, where
        // the last ball update held it ([`crate::held`]). Without the
        // disc's animations the release point is the sim's own: above the
        // player, who goes up 1.8 from the rim's middle. (Reaching 0.6
        // further, as the sim used to, put the ball inside the rim's reach,
        // `r + 0.83645` from the middle, at the release, and the game's rim
        // test then knocked it away.)
        let p = &self.players[i];
        let from = if self.poses.get().is_some() {
            self.ball.position
        } else {
            [
                p.position[0],
                p.ratings.reach + p.height - 1.0,
                p.position[1],
            ]
        };
        self.lay_in(i, hoop, from);
    }

    /// Lay the ball in from `from` (a lay-up, or a dunk turned into one).
    pub(crate) fn lay_in(&mut self, i: usize, hoop: usize, from: [f32; 3]) {
        let rim = self.rim_spot(hoop);
        // Slot 11: the lay-up is tried (base 2).
        let team = self.players[i].team;
        self.stats.shot(i, team, false);
        if let Some(blocker) = self.blocker(i, 3.5, 1.2) {
            let b = self.players[blocker].ratings.blocks;
            if self.rng.chance(0.1 + 0.3 * b) {
                self.swat(blocker, i, from, rim);
                return;
            }
        }
        // The original's lay-up goes through the same release callback and
        // launcher as a jump shot (state bit 13, `FUN_0016ea40`; the
        // packet's kind 5), from inside the arc on the Shots rating.
        let call = self.shot_call(i, hoop, from, true, 0, false);
        self.launch_shot(
            hoop,
            from,
            call,
            Shot {
                shooter: i,
                hoop,
                made: call.made,
                points: 1,
                gamebreaker: 0,
                layup: true,
                tried: 0,
                slam: None,
            },
            None,
        );
    }

    /// The rewrite's stand-in goaltend, without the disc's animations: a
    /// shot on its way down above the rim can be swatted away by a
    /// defender in a turbo block jump near the basket. Each defender gets
    /// one try per shot. ("goal tend" in the original is a computer
    /// player's behaviour, `FUN_00116fb8`, not a rule.)
    pub(crate) fn check_goaltend(&mut self) -> bool {
        // With the disc's animations the block test decides it
        // ([`World::block_test`]): a ball on the rim within the ring is not
        // touched, a shot late in its flight is let go by chance, and the
        // original awards nothing for goaltending.
        if self.poses.get().is_some() {
            return false;
        }
        let Some(shot) = self.ball.shot else {
            return false;
        };
        let b = self.ball.position;
        let rim = self.hoops[shot.hoop].rim;
        if self.ball.velocity[1] >= 0.0 || b[1] < rim[1] - 0.5 {
            return false;
        }
        let team = self.players[shot.shooter].team;
        for j in 0..self.players.len() {
            let q = &self.players[j];
            let bit = 1u8 << j.min(7);
            let high = matches!(q.action, Action::Jumping { high: true, .. });
            if q.team == team || !high || shot.tried & bit != 0 {
                continue;
            }
            let reach = q.ratings.reach + q.height + 0.5;
            if distance(q.position, [b[0], b[2]]) < tuning::GOALTEND_REACH && b[1] < reach {
                if let Some(s) = &mut self.ball.shot {
                    s.tried |= bit;
                }
                let (blocks, blocker_team) = (q.ratings.blocks, q.team);
                if self.rng.chance(0.45 + 0.4 * blocks) {
                    let at = self.ball.position;
                    // The stand-in's swat is a block (slot 16).
                    let _ = blocker_team;
                    self.swat(j, shot.shooter, at, [rim[0], rim[2]]);
                    self.events.push(Event::Goaltend {
                        player: j,
                        shooter: shot.shooter,
                    });
                    return true;
                }
            }
        }
        false
    }

    /// Dunk style for scoring.
    pub(crate) fn dunk_basket(style: DunkStyle) -> street::Basket {
        street::Basket::Dunk {
            advanced: style == DunkStyle::Advanced,
        }
    }
}

/// `FUN_0016b3e8`: the angle from the rim to the shooter (game+0x35c,
/// `FUN_0016f298`) within `trunc(31.75)` units of the rim's line, the
/// court's width: at the end attacked by a side of `side` +1, 993 or more
/// or 543 or less; at the other, 481 or more or 31 or less.
fn on_rim_line(angle: i32, side: f32) -> bool {
    let k = f32::from_bits(0x41fe_4f77) as i32;
    if side > 0.0 {
        angle >= 0x400 - k || angle <= 0x200 + k
    } else {
        angle >= 0x200 - k || angle <= k
    }
}

/// Native rim plane height from FUN_001515a8 and FUN_001526d0.
pub const NATIVE_RIM_HEIGHT: f32 = 11.91355;
/// Native inner rim radius (0.75 ft).
pub const NATIVE_RIM_RADIUS: f32 = 0.75;
/// Native outer rim radius including ring thickness (0.83645 ft) from FUN_001526d0.
pub const NATIVE_RIM_OUTER_RADIUS: f32 = 0.83645;
/// Native rim entry critical cone angle (0.86393803 rad ≈ 49.5°) from FUN_001526d0.
pub const NATIVE_RIM_ENTRY_CONE_ANGLE: f32 = 0.86393803;
/// Native net bottom plane height from FUN_00153550.
pub const NATIVE_NET_BOTTOM_HEIGHT: f32 = 10.5;
/// Native net cone scaling denominator from FUN_00153550.
pub const NATIVE_NET_CONE_SCALE: f32 = 1.4567747;
/// Native backboard bounding box in X (38.0..38.25 ft) from FUN_00151928.
pub const NATIVE_BACKBOARD_BOUNDS_X: [f32; 2] = [38.0, 38.25];
/// Native backboard bounding box in Y (11.5..15.0 ft) from FUN_00151928.
pub const NATIVE_BACKBOARD_BOUNDS_Y: [f32; 2] = [11.5, 15.0];
/// Native backboard bounding box in Z (-3.0..3.0 ft) from FUN_00151928.
pub const NATIVE_BACKBOARD_BOUNDS_Z: [f32; 2] = [-3.0, 3.0];
/// Native rim collar neck bounding range in X (36.75..38.0 ft) from FUN_00152418.
pub const NATIVE_RIM_COLLAR_X: [f32; 2] = [36.75, 38.0];
/// Native rim collar neck half-width in Z from FUN_00152418.
pub const NATIVE_RIM_COLLAR_Z_MAX: f32 = 0.25;

/// A defender as the contest (`FUN_001801e0`) reads him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContestDefender {
    /// `FUN_0018cde0`: his distance from the shooter.
    pub distance_to_shooter: f32,
    /// `FUN_0017dd10`: his distance from the rim the shooter attacks.
    pub distance_to_rim: f32,
    /// The record's height in feet (attribute 8 / 12, `FUN_001da4e0(rec,
    /// 8)`): the contest reads no jump.
    pub height_ft: f32,
    /// Blocks, 0..99 (`FUN_001d7d00(rec, 5)`).
    pub blocks: i32,
}

/// `FUN_001801e0`: how hard a defender contests a shot, 0..2. Not at all
/// from more than 15 farther from the rim than the shooter; else the
/// angle up from the shooter's height (the record's, no jump) to the
/// defender's height (at least 4) plus his reach (`(h − 4) · 0.375 +
/// 0.5`, at most 2.5) plus `blocks · 6.5 / 99 + 3`, over his distance less
/// 3.6 (at least 1), in 10-bit units (the polynomial arctangent at
/// `0x4b3140`, `· 162.97466 + 0.5` truncated): over 165 (`0x43257a31`
/// truncated) is 2, over 108 (`0x42d92d5b`) 1.
pub fn evaluate_contest_angle(
    shooter_height_ft: f32,
    shooter_rim_dist: f32,
    defender: &ContestDefender,
) -> i32 {
    // `0x180220`: the shooter nearer than the defender's distance less 15.
    if shooter_rim_dist < defender.distance_to_rim - 15.0 {
        return 0;
    }
    let def_h = defender.height_ft.max(4.0);
    let reach = ((def_h - 4.0) * 0.375 + 0.5).min(2.5);
    // The original's order of operations (`0x1802ec`, `0x180340`).
    let dy = ((def_h + reach) + ((defender.blocks as f32 * 6.5) / 99.0 + 3.0)) - shooter_height_ft;
    let dx = (defender.distance_to_shooter - 3.6).max(1.0);
    // dx is at least 1, so the quadrant fix-ups never apply.
    let angle = crate::ordinary::native_atan(dy / dx);
    let units = (angle * 162.974_66 + 0.5) as i32;
    if 165 < units {
        2
    } else if 108 < units {
        1
    } else {
        0
    }
}

/// The shooter as the shot's quality (`FUN_001805e8`) reads him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotView {
    /// `FUN_0017dd10`: his distance from the rim he attacks.
    pub distance_to_rim: f32,
    /// Shots, Threes and Dunks, 0..99 (`FUN_001d7d00(rec, 0 / 1 / 2)`).
    pub shots: i32,
    pub threes: i32,
    pub dunks: i32,
    /// `FUN_0017deb0(0.5)`: beyond the long-range line by half a unit.
    pub outside: bool,
    /// His side's meter is full (team+0xc == 50000).
    pub meter_full: bool,
    /// The record's height in feet (attribute 8 / 12).
    pub height_ft: f32,
}

/// `FUN_001805e8`: the AI's view of a shot, 0..7 (it is not what decides a
/// make: the release callback below does). Within 30.75 of the rim and
/// not beyond the line: Shots (Dunks within 6, their mean within 10, when
/// better) plus itself scaled by distance (1 within 5, `1 − (d − 5) · 1.3
/// / 18.75` to 23.75, −0.3 beyond), over 25, plus 4, truncated (`cvt.w.s`),
/// 10 or more taken as 8; beyond the line 7 with a full meter, else
/// `Threes / 25 + 4` (3 past 27.85). Farther, 7 with a full meter within
/// 38.75, else 0. Then the nearest defender who is not down
/// (`FUN_0017fd08`; 30 away when there is none): within 18 one less within
/// 10 and another within 6, his contest ([`evaluate_contest_angle`]) off
/// and one more with a contest within 14, and the next defender who is not
/// down within 18 one less within 8 and his contest off; beyond 18 one
/// more. Kept to 0..7.
///
/// The full meter's long shot drops to 5 for a shooter under 25 Threes
/// whose actor answers its vt+0x5c over 7 (`0x180904`); that call is not
/// traced, so the rewrite keeps 7.
pub fn evaluate_shot_quality_tier(
    shot: &ShotView,
    first: Option<ContestDefender>,
    second: Option<ContestDefender>,
) -> i32 {
    let d = shot.distance_to_rim;
    let mut tier: i32 = 0;
    if d < 30.75 {
        if !shot.outside {
            let (shots, dunks) = (shot.shots as f32, shot.dunks as f32);
            let mut rating = shots;
            if shots < dunks && d < 10.0 {
                rating = if d < 6.0 {
                    dunks
                } else {
                    (shots + dunks) * 0.5
                };
            }
            let scaled = if d <= 5.0 {
                1.0 * rating
            } else if 23.75 <= d {
                -0.3 * rating
            } else {
                (1.0 - ((d - 5.0) * 1.3) / 18.75) * rating
            };
            // `0x180878`: `cvt.w.s` truncates; `0x180880`: 10 or more is 8.
            tier = ((rating + scaled) / 25.0 + 4.0) as i32;
            if tier >= 10 {
                tier = 8;
            }
        } else if shot.meter_full {
            tier = 7;
        } else {
            tier = shot.threes / 25 + if 27.85 < d { 3 } else { 4 };
        }
    } else if d < 38.75 && shot.meter_full {
        tier = 7;
    }
    let near = first.map_or(30.0, |f| f.distance_to_shooter);
    if near < 18.0 {
        if near < 10.0 {
            tier -= 1;
        }
        if near < 6.0 {
            tier -= 1;
        }
        if let Some(f) = first {
            let contest = evaluate_contest_angle(shot.height_ft, d, &f);
            if contest != 0 {
                tier -= contest;
                if near < 14.0 {
                    tier -= 1;
                }
            }
        }
        if let Some(s) = second
            && s.distance_to_shooter < 18.0
        {
            if s.distance_to_shooter < 8.0 {
                tier -= 1;
            }
            tier -= evaluate_contest_angle(shot.height_ft, d, &s);
        }
    } else {
        tier += 1;
    }
    tier.clamp(0, 7)
}

// ---------------------------------------------------------------------
// Making or missing a shot, as the original decides it.
//
// The release callback of a jump shot or lay-up (the code at
// `0x1729f8..0x17356c`, which Ghidra folds into the stub `FUN_00333ef8`)
// works out a percentage, rolls `rand() % 100` against it (`0x1731a8`),
// picks how the shot goes in or misses, and hands (angle, made, kind,
// base points) to the shot launcher `FUN_0014aaf8` (`0x1734f4`). The
// launcher aims at the basket's middle 11.9567747 up (`0x413f4ef3`,
// `0x14abbc`) or, for a rim-in or rim-out, searches 50 random aims for a
// flight that ends the way it was told to. `FUN_001805e8` (the shot tier)
// is only the AI's view of a shot and plays no part here.
// ---------------------------------------------------------------------

/// Where the launcher aims a make's or a miss's ball
/// (`FUN_0014aaf8`, `0x14abbc`): 0.043225 (the rim's tube) above the rim
/// plane, over the basket's middle.
pub const NATIVE_AIM_HEIGHT: f32 = 11.956_775;

/// Callback 21's bar on the shooter (actor+0x1a8 = `fps / 3`, raw
/// `0x172a44..0x172a64`): updates his own tests leave the ball alone after
/// the release.
pub const SHOOTER_BAR: u16 = 20;

/// The release callback's verdict, as it is passed to `FUN_0014aaf8`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShotCall {
    /// The make chance in percent.
    pub percent: i32,
    pub made: bool,
    /// The launcher's aim (`0x3d6588`): 0 straight at the basket (a make
    /// through the middle; a miss short of or off the side of it), 1 off
    /// the glass, 2 at the front of the rim, 3 the back, 4 and 5 the sides.
    pub kind: u8,
    /// The launch angle (`0x3d6584`).
    pub angle: f32,
}

/// What the release callback reads about the shooter and the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShotOdds {
    /// From the basket across the floor (`FUN_0016e930`).
    pub distance: f32,
    pub long: bool,
    /// Shots, or Threes from long range, 0..99 (`FUN_00171670`).
    pub rating: i32,
    /// The shooter stands in the half of the basket shot at
    /// (`FUN_00156db0` against the sign of the shooter's x).
    pub frontcourt: bool,
    /// The game mode's catch-up bonus (the game object's virtual +0x44).
    pub catch_up: i32,
    /// Easy and Hard change the odds only when some team is human
    /// (`game+0x9a/0x9b`): `None` when none is, else whether the shooter's
    /// is.
    pub human: Option<bool>,
    pub difficulty: Difficulty,
    /// Opponents within 7 of the shooter (`FUN_0018d0c0`) ...
    pub near: u32,
    /// ... and how many of those have state bit 12 set (`FUN_0016e9c0`).
    pub contesting: u32,
    /// 0, or the gamebreaker in play (`ball+0x178`, `FUN_00141018`).
    pub gamebreaker: u8,
}

/// `FUN_00171690`: the base make percentage, and the distance the callback
/// goes on with (a mid-range shot's clamped to 22). Inside the arc, at most
/// 5 from the basket `Shots/100 × 15 + 80`, else `Shots/100 × 40 + 50`
/// less `min(d, 22)/22 × 15`, at least 45; beyond it `Threes/99 × 25 + 10`,
/// less `min(d − 22, 12)/12 × 10` from 24.5 out. Every step truncates
/// (`cvt.w.s`).
pub(crate) fn base_percent(distance: f32, long: bool, rating: i32) -> (i32, f32) {
    let r = rating as f32;
    if long {
        let mut percent = (r / 99.0 * 25.0 + 10.0) as i32;
        if distance >= 24.5 {
            percent -= ((distance - 22.0).min(12.0) / 12.0 * 10.0) as i32;
        }
        (percent, distance)
    } else if distance <= 5.0 {
        ((r / 100.0 * 15.0 + 80.0) as i32, distance)
    } else {
        let d = distance.min(22.0);
        let percent = (r / 100.0 * 40.0 + 50.0) as i32 - (d / 22.0 * 15.0) as i32;
        (percent.max(45), d)
    }
}

/// The release callback's make percentage (`0x172a44..0x1731a4`) for a
/// jump shot or lay-up. On Easy, with a person playing, the person's team
/// gets twice the catch-up bonus and +40 within 33.75, the computer's none
/// and −10; on Hard the person's team gets no catch-up. With nobody within
/// 7 a shot inside 33.75 gets `rating/99 × 20` (31 from long range); else
/// 5 comes off per contesting opponent. A gamebreaker replaces all of it
/// with `rating × 3/2 + 55` (Level 2: 100). The wrong half of the court is
/// 3, the floor is 3, and outside a gamebreaker the cap is 98.
pub(crate) fn shot_percent(o: &ShotOdds) -> i32 {
    let (base, distance) = base_percent(o.distance, o.long, o.rating);
    let mut percent = if o.frontcourt { base } else { 3 };
    let mut catch_up = o.catch_up;
    if let Some(human) = o.human {
        match o.difficulty {
            Difficulty::Easy => {
                catch_up = if human { catch_up * 2 } else { 0 };
                if distance <= 33.75 {
                    percent += if human { 40 } else { -10 };
                }
            }
            Difficulty::Hard if human => catch_up = 0,
            _ => {}
        }
    }
    if o.gamebreaker >= 2 {
        percent = 100;
    } else if o.gamebreaker == 1 {
        percent = o.rating * 3 / 2 + 55;
    } else {
        percent += catch_up;
        if o.near > 0 {
            percent -= 5 * o.contesting as i32;
        } else if distance < 33.75 {
            let open = if o.long { 31.0 } else { 20.0 };
            percent += (o.rating as f32 / 99.0 * open) as i32;
        }
    }
    if !o.frontcourt {
        percent = 3;
    }
    percent = percent.max(3);
    if o.gamebreaker == 0 {
        percent = percent.min(98);
    }
    percent
}

/// The catch-up bonus (`0x3dc19a`, and the identical `0x3dc3ca`: the third
/// 31×3 signed-byte table of `FUN_00186638`'s groups 1 and 2), for the side
/// `behind` (−15..−1, clamped) and the game's stage (0..2). Ahead or level
/// it is 0.
const CATCH_UP: [[i8; 3]; 15] = [
    [10, 15, 20],
    [10, 15, 20],
    [10, 15, 20],
    [10, 15, 20],
    [10, 15, 20],
    [10, 15, 20],
    [9, 14, 18],
    [8, 12, 16],
    [7, 10, 14],
    [6, 8, 12],
    [5, 6, 12],
    [4, 5, 10],
    [3, 5, 10],
    [2, 5, 10],
    [2, 5, 10],
];

pub(crate) fn catch_up_row(lead: i64, stage: usize) -> i32 {
    let row = lead.clamp(-15, 15) + 15;
    if row < 15 {
        i32::from(CATCH_UP[row as usize][stage.min(2)])
    } else {
        0
    }
}

/// The catch-up tables' stage (0..2) from the two sides' totals: the
/// larger of the two against the game's bounds (game+0x4b0/+0x4b4), 2 from
/// `late`, 1 from `early`. The points game takes the larger with `pmaxw`
/// (`0x12eb6c`, MMI0 function 3; the `pextlw` with `$zero` after it
/// zero-extends the low word). The project's Ghidra copy rewrites MMI as
/// `or` (`tools/ee_patch.py`), which is why its decompile shows the
/// scaled scores ORed; the trick-point and Gamebreaker Wins games take
/// the larger with `slt`/`movn` (`0x18c810`, `0x145c44`).
pub(crate) fn catch_up_stage(own: i64, other: i64, early: i64, late: i64) -> usize {
    let most = own.max(other);
    if most >= late {
        2
    } else if most >= early {
        1
    } else {
        0
    }
}

/// The level an advanced dunk subtracts: the dunk move's own (`player+0x24c`).
/// The original copies it from the dunk table at `*(0x3d17a8)`, eleven
/// `{turbo mask, level}` pairs (`0x43d3b0`; layouts A/B read the same
/// masks at `0x43d408`) whose level is the number of turbo buttons in the
/// combo: so the slot the latched turbo buttons match
/// ([`crate::player::NativeTricks::slot`]), or with no slot matched, the
/// turbo buttons held.
pub fn dunk_move_level(buttons: u16) -> i32 {
    match crate::player::NativeTricks::slot(buttons) {
        Some(slot) => i32::from(crate::player::TRICK_SLOT_LEVELS[slot]),
        None => [
            crate::button::L1,
            crate::button::R1,
            crate::button::L2,
            crate::button::R2,
        ]
        .into_iter()
        .filter(|&b| buttons & b != 0)
        .count() as i32,
    }
}

/// The release callback's dunk branch (`0x172e88..0x173080`, then the
/// shared tail to `0x17315c`). Dunks 0..99 gives a tier: 1..4 by 20s, 5
/// for 81..90, 6 for 91..99, with the fraction `f` inside it (an exact
/// multiple of 20 is the tier below with `f = 1`; 0 is nothing at all).
/// The tier loses 2, or the move's level for an advanced dunk (`FUN_0016e9d0`
/// or `FUN_00171de8 & 0x800`); then -3 is `10f`, -2 `10(1-f) + 40f`, -1
/// `40(1-f) + 70f`, 0 `70(1-f) + 98f`, above 0 98, below -3 0 (every step
/// truncated). The catch-up bonus is added; on Easy with a person playing
/// the person's dunks get +20 and the computer's -10; the wrong half is 3;
/// the floor is 0; the cap is 98. A gamebreaker dunk is 100.
pub(crate) fn dunk_percent(
    rating: i32,
    advanced_level: Option<i32>,
    catch_up: i32,
    human: Option<bool>,
    difficulty: Difficulty,
    frontcourt: bool,
    gamebreaker: bool,
) -> i32 {
    if gamebreaker {
        return 100;
    }
    let (tier, f) = match rating {
        r if r <= 0 => (0, 0.0),
        r if r <= 80 => {
            if r % 20 == 0 {
                (r / 20, 1.0)
            } else {
                (r / 20 + 1, (r % 20) as f32 / 20.0)
            }
        }
        r if r <= 90 => (5, (r - 80) as f32 / 10.0),
        r => (6, ((r - 90) as f32 / 10.0).clamp(0.0, 1.0)),
    };
    let mut percent = if tier <= 0 {
        0
    } else {
        let lowered = tier - advanced_level.unwrap_or(2);
        let blend = |a: f32, b: f32| (a * (1.0 - f) + b * f) as i32;
        match lowered {
            l if l < -3 => 0,
            -3 => (10.0 * f) as i32,
            -2 => blend(10.0, 40.0),
            -1 => blend(40.0, 70.0),
            0 => blend(70.0, 98.0),
            _ => 98,
        }
    };
    percent += catch_up;
    if let Some(human) = human
        && difficulty == Difficulty::Easy
    {
        percent += if human { 20 } else { -10 };
    }
    if !frontcourt {
        percent = 3;
    }
    percent.clamp(0, 98)
}

impl World {
    /// [`dunk_percent`] for `player` dunking on `hoop`.
    pub(crate) fn dunk_call(&self, player: usize, hoop: usize, advanced: bool) -> i32 {
        let p = &self.players[player];
        let side = if self.hoops[hoop].rim[0] >= 0.0 {
            1.0
        } else {
            -1.0
        };
        dunk_percent(
            (p.ratings.dunks * 99.0).round().clamp(0.0, 99.0) as i32,
            // `FUN_00171de8 & 0x800`: the dunk in the slot the turbos
            // pick is advanced. Without the player's dunk set, the
            // rewrite's four-turbo advanced dunk stands in.
            match p.native_dunks {
                Some(mask) => crate::player::NativeTricks::slot(p.action_buttons)
                    .filter(|&slot| mask & (1 << slot) != 0)
                    .map(|_| dunk_move_level(p.action_buttons)),
                None => advanced.then(|| dunk_move_level(p.action_buttons)),
            },
            catch_up_bonus(self, p.team),
            self.teams
                .iter()
                .any(|t| t.human)
                .then(|| self.teams[p.team.min(1)].human),
            self.rules.difficulty,
            (p.position[0] >= 0.0) == (side > 0.0),
            // The sim's gamebreakers are jump shots; its dunks never are.
            false,
        )
    }
}

/// The game mode's catch-up bonus for `team`, the game object's virtual
/// +0x44 called as `(2, side)`; each reads its table's signed byte at
/// `(lead + 15) × 3 + stage` (`lb`), the table found by `FUN_00186538`
/// (31 rows of 3).
/// - Points (`FUN_0012eac0`, raw `0x12eac0..0x12ebe0`): both scores scaled
///   to 21 (`points × 21 / target`, `div`), the lead clamped to ±15 the
///   row, the larger scaled score against the bounds the game drew once
///   ([`World::stage_bounds`], `FUN_0012e798`) the stage.
/// - Trick points (`FUN_0018c788`): the trick-point lead clamped to
///   ±50,000 × 31 / 100,000, the stage by the larger total against a
///   third and two thirds of the target (`FUN_0018c5c0`).
/// - Gamebreaker Wins (`FUN_00145b48`): meters in 500s (`cvt.w.s`, then
///   `div`), 100 more for a side with a Level 1 gamebreaker scored (side
///   stat 0xc), the lead clamped to ±span × 31 / 2·span, the stage by the
///   larger against a third and two thirds of the span (`FUN_001457f8`);
///   the span is 100 when game+0xf8 is 0 and 200 when it is 1 (the sim
///   takes those for Level 1 and Level 2, the record's +0xf0 holding the
///   level less one).
fn catch_up_bonus(world: &World, team: usize) -> i32 {
    catch_up_cell(world, team).map_or(0, |(lead, stage)| catch_up_row(lead, stage))
}

/// The catch-up tables' cell for `team`: the clamped lead (the row less 15)
/// and the stage, as the game mode's virtual +0x44 computes them for any
/// of its tables (the AI's skill blend reads table 0 the same way,
/// `crate::ai`). `None` for a mode without tables.
pub(crate) fn catch_up_cell(world: &World, team: usize) -> Option<(i64, usize)> {
    let (us, them) = (team.min(1), 1 - team.min(1));
    Some(match world.rules.goal {
        Goal::Points(target) => {
            let target = i64::from(target.max(1));
            let scaled = |t: usize| i64::from(world.score[t]) * 21 / target;
            let (own, other) = (scaled(us), scaled(them));
            let [early, late] = world.stage_bounds.map(i64::from);
            (own - other, catch_up_stage(own, other, early, late))
        }
        Goal::TrickPoints(target) => {
            let (own, other) = (
                i64::from(world.teams[us].trick_points),
                i64::from(world.teams[them].trick_points),
            );
            let target = i64::from(target);
            let lead = (own - other).clamp(-50_000, 50_000) * 31 / 100_000;
            (lead, catch_up_stage(own, other, target / 3, target * 2 / 3))
        }
        Goal::Gamebreaker(level) => {
            let span: i64 = match level {
                1 => 100,
                2 => 200,
                _ => return None,
            };
            let total = |t: usize| {
                let scored = world.stats.team(t, crate::stats::LEVEL1) != 0;
                i64::from(world.teams[t].meter / 500) + if scored { 100 } else { 0 }
            };
            let (own, other) = (total(us), total(them));
            let lead = (own - other).clamp(-span, span) * 31 / (2 * span);
            (lead, catch_up_stage(own, other, span / 3, span * 2 / 3))
        }
    })
}

/// The dunk path's pace bounds, units an update: `DAT_004b84a4` and
/// `DAT_004b84a0`, `5280 / (fps × 3600) × 10` and `× 20` (10 and 20 mph a
/// frame), set at `0x153e44..0x153e84`.
pub const DUNK_PACE: [f32; 2] = [f32::from_bits(0x3e7a_4fa5), f32::from_bits(0x3efa_4fa5)];

/// Where the launcher's dunk path puts the ball (raw `0x14ad40..0x14ae40`):
/// from the hand (the held ball's place, which followed the hand in the
/// last update) toward the basket's `middle` no farther than `0.75 − r`
/// along the line between them (all three axes: `vsub`, the squares
/// summed, `vsqrt`), then no lower than `r + 12`.
pub(crate) fn dunk_place(hand: [f32; 3], middle: [f32; 3], radius: f32) -> [f32; 3] {
    let off = [
        hand[0] - middle[0],
        hand[1] - middle[1],
        hand[2] - middle[2],
    ];
    let d = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
    let mut at = hand;
    if d > 0.0001 {
        let k = d.min(0.75 - radius) / d;
        at = [
            middle[0] + off[0] * k,
            middle[1] + off[1] * k,
            middle[2] + off[2] * k,
        ];
    }
    at[1] = at[1].max(radius + 12.0);
    at
}

/// The dunk path's pace (raw `0x14ac7c..0x14acf8`): the length of the
/// ball's last move (`ball+0xa0`, the hand's) held to [`DUNK_PACE`], or
/// the top one outright when `top` (a rebound jumper's dunk,
/// `FUN_0016e9f0`, or an actor with `+0x1e8` 0x15).
pub(crate) fn dunk_pace(moved: f32, top: bool) -> f32 {
    if top || DUNK_PACE[1] < moved {
        DUNK_PACE[1]
    } else if moved < DUNK_PACE[0] {
        DUNK_PACE[0]
    } else {
        moved
    }
}

/// A straight flight's timer (`ball+0x158`, raw `0x14bd30..0x14bda0`):
/// `updates` to the aim, `vy` the launch's rise an update; the rise at the
/// aim `v = vy − g·updates`, then `ceil(updates + (v + √(v² + 2g·0.043225))
/// / g) + 1`, in the original's order (`cvt.w.s` truncates, one added when
/// it fell short).
pub(crate) fn straight_timer(updates: f32, vy: f32) -> u32 {
    let g = crate::GRAVITY * TICK_SECONDS * TICK_SECONDS;
    let v = -g * updates + vy;
    let t = updates + (v + (v * v + (g + g) * 0.043225).sqrt()) / g;
    let whole = (t as i32) as f32;
    let up = if whole < t { whole + 1.0 } else { whole };
    (up as i32 + 1).max(1) as u32
}

/// A native random draw as the launcher scales it: `(rand() & 0x7fffffff)
/// × 2⁻³¹` (`0x30000000`).
fn unit(rng: &mut crate::rng::Rng) -> f32 {
    (rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000)
}

/// Turn `v` about the vertical by `angle` (the launcher's quaternion
/// `q ⊗ v ⊗ q̄`, `FUN_002f64b0` about `(0, 1, 0)`).
fn turn(v: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = (libm::sinf(angle), libm::cosf(angle));
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

impl World {
    /// The release callback (`0x1729f8..0x17356c`) for `player`'s jump
    /// shot or lay-up at `hoop`, the ball leaving the hand at `from`:
    /// the make roll, the aim kind and the launch angle.
    ///
    /// Kinds: a lay-up that can bank (`FUN_0014faf0`) goes off the glass
    /// half the time; a make is otherwise straight in half the time, the
    /// rest roll in off the rim (2..5). A jump shot in a gamebreaker rolls
    /// off a side (4, 5); a make is straight in half the time; a miss from
    /// beyond 25 is an airball or brick with chance
    /// `(max(10 − (rating + 1)/10, 0) + 1)/100`; other misses and the other
    /// makes go off the rim (2..5), or 1..5 where a bank can reach. While
    /// either side's meter is full no miss is an airball. The angle is the
    /// aim routine's `π/4 + max((20 − d)/20, 0) · π/8`, or 3π/8 for a lay-up
    /// let go below 12 (or behind the board's plane).
    pub(crate) fn shot_call(
        &mut self,
        player: usize,
        hoop: usize,
        from: [f32; 3],
        layup: bool,
        gamebreaker: u8,
        long: bool,
    ) -> ShotCall {
        let p = &self.players[player];
        let rim = self.rim_spot(hoop);
        let side = if self.hoops[hoop].rim[0] >= 0.0 {
            1.0
        } else {
            -1.0
        };
        // `FUN_00171670` → `FUN_001d7d00`: Shots, or Threes from long range,
        // where the "Easy/Hard 2 Pointers" cheats (singleton+0x4c) force 99
        // or 0. In Street scoring the long-range basket is the two-pointer.
        let rating = if !long {
            p.ratings.shots
        } else if self.rules.easy_2_pointers {
            1.0
        } else if self.rules.hard_2_pointers {
            0.0
        } else {
            p.ratings.threes
        };
        let rating = (rating * 99.0).round().clamp(0.0, 99.0) as i32;
        let distance = distance(p.position, rim);
        // `FUN_0018d0c0` reads the game's player-to-player distances; the
        // original skips opponents with animation flag 0x40 or state bit 16
        // (meaning not traced) and counts as contesting those with state
        // bit 12, which the sim takes to be a block jump (an inference).
        let (mut near, mut contesting) = (0, 0);
        for q in self.players.iter().filter(|q| q.team != p.team) {
            if distance_to(q.position, p.position) <= 7.0 {
                near += 1;
                contesting += u32::from(matches!(q.action, Action::Jumping { .. }));
            }
        }
        let odds = ShotOdds {
            distance,
            long,
            rating,
            frontcourt: (p.position[0] >= 0.0) == (side > 0.0),
            catch_up: catch_up_bonus(self, p.team),
            human: self
                .teams
                .iter()
                .any(|t| t.human)
                .then(|| self.teams[p.team.min(1)].human),
            difficulty: self.rules.difficulty,
            near,
            contesting,
            gamebreaker,
        };
        let percent = shot_percent(&odds);
        let made = ((self.rng.next_u32() % 100) as i32) < percent;
        self.ball.rolled = made;
        let reach = base_percent(odds.distance, odds.long, odds.rating).1;
        let bank =
            crate::ball::bank_possible(from, side, self.ball.radius, self.hoops[hoop].rim[2]);
        let rim_kind = |rng: &mut crate::rng::Rng| {
            if bank {
                1 + (rng.next_u32() % 5) as u8
            } else {
                2 + (rng.next_u32() & 3) as u8
            }
        };
        let kind = if layup {
            if bank && self.rng.next_u32() & 1 != 0 {
                1
            } else if made && self.rng.next_u32() & 1 != 0 {
                0
            } else {
                2 + (self.rng.next_u32() & 3) as u8
            }
        } else if gamebreaker > 0 {
            4 + (self.rng.next_u32() & 1) as u8
        } else if made {
            if self.rng.next_u32() & 1 != 0 {
                0
            } else {
                rim_kind(&mut self.rng)
            }
        } else {
            let airball = (10 - (rating + 1) / 10).max(0);
            if reach > 25.0 && ((self.rng.next_u32() % 100) as i32) <= airball {
                0
            } else {
                rim_kind(&mut self.rng)
            }
        };
        let full = self.teams.iter().any(|t| t.meter_full());
        let kind = if full && kind == 0 && !made {
            rim_kind(&mut self.rng)
        } else {
            kind
        };
        let steep = f32::from_bits(0x3f96_cbe4);
        let angle = if layup && (from[1] < 12.0 || crate::ball::BOARD_FACE - from[0].abs() < 0.0) {
            steep
        } else {
            ((20.0 - reach) / 20.0).max(0.0) * f32::from_bits(0x3ec9_0fda)
                + f32::from_bits(0x3f49_0fdb)
        };
        ShotCall {
            percent,
            made,
            kind,
            angle,
        }
    }

    /// The shot launcher `FUN_0014aaf8` puts the ball in the air from
    /// `from` toward `hoop` as `call` asks. `dunk` is the dunk path's pace
    /// in units an update ([`World::launch_dunk`]), `None` for a jump shot
    /// or lay-up.
    ///
    /// It aims at the basket's middle [`NATIVE_AIM_HEIGHT`] up, a bank shot
    /// at that point mirrored behind the glass (`FUN_0014fa60`). A make
    /// straight in or off the glass flies there at the requested angle,
    /// steepened if need be to drop through the ring (`FUN_0014fc00`); a
    /// straight miss flies `r + 1.83645` off the middle in a random
    /// direction on the court's side. Other kinds try 50 aims around the
    /// ring (at a random angle within 45° of the kind's direction, at
    /// `0.83645 − (r + 0.08645)·u` from the middle, `(r + 0.83645) −
    /// (2r + 0.08645)·u` for the front, lifted to sit on the tube, its x
    /// stretched by 1/0.75 for a bank), watch each flight
    /// ([`crate::Ball::forecast`]) and keep the one that ends as asked
    /// (made or not) and stays longest around the basket; a miss takes the
    /// first that works half the time. The 50th try, if none has worked,
    /// aims `0.75 − r` (make) or `r + 0.83645` (miss) beyond the middle with
    /// no turn; if still none works it flies straight (kind 0). The flight
    /// times come from `FUN_001505a0` ([`crate::ball::shot_flight_time`]).
    ///
    /// The dunk path flies every aim at its pace instead
    /// ([`crate::Ball::launch_at_pace`]), steepens nothing, and its miss
    /// takes the first try that works, with no draw for it (`0x14ae8c`:
    /// the half-time draw is the other path's).
    ///
    /// The timer (`ball+0x158`, which [`crate::Ball::fly`] runs down): for a
    /// flight the tries watched, one more than the update of its last
    /// contact ([`crate::ball::Forecast::last`], raw `0x14b9d4..0x14b9e0`);
    /// for a straight one, `T` the updates to the aim and `v` the rise
    /// there (`vy − gT`), `ceil(T + (v + √(v² + 2g·0.043225))/g) + 1`
    /// (`0x14bd30..0x14bda0`): the flight to the aim and through the
    /// rim's tube, and one more.
    ///
    /// The sim's own guard: the original trusts a straight flight without
    /// watching it; here a straight make that would not drop is replaced by
    /// one through the middle without the glass, and a straight miss that
    /// would drop gets up to seven more random directions.
    pub(crate) fn launch_shot(
        &mut self,
        hoop: usize,
        from: [f32; 3],
        call: ShotCall,
        shot: Shot,
        dunk: Option<f32>,
    ) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
        let rim = self.hoops[hoop].rim;
        let side = if rim[0] >= 0.0 { 1.0 } else { -1.0 };
        let r = self.ball.radius;
        self.ball.state = BallState::Shot;
        self.ball.shooter = Some(shot.shooter as u8);
        self.ball.shot = Some(shot);
        self.ball.pass = None;
        self.ball.rebound = false;
        // Callback 21 after the launcher: the shooter's side's give 'n go
        // is over ([`World::shot_ends_hold`]).
        self.shot_ends_hold(shot.shooter);
        // Slot 11 (`0x141558`, the launcher's post of `0x3c6530`; a dunk's
        // slot 12 before it does nothing): the game's phase 3.
        self.game_phase.launched(self.sequence.is_some());
        let middle = [rim[0], NATIVE_AIM_HEIGHT, rim[2]];
        let mut target = middle;
        if call.kind == 1 {
            target[0] = crate::ball::bank_mirror(target[0], side, r);
        }
        let toward = |to: [f32; 3]| [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let rims = self.hoops.map(|h| h.rim);
        let reversed = self.layup_reversed(shot.shooter);
        // Each flight leaves with the shot's backspin and, off the glass
        // (kind 1), the bank's frictionless first touch (`ball+0x198`, set
        // at `0x0014bda4`), and is watched as it will fly. (The original's
        // look-ahead clears `ball+0x198` for its watch, so its kind-1 misses
        // are watched with the glass's friction that their flights skip.)
        // A dunk's ball keeps half its speed in the net (`ball+0x184` 3 or
        // 4). Also the updates to the aim.
        let trial = |ball: &Ball, aim: [f32; 3], angle: f32, bank: bool| {
            let mut ball = ball.clone();
            let to = [from[0] + aim[0], from[1] + aim[1], from[2] + aim[2]];
            let updates = match dunk {
                Some(pace) => ball.launch_at_pace(from, to, pace),
                None => {
                    let t = crate::ball::shot_flight_time(from, to, angle);
                    ball.launch(from, to, t);
                    t * crate::TICK_HZ as f32
                }
            };
            ball.shot_spin(reversed);
            ball.bank = bank;
            ball.dunked = dunk.is_some();
            let forecast = ball.forecast(rims, rim);
            (ball, forecast, updates)
        };
        let straight = call.kind == 0 || (call.kind == 1 && call.made);
        let mut chosen: Option<(Ball, crate::ball::Forecast, f32)> = None;
        let mut angle = call.angle;
        if !straight {
            let take_first = dunk.is_some() || unit(&mut self.rng) < 0.5;
            let mut best = -1.0;
            for attempt in 0..50 {
                let mut turn_by = unit(&mut self.rng) * FRAC_PI_2 - FRAC_PI_4;
                match call.kind {
                    4 => turn_by += FRAC_PI_2,
                    5 => turn_by -= FRAC_PI_2,
                    2 => {
                        turn_by += PI;
                        if turn_by > PI {
                            turn_by -= TAU;
                        }
                    }
                    _ => {}
                }
                let mut dir = [target[0] - from[0], 0.0, target[2] - from[2]];
                let len = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt();
                dir = if len <= 0.0001 {
                    [0.0, 0.0, 1.0]
                } else {
                    [dir[0] / len, 0.0, dir[2] / len]
                };
                let off = if attempt == 49 && best == -1.0 {
                    let reach = if call.made {
                        0.75 - r
                    } else {
                        r + crate::shooting::NATIVE_RIM_OUTER_RADIUS
                    };
                    dir.map(|d| d * reach)
                } else {
                    let u = unit(&mut self.rng);
                    let reach = if call.kind == 2 {
                        (r + NATIVE_RIM_OUTER_RADIUS) - (r + r + 0.08645) * u
                    } else {
                        NATIVE_RIM_OUTER_RADIUS - (r + 0.08645) * u
                    };
                    let mut off = turn(dir, turn_by).map(|d| d * reach);
                    let out = (off[0] * off[0] + off[2] * off[2]).sqrt() - crate::ball::RIM_ENTRY;
                    let lift =
                        (r + crate::ball::RIM_TUBE) * (r + crate::ball::RIM_TUBE) - out * out;
                    if lift > 0.0 {
                        off[1] = lift.sqrt();
                    }
                    if call.kind == 1 {
                        off[0] *= 1.0 / crate::ball::BOARD_RESTITUTION;
                    }
                    off
                };
                let aim = toward([target[0] + off[0], target[1] + off[1], target[2] + off[2]]);
                if call.kind == 1 {
                    angle = crate::ball::steepen_entry(0.0, from, aim, r, true);
                }
                let (ball, forecast, updates) = trial(&self.ball, aim, angle, call.kind == 1);
                if take_first && forecast.score >= 0.0 && !call.made && !forecast.scored {
                    chosen = Some((ball, forecast, updates));
                    break;
                }
                if forecast.scored == call.made && forecast.score >= 0.0 && best < forecast.score {
                    best = forecast.score;
                    chosen = Some((ball, forecast, updates));
                }
            }
        }
        let (ball, timer) = match chosen {
            Some((ball, forecast, _)) => (ball, forecast.last + 1),
            None => {
                // Straight at the basket (`0x14ba60`), or the fallback's
                // recursive call with kind 0.
                let bank = call.kind == 1 && straight;
                let goal = if bank { target } else { middle };
                let (ball, _, updates) = if call.made {
                    let aim = toward(goal);
                    let steep = if dunk.is_some() {
                        angle
                    } else {
                        crate::ball::steepen_entry(angle, from, aim, r, bank)
                    };
                    let (ball, forecast, updates) = trial(&self.ball, aim, steep, bank);
                    if forecast.scored || !bank {
                        (ball, forecast, updates)
                    } else {
                        let aim = toward(middle);
                        trial(
                            &self.ball,
                            aim,
                            crate::ball::steepen_entry(angle, from, aim, r, false),
                            false,
                        )
                    }
                } else {
                    let reach = r + NATIVE_RIM_OUTER_RADIUS + 1.0;
                    let mut flight = None;
                    for _ in 0..8 {
                        let mut a = unit(&mut self.rng) * PI - PI;
                        if rim[0] < 0.0 {
                            a = -a;
                        }
                        let off = turn([0.0, 0.0, reach], a);
                        let aim = toward([middle[0] + off[0], middle[1], middle[2] + off[2]]);
                        let tried = trial(&self.ball, aim, angle, false);
                        let drops = tried.1.scored;
                        flight = Some(tried);
                        if !drops {
                            break;
                        }
                    }
                    flight.expect("eight tries")
                };
                let timer = straight_timer(updates, ball.velocity[1] * TICK_SECONDS);
                (ball, timer)
            }
        };
        self.ball = ball;
        self.ball.flight_left = (timer as f32 - 0.5) * TICK_SECONDS;
        self.ball.flight_start = self.ball.flight_left;
        // `0x14bed0..0x14befc`: the shooter's side keeps its shot's kind
        // (a long shot's 3), the other side 2.
        let side = self.players[shot.shooter].team.min(1);
        self.ball.touch[side] = Some(shot.shooter as u8);
        self.ball.long = [false; 2];
        self.ball.long[side] = shot.points >= 2;
        // Callback 21 (`FUN_001729f8`, `0x172a44..0x172a64`): the shooter
        // may not touch the ball for `fps / 3` updates.
        self.players[shot.shooter].barred = SHOOTER_BAR;
    }

    /// A dunk or alley-oop dunk leaves the dunker's hands through the
    /// launcher's dunk path (`FUN_0014aaf8` for an actor in a dunk,
    /// `FUN_0016e9d0`, or an alley-oop dunk, `FUN_0016e9e0`; raw
    /// `0x14ac7c..0x14ae40`), `made` as the release callback rolled it,
    /// scoring `basket`. The ball is put in the ring: from the hand toward
    /// the basket's middle ([`NATIVE_AIM_HEIGHT`] up) no farther than
    /// `0.75 − r` (along the line between them, all three axes), and no
    /// lower than `r + 12`. It leaves at the hand's pace, the length of
    /// the ball's last move (`ball+0xa0`) held to [`DUNK_PACE`] (the top
    /// one outright for a rebound jumper's dunk, `FUN_0016e9f0`, or an
    /// actor with `+0x1e8` 0x15): a make straight at the middle (kind 0),
    /// a miss off the rim from the tries (kind 3), [`World::launch_shot`].
    /// The basket comes when the ball goes down through the ring (phase 3
    /// in the launch's update, then the restart's 6): the update after the
    /// launch in 53 of the three attract captures' 54 made dunks, the one
    /// after that for the third's gamebreaker dunk at update 16,562, whose
    /// hand moved 0.30 (the hand is under the top pace in 14 of the
    /// captures' 56 dunk launches). The net keeps half a dunked ball's
    /// speed.
    ///
    /// The held ball is on the dunker's hand (`FUN_0014e060`, [`crate::held`]),
    /// its last move [`Ball::moved`]: the rewrite's slams find it under the
    /// top pace about as often (18% and 20% of them on the first and third
    /// captures' rosters, against 25% of the captures'). The rewrite's own:
    /// without the disc's bones (online) the held ball is carried at the
    /// waist, and the pace is the top one. The putback stands in for the
    /// rebound jumper's dunk.
    pub(crate) fn launch_dunk(&mut self, player: usize, hoop: usize, made: bool, basket: Basket) {
        let rim = self.hoops[hoop].rim;
        let middle = [rim[0], NATIVE_AIM_HEIGHT, rim[2]];
        let from = dunk_place(self.ball.position, middle, self.ball.radius);
        let top = self.poses.get().is_none()
            || match self.players[player].action {
                Action::Dunking { style, row, .. } => {
                    style == DunkStyle::Putback
                        || crate::timing::move_row(row).is_some_and(|r| r.id == 0x15)
                }
                _ => false,
            };
        let pace = dunk_pace(crate::pose::length(self.ball.moved), top);
        let call = ShotCall {
            percent: 0,
            made,
            kind: if made { 0 } else { 3 },
            angle: 0.0,
        };
        self.launch_shot(
            hoop,
            from,
            call,
            Shot {
                shooter: player,
                hoop,
                made,
                points: 1,
                gamebreaker: 0,
                layup: false,
                tried: 0,
                slam: Some(basket),
            },
            Some(pace),
        );
    }
}

fn distance_to(a: [f32; 2], b: [f32; 2]) -> f32 {
    distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rules;

    fn view(d: f32, rating: i32) -> ShotView {
        ShotView {
            distance_to_rim: d,
            shots: rating,
            threes: rating,
            dunks: rating,
            outside: false,
            meter_full: false,
            height_ft: 6.5,
        }
    }

    fn defender(away: f32, rim: f32) -> ContestDefender {
        ContestDefender {
            distance_to_shooter: away,
            distance_to_rim: rim,
            height_ft: 6.5,
            blocks: 50,
        }
    }

    #[test]
    fn the_shots_quality_truncates_like_fun_001805e8() {
        // 50 Shots from 20: (50 + (1 − 15 · 1.3 / 18.75) · 50) / 25 + 4 =
        // 5.92, truncated by `cvt.w.s` to 5, one more with nobody within 18.
        assert_eq!(evaluate_shot_quality_tier(&view(20.0, 50), None, None), 6);
        // From 15: 6.61 → 6, + 1.
        assert_eq!(evaluate_shot_quality_tier(&view(15.0, 50), None, None), 7);
        // 9 is kept (only 10 or more is 8, `0x180880`): 70 from 3 is 9.6 →
        // 9, two off for a defender within 6 (behind the play: no contest).
        let behind = defender(5.0, 40.0);
        assert_eq!(
            evaluate_shot_quality_tier(&view(3.0, 70), Some(behind), None),
            7
        );
        // Beyond the line: Threes / 25 + 4 (+ 3 past 27.85), 7 with a full
        // meter; beyond 30.75 nothing without one, 7 with one to 38.75.
        let mut long = view(25.0, 50);
        long.outside = true;
        assert_eq!(evaluate_shot_quality_tier(&long, None, None), 7);
        long.distance_to_rim = 29.0;
        assert_eq!(evaluate_shot_quality_tier(&long, None, None), 6);
        long.distance_to_rim = 32.0;
        assert_eq!(evaluate_shot_quality_tier(&long, None, None), 1);
        long.meter_full = true;
        // Within 10 (one off), too far behind the play to contest.
        let near = defender(9.0, 60.0);
        assert_eq!(evaluate_shot_quality_tier(&long, Some(near), None), 6);
        long.distance_to_rim = 39.0;
        assert_eq!(evaluate_shot_quality_tier(&long, Some(near), None), 0);
    }

    #[test]
    fn the_contest_reads_record_heights_like_fun_001801e0() {
        // A 6.5 ft defender with 50 Blocks: 6.5 + 1.4375 + 6.2828 over the
        // shooter's 6.5. From 4.6 (dx 1): atan 7.72 → 235 units, 2; from
        // 12 (dx 8.4): 121, 1; from 15 (dx 11.4): 97, none.
        assert_eq!(evaluate_contest_angle(6.5, 10.0, &defender(4.6, 8.0)), 2);
        assert_eq!(evaluate_contest_angle(6.5, 10.0, &defender(12.0, 8.0)), 1);
        assert_eq!(evaluate_contest_angle(6.5, 10.0, &defender(15.0, 8.0)), 0);
        // More than 15 farther from the rim than the shooter: none.
        assert_eq!(evaluate_contest_angle(6.5, 10.0, &defender(4.6, 25.5)), 0);
        assert_eq!(evaluate_contest_angle(6.5, 10.0, &defender(4.6, 25.0)), 2);
        // The thresholds: over 108 units is 1, over 165 is 2. At dx 1 a
        // drop of the defender's height moves the angle across them.
        let at = |dy: f32| {
            let d = ContestDefender {
                distance_to_shooter: 4.6,
                distance_to_rim: 0.0,
                height_ft: 4.0,
                blocks: 0,
            };
            // def 4 + reach 0.5 + 3 = 7.5 over the shooter.
            evaluate_contest_angle(7.5 - dy, 0.0, &d)
        };
        // 0.78 is 108.46 (108), 0.80 110.47; 1.58 is 164.54, 1.64 167.26.
        assert_eq!(at(0.78), 0);
        assert_eq!(at(0.80), 1);
        assert_eq!(at(1.58), 1);
        assert_eq!(at(1.64), 2);
    }

    fn odds(distance: f32, long: bool, rating: i32) -> ShotOdds {
        ShotOdds {
            distance,
            long,
            rating,
            frontcourt: true,
            catch_up: 0,
            human: None,
            difficulty: Difficulty::Medium,
            near: 0,
            contesting: 0,
            gamebreaker: 0,
        }
    }

    #[test]
    fn the_base_percent_follows_fun_00171690() {
        // At the rim: Shots/100 × 15 + 80, truncated.
        assert_eq!(base_percent(4.0, false, 50), (87, 4.0));
        assert_eq!(base_percent(5.0, false, 99), (94, 5.0));
        // Mid-range: Shots/100 × 40 + 50 less min(d, 22)/22 × 15, at
        // least 45; the distance carried on is clamped to 22.
        assert_eq!(base_percent(10.0, false, 50), (70 - 6, 10.0));
        assert_eq!(base_percent(30.0, false, 0), (45, 22.0));
        // Long range: Threes/99 × 25 + 10, from 24.5 less
        // min(d − 22, 12)/12 × 10.
        assert_eq!(base_percent(24.0, true, 99), (35, 24.0));
        assert_eq!(base_percent(31.0, true, 50), (22 - 7, 31.0));
        assert_eq!(base_percent(40.0, true, 0), (0, 40.0));
    }

    #[test]
    fn the_shot_callback_adds_the_open_bonus_and_takes_off_for_contests() {
        // Open (no opponent within 7): + Shots/99 × 20, or Threes/99 × 31.
        assert_eq!(shot_percent(&odds(10.0, false, 50)), 64 + 10);
        assert_eq!(shot_percent(&odds(25.0, true, 99)), 35 - 2 + 31);
        // Someone within 7: no bonus, and 5 off for each contesting.
        let mut o = odds(10.0, false, 50);
        o.near = 2;
        assert_eq!(shot_percent(&o), 64);
        o.contesting = 2;
        assert_eq!(shot_percent(&o), 54);
        // The catch-up bonus adds in; 98 is the cap, 3 the floor; the
        // wrong half of the court is 3.
        o = odds(4.0, false, 99);
        o.catch_up = 20;
        assert_eq!(shot_percent(&o), 98);
        o = odds(40.0, true, 0);
        o.near = 1;
        o.contesting = 3;
        assert_eq!(shot_percent(&o), 3);
        o = odds(10.0, false, 99);
        o.frontcourt = false;
        assert_eq!(shot_percent(&o), 3);
        // A gamebreaker: rating × 3/2 + 55, uncapped; Level 2 always.
        o = odds(10.0, false, 20);
        o.gamebreaker = 1;
        assert_eq!(shot_percent(&o), 85);
        o.gamebreaker = 2;
        assert_eq!(shot_percent(&o), 100);
        // Easy against a person: the person's team +40 and double catch-up
        // within 33.75, the computer's −10 and none; Hard drops the
        // person's catch-up.
        o = odds(10.0, false, 0);
        o.catch_up = 5;
        o.near = 1;
        o.difficulty = Difficulty::Easy;
        o.human = Some(true);
        assert_eq!(shot_percent(&o), 45 + 40 + 10);
        o.human = Some(false);
        assert_eq!(shot_percent(&o), 45 - 10);
        o.difficulty = Difficulty::Hard;
        o.human = Some(true);
        assert_eq!(shot_percent(&o), 45);
        o.human = None;
        assert_eq!(shot_percent(&o), 50);
    }

    /// A match at the native hoops: team 0's shooter 26 out from the +x
    /// basket it attacks, alone (nobody within 7), team 1 far off.
    fn catch_up_world(rules: Rules) -> World {
        World::new(
            crate::Hoop::native(),
            vec![
                crate::Player::new(0, [10.75, 0.0], std::f32::consts::FRAC_PI_2),
                crate::Player::new(1, [-30.0, 10.0], 0.0),
            ],
            rules,
            1,
        )
    }

    #[test]
    fn the_catch_up_stage_is_the_larger_scaled_score_not_their_or() {
        // `FUN_0012eac0` forms the stage with `pmaxw` (0x12eb6c): the
        // larger of the two scaled scores against the game's bounds. The
        // decompile's `uVar2 | uVar3` is the Ghidra copy's stand-in for
        // the MMI instruction. 16 and 1 OR to 17, 2 and 1 to 3: against
        // bounds 3 and 17 the OR would read stages 2 and 1 where the
        // larger reads 1 and 0.
        let world = |score: [u32; 2]| {
            let mut w = catch_up_world(Rules::default());
            w.stage_bounds = [3, 17];
            w.score = score;
            w
        };
        let w = world([1, 16]);
        assert_eq!(catch_up_cell(&w, 0), Some((-15, 1)));
        assert_eq!(catch_up_bonus(&w, 0), 15);
        assert_eq!(catch_up_cell(&w, 1), Some((15, 1)));
        assert_eq!(catch_up_bonus(&w, 1), 0);
        let w = world([2, 1]);
        assert_eq!(catch_up_cell(&w, 1), Some((-1, 0)));
        assert_eq!(catch_up_bonus(&w, 1), 2);
        // Through the release callback: the shooter 15 behind at 1 to 16
        // gets 15 more than level (the OR's stage 2 would give 20).
        let percent = |mut w: World| w.shot_call(0, 0, [10.75, 7.0, 0.0], false, 0, true).percent;
        assert_eq!(percent(world([1, 16])) - percent(world([0, 0])), 15);
        // The scores are scaled to 21 first (`div`): to 11, 3 and 5 are 5
        // and 9 (OR 13), the stage 1 against 3 and 13.
        let mut w = catch_up_world(Rules {
            goal: Goal::Points(11),
            ..Rules::default()
        });
        w.stage_bounds = [3, 13];
        w.score = [3, 5];
        assert_eq!(catch_up_cell(&w, 0), Some((-4, 1)));
    }

    #[test]
    fn the_points_game_keeps_its_stage_bounds_for_the_match() {
        // The original draws them once a game (`FUN_0012e750` ->
        // `FUN_0012e798`: `rand() % 6`, then `15 + rand() % 3`) from a
        // generator the boot and the front end have run on (the captured
        // match drew 4 and 15). Drawn first from the sim's generator, as
        // `Rng::new` seeds it (the seed added to six words), they would be
        // 5 and 16 for every seed of a match: the first draw grows by 6 a
        // seed and the second by 21. So the sim starts at its own pair.
        for seed in [0, 1, 2, 7, 2002, 123_456] {
            let mut rng = crate::rng::Rng::new(seed);
            let early = rng.next_u32() % 6;
            assert_eq!([early, 15 + rng.next_u32() % 3], [5, 16], "seed {seed}");
        }
        let mut w = catch_up_world(Rules::default());
        assert_eq!(w.stage_bounds, crate::STAGE_BOUNDS);
        // They hold the stage for the match: 3 all is stage 1 against the
        // sim's 2 and 16, stage 0 against the captured 4 and 15.
        w.score = [3, 3];
        assert_eq!(catch_up_cell(&w, 0), Some((0, 1)));
        w.stage_bounds = [4, 15];
        assert_eq!(catch_up_cell(&w, 0), Some((0, 0)));
        w.score = [3, 15];
        assert_eq!(catch_up_cell(&w, 0), Some((-12, 2)));
        // They are the world's state: a snapshot carries them.
        let back = World::from_snapshot(&w.to_snapshot()).unwrap();
        assert_eq!(back.stage_bounds, [4, 15]);
    }

    #[test]
    fn a_points_game_built_on_the_sessions_generator_draws_its_bounds_after_the_construction() {
        // The fresh boot's Pick Up game (`street-played-fresh-9`): Choose
        // Team's commit reseeded the shared generator from the timer at
        // 13,943 (139.43 s after the boot, `FUN_001dbe30`); 46 draws later
        // the constructor drew 2 and 17, and update 0 holds these words.
        let built = |rules: Rules, rng: crate::rng::Rng| {
            let players = catch_up_world(rules).players;
            World::from_generator(crate::Hoop::native(), players, rules, rng)
        };
        let w = built(Rules::default(), crate::rng::Rng::new(13_943));
        assert_eq!(w.stage_bounds, [2, 17]);
        assert_eq!(
            w.generator().state(),
            [
                2198666930, 3733697278, 533370113, 2608621288, 2551894892, 1876914699
            ]
        );
        // A trick-point game (`FUN_0018c4f0`) makes the construction's
        // draws but not the pair.
        let trick = built(
            Rules::mode(crate::Mode::TrickPointChallenge),
            crate::rng::Rng::new(13_943),
        );
        assert_eq!(trick.stage_bounds, crate::STAGE_BOUNDS);
        let mut after = crate::rng::Rng::new(13_943);
        for _ in 0..crate::CONSTRUCTION_DRAWS {
            after.next_u32();
        }
        assert_eq!(trick.generator(), &after);
        // A world from a bare seed keeps the rewrite's pair and its
        // generator untouched.
        let bare = catch_up_world(Rules::default());
        assert_eq!(bare.stage_bounds, crate::STAGE_BOUNDS);
        assert_eq!(bare.generator(), &crate::rng::Rng::new(1));
        // Reseeded from a count of hundredths, the pair follows the count.
        // Over five periods of the parity below (28.6 minutes of counts)
        // every pair comes up near an eighteenth of the time, but the
        // generator's low bits mix slowly:
        // the lower bound's parity holds for 17,187 counts (171.87 s) at a
        // time, so the first game after a reseed in 1,137..18,323 (the
        // fresh boot's 13,943) always has an even lower bound, and one in
        // 18,324..35,510 an odd one.
        let pair = |count: u32| {
            let mut rng = crate::rng::Rng::new(count);
            for _ in 0..crate::CONSTRUCTION_DRAWS {
                rng.next_u32();
            }
            let early = rng.next_u32() % 6;
            [early, 15 + rng.next_u32() % 3]
        };
        for count in [0, 1_137, 13_943, 18_324, 123_456] {
            let w = built(Rules::default(), crate::rng::Rng::new(count));
            assert_eq!(w.stage_bounds, pair(count), "count {count}");
        }
        let mut seen = [[0u32; 3]; 6];
        for count in 1_137..1_137 + 5 * 2 * 17_187 {
            let [early, late] = pair(count);
            seen[early as usize][late as usize - 15] += 1;
        }
        for row in seen {
            for n in row {
                assert!((8_900..10_200).contains(&n), "{seen:?}");
            }
        }
        assert!((1_137..18_324).all(|count| pair(count)[0] % 2 == 0));
        assert!((18_324..35_511).all(|count| pair(count)[0] % 2 == 1));
        assert_eq!(pair(1_136)[0] % 2, 1);
    }

    #[test]
    fn gamebreaker_wins_counts_a_level_1_gamebreaker_as_100() {
        // `FUN_00145b48`: each side's meter in 500s, 100 more once it has
        // scored a Level 1 gamebreaker (side stat 0xc); the lead clamped
        // to the span (100 at Level 1) × 31 / 200, the stage by the larger
        // against 33 and 66 (`FUN_001457f8`).
        let mut w = catch_up_world(Rules {
            goal: Goal::Gamebreaker(1),
            ..Rules::default()
        });
        w.teams[0].meter = 10_000;
        w.teams[1].meter = 5_000;
        assert_eq!(catch_up_cell(&w, 0), Some((1, 0)));
        w.stats.add_team(1, crate::stats::LEVEL1, 1);
        assert_eq!(catch_up_cell(&w, 0), Some((-13, 2)));
        assert_eq!(catch_up_cell(&w, 1), Some((13, 2)));
    }

    #[test]
    fn the_catch_up_table_helps_only_the_side_behind() {
        assert_eq!(catch_up_row(-15, 2), 20);
        assert_eq!(catch_up_row(-30, 0), 10);
        assert_eq!(catch_up_row(-5, 2), 12);
        assert_eq!(catch_up_row(-1, 0), 2);
        assert_eq!(catch_up_row(0, 2), 0);
        assert_eq!(catch_up_row(7, 1), 0);
    }

    #[test]
    fn a_dunk_leaves_at_the_hands_pace_on_the_discs_bones() {
        // The launcher's dunk path (raw `0x14ac7c..0x14acf8`) sends the
        // ball at the length of its last move (`ball+0xa0`) held to
        // `DUNK_PACE`, the top outright for a rebound jumper's dunk (the
        // putback stands in) or dunk id 0x15 (`dunk_3`). With the disc's
        // animations the held ball is on the dunker's hand; without them
        // it is carried at the waist and the pace is the top one.
        use crate::timing::{MOVE_ROWS, NO_MOVE_ROW};
        let bones = crate::pose::disc_bank();
        if bones.get().is_none() {
            return;
        }
        let row = |dunk_3: bool| {
            MOVE_ROWS
                .iter()
                .position(|r| r.family == 0 && (r.id == 0x15) == dunk_3)
                .unwrap() as u8
        };
        let launch = |poses: crate::pose::Bank, row: u8, style: DunkStyle, moved: f32| {
            let mut w = crate::World::new(
                crate::tests::hoops(),
                vec![crate::Player::new(0, [30.0, 0.0], 0.0)],
                Rules::default(),
                1,
            );
            w.poses = poses;
            w.ball.hold(0);
            w.ball.position = [35.5, 12.6, 0.4];
            w.ball.moved = [moved * 0.6, -moved * 0.8, 0.0];
            w.players[0].start_dunk(0, [34.0, 0.0], style, (row, 40, 60));
            w.launch_dunk(0, 0, true, Basket::Dunk { advanced: false });
            w
        };
        let flight = |w: &crate::World, pace: f32| {
            let rim = w.hoops[0].rim;
            let middle = [rim[0], NATIVE_AIM_HEIGHT, rim[2]];
            let placed = dunk_place([35.5, 12.6, 0.4], middle, w.ball.radius);
            let mut ball = Ball::loose_at(placed);
            ball.launch_at_pace(placed, middle, pace);
            ball.velocity
        };
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
        let normal = row(false);
        for (moved, row, style, pace) in [
            (0.3, normal, DunkStyle::Normal, 0.3),
            (0.1, normal, DunkStyle::Normal, DUNK_PACE[0]),
            (0.9, normal, DunkStyle::Normal, DUNK_PACE[1]),
            (0.3, row(true), DunkStyle::Normal, DUNK_PACE[1]),
            (0.3, NO_MOVE_ROW, DunkStyle::Putback, DUNK_PACE[1]),
        ] {
            let w = launch(bones.clone(), row, style, moved);
            assert!(
                close(w.ball.velocity, flight(&w, pace)),
                "{moved} {row} {style:?}: {:?}",
                w.ball.velocity
            );
        }
        let w = launch(crate::pose::Bank::StandIns, normal, DunkStyle::Normal, 0.3);
        assert!(close(w.ball.velocity, flight(&w, DUNK_PACE[1])));
    }
}
