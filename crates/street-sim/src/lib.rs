//! The game simulation: everything that decides what happens on the court,
//! separate from how it is drawn.
//!
//! The world advances in fixed ticks of [`TICK_SECONDS`] from per-player
//! [`Input`]s and nothing else, so the same inputs give the same game. That is
//! what online play will build on: peers (or a server) only need to agree on
//! the inputs. Randomness (shot outcomes, steals, rebounds) comes from a seeded
//! generator inside the [`World`], and the computer players ([`ai`]) are part
//! of the world: an input marked as the computer's is replaced by the pad the
//! AI builds inside [`World::step`], so every peer computes the same pads.
//!
//! The game is the original game's: three on three, full court, played to 21
//! and won by two, 1 point inside the long-range arc and 2 beyond it, with a
//! shot clock.
//!
//! Coordinates are the original's own frame: the court's, in the game's
//! units (the rim is 11.75 up, so a unit is about 0.85 ft), y up, the long
//! axis along x. Team 0 attacks the +x basket (hoop 0), which is on the
//! left of the game's sideline camera, and team 1 the -x basket (hoop 1),
//! as the original's side bytes have it (game+0x98: +1 for team 0, -1 for
//! team 1; [`restart::side`]). Positions, sides and the original's angle
//! words agree with captures of the original without any mirroring
//! (docs/TRACE.md "The court's frame").

pub mod ai;
pub mod backdown;
pub mod ball;
pub mod block;
pub mod bracket;
pub mod cab;
pub mod combo;
pub mod contact;
pub mod control;
pub mod counter;
pub mod disc;
pub mod dive;
pub mod dribble;
pub mod gait_select;
pub mod game_class;
pub mod game_phase;
pub mod gamebreaker;
pub mod guard;
pub mod held;
pub mod input;
pub mod jump;
pub mod leap;
pub mod look;
pub mod math;
pub mod meter;
pub mod ordinary;
mod passing;
pub mod passout;
pub mod player;
pub mod pose;
pub mod putback;
pub mod rebound;
pub mod receiver;
pub mod records;
pub mod replay;
pub mod restart;
pub mod rng;
mod rules;
pub mod school;
pub mod scoring;
pub mod self_toss;
mod shooting;
pub mod shot_clock;
mod snapshot;
pub mod stand;
pub mod stats;
pub mod story;
pub mod street;
pub mod swipe;
pub mod timing;
mod timing_tables;
pub mod touch;
pub mod trick;
pub mod turbo;
pub mod validation;

pub use ball::{Ball, BallState, Pass, Shot};
pub use combo::{Chain, Slot};
pub use input::{
    ButtonEdges, GaitFamily, Input, button, combine_input_intensity, compute_physical_motion, dpad,
    dpad_magnitude, evaluate_gait_family_hysteresis, gait_query_mask, native_cos_1024,
    native_sin_1024, native_velocity_step,
};
pub use passing::{
    FIRST_PASS_FLIGHT, INBOUND_RELEASE, PASS_MIN_FLIGHT, PASS_SPEED, PassAim, PassStyle, Windup,
    flight_updates,
};
pub use player::{Action, Gait, NativeTricks, Player, Ratings, TrickIdentity};
pub use rng::Rng;
pub use rules::{CLOCKS, Difficulty, Goal, Mode, Rules, Scoring};
pub use shooting::{
    NATIVE_AIM_HEIGHT, NATIVE_BACKBOARD_BOUNDS_X, NATIVE_BACKBOARD_BOUNDS_Y,
    NATIVE_BACKBOARD_BOUNDS_Z, NATIVE_NET_BOTTOM_HEIGHT, NATIVE_NET_CONE_SCALE,
    NATIVE_RIM_COLLAR_X, NATIVE_RIM_COLLAR_Z_MAX, NATIVE_RIM_ENTRY_CONE_ANGLE, NATIVE_RIM_HEIGHT,
    NATIVE_RIM_OUTER_RADIUS, NATIVE_RIM_RADIUS, evaluate_shot_quality_tier,
};
pub use street::{Basket, Combo, TeamState};

/// Simulation rate: 60 ticks a second, the PS2 game's frame rate.
pub const TICK_HZ: u32 = 60;
pub const TICK_SECONDS: f32 = 1.0 / TICK_HZ as f32;

/// Gameplay tuning. Placeholder values chosen to look and play right with the
/// game's animations; the original game's numbers are not traced yet. Court
/// measurements are Rucker Park's painted lines.
pub mod tuning {
    /// Top speeds for an average player, units per second; the speed rating
    /// scales them by 0.9..1.1.
    pub const WALK_SPEED: f32 = 6.0;
    pub const RUN_SPEED: f32 = 13.0;
    pub const TURBO_SPEED: f32 = 17.0;
    /// Stick deflection from which the player runs rather than walks.
    pub const RUN_THRESHOLD: f32 = 0.6;
    /// How fast speed follows the stick, units per second per second, in
    /// the rewrite's own walk while a pass winds up or a swipe reaches
    /// without the disc's animations; the original's walk takes its
    /// record's speed at once ([`crate::guard`]). That walk slows by
    /// `DECELERATION`; the original's stand path zeroes the velocity at
    /// once and the stand script's planted foot carries him on
    /// ([`World::stand_tick`]).
    pub const ACCELERATION: f32 = 60.0;
    pub const DECELERATION: f32 = 45.0;
    /// Turning rate in that walk, radians per second; the original's
    /// facing turns 46 of 1024 an update ([`crate::guard::TURN`]).
    pub const TURN_RATE: f32 = 12.0;

    /// Where players can go: as far as the original's restart spots
    /// reach, 47 behind an end line and 31.5 beyond a sideline, each
    /// pushed out up to a foot more for a tall inbounder (`0x3cea20`,
    /// `FUN_0012b8b8`), so an inbounder can walk to his spot (Rucker
    /// Park's floor is Â±47.9 by Â±32.6; its painted court Â±42.3 by Â±27.4).
    /// The limit itself is the rewrite's: the original's actors are not
    /// held to a box. Streetball has no out of bounds, so players can
    /// chase a loose ball to the fence.
    pub const COURT_HALF_LENGTH: f32 = 48.0;
    pub const COURT_HALF_WIDTH: f32 = 32.5;
    /// Native court boundaries for ball containment from FUN_0014d1f8:
    /// X clamped at Â±42.0, Z clamped at Â±26.5 with outward velocity zeroed.
    pub const BALL_HALF_LENGTH: f32 = 42.0;
    pub const BALL_HALF_WIDTH: f32 = 26.5;
    /// The long-range arc: this far from the rim's spot on the floor, and
    /// straight lines at this distance from the long axis in the corners.
    pub const ARC_RADIUS: f32 = 24.5;
    pub const ARC_CORNER: f32 = 22.4;

    /// A jump shot's old stopwatch (before the script clock): the
    /// shooting hand peaked about 27 frames (0.9 s) in, 7.6 up; off the
    /// floor at `SHOT_TAKEOFF`, highest at the release, down at
    /// `SHOT_LANDING`. The display plays the shot script at its own rate
    /// (`street-game`'s `channel`).
    pub const SHOT_TAKEOFF: f32 = 0.5;
    pub const SHOT_RELEASE: f32 = 0.9;
    pub const SHOT_LANDING: f32 = 1.3;
    /// The shooters', dunkers' and lay-up jumpers' flights are their
    /// scripts' ([`crate::jump`]). A dunk with no row (the putback, which
    /// nothing starts while `FUN_00169c98` is not ported) rises
    /// `PUTBACK_RISE` updates straight up to the rim.
    pub const PUTBACK_RISE: f32 = 12.0;
    /// A scripted move past its last opcode still in the air comes down by
    /// itself this many updates on (the rewrite's guard; the scripts' own
    /// landing waits bring every one down first).
    pub const LANDING_LIMIT: i32 = 120;
    /// Height of the shooting hand at the release, before the jump.
    pub const RELEASE_HAND_HEIGHT: f32 = 7.6;
    /// Chance of a make for an average shooter, open: `MAKE_CLOSE` at the rim,
    /// falling by `MAKE_FALLOFF` per unit, never below `MAKE_FLOOR`.
    pub const MAKE_CLOSE: f32 = 0.9;
    pub const MAKE_FALLOFF: f32 = 0.02;
    pub const MAKE_FLOOR: f32 = 0.2;

    /// Dunks (with turbo held, where `World::dunk_on` has one on): the
    /// player flies to the rim and slams `DUNK_SLAM` seconds in.
    pub const DUNK_SLAM: f32 = 0.65;
    pub const DUNK_HEIGHT: f32 = 4.5;
    /// The putback's clip length on screen (the sim's own clock; the
    /// scripted dunks play their script's).
    pub const PUTBACK_TIME: f32 = 1.1;

    /// Passes fly at this speed, a little above chest height.
    pub const PASS_SPEED: f32 = 42.0;
    pub const PASS_HEIGHT: f32 = 4.8;
    /// The passer is busy this long.
    pub const PASS_TIME: f32 = 0.35;
    /// The receiver catches a pass within this distance of where it arrives.
    pub const CATCH_RANGE: f32 = 3.5;
    /// The rewrite's stand-in for the limbs a pass meets, used only
    /// without the disc's animations ([`crate::pose`]): a defender this
    /// close to a pass in its last quarter takes it.
    pub const INTERCEPT_RANGE: f32 = 1.8;

    /// Steals without the disc's animations (the swipe with them is
    /// [`crate::swipe`]: its script's span, its hand posed and tested every
    /// update, within 1.5 standing or 0.75 moving, `0x16d160..0x16d1bc`).
    /// The stand-in: the swipe lasts `STEAL_TIME`, 32 updates in 66 of the
    /// 115 swipes of the first attract capture (27..42), and tests once,
    /// `SWIPE_AT` into the swipe (update 13, the mode of the captures' nine
    /// strips, 1..14), a hand `HAND_REACH` ahead of the defender at the
    /// carried ball's height, within `HAND_RADIUS` (the standing distance)
    /// of the ball: the two captures' 126 swipes on a held ball had it
    /// there at update 13 in 9.5% (with the odds about 5% strips, against
    /// 7.1% seen).
    pub const STEAL_TIME: f32 = 32.0 / 60.0;
    pub const SWIPE_AT: f32 = 12.5 / 60.0;
    pub const HAND_REACH: f32 = 4.0;
    pub const HAND_RADIUS: f32 = 1.5;

    /// Jumps to block or rebound: `JUMP_HEIGHT` at `JUMP_PEAK`, down at `JUMP_TIME`.
    pub const JUMP_TIME: f32 = 0.8;
    pub const JUMP_PEAK: f32 = 0.4;
    pub const JUMP_HEIGHT: f32 = 3.0;
    /// A jumping defender this close to a shooter may block the shot.
    pub const BLOCK_RANGE: f32 = 4.5;
    pub const BLOCK_BASE: f32 = 0.2;
    pub const BLOCK_SKILL: f32 = 0.5;

    /// Loose balls: anyone in reach picks up a ball below `PICKUP_HEIGHT`; a
    /// jumping player catches it up to their reach plus the jump.
    pub const PICKUP_RANGE: f32 = 2.2;
    pub const PICKUP_HEIGHT: f32 = 5.5;

    /// Seconds between a basket and the inbound in a game without the
    /// original's six players (the rewrite's own restart: the nearest
    /// player is put on the baseline with the ball).
    pub const DEAD_BALL_TIME: f32 = 1.8;
    /// The six-player restart ends when all six are ready and the ball
    /// has been passed in (`FUN_0013e4c8`, phase 6); the original has no
    /// limit. The rewrite puts the ball in play as above if one runs this
    /// long, so a restart can never stall a game.
    pub const RESTART_LIMIT: f32 = 20.0;

    /// The turbo bar (0 to 1) empties in five seconds of held turbo and
    /// refills in seven and a half. The static initializer at `0x178860`
    /// stores `1 / (fps Ã— 5.0)` at `0x4b99a0` (`0x178a64..0x178ad0`) and
    /// `1 / (fps Ã— 7.5)` at `0x4b99a4` (`0x178ad4..0x178aec`); the actor
    /// update spends the first each frame a turbo bit (0x2000) is held in
    /// live play (`FUN_00190c08` at `0x1589a0`) and adds the second
    /// otherwise (`FUN_00190c68` at `0x1589c8`). (`FUN_00147cf8`'s
    /// `0x4b7e80`/`0x4b7e90`, the earlier reading, are 0.0 and 1.0 words of
    /// a static vector block, not turbo rates.)
    pub const TURBO_DRAIN_SECONDS: f32 = 5.0;
    pub const TURBO_REFILL_SECONDS: f32 = 7.5;
    /// The per-update amounts, divided as the original does.
    pub const TURBO_DRAIN: f32 = 1.0 / (crate::TICK_HZ as f32 * TURBO_DRAIN_SECONDS);
    pub const TURBO_REFILL: f32 = 1.0 / (crate::TICK_HZ as f32 * TURBO_REFILL_SECONDS);
    // A dunk costs nothing at its press (`FUN_0016b4b8`, `FUN_001693b8`
    // read the turbo buttons, not the bar; the third attract capture's
    // bars step at none of its 19 dunks). A trick move's turbo is the juke
    // press's ([`crate::turbo::MOVE`]); its length, fakeout and the
    // defender's reaction are the original's scripts and odds
    // (`crate::trick`).

    /// A passout special asks for and spends 0.2 of the bar
    /// (`FUN_00146278`, `0x1469d4..0x1469f8` and `0x14741c`).
    pub const TRICK_PASS_TURBO: f32 = f32::from_bits(0x3e4c_cccd);

    /// Defenders contest a fadeaway this much less (its drift is its
    /// script's `a1 f0`).
    pub const FADE_CONTEST: f32 = 0.5;
    /// The turbo block: a higher jump for this much turbo; it can goaltend a
    /// ball coming down within `GOALTEND_REACH`.
    pub const TURBO_JUMP_HEIGHT: f32 = 3.8;
    pub const TURBO_BLOCK_TURBO: f32 = 0.15;
    pub const GOALTEND_REACH: f32 = 2.5;
    /// How far a diver reaches for the ball without the disc's animations
    /// (the rewrite's stand-in for the dive's catch, [`crate::dive`]).
    pub const DIVE_PICKUP: f32 = 3.2;
}

/// Native ball gravity: `0014a3a4` uses 0x42009be3 times the default
/// scalar installed by `00153ddc..00153e24` (minus 0xbfa13ba4).
/// The original stores acceleration per frame, dividing this by 60Â².
pub const GRAVITY: f32 = f32::from_bits(0x4200_9be3) * -f32::from_bits(0xbfa1_3ba4);

/// A basket, as the simulation needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hoop {
    /// Centre of the rim.
    pub rim: [f32; 3],
    pub rim_radius: f32,
}

impl Hoop {
    /// The original's two rims, the one each team attacks: the ring's
    /// centre at x = side Ã— 36.75 (`FUN_001404e0`; 1.25 ft in from the
    /// board at 38), +36.75 for team 0 and -36.75 for team 1
    /// ([`restart::side`]), on the rim plane 11.91355 (`FUN_001515a8`,
    /// `FUN_001526d0`).
    pub fn native() -> [Hoop; 2] {
        [0, 1].map(restart::side).map(|side| Hoop {
            rim: [side * 36.75, crate::shooting::NATIVE_RIM_HEIGHT, 0.0],
            rim_radius: 0.75,
        })
    }
}

/// Where the game is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Play is on.
    Live,
    /// After a basket or a violation: nobody may touch the ball until `left`
    /// runs out, then `team` inbounds it.
    DeadBall {
        left: f32,
        team: usize,
    },
    Over {
        winner: usize,
    },
}

/// Something that happened during a tick, for sound, effects and the HUD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot {
        player: usize,
        hoop: usize,
        points: u32,
    },
    Dunk {
        player: usize,
        hoop: usize,
    },
    Score {
        team: usize,
        player: usize,
        points: u32,
        basket: Basket,
    },
    RimOut,
    Block {
        player: usize,
        shooter: usize,
    },
    Pass {
        from: usize,
        to: usize,
        style: PassStyle,
    },
    Catch {
        player: usize,
    },
    Steal {
        player: usize,
        from: usize,
        /// The possession came from an airborne pass rather than a held-ball swipe.
        intercepted_pass: bool,
    },
    /// Any free-ball recovery, including a block caught in the air or an
    /// inbound pickup. Only [`Event::Rebound`] posts the rebound's slot 17.
    Pickup {
        player: usize,
    },
    /// The take posted slot 17 (`FUN_0014cb28`): a live ball in native
    /// state 0 or 6. Ordinary pickups do not pay or announce a rebound.
    Rebound {
        player: usize,
    },
    ShotClockViolation {
        team: usize,
    },
    Inbound {
        player: usize,
    },
    GameOver {
        winner: usize,
    },
    /// A trick move scored at its script's end; `fakeout` is the
    /// defender it fooled, `level` its value class plus one, `script` its
    /// script (actor+0x264, [`trick::SCRIPTS`]).
    Trick {
        player: usize,
        level: u8,
        script: u16,
        fakeout: Option<usize>,
        /// The trick points it was worth (team `+0x24`, the HUD's pop-up).
        points: u32,
    },
    /// The handler lost the ball mid-trick. No longer sent: a trick alone
    /// never loses the ball in the original (`World::score_trick`); kept
    /// for the snapshot's and the listeners' sake.
    Fumble {
        player: usize,
    },
    /// A limb knocked a pass loose (`FUN_00137ed8` from the actor's
    /// update).
    Deflection {
        player: usize,
    },
    /// A trick counter took the ball from `from`.
    Counter {
        player: usize,
        from: usize,
    },
    /// A scoring possession's combo paid out: `points` in all, at `multiplier`.
    Combo {
        team: usize,
        points: u32,
        multiplier: u32,
        /// What the HUD reads from the ring as it pays (`FUN_002b87c0`): the
        /// classes of the slots the matched row walked, oldest first, âˆ’1 for
        /// slots the row left out.
        walked: [i8; 6],
        /// The payout (`game+0x150`).
        paid: u32,
        /// The newest slot's value (`game+0x88`), the paying event's own.
        newest: i32,
    },
    GamebreakerReady {
        team: usize,
    },
    /// A full meter's 30 seconds ran out and it emptied, with any pocketed
    /// gamebreaker ("meter drops", slot 67, from `FUN_00143c20`).
    MeterDropped {
        team: usize,
    },
    Pocket {
        team: usize,
    },
    Cancel {
        team: usize,
    },
    /// A gamebreaker shot went up.
    Gamebreaker {
        player: usize,
        level: u8,
    },
    /// One newly applied staged frame; freeze ticks do not repeat it.
    GamebreakerFrame {
        script: usize,
        frame: u32,
        mirrored: bool,
    },
    Layup {
        player: usize,
    },
    PumpFake {
        player: usize,
    },
    /// A defender jumped at a pump fake: `FUN_0013f950`'s slot 73, the
    /// post the Pump Fake lesson passes on (`FUN_0018e990`).
    PumpFakeBite {
        player: usize,
    },
    /// A turbo block swatted a shot on its way down.
    Goaltend {
        player: usize,
        shooter: usize,
    },
    Dive {
        player: usize,
    },
    /// A screen on the holder's own defender by the arc (`0x3c6910`'s
    /// kind 3, [`World::post_contact`]'s 500): `defender` was screened.
    Pick {
        screener: usize,
        defender: usize,
    },
    /// The steal button without the ball: a reach for it, or with `counter`
    /// the trick-counter stance (the game's swipe and counter-stance
    /// scripts, `0x15da28`).
    Reach {
        player: usize,
        counter: bool,
    },
    /// A jump began. `block` unless the ball is loose: the game takes its
    /// block jumps (`0x15d3a0`) while the ball is held or in the air, its
    /// rebound jumps (`0x15ce50`) for a loose ball.
    Jump {
        player: usize,
        block: bool,
    },
    /// A dunker lets go of the ball at the rim, in or not (the game's shot
    /// release `0x14aaf8` for a dunk or an alley-oop dunk).
    Slam {
        player: usize,
        hoop: usize,
    },
    /// The replay director's draw at a basket (`FUN_003248d8`, raw
    /// `0x324bac`): one value of the shared generator (`FUN_003a16b8`)
    /// whose `% 100` under the basket's chance plays the instant replay.
    /// It follows the [`Event::Score`] it was drawn for.
    ReplayRoll {
        draw: u32,
    },
    /// A staged Level 2 gamebreaker's script reached the end of its replay
    /// command's span (`FUN_003355c8` dispatches category 1 opcode 0 at its
    /// frame plus its span, `FUN_00334b30`): the script manager replays the
    /// span on the command's camera (its state 5, `FUN_00336188`) and asks
    /// the replay manager for it (`crate::replay`, mode 7). The span's
    /// `frames`, `from` its first frame; `speed` in percent.
    StagedRerun {
        script: usize,
        mirrored: bool,
        from: u32,
        frames: u32,
        speed: u8,
        camera: u8,
    },
}

/// The receiver of a caught pass and the tick his catch animation ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catch {
    pub player: usize,
    pub until: u64,
    /// The catch row whose script holds him ([`timing::CATCH_ROWS`]).
    pub row: u8,
    /// The pass's nominal flight in updates (`ball+0x64`), which the
    /// script's flight wait lasts the rest of.
    pub flight: u8,
    /// The update the pass was released in, when the script started.
    pub since: u64,
}

/// [`World::update_order`] of a world made by [`World::new`]: the
/// rewrite's own, the highest index first.
pub const HIGHEST_FIRST: [u8; 6] = [5, 4, 3, 2, 1, 0];

/// Which of the construction's draws ([`CONSTRUCTION_DRAWS`], counted from
/// one) place actors 1 to 5 in the update order (`FUN_0033cf98`, actor 0
/// going in first without one): save state 1's match comes out of the
/// draws 26, 29, 32, 35 and 38 after its reseed with three draws to each
/// actor's construction. Not confirmed: the draws sit elsewhere in other
/// matches (save states 4 and 13 fit no fixed spacing), so a given game's
/// order is the original's only in kind, each of the 720 as likely
/// (docs/TRACE.md "The actors' update order").
pub const ORDER_DRAWS: [u32; 5] = [26, 29, 32, 35, 38];

/// [`World::stage_bounds`] of a world made by [`World::new`]: the
/// rewrite's own pair for the two draws of `FUN_0012e798` (`rand() % 6`,
/// 0..5, and `15 + rand() % 3`, 15..17), near their means. The original
/// draws a pair every game, the shared generator's two outputs when the
/// game is built (ten captured games hold nine pairs: docs/TRACE.md "Held
/// side fades and the stage bounds across games"); a game built on the
/// session's generator ([`World::from_generator`]) draws them so. A world
/// from a bare seed keeps this pair: its generator was never run, and the
/// first two draws of [`rng::Rng::new`] are 5 and 16 for every seed below
/// a wrap (docs/TRACE.md "The session's generator").
pub const STAGE_BOUNDS: [u32; 2] = [2, 16];

/// The draws the original makes between the last reseed of the shared
/// generator (`FUN_001dbe30`, a team set for a side no person holds) and
/// `FUN_0012e798`'s pair, which [`World::from_generator`] makes and
/// discards before the pair: 46 in both captured first games after a
/// reseed, the fresh boot's Pick Up game (Choose Team's commit, then
/// Choose Players, Uptown, My Rules and the loading) and the first attract
/// match (`FUN_001db7f8`'s team picks, then straight to the loading). The
/// two front ends differ, so the draws belong to the loading and the
/// game's construction (`FUN_002f3000`, the base constructor
/// `FUN_0013c4e8`); which calls make them is not traced (the crowd's
/// spectators `FUN_0030b9b8` and the actors' `FUN_001563a8`/`FUN_00156ab0`
/// reach the generator), nor whether every court and mode makes the same
/// number (the second attract capture's update 0 came 30 draws after its
/// reseed, mid-construction, and its update 1 8,763, so its count is not
/// read). The rewrite's own count for every game, from those two.
pub const CONSTRUCTION_DRAWS: u32 = 46;

/// [`World::side_teams`] for a side whose team the game doesn't name.
pub const NO_TEAM: u16 = u16::MAX;

/// The updates every actor is barred after a basket (`FUN_00141818`,
/// `0x141970`: actor+0x1a8 = 0x28).
pub const BASKET_BAR: u16 = 40;
/// Team Whitewater (database team 44, `0x2c`), the boss of the tournament
/// at event 5: the type the points, lesson and trick-point games' pocket
/// and cancel checks test (`FUN_0012ebf0`, `FUN_0012ed28`, `FUN_0018c8e0`,
/// `FUN_0018ca20`).
pub const WHITEWATER: u16 = 0x2c;

#[derive(Clone, Debug, PartialEq)]
/// The game: the players, the ball, the score and the dice. The same inputs
/// and seed give the same game (see the module docs) â€” which is what a
/// replay is: restore an older snapshot and re-step it with the recorded
/// inputs ([`World`] is [`Clone`] exactly for that).
pub struct World {
    pub tick: u64,
    /// The baskets by the team that attacks each ([`target_hoop`]): the
    /// +x one first.
    pub hoops: [Hoop; 2],
    pub players: Vec<Player>,
    pub ball: Ball,
    pub rules: Rules,
    /// Points per team.
    pub score: [u32; 2],
    /// game+0x4b0/+0x4b4 of a points game: the bounds of the catch-up
    /// tables' three stages ([`shooting::catch_up_cell`]). The original's
    /// constructor draws them once a game from the shared generator
    /// (`FUN_0012e750` -> `FUN_0012e798`: `rand() % 6`, then
    /// `15 + rand() % 3`), whose state then carries the boot, the front
    /// end and the earlier games' draws. A game built on the session's
    /// generator ([`World::from_generator`]) draws them so; one from a bare
    /// seed ([`World::new`]) keeps [`STAGE_BOUNDS`], the rewrite's own
    /// (docs/TRACE.md "The session's generator"). A replay of a captured
    /// match sets them. The trick-point and Gamebreaker Wins games derive
    /// theirs from the target (`FUN_0018c5c0`, `FUN_001457f8`) and do not
    /// read these.
    pub stage_bounds: [u32; 2],
    /// The order the six actors update in each frame, first to last: the
    /// process registry's (`FUN_0033ce00` runs it front to back), where
    /// `FUN_0033cf98` puts each AIPhysPlayer the AITeams build at a random
    /// place among those already in, `rand() % (k + 1)` for the k in so
    /// far. A game built on the session's generator
    /// ([`World::from_generator`]) draws it so; one from a bare seed
    /// ([`World::new`]) keeps [`HIGHEST_FIRST`]. Each computer actor's AI
    /// frame runs in his update, so what one reads of another (a shot let
    /// go, a lob's leaper, a spot pass's receiver) is that update's for an
    /// actor earlier in the order and the last update's for one later.
    pub update_order: [u8; 6],
    /// Trick points, combos and gamebreakers per team.
    pub teams: [TeamState; 2],
    /// The team on offense: holding the ball, or the last to have held it.
    pub possession: usize,
    /// Seconds left to shoot.
    pub shot_clock: f32,
    pub phase: Phase,
    /// The original's game phase and the fields its transitions keep
    /// (game+0xf4, +0x110, +0x440, +0x104, +0x448; [`game_phase`]), which
    /// the computer players and the restarts read. [`World::phase`] moves
    /// with it: its phase 6 is [`Phase::DeadBall`], its 9 follows
    /// [`Phase::Over`].
    pub game_phase: game_phase::GamePhase,
    /// What happened in the last tick.
    pub events: Vec<Event>,
    /// A give 'n go in progress: (passer, receiver). The passer gets it back
    /// by calling for it.
    pub give_and_go: Option<(usize, usize)>,
    /// game+0x94: the ball has changed sides at a take (`FUN_001415c0`)
    /// and nobody has shot, passed or juked since (`0x15b204`, the pass
    /// start `0x15fa20`, the trick press `0x15f1c8`; the actors' reset
    /// `0x1569c0`). While it is set a person's shot needs the button
    /// pressed; otherwise holding it is enough ([`World::shoots`]).
    pub fresh_take: bool,
    /// A pass pressed and still in the passer's hands until its animation
    /// lets go ([`passing::Windup`]).
    pub windup: Option<passing::Windup>,
    /// A caught pass still holding its receiver in the catch animation
    /// (actor+0xa4 bit 9) until `until` ([`timing::catch_hold`]).
    pub catch: Option<Catch>,
    /// The tick each player last let go of each turbo button (L1, R1, L2,
    /// R2; `FUN_00130a28`'s controller+0xec), `u64::MAX` for none: one
    /// pressed again soon after dives ([`dive`]).
    turbo_releases: Vec<[u64; 4]>,
    rng: rng::Rng,
    previous_inputs: Vec<Input>,
    /// The disc's staged Level 2 gamebreakers, when the match has them
    /// (`street-game`'s `staged::config`). Fixed for the match.
    pub staged: Option<std::sync::Arc<gamebreaker::Config>>,
    /// The original's gait records and each player's profile, when the
    /// match has them (`street-game` reads them from the disc). Fixed for
    /// the match; without them the generic walk and run stand in.
    pub gait: Option<std::sync::Arc<gait_select::GaitConfig>>,
    /// Which variants of each staged family have played.
    pub variety: gamebreaker::Variety,
    /// A staged gamebreaker playing: the players and ball follow its
    /// script, the clocks stop, and no input counts until it ends.
    pub sequence: Option<gamebreaker::Sequence>,
    /// A staged gamebreaker asked for and not yet posed, with the updates
    /// left before it is: the gamebreaker task's first update starts its
    /// script (`FUN_00144ef8` state 0, `FUN_00335e90`), the script manager
    /// poses its frame 0 the update after (the School's Level 2 capture:
    /// two updates after the press). Play goes on meanwhile.
    pub staged_start: Option<(gamebreaker::Sequence, u8)>,
    /// The combo manager (game+0xd0, [`combo::Manager`]): the match's one
    /// ring of events and the words its dispatchers keep ([`scoring`]).
    pub combo: combo::Manager,
    /// Ball+0x10c and +0x158: the last holder a teammate's take keeps as
    /// the assister, and the held updates left before he is forgotten
    /// ([`scoring::ASSIST_UPDATES`]; `FUN_0014cb28`, `FUN_0014e060`).
    pub assist: Option<(usize, u16)>,
    /// Game+0x44c..+0x460: the block jumpers watched for a pump fake's
    /// points ([`World::step_pump_fake_watch`]).
    pub pump_fake_watch: Vec<bool>,
    /// Game+0x494: the count when the holder was last in a jump shot on
    /// the floor (0 once he is in the air).
    pub pump_fake_left: i32,
    /// Game+0x498: the updates the holder has been in his jump shot.
    pub pump_fake_shot: i32,
    /// The shot clock's words beside its time ([`shot_clock::ShotClock`]).
    pub clock: shot_clock::ShotClock,
    /// A Street School lesson's id (game+0x9c 4, the game's vt+0xb4), when
    /// the match is one: the meter stops at 49,999 outside the
    /// gamebreaker lessons ([`World::fill_meter`]).
    pub lesson: Option<u8>,
    /// `DAT_003c7af8` clear: a tutorial page was due at the lesson's
    /// launch (`FUN_0018ec28`), and the inbound waits `fps Ã— 0.5`, not
    /// `fps Ã— 2` ([`crate::ai`]'s "throw inbound").
    pub lesson_page: bool,
    /// The story modes' outer weight (the game record's mode 4, the league
    /// Challenge, or 5, Be A Legend: `FUN_00140a30` and `FUN_00143458`
    /// recompute their mix's weight from the run's progress,
    /// [`crate::meter::league_weight`], [`crate::meter::legend_weight`]); `None`
    /// in the other modes. Set with the meters' fill
    /// ([`World::configure_meter_fill`]).
    pub scenario: Option<f32>,
    /// The database team each side plays as ([`NO_TEAM`] when the game
    /// doesn't say): game+0x184 by side, its team record (+0x40) and the
    /// record's index across the team tables (`FUN_001ecdb8`, vt+0x14 of
    /// `0x461030`). A story game's career is its own record (49 in save
    /// state 4's), its crews the generic ballers' (36). Only Team
    /// Whitewater's ([`WHITEWATER`]) is read: the points, lesson and
    /// trick-point games' pocket checks never pocket for it and their
    /// cancel checks always cancel ([`crate::ai`]).
    pub side_teams: [u16; 2],
    /// The referee's toss that opens a match ([`World::start_opening_toss`]).
    pub opening_toss: OpeningToss,
    /// The jumpers' contest for the toss ([`Tip`]).
    pub tip: Tip,
    /// The dead ball is the rewrite's timed one ([`World::stage_dead_ball`],
    /// the lessons' entry): nobody walks to a restart spot.
    pub timed_dead_ball: bool,
    /// The game listener's +4: the last basket's scorer (`FUN_00141818`
    /// outside phase 9), whom the hold and the win celebrate.
    pub scorer: Option<usize>,
    /// game+0x10c: the win's celebration waits for the scorer to leave
    /// his dunk or leap ([`World::won_celebration`]).
    pub celebrate_pending: bool,
    /// The C library's generator (`FUN_003b42a0`'s word) as the stand
    /// scripts' callback 3 steps it ([`crate::stand`]). The original's is
    /// one with the presentation's and the speech's; the simulation keeps
    /// its own.
    pub libc: u32,
    /// The save's Replays option as the replay director reads it at a
    /// basket (`+0xc42f0`: 0 Always, 1 Automatic, 2 Manual), which decides
    /// whether its roll draws from the shared generator
    /// ([`Event::ReplayRoll`]). The game sets it each update; online peers
    /// share it.
    pub replays: u8,
    /// The contact manager's pushing counts ([`contact::Contacts`]).
    pub contacts: contact::Contacts,
    /// The computer players' state ([`ai`]).
    pub ai: ai::State,
    /// Game+0x4a4: the meters run ([`street::MeterClock`]).
    pub meter_clock: street::MeterClock,
    /// The match's statistics ([`stats::Stats`]).
    pub stats: stats::Stats,
    /// The disc's animations, for the rules that read bones ([`pose`]):
    /// not game state ([`pose::Bank`]).
    pub poses: pose::Bank,
    /// `0x3d75bc`: the last trick script the draw picked, which the next
    /// draw leaves out ([`World::start_trick`]); 255 for none yet.
    pub last_trick: u8,
    /// The receiver's catch row playing while a pass flies
    /// ([`touch::Catching`]), with the disc's animations.
    pub catching: Option<touch::Catching>,
    /// `DAT_003d70b4` and its spot: the pass's receiver runs to the spot
    /// and stops there ([`receiver::SpotRun`]).
    pub spot_run: receiver::SpotRun,
    /// The passout identity through its catch, with the board route's
    /// row and return leg ([`passout::Passout`], game+0x140..+0x160).
    pub passout: Option<passout::Passout>,
    /// The ball's offset its holder's blend stored (channel +0xa0 while
    /// +0xb0): the held ball eases from it to the new clip's hand over the
    /// blend ([`held`]).
    pub held_anchor: Option<held::Anchor>,
    /// Game+0xb0: the six turbo bars, a computer side's three sharing one
    /// ([`turbo`]).
    pub turbo: [f32; turbo::SLOTS],
    /// Actor+0x2ac by player: his update fills or drains his bar
    /// ([`World::update_turbo_owner`]).
    pub turbo_owner: Vec<bool>,
    /// The players a person controls (`FUN_00156d98` below 4): as this
    /// update's inputs had them, moved where the game moves them
    /// ([`control`]); a client drives the players it names next.
    pub people: Vec<bool>,
    /// What the game keeps of the people's control between updates
    /// ([`control::Control`]).
    pub control: control::Control,
    /// The gamebreaker task (game+0x434, [`street::GamebreakerTask`]): its
    /// end empties the meter whose gamebreaker it was.
    pub gamebreaker_task: Option<street::GamebreakerTask>,
}

/// The referee's jump ball that opens a match in the original (game phase
/// 1, `FUN_00141fd8`): the players wait at their tip-off spots while the
/// referee holds the ball, then his toss sends it up from the centre
/// circle (`FUN_0014c858`, ball state 7). The phase ends when someone has
/// the ball (`FUN_0013ee10`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpeningToss {
    /// No toss: play starts with an inbound, or the toss is over.
    #[default]
    None,
    /// The referee holds the ball; it goes up after this many more updates.
    Waiting(u16),
    /// The ball is up and nobody has caught it yet.
    Flight,
}

/// The jump ball's contest (`FUN_0015eaa0`, the actor's update while
/// the toss is up): each side's timing (`DAT_003d6578`, 1..20, -1 not
/// yet) and the winning jumper's side (`DAT_003d6570`, the winner's
/// actor index; -1 not decided). `FUN_00156870` clears both when the
/// actors are set up for a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tip {
    pub timing: [i32; 2],
    pub winner: i32,
    /// Each side's jumper has started his jump (the script lock, the
    /// animation's flag 2, keeps it from starting again).
    pub jumped: [bool; 2],
    /// `DAT_003d65b8`: the actors' reset for a match (`FUN_00156870`)
    /// sets it, and the next pass's catch draw flies that pass
    /// [`FIRST_PASS_FLIGHT`] updates and clears it: the tip.
    pub first_pass: bool,
}

impl Default for Tip {
    fn default() -> Self {
        Self {
            timing: [-1; 2],
            winner: -1,
            jumped: [false; 2],
            first_pass: false,
        }
    }
}

/// The opening toss's numbers.
pub mod toss {
    /// Updates from the intro's end to the launch. The original counts
    /// `DAT_004b5270` (the frame rate, 60, written at `0x11b230`) once all
    /// six players stand ready (`FUN_0011aea0`), then the referee's toss
    /// script (`0x44cb8f`) blends over 8 frames and reaches frame 6 over 50
    /// updates before its callback 44 (`0x17d278`) launches the ball:
    /// 60 + 8 + 50. A retained attract capture puts the launch 118 updates
    /// (within 2) after the intro cut.
    pub const WAIT_UPDATES: u16 = 118;
    /// The toss's apex (`FUN_0014c858`: 20.0).
    pub const APEX: f32 = 20.0;
    /// The tip-off spots by slot (`0x3cfbb0`, `FUN_0012e418`): x, times the
    /// team's side (the direction it attacks, [`crate::restart::side`]:
    /// the jumper and the man behind him stand in the half the team
    /// defends), and z. `FUN_0018af20` stands each player there facing the
    /// centre (`FUN_00170608`).
    pub const SPOTS: [[f32; 2]; 3] = [[-3.25, 0.0], [3.25, 7.0], [-7.0, 0.0]];
    /// Where the referee's hand lets the ball go: the retained attract
    /// capture's held ball in the update before its state-7 toss (sample
    /// 583). The ball waits there for the launch (the renderer puts it in
    /// the drawn referee's hands, `street-game`'s `referee`).
    pub const RELEASE: [f32; 3] = [
        f32::from_bits(0xbea4_b79a),
        f32::from_bits(0x40cb_d176),
        f32::from_bits(0xbf48_0b87),
    ];
    /// Where the referee stands while he holds the ball: `FUN_0017cf28`
    /// stores the vector (-0.9, 0, -3.0, 1) (raw `0x17cf2c..0x17cf7c`), as
    /// the retained attract capture shows at the intro cut.
    pub const REFEREE: [f32; 3] = [-0.9, 0.0, -3.0];
    /// The toss's spin, radians per update: `DAT_004b854c`, which the ball
    /// constants' initializer writes at `0x154020` as
    /// `(1 / 60) * 2pi * 0.5` (the config's frame rate `+0x28`, `0x40c90fdb`
    /// and `0x3f000000`), half a turn a second.
    pub const SPIN: f32 = (1.0_f32 / 60.0) * f32::from_bits(0x40c9_0fdb) * 0.5;
    /// The axis a degenerate random draw spins about: `0x4b83b0`, which
    /// the same initializer fills with (0, 1, 0, 0) at `0x153c00`.
    pub const SPIN_FALLBACK: [f32; 3] = [0.0, 1.0, 0.0];
    /// The jumpers' scripts (`FUN_0015eaa0`, raw `0x15edc4..0x15ee10`):
    /// the winner's `tipoff_jump_2` (`0x457a7b`), and the loser's
    /// `tipoff_jump_1` (`0x457a12`) or `tipoff_jump_3` (`0x457ae4`) as
    /// `r & 1` is 0 or 1 (the pair at `0x43f078`).
    pub const TIP_SCRIPTS: [u32; 3] = [0x457a7b, 0x457a12, 0x457ae4];
    /// The script tick of each script's jump (`a1 ef`, handler
    /// `FUN_00194d58`): a 3-update blend and a 5-update wait come first
    /// in all three.
    pub const JUMP_TICK: u16 = 8;
    /// `a1 ef`'s first operand (25.0, times fps / 60): the updates until
    /// the hand, posed at the clip frame of its second operand
    /// ([`JUMP_FRAMES`]), meets the ball. The jump aims it there
    /// (`0x19537c..0x1956a4`): the ball's place that many updates on
    /// (`FUN_001537d0`, its own path stepped ahead) less the hand's, over
    /// the updates, plus half the actor's gravity times them upward.
    pub const JUMP_UPDATES: f32 = 25.0;
    /// `a1 ef`'s second operand per script: the clip frame the hand is
    /// posed at for the aim (`FUN_0019bda8`; bones `0x13` and `0xf`, or
    /// their midpoint, by the pose's flags). The audit read these as
    /// heights; the jumpers rise 3.75 to 4.23 in the captured openings.
    pub const JUMP_FRAMES: [f32; 3] = [15.0, 10.0, 11.0];
    /// Each script's clip, which its first opcode blends to over three
    /// updates (`8e 03 00 00 00 00 r`): `tipoff_jump_2` 380,
    /// `tipoff_jump_1` 384, `tipoff_jump_3` 382. The jump's aim poses it
    /// at [`JUMP_FRAMES`].
    pub const JUMP_RECORDS: [u16; 3] = [380, 384, 382];
    /// The clip frame each script's wait before the jump reaches over its
    /// five updates (`8d ee s 05`); the jump's next wait then runs the clip
    /// from it to [`JUMP_FRAMES`] over the [`JUMP_UPDATES`] to the meet
    /// (`8d ee F 19`).
    pub const JUMP_START_FRAMES: [f32; 3] = [4.0, 3.0, 2.0];
    /// The script tick its first blend (`8e 03`) is over.
    pub const BLEND_TICKS: u16 = 3;
    /// The actor's gravity, units per update squared (the jump's
    /// `32.152233 / fpsÂ² Ã— 1.2596`, the ball's too).
    pub const JUMP_GRAVITY: f32 =
        f32::from_bits(0x4200_9be3) / (60.0 * 60.0) * -f32::from_bits(0xbfa1_3ba4);
    /// The tip: the winner holds the ball once either hand bone is within
    /// 1.25 of it (`0x15eec4`), the ball where the last update left it,
    /// and passes at once (`FUN_0014cb08`, `FUN_0015f930(actor, 0)`). The
    /// rewrite has no bones: its hand is the aimed one, the jumper's
    /// standing reach above his feet ([`crate::Ratings::reach`]), carried
    /// rigidly on the jump that brings it to the ball.
    pub const TIP_REACH: f32 = 1.25;
    /// The person's timing window: a press of shoot with the toss above
    /// this share of its apex scores 1..20 by how near the apex
    /// (`FUN_0015eaa0`: `(y - 0.8 apex) / (0.2 apex) Ã— 20`, rounded, plus
    /// one).
    pub const TIMING_SHARE: f32 = 0.8;
}

/// The basket `team` shoots at: team 0 attacks +x (hoop 0), team 1 -x (hoop 1).
pub fn target_hoop(team: usize) -> usize {
    team.min(1)
}

/// The basket `team` defends.
pub fn defended_hoop(team: usize) -> usize {
    1 - team.min(1)
}

impl World {
    /// The catch row's tail still holds `i`, who has the ball: bit 9 after
    /// the catch, for [`timing::catch_hold`] updates, until a move of his
    /// replaces the row (the new script's `9b` replaces the state word:
    /// a computer's shot 4 updates into `catch_stand_3r`'s 16, the shot
    /// clock's violation 9 in; docs/TRACE.md "The pass captures' five
    /// checks").
    pub(crate) fn catch_holds(&self, i: usize) -> bool {
        self.ball.state == BallState::Held(i)
            && self
                .players
                .get(i)
                .is_some_and(|p| matches!(p.action, Action::None))
            && self
                .catch
                .is_some_and(|c| c.player == i && self.tick < c.until)
    }

    /// Actor `i`'s state bits (actor+0xa4) as the simulation's actions show
    /// them, the bits the computer players read (`ai/court.rs`): 3 a jump
    /// shot or a lay-up, 4 a pass's throw, 7 a dunk, 8 a rebound jump, 9 a
    /// pass's catch, 10 a trick, 12 a block jump, 13 a lay-up, 15 an
    /// alley-oop's leap, 16 the counter stance. Bits 2, 6, 11, 14, 17 and
    /// 18 (all inside the camera's mask `0x7ffdc`) are not modelled.
    pub fn actor_state_bits(&self, i: usize) -> u32 {
        let Some(p) = self.players.get(i) else {
            return 0;
        };
        let mut bits = 0;
        let mut set = |bit: u32, on: bool| {
            if on {
                bits |= 1 << bit;
            }
        };
        set(
            3,
            matches!(
                p.action,
                Action::Shooting { .. }
                    | Action::Layup { .. }
                    | Action::Dunking { layup: true, .. }
            ),
        );
        set(4, matches!(p.action, Action::Passing { .. }));
        set(7, matches!(p.action, Action::Dunking { layup: false, .. }));
        set(8, matches!(p.action, Action::ReboundJump { .. }));
        set(
            9,
            match self.ball.state {
                // A row parked for its own count sets it when its script
                // starts ([`touch::Catching::parked`]).
                BallState::Pass => {
                    self.ball.pass.is_some_and(|q| q.to == i)
                        && !self.catching.is_some_and(|c| c.player == i && c.parked)
                }
                _ => self.catch_holds(i),
            },
        );
        set(10, p.action.juking());
        set(12, matches!(p.action, Action::Jumping { .. }));
        set(
            13,
            matches!(
                p.action,
                Action::Layup { .. } | Action::Dunking { layup: true, .. }
            ),
        );
        set(15, p.leaping());
        set(
            16,
            matches!(p.action, Action::Stealing { counter: true, .. }),
        );
        bits
    }

    /// Actor `i`'s physical vector (actor+0x40) as his update leaves it,
    /// units an update (`FUN_00156f48` adds it to his place): his flight's
    /// on a shot's, dunk's, lay-up's, leap's or dive's script
    /// ([`jump::scripted`]); none on a trick's or a reaction's, whose `97`
    /// zeroes it while the clip's root carries him; else his locomotion's
    /// speed along his course (no rise on the floor).
    pub fn actor_motion(&self, i: usize) -> [f32; 3] {
        let Some(p) = self.players.get(i) else {
            return [0.0; 3];
        };
        if jump::scripted(&p.action) {
            return p.flight.velocity;
        }
        if matches!(
            p.action,
            Action::Trick { .. }
                | Action::Stumbling {
                    reaction: Some(_),
                    ..
                }
        ) {
            return [0.0; 3];
        }
        let [x, z] = p.velocity();
        [x * TICK_SECONDS, 0.0, z * TICK_SECONDS]
    }

    /// A checksum of the settled state, for net play: peers compare this
    /// after the same tick and re-sync a drifter (the roadmap's "state
    /// checksums"). FNV-1a, bit-exact, over everything the game can act on:
    /// the score, the possession, every player's placement and the ball's.
    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325;
        // The canonical snapshot includes the RNG, actions, meters, rules,
        // clocks and previous input, using fixed-width/varint encodings.
        // Hashing a subset allowed divergent games to report equal checksums.
        for byte in self.snapshot_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
    /// A game between `players` (each knows its team). The ball starts dead,
    /// and team 0 inbounds it from under its own basket.
    pub fn new(hoops: [Hoop; 2], players: Vec<Player>, rules: Rules, seed: u32) -> Self {
        let n = players.len();
        Self {
            tick: 0,
            hoops,
            players,
            ball: Ball::loose_at([0.0, ball::RADIUS, 0.0]),
            rules,
            score: rules.start_score(),
            stage_bounds: STAGE_BOUNDS,
            update_order: HIGHEST_FIRST,
            teams: [0, 1].map(|side| {
                let (pocketed, meter) = rules.meter_spot(side);
                TeamState {
                    trick_points: rules.spot_trick_points[side],
                    meter,
                    pocketed,
                    ..Default::default()
                }
            }),
            possession: 0,
            shot_clock: rules.shot_clock.unwrap_or(0.0),
            // The six walk to their restart spots (the computer's
            // `ai::inbound`); otherwise a second to the rewrite's inbound.
            phase: Phase::DeadBall {
                left: if n == 6 { tuning::RESTART_LIMIT } else { 1.0 },
                team: 0,
            },
            game_phase: game_phase::GamePhase::restarting(0),
            events: Vec::new(),
            give_and_go: None,
            fresh_take: false,
            windup: None,
            catch: None,
            turbo_releases: vec![[u64::MAX; 4]; n],
            rng: rng::Rng::new(seed),
            previous_inputs: vec![Input::default(); n],
            staged: None,
            gait: None,
            variety: gamebreaker::Variety::default(),
            sequence: None,
            combo: combo::Manager::default(),
            assist: None,
            pump_fake_watch: vec![false; n],
            pump_fake_left: 0,
            pump_fake_shot: 0,
            clock: shot_clock::ShotClock::default(),
            lesson: None,
            lesson_page: false,
            scenario: None,
            side_teams: [NO_TEAM; 2],
            opening_toss: OpeningToss::None,
            tip: Tip::default(),
            timed_dead_ball: false,
            scorer: None,
            celebrate_pending: false,
            libc: 1,
            replays: 1,
            contacts: contact::Contacts::default(),
            ai: ai::State::default(),
            meter_clock: street::MeterClock::Off,
            stats: stats::Stats::new(n),
            poses: pose::Bank::default(),
            last_trick: u8::MAX,
            catching: None,
            spot_run: receiver::SpotRun::default(),
            passout: None,
            held_anchor: None,
            turbo: [1.0; turbo::SLOTS],
            turbo_owner: vec![false; n],
            people: vec![true; n],
            control: control::Control::default(),
            gamebreaker_task: None,
            staged_start: None,
        }
    }

    /// A game built on the session's generator, as the original builds its
    /// game object on the shared one (`FUN_002f3000`): the loading's and
    /// the construction's [`CONSTRUCTION_DRAWS`], then, for a points game
    /// (`FUN_0012e750`, which the factory uses for every game but the
    /// trick-point and Gamebreaker Wins ones, `config+0xe8` 1 and 6),
    /// `FUN_0012e798`'s pair into [`World::stage_bounds`]. Five of the
    /// construction's draws place the actors in the update order
    /// ([`ORDER_DRAWS`], [`World::update_order`]). The game runs on from
    /// that state; [`World::generator`] hands it back when the match is
    /// over, for the next.
    pub fn from_generator(
        hoops: [Hoop; 2],
        players: Vec<Player>,
        rules: Rules,
        generator: rng::Rng,
    ) -> Self {
        let mut world = Self::new(hoops, players, rules, 0);
        world.rng = generator;
        let mut order = vec![0u8];
        for draw in 1..=CONSTRUCTION_DRAWS {
            let r = world.rng.next_u32();
            if let Some(k) = ORDER_DRAWS.iter().position(|&d| d == draw) {
                let at = (r % (k as u32 + 2)) as usize;
                order.insert(at, k as u8 + 1);
            }
        }
        world.update_order = order.try_into().unwrap_or(HIGHEST_FIRST);
        if matches!(rules.goal, Goal::Points(_)) {
            let early = world.rng.next_u32() % 6;
            let late = 15 + world.rng.next_u32() % 3;
            world.stage_bounds = [early, late];
        }
        world
    }

    /// The generator as the game has left it (the session's, which runs on
    /// from game to game: docs/TRACE.md "The session's generator").
    pub fn generator(&self) -> &rng::Rng {
        &self.rng
    }

    /// The player holding the ball.
    pub fn handler(&self) -> Option<usize> {
        match self.ball.state {
            BallState::Held(i) => Some(i),
            _ => None,
        }
    }

    pub fn team_of(&self, player: usize) -> usize {
        self.players[player].team
    }

    /// Where `hoop`'s rim is over the floor, (x, z).
    pub fn rim_spot(&self, hoop: usize) -> [f32; 2] {
        let r = self.hoops[hoop].rim;
        [r[0], r[2]]
    }

    /// True when a shot from `at` at `hoop` is from beyond the long-range arc.
    pub fn is_long_range(&self, at: [f32; 2], hoop: usize) -> bool {
        let rim = self.rim_spot(hoop);
        distance(at, rim) > tuning::ARC_RADIUS || at[1].abs() > tuning::ARC_CORNER
    }

    /// Advance one tick. `inputs[i]` drives `players[i]`; missing inputs
    /// count as no input.
    ///
    /// # Panics
    ///
    /// Without the disc's data installed ([`World::install`]): a match never
    /// plays on without the disc's animations, gait and staged plans, as a
    /// match on other rules would ([`pose::Bank`]).
    pub fn step(&mut self, inputs: &[Input]) {
        assert!(
            !self.poses.is_missing(),
            "a match can't play without the disc's animations (World::install)"
        );
        self.events.clear();
        // `FUN_00131560`, before the game's count moves on.
        self.combo_update();
        // The pad each side's one person plays, as the client gave it: the
        // pad's own last word for the next update ([`control`]).
        let pads = self.side_pads(inputs);
        // A staged gamebreaker's script poses its frame 0 two updates after
        // the press ([`World::staged_start`]).
        if let Some((sequence, wait)) = self.staged_start {
            if wait > 1 {
                self.staged_start = Some((sequence, wait - 1));
            } else {
                self.staged_start = None;
                self.begin_staged(sequence);
            }
        }
        if self.sequence.is_some() {
            // The game's update runs on through a staged gamebreaker: its
            // hand-off gives the pad to the computer holder the update
            // after a staged catch (`FUN_0013d868`; the staged pass is no
            // row's callback 38 and its catch no take, so nothing else
            // moves it: the School's Level 2 capture).
            let mut held: Vec<Input> = (0..self.players.len())
                .map(|i| inputs.get(i).copied().unwrap_or_default())
                .collect();
            self.game_hand_off(&mut held);
            self.step_staged();
            self.step_gamebreaker_task();
            // Buttons held through the sequence don't count as presses
            // when it ends. (A computer marker counts as no buttons.)
            for (i, previous) in self.previous_inputs.iter_mut().enumerate() {
                let input = inputs.get(i).copied().unwrap_or_default();
                *previous = if input.is_computer() {
                    Input::default()
                } else {
                    input
                };
            }
            self.remember_pads(pads);
            self.tick += 1;
            return;
        }
        self.sync_game_phase();
        // vt+0xbc as the game's update reads it: won by an earlier
        // update's basket (the hoop posts a basket after that update).
        let won = matches!(self.phase, Phase::Over { .. });
        // Which sides a person plays (game+0x9a), for the tip-off.
        let mut people = [false; 2];
        for (i, p) in self.players.iter().enumerate() {
            if !inputs.get(i).is_some_and(Input::is_computer) {
                people[p.team.min(1)] = true;
            }
        }
        // Who holds a controller, for the turbo bars ([`turbo`]).
        self.set_people(inputs);
        let n = self.players.len();
        let mut inputs: Vec<Input> = (0..n)
            .map(|i| inputs.get(i).copied().unwrap_or_default())
            .collect();
        // The action tap's pass messages (a person's switch), then the
        // game's update's hand-off to the holder and the team's update's
        // end of a give 'n go off the ball ([`control`]).
        self.action_tap(&mut inputs);
        self.game_hand_off(&mut inputs);
        self.hold_without_ball();
        let holder_before = self.handler();
        // A pass's receiver keeps his stick from the press, and a person's
        // runs on his AI's frame while he is one ([`World::pad_off`]).
        self.freeze_receiver();
        let off: Vec<bool> = (0..n).map(|i| self.pad_off(i)).collect();
        // The AI's frame up to the players' ([`ai`]); each computer
        // player's own runs at the start of his actor's update below.
        let frame = self.ai_begin(&inputs, &off);
        // The opening toss: until the referee lets go, the players stand
        // frozen at their spots (`FUN_00140880` clears `actor+0x298` only
        // once the ball is in state 7) and no button counts.
        if self.phase == Phase::Live
            && let OpeningToss::Waiting(left) = self.opening_toss
        {
            self.ai_players(frame.as_ref(), &mut inputs, &self.actor_order());
            for (i, previous) in self.previous_inputs.iter_mut().enumerate() {
                *previous = inputs.get(i).copied().unwrap_or_default();
            }
            self.remember_pads(pads);
            if left > 1 {
                self.opening_toss = OpeningToss::Waiting(left - 1);
                self.tick += 1;
                return;
            }
            self.launch_opening_toss();
        }
        if let Phase::DeadBall { left, team } = &mut self.phase {
            *left -= TICK_SECONDS;
            let (left, team) = (*left, *team);
            // The six's restart ends in the game's update
            // ([`World::step_game_phase`]); past the rewrite's limit, or
            // without the six, the rewrite inbounds.
            if left <= 0.0 {
                self.inbound(team);
            }
        }
        let restarting = self.restarting();
        // The game's distance table (`FUN_0013fbc8`) is built before the
        // actors update: a release times a led pass on these places (and the
        // next update's action tap switches on them).
        let start = self.floor_positions();
        self.control.tables = Some(control::Tables {
            places: start.clone(),
            ball: self.ball.position,
        });
        // Where the held ball lies against its holder as the actors begin
        // (what a blend starting in his update stores, [`held`]).
        let held_before = self.held_offset();
        // The actors update in the match's order ([`World::update_order`]):
        // what one reads of another in his update (a shot let go, a lob's
        // leaper, a spot pass's receiver) is that update's state for one
        // earlier in it and the last update's for one later.
        let order = self.actor_order();
        for (k, &i) in order.iter().enumerate() {
            // A pass pressed earlier in this update freezes its receiver's
            // stick before his own update.
            self.freeze_receiver();
            // His AI's frame (`FUN_001587d8`'s `FUN_0013a970`), on the
            // others as their updates left them.
            if let Some(input) = frame.as_ref().and_then(|f| self.ai_player(f, i)) {
                inputs[i] = input;
            }
            let input = self.frozen_input(i, inputs.get(i).copied().unwrap_or_default());
            // A person's presses are his pad's own ([`World::pad_before`]),
            // unless his AI's frame drives him.
            let before = if off[i] {
                self.previous_inputs[i]
            } else {
                self.pad_before(i)
            };
            let pressed = input.pressed_since(&before);
            if self.windup.is_some_and(|w| w.from == i) {
                // The passer's animation: the ball leaves his hands at
                // the pass script's callback 38.
                self.step_windup(&start);
            }
            if self.opening_toss == OpeningToss::Flight {
                self.tip_off(i, pressed, people);
            }
            // A person's dive is a turbo tapped again (the controller's
            // word, kept every update).
            let released = before.buttons & !input.buttons;
            let tapped = !input.is_computer() && self.dive_tap(i, pressed, released);
            let pressed = if tapped {
                pressed | button::DIVE
            } else {
                pressed
            };
            // The turbo bar's owner and its update (`FUN_001587d8`,
            // `0x1588f0..0x1589d0`), before his side's update reads it.
            self.update_turbo_owner(i);
            self.hold_turbo(i, &input);
            // The person's pad off the ball: the give 'n go's call or its
            // end (`FUN_001587d8`).
            let calls = if self.people[i] && !off[i] && self.ball.state != BallState::Held(i) {
                self.off_ball_call(i, &input, &before)
            } else {
                Vec::new()
            };
            if let Some(f) = frame.as_ref() {
                for &call in &calls {
                    self.call_for_ball(i, call, f);
                }
            }
            let calls = !calls.is_empty();
            if self.phase == Phase::Live {
                self.buttons(i, &input, pressed, &before, calls, &start);
            } else if restarting
                && pressed & button::PASS != 0
                && self.ball.state == BallState::Held(i)
                && self.players[i].free()
            {
                // The inbound pass ("throw inbound" presses only pass;
                // nobody else presses anything in phase 6).
                self.pass_button(i, &input, false, &start);
            }
            if self.sequence.is_some() {
                self.count_down_bars(i);
                self.ai_players(frame.as_ref(), &mut inputs, &order[k + 1..]);
                for (j, previous) in self.previous_inputs.iter_mut().enumerate() {
                    *previous = inputs.get(j).copied().unwrap_or_default();
                }
                self.remember_pads(pads);
                self.step_staged();
                self.step_gamebreaker_task();
                self.tick += 1;
                return;
            }
            let held = self.ball.state == BallState::Held(i);
            // `FUN_0016d888` for a block jumper: his hands as the last
            // update posed them (the model is posed after the actor's
            // tests) against the ball the last ball update left.
            self.block_test(i);
            // The receiver's run to the pass: the court's edges and the
            // spot run's stop ([`receiver`]); then his parked catch row
            // starts before his tests ([`touch::Catching`]); its channel
            // steps after them.
            self.receiver_run(i);
            let caught_started = self.catching_start(i);
            // A receiver on a standing catch row (0x4000) stands for it:
            // the row's script, started at the release (`FUN_001611d8`),
            // holds him through the flight whatever his behaviour asks. With
            // the disc's animations every row's script takes him from his
            // stick from the update it starts in ([`touch::Catching::holds`];
            // while it is parked his behaviour still moves him).
            let standing = self.ball.state == BallState::Pass
                && self.ball.pass.is_some_and(|p| {
                    p.to == i
                        && if self.poses.get().is_some() {
                            self.catch_row_playing(i).is_some()
                        } else {
                            timing::CATCH_ROWS
                                .get(usize::from(p.catch))
                                .is_some_and(|r| r.flags & 0x4000 != 0)
                        }
                });
            let input = match self.receiver_stick(i) {
                Some(stick) => Input { stick, ..input },
                None => input,
            };
            let standing = standing || self.receiver_holds(i);
            let input = if standing {
                Input {
                    stick: [0, 0],
                    ..input
                }
            } else {
                input
            };
            // The tail's catches, pickups and knocks off the body, on his
            // bones as his channel now shows them at his place before he
            // moves, against the ball the last ball update left ([`touch`]);
            // a diver's catch first, which ends his tests when it meets the
            // ball (`FUN_0015c598`, [`dive`]).
            if !self.dive_catch(i) {
                self.touch_ball(i);
            }
            if !caught_started {
                self.catching_tick(i);
            }
            self.passout_tick(i);
            // A rebound jump's callback 36 at its script's jump tick; a
            // block jump's callback 35 at its own.
            self.rebound_jump_tick(i);
            self.block_jump_tick(i);
            // His side's first test: a celebration due in the restart.
            self.celebrate_due(i);
            // A trick's or a reaction's script moves him ([`trick`]).
            let trick = match self.players[i].action {
                Action::Trick { id, .. } => Some(id.script),
                _ => None,
            };
            // Any move's script but the stands' and the gait's replaces
            // the state word (`9b`), and none of them posts up (bit 11).
            if !matches!(self.players[i].action, Action::None) {
                self.players[i].posting = false;
            }
            // The defence's update keeps a post defender round the posting
            // holder; the offence's lets a man off the ball post for it
            // ([`backdown`]).
            if self.players[i].team != self.possession {
                self.post_slide(i);
            } else {
                self.post_call(i, &input);
            }
            self.players[i].restart_walk = self.game_phase.phase == game_phase::phase::RESTART;
            // `FUN_00163d40`: a guard's mode and the gait record, before
            // the walk sets the speed from it.
            let stance = self.guard_stance(i, &input, &start);
            // A holder who picked up his dribble does not move: his stick
            // only turns him on the stand path.
            let walk = if self.held_still(i) {
                Input {
                    stick: [0, 0],
                    ..input
                }
            } else {
                input
            };
            self.select_gait(i, &walk, held, stance);
            self.stand_path(i, &input);
            self.idle_face(i);
            let input = walk;
            // A backdown move holds his walk until it ends (its `91`'s 0x2:
            // the locomotion is not called, `0x15b084`), as does the post
            // defence's hold.
            let moved = if self.locomotion_held(i, &input) {
                Input {
                    stick: [0, 0],
                    ..input
                }
            } else {
                input
            };
            let ended = self.trick_tick(i, &input, held);
            self.reaction_tick(i);
            self.stand_tick(i);
            self.counter_tick(i);
            self.scoop_tick(i);
            let before = self.players[i].position;
            let facing = self.players[i].heading;
            self.catching_coast(i);
            // The swipe's test (`FUN_0015da28` after its presses, while bit
            // 6 is set): in his side's update, before his channel steps and
            // he moves, on the clock, place and speed the last update left;
            // then the channel's step.
            self.swipe_due(i);
            self.swipe_tick(i);
            let rebound = match self.players[i].action {
                Action::ReboundJump { script, .. } => Some(script),
                _ => None,
            };
            match self.players[i].step(&moved, held, stance) {
                Some(player::Moment::Release(hoop)) => self.release_shot(i, hoop),
                Some(player::Moment::Slam(hoop)) => self.slam(i, hoop),
                Some(player::Moment::Swipe(_)) => self.swipe(i),
                Some(player::Moment::LayupRelease(hoop)) => self.release_layup(i, hoop),
                None => {}
            }
            self.contain(i, before);
            if let Some(script) = rebound
                && self.players[i].action == Action::None
            {
                self.rebound_landed(i, script, &start);
            }
            self.catching_turn(i, facing);
            // `FUN_0015da28`'s catch for a rebound jumper (`FUN_0016ed18`).
            self.rebound_catch(i);
            let channel = &mut self.players[i].gait_channel;
            if !channel.moving || channel.family == 0 {
                channel.record = None;
            }
            self.follow_hand(i, trick.zip(ended));
            self.check_pump_fake(i, &input);
            // A shot's, dunk's or lay-up's flight on its script ([`jump`]),
            // after the release and the fadeaway's switch, then the court's
            // bound again.
            let before = self.players[i].position;
            self.flight_tick(i, start.get(i).copied().unwrap_or(before));
            self.contain(i, before);
            // The head's look ([`look`], `0x157d9c`).
            self.head_look(i);
            self.previous_inputs[i] = input;
            self.count_down_bars(i);
        }
        // The passer stays in his pass animation (bit 4) for the whole
        // windup, however long the row's script holds the ball.
        if let Some(w) = self.windup
            && self.players[w.from].action == Action::None
        {
            self.players[w.from].action = Action::Passing {
                elapsed: 0.0,
                row: w.row,
                tick: u16::from(timing::PASS_ROWS[usize::from(w.row)].release - w.left + 1),
            };
        }
        // `FUN_00133bb0`: players in contact push apart.
        self.step_contacts(&start);
        if self.rules.unlimited_turbo {
            self.fill_turbo();
        }
        if self.ball.state == BallState::Shot {
            self.stats.rebound = true;
        }
        // `FUN_0014e060`: a held ball counts the assist window down.
        if matches!(self.ball.state, BallState::Held(_))
            && let Some((passer, left)) = self.assist
        {
            self.assist = (left > 1).then_some((passer, left - 1));
        }
        let before = self.ball.position;
        self.step_ball(held_before);
        self.ball.moved = std::array::from_fn(|k| self.ball.position[k] - before[k]);
        // The gamebreaker task's process runs before the game's update.
        self.step_gamebreaker_task();
        self.step_game_phase(won);
        self.step_shot_clock();
        if self.game_phase.live() {
            // Actor+0x2a4 (`FUN_00156f48`): each actor's updates of live
            // play (`FUN_00140558`).
            for frames in &mut self.stats.on_court {
                *frames += 1;
            }
        }
        self.step_pump_fake_watch();
        self.step_meters();
        // The take's hand-off (`FUN_001415c0`, slot 24): whoever was given
        // the ball in this update.
        if let Some(h) = self.handler()
            && holder_before != Some(h)
        {
            self.take_hand_off(h);
        }
        self.remember_pads(pads);
        self.tick += 1;
    }

    /// The actors in the order they update ([`World::update_order`]; a
    /// world of other than six, the highest index first).
    pub fn actor_order(&self) -> Vec<usize> {
        if self.players.len() == 6 {
            self.update_order.iter().map(|&i| usize::from(i)).collect()
        } else {
            (0..self.players.len()).rev().collect()
        }
    }

    /// The AI frames of `players` ([`World::ai_player`]), for an update
    /// that stops before their actors' updates.
    fn ai_players(&mut self, frame: Option<&ai::Frame>, inputs: &mut [Input], players: &[usize]) {
        let Some(frame) = frame else {
            return;
        };
        for &i in players {
            if let Some(input) = self.ai_player(frame, i)
                && let Some(slot) = inputs.get_mut(i)
            {
                *slot = input;
            }
        }
    }

    /// Each side's one person's pad as `inputs` give it (none for a side
    /// with no person or several).
    fn side_pads(&self, inputs: &[Input]) -> [Option<Input>; 2] {
        let mut pads = [None; 2];
        for (side, pad) in pads.iter_mut().enumerate() {
            let mut theirs = (0..self.players.len()).filter(|&j| {
                self.players[j].team.min(1) == side
                    && !inputs.get(j).copied().unwrap_or_default().is_computer()
            });
            if let (Some(j), None) = (theirs.next(), theirs.next()) {
                *pad = inputs.get(j).copied();
            }
        }
        pads
    }

    /// [`control::Control::pad_before`] for the next update.
    /// The pads' presses start over (`FUN_00130b40`, the controllers'
    /// records cleared when Street School launches a lesson or opens a
    /// tutorial page over the held game): what `inputs` hold counts as held
    /// at the next update, not pressed.
    pub fn settle_presses(&mut self, inputs: &[Input]) {
        for (i, previous) in self.previous_inputs.iter_mut().enumerate() {
            *previous = inputs.get(i).copied().unwrap_or_default();
        }
        let pads = self.side_pads(inputs);
        self.remember_pads(pads);
    }

    fn remember_pads(&mut self, pads: [Option<Input>; 2]) {
        for (side, pad) in pads.into_iter().enumerate() {
            self.control.pad_before[side] = pad.unwrap_or_default();
        }
    }

    /// The live ball hand of a player holding the ball: from the frame of
    /// the gait row or standing dribble it plays, and from the frame a
    /// trick's clip shows (`ended`: the script and its last frame of a
    /// trick that ended this tick). Only clips the sim knows the frames of
    /// update it; the original also reads every other held clip.
    fn follow_hand(&mut self, i: usize, ended: Option<(u16, f32)>) {
        let Some(config) = self.gait.clone() else {
            return;
        };
        if self.ball.state != BallState::Held(i) {
            return;
        }
        let p = &mut self.players[i];
        let channel = &mut p.gait_channel;
        let flags = match (ended, &p.action) {
            // A trick ends on its last wait's frame.
            (Some((script, frame)), action) if !matches!(action, Action::Trick { .. }) => {
                config.trick_hand_at(script, channel.trick_side, frame as usize)
            }
            // Under way: the frame its script shows, on the side the live
            // hand picked when it started (`FUN_001978b0`'s 0xf8).
            (_, Action::Trick { id, play }) => {
                channel.trick_side = u8::from(play.run.left);
                let shown = play.run.shown();
                config.trick_hand_at(id.script, channel.trick_side, shown.frame as usize)
            }
            (_, Action::None) if channel.moving && channel.record.is_some() => {
                config.row_hand(channel.record_row(), channel.phase.max(0.0) as usize)
            }
            (_, Action::None) if !channel.moving && !config.stand_hands.is_empty() => {
                let frames = &config.stand_hands;
                Some(frames[channel.phase.max(0.0) as usize % frames.len()])
            }
            _ => None,
        };
        if let Some(flags) = flags {
            channel.hand = gait_select::live_hand(channel.hand, flags);
        }
    }

    /// `FUN_00168b30` for player `i`, before his walk sets the speed from
    /// it (`FUN_00163d40` picks the record and then the speed): with his
    /// stick out, the gait record for the family his stick asks for and
    /// whether he holds the ball, or a guard's record, kept while still
    /// eligible.
    fn select_gait(&mut self, i: usize, input: &Input, held: bool, stance: Option<guard::Stance>) {
        let p = &self.players[i];
        let walks = match p.action {
            Action::None | Action::Passing { .. } | Action::Stealing { .. } => true,
            // A trick's press update: its gait still runs.
            Action::Trick { play, .. } => play.run.tick < 0,
            _ => false,
        };
        if !walks || !input.moves() || self.locomotion_held(i, input) {
            return;
        }
        let channel = &p.gait_channel;
        let intensity = input.strength().clamp(0, 64) as u32;
        let family = input::evaluate_gait_family_hysteresis(channel.family, intensity) as u32;
        let mut request = match stance {
            Some(s) => s.flags,
            None => gait_select::locomotion_flags(family, held),
        };
        // A holder's backdown ([`backdown`], `0x167564..0x16780c`): a
        // script takes the update, or the backdown's records.
        if held && stance.is_none() {
            let stick = backdown::stick_word(input);
            if self.backdown_move(i, input, stick, &mut request) {
                return;
            }
        }
        let Some(config) = self.gait.clone() else {
            return;
        };
        // A holder walking in a restart (`0x1676a4`, game+0xf4 6) asks
        // 0x201, the one record BPJOG (957): he carries the ball to his
        // spot without a dribble (the held ball stays 2 or more up through
        // every captured restart) and cradles it there ([`stand`]); its
        // motion by the stick is the captured inbounders' 0.218, 0.281
        // and 0.300 an update at strengths 38, 58 and 64.
        if held && stance.is_none() && self.game_phase.phase == game_phase::phase::RESTART {
            request = 0x201;
        }
        let p = &mut self.players[i];
        let channel = &mut p.gait_channel;
        let profile = config.profiles.get(i).copied().unwrap_or_default();
        let current = channel.record.map(|_| channel.record_row());
        let rng = &mut self.rng;
        let index = gait_select::select_drawing(&config.records, current, request, profile, || {
            rng.next_u32()
        });
        if let Some(record) = config.records.get(index) {
            // A record started anew runs its script's `9b`: only the
            // backdown's (`dribble_backdown_1`, `9b 0x820`) posts up.
            if channel.record != Some(index as u16) {
                p.posting = record.rows[0] == 704;
                // `0x167f40..0x167f54`: a record whose script starts anew
                // with the ball (or a post-up's) is a dribble; the row
                // kept (`0x167eb0`) starts nothing.
                if request & 0x102 != 0 {
                    p.dribble = 1;
                }
            }
            channel.record = Some(index as u16);
            channel.rows = record.rows;
            channel.root_distance = record.root_distance;
            channel.motion = record.motion;
        }
    }

    /// Start whatever the newly `pressed` buttons ask for.
    ///
    /// With the ball: shoot (two turbos: gamebreaker; turbo near the basket:
    /// dunk, all four for an advanced dunk), pass, trick move (level by the
    /// turbos held), pocket or cancel a gamebreaker. Without: steal (two
    /// turbos: trick counter), jump to block or rebound.
    ///
    /// `before` is the input the presses are taken against
    /// ([`World::pad_before`]); `calls` that a person off the ball calls for
    /// it ([`World::off_ball_call`]).
    fn buttons(
        &mut self,
        i: usize,
        input: &Input,
        pressed: u16,
        before: &Input,
        calls: bool,
        start: &[[f32; 2]],
    ) {
        use tuning::*;
        // The right stick pushed home is a pass press of its own (the
        // held word's 0x400, [`passing::aim_pressed`]): the give 'n go.
        let aimed = passing::aim_pressed(input, before);
        let pass_press = pressed & button::PASS != 0 || aimed;
        if !self.players[i].free() {
            // The holder's ball update (`FUN_0015a060`) tests the pocket
            // press (`FUN_00159eb8`: pad +0x7c & 0x8000000 and
            // `FUN_00141098`) every update he holds the ball, whatever move
            // he is in: a pocket pressed during a trick or a pass still
            // goes in. (Before, a busy holder dropped it, and a computer
            // handler, whose `pocket_check` keeps pressing while it holds,
            // never pressed it anew.)
            if self.ball.state == BallState::Held(i)
                && pressed & button::SELECT != 0
                && (self.players[i].turbo <= 0.0 || input.turbos() == 0)
            {
                self.pocket(i, 0);
            }
            // The holder's pass press goes in whatever move he is in
            // ([`World::passes_out`]): the move gives way to the pass row.
            if self.ball.state == BallState::Held(i) && pass_press && self.passes_out(i) {
                let before = self.players[i].action;
                self.players[i].action = Action::None;
                self.pass_button(i, input, aimed, start);
                let went = self.windup.is_some_and(|w| w.from == i)
                    || self.ball.state != BallState::Held(i);
                if !went {
                    self.players[i].action = before;
                }
                return;
            }
            // The give 'n go's call is the pad builder's (`FUN_001587d8`),
            // not the actor's: it went in whatever move the caller is in,
            // his pass's follow-through included ([`World::call_for_ball`]).
            if calls {
                return;
            }
            self.busy_buttons(i, pressed, input);
            return;
        }
        self.players[i].action_buttons = input.buttons;
        let turbos = if self.players[i].turbo > 0.0 {
            input.turbos()
        } else {
            0
        };
        if self.ball.state == BallState::Held(i) {
            if self.shoots(input, pressed) {
                self.shoot_button(i, input);
            } else if pass_press {
                self.pass_button(i, input, aimed, start);
            } else if pressed & button::TRICK != 0 {
                self.juke_press(i, input, turbos);
            } else if pressed & button::SELECT != 0 {
                self.pocket(i, turbos);
            }
        } else if self.players[i].team == self.possession
            && pressed & button::SHOOT != 0
            && self.leap_press(i, input, turbos)
        {
            // Shoot with a turbo off the ball on the side with it: the
            // alley-oop's leap (`FUN_0015a060`, [`leap`]).
        } else if pressed & button::DIVE != 0 && self.dive_press(i) {
            // The dive for a loose ball ([`dive`]).
        } else if calls {
            // The call's node is pushed in his controller's update
            // ([`World::call_for_ball`]); the press starts nothing else.
        } else if self.attacking(i) && pressed & button::JUMP != 0 {
            // A teammate holds the ball: Triangle starts nothing
            // (`FUN_0015a060` takes the rebound press only while nobody
            // holds it, `0x15b49c..0x15b4c0`).
        } else if self.attacking(i) && pressed & button::TRICK != 0 {
            // A teammate holds the ball: no swipe (it is the defence's,
            // `FUN_0015da28`); the held juke is the post call
            // ([`World::post_call`]).
        } else if pressed & button::TRICK != 0 {
            // `FUN_0015da28` (raw `0x15df60..0x15e0e4`): two turbos held
            // (actor+0x78 & 0x40000) without the No Counters cheat
            // (`FUN_00131208()+0x78`) ask for the counter stance, which
            // [`turbo::MOVE`] of the bar buys ([`World::start_counter`]);
            // short of it, the "no turbo" message and nothing. Otherwise
            // the swipe.
            if input.turbos() >= 2 && !self.rules.no_counters {
                if self.spend_turbo(i, turbo::MOVE) {
                    let word = self.wanted_facing(i, input);
                    self.start_counter(i, word);
                    self.events.push(Event::Reach {
                        player: i,
                        counter: true,
                    });
                }
                return;
            }
            let swipe = self.start_swipe(i);
            // The swiper's face (`FUN_00159190(swiper, 2, fps Ã— 2)`,
            // `0x15e364..0x15e378`).
            self.face(i, 2 * TICK_HZ as i32);
            // The press faces him at the holder (actor+0xb0 and the stick,
            // in every played capture's swipe); he turns to it 46 an
            // update ([`World::swipe_tick`]).
            if swipe.is_some()
                && let BallState::Held(h) = self.ball.state
            {
                let word = contact::angle(self.players[i].position, self.players[h].position);
                self.players[i].wanted = restart::heading_radians(word);
            }
            self.players[i].action = Action::Stealing {
                elapsed: 0.0,
                counter: false,
                swipe,
                stance: None,
            };
            self.events.push(Event::Reach {
                player: i,
                counter: false,
            });
        } else if pressed & button::JUMP != 0
            && self.players[i].team == self.possession
            && self.players[i].height == 0.0
            && !matches!(self.players[i].action, Action::ReboundJump { .. })
            && !self
                .ball
                .pass
                .is_some_and(|p| self.ball.state == BallState::Pass && p.to == i)
            && self.putback(i, input)
        {
            // `FUN_0015a060`'s rebound press with two turbos: the putback
            // ([`putback`], `0x15b4c0..0x15b52c`).
        } else if pressed & button::JUMP != 0 {
            // `FUN_0015da28`: at a rebound coming off (ball+0x180 0), a
            // shot on the hoop (6) or a loose ball (1) a rebound jump
            // (`FUN_0015ce50`, [`rebound`]); at any other a block jump
            // (`FUN_0015d3a0`, the rewrite's own jump).
            let rebound = matches!(
                self.native_ball_state(),
                game_phase::ball::REBOUND | game_phase::ball::RIM | game_phase::ball::LOOSE
            );
            if rebound {
                self.start_rebound_jump(i);
            } else if self.poses.get().is_some() {
                // `FUN_0015d3a0`'s script ([`block`]); the turbo block
                // without half a bar is no jump at all.
                let Some(jump) = self.start_block_jump(i, turbos) else {
                    return;
                };
                self.players[i].action = Action::Jumping {
                    elapsed: 0.0,
                    high: jump.turbo,
                    block: Some(jump),
                };
            } else {
                let high = turbos >= 2 && self.spend_turbo(i, TURBO_BLOCK_TURBO);
                self.players[i].action = Action::Jumping {
                    elapsed: 0.0,
                    high,
                    block: None,
                };
            }
            self.events.push(Event::Jump {
                player: i,
                block: !rebound,
            });
        }
    }

    /// `FUN_0015a060` (raw `0x15b860..0x15b944`): the holder's pass press
    /// is taken in any move unless he is passing already (bit 4,
    /// `FUN_00154680`), down or on no clip (`FUN_0015f898`: the channel's
    /// 0x40, `FUN_0016ea70`; its list `0x43f098` holds only -1), or the ball
    /// is a gamebreaker's (ball+0x178, `FUN_00141018`) and he is in a jump
    /// shot, a lay-up, a dunk or a leap (bits 3, 13, 7, 15). The rewrite's
    /// gamebreakers are jump shots: the flag stands set through one.
    pub(crate) fn passes_out(&self, i: usize) -> bool {
        let a = &self.players[i].action;
        if matches!(
            a,
            Action::Passing { .. } | Action::Stumbling { .. } | Action::Dive { .. }
        ) {
            return false;
        }
        let gamebreaker = matches!(
            a,
            Action::Shooting {
                gamebreaker: 1..,
                ..
            }
        );
        !(gamebreaker && block::holder_bits(a) & 0xa088 != 0)
    }

    /// `FUN_0015a060`'s shoot press for a free holder on the floor (raw
    /// `0x15b0f8..0x15b1ac`): a computer's held button (`FUN_00172220`); a
    /// person's held button, or right after the ball changed sides
    /// ([`World::fresh_take`]) a new press. So a person holding shoot
    /// through the end of a move or a catch shoots once he is free (the
    /// pads capture's rebound at update 2690: pressed in the jump's tail,
    /// the shot two updates later on the held button).
    pub(crate) fn shoots(&self, input: &Input, pressed: u16) -> bool {
        if input.is_computer() || !self.fresh_take {
            input.held(button::SHOOT)
        } else {
            pressed & button::SHOOT != 0
        }
    }

    /// True when a teammate of `i` (not `i`) holds the ball.
    pub fn attacking(&self, i: usize) -> bool {
        self.handler()
            .is_some_and(|h| h != i && self.team_of(h) == self.team_of(i))
    }

    /// The ball leaves `player`'s hands toward `hoop`, going in or not by the
    /// shot's chance, unless a defender blocks it.
    fn release_shot(&mut self, player: usize, hoop: usize) {
        use tuning::*;
        if self.ball.state != BallState::Held(player) {
            return;
        }
        let p = self.players[player].clone();
        let rim = self.rim_spot(hoop);
        let [fx, fz] = p.forward();
        // Callback 21 launches the ball from where it lies: in the shot
        // clip's hand, where the last ball update held it ([`held`]).
        // Without the disc's animations, the rewrite's own point.
        let from = if self.poses.get().is_some() {
            self.ball.position
        } else {
            [
                p.position[0] + fx * 0.5,
                RELEASE_HAND_HEIGHT + p.height + self.ball.radius,
                p.position[1] + fz * 0.5,
            ]
        };
        let long = self.is_long_range(p.position, hoop);
        // Slot 11: the shot is tried (the rewrite's blocks take it at the
        // release, where the original's catch it in flight).
        self.stats.shot(player, p.team, long);
        let (gamebreaker, fade) = match p.action {
            Action::Shooting {
                gamebreaker, fade, ..
            } => (gamebreaker, fade),
            _ => (0, false),
        };
        // The rewrite's stand-in block, without the disc's animations
        // (with them the jumpers' hands meet the ball in flight,
        // [`World::block_test`]): Level 2 gamebreakers cannot be blocked;
        // turbo blocks reach further.
        if gamebreaker < 2
            && self.poses.get().is_none()
            && let Some(blocker) = self.blocker(player, BLOCK_RANGE * 1.25, 1.0)
        {
            let q = &self.players[blocker];
            let high = matches!(q.action, Action::Jumping { high: true, .. });
            let near = distance(q.position, p.position) < BLOCK_RANGE;
            let skill = q.ratings.blocks;
            let fade_cover = if fade { 0.6 } else { 1.0 };
            let chance =
                (BLOCK_BASE + BLOCK_SKILL * skill + if high { 0.15 } else { 0.0 }) * fade_cover;
            if (near || high) && self.rng.chance(chance) {
                self.swat(blocker, player, from, rim);
                return;
            }
        }
        let points = if long { 2 } else { 1 };
        // The original's release callback decides the make and how it goes
        // in or misses (on Shots, or Threes with the 2-pointer cheats), and
        // its launcher `FUN_0014aaf8` aims the ball
        // (`shooting::World::shot_call`, `launch_shot`). A fadeaway changes
        // nothing there.
        let call = self.shot_call(player, hoop, from, false, gamebreaker, long);
        self.launch_shot(
            hoop,
            from,
            call,
            Shot {
                shooter: player,
                hoop,
                made: call.made,
                points,
                gamebreaker,
                layup: false,
                tried: 0,
                slam: None,
            },
            None,
        );
        self.events.push(Event::Shot {
            player,
            hoop,
            points,
        });
    }

    /// The dunker reaches the rim (the dunk script's callback 21): the
    /// dunk leaves his hands unless a jumping defender at the rim rejects
    /// it (the rewrite's own block), made or not by the release callback's
    /// dunk odds, through the launcher's dunk path ([`World::launch_dunk`]):
    /// the basket comes when it drops through the ring. A leaper's
    /// callback 21 with the ball is an alley-oop finish (the launcher's
    /// kind 3, `0x14ad14..0x14ad3c`: not in bit 7, his dunk id's flag
    /// 0x800 at `0x43f5f0` clear); without it, nothing.
    fn slam(&mut self, player: usize, hoop: usize) {
        if self.ball.state != BallState::Held(player) {
            return;
        }
        let h = self.hoops[hoop];
        let rim = self.rim_spot(hoop);
        let at = [h.rim[0], h.rim[1] + 0.3, h.rim[2]];
        let (style, layup, oop) = match self.players[player].action {
            Action::Dunking {
                style, layup, row, ..
            } => (style, layup, leap::leap_row(row)),
            _ => (player::DunkStyle::Normal, false, false),
        };
        // Turned into a lay-up on the way up: laid in from just below the rim.
        if layup {
            self.lay_in(player, hoop, [h.rim[0], h.rim[1] - 0.3, h.rim[2]]);
            return;
        }
        // Slot 11: a dunk goes through the launcher too, base 2.
        let team = self.players[player].team;
        self.stats.shot(player, team, false);
        if self.poses.get().is_none()
            && let Some(blocker) = self.blocker(player, tuning::BLOCK_RANGE * 0.8, 1.5)
        {
            let b = self.players[blocker].ratings.blocks;
            let d = self.players[player].ratings.dunks;
            if self.rng.chance(0.15 + 0.35 * b - 0.2 * d) {
                self.swat(blocker, player, at, rim);
                return;
            }
        }
        // The release callback's dunk odds ("Performing an Advanced Dunk
        // requires a player with a high Dunks rating, or else you risk
        // missing the dunk!"), rolled as `rand() % 100 < percent`.
        let advanced = style == player::DunkStyle::Advanced;
        let percent = self.dunk_call(player, hoop, advanced);
        let made = (self.rng.next_u32() % 100) < percent as u32;
        self.ball.rolled = made;
        self.events.push(Event::Slam { player, hoop });
        // Slot 39 (`FUN_00187930`): the dunker's slam.
        self.stats.add(player, team, stats::SLAMS, 1);
        // The dunk leaves the hands through the launcher (`FUN_0014aaf8`
        // posts slot 12 for a dunk, then slot 11 for every launch, raw
        // `0x14bf00..0x14bf80`): the game's phase 3 until the basket.
        let basket = if oop {
            street::Basket::AlleyOop {
                advanced: self.advanced_move(player),
            }
        } else {
            Self::dunk_basket(style)
        };
        self.launch_dunk(player, hoop, made, basket);
    }

    /// The nearest opponent of `shooter` in the air (at least `min_height`
    /// up) within `range`.
    fn blocker(&self, shooter: usize, range: f32, min_height: f32) -> Option<usize> {
        let s = &self.players[shooter];
        self.players
            .iter()
            .enumerate()
            .filter(|(_, q)| {
                q.team != s.team
                    && matches!(q.action, Action::Jumping { .. })
                    && q.height >= min_height
            })
            .map(|(j, q)| (j, distance(q.position, s.position)))
            .filter(|&(_, d)| d < range)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(j, _)| j)
    }

    /// A block, the rewrite's stand-in without the disc's animations: the
    /// ball is knocked away from the rim, loose. With them the original's
    /// block test ([`World::block_test`], `FUN_0016d888`) knocks it.
    fn swat(&mut self, blocker: usize, shooter: usize, at: [f32; 3], rim: [f32; 2]) {
        let away = normalize2(sub(self.players[shooter].position, rim));
        let side = self.rng.unit() - 0.5;
        // Forcing `Loose` must clear the flight payload: the snapshot
        // validator requires `Loose` with neither shot nor pass
        // (`validation.rs`). A goaltended shot otherwise survives as
        // `Loose` with `shot.is_some()`.
        self.ball.drop_loose();
        self.ball.position = at;
        self.ball.velocity = [
            away[0] * 14.0 - away[1] * side * 8.0,
            2.0,
            away[1] * 14.0 + away[0] * side * 8.0,
        ];
        self.credit_block(blocker, shooter, false);
    }

    /// Slot 16, posted by the knock `FUN_0014c408` when the side without
    /// the ball knocks a shot loose ([`World::post_block`]); `shooting`:
    /// the ball's holder at the knock was in his shot.
    pub(crate) fn credit_block(&mut self, blocker: usize, shooter: usize, shooting: bool) {
        self.post_block(blocker, shooting);
        self.events.push(Event::Block {
            player: blocker,
            shooter,
        });
    }

    /// The swipe's strip (`FUN_0016cf30`, from the actor's update
    /// `FUN_0015da28` while the swipe's bit 6 is set, once a swipe:
    /// actor+0x23c), here the stand-in's test without the disc's
    /// animations (with them, [`World::swipe_test`] poses the hand every
    /// update). The roll `r % 100` is drawn first; the swiping hand must
    /// reach the ball (the stand-in's [`tuning::HAND_REACH`] and
    /// [`tuning::HAND_RADIUS`]); then [`World::swipe_lands`]: the
    /// defender's Steals give
    /// `trunc(Steals / 99 Â· 64) + 20` (30 for a person's defender on Easy;
    /// 10 less for the computer's against a person's side on Easy), and it
    /// lands unless the roll is over that. Then `r % 100` at most
    /// `trunc((Steals âˆ’ 50) / 50 Â· 65)` catches the ball outright (a steal);
    /// otherwise it is knocked loose (`0x16d428`): 0.35 an update along the
    /// defender's facing turned by `r & 0x7f` (moving) or `(r & 0x7f) âˆ’
    /// 0x80`, rising 0.125, spinning (four more draws). The holder's face (`FUN_00159190`, 3 or 4
    /// by `r & 1`, which draws again); on the floor he reels on his
    /// channel ([`trick::STRIPPED`]), caught or knocked; knocked, he is
    /// barred from the ball `r % 5 + 25` updates and the swiper `fps ×
    /// 0.84`; a catch also gives the swiper face 2 (one more draw).
    /// Handles play no part.
    fn swipe(&mut self, i: usize) {
        use tuning::*;
        let Some(h) = self.handler() else {
            return;
        };
        if self.players[h].team == self.players[i].team {
            return;
        }
        let roll = (self.rng.next_u32() % 100) as i32;
        let p = &self.players[i];
        let f = p.forward();
        let hand = [
            p.position[0] + f[0] * HAND_REACH,
            3.0 + p.height,
            p.position[1] + f[1] * HAND_REACH,
        ];
        let b = self.ball.position;
        let reach = math::len(math::len(hand[0] - b[0], hand[2] - b[2]), hand[1] - b[1]);
        let moving = p.moving();
        if reach > HAND_RADIUS {
            return;
        }
        self.swipe_lands(i, h, roll, moving);
    }

    /// The swipe's hand reached the ball held by `h` with `roll` drawn
    /// (`FUN_0016cf30` from `0x16d1cc`): the odds, then the catch or the
    /// knock.
    fn swipe_lands(&mut self, i: usize, h: usize, roll: i32, moving: bool) {
        let (p, q) = (&self.players[i], &self.players[h]);
        let steals = (p.ratings.steals * 99.0).round().clamp(0.0, 99.0) as i32;
        let mut odds = (steals as f32 / 99.0 * 64.0) as i32 + 20;
        let (team, other) = (p.team, q.team);
        if self.rules.difficulty == Difficulty::Easy {
            if self.teams[team].human {
                odds += 10;
            } else if self.teams[other].human {
                odds -= 10;
            }
        }
        if odds < roll {
            return;
        }
        let caught =
            (self.rng.next_u32() % 100) as i32 <= ((steals - 50) as f32 / 50.0 * 65.0) as i32;
        // The holder's face (`FUN_00159190(holder, r & 1 ? 3 : 4, fps Ã—
        // 2)`, raw `0x16d318..0x16d33c`; not played): the `r & 1`, and the
        // reaction's own draw for its clip (its time is over 0).
        self.rng.next_u32();
        self.face(h, 2 * TICK_HZ as i32);
        // A holder on the floor (actor+0x34 0) reels on his channel by his
        // dribble hand, caught or knocked (`0x16d360`, `0x16d3f8`).
        if self.players[h].height == 0.0 {
            let left = self.players[h].gait_channel.hand == 2;
            self.begin_reaction_by(h, trick::STRIPPED, left);
            self.reaction_tick(h);
        }
        if !caught {
            let turn = (self.rng.next_u32() & 0x7f) as i32 - if moving { 0 } else { 0x80 };
            let facing = ((self.players[i].heading + std::f32::consts::PI) * 162.974_66 + 0.5)
                as i32
                & 0x3ff;
            let a = (facing - turn) & 0x3ff;
            let away = [-input::native_sin_1024(a), -input::native_cos_1024(a)];
            // The holder barred (actor+0x1a8) `r % 5 + 25` updates, the
            // swiper `fps × 0.84` (`fps × 0.17` in lesson 16,
            // `0x16d4b8..0x16d554`).
            self.players[h].barred = (self.rng.next_u32() % 5 + 25) as u16;
            let k = if self.lesson == Some(16) {
                f32::from_bits(0x3e2e_147b)
            } else {
                f32::from_bits(0x3f57_0a3d)
            };
            self.players[i].barred = (TICK_HZ as f32 * k) as u16;
            // Spinning about a random axis at `0.075 + u × 0.075`, free
            // (`FUN_0014c408`), as a block's knock.
            let mut axis = [0.0_f32; 3];
            for c in &mut axis {
                *c = (self.rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000) - 0.5;
            }
            let l = math::len(math::len(axis[0], axis[1]), axis[2]);
            let axis = if l <= 0.0001 {
                [0.0, 1.0, 0.0]
            } else {
                axis.map(|c| c / l)
            };
            let u = (self.rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000);
            self.ball.drop_loose();
            self.ball.velocity = [away[0] * 0.35 * 60.0, 0.125 * 60.0, away[1] * 0.35 * 60.0];
            self.ball.spin_axis = axis;
            self.ball.spin = u * 0.075 + 0.075;
            self.ball.airborne = true;
            self.ball.flight_left = 0.0;
            self.ball.flight_time = 0.0;
            return;
        }
        // The take of the held ball by the other side is slot 15's steal
        // ([`World::post_steal`]).
        self.take_ball(i);
        // The swiper's face (`FUN_00159190(swiper, 2, fps Ã— 2)`, raw
        // `0x16d3c4..0x16d3d0`, after the give; not played): its draw.
        self.face(i, 2 * TICK_HZ as i32);
        self.events.push(Event::Steal {
            player: i,
            from: h,
            intercepted_pass: false,
        });
    }

    /// `i` now holds the ball; their team has possession. The take posts
    /// its messages as `FUN_0014cb28` does ([`World::post_take`]): a pass
    /// caught or intercepted, a rebound, a block, a steal, and the catch.
    fn take_ball(&mut self, i: usize) {
        let state = self.native_ball_state();
        let possession = self.possession;
        let previous = match self.ball.pass {
            Some(pass) if self.ball.state == BallState::Pass => Some(pass.from),
            _ => self.ball.shooter.map(usize::from),
        };
        // FUN_0014c348's special flag survives both legs, including a
        // teammate's catch. Self throws retain the rewrite's fallback.
        let passout = self.ball.state == BallState::Pass
            && self.ball.pass.is_some_and(|p| {
                p.from == p.to
                    || self
                        .passout
                        .is_some_and(|po| po.passer == p.from && po.tossed)
            });
        let diving = matches!(self.players[i].action, Action::Dive { .. });
        // What the take reads of the ball before it is his: the holder in a
        // shot (bits 3, 7, 13, 15) and whether a loose ball came off the
        // other side's own shot.
        let shooting = self.holder_state().is_some_and(|b| b & 0xa088 != 0);
        let loose_steal = self.loose_steal(i);
        let blocked = self.game_phase.live()
            && self.players[i].team != possession
            && (state == game_phase::ball::SHOT || (state == game_phase::ball::HELD && shooting));
        let shooter = self.handler().or(previous);
        self.stats.rebound = false;
        // The dribble (actor+0x1a0): a take of a free ball starts it over
        // (`FUN_0014cb08`'s callers write 0); a pass's catch keeps the
        // receiver's; the ball changing sides clears everyone's
        // (`FUN_00169138`).
        if self.ball.state != BallState::Pass {
            self.players[i].dribble = 0;
        }
        self.ball.hold(i);
        let team = self.players[i].team;
        self.ball.touch[team.min(1)] = Some(i as u8);
        if team != self.possession {
            for p in &mut self.players {
                p.dribble = 0;
            }
            self.possession = team;
            self.fresh_take = true;
        }
        // Slot 24 (`FUN_001415c0`): the game's phase follows the catch.
        let x = self.players[i].position[0];
        let sides = self.sides();
        let scripted = self.sequence.is_some();
        self.game_phase.gained(team, x, sides, scripted);
        self.post_take(scoring::Take {
            taker: i,
            state,
            previous,
            possession,
            diving,
            passout,
            shooting,
            loose_steal,
        });
        // The caught block's slot 16 is also heard by the HUD and audio;
        // its following generic pickup must not replace it with a rebound.
        if blocked && let Some(shooter) = shooter {
            self.events.push(Event::Block { player: i, shooter });
        }
    }

    fn score_basket(&mut self, player: usize, basket: Basket) {
        let team = self.players[player].team;
        if self.phase != Phase::Live {
            return;
        }
        let points = basket.points(&self.rules);
        let taken = basket.taken(&self.rules);
        self.score[team] += points;
        self.score[1 - team] = self.score[1 - team].saturating_sub(taken);
        let gamebreaker = match basket {
            Basket::Gamebreaker { level, .. } => level,
            _ => 0,
        };
        let (kind, long) = match basket {
            Basket::Jumper { long } => (2, long),
            // The ball's kind (+0x184): an alley-oop finish by a move
            // with 0x800 is a dunk's.
            Basket::AlleyOop { advanced } => (if advanced { 4 } else { 3 }, false),
            Basket::Dunk { .. } => (4, false),
            Basket::Layup => (5, false),
            Basket::Gamebreaker { dunk: true, .. } => (4, false),
            Basket::Gamebreaker { long, .. } => (2, long),
        };
        // Slot 26 (`FUN_00187c10`): the scorer's counts. Dunks-only games
        // count dunks and alley-oops alone.
        if !self.rules.dunks_only || matches!(kind, 3 | 4) {
            self.stats
                .basket(player, team, points, taken, kind, long, gamebreaker, 0);
        }
        // The dunker hangs on in his dunk as the ball goes through: his
        // dunk's finish (actor+0x24c, the turbo combo's level) and a tip
        // slam (actor+0x1e4 1, a putback).
        let p = &self.players[player];
        let tip_slam = matches!(
            p.action,
            Action::Dunking {
                style: player::DunkStyle::Putback,
                ..
            }
        );
        let finish = shooting::dunk_move_level(p.action_buttons).clamp(0, 4);
        let assister = self
            .assist
            .map(|(a, _)| a)
            .filter(|&a| self.players[a].team == team && a != player);
        let wins = self.basket_wins(team, basket);
        self.post_basket(scoring::BasketPost {
            shooter: player,
            assister,
            base: if long { 3 } else { 2 },
            kind: i32::from(kind),
            gamebreaker,
            wins,
            tip_slam,
            finish,
            style: 0,
        });
        self.events.push(Event::Score {
            team,
            player,
            points,
            basket,
        });
        // The replay director's basket handler (FUN_003248d8, raw
        // 0x3248d8..0x324be4): with points, unless the game is won (the
        // game's vt+0xbc) and unless Manual replays offer it instead (no
        // gamebreaker ball and no full meter), it draws once from the
        // shared generator.
        let won = wins || matches!(self.phase, Phase::Over { .. });
        let meter_full = self.teams.iter().any(|t| t.meter_full());
        if points != 0 && !won && (self.replays != 2 || gamebreaker > 0 || meter_full) {
            let draw = self.rng.next_u32();
            self.events.push(Event::ReplayRoll { draw });
        }
        if self.game_phase.phase != game_phase::phase::WON {
            self.scorer = Some(player);
        }
        if matches!(self.phase, Phase::Over { .. }) {
            return; // the trick points just won it
        }
        if self.basket_wins(team, basket) {
            self.phase = Phase::Over { winner: team };
            self.events.push(Event::GameOver { winner: team });
        } else {
            self.start_restart(1 - team);
            // A gamebreaker's basket holds the restart through its replays
            // (`FUN_00141818`, `0x1419a4..0x141a38`: the basket's level and
            // the ball's gamebreaker flag +0x178 set, the game not won).
            if gamebreaker > 0 {
                self.game_phase.hold = game_phase::GAMEBREAKER_HOLD;
                self.game_phase.since = 0;
            }
        }
        self.basket_bars_and_faces(player, kind);
    }

    /// The game listener's slot 27 (`0x141c38`, a shot done without a
    /// basket): a face for each of the shooter's side by slot, his own 3
    /// or 4 by `r & 1` for fps Ã— 2, the others' 4 for fps Ã— 3.
    fn missed_faces(&mut self, shooter: usize) {
        let team = self.players[shooter].team;
        for i in 0..self.players.len() {
            if self.players[i].team != team {
                continue;
            }
            if i == shooter {
                self.rng.next_u32();
                self.face(i, 2 * TICK_HZ as i32);
            } else {
                self.face(i, 3 * TICK_HZ as i32);
            }
        }
    }

    /// `FUN_00159190(i, kind, duration)`: a face for `duration` updates
    /// (actor+0x240, +0x244), its clip drawn (`r %` the kind's count at
    /// `0x44f750`; the faces are not played, their draws kept). None for
    /// a duration under 1.
    pub(crate) fn face(&mut self, i: usize, duration: i32) {
        if duration <= 0 {
            return;
        }
        self.players[i].face_wait = duration;
        self.rng.next_u32();
    }

    /// `FUN_00159240` (the actor's update after his side's, `0x157270`;
    /// none under the script manager): his face's time counts down, and at
    /// 0 the idle face (kind 0) for `fps + r % fps` updates, its clip drawn.
    fn idle_face(&mut self, i: usize) {
        let p = &mut self.players[i];
        p.face_wait = (p.face_wait - 1).max(0);
        if p.face_wait != 0 {
            return;
        }
        let fps = TICK_HZ;
        let r = self.rng.next_u32();
        self.face(i, (fps + r % fps) as i32);
    }

    /// Actor+0x1a8 and +0x1cc count down each update (`FUN_00156f48`,
    /// `0x157300..0x157314`), after his press dispatch and his side's tests
    /// read them: a bar another actor or the ball set since his last update
    /// holds his next tests at its whole length.
    fn count_down_bars(&mut self, i: usize) {
        self.players[i].barred = self.players[i].barred.saturating_sub(1);
        self.players[i].cooldown = self.players[i].cooldown.saturating_sub(1);
    }

    /// The end of the game's basket listener (`FUN_00141818`, raw
    /// `0x141968..0x141c08`), after the restart or the win: every actor
    /// barred [`BASKET_BAR`] updates (actor+0x1a8; nobody takes the ball as
    /// it comes down through the net, the restart's inbounder included),
    /// then a face for each (`FUN_00159190(actor, kind, fps Ã— 3)`, its own
    /// draw for the clip; the faces are not played, their draws kept): the
    /// scorer's side by index first, the scorer's 5 or 2 by `r & 1` after
    /// a dunk or an alley-oop (the ball's kind 3 or 4), else 6, his
    /// teammates' 9 or 6 by `r & 1`; then the other side's, the scorer's
    /// defender's (`FUN_0016e8f8`) 2 after a dunk or an alley-oop, else 3
    /// or 4 by `r & 1`, the others' 4. Nine draws in all.
    fn basket_bars_and_faces(&mut self, scorer: usize, kind: u8) {
        for p in &mut self.players {
            p.barred = BASKET_BAR;
        }
        let team = self.players[scorer].team;
        let slam = matches!(kind, 3 | 4);
        let defender = self.holder_defender(scorer);
        let sides = [team, 1 - team.min(1)];
        for (k, &side) in sides.iter().enumerate() {
            for i in 0..self.players.len() {
                if self.players[i].team != side {
                    continue;
                }
                let picked = if k == 0 {
                    i != scorer || slam
                } else {
                    Some(i) == defender && !slam
                };
                if picked {
                    self.rng.next_u32();
                }
                self.face(i, 3 * TICK_HZ as i32);
            }
        }
    }

    /// The game won at `team`'s `basket`, the scores as they stand (the
    /// game's vt+0xbc after the basket's listeners): the points game's
    /// `FUN_0012e800` (a side at the target, two apart); the trick-point
    /// game's `FUN_0018c620`, won once its basket listener (`FUN_0018c450`,
    /// slot 26, after the meters' and the statistics' listeners have paid
    /// the basket's trick points) found the scorer's side past the target
    /// and the other side's ([`World::basket_would_win`], game+0x4b8);
    /// Gamebreaker Wins' `FUN_00145940` on its listener's flags (a Level 1
    /// basket +0x4b8, a Level 2 +0x4bc, `0x1456f0`); the lesson game's
    /// never (`0x190b80`).
    fn basket_wins(&self, team: usize, basket: Basket) -> bool {
        let (us, them) = (self.score[team], self.score[1 - team]);
        match (self.class(), self.rules.goal) {
            (game_class::Class::Lesson, _) => false,
            (_, Goal::Points(n)) => us >= n && us >= them + self.rules.win_by,
            (_, Goal::TrickPoints(_)) => self.basket_would_win(team, 0, 0),
            (_, Goal::Gamebreaker(level)) => {
                matches!(basket, Basket::Gamebreaker { level: l, .. } if l >= level)
            }
        }
    }

    /// `FUN_001406b8`: the restart (game phase 6) with `team` to inbound.
    /// Every ready flag goes down, the computer drives all six
    /// ([`World::restarting`]), a ball the other team holds is let go
    /// (`FUN_0014c408`), and every controller's turbo is topped up by a
    /// whole bar (`FUN_00190c68(1.0)` for `game+0xb0..0xc4`). Without the
    /// six, the rewrite's dead ball counts down to its own inbound.
    pub(crate) fn start_restart(&mut self, team: usize) {
        self.timed_dead_ball = false;
        self.game_phase.start_restart(team);
        self.possession = team;
        self.phase = Phase::DeadBall {
            left: if self.players.len() == 6 {
                tuning::RESTART_LIMIT
            } else {
                tuning::DEAD_BALL_TIME
            },
            team,
        };
        self.lower_ready_flags();
        // Game+0x4a4 is cleared: the meters wait for the next phase 4.
        self.meter_clock = street::MeterClock::Inbound;
        self.stats.rebound = false;
        if let Some(h) = self.handler()
            && self.players[h].team != team
        {
            self.ball.drop_loose();
        }
        self.fill_turbo();
    }

    /// The rewrite's timed dead ball, as the lessons stage it: after a
    /// second the player of `team` nearest its own basket is put on the
    /// baseline with the ball. The original's lesson launch
    /// (`FUN_0018ec28`) snaps the six to their restart spots instead
    /// (`FUN_001407c8`), which the rewrite's lessons do not follow.
    pub fn stage_dead_ball(&mut self, team: usize) {
        self.game_phase.start_restart(team);
        self.possession = team;
        self.phase = Phase::DeadBall { left: 1.0, team };
        self.timed_dead_ball = true;
    }

    /// The game's side of a lesson's launch (`FUN_0018ec28`, School vt+0xa8,
    /// which the lesson menu calls): `lesson` is the game's vt+0xb4, `team`
    /// the side it starts with the ball (`FUN_0018ebe0`). The restart for
    /// it (vt+0x14c, `FUN_001406b8`), its hold cleared (+0x104), both teams
    /// at their restart spots with the inbounder holding the ball
    /// (vt+0x154, `FUN_001407c8`; the row is 2 in a lesson), the shot clock
    /// full (vt+0xfc, `FUN_0013f188`); then the gamebreaker lessons' meters
    /// (`0x18ed4c..0x18ee70`): Gamebreaker (0xe) and Pocket (0x17) clear
    /// both pockets and fill `team`'s meter; Cancel (0x18) pockets the
    /// other side's and fills `team`'s; Level 2 (0x19) pockets and fills
    /// `team`'s; the rest clear both pockets. The restart then ends as any
    /// does, with the inbound pass.
    pub fn launch_lesson(&mut self, lesson: u8, team: usize, page: bool, inputs: &[Input]) {
        let team = team.min(1);
        self.lesson = Some(lesson);
        self.lesson_page = page;
        // Who the controllers drive (game+0x24, as the School's binding
        // left them), and team+0x88, the slots the computer drives, as the
        // AI's last frame leaves it: the School's game opens held on its
        // lesson menu, so no frame has run before the first launch, and
        // the restart's strategies below read it (a slot taken for a
        // person keeps no walk when the computer then drives it, and never
        // readies). `inputs` empty keeps the world's own.
        if !inputs.is_empty() {
            self.set_people(inputs);
        }
        for side in 0..2 {
            let squad: Vec<usize> = (0..self.players.len())
                .filter(|&i| self.players[i].team.min(1) == side)
                .collect();
            let computer = std::array::from_fn(|k| {
                squad
                    .get(k)
                    .is_none_or(|&i| !self.people.get(i).copied().unwrap_or(false))
            });
            self.ai.coaches[side].set_computer(computer);
        }
        self.start_restart(team);
        self.game_phase.hold = 0;
        self.snap_formation(team);
        self.refill_shot_clock();
        // `FUN_001448e0(50000)` for the filled meter, its hold starting.
        let full = street::METER_FULL as f32;
        match lesson {
            0xe | 0x17 => {
                self.teams.iter_mut().for_each(|t| t.pocketed = false);
                self.set_meter(team, full);
            }
            0x18 => {
                self.teams[1 - team].pocketed = true;
                self.teams[team].pocketed = false;
                self.set_meter(team, full);
            }
            0x19 => {
                self.teams[team].pocketed = true;
                self.set_meter(team, full);
            }
            _ => self.teams.iter_mut().for_each(|t| t.pocketed = false),
        }
    }

    /// `FUN_0013e4c8`'s way out of phase 6: phase 4 (live play), the
    /// players free again (`FUN_00140880(0)`). The inbound is announced
    /// for the passer; the shot clock starts full (the rewrite's place for
    /// it).
    fn end_restart(&mut self, team: usize) {
        let player = match (self.ball.state, self.ball.pass) {
            (BallState::Pass, Some(pass)) => pass.from,
            _ => {
                let order = self.restart_plan().order;
                (0..self.players.len())
                    .filter(|&i| self.players[i].team.min(1) == team.min(1))
                    .nth(order[0])
                    .unwrap_or(0)
            }
        };
        self.phase = Phase::Live;
        self.opening_toss = OpeningToss::None;
        self.events.push(Event::Inbound { player });
    }

    /// `team` puts the ball in play: whoever of them is nearest their own
    /// basket takes it from the baseline under it.
    fn inbound(&mut self, team: usize) {
        self.timed_dead_ball = false;
        let hoop = defended_hoop(team);
        let rim = self.rim_spot(hoop);
        let Some(i) = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.team == team)
            .min_by(|a, b| distance(a.1.position, rim).total_cmp(&distance(b.1.position, rim)))
            .map(|(i, _)| i)
        else {
            return;
        };
        let p = &mut self.players[i];
        p.position = inbound_spot(rim);
        p.set_heading(if rim[0] < 0.0 {
            std::f32::consts::FRAC_PI_2
        } else {
            -std::f32::consts::FRAC_PI_2
        });
        p.speed = 0.0;
        p.action = Action::None;
        self.phase = Phase::Live;
        self.game_phase.end_restart();
        self.opening_toss = OpeningToss::None;
        self.possession = team;
        self.take_ball(i);
        self.events.push(Event::Inbound { player: i });
    }

    /// Native FUN_001420e8 / FUN_0012c748 formation restart: resets all 6 players
    /// to their tactical formation coordinates and 10-bit headings, clears movement,
    /// and assigns the ball to possessing team slot 0 (`possessing_team * 3`).
    ///
    /// Nothing in the USA ELF reaches it: `FUN_001420e8` is the game
    /// listener's slot `+0x1b4`, called only by the accept function
    /// `FUN_00105988` of the record descriptor `0x3c67d0` (and its copies),
    /// and no code builds a record with any of those descriptors. Restarts
    /// use [`World::snap_formation`] instead.
    pub fn restart_formation(&mut self, possessing_team: usize) {
        if self.players.len() < 6 {
            self.inbound(possessing_team);
            return;
        }
        let order = ordinary::restart_order(possessing_team);
        for (actor_idx, _team, _slot, spot) in order {
            if let Some(player) = self.players.get_mut(actor_idx) {
                player.position = [spot.position[0], spot.position[2]];
                // The 10-bit heading in the simulation's convention.
                player.set_heading(restart::heading_radians(spot.heading));
                player.speed = 0.0;
                player.action = Action::None;
            }
        }
        let inbounder = possessing_team * 3;
        self.phase = Phase::Live;
        self.game_phase.end_restart();
        self.opening_toss = OpeningToss::None;
        self.possession = possessing_team;
        self.take_ball(inbounder);
        self.events.push(Event::Inbound { player: inbounder });
    }

    /// The six-player snap after a replay (`FUN_00142648`, the game
    /// listener's replay-end slot `+0x1dc`): while a restart is pending
    /// (game phase 6) and no gamebreaker task is busy (`FUN_00140ff8`),
    /// `FUN_001407c8` runs `FUN_0018adf8` for the possessing team and then
    /// the other one: the team's restart strategy becomes its strategy and
    /// starts (the possessing team's `FUN_0012bc40` choosing its play from
    /// where everyone stands), the three stand at their spots
    /// (`FUN_0012bb50`, `FUN_001704e0`), and the possessing team's
    /// inbounder is handed the ball (`FUN_0014cb28`). See [`restart`] for
    /// the spots. The restart then finishes as any does: phase 6 lasts
    /// until all six are ready and the ball is passed in.
    pub fn snap_formation(&mut self, possessing_team: usize) {
        if self.players.len() != 6 {
            self.inbound(possessing_team);
            return;
        }
        let possessing_team = possessing_team.min(1);
        let actors: [[usize; 3]; 2] = std::array::from_fn(|team| {
            let mut slots = [usize::MAX; 3];
            let mut k = 0;
            for (i, p) in self.players.iter().enumerate() {
                if p.team.min(1) == team && k < 3 {
                    slots[k] = i;
                    k += 1;
                }
            }
            slots
        });
        if actors.iter().flatten().any(|&i| i == usize::MAX) {
            self.inbound(possessing_team);
            return;
        }
        if !matches!(self.phase, Phase::DeadBall { team, .. } if team == possessing_team) {
            self.start_restart(possessing_team);
        }
        self.timed_dead_ball = false;
        // The rewrite's limit counts from the snap.
        self.phase = Phase::DeadBall {
            left: tuning::RESTART_LIMIT,
            team: possessing_team,
        };
        let plan = self.start_restart_strategies(possessing_team);
        let heights: [[f32; 3]; 2] =
            std::array::from_fn(|team| actors[team].map(|i| self.players[i].height_ft()));
        // FUN_001407c8: the possessing team first.
        for team in [possessing_team, 1 - possessing_team] {
            for (slot, &actor) in actors[team].iter().enumerate() {
                let spot = restart::spot(&plan, possessing_team, team, slot, &heights);
                let p = &mut self.players[actor];
                p.position = [spot.position[0], spot.position[2]];
                p.set_heading(restart::heading_radians(spot.heading));
                p.speed = 0.0;
                p.height = 0.0;
                p.action = Action::None;
            }
        }
        let inbounder = actors[possessing_team][plan.order[0]];
        self.opening_toss = OpeningToss::None;
        self.give_and_go = None;
        self.possession = possessing_team;
        // A carried ball is let go first (`FUN_0014c408`).
        self.ball.drop_loose();
        self.take_ball(inbounder);
    }

    /// The original's opening (`FUN_00141fd8`, game phase 1): every player
    /// at his tip-off spot facing the centre ([`toss::SPOTS`], the k-th
    /// player of a team in slot k), the ball in the referee's hand
    /// ([`toss::RELEASE`]), and his toss [`toss::WAIT_UPDATES`] updates
    /// away. Play is live but nothing moves until the toss
    /// ([`World::step`]); an inbound or a formation restart cancels it.
    pub fn start_opening_toss(&mut self) {
        let mut slots = [0_usize; 2];
        for p in &mut self.players {
            let team = p.team.min(1);
            let slot = slots[team];
            slots[team] += 1;
            if let Some(&[x, z]) = toss::SPOTS.get(slot) {
                p.position = [x * restart::side(team), z];
                // FUN_00170608 turns him to face `0x4b9880`, the origin.
                p.set_heading(math::atan2(-p.position[0], -p.position[1]));
            }
            p.speed = 0.0;
            p.height = 0.0;
            p.action = Action::None;
        }
        let radius = self.ball.radius;
        let ceiling = self.ball.ceiling;
        self.ball = Ball::loose_at(toss::RELEASE);
        self.ball.radius = radius;
        self.ball.ceiling = ceiling;
        self.phase = Phase::Live;
        self.game_phase.start_tip();
        self.possession = 0;
        self.shot_clock = self.rules.shot_clock.unwrap_or(0.0);
        self.give_and_go = None;
        self.opening_toss = OpeningToss::Waiting(toss::WAIT_UPDATES);
        // The actors' reset for the match (`FUN_00156870`): game+0x94 too
        // (`0x1569c0`).
        self.fresh_take = false;
        self.tip = Tip {
            first_pass: true,
            ..Tip::default()
        };
    }

    /// The tip-off (`FUN_0015eaa0`) for player `i` this update, while the
    /// toss is up. `people` says which sides a person plays.
    ///
    /// A person's jumper times his jump with shoot (actor+0x7c `0x80`):
    /// with the toss in the top fifth of its rise his side's timing is
    /// 1..20 by how near the apex ([`toss::TIMING_SHARE`]). Once the toss
    /// comes down (ball+0x44 below 0) the first jumper over it (|x| under
    /// 5 after truncating) decides the contest: a side without a timing
    /// takes `r % 20 + 1` with nobody playing it, else 1, and the winner
    /// is the side with the larger `height + Rebounds Ã— timing`, a tie
    /// going to team 0. The original reads both scores from actor 0's
    /// record (`0x15ecf0`: its loop index is the best so far, -1 then 0,
    /// and `FUN_001408e0(game, -3)` is actor 0), so only the timings
    /// decide unless actor 0's Rebounds is 0. Both jumpers start their
    /// scripts ([`toss::TIP_SCRIPTS`]): the winner his, the loser one of
    /// two by `r & 1`, and each jumps at the script's `a1` ([`World::
    /// tip_jump`]). The winner tips the ball once his hand is within
    /// [`toss::TIP_REACH`] of it: caught and passed at once
    /// (`FUN_0014cb08`, `FUN_0015f930`) to his side's slot 2 (actor+0x74 =
    /// actor 2 or 5, `FUN_001587d8` in phase 1), standing behind him, a
    /// first pass of [`FIRST_PASS_FLIGHT`] updates. The losers
    /// cannot catch it (actor+0x1a8 is the toss's updates left,
    /// `0x15ee20`).
    fn tip_off(&mut self, i: usize, pressed: u16, people: [bool; 2]) {
        if self.opening_toss != OpeningToss::Flight || self.ball.state != BallState::Loose {
            return;
        }
        let team = self.players[i].team.min(1);
        let squad: Vec<usize> = (0..self.players.len())
            .filter(|&j| self.players[j].team.min(1) == team)
            .collect();
        if squad.first() != Some(&i) {
            return;
        }
        // Ball+0x118, the gravity's size an update (positive: the apex
        // `y + vy t - g t²/2` at `t = vy / g` is above a rising toss).
        let g = GRAVITY * TICK_SECONDS * TICK_SECONDS;
        let y = self.ball.position[1];
        let vy = self.ball.velocity[1] * TICK_SECONDS;
        if self.tip.winner < 0
            && people[team]
            && self.tip.timing[team] < 0
            && pressed & button::SHOOT != 0
        {
            let t = vy / g;
            let apex = y + vy * t - g * 0.5 * t * t;
            let low = apex * toss::TIMING_SHARE;
            if low <= y {
                // `cvt.w.s`, which truncates (`0x15ebc4`).
                let k = ((y - low) / (apex - low) * 20.0) as i32 + 1;
                self.tip.timing[team] = k.clamp(1, 20);
            }
        }
        // |`cvt.w.s(x)`| under 5 (`0x15ec68`): the truncation keeps |x| < 5.
        if vy < 0.0 && (self.players[i].position[0] as i32).abs() < 5 && !self.tip.jumped[team] {
            if self.tip.winner < 0 {
                let first = (0..self.players.len()).find(|&j| self.players[j].team.min(1) == 0);
                let (height, rebounds) = first.map_or((0, 0), |j| {
                    let p = &self.players[j];
                    (
                        (p.height_ft() * 12.0).round() as i32,
                        (p.ratings.rebounds * 99.0).round() as i32,
                    )
                });
                let (mut best, mut winner) = (-1, 0);
                for (k, &person) in people.iter().enumerate() {
                    if self.tip.timing[k] < 0 {
                        self.tip.timing[k] = if person {
                            1
                        } else {
                            (self.rng.next_u32() % 20) as i32 + 1
                        };
                    }
                    let score = height + rebounds * self.tip.timing[k];
                    if best < score {
                        best = score;
                        winner = k as i32;
                    }
                }
                self.tip.winner = winner * 3;
            }
            self.tip.jumped[team] = true;
            // The winner's script, or the loser's, one of two.
            let script = if self.tip.winner == team as i32 * 3 {
                0
            } else {
                1 + (self.rng.next_u32() & 1) as u8
            };
            if self.players[i].free() {
                self.players[i].action = Action::TipJump {
                    script,
                    tick: 0,
                    vy: 0.0,
                    drift: [0.0; 2],
                    up: false,
                };
                self.events.push(Event::Jump {
                    player: i,
                    block: false,
                });
            }
        }
        if let Action::TipJump { tick, .. } = self.players[i].action
            && tick == toss::JUMP_TICK
        {
            self.tip_jump(i);
        }
        let reach = |hand: [f32; 3]| {
            (0..3)
                .map(|k| (hand[k] - self.ball.position[k]).powi(2))
                .sum::<f32>()
                .sqrt()
                <= toss::TIP_REACH
        };
        let near = match self.tip_hands(i) {
            Some(hands) => hands.into_iter().any(reach),
            None => reach(self.tip_hand(i)),
        };
        if self.tip.winner == team as i32 * 3 && near {
            let Some(&to) = squad.get(2) else {
                return;
            };
            self.take_ball(i);
            let from = self.ball.position;
            let aim = self.pass_aim(i, to);
            let start = self.floor_positions();
            self.launch(i, to, PassStyle::Normal, aim, &start, Some(from));
            self.events.push(Event::Pass {
                from: i,
                to,
                style: PassStyle::Normal,
            });
        }
    }

    /// The rewrite's hand for jumper `i`'s tip: his standing reach above
    /// his feet ([`Ratings::reach`]), where the original poses the hand
    /// bones.
    fn tip_hand(&self, i: usize) -> [f32; 3] {
        let p = &self.players[i];
        [p.position[0], p.height + p.ratings.reach, p.position[1]]
    }

    /// The tip jumper's hand bones (`0x15eec4`, bones 0x13 and 0xf) on his
    /// script's clip as it stands at his tick: the blend's three updates on
    /// the clip's first frame, then its frame toward the wait's
    /// ([`toss::JUMP_START_FRAMES`]) over five updates, from the jump on
    /// toward [`toss::JUMP_FRAMES`] over [`toss::JUMP_UPDATES`]; at his
    /// place and facing. `None` without the disc's animations or a jump.
    fn tip_hands(&self, i: usize) -> Option<[[f32; 3]; 2]> {
        let poses = self.poses.get()?;
        let p = &self.players[i];
        let Action::TipJump { script, tick, .. } = p.action else {
            return None;
        };
        let k = usize::from(script);
        let record = *toss::JUMP_RECORDS.get(k)?;
        let (start, aim) = (toss::JUMP_START_FRAMES[k], toss::JUMP_FRAMES[k]);
        let t = f32::from(tick);
        let blend = f32::from(toss::BLEND_TICKS);
        let jump = f32::from(toss::JUMP_TICK);
        let time = if t < blend {
            0.0
        } else if t < jump {
            start * (t - blend) / (jump - blend)
        } else {
            (start + (aim - start) * (t - jump) / toss::JUMP_UPDATES).min(aim)
        };
        let place = pose::Place {
            position: [p.position[0], p.height, p.position[1]],
            word: timing::facing_word(p.heading),
        };
        let at = pose::Frame {
            record,
            time,
            looping: false,
        };
        let posed = poses.pose(at, p.body(), place)?;
        Some([
            posed.joints[pose::RIGHT_HAND].position,
            posed.joints[pose::LEFT_HAND].position,
        ])
    }

    /// The hand a tip jump of `script` aims (`0x19537c..0x195570`): the
    /// channel's clip ([`toss::JUMP_RECORDS`]) posed at the script's frame
    /// ([`toss::JUMP_FRAMES`], `FUN_0019bda8`) where he stands, turned
    /// toward the ball (`FUN_0016f2b0`, the heading of the actor less the
    /// ball, `FUN_0018ce78`'s form); the right hand (bone 0x13) when the
    /// pose's flags hold 8, the left (0xf) at 4, else their midpoint.
    /// Without the disc's animations, [`World::tip_hand`].
    fn tip_aim_hand(&self, i: usize, script: usize) -> [f32; 3] {
        let stand_in = self.tip_hand(i);
        let Some(poses) = self.poses.get() else {
            return stand_in;
        };
        let (Some(&record), Some(&frame)) = (
            toss::JUMP_RECORDS.get(script),
            toss::JUMP_FRAMES.get(script),
        ) else {
            return stand_in;
        };
        let p = &self.players[i];
        let feet = [p.position[0], p.height, p.position[1]];
        let place = pose::Place {
            position: feet,
            word: ordinary::native_heading(feet, self.ball.position),
        };
        let at = pose::Frame {
            record,
            time: frame,
            looping: false,
        };
        let Some(posed) = poses.pose(at, p.body(), place) else {
            return stand_in;
        };
        let right = posed.joints[pose::RIGHT_HAND].position;
        let left = posed.joints[pose::LEFT_HAND].position;
        match poses.state(record, frame as i32) & 0xc {
            8 => right,
            4 => left,
            _ => std::array::from_fn(|k| (right[k] + left[k]) * 0.5),
        }
    }

    /// A tip-off script's `a1 ef 25.0 F -1.0` (`FUN_00194d58`, raw
    /// `0x19537c..0x1956a4`, then `0x195728..0x195784`): the jumper leaves
    /// the floor so that his hand, posed at frame F, meets the ball
    /// [`toss::JUMP_UPDATES`] updates on. The ball's place then is its own
    /// path stepped ahead (`FUN_001537d0`; here its free flight, which the
    /// toss is until the tip); the velocity is that place less the hand's
    /// over the updates, plus half the actor's gravity times them upward
    /// (`0x195618..0x1956a4`), and the jump lasts until he lands
    /// (`FUN_0019bed8` at actor+0x64). The hand is the clip's, posed
    /// ([`World::tip_aim_hand`]).
    fn tip_jump(&mut self, i: usize) {
        let n = toss::JUMP_UPDATES;
        let g_ball = GRAVITY * TICK_SECONDS * TICK_SECONDS;
        let b = self.ball.position;
        let v = self.ball.velocity.map(|c| c * TICK_SECONDS);
        let ahead = [
            b[0] + v[0] * n,
            b[1] + v[1] * n - 0.5 * g_ball * n * n,
            b[2] + v[2] * n,
        ];
        let script = match self.players[i].action {
            Action::TipJump { script, .. } => usize::from(script),
            _ => 0,
        };
        let hand = self.tip_aim_hand(i, script);
        let g = toss::JUMP_GRAVITY;
        if let Action::TipJump { vy, drift, up, .. } = &mut self.players[i].action {
            *drift = [(ahead[0] - hand[0]) / n, (ahead[2] - hand[2]) / n];
            *vy = (ahead[1] - hand[1]) / n + 0.5 * g * n;
            *up = true;
        }
    }

    /// `FUN_0014c858`: the toss leaves for a [`toss::APEX`] apex and comes
    /// down on the centre spot. With the per-update gravity `g`, it rises
    /// for `t_up = sqrt(2 (20 - y) / g)` updates and falls for
    /// `sqrt(2 (20 - r) / g)` to its resting height `r`; the floor speeds
    /// are `-x / T` and `-z / T` over the whole flight `T`, the rise speed
    /// `t_up * g`, all per update. It spins at [`toss::SPIN`] about a
    /// random axis (raw `0x14c8bc..0x14ca6c`): three draws of the shared
    /// generator (`FUN_003a16b8`), each `(d & 0x7fffffff) * 2^-31 - 0.5`,
    /// normalized by the VU0 (here in plain float arithmetic), or
    /// [`toss::SPIN_FALLBACK`] when the vector's length is at most 1e-4.
    fn launch_opening_toss(&mut self) {
        let g = GRAVITY * TICK_SECONDS * TICK_SECONDS;
        let [x, y, z] = self.ball.position;
        let rise = toss::APEX - y;
        let up = ((rise + rise) / g).sqrt();
        let fall = toss::APEX - self.ball.radius;
        let down = ((fall + fall) / g).sqrt();
        let k = -1.0 / (down + up);
        let per_update = [x * k, up * g, z * k];
        let draw = |rng: &mut rng::Rng| {
            (rng.next_u32() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000) - 0.5
        };
        let axis = [
            draw(&mut self.rng),
            draw(&mut self.rng),
            draw(&mut self.rng),
        ];
        let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        self.ball.spin_axis = if length <= f32::from_bits(0x38d1_b717) {
            toss::SPIN_FALLBACK
        } else {
            axis.map(|c| c / length)
        };
        self.ball.spin = toss::SPIN;
        self.ball.state = BallState::Loose;
        self.ball.shot = None;
        self.ball.pass = None;
        self.ball.velocity = per_update.map(|v| v * TICK_HZ as f32);
        self.ball.airborne = true;
        self.ball.through = false;
        self.ball.bank = false;
        self.ball.armed = true;
        self.ball.dunked = false;
        self.opening_toss = OpeningToss::Flight;
    }

    /// Authoritative native self-toss trick launch (callback 53, FUN_001741a8/FUN_0014bff8).
    /// Transitions ball to BallState::Pass (state 4) with from == to == player, targeting
    /// the native sampled rig destination over the callback flight duration (e.g. 28 updates for script 69).
    pub(crate) fn self_toss_launch(&mut self, player: usize, script: u16) {
        if self.ball.state != BallState::Held(player) {
            return;
        }
        let Some(cue) = self_toss::cue_for_script(script) else {
            return;
        };
        let p = &self.players[player];
        let from = self.ball.position;
        let target = self_toss::target_for_player(p.position, p.heading, p.height, script);
        let flight_updates = cue.flight_updates(TICK_HZ as i32);
        let duration = flight_updates as f32 * TICK_SECONDS;
        if cue.bounce {
            if let Some(velocity) =
                cue.bounce_velocity(from, target, self.ball.radius, ball::FLOOR_RESTITUTION)
            {
                self.ball.throw_with_velocity(
                    from,
                    velocity,
                    duration,
                    Pass {
                        from: player,
                        to: player,
                        tried: u8::MAX,
                        via: ball::Via::Floor,
                        catch: ball::NO_CATCH,
                        since: self.tick,
                    },
                );
            }
        } else {
            self.ball.throw(
                from,
                target,
                duration,
                Pass {
                    from: player,
                    to: player,
                    tried: u8::MAX,
                    via: ball::Via::Direct,
                    catch: ball::NO_CATCH,
                    since: self.tick,
                },
            );
        }
    }

    fn step_ball(&mut self, held_before: Option<(usize, [f32; 3])>) {
        match self.ball.state {
            // `FUN_0014e060`: the ball on whatever clip the holder's channel
            // shows ([`held`]: his gait's dribble, a trick, a shot, a dunk,
            // a lay-up, a pass, a catch row); without the disc's animations
            // the rewrite carries it at his waist.
            BallState::Held(i) => {
                if !self.place_held(i, held_before) {
                    self.held_anchor = None;
                    self.ball.carry(&self.players[i]);
                }
            }
            BallState::Shot => {
                if self.phase == Phase::Live && self.check_goaltend() {
                    return;
                }
                let rims = self.hoops.map(|h| h.rim);
                let (shot, rim) = self.ball.fly(rims, self.sequence.is_some());
                if rim {
                    self.rim_touched();
                }
                if let Some(shot) = shot {
                    if shot.made {
                        let long = shot.points == 2;
                        let basket = match (shot.slam, shot.gamebreaker, shot.layup) {
                            (Some(slam), _, _) => slam,
                            (None, 0, true) => Basket::Layup,
                            (None, 0, false) => Basket::Jumper { long },
                            (None, level, _) => Basket::Gamebreaker {
                                level,
                                long,
                                dunk: false,
                            },
                        };
                        self.score_basket(shot.shooter, basket);
                    } else {
                        self.events.push(Event::RimOut);
                        self.missed_faces(shot.shooter);
                    }
                }
            }
            BallState::Pass => {
                let armed = self.ball.armed;
                self.step_pass();
                if armed && !self.ball.armed && self.ball.state != BallState::Shot {
                    // A pass, or the ball it knocked loose, down through
                    // the ring (`FUN_001515a8`, event 0x55).
                    if !matches!(self.ball.state, BallState::Held(_)) {
                        self.loose_basket();
                        return;
                    }
                }
                // The tip-off is caught: the opening phase ends
                // (`FUN_001415c0`, `FUN_0013e4c8`).
                if self.opening_toss == OpeningToss::Flight
                    && matches!(self.ball.state, BallState::Held(_))
                {
                    self.opening_toss = OpeningToss::None;
                }
            }
            BallState::Loose => {
                // `FUN_0014a968`: a ball nobody holds runs `FUN_0014d1f8`,
                // floor, backboard, collar, rim and net, whatever its state;
                // a rebound (0) that meets the floor or the ceiling is
                // loose (1), on the hoop it stays one (`0x14d72c..0x14d784`).
                let armed = self.ball.armed;
                let contact = self
                    .ball
                    .step(self.hoops.map(|h| h.rim), ball::Step::default());
                if contact.rim {
                    self.rim_touched();
                }
                if contact.floor {
                    self.ball.rebound = false;
                }
                // The inbounder fetches the ball ("loose ball", mode 3, from
                // `FUN_0012bc40`): with the disc's animations the restarting
                // side's own tail takes it ([`touch`], `0x15c3c8`; the
                // other side's update leaves a loose ball in phase 6);
                // without them the rewrite's reach stands in.
                if let Phase::DeadBall { team, .. } = self.phase
                    && self.restarting()
                    && self.poses.get().is_none()
                    && let Some(i) = self.catcher(Some(team))
                {
                    self.take_ball(i);
                    self.events.push(Event::Pickup { player: i });
                    return;
                }
                // `FUN_001515a8`'s basket outside a shot (event 0x55).
                if contact.scored && armed && !self.ball.armed {
                    self.loose_basket();
                    return;
                }
                if self.phase == Phase::Live
                    && let Some(i) = self.catcher(None)
                {
                    self.take_ball(i);
                    if self.opening_toss == OpeningToss::Flight {
                        // The tip-off is caught: the opening phase ends
                        // (`FUN_0013ee10`). Nobody rebounded anything.
                        self.opening_toss = OpeningToss::None;
                    }
                    self.events.push(Event::Pickup { player: i });
                }
            }
        }
    }

    /// The nearest player (of `team`, if given) in reach of a loose ball:
    /// low enough to grab, or within reach of a jump.
    fn catcher(&self, team: Option<usize>) -> Option<usize> {
        let b = self.ball.position;
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| team.is_none_or(|t| p.team == t))
            // `FUN_0015da28`: a barred actor (+0x1a8) takes nothing; a block
            // jumper's script ([`block`]) takes only the floor's ball.
            .filter(|(_, p)| {
                p.barred == 0
                    && !matches!(p.action, Action::Jumping { block: Some(_), .. } if p.height > 0.0)
            })
            // With the disc's animations the actors' own tests take a loose
            // ball in live play ([`touch`], the dive's [`dive`]); the
            // rewrite's reach stays for its own jump.
            .filter(|(_, p)| {
                team.is_some()
                    || self.poses.get().is_none()
                    || self.opening_toss != OpeningToss::None
                    || matches!(p.action, Action::Jumping { block: None, .. })
            })
            .filter_map(|(i, p)| {
                let (reach, range) = match p.action {
                    Action::None => (
                        tuning::PICKUP_HEIGHT,
                        tuning::PICKUP_RANGE + 0.6 * p.ratings.rebounds,
                    ),
                    Action::Jumping { .. } => (
                        p.ratings.reach + p.height,
                        tuning::PICKUP_RANGE + 0.6 * p.ratings.rebounds,
                    ),
                    _ if p.leaping() => (
                        p.ratings.reach + p.height,
                        tuning::PICKUP_RANGE + 0.6 * p.ratings.rebounds,
                    ),
                    Action::Dive { .. } => (tuning::PICKUP_HEIGHT, tuning::DIVE_PICKUP),
                    _ => return None,
                };
                let d = distance(p.position, [b[0], b[2]]);
                (b[1] < reach && d < range).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// game+0x98 by side: 1 for a side attacking +x, -1 for -x.
    pub(crate) fn sides(&self) -> [i32; 2] {
        [0, 1].map(|team| {
            if self.hoops[target_hoop(team)].rim[0] > 0.0 {
                1
            } else {
                -1
            }
        })
    }

    /// The ball's state as the original's game reads it (ball+0x180,
    /// [`game_phase::ball`]): a pass from its press (`FUN_0014bff8` sets
    /// state 4 with the ball still in the passer's hands), the referee's
    /// toss, a shot that has touched the hoop (6, [`Ball::touched`]) and
    /// its ball come off as a rebound (0, [`Ball::rebound`]) apart from one
    /// let go, knocked away or on the floor (1).
    pub(crate) fn native_ball_state(&self) -> i32 {
        use game_phase::ball;
        match self.ball.state {
            BallState::Held(_) if self.windup.is_some() => ball::PASS,
            BallState::Held(_) => ball::HELD,
            BallState::Pass => ball::PASS,
            BallState::Shot if self.ball.touched => ball::RIM,
            BallState::Shot => ball::SHOT,
            BallState::Loose if self.opening_toss == OpeningToss::Flight => ball::TOSS,
            BallState::Loose if self.ball.rebound => ball::REBOUND,
            BallState::Loose => ball::LOOSE,
        }
    }

    /// What the game's phase reads this update ([`game_phase::View`]).
    /// `won`: the game was won before this update.
    fn phase_view(&self, won: bool) -> game_phase::View {
        use game_phase::ball;
        // A basket this update: the hoop posts it, and the net catches
        // the ball (state 5 to 1), after the game's update.
        let scored = self.events.iter().any(|e| matches!(e, Event::Score { .. }));
        let state = match self.native_ball_state() {
            ball::LOOSE if scored => ball::SHOT,
            state => state,
        };
        let holder = self.handler();
        // vt+0x164 (`FUN_0013ecd8`): not passed, shot or on the rim, and
        // the holder not in a shot, dunk or lay-up (`FUN_0016e990`,
        // `FUN_0016e9d0`, `FUN_0016ea40`, `FUN_0016e9e0`).
        let settled = !matches!(state, ball::PASS | ball::SHOT | ball::RIM)
            && holder.is_none_or(|h| {
                !matches!(
                    self.players[h].action,
                    Action::Shooting { .. } | Action::Dunking { .. } | Action::Layup { .. }
                )
            });
        game_phase::View {
            ball: state,
            ball_x: self.ball.position[0],
            ball_y: self.ball.position[1],
            holder: holder.map(|h| (self.players[h].team.min(1), self.players[h].position[0])),
            sides: self.sides(),
            ready: self.restarting() && self.restart_ready(),
            brought_up: self.meter_clock == street::MeterClock::On,
            won,
            settled,
            scripted: self.sequence.is_some(),
        }
    }

    /// The game's update (`FUN_0013d868`) for its phase, after the players
    /// and the ball have moved and before the shot clock (vt+0x104) and
    /// the meters (vt+0x174): a restart ends (phase 6 to 4,
    /// `FUN_00140880(0)`) and game+0x4a4 is set here.
    fn step_game_phase(&mut self, won: bool) {
        let view = self.phase_view(won);
        let changes = self.game_phase.update(&view);
        if changes.brought_up {
            self.meter_clock = street::MeterClock::On;
        }
        if changes.restart_over
            && let Phase::DeadBall { team, .. } = self.phase
        {
            self.end_restart(team);
        }
        if changes.celebrate
            && let Some(s) = self.scorer
        {
            self.players[s].celebrate = true;
        }
        if changes.won
            && let Phase::Over { winner } = self.phase
        {
            self.won_celebration(winner);
        }
        if changes.won_update {
            self.won_countdown();
        }
    }

    /// The rewrite's own: a [`World::phase`] set from outside the game's
    /// transitions (a test, a capture's replay, a lesson's staging) takes
    /// the game's phase with it as the transition there would have: a
    /// dead ball is a restart for its team, and live play out of a
    /// restart or a finished game resumes where the ball is.
    fn sync_game_phase(&mut self) {
        use game_phase::phase;
        match self.phase {
            Phase::DeadBall { team, .. } if self.game_phase.phase != phase::RESTART => {
                self.game_phase.start_restart(team);
            }
            Phase::Live
                if matches!(
                    self.game_phase.phase,
                    phase::PREGAME | phase::RESTART | phase::OVER | phase::WON
                ) =>
            {
                if self.opening_toss != OpeningToss::None {
                    self.game_phase.start_tip();
                    return;
                }
                let x = self
                    .handler()
                    .map_or(self.ball.position[0], |h| self.players[h].position[0]);
                let state = self.native_ball_state();
                let sides = self.sides();
                self.game_phase
                    .resume(self.possession.min(1), state, x, sides);
            }
            _ => {}
        }
    }
}

/// Where the inbounder stands: on the baseline beside the basket.
pub fn inbound_spot(rim: [f32; 2]) -> [f32; 2] {
    [43.0_f32.copysign(rim[0]), rim[1] + 4.0]
}

/// Distance between two points on the floor.
pub fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    math::len(a[0] - b[0], a[1] - b[1])
}

pub(crate) fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn normalize2(v: [f32; 2]) -> [f32; 2] {
    let n = math::len(v[0], v[1]);
    if n < 1e-6 {
        [1.0, 0.0]
    } else {
        [v[0] / n, v[1] / n]
    }
}

/// Wrap an angle to -pi..=pi.
fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = (a + PI).rem_euclid(TAU) - PI;
    if a < -PI { a + TAU } else { a }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod played_capture;

#[cfg(test)]
mod row_capture;
