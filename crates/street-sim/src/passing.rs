//! Passing: ordinary passes and the street ones, as the game's Street School
//! describes them (`XLOC_GM`, `school.txt`):
//!
//! - Pass: "Press Pass to pass the ball to a teammate", toward the stick.
//! - Alley-oop: "pass the ball to a teammate who is jumping high in the air
//!   towards the basket." The computer's teammates leap on their own, or
//!   when the strategy button calls the play; a player without the ball
//!   leaps with shoot and a turbo ([`crate::leap`]). The lob to a leaper is
//!   timed and aimed at his hand (`FUN_00162aa8`, [`World::lob_to_leaper`]).
//! - Give 'n go: pass with the right stick toward a teammate, "then tap Pass
//!   to call for the pass" back. Any player without the ball calls for it
//!   with Pass.
//! - Kick pass: hold the left stick down (L3) and press Pass.
//! - Off the Heezay: "bounce the ball off a defender's head, and back to
//!   yourself. First, get close to your defender. Then, without moving, hold
//!   any two Turbos and press Pass."
//! - Back 2 Papa: "pass the ball off the backboard and back to yourself.
//!   Position yourself in front of the backboard and behind the free throw
//!   line. While moving towards the basket, hold any two Turbos and press
//!   Pass." The ball comes back for an alley-oop of your own.

use crate::{
    Action, BallState, Event, Input, Pass, World, ball::Via, distance, math, normalize2, sub,
    target_hoop, tuning,
};

/// How a pass was thrown, for trick points and animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassStyle {
    Normal,
    /// Thrown with the right stick: control stays with the passer.
    GiveAndGo,
    Kick,
    /// Off the defender's head.
    Heezay,
    /// Off the backboard.
    Glass,
    /// A lob to a teammate leaping for an alley-oop.
    Oop,
}

/// How the rewrite releases a pass: its speed, its release height and the
/// kind it leaves on the passer (+0x268, which the drain handler reads).
///
/// These are the rewrite's values, not rows of the original's pass table
/// (`0x3d65e8`, 93 records of 28 bytes, read word for word by
/// `street_assets::passes`): the table's +0x0c/+0x10 floats have no traced
/// consumer to name them speed or height. Plain, give-and-go, kick and
/// board passes fly the original's flight ([`flight_updates`] and
/// [`World::board_flight`]); their legacy speed field is not used. Of
/// the kinds, only 2 (row 0, kind of rows 0..4) and 5 (rows 6..9) are seen
/// in the table for plain and kick passes; the others are unverified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassRelease {
    pub speed: f32,
    pub release_height: f32,
    pub kind: u32,
}

impl PassStyle {
    /// The rewrite's own pop-up values for the pass styles (the HUD's).
    /// The original pays a pass nothing at its press: a pass enters the
    /// combo ring as class 8, 9 or 14 at its catch, and only a passout
    /// special fills the meter ([`World::post_pass_caught`]).
    pub fn trick_points(self) -> u32 {
        match self {
            PassStyle::Normal => 0,
            PassStyle::GiveAndGo => 500,
            PassStyle::Kick => 1_500,
            PassStyle::Heezay | PassStyle::Glass => 3_000,
            PassStyle::Oop => 1_000,
        }
    }

    /// The rewrite's release for this style (see [`PassRelease`]).
    pub fn release(self) -> PassRelease {
        match self {
            PassStyle::Normal | PassStyle::GiveAndGo => PassRelease {
                speed: 50.0,
                release_height: 4.2,
                kind: 2,
            },
            PassStyle::Kick => PassRelease {
                speed: 50.0,
                release_height: 1.6,
                kind: 5,
            },
            PassStyle::Oop => PassRelease {
                speed: 35.0,
                release_height: 4.5,
                kind: 0,
            },
            PassStyle::Heezay | PassStyle::Glass => PassRelease {
                speed: 45.0,
                release_height: 4.0,
                kind: 4,
            },
        }
    }

    /// The kind a pass of this style leaves on the passer (see
    /// [`PassRelease`] for which values the original's table supports).
    pub fn native_kind(self) -> u32 {
        self.release().kind
    }
}

/// A pass's speed, units per update (`DAT_004b99c0`): set once at boot
/// (`0x178bf4..0x178c34`) as `5280 / (fps × 3600) × 30.762`, 30.762 mph
/// had a unit been a foot, 0.75196 at 60 fps. Traced 2026-10-16
/// (docs/TRACE.md "Pass flights and the tip-off").
pub const PASS_SPEED: f32 = 5280.0 / (60.0 * 3600.0) * f32::from_bits(0x41f6_1893);

/// The shortest flight, updates (`DAT_004b99c4`: `fps / 60 × 20`, set
/// beside the speed at `0x178c38..0x178c74`).
pub const PASS_MIN_FLIGHT: f32 = 20.0;

/// The flight of the first pass after the actors' reset, updates: the
/// reset (`FUN_00156870`) sets `DAT_003d65b8`, and the release's catch
/// draw (`FUN_001611d8`, `0x161828..0x16183c`) flies the pass 40 while it
/// is set, then clears it (`0x1622c0`). The tip is that pass: 40 in both
/// captured openings.
pub const FIRST_PASS_FLIGHT: f32 = 40.0;

/// `FUN_001611d8`'s flight over `distance` (horizontal, `0x161408..
/// 0x161428`): the distance over [`PASS_SPEED`], at least
/// [`PASS_MIN_FLIGHT`], truncated (`cvt.w.s`, `0x161840`) into the
/// receiver's +0x1b0 and `DAT_003d65a0`, the updates the ball flies
/// (`FUN_0014c0c0` stores it at ball+0x64).
pub fn flight_updates(distance: f32) -> i32 {
    let f = distance / PASS_SPEED;
    (if PASS_MIN_FLIGHT < f {
        f
    } else {
        PASS_MIN_FLIGHT
    }) as i32
}

/// `div.s` as the EE does it: a division by zero gives the largest float
/// with the operands' signs combined, where IEEE gives an infinity or NaN.
fn ee_divide(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        if a.is_sign_negative() != b.is_sign_negative() {
            -f32::MAX
        } else {
            f32::MAX
        }
    } else {
        a / b
    }
}

/// Where a pass goes, as the pass start (`FUN_0015f930`, `0x15fcc4..
/// 0x15fe48`) settles it at the press: the receiver's spot
/// (`DAT_003d70c0`/`c4`) and whether the pass leads him
/// (`DAT_003d75c4`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassAim {
    /// Where the receiver is going (`FUN_0016c970`: his front behaviour's
    /// destination, kept within ±40 by ±24.5), or where he stands.
    pub spot: [f32; 2],
    /// The flight is timed on the passer-to-receiver distance and the ball
    /// goes where the receiver will be after it (`FUN_001636c0`); else it
    /// is timed on the distance to the spot and goes there.
    pub lead: bool,
    /// Updates the receiver takes to the spot at his speed then
    /// (`DAT_003d9a28`; the largest float for a receiver standing still,
    /// the EE's quotient for a division by zero).
    pub reach: f32,
}

/// A pass pressed and not yet out of the passer's hands. The original's
/// pass start (`FUN_0015f930`) picks a row of the pass table and calls the
/// pass setup (`FUN_0014bff8`, `0x160e34`: ball state 4, the receiver set)
/// at the press, then plays the row's script; the ball stays in the
/// passer's hands (ball+0x100) until the script's callback 38 lets it go.
/// Two attract captures of the original (`tools/pine_ai_capture.py`) show
/// it: state 4 from the press, the ball in the hands for the script's
/// blend and wait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Windup {
    pub from: usize,
    pub to: usize,
    pub style: PassStyle,
    /// Updates left until the release.
    pub left: u8,
    /// The pass table row drawn at the press ([`crate::timing::PASS_ROWS`]).
    pub row: u8,
    /// The press's aim ([`World::pass_aim`]).
    pub aim: PassAim,
}

/// What a pass press comes to (`FUN_00146278`, the pass target the pad
/// builder `FUN_001587d8` writes to actor+0x74; the holder's update
/// `FUN_0015a060` passes to it, `0x15b864..0x15ba14`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PassChoice {
    /// A pass to `to`: `style` as the press threw it (the right stick's
    /// give 'n go, a kick pass with L3, else plain).
    Pass { to: usize, style: PassStyle },
    /// A passout special off `defender`'s head (`FUN_00142068`'s kind 2),
    /// coming down to `to`: the passer, or a teammate leaping for an
    /// alley-oop.
    Heezay { defender: usize, to: usize },
    /// A passout special off the backboard (the kind the attacking side's
    /// sign), coming down to `to` (the passer, a teammate across the lane,
    /// or a leaper) from `point` on the board.
    Glass { to: usize, point: [f32; 3] },
    /// No pass: a trick pass short of turbo or out of position (the "no
    /// turbo" message `0x3c6520`, or `FUN_00171e30`'s failed passout).
    Refused,
}

/// The passout specials' cvars (`FUN_00147cf8`, `0x147f28..0x147f90`):
/// `debug_passoutOneTurbo` (0x3d4658+0x18) is 0, so the trick pass asks
/// for the held word's two-turbo bit (0x40000, `FUN_001308e8`), not any
/// turbo (0x2000); `debug_passoutOnGround` (0x3d4678+0x18) is 1, so the
/// passer must stand on the floor.
pub const PASSOUT_ONE_TURBO: bool = false;
pub const PASSOUT_ON_GROUND: bool = true;

/// `tan(20°)` as `FUN_00146278` takes it (`0x146cbc..0x146dd4`: the EE's
/// own sine and cosine polynomials of `0x3eb2b8c3`): the board's wedge,
/// `(|z| - 3) tan 20° <= 38 - |x|`.
fn tan20() -> f32 {
    let x = f32::from_bits(0x3eb2_b8c3);
    let x2 = x * x;
    let s = ((((x2 * f32::from_bits(0x3638_ef1d) + f32::from_bits(0xb950_0d01)) * x2
        + f32::from_bits(0x3c08_8889))
        * x2
        + f32::from_bits(0xbe2a_aaab))
        * x2
        + 1.0)
        * x;
    let c = libm::sqrtf(1.0 - s * s);
    s / c
}

/// Inside the board's wedge (`0x146c90..0x146df4`): `(|z| - 3) tan 20°`
/// no more than `38 - |x|`, or within 3 of the middle.
fn in_wedge([x, z]: [f32; 2]) -> bool {
    let off = z.abs() - 3.0;
    off < 0.0 || off * tan20() <= 38.0 - x.abs()
}

/// The angle word that moves along `stick` (court axes): an actor's word
/// `a` moves him along `(-sin a, -cos a)`.
pub(crate) fn stick_word(stick: [f32; 2]) -> i32 {
    crate::timing::angle_units(math::atan2(-stick[0], -stick[1]))
}

/// The right stick's press (`FUN_001479f8`): pad[6], `trunc((|stick| +
/// 0.5) × 64 / 127)` of its raw bytes, at 60 or more sets the held word's
/// 0x400 (`0x147994`), and the pass press takes 0x400 pressed as its own
/// (`FUN_0015a060`'s `0x500`).
pub(crate) fn aim_pressed(now: &Input, before: &Input) -> bool {
    let pushed = |i: &Input| {
        let [x, z] = i.aim;
        let n = math::len(f32::from(x), f32::from(z));
        ((n + 0.5) * 64.0 / 127.0) as i32 >= 0x3c
    };
    pushed(now) && !pushed(before)
}

/// Updates from an inbound pass's press to its release. In phase 6 the
/// pass start's row filter admits only rows with flag bit 16 (raw
/// `0x160aa4..0x160ac4`), and row 10 is the only one: its script
/// (`0x4548b8`) blends 6 updates (`8e f8 06`) and waits 12 (`8d ee 0a
/// 0c`) before callback 38. The captures agree: 18 updates in all 56
/// inbounds. Live passes take their own row's callback 38
/// ([`crate::timing::pass_row`]: 4..46 over the 93 rows).
pub const INBOUND_RELEASE: u8 = crate::timing::PASS_ROWS[crate::timing::INBOUND_ROW].release;

impl World {
    /// Start a pass: the ball stays in the passer's hands ([`Windup`])
    /// until the pass animation lets it go. The press is the pass as far
    /// as the game is concerned: its event and trick points come now (the
    /// pass setup posts its sound at the press). A lob to a leaper plays
    /// the alley-oop rows (flag 0x8000: `allyoop_1`'s callback 38 at 12).
    pub(crate) fn wind_up(&mut self, i: usize, to: usize, style: PassStyle, input: &Input) {
        // The row filter and draw (`0x160870..0x160ebc`): the row's script
        // lets go at its callback 38. A handoff to a teammate within 5
        // calls the release at once (`0x160ea0`).
        let aim = self.pass_aim(i, to);
        self.spot_at_press(i, to, &aim);
        let pick = crate::timing::pass_row(self, i, to, &aim, input);
        // A lob to a leaper is an Oop from its press (its row's kind).
        let style = if self.players[to].leaping() {
            PassStyle::Oop
        } else {
            style
        };
        let Some(row) = pick.row.filter(|_| pick.release > 0) else {
            let positions = self.floor_positions();
            self.throw_aimed(i, to, style, aim, &positions);
            return;
        };
        self.players[i].pass_kind = style.release().kind;
        self.begin_script(i, crate::timing::PASS_ROWS[usize::from(row)].script);
        // The passer's face (`0x1610ec..0x161110`: `FUN_00159190(passer,
        // r & 1 ? 1 : 2, fps × 2)`).
        self.rng.next_u32();
        self.face(i, 2 * crate::TICK_HZ as i32);
        self.players[i].action = Action::Passing {
            elapsed: 0.0,
            row,
            tick: 0,
        };
        // The row's script moves him from his velocity at the press
        // ([`crate::jump`]): none once the stand path has run under a
        // stick let go (as the filter reads it).
        let r = &crate::timing::PASS_ROWS[usize::from(row)];
        if r.flags & 0x400 == 0 && crate::jump::scripted(&self.players[i].action) {
            let still = crate::timing::stick_strength(self, i, input) < crate::input::MOVES;
            let p = &mut self.players[i];
            let [vx, vz] = if still { [0.0; 2] } else { p.velocity() };
            let dt = crate::TICK_SECONDS;
            p.flight = crate::jump::Flight::new([vx * dt, 0.0, vz * dt]);
            p.flight.script = r.script;
        }
        self.windup = Some(Windup {
            from: i,
            to,
            style,
            left: pick.release,
            row,
            aim,
        });
        self.after_throw(i, to, style);
    }

    /// The players' floor positions, as the game's distance table
    /// (`FUN_0013fbc8`, game+0x1ac) holds them through an update: built
    /// before the actors move.
    pub(crate) fn floor_positions(&self) -> Vec<[f32; 2]> {
        self.players.iter().map(|p| p.position).collect()
    }

    /// The pass start's aim (`FUN_0015f930`, raw `0x15fcc4..0x15fe48`).
    ///
    /// The spot is where the receiver is going (`FUN_0016c970`): the
    /// destination of his front behaviour (`FUN_0013b900` →
    /// `FUN_00133380`, vt+0x24, which for most behaviours is their target
    /// `FUN_00100768` → vt+0x1c), kept within ±40 by ±24.5, or where he
    /// stands; a leaper for an alley-oop (actor+0xa4 `0x8000`) and a pass
    /// to oneself take his place. The pass leads him unless he gets to
    /// the spot (its distance over his speed, `DAT_003d9a28`) within
    /// `fps / 2 - trunc(fps × 0.12)` = 23 updates and the passer is
    /// farther from it than he is (`FUN_0018cdb0`). A receiver standing
    /// still is led: the original's division by his speed (`0x15fd60`)
    /// gives the EE's largest float, which fails the test.
    ///
    /// Not carried (the rewrite's own): the airborne receiver's landing
    /// spot (his place stands in), "fast break" and "post cut"'s own
    /// destinations (`FUN_0010aea8`, `FUN_00112f48`; their targets stand
    /// in), and the passer-record list `0x43f870` and actor+0xa4 bit 11
    /// gates (read as clear).
    pub(crate) fn pass_aim(&self, i: usize, to: usize) -> PassAim {
        let p = &self.players[i];
        let r = &self.players[to];
        let leaper = r.leaping();
        let mut spot = r.position;
        if !leaper
            && to != i
            && r.height <= 0.0
            && self.passout.is_none_or(|po| po.passer != i || po.tossed)
        {
            let rim = self.rim_spot(target_hoop(self.possession.min(1)));
            if let Some(target) = self
                .ai
                .brains
                .get(to)
                .and_then(|b| b.front_pass_target(r.position, rim))
            {
                spot = target;
            }
            spot = [spot[0].clamp(-40.0, 40.0), spot[1].clamp(-24.5, 24.5)];
            // `0x16cc5c..0x16ccb0`: within 1.5 he stays where he is.
            if distance(r.position, spot) < 1.5 {
                spot = r.position;
            }
        }
        let fps = crate::TICK_HZ as f32;
        let reach = ee_divide(distance(spot, r.position), r.speed / fps);
        let margin = (fps * f32::from_bits(0x3df5_c28f)) as i32 as f32;
        let window = (fps * 0.5) as i32 as f32;
        let lead = if reach + margin <= window {
            distance(p.position, spot) <= distance(r.position, spot)
        } else {
            true
        };
        PassAim { spot, lead, reach }
    }

    /// The release's flight (`FUN_001611d8`, raw `0x16138c..0x161848`) in
    /// updates, and whether the pass leads its receiver after all.
    ///
    /// Led, it is timed on the game's distance table (`FUN_0016e948`:
    /// the passer and the receiver as they stood when the update began);
    /// else on the passer's distance to the spot; [`flight_updates`] of
    /// that, as a float until it is stored. A spot pass to a moving
    /// receiver is looked at again: with the passer in his front court
    /// and the spot within 12 of the rim his side attacks, it leads him if
    /// the ball would beat him there; else, once the ball would be there
    /// no later than 7 updates after him, it leads him unless both stand
    /// in the passer's front court and he is coming back to a spot nearer
    /// the centre line than he is and within 13 of it, when the flight
    /// grows by `trunc(fps × 0.09)` = 5 if he still gets there within it
    /// (else it leads him). A spot more than 50 units (of 1024) off his
    /// facing (`FUN_0018d290`) leads him too. The first pass after the
    /// actors' reset ([`crate::Tip::first_pass`]) flies
    /// [`FIRST_PASS_FLIGHT`].
    pub(crate) fn release_flight(
        &self,
        i: usize,
        to: usize,
        aim: &mut PassAim,
        start: &[[f32; 2]],
    ) -> f32 {
        let p = &self.players[i];
        let r = &self.players[to];
        let fps = crate::TICK_HZ as f32;
        let at = |k: usize| start.get(k).copied().unwrap_or(self.players[k].position);
        let d = if aim.lead {
            distance(at(i), at(to))
        } else {
            distance(at(i), aim.spot)
        };
        let mut f = self.board_flight(i, aim.spot).map_or_else(
            || (d / PASS_SPEED).max(PASS_MIN_FLIGHT),
            |(_, flight)| flight,
        );
        let per_update = r.speed / fps;
        if !aim.lead && per_update != 0.0 {
            let side = crate::restart::side(p.team.min(1));
            let front = |x: f32| if x >= 0.0 { side > 0.0 } else { side < 0.0 };
            let margin = (fps * f32::from_bits(0x3df5_c28f)) as i32 as f32;
            let mut again = true;
            if front(p.position[0]) {
                let rim = crate::restart::side(r.team.min(1)) * 36.75;
                let dx = aim.spot[0] - rim;
                if dx * dx + aim.spot[1] * aim.spot[1] < 144.0 {
                    if f <= aim.reach {
                        aim.lead = true;
                    }
                    again = false;
                }
            }
            if again {
                let reach = ee_divide(distance(r.position, aim.spot), per_update);
                if f <= reach + margin {
                    let same_half = (r.position[0] >= 0.0) == (p.position[0] >= 0.0);
                    let toward_centre = aim.spot[0].abs() < r.position[0].abs();
                    if same_half
                        && front(p.position[0])
                        && toward_centre
                        && aim.spot[0].abs() < 13.0
                    {
                        let grow = (fps * f32::from_bits(0x3db8_51ec)) as i32 as f32;
                        if reach + margin <= f + grow {
                            f += grow;
                        } else {
                            aim.lead = true;
                        }
                    } else {
                        aim.lead = true;
                    }
                }
            }
        }
        if !aim.lead && crate::timing::facing_angle(r.wanted, r.position, aim.spot).1 >= 0x33 {
            aim.lead = true;
        }
        if self.tip.first_pass {
            f = FIRST_PASS_FLIGHT;
        }
        f
    }

    /// `FUN_001636c0`: where receiver `to` will be after `updates`, on at
    /// his present velocity; a run that would leave ±40.5 or ±25 turns
    /// along that edge at his speed for the rest of the time.
    pub(crate) fn lead_spot(&self, to: usize, updates: i32) -> [f32; 2] {
        let r = &self.players[to];
        let fps = crate::TICK_HZ as f32;
        let v = r.velocity().map(|c| c / fps);
        let speed = math::len(v[0], v[1]);
        let n = updates as f32;
        let [x, z] = r.position;
        let x1 = x + n * v[0];
        if x1.abs() >= 40.5 {
            let edge = if 0.0 < x1 { 40.5 } else { -40.5 };
            let mut t = if 0.001 <= v[0].abs() {
                (edge - x) / v[0]
            } else {
                1.0
            };
            if t < 1.0 {
                t = 1.0;
            }
            let along = if v[1] == 0.0 {
                0.0
            } else if v[1] < 0.0 {
                -speed
            } else {
                speed
            };
            let z1 = (z + t * v[1] + (n - t) * along).clamp(-25.0, 25.0);
            return [edge, z1];
        }
        let z1 = z + n * v[1];
        if z1.abs() >= 25.0 {
            let edge = if 0.0 < z1 { 25.0 } else { -25.0 };
            let mut t = if 0.001 <= v[1].abs() {
                (edge - z) / v[1]
            } else {
                1.0
            };
            if t < 1.0 {
                t = 1.0;
            }
            let along = if v[0] == 0.0 {
                0.0
            } else if v[0] < 0.0 {
                -speed
            } else {
                speed
            };
            let x2 = (x + t * v[0] + (n - t) * along).clamp(-40.5, 40.5);
            return [x2, edge];
        }
        [x1, z1]
    }

    /// One update of the passer's animation: the release when it is due.
    /// A windup whose passer lost the ball is dropped. `start` holds the
    /// players' places as the update began ([`World::floor_positions`]).
    pub(crate) fn step_windup(&mut self, start: &[[f32; 2]]) {
        let Some(mut w) = self.windup else {
            return;
        };
        if self.ball.state != BallState::Held(w.from) {
            self.windup = None;
            return;
        }
        // The passer is in his pass animation (bit 4) for the whole
        // windup, however long the row's script holds the ball.
        let toss = crate::passout::toss(w.row).filter(|_| w.from == w.to);
        let release = toss.map_or(crate::timing::PASS_ROWS[usize::from(w.row)].release, |t| {
            t.tick
        });
        if self.players[w.from].action == Action::None {
            self.players[w.from].action = Action::Passing {
                elapsed: 0.0,
                row: w.row,
                tick: u16::from(release.saturating_sub(w.left)),
            };
        }
        w.left = w.left.saturating_sub(1);
        if w.left > 0 {
            self.windup = Some(w);
            return;
        }
        let tick = match self.players[w.from].action {
            Action::Passing { tick, .. } => tick,
            _ => u16::from(release),
        };
        self.windup = None;
        if toss.is_some() && self.passout.is_some_and(|po| po.passer == w.from) {
            // A passout to himself tosses at its callback 53
            // ([`crate::passout`]).
            self.passout_toss(w);
        } else {
            self.launch(w.from, w.to, w.style, w.aim, start, None);
        }
        // The follow-through plays from the release: the row's script goes
        // on past its callback 38.
        self.players[w.from].action = Action::Passing {
            elapsed: 0.0,
            row: w.row,
            tick,
        };
    }

    /// The pass press with the ball: what [`World::pass_press`] chooses,
    /// thrown. `aimed` is the right stick's press (the held word's 0x400,
    /// [`aim_pressed`]); `start` the players' places as the update began.
    pub(crate) fn pass_button(&mut self, i: usize, input: &Input, aimed: bool, start: &[[f32; 2]]) {
        // The right stick's press starts a give 'n go before the pass
        // start (`0x15b900..0x15b940`): the person keeps the passer.
        if aimed {
            self.give_and_go_press(i);
        }
        let choice = self.pass_press(i, input, aimed, start);
        if choice != PassChoice::Refused {
            // The pass start clears game+0x94 (`0x15fa20`).
            self.fresh_take = false;
        }
        match choice {
            // A pass back to the man who gave and went is the give 'n
            // go's second half.
            PassChoice::Pass { to, style }
                if style == PassStyle::Normal && self.give_and_go == Some((to, i)) =>
            {
                self.wind_up(i, to, PassStyle::GiveAndGo, input);
                self.give_and_go = None;
            }
            PassChoice::Pass { to, style } => self.wind_up(i, to, style, input),
            PassChoice::Heezay { defender, to } => self.heezay(i, defender, to),
            // Callback 53 to himself, callback 38 to a teammate.
            PassChoice::Glass { to, point } => self.passout_press(i, to, point),
            PassChoice::Refused => {}
        }
    }

    /// `FUN_00146278(pad)` (raw `0x146278..0x1476bc`), the target of a
    /// pass press, which the pad builder writes to actor+0x74 in the
    /// passer's update; the holder's update throws to it.
    ///
    /// - A computer's pad names its receiver (`FUN_00146228(pad, k)`,
    ///   pad[9]): that is the target.
    /// - The direction: the right stick's angle for its press (0x400,
    ///   pad[5]); else the D-pad's or the stick's pushed 16 or more of 64;
    ///   else the facing he wants (actor+0xb0). The original adds the row
    ///   camera's yaw (`DAT_00402020`+0x94, in units) to each, the pad's
    ///   being the screen's: the sim's sticks are already the court's, and
    ///   at rest the facing word is turned by the person's [`Input::view`]
    ///   (the client's camera; the pass captures' helper turns -0.98..2.41
    ///   radians, mostly 45 to 130 units either way).
    /// - Each teammate's arrival: the pass's flight over his distance
    ///   ([`flight_updates`], the three-dimensional distance) plus the
    ///   windup, `trunc(fps × 0.3167)` = 19 for a passer in a dunk, a leap
    ///   or the air, else `trunc(fps × 0.1834)` = 11.
    /// - Teammates leaping for an alley-oop (bit 15) and in the air: one is
    ///   the target, unless his meet (actor+0x31c) less his arrival is more
    ///   than `trunc(fps × 0.1)` = 6 short while he falls below 6.5, when
    ///   the other teammate is (`0x14674c..0x14683c`); of two, the most
    ///   risen (actor+0x60) of those whose meet less arrival is at least
    ///   his lead (actor+0x320, [`crate::leap::lead`]), else the one with
    ///   the latest meet (`0x146844..0x146934`).
    /// - A passout special ([`World::passout`]) with two turbos held, on the
    ///   floor, not for the right stick's press.
    /// - Else the leaper, or the teammate best lined up
    ///   ([`World::lined_up`]).
    pub(crate) fn pass_press(
        &mut self,
        i: usize,
        input: &Input,
        aimed: bool,
        start: &[[f32; 2]],
    ) -> PassChoice {
        let p = &self.players[i];
        let team = p.team;
        let style = if aimed {
            PassStyle::GiveAndGo
        } else if input.held(crate::button::L3) {
            PassStyle::Kick
        } else {
            PassStyle::Normal
        };
        if input.is_computer()
            && let Some(to) = self
                .ai
                .brains
                .get(i)
                .and_then(|b| usize::try_from(b.pad.target).ok())
                .filter(|&to| to < self.players.len() && to != i)
        {
            return PassChoice::Pass { to, style };
        }
        let (word, intensity) = if aimed {
            let a = input.aim();
            (stick_word(a), 0)
        } else {
            let s = input.stick();
            let n = (math::len(s[0], s[1]) * 64.0).round() as i32;
            if n >= 0x10 {
                (stick_word(s), n.min(64))
            } else {
                // The facing word turned by the camera's yaw, as the
                // original turns every direction it reads (`0x146510`):
                // the sim's sticks are already the court's, the facing
                // is not the screen's.
                let word = crate::timing::facing_word(p.wanted) + i32::from(input.view);
                (word & 0x3ff, 0)
            }
        };
        let fps = crate::TICK_HZ as f32;
        let bits = crate::block::holder_bits(&p.action);
        let windup = if bits & 0x8080 != 0 || p.height > 0.0 {
            (fps * f32::from_bits(0x3ea2_2681)) as i32
        } else {
            (fps * f32::from_bits(0x3e3b_cd36)) as i32
        };
        let squad: Vec<usize> = (0..self.players.len())
            .filter(|&j| self.players[j].team == team)
            .collect();
        let at3 = |j: usize| {
            let q = &self.players[j];
            [q.position[0], q.height, q.position[1]]
        };
        let arrive = |j: usize| {
            let (a, b) = (at3(i), at3(j));
            let d = math::len3(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
            flight_updates(d) + windup
        };
        let leapers: Vec<usize> = squad
            .iter()
            .copied()
            .filter(|&j| j != i && self.players[j].leaping() && self.players[j].height > 0.0)
            .collect();
        let mut target = None;
        match leapers.as_slice() {
            [j] => {
                let q = &self.players[*j];
                let late = i32::from(q.flight.meet) - arrive(*j) < -((fps * 0.1) as i32);
                target = Some(*j);
                if late && q.flight.velocity[1] < 0.0 && q.height <= 6.5 {
                    // The other teammate, unless he leaps too.
                    for &k in &squad {
                        if k != i && k != *j && !self.players[k].leaping() {
                            target = Some(k);
                        }
                    }
                }
            }
            [_, _] => {
                let lead = |j: usize| {
                    let row = match self.players[j].action {
                        Action::Dunking { row, .. } => row,
                        _ => crate::timing::NO_MOVE_ROW,
                    };
                    crate::timing::move_row(row)
                        .map_or(0, |r| i32::from(crate::leap::lead(r.script)))
                };
                // Actor+0x60, the updates since his jump: the leap's
                // updates stand in (both leaps run their scripts' blend
                // first).
                let risen = |j: usize| match self.players[j].action {
                    Action::Dunking { tick, .. } => i32::from(tick),
                    _ => 0,
                };
                let mut best = -1;
                for &j in &leapers {
                    if i32::from(self.players[j].flight.meet) - arrive(j) >= lead(j)
                        && best < risen(j)
                    {
                        best = risen(j);
                        target = Some(j);
                    }
                }
                if target.is_none() {
                    for &j in &leapers {
                        if target.is_none_or(|t: usize| {
                            self.players[t].flight.meet < self.players[j].flight.meet
                        }) {
                            target = Some(j);
                        }
                    }
                }
            }
            _ => {}
        }
        let two = if PASSOUT_ONE_TURBO {
            input.turbos() >= 1
        } else {
            input.turbos() >= 2
        };
        let floor = !PASSOUT_ON_GROUND || self.players[i].height == 0.0;
        if !aimed && two && floor {
            return self.passout(i, word, intensity, target);
        }
        match target {
            Some(to) => PassChoice::Pass { to, style },
            None => match self.lined_up(i, word, start) {
                Some(to) => PassChoice::Pass { to, style },
                None => PassChoice::Refused,
            },
        }
    }

    /// `FUN_00146278`'s plain pick (`0x14743c..0x14766c`): for each
    /// teammate the angle between `word` and the line to him (the game's
    /// angle table, game+0x2cc), less 9 if he posts up (bit 11) or his
    /// lane to the rim is open (`FUN_0017f4d0(ai, 0)`)
    /// within 14 of it, 9 if his defender (`FUN_0016e8f8`) is more than 15
    /// off him or down (`FUN_0016ea70`), 11 if his nearest opponent
    /// (`FUN_00170dc0`) is more than 15 off, 9 if he is his side's nearest
    /// the rim (the second nearest when the passer is the nearest,
    /// `FUN_0013f8a8`). The one with the least of that under 228 (the
    /// first of equals), else the one with the least of it plus his
    /// distance (game+0x1ac). `start`: the places the tables hold.
    pub(crate) fn lined_up(&self, i: usize, word: i32, start: &[[f32; 2]]) -> Option<usize> {
        let team = self.players[i].team;
        let at = |k: usize| start.get(k).copied().unwrap_or(self.players[k].position);
        let between = |a: usize, b: usize| distance(at(a), at(b));
        let rim = self.rim_spot(target_hoop(self.possession.min(1)));
        let to_rim = |k: usize| distance(at(k), rim);
        let squad: Vec<usize> = (0..self.players.len())
            .filter(|&j| self.players[j].team == team)
            .collect();
        let mut by_rim = squad.clone();
        by_rim.sort_by(|&a, &b| to_rim(a).total_cmp(&to_rim(b)));
        let nearest = if by_rim.first() == Some(&i) {
            by_rim.get(1)
        } else {
            by_rim.first()
        }
        .copied();
        let (mut best_angle, mut by_angle) = (0x400, None);
        let (mut best_sum, mut by_sum) = (1224.0f32, None);
        for &k in &squad {
            if k == i {
                continue;
            }
            let toward = crate::contact::angle(at(i), at(k));
            let mut s = crate::timing::angle_diff(word, toward).abs();
            if self.posting(k) || (crate::ai::lane_open(self, k) && to_rim(k) < 14.0) {
                s -= 9;
            }
            if let Some(d) = self.holder_defender(k)
                && (15.0 < between(k, d) || self.down(d))
            {
                s -= 9;
            }
            if self
                .nearest_opponent(k)
                .is_some_and(|o| 15.0 < between(k, o))
            {
                s -= f32::from_bits(0x413e_0b3f) as i32;
            }
            if nearest == Some(k) {
                s -= 9;
            }
            let sum = between(i, k) + s as f32;
            if sum < best_sum {
                best_sum = sum;
                by_sum = Some(k);
            }
            if s < best_angle {
                best_angle = s;
                by_angle = Some(k);
            }
        }
        if best_angle < f32::from_bits(0x4364_0e0f) as i32 {
            by_angle
        } else {
            by_sum.or(by_angle)
        }
    }

    /// `FUN_00146278`'s passout special (`0x1469b8..0x147434`), for a
    /// passer with two turbos held on the floor, aiming along `word`
    /// pushed `intensity` (0 at rest), `leaper` the target the alley-oop
    /// rule gave.
    ///
    /// Short of 0.2 of his bar ([`World::turbo_of`]) no pass goes, and
    /// the "no turbo" message (`0x3c6520`). With the stick at rest (under
    /// 16) it is Off the Heezay: his defender (`FUN_0016e8f8`) within 12
    /// (else no pass, `FUN_00171e30(actor, 0)`), the ball off his head
    /// coming down to the leaper or the passer. Pushed, it is Back 2 Papa:
    /// the stick within 128 of the line to the rim (`FUN_0016f260`; else
    /// `FUN_00171e30(actor, 0)`), the passer in the half his side attacks,
    /// within 35 of the middle and inside the board's wedge (each else
    /// `FUN_00171e30(actor, 1)`); off the board to the leaper, or from
    /// within 8 across and 29 along to himself, else to the nearest
    /// teammate across the lane in the same wedge ([`World::glass_mate`];
    /// none, `FUN_00171e30(actor, 1)`). Then 0.2 of the bar
    /// (`FUN_00190c08` at `0x14741c`) and the passout state
    /// (`FUN_00142068`, game+0x140..+0x160).
    fn passout(
        &mut self,
        i: usize,
        word: i32,
        intensity: i32,
        leaper: Option<usize>,
    ) -> PassChoice {
        if self.turbo_of(i) < tuning::TRICK_PASS_TURBO {
            return PassChoice::Refused;
        }
        let p = &self.players[i];
        let [x, z] = p.position;
        if intensity < 0x10 {
            let Some(d) = self.holder_defender(i) else {
                self.passout_failed(i, false);
                return PassChoice::Refused;
            };
            let q = &self.players[d];
            let gap = math::len3(q.position[0] - x, q.height - p.height, q.position[1] - z);
            if gap >= 12.0 {
                self.passout_failed(i, false);
                return PassChoice::Refused;
            }
            let to = leaper.unwrap_or(i);
            self.spend_turbo(i, tuning::TRICK_PASS_TURBO);
            return PassChoice::Heezay { defender: d, to };
        }
        let rim = self.rim_spot(target_hoop(p.team));
        let to_rim = crate::contact::angle(p.position, rim);
        if crate::timing::angle_diff(to_rim, word).abs() > 0x80 {
            self.passout_failed(i, false);
            return PassChoice::Refused;
        }
        let side = crate::restart::side(p.team.min(1));
        let front = (x >= 0.0) == (side > 0.0);
        if !front || x.abs() >= 35.0 || !in_wedge([x, z]) {
            self.passout_failed(i, true);
            return PassChoice::Refused;
        }
        let to = match leaper {
            Some(j) => j,
            None if z.abs() < 8.0 && x.abs() < 29.0 => i,
            None => match self.glass_mate(i) {
                Some(k) => k,
                None => {
                    self.passout_failed(i, true);
                    return PassChoice::Refused;
                }
            },
        };
        let point = self.glass_point(i, to, side);
        self.spend_turbo(i, tuning::TRICK_PASS_TURBO);
        PassChoice::Glass { to, point }
    }

    /// Back 2 Papa's teammate from the wing (`0x146e5c..0x147124`): of the
    /// teammates in the half the side attacks within 35 of the middle, the
    /// nearest the rim (game+0x374) across the lane from the passer
    /// (`|z|` 3 or more on the other side) inside the board's wedge. A
    /// teammate within 3 of the middle line is never taken: the walk keeps
    /// the best side in a register that starts at -1, which a middle
    /// teammate (side 0) never matches.
    fn glass_mate(&self, i: usize) -> Option<usize> {
        let p = &self.players[i];
        let own = if p.position[1] >= 0.0 { 1 } else { -1 };
        let side = crate::restart::side(p.team.min(1));
        let rim = self.rim_spot(target_hoop(p.team));
        let (mut best, mut kept, mut found) = (200.0f32, -1, None);
        for k in (0..self.players.len()).filter(|&k| k != i && self.players[k].team == p.team) {
            let [x, z] = self.players[k].position;
            if (x >= 0.0) != (side > 0.0) || x.abs() >= 35.0 {
                continue;
            }
            let lane = if z.abs() < 3.0 {
                0
            } else if z >= 0.0 {
                1
            } else {
                -1
            };
            if lane == own {
                continue;
            }
            let d = distance(self.players[k].position, rim);
            if lane == -own {
                if !in_wedge([x, z]) {
                    continue;
                }
                if kept != lane || d < best {
                    best = d;
                    kept = lane;
                    found = Some(k);
                }
            } else if kept == 0 && d < best {
                best = d;
                kept = lane;
                found = Some(k);
            }
        }
        found
    }

    /// Back 2 Papa's point on the board (`0x14714c..0x1473f4`): `38 - r`
    /// along the court on the side the team attacks and `15 - r` up (`r`
    /// the ball's radius, ball+0x11c); then `rand() % 80` under the
    /// passer's Handles (`FUN_001d7d00(record, 7)`) puts it across where
    /// the line from the passer to the receiver's x meets the board,
    /// within ±3, and `15 - rand · 0.4 · 3.5` up; otherwise anywhere
    /// across `-3 - r .. 3 + r` and `11.5 - r + rand · (r + 1.4)` up,
    /// where the pass can miss (School: "If you are out of position when
    /// attempting this move, the ball will miss its target").
    fn glass_point(&mut self, i: usize, to: usize, side: f32) -> [f32; 3] {
        let r = self.ball.radius;
        let x = side * (38.0 - r);
        let handles = (self.players[i].ratings.handles * 99.0)
            .round()
            .clamp(0.0, 99.0) as u32;
        let unit = |v: u32| (v & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
        let (y, z);
        if self.rng.next_u32() % 80 < handles {
            let p = self.players[i].position;
            let q = self.players[to].position;
            // `FUN_0014fa60(receiver x, ball)`: his x mirrored behind the
            // board for its restitution ([`crate::ball::bank_mirror`]).
            let mirrored = crate::ball::bank_mirror(q[0], side, r);
            let t = (x - p[0]) / (mirrored - p[0]);
            z = (t * q[1] + (1.0 - t) * p[1]).clamp(-3.0, 3.0);
            y = 15.0 - unit(self.rng.next_u32()) * 0.4 * 3.5;
        } else {
            z = (-3.0 - r) + (unit(self.rng.next_u32()) * 2.0) * (r + 3.0);
            y = (11.5 - r) + unit(self.rng.next_u32()) * (r + 1.4);
        }
        [x, y, z]
    }

    /// `FUN_00171e30(actor, kind)`: the failed passout. Unless the ball is
    /// a gamebreaker's (`FUN_00141018`) or he already plays one of the
    /// fails (records 0x21d..0x220, 0x223, 0x224), kind 0 plays
    /// `passout_boomerang_1_fail` or `_2_fail` (`rand() % 100` under 50)
    /// and kind 1 turns him to the rim (actor+0xb0 the line to it,
    /// `FUN_0016f260`) for `passout_2_fail`. The rewrite draws and turns
    /// but does not play the fails' scripts (the holder stays as he was).
    fn passout_failed(&mut self, i: usize, board: bool) {
        if self.gamebreaker_held(i) {
            return;
        }
        if board {
            let hoop = target_hoop(self.players[i].team);
            let rim = self.rim_spot(hoop);
            let p = &mut self.players[i];
            let (dx, dz) = (rim[0] - p.position[0], rim[1] - p.position[1]);
            if math::len(dx, dz) > 1e-3 {
                p.wanted = math::atan2(dx, dz);
            }
        } else {
            self.rng.next_u32();
        }
    }

    /// The ball is a gamebreaker's (ball+0x178, `FUN_00141018`): the
    /// rewrite's gamebreakers are jump shots, so while its holder shoots
    /// one.
    pub(crate) fn gamebreaker_held(&self, i: usize) -> bool {
        matches!(
            self.players[i].action,
            Action::Shooting {
                gamebreaker: 1..,
                ..
            }
        )
    }

    /// Throw a pass from `i` to `to` now, aimed as the press aims it: for a
    /// teammate leaping for an alley-oop, the lob to his hand.
    #[cfg(test)]
    pub(crate) fn throw(&mut self, i: usize, to: usize, style: PassStyle) {
        let aim = self.pass_aim(i, to);
        let start = self.floor_positions();
        self.throw_aimed(i, to, style, aim, &start);
    }

    /// [`World::throw`] with the press's aim already taken.
    pub(crate) fn throw_aimed(
        &mut self,
        i: usize,
        to: usize,
        style: PassStyle,
        aim: PassAim,
        start: &[[f32; 2]],
    ) {
        let style = self.launch(i, to, style, aim, start, None);
        self.after_throw(i, to, style);
    }

    /// The ball leaves the passer's hands (or `from`, where it already
    /// is) toward `to`; the style it went as (a lob to a leaper is an
    /// Oop).
    ///
    /// The release callback (`FUN_00173570`) times it: the catch draw
    /// (`FUN_001611d8`) sets the flight ([`World::release_flight`]) and
    /// the point (the spot, or where a led receiver will be), and the
    /// launcher (`FUN_0014c0c0`) gives the ball the velocity that gets it
    /// there in exactly that many updates under gravity: the distance
    /// over the updates plus half the gravity times them upward
    /// ([`crate::Ball::throw`] with the flight in seconds). The point is the
    /// catch row's hand posed where the receiver will stand
    /// ([`World::catch_target`]; without the disc's animations the
    /// rewrite's [`PassRelease::release_height`]). A lob to a leaper draws no
    /// catch row: `FUN_00162aa8` times it to his script's span and aims it
    /// at his hand then ([`World::lob_to_leaper`]).
    pub(crate) fn launch(
        &mut self,
        i: usize,
        to: usize,
        style: PassStyle,
        mut aim: PassAim,
        start: &[[f32; 2]],
        from: Option<[f32; 3]>,
    ) -> PassStyle {
        let r = &self.players[to];
        // A teammate leaping for an alley-oop makes it an Oop before its
        // release is taken, so the lob's launch and kind are its own.
        let leaper = r.leaping() && r.height > 0.0;
        let board = self.passout.is_some_and(|po| po.passer == i && !po.tossed);
        let style = if leaper && !board {
            PassStyle::Oop
        } else {
            style
        };
        let rec = style.release();
        let p = &self.players[i];
        let f = p.forward();
        // From where the ball already is (the tip, thrown from the
        // jumper's hand in the air), the thrower stays in his own script.
        // Callback 38 throws a held ball from the pass clip's hand, where
        // the last ball update held it ([`crate::held`]); without the
        // disc's animations from the rewrite's own point.
        let in_hand = from.is_none();
        let from = from.unwrap_or(
            if self.poses.get().is_some() && self.ball.state == BallState::Held(i) {
                self.ball.position
            } else {
                [
                    p.position[0] + f[0] * 0.8,
                    rec.release_height,
                    p.position[1] + f[1] * 0.8,
                ]
            },
        );
        let (mut target, t, flight) = if leaper {
            let (n, point) = self.lob_to_leaper(i, to, start);
            (point, n as f32 / crate::TICK_HZ as f32, None)
        } else {
            let n = self.release_flight(i, to, &mut aim, start) as i32;
            // The distance the flight was timed on (`f24`).
            let at_k = |k: usize| start.get(k).copied().unwrap_or(self.players[k].position);
            let d = self.board_flight(i, aim.spot).map_or_else(
                || {
                    if aim.lead {
                        distance(at_k(i), at_k(to))
                    } else {
                        distance(at_k(i), aim.spot)
                    }
                },
                |(distance, _)| distance,
            );
            let at = self.release_point(to, &aim, n, d);
            (
                [at[0], rec.release_height, at[1]],
                n as f32 / crate::TICK_HZ as f32,
                Some((at, n)),
            )
        };
        self.players[i].pass_kind = rec.kind;
        if in_hand {
            self.players[i].action = Action::Passing {
                elapsed: 0.0,
                row: crate::timing::NO_PASS_ROW,
                tick: 0,
            };
        }
        // The release callback draws the receiver's catch row
        // (`FUN_001611d8`), which holds him after the catch. Its count
        // goes on the receiver (+0x1b0) first, and a row with its own
        // count (+4) replaces it there (`0x162a4c`); the ball keeps the
        // flight (`DAT_003d65a0`, taken at `0x162330`). The first-pass
        // flag is read by the row filter and cleared (`0x1622c0`). Then it
        // aims the pass at the row's hand posed where the receiver will
        // stand (`DAT_003d6590`, [`World::catch_target`]) and starts the
        // row's script.
        let mut catch = crate::ball::NO_CATCH;
        if let Some((spot, n)) = flight {
            let passer_flight = self.players[i].received_flight;
            let first = self.tip.first_pass;
            // A spot run's row list is `catch_stand_1` alone, still drawn
            // (`0x1622c4..0x162310`, `0x162404`).
            let row = if self.spot_run.count != 0 && self.players[to].height <= 0.0 {
                self.rng.next_u32();
                Some(crate::receiver::STAND_ROW)
            } else {
                crate::timing::catch_row(self, i, to, spot, passer_flight, first)
            };
            self.tip.first_pass = false;
            // No row: the flight's count stays on him and the pass keeps
            // its own target (`0x1623a4` goes on past `0x162a4c`).
            let own = row.map_or(0, |row| crate::timing::CATCH_ROWS[row].min_flight);
            self.players[to].received_flight = if own > 0 { i32::from(own) } else { n };
            if let Some(row) = row {
                catch = row as u8;
                let run = self.spot_run.count != 0;
                if let Some(hand) = self.catch_target(to, row, spot, n, run) {
                    target = hand;
                }
            }
        }
        let pass = Pass {
            from: i,
            to,
            tried: 0,
            via: Via::Direct,
            catch,
            since: self.tick,
        };
        if board {
            self.board_release(
                from,
                target,
                (t * crate::TICK_HZ as f32).round() as i32,
                pass,
            );
        } else {
            self.ball.throw(from, target, t, pass);
        }
        // `FUN_0014c0c0`: both sides' kind 2.
        self.ball.long = [false; 2];
        if let Some((_, n)) = flight {
            // A spot run parks no row (`0x162420..0x162450`); the row's
            // hand is his live hand either way.
            if catch == crate::ball::NO_CATCH {
                self.catching = None;
            } else if self.spot_run.count == 0 {
                self.start_catching(to, usize::from(catch));
            } else {
                self.catching = None;
                self.catch_hand(to, usize::from(catch));
            }
            if self.poses.get().is_some() {
                self.release_bars(i, to, n);
            }
        } else if leaper && self.poses.get().is_some() {
            let n = (t * crate::TICK_HZ as f32).round() as i32;
            self.release_bars(i, to, n);
        }
        // Callback 38's last: the controller word goes with the ball
        // (`0x173dd0..0x173e84`, [`World::release_hand_off`]).
        if in_hand {
            self.release_hand_off(i, to);
        }
        style
    }

    pub(crate) fn after_throw(&mut self, from: usize, to: usize, style: PassStyle) {
        // Slot 9 (`FUN_0014bff8`, the press): the thrower in the air or
        // not, which the pass's catch reads for its ring class. A pass
        // itself is worth nothing to the meter: its class enters the ring
        // at the catch (slot 19, [`World::post_pass_caught`]).
        self.post_pass_thrown(from);
        self.give_and_go = (style == PassStyle::GiveAndGo).then_some((from, to));
        self.events.push(Event::Pass { from, to, style });
    }

    /// A computer player's trick pass to its named target (`FUN_00183318`, which
    /// asks the game for the throw directly, without the button rules a
    /// person's press goes through): off defender `d`'s head, or off the
    /// backboard. The turbo is spent as for a person's (the rewrite's
    /// rule). False when he cannot throw it now.
    pub(crate) fn computer_trick_pass(
        &mut self,
        i: usize,
        to: usize,
        defender: Option<usize>,
        point: [f32; 3],
    ) -> bool {
        if self.ball.state != BallState::Held(i) || !self.players[i].free() {
            return false;
        }
        if !self.spend_turbo(i, tuning::TRICK_PASS_TURBO) {
            return false;
        }
        match defender {
            Some(d) => self.heezay(i, d, to),
            // `FUN_00183318`'s board point (its draws are the AI's,
            // `ai::offence`), tossed on a passout row ([`crate::passout`]).
            None => self.passout_press(i, to, point),
        }
        true
    }

    /// Off the Heezay: off defender `d`'s head and down to `to` (the passer
    /// or a teammate) on a passout row ([`World::special_pass_press`]),
    /// the legs timed at its release ([`World::head_legs`]) and met at the
    /// head bone ([`World::head_leg`]).
    fn heezay(&mut self, i: usize, d: usize, to: usize) {
        // The press stores the head's point (`0x146a44..`, `0x183378..`)
        // and the pass waits for its row's callback 53 (to himself) or 38.
        let point = self.head_point(d);
        self.special_pass_press(i, to, point, Some(d));
    }

    /// Callback 53's head legs for a pass to himself ([`World::head_legs`]):
    /// the first to the defender's head, the second to the passer's hand.
    pub(crate) fn throw_head(&mut self, i: usize, d: usize, from: [f32; 3], target: [f32; 3]) {
        let (head, first, total) = self.head_legs(d, from, target);
        let rec = PassStyle::Heezay.release();
        self.players[i].pass_kind = rec.kind;
        // Game+0x160 is 2 (0x146ab8..0x146ac4). Keep it after the
        // head bounce too: the catch pays FUN_00143a88's separate value.
        self.passout = Some(crate::passout::Passout {
            passer: i,
            kind: crate::passout::Kind::Head,
            defender: Some(d),
            point: head,
            back: Some((target, total)),
            tossed: true,
            tail: None,
        });
        self.ball.throw(
            from,
            head,
            first as f32 / crate::TICK_HZ as f32,
            Pass {
                from: i,
                to: i,
                tried: u8::MAX,
                via: Via::Head { defender: d },
                catch: crate::ball::NO_CATCH,
                since: self.tick,
            },
        );
        // Callback 53 bars the passer's side until the whole flight ends
        // (0x174b1c..0x174b68).
        let n = total.clamp(0, i32::from(u16::MAX)) as u16;
        let team = self.players[i].team;
        for p in self.players.iter_mut().filter(|p| p.team == team) {
            p.barred = n;
        }
    }

    /// A person off the ball on a held side calls ([`World::off_ball_call`]):
    /// a holder the computer drives has his AI's queue cleared
    /// (`FUN_0013aa48`) and one node pushed (`FUN_0013aa88`), built here as
    /// the caller's update builds it: for Circle a shoot node
    /// (`FUN_00100e38`) unless the holder shoots, dunks, lays up or leaps
    /// (`0xa088`); for Cross, unless he is passing (bit 4), a pass node to
    /// the caller (`FUN_00100a08`) or with two turbos a trick pass node
    /// with no receiver (`FUN_00100c60(ai, 0, 0)`).
    pub(crate) fn call_for_ball(
        &mut self,
        i: usize,
        call: crate::ai::Call,
        frame: &crate::ai::Frame,
    ) {
        let Some(h) = self.handler() else {
            return;
        };
        if h == i || self.team_of(h) != self.team_of(i) || self.people[h] {
            return;
        }
        let busy = match call {
            crate::ai::Call::Shoot => {
                crate::block::holder_bits(&self.players[h].action) & 0xa088 != 0
            }
            _ => {
                self.windup.is_some_and(|w| w.from == h)
                    || matches!(self.players[h].action, Action::Passing { .. })
            }
        };
        if busy {
            return;
        }
        self.push_call(frame, h, i, call);
    }

    /// The spot `gap` from the rim on player `j`'s side, where a dunker or
    /// leaper ends up.
    pub(crate) fn rim_side(&self, j: usize, hoop: usize, gap: f32) -> [f32; 2] {
        let rim = self.rim_spot(hoop);
        let away = normalize2(sub(self.players[j].position, rim));
        [rim[0] + away[0] * gap, rim[1] + away[1] * gap]
    }

    /// Defender `j` picks off `pass`: a steal (slot 15), with its points.
    pub(crate) fn intercept(&mut self, j: usize, pass: Pass) {
        // The take of a pass by the other side is slot 15's steal
        // ([`World::post_take`]).
        self.take_ball(j);
        self.events.push(Event::Steal {
            player: j,
            from: pass.from,
            intercepted_pass: true,
        });
    }

    /// One tick of a pass in the air: at the end of a leg off a head or
    /// the glass it carries on (or a defender in the counter stance catches
    /// it off his head); at its target the receiver catches it if they are
    /// there.
    ///
    /// With the disc's animations the actors' own tests touch it in their
    /// updates ([`crate::touch`]): a defender's limb knocks it in its last
    /// quarter on his Steals roll, the receiver's hands catch it as they
    /// meet it, and his other limbs knock it off; a pass with a catch row
    /// flies on past its target until they do or its timer runs out
    /// ([`crate::touch::PASS_TAIL`]). Without them the rewrite's stand-in:
    /// a defender standing free within [`tuning::INTERCEPT_RANGE`] on the
    /// floor and under his reach takes it at once (the original's two
    /// captured matches have no such catch: their passes lost to a defender
    /// were all blocks or strips), and the receiver catches it on arrival.
    pub(crate) fn step_pass(&mut self) {
        let Some(pass) = self.ball.pass else {
            return;
        };
        let b = self.ball.position;
        let passer_team = self.players[pass.from].team;
        let flown = 1.0 - self.ball.flight_left / self.ball.flight_time.max(1e-6);
        if self.poses.get().is_none() {
            self.intercept_stand_in(pass, b, passer_team, flown);
            if self.ball.pass.is_none() {
                return;
            }
        }
        self.carry_pass(pass);
    }

    /// The rewrite's stand-in interception ([`World::step_pass`]).
    fn intercept_stand_in(&mut self, pass: Pass, b: [f32; 3], passer_team: usize, flown: f32) {
        for j in 0..self.players.len() {
            // A pass to oneself (`tried` all set: the trick passes and
            // self-tosses) is the rewrite's own exemption.
            if flown <= 0.75 || pass.tried == u8::MAX {
                break;
            }
            let q = &self.players[j];
            if q.team == passer_team || q.action != Action::None {
                continue;
            }
            let steals = (q.ratings.steals * 99.0).round().clamp(0.0, 99.0);
            let odds = (steals / 99.0 * 15.0) as u32;
            if self.rng.next_u32() % 100 >= odds {
                continue;
            }
            let near = distance(q.position, [b[0], b[2]]) < tuning::INTERCEPT_RANGE;
            if near && b[1] < q.ratings.reach + q.height {
                self.intercept(j, pass);
                return;
            }
        }
    }

    /// The pass flies on: the end of a leg, or the catch
    /// ([`World::step_pass`]).
    fn carry_pass(&mut self, pass: Pass) {
        if !self.ball.fly_pass() {
            return;
        }
        match pass.via {
            Via::Head { defender }
                if matches!(
                    self.players[defender].action,
                    Action::Stealing { counter: true, .. }
                ) =>
            {
                // `FUN_00148378`'s head leg (kind 2): a defender in the
                // counter stance (`FUN_0016e9b0`) catches the ball off his
                // head.
                self.intercept(defender, pass);
                self.players[defender].action = Action::None;
            }
            Via::Head { defender } => self.head_leg(pass, defender),
            Via::Glass { .. } if self.passout_leg(pass) => {}
            // A board pass without its queued leg has lost its route.
            Via::Glass { .. } => self.ball.drop_loose(),
            Via::Direct | Via::Floor if crate::touch::bone_catch(self, &pass) => {
                // The receiver's tests catch it ([`crate::touch`]); at the
                // end of its timer (flight + 5, `FUN_0014d1f8`'s 4 to 1) it
                // is loose.
                if self.ball.flown_ticks() >= self.ball.flight_ticks() + crate::touch::PASS_TAIL {
                    self.catching = None;
                    self.ball.drop_loose();
                }
            }
            Via::Direct | Via::Floor => {
                let r = &self.players[pass.to];
                let b = self.ball.position;
                let is_self_toss = pass.from == pass.to;
                let reach = match r.action {
                    Action::None | Action::Passing { .. } => r.ratings.reach,
                    Action::Jumping { .. } => r.ratings.reach + r.height + 1.0,
                    _ if r.leaping() => r.ratings.reach + r.height + 1.0,
                    Action::Trick { .. } if is_self_toss => r.ratings.reach + 2.0,
                    _ => 0.0,
                };
                if (distance(r.position, [b[0], b[2]]) < tuning::CATCH_RANGE || is_self_toss)
                    && (b[1] < reach || is_self_toss)
                {
                    self.receive(pass);
                } else {
                    self.ball.drop_loose();
                }
            }
        }
    }

    /// The pass's receiver has it (`FUN_0014cb08`). The catch row's script
    /// keeps him catching (bit 9) for the flight's nominal end and its tail
    /// (`timing::catch_hold`).
    pub(crate) fn receive(&mut self, pass: Pass) {
        // The tip's pass caught: the opening phase ends (`FUN_001415c0`,
        // `FUN_0013e4c8`).
        if self.opening_toss == crate::OpeningToss::Flight {
            self.opening_toss = crate::OpeningToss::None;
        }
        let left = (self.ball.flight_left * crate::TICK_HZ as f32).round() as i32;
        let flight = self.ball.flight_ticks();
        self.take_ball(pass.to);
        if usize::from(pass.catch) < crate::timing::CATCH_ROWS.len() {
            let hold = crate::timing::catch_hold(usize::from(pass.catch), left);
            self.catch = Some(crate::Catch {
                player: pass.to,
                until: self.tick + hold as u64,
                row: pass.catch,
                flight: flight.clamp(0, i32::from(u8::MAX)) as u8,
                since: pass.since,
            });
        }
        self.events.push(Event::Catch { player: pass.to });
    }
}

/// One pass flight read from a per-update capture of the original, for
/// [`flight_agreement`] (`ai::capture`, `played_capture`).
#[cfg(test)]
pub(crate) struct CapturedFlight {
    /// Ball+0x64 in the release sample.
    pub flight: i32,
    /// The first pass after the opening toss (the tip).
    pub first: bool,
    /// Thrown to oneself (the trick passes: off a head, off the glass).
    pub to_self: bool,
    /// A lob to a leaper (`FUN_00162aa8`): the receiver off the floor or
    /// the target above 12.
    pub lob: bool,
    /// The passer's and the receiver's places in the sample before the
    /// release (as its update began, when the distance table is built).
    pub passer: [f32; 2],
    pub receiver: [f32; 2],
    /// The receiver's place when the press's pass start ran (the spot of
    /// a receiver who stands, or nearly, until the release): the press's
    /// sample if his actor updates before the passer's, else the one
    /// before.
    pub receiver_at_press: [f32; 2],
}

/// How a capture's pass flights compare with the rewrite's.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct FlightAgreement {
    /// The first passes' flights.
    pub first: Vec<i32>,
    pub ordinary: usize,
    /// Ordinary flights the production release gives exactly for a led
    /// receiver standing where the capture had him.
    pub led: usize,
    /// The rest that the rule gives for the receiver's place at the press
    /// (a spot pass to where he stood).
    pub spot: usize,
    /// The others: (captured, led, spot).
    pub off: Vec<(i32, i32, i32)>,
    pub to_self: usize,
    pub lobs: usize,
}

/// Compare captured flights with the rewrite's: each ordinary pass is
/// thrown through [`World::throw`] by a passer and a receiver standing
/// where the capture had them as the release update began (a standing
/// receiver is led, timed on that distance), and also timed by
/// [`flight_updates`] on the receiver's place at the press. The spot of a
/// running receiver (his behaviour's destination) is not in the captures.
#[cfg(test)]
pub(crate) fn flight_agreement(flights: &[CapturedFlight]) -> FlightAgreement {
    let mut out = FlightAgreement::default();
    for f in flights {
        if f.first {
            out.first.push(f.flight);
            continue;
        }
        if f.to_self {
            out.to_self += 1;
            continue;
        }
        if f.lob {
            out.lobs += 1;
            continue;
        }
        out.ordinary += 1;
        let mut w = World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, f.passer, 0.0),
                crate::Player::new(0, f.receiver, 0.0),
            ],
            crate::Rules::default(),
            1,
        );
        w.take_ball(0);
        w.throw(0, 1, PassStyle::Normal);
        let led = w.ball.flight_ticks();
        let spot = flight_updates(distance(f.passer, f.receiver_at_press));
        if led == f.flight {
            out.led += 1;
        } else if spot == f.flight {
            out.spot += 1;
        } else {
            out.off.push((f.flight, led, spot));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lob_to_a_leaper_uses_the_oop_release() {
        // G08: the style is settled before its release is taken, so a pass
        // to a teammate leaping for an alley-oop launches and marks the
        // passer as an Oop, not as the style the button asked for.
        let mut w = crate::World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, [-10.0, 0.0], 0.0),
                crate::Player::new(0, [-25.0, 4.0], 0.0),
            ],
            crate::Rules::default(),
            1,
        );
        w.ball.hold(0);
        // An alley-oop row's leap (`allydunk_2`).
        w.start_leap(1, 94);
        w.players[1].height = 3.0; // Callback 38 also requires airborne height.
        assert!(w.players[1].leaping());
        w.throw(0, 1, PassStyle::Kick);
        let oop = PassStyle::Oop.release();
        assert_eq!(w.players[0].pass_kind, oop.kind);
        assert_eq!(w.ball.position[1], oop.release_height);
        // Kick and plain passes keep the kinds the table shows for them.
        assert_eq!(PassStyle::Kick.native_kind(), 5);
        assert_eq!(PassStyle::Normal.native_kind(), 2);
    }

    #[test]
    fn the_flight_is_the_distance_over_the_pass_speed_and_never_under_20_updates() {
        // DAT_004b99c0 = 5280 / (60 × 3600) × 30.762, DAT_004b99c4 = 20.
        assert!((PASS_SPEED - 0.751_96).abs() < 1e-5, "{PASS_SPEED}");
        assert_eq!(flight_updates(0.0), 20);
        assert_eq!(flight_updates(15.0), 20);
        assert_eq!(flight_updates(15.1), 20);
        // Truncated, not rounded: 39.9 is 39, 55.85 is 55.
        assert_eq!(flight_updates(30.0), 39);
        assert_eq!(flight_updates(42.0), 55);
    }

    /// A person's pass to a teammate standing `gap` ahead, through
    /// `World::step`: the update of the release, the flight the ball took,
    /// the passer's and receiver's places as that update began, and the
    /// updates from the release to the catch.
    fn pass_to_standing(gap: f32) -> (i32, [f32; 2], [f32; 2], u64) {
        let mut w = crate::World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, [-gap * 0.5, 0.0], std::f32::consts::FRAC_PI_2),
                crate::Player::new(0, [gap * 0.5, 0.0], -std::f32::consts::FRAC_PI_2),
            ],
            crate::Rules::default(),
            3,
        );
        w.phase = crate::Phase::Live;
        w.take_ball(0);
        let press = [Input::default().with(crate::button::PASS), Input::default()];
        let idle = [Input::default(); 2];
        let mut released = None;
        for k in 0..240_u64 {
            let start = w.floor_positions();
            w.step(if k == 0 { &press } else { &idle });
            if released.is_none() && w.ball.state == BallState::Pass {
                released = Some((k, w.ball.flight_ticks(), start));
            }
            if let Some((at, flight, start)) = &released
                && w.events
                    .iter()
                    .any(|e| matches!(e, Event::Catch { player: 1 }))
            {
                return (*flight, start[0], start[1], k - at);
            }
        }
        panic!("no catch");
    }

    #[test]
    fn a_pass_flies_the_originals_count_of_updates_for_its_distance() {
        // A receiver standing still is led (the aim's 0/0 fails its test):
        // the flight is timed on the passer-to-receiver distance as the
        // release update began, at least 20 updates, and the ball arrives
        // on its last update, where he takes it.
        for gap in [6.0, 14.0, 22.0, 31.5] {
            let (flight, from, to, caught) = pass_to_standing(gap);
            assert_eq!(flight, flight_updates(distance(from, to)), "gap {gap}");
            assert_eq!(caught + 1, flight as u64, "gap {gap}");
        }
        assert_eq!(pass_to_standing(6.0).0, 20);
        assert_eq!(pass_to_standing(31.5).0, 41);
    }

    #[test]
    fn the_first_pass_after_the_opening_reset_flies_40_updates_and_only_it() {
        let mut w = crate::World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, [-3.0, 0.0], std::f32::consts::FRAC_PI_2),
                // Facing the passer, as the tip's receiver stands facing the
                // centre: the two first-pass rows (`catch_stand_3r`/`3l`)
                // take only a ball coming at his face (sector 4, 0x10).
                crate::Player::new(0, [5.0, 0.0], -std::f32::consts::FRAC_PI_2),
            ],
            crate::Rules::default(),
            3,
        );
        w.tip.first_pass = true;
        w.take_ball(0);
        w.throw(0, 1, PassStyle::Normal);
        assert_eq!(w.ball.flight_ticks(), FIRST_PASS_FLIGHT as i32);
        assert!(!w.tip.first_pass);
        // Its catch row is one of the two the flag admits (0x20000).
        let row = w.ball.pass.unwrap().catch;
        let flags = crate::timing::CATCH_ROWS[usize::from(row)].flags;
        assert_ne!(flags & 0x20000, 0, "row {row}");
        assert_eq!(w.players[1].received_flight, 40);
        w.take_ball(0);
        w.throw(0, 1, PassStyle::Normal);
        assert_eq!(w.ball.flight_ticks(), 20);
    }

    #[test]
    fn a_receiver_running_to_a_near_spot_is_passed_to_the_spot() {
        // FUN_0015f930: the spot is the receiver's front behaviour's
        // destination; reached within 23 updates, with the passer farther
        // from it than he is, the pass goes to the spot and is timed on
        // the passer's distance to it.
        let mut w = crate::World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, [-10.0, 0.0], std::f32::consts::FRAC_PI_2),
                crate::Player::new(0, [10.0, 0.0], 0.0),
            ],
            crate::Rules::default(),
            3,
        );
        w.players[1].speed = 18.0;
        let spot = [10.0, 4.0];
        let mut aim = PassAim {
            spot,
            lead: false,
            reach: distance(spot, w.players[1].position) / (18.0 / 60.0),
        };
        let start = w.floor_positions();
        let f = w.release_flight(0, 1, &mut aim, &start);
        assert!(!aim.lead);
        assert_eq!(f as i32, flight_updates(distance([-10.0, 0.0], spot)));
        // Facing away from the spot (more than 50 of 1024 off), he is led.
        w.players[1].set_heading(std::f32::consts::PI);
        let mut aim = PassAim { lead: false, ..aim };
        w.release_flight(0, 1, &mut aim, &start);
        assert!(aim.lead);
        // A receiver standing still is led from the press.
        w.players[1].speed = 0.0;
        assert!(w.pass_aim(0, 1).lead);
    }

    #[test]
    fn a_led_receiver_is_met_where_his_run_takes_him_turning_along_an_edge() {
        let mut w = crate::World::new(
            crate::tests::hoops(),
            vec![
                crate::Player::new(0, [0.0, 0.0], 0.0),
                crate::Player::new(0, [10.0, 0.0], std::f32::consts::FRAC_PI_2),
            ],
            crate::Rules::default(),
            3,
        );
        // 0.3 an update toward +x: 30 updates on, 9 further.
        w.players[1].speed = 18.0;
        let at = w.lead_spot(1, 30);
        assert!((at[0] - 19.0).abs() < 1e-3 && at[1].abs() < 1e-3, "{at:?}");
        // Heading for the side line at z 25 (FUN_001636c0): he reaches it and
        // runs along it at his speed for the rest of the time.
        w.players[1].position = [10.0, 20.0];
        w.players[1].set_heading(std::f32::consts::FRAC_PI_4);
        let v = w.players[1].velocity().map(|c| c / 60.0);
        let at = w.lead_spot(1, 40);
        let t = (25.0 - 20.0) / v[1];
        let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
        assert_eq!(at[1], 25.0);
        assert!(
            (at[0] - (10.0 + t * v[0] + (40.0 - t) * speed)).abs() < 1e-3,
            "{at:?}"
        );
    }
}
