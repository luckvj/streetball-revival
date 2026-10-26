//! A player's movement, turbo bar and actions.

use crate::{TICK_SECONDS, math, tuning, wrap_angle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
    Turbo,
}

/// The kinds of dunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DunkStyle {
    Normal,
    /// All four turbos: worth more, and misses more for poor dunkers.
    Advanced,
    /// Off a rebound caught in the air.
    Putback,
}

/// Direct trick-slot masks and value classes at 0x43d408 (SLUS-20651).
/// The native selector tests mask inclusion in this order (0015f2d8).
pub const TRICK_SLOT_MASKS: [u32; 11] = [
    0x3c000, 0x1c000, 0x2c000, 0x34000, 0x38000, 0xc000, 0x14000, 0x24000, 0x18000, 0x28000,
    0x30000,
];
pub const TRICK_SLOT_LEVELS: [u8; 11] = [4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2];

/// A trick move's identity: its direct slot ([`crate::trick::DRAWN`] for
/// one the draw picked), its script (an id into the 75 trick scripts of
/// `0x3d7238`, [`crate::trick::SCRIPTS`]) and its value class. Equipped
/// row numbers are XDBMOVE indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickIdentity {
    pub slot: u8,
    pub script: u16,
    /// Native actor+0x248 value class, indexing the five-entry scorer directly.
    pub level: u8,
    pub equipped_row: Option<u8>,
}

/// Match-owned direct-slot identities resolved from the player's disc/save.
/// A player without them plays the ordinary players' slots
/// ([`NativeTricks::ordinary`]); a press that matches no slot draws among
/// the 75 scripts ([`crate::World::start_trick`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTricks {
    pub slots: [TrickIdentity; 11],
}

impl NativeTricks {
    /// The ordinary players' slots (`FUN_00139850`'s row, `0x43d498`):
    /// what `FUN_0013a1f8` gives a record that is not a created player, the
    /// generic team's or a legend's slot 1.
    pub fn ordinary() -> Self {
        Self {
            slots: std::array::from_fn(|k| TrickIdentity {
                slot: k as u8,
                script: u16::from(crate::trick::SLOTS[k]),
                level: TRICK_SLOT_LEVELS[k],
                equipped_row: None,
            }),
        }
    }

    /// Only the four turbo bits participate; other action buttons may coexist.
    pub fn slot(buttons: u16) -> Option<usize> {
        let mask = Self::turbo_mask(buttons);
        TRICK_SLOT_MASKS
            .iter()
            .position(|&required| mask & required == required)
    }

    /// The jump shot's slot (`FUN_0016b4b8`, raw `0x16b760..0x16b7b0`):
    /// the first of the six two-turbo pairs at `0x43d460` (the last six of
    /// [`TRICK_SLOT_MASKS`]) the held turbos cover, 0..5, so three or four
    /// turbos take the first pair they hold.
    pub fn shot_slot(buttons: u16) -> Option<usize> {
        let mask = Self::turbo_mask(buttons);
        TRICK_SLOT_MASKS[5..]
            .iter()
            .position(|&required| mask & required == required)
    }

    /// The held turbos as the original's mask (L1 0x4000, R1 0x8000, L2
    /// 0x10000, R2 0x20000).
    fn turbo_mask(buttons: u16) -> u32 {
        [
            (crate::button::L1, 0x4000),
            (crate::button::R1, 0x8000),
            (crate::button::L2, 0x10000),
            (crate::button::R2, 0x20000),
        ]
        .into_iter()
        .filter(|&(button, _)| buttons & button != 0)
        .map(|(_, mask)| mask)
        .sum()
    }

    pub fn select(&self, buttons: u16) -> Option<TrickIdentity> {
        Self::slot(buttons).map(|slot| self.slots[slot])
    }
}

/// How long a trick runs: its script's updates ([`crate::trick::length`],
/// a self-toss's with its cue's flight), in seconds.
pub fn trick_seconds(id: TrickIdentity) -> f32 {
    f32::from(crate::trick::length(id.script).max(0) as u16) * crate::TICK_SECONDS
}

/// What the player is busy doing, if anything. Each action runs for a fixed
/// time; `elapsed` counts seconds since it started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    /// A jump shot at `hoop`; movement waits for it. `gamebreaker` is 0 for
    /// an ordinary shot, else the gamebreaker's level (1 or 2). A `fade`away
    /// drifts back from the basket, away from the defender.
    ///
    /// The clock is the shot script's (`crate::timing`): `tick` counts the
    /// updates since the press plus one (1 at the end of the press
    /// update), `script` is the id in [`crate::timing::SHOT_SCRIPTS`],
    /// `release` the script's callback 21, `check` its callbacks 17 and 20
    /// (the fade and the pump fake are decided there; 0 for none), `end`
    /// its last opcode, all in the script's own ticks: the script is at
    /// tick `tick - 1 - stall`, the press update being its tick 0.
    /// `gate` is the script's release gate (`86 19`, callback 19
    /// `FUN_00174c10`: a person's script waits there, an update at a time,
    /// while the shoot button is still held) and `latest` the update after
    /// the press the gate opens by itself (15 updates before the jump
    /// lands, 5 for the clips outside `0x3d9218`; the side fades' jump is
    /// callback 18's); `stall` counts the updates waited, so
    /// the ball leaves `release + stall` updates after the press. A
    /// computer's script never waits (the computer holds the button through
    /// the shot and `FUN_00172220` passes its controller).
    Shooting {
        hoop: usize,
        elapsed: f32,
        gamebreaker: u8,
        fade: bool,
        tick: u16,
        script: u8,
        release: u8,
        check: u8,
        end: u8,
        gate: u8,
        latest: u8,
        stall: u8,
    },
    /// Flying from `from` to `to` (beside the rim) to slam the ball. With
    /// `layup` set (shoot pressed again on the way up) it is laid in instead.
    /// `release` and `end` are the dunk script's callback 21 and last
    /// opcode in the script's ticks (the press update its tick 0), `tick`
    /// the updates since the press plus one, `row` the script's row
    /// ([`crate::timing::move_row`]: of [`crate::timing::MOVE_ROWS`], or
    /// past it a leap's; [`crate::timing::NO_MOVE_ROW`] for the putback,
    /// the sim's own). `hang` is callback 28's answer once
    /// the script asks it (`FUN_00172400`: he hangs on the rim after the
    /// slam, [`crate::trick::Step::Hang`]).
    Dunking {
        hoop: usize,
        elapsed: f32,
        from: [f32; 2],
        to: [f32; 2],
        style: DunkStyle,
        layup: bool,
        tick: u16,
        release: u8,
        end: u8,
        row: u8,
        hang: Option<bool>,
    },
    /// A driving lay-up from `from` to `to`, released at its script's
    /// callback 21 (`release` updates after the press) and over at `end`;
    /// `tick` the updates since the press plus one, `row` the script's row
    /// ([`crate::timing::move_row`]: of [`crate::timing::MOVE_ROWS`], or
    /// past it the board's lay-up and the mid-air ones).
    Layup {
        hoop: usize,
        elapsed: f32,
        from: [f32; 2],
        to: [f32; 2],
        tick: u16,
        release: u8,
        end: u8,
        row: u8,
    },
    /// A shot faked with a quick tap of the shoot button: the shot
    /// `script` (an id in [`crate::timing::SHOT_SCRIPTS`]) falls through
    /// its check to its end; `tick` goes on counting from the press.
    PumpFake {
        elapsed: f32,
        script: u8,
        tick: u16,
    },
    /// Throwing a pass; the player slows down meanwhile. `row` is the
    /// pass table row whose script plays ([`crate::timing::PASS_ROWS`];
    /// [`crate::timing::NO_PASS_ROW`] for a pass thrown without one),
    /// `tick` the updates since the press.
    Passing {
        elapsed: f32,
        row: u8,
        tick: u16,
    },
    /// Reaching for a steal, or with two turbos a trick counter. A swipe
    /// with the disc's animations plays its script ([`crate::swipe`]); the
    /// counter stance and the take after it play theirs (`stance`, always
    /// there for a counter, [`crate::counter`]).
    Stealing {
        elapsed: f32,
        counter: bool,
        swipe: Option<crate::swipe::Swipe>,
        stance: Option<crate::counter::Stance>,
    },
    /// A block jump (state bit 12). With the disc's animations it plays
    /// the original's script ([`crate::block`]); without them, the
    /// rewrite's stand-in jump, `high` with two turbos (its turbo block,
    /// which can also goaltend).
    Jumping {
        elapsed: f32,
        high: bool,
        block: Option<crate::block::Jump>,
    },
    /// A trick move with the ball, on its script ([`crate::trick`]): `id`
    /// the trick (a direct slot's or the draw's script and value class),
    /// `play` the script under way, which moves him
    /// ([`crate::World::trick_tick`]).
    Trick {
        id: TrickIdentity,
        play: crate::trick::Play,
    },
    /// Thrown off balance: no control for `duration`; or, with a
    /// `reaction`, a defender's reaction to a trick, for as long as its
    /// script runs ([`crate::World::reaction_tick`] moves him).
    Stumbling {
        elapsed: f32,
        duration: f32,
        reaction: Option<crate::trick::Reacting>,
    },
    /// A dive for a loose ball on its script (`script` of
    /// [`crate::timing::DIVE_SCRIPTS`], far or near; [`crate::dive`]),
    /// `tick` the updates since the press plus one: the hop toward where
    /// the ball comes down, the slide and getting up, [`crate::jump`]
    /// moving him.
    Dive {
        script: u8,
        tick: u16,
    },
    /// Scooping a low, slow ball ([`crate::touch::Scooping`]: script
    /// `0x45748a`, its callback 0 the take).
    Scoop {
        play: crate::touch::Scooping,
    },
    /// A tip-off jumper's script ([`crate::toss::TIP_SCRIPTS`]: 0 the
    /// winner's, 1 or 2 the loser's), `tick` updates since it started (that
    /// update its tick 0). At [`crate::toss::JUMP_TICK`] its `a1` sends
    /// him `up` at `vy` units an update, drifting `drift` an update on the
    /// floor, under [`crate::toss::JUMP_GRAVITY`] until he lands, where
    /// the rewrite lets him go (the script's play-out is the display's).
    TipJump {
        script: u8,
        tick: u16,
        vy: f32,
        drift: [f32; 2],
        up: bool,
    },
    /// A rebound jump (actor state bit 8, `FUN_0015ce50`): `script` its
    /// script ([`crate::rebound::SCRIPTS`]), `tick` updates since the press
    /// (that update its tick 0). At the script's jump tick its callback 36
    /// sends him `up` at `vy` units an update, drifting `drift` an update
    /// ([`crate::World::rebound_aim`]), to meet the ball `meet` updates on,
    /// under [`crate::toss::JUMP_GRAVITY`] until he lands; the script's
    /// tail holds him until `end` (the standing catch, which has no jump,
    /// from the start). `barred`: a ball on the rim slipped his hands
    /// (actor+0x1a8), so he takes nothing more this jump.
    ReboundJump {
        script: u8,
        tick: u16,
        vy: f32,
        drift: [f32; 2],
        up: bool,
        meet: u8,
        end: u16,
        barred: bool,
        /// Callback 47's answer at the script's start: its branch's clip
        /// ([`crate::rebound::Script::records`]).
        branch: bool,
        /// The clip channel 0 showed at the start (record and frame), held
        /// through the blend.
        from: Option<(u16, f32)>,
        /// The jump's airtime from callback 36 (`FUN_0019bed8`).
        airtime: u16,
        /// Callback 6's facing word (actor+0xb0).
        word: i32,
    },
}

impl Action {
    /// A trick move under way, the actor's bit 10 (`FUN_0016ea10`): any
    /// trick script but the clearout's, which never sets it.
    pub fn juking(&self) -> bool {
        matches!(self, Action::Trick { id, .. } if id.script != crate::trick::CLEAROUT)
    }

    /// Seconds since the action started, or 0 for none.
    pub fn elapsed(&self) -> f32 {
        match *self {
            Action::None => 0.0,
            Action::Shooting { elapsed, .. }
            | Action::Dunking { elapsed, .. }
            | Action::Layup { elapsed, .. }
            | Action::PumpFake { elapsed, .. }
            | Action::Passing { elapsed, .. }
            | Action::Stealing { elapsed, .. }
            | Action::Jumping { elapsed, .. }
            | Action::Stumbling { elapsed, .. } => elapsed,
            Action::TipJump { tick, .. }
            | Action::ReboundJump { tick, .. }
            | Action::Dive { tick, .. } => f32::from(tick) * TICK_SECONDS,
            Action::Trick { play, .. } => f32::from(play.tick()) * TICK_SECONDS,
            Action::Scoop { play } => f32::from(play.run.tick.max(0) as u16) * TICK_SECONDS,
        }
    }
}

/// A player's skills, 0 to 1 (the database's 0..99 ratings scaled), and size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratings {
    pub shots: f32,
    pub threes: f32,
    pub dunks: f32,
    pub rebounds: f32,
    pub blocks: f32,
    pub steals: f32,
    pub handles: f32,
    pub speed: f32,
    /// Rating 4 (the record's field 20): the post ups' and the back
    /// downs' tests (`FUN_00121398`, `FUN_0011bce0`), the screen's
    /// reaction (`FUN_001343e8`), the block and rebound jumps' odds.
    pub power: f32,
    /// Rating 9 (field 25; `FUN_00181bf8(ai, 0)`): the star's share
    /// (`FUN_0012a7d0`) and the playbook's first weights (`FUN_00126f60`).
    /// A created player's is the table `0x45fdc0` by his shots and dunks
    /// levels (`FUN_001d7d00`).
    pub overall: f32,
    /// Standing reach with the arms up, game units.
    pub reach: f32,
}

impl Default for Ratings {
    fn default() -> Self {
        Self {
            shots: 0.5,
            threes: 0.5,
            dunks: 0.5,
            rebounds: 0.5,
            blocks: 0.5,
            steals: 0.5,
            handles: 0.5,
            speed: 0.5,
            power: 0.5,
            overall: 0.5,
            reach: reach_for_height(78.0),
        }
    }
}

impl Ratings {
    /// A rating 0..99 by the original's index (`FUN_00171670` /
    /// `FUN_001d7d00`: 0 shots, 1 threes, 2 dunks, 3 rebounds, 4 power, 5
    /// blocks, 6 steals, 7 handles, 8 speed, 9 the overall). Rating 10
    /// (field 26) has no reader in play (only the front end's,
    /// `FUN_001d89d0`): 0 here, as past the eleven.
    pub fn native(&self, k: usize) -> i32 {
        let v = match k {
            0 => self.shots,
            1 => self.threes,
            2 => self.dunks,
            3 => self.rebounds,
            4 => self.power,
            5 => self.blocks,
            6 => self.steals,
            7 => self.handles,
            8 => self.speed,
            9 => self.overall,
            _ => 0.0,
        };
        (v * 99.0).round().clamp(0.0, 99.0) as i32
    }

    /// The sim's single-player rubber-band (our model — the game's
    /// `FUN_00143458` tunes the meter, see `docs/TRACE.md` "Difficulty"):
    /// every rating field goes to `mine × blend.0 + theirs × blend.1`,
    /// the difficulty's pair. Table rows (cheats like the ratings floor,
    /// degenerate "5 entry" branches) are not modeled here.
    pub fn blended(self, blend: (f32, f32), theirs: Ratings) -> Ratings {
        let (a, b) = blend;
        let mix = |x: f32, y: f32| (x * a + y * b).clamp(0.0, 1.0);
        Ratings {
            shots: mix(self.shots, theirs.shots),
            threes: mix(self.threes, theirs.threes),
            dunks: mix(self.dunks, theirs.dunks),
            rebounds: mix(self.rebounds, theirs.rebounds),
            blocks: mix(self.blocks, theirs.blocks),
            steals: mix(self.steals, theirs.steals),
            handles: mix(self.handles, theirs.handles),
            speed: mix(self.speed, theirs.speed),
            power: mix(self.power, theirs.power),
            overall: mix(self.overall, theirs.overall),
            // Reach stays the player's own (height, not skill).
            reach: self.reach,
        }
    }
}

/// Standing reach for a player `inches` tall: about 1.33 times the height, in
/// game units (about 1.18 to the foot).
pub fn reach_for_height(inches: f32) -> f32 {
    inches / 12.0 * 1.33 * 1.18
}

/// Something a player's action does at a particular moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Moment {
    /// A jump shot leaves the hands.
    Release(usize),
    /// A dunk reaches the rim.
    Slam(usize),
    /// A lay-up leaves the hand.
    LayupRelease(usize),
    /// The stand-in swipe's one test (without the disc's animations; the
    /// scripted swipe's are the world's, [`crate::World::swipe_due`]).
    Swipe(Option<crate::swipe::Swipe>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub team: usize,
    pub ratings: Ratings,
    /// Feet position on the court (x, z).
    pub position: [f32; 2],
    /// Facing, radians: 0 faces +z, pi/2 faces +x (the original's
    /// actor+0xa8, half a turn round: [`crate::timing::facing_word`]).
    pub heading: f32,
    /// Current speed along [`Player::course`], units per second.
    pub speed: f32,
    /// The way he moves, radians as the heading: the original's velocity
    /// (actor+0x40) points along the stick, not the facing
    /// ([`crate::guard`]).
    pub course: f32,
    /// The facing the walk turns toward (actor+0xb0): the stick's, or the
    /// ball's holder for a guard.
    pub wanted: f32,
    /// The guarding gait's state ([`crate::guard::Guard`]).
    pub guard: crate::guard::Guard,
    pub gait: Gait,
    pub action: Action,
    /// Buttons latched at action start; animation selection survives releases,
    /// replays and rollback snapshots.
    pub action_buttons: u16,
    /// Resolved direct trick slots, fixed for the match and carried in snapshots.
    /// None is the disc-free simulation's generic selection path.
    pub native_tricks: Option<NativeTricks>,
    /// Which of the eleven dunk slots (bit per slot) hold an advanced
    /// dunk (its id's flag 0x800 at `0x43f5f0`), when the match knows the
    /// player's dunk set. Fixed for the match, carried in snapshots.
    pub native_dunks: Option<u16>,
    /// Feet above the floor (jumping).
    pub height: f32,
    /// The turbo bar he draws on, 0 to 1: his slot's of the game's six
    /// ([`crate::World::turbo`], a computer side's three share one), as
    /// [`crate::World::sync_turbo`] copies it. Only the world changes it.
    pub turbo: f32,
    pub gait_channel: crate::ordinary::PlayerGaitChannel,
    /// Native actor+0x268 pass record kind (default 4; kick-pass writes 5).
    pub pass_kind: u32,
    /// Actor+0x1b0: the flight in updates of the last pass thrown to him,
    /// or his catch row's own count when it has one (the release's catch
    /// draw, `FUN_001611d8`, `0x161848` and `0x162a4c`), and -1 once that
    /// row's parked script has started (`0x15ab4c`). The catch row filter
    /// reads the passer's ([`crate::timing::catch_row`]).
    pub received_flight: i32,
    /// The record's weight in pounds (attribute 9, `FUN_001da4e0(record,
    /// 9)`), which the contact manager shares a push by
    /// ([`crate::contact`]). [`contact::WEIGHT`](crate::contact::WEIGHT)
    /// until a roster sets it.
    pub weight: u16,
    /// The model's scales (`model+0x40`, `model+8`, `FUN_003153d0`) when a
    /// roster gives them; else [`Player::body`] makes a plain one from the
    /// height.
    pub body: Option<crate::pose::Body>,
    /// The last update's change of height, units an update (the original's
    /// actor+0x44 as the block jump's callback 35 reads a holder's).
    pub rise: f32,
    /// Actor+0x1a8: updates he may not touch the ball (a block's knock bars
    /// the jumper and the ball's owner), counted down each update.
    pub barred: u16,
    /// Actor+0x1d4 and +0x1b8: a trick press rolled a fumble
    /// ([`World::fumble_tick`]), and the updates until it comes (counted
    /// down each update, `0x157168`).
    pub fumble: bool,
    pub fumble_timer: u16,
    /// Actor+0x1cc: updates his body keeps sweeping the ball whatever its
    /// state, from a knock off it ([`crate::touch::COOLDOWN`]), counted
    /// down each update.
    pub cooldown: u16,
    /// Actor+0x1c0 (with +0x1c4): the game told him to celebrate, which
    /// his side's update acts on in the restart ([`World::celebrate_due`]).
    ///
    /// [`World::celebrate_due`]: crate::World::celebrate_due
    pub celebrate: bool,
    /// The stand script on his channel ([`crate::stand`]), if one plays.
    pub stand: Option<crate::stand::Standing>,
    /// Actor+0x318: updates a defender has stood on the post defence's
    /// rows ([`crate::stand::POST_HOLD`]).
    pub stand_count: u16,
    /// State bit 17 (`FUN_0016e970`): boxing out ([`crate::stand::BOX_OUT`]).
    pub boxing: bool,
    /// State bit 11 (`FUN_0016ea20`): posting up, set by the backdown's
    /// scripts' `9b 0x820` ([`crate::backdown`]).
    pub posting: bool,
    /// A restart (game phase 6) this update: the walk's speed goes by the
    /// stick, never the speed rating (`0x167b98..0x167bd4`).
    pub restart_walk: bool,
    /// State bit 18 (`FUN_0016e980`): boxed out by another's box-out.
    pub boxed: bool,
    /// Actor+0x234: the man his box-out holds, and +0x230 the boxer who
    /// holds him ([`crate::stand`]'s contact).
    pub partner: Option<u8>,
    pub boxer: Option<u8>,
    /// Actor+0x244: updates his face has left (`FUN_00159190`, counted
    /// down by `FUN_00159240`, [`crate::World::face`]); the actors' reset
    /// (`FUN_00156870`, `0x156a94`) starts it at fps.
    pub face_wait: i32,
    /// His head's look ([`crate::look`]).
    pub look: crate::look::Look,
    /// Actor+0xc0 2: the record's field 13 set (`FUN_001563a8`,
    /// `0x156648`), a left-hander: the `8e f2` blends take their alternate
    /// ([`crate::trick::Select::Handed`]).
    pub left_handed: bool,
    /// Actor+0x1a0, the dribble: 0 from a take of the ball (and for
    /// everyone else when the ball changes sides, `FUN_00169138`,
    /// `0x1691d4`), 1 once he starts a locomotion record with the ball
    /// (`FUN_00163d40`, `0x167f54`: the request's 0x102), −1 when he
    /// pump-fakes out of a dribble (callback 20, `0x1723b0`). The
    /// computer fakes a shot only while it is not 1 (`FUN_001816b0`).
    pub dribble: i8,
    /// What the script on his channel 0 started from, recorded at its
    /// press ([`crate::held::ChannelStart`]).
    pub channel_start: Option<crate::held::ChannelStart>,
    /// His motion on a shot's, dunk's or lay-up's script ([`crate::jump`]):
    /// actor+0x40 and channel 0's flags.
    pub flight: crate::jump::Flight,
}

impl Player {
    pub fn new(team: usize, position: [f32; 2], heading: f32) -> Self {
        Self {
            team,
            ratings: Ratings::default(),
            position,
            heading,
            speed: 0.0,
            course: heading,
            wanted: heading,
            guard: crate::guard::Guard::default(),
            gait: Gait::Stand,
            action: Action::None,
            action_buttons: 0,
            native_tricks: None,
            native_dunks: None,
            height: 0.0,
            turbo: 1.0,
            gait_channel: crate::ordinary::PlayerGaitChannel::default(),
            pass_kind: 4,
            received_flight: 0,
            weight: crate::contact::WEIGHT as u16,
            body: None,
            rise: 0.0,
            barred: 0,
            fumble: false,
            fumble_timer: 0,
            cooldown: 0,
            celebrate: false,
            stand: None,
            stand_count: 0,
            boxing: false,
            posting: false,
            restart_walk: false,
            boxed: false,
            partner: None,
            boxer: None,
            face_wait: crate::TICK_HZ as i32,
            look: crate::look::Look::default(),
            left_handed: false,
            dribble: 0,
            channel_start: None,
            flight: crate::jump::Flight::new([0.0; 3]),
        }
    }

    /// The record's weight in pounds (attribute 9), at least 1.
    pub fn with_weight(mut self, pounds: u16) -> Self {
        self.weight = pounds.max(1);
        self
    }

    pub fn with_ratings(mut self, ratings: Ratings) -> Self {
        self.ratings = ratings;
        self
    }

    /// Standing height in feet, calculated from reach: reach = inches / 12.0 * 1.33 * 1.18.
    pub fn height_ft(&self) -> f32 {
        (self.ratings.reach / (1.33 * 1.18)).max(4.0)
    }

    /// The model's scales: the roster's, or a plain body of the height
    /// scale `inches * 1.2 / 78` (`0x3153d0`).
    pub fn body(&self) -> crate::pose::Body {
        self.body
            .unwrap_or_else(|| crate::pose::Body::uniform(self.height_ft() * 12.0 * 1.2 / 78.0))
    }

    /// Unit vector of the facing direction, (x, z).
    pub fn forward(&self) -> [f32; 2] {
        [math::sin(self.heading), math::cos(self.heading)]
    }

    /// Actor+0x40 not the zero vector, as the original's readers test it:
    /// his speed, but not on the stand path, which zeroes the velocity at
    /// once (a gait left over from the update before reads as standing).
    pub fn moving(&self) -> bool {
        self.speed != 0.0 && self.gait != Gait::Stand
    }

    /// Velocity on the floor, (x, z): along the course.
    pub fn velocity(&self) -> [f32; 2] {
        [
            math::sin(self.course) * self.speed,
            math::cos(self.course) * self.speed,
        ]
    }

    /// Face `heading` at once, and move that way: what the original's
    /// placements and moves write to the facing, the wanted facing and the
    /// velocity's direction alike.
    pub fn set_heading(&mut self, heading: f32) {
        self.heading = heading;
        self.wanted = heading;
        self.course = heading;
    }

    /// True when the player can start something new.
    pub fn free(&self) -> bool {
        self.action == Action::None
    }

    /// Start a jump shot at `hoop`: stop and face its rim spot, on the
    /// clock of shot `script` ([`crate::timing::SHOT_SCRIPTS`]).
    pub(crate) fn start_shot(
        &mut self,
        hoop: usize,
        rim: [f32; 2],
        gamebreaker: u8,
        fade: bool,
        script: usize,
    ) {
        self.begin_flight();
        self.want(rim);
        self.speed = 0.0;
        self.gait = Gait::Stand;
        let s = &crate::timing::SHOT_SCRIPTS[script];
        self.action = Action::Shooting {
            hoop,
            elapsed: 0.0,
            gamebreaker,
            // The post fadeaway is one from its pick (actor+0x1e4 8,
            // `0x16b8dc`).
            fade: fade || script == crate::timing::POST_FADEAWAY,
            tick: 0,
            script: script as u8,
            release: s.release,
            check: s.check,
            end: s.end,
            gate: s.gate,
            latest: s.latest,
            stall: 0,
        };
    }

    /// Start a dunk at `hoop`, flying to `to`, slamming `release` updates
    /// in and done at `end` (the dunk script's, of `row`).
    pub(crate) fn start_dunk(
        &mut self,
        hoop: usize,
        to: [f32; 2],
        style: DunkStyle,
        (row, release, end): (u8, u8, u8),
    ) {
        self.begin_flight();
        self.want(to);
        self.action = Action::Dunking {
            hoop,
            elapsed: 0.0,
            from: self.position,
            to,
            style,
            layup: false,
            tick: 0,
            release,
            end,
            row,
            hang: None,
        };
    }

    /// Start a lay-up at `hoop`, gliding to `to`, let go `release` updates
    /// in and done at `end` (the lay-up script's, of `row`).
    pub(crate) fn start_layup(
        &mut self,
        hoop: usize,
        to: [f32; 2],
        (row, release, end): (u8, u8, u8),
    ) {
        self.begin_flight();
        self.want(to);
        self.action = Action::Layup {
            hoop,
            elapsed: 0.0,
            from: self.position,
            to,
            tick: 0,
            release,
            end,
            row,
        };
    }

    /// A lay-up started in the air (`FUN_0016b1e8`'s rows): as
    /// [`Player::start_layup`], his flight going on.
    pub(crate) fn start_midair_layup(
        &mut self,
        hoop: usize,
        to: [f32; 2],
        (row, release, end): (u8, u8, u8),
    ) {
        self.flight.landing = -1;
        self.flight.script = 0;
        self.flight.updates = 0;
        self.action = Action::Layup {
            hoop,
            elapsed: 0.0,
            from: self.position,
            to,
            tick: 0,
            release,
            end,
            row,
        };
    }

    /// Dive on `script` of [`crate::timing::DIVE_SCRIPTS`] (0 far, 1
    /// near): his velocity as he pressed until the script's steps set it.
    pub(crate) fn start_dive(&mut self, script: u8) {
        self.begin_flight();
        self.action = Action::Dive { script, tick: 0 };
    }

    /// State bit 15: in an alley-oop's leap ([`crate::leap::leaping`]).
    pub fn leaping(&self) -> bool {
        crate::leap::leaping(&self.action)
    }

    /// Lose balance for `duration` seconds (the tests' stand-in: every
    /// knock in play is a reaction script).
    #[cfg(test)]
    pub(crate) fn stumble(&mut self, duration: f32) {
        self.action = Action::Stumbling {
            elapsed: 0.0,
            duration,
            reaction: None,
        };
        self.height = 0.0;
    }

    /// A scripted move's flight from the velocity he pressed with
    /// (actor+0x40; the scripts zero it or set it themselves).
    fn begin_flight(&mut self) {
        let [vx, vz] = self.velocity();
        self.flight = crate::jump::Flight::new([vx * TICK_SECONDS, self.rise, vz * TICK_SECONDS]);
    }

    /// The wanted facing (actor+0xb0) toward `at`; his facing turns to it
    /// on the move's script ([`crate::jump`], `FUN_00168ea8`).
    fn want(&mut self, at: [f32; 2]) {
        let (dx, dz) = (at[0] - self.position[0], at[1] - self.position[1]);
        if math::len(dx, dz) > 1e-3 {
            self.wanted = math::atan2(dx, dz);
        }
    }

    /// Glide from `from` to `to` over `arrive` seconds, quick off the floor
    /// and slowing at the end.
    fn glide(&mut self, from: [f32; 2], to: [f32; 2], t: f32, arrive: f32) {
        let u = (t / arrive).min(1.0);
        let s = 1.0 - (1.0 - u) * (1.0 - u);
        self.position = [
            from[0] + (to[0] - from[0]) * s,
            from[1] + (to[1] - from[1]) * s,
        ];
        self.speed = 0.0;
    }

    /// One tick of movement and action. Returns what the action does this
    /// tick, if anything.
    pub(crate) fn step(
        &mut self,
        input: &crate::Input,
        held: bool,
        stance: Option<crate::guard::Stance>,
    ) -> Option<Moment> {
        use tuning::*;
        let crossed = |before: f32, now: f32, at: f32| before < at && now >= at;
        let lift = self.height;
        let moment = match &mut self.action {
            Action::None => {
                self.walk(input, 1.0, held, stance);
                None
            }
            Action::Shooting {
                hoop,
                elapsed,
                tick,
                release,
                end,
                gate,
                latest,
                stall,
                ..
            } => {
                // The script's clock: the ball leaves at its callback 21
                // and the move is over at its last opcode once he is down
                // (his flight is the script's, [`crate::jump`], after this
                // step). The script's tick is the updates since the press
                // (the press update its 0) less the updates its release
                // gate waited: a person's script waits at the gate while
                // the shoot button is held, until the gate opens by itself
                // `latest` updates after the press (`FUN_00174c10`: 15
                // updates before the jump lands, 5 for the clips outside
                // `0x3d9218`, [`crate::timing::gate_waits`]); a computer's
                // never waits.
                *elapsed += TICK_SECONDS;
                *tick += 1;
                let vm = i32::from(*tick) - 1 - i32::from(*stall);
                if crate::timing::gate_waits(
                    *gate,
                    *latest,
                    vm,
                    i32::from(*tick) - 1,
                    !input.is_computer() && input.held(crate::button::SHOOT),
                ) {
                    *stall = stall.saturating_add(1);
                }
                let vm = i32::from(*tick) - 1 - i32::from(*stall);
                let hoop = *hoop;
                let (release, end) = (
                    i32::from(*release),
                    i32::from(*end).max(i32::from(*release) + 1),
                );
                if vm >= end && (self.height <= 0.0 || vm >= end + LANDING_LIMIT) {
                    self.action = Action::None;
                    self.height = 0.0;
                }
                (vm == release).then_some(Moment::Release(hoop))
            }
            Action::Dunking {
                hoop,
                elapsed,
                from,
                to,
                tick,
                release,
                end,
                row,
                ..
            } => {
                *elapsed += TICK_SECONDS;
                *tick += 1;
                // The script's tick: the press update is its 0.
                let (hoop, now, from, to) = (*hoop, f32::from(*tick - 1), *from, *to);
                let release = f32::from(*release);
                let end = f32::from(*end).max(release + 1.0);
                if crate::timing::move_row(*row).is_none() {
                    // The putback's own clock: straight up to the rim.
                    self.glide(from, to, now, release);
                    self.height = jump_height(
                        now,
                        PUTBACK_RISE.min(release - 1.0),
                        release,
                        end,
                        DUNK_HEIGHT,
                    );
                }
                // A row's flight is its script's ([`crate::jump`]), over
                // once he is down.
                if now >= end && (self.height <= 0.0 || now >= end + f32::from(LANDING_LIMIT as u8))
                {
                    self.action = Action::None;
                    self.height = 0.0;
                }
                (now == release).then_some(Moment::Slam(hoop))
            }
            Action::Layup {
                hoop,
                elapsed,
                from,
                to,
                tick,
                release,
                end,
                ..
            } => {
                *elapsed += TICK_SECONDS;
                *tick += 1;
                // The script's tick: the press update is its 0.
                let (hoop, now, from, to) = (*hoop, f32::from(*tick - 1), *from, *to);
                let release = f32::from(*release);
                let end = f32::from(*end).max(release + 1.0);
                // The flight is its script's ([`crate::jump`]), over once
                // he is down.
                let _ = (from, to);
                if now >= end && (self.height <= 0.0 || now >= end + f32::from(LANDING_LIMIT as u8))
                {
                    self.action = Action::None;
                    self.height = 0.0;
                }
                (now == release).then_some(Moment::LayupRelease(hoop))
            }
            Action::PumpFake {
                elapsed,
                tick,
                script,
            } => {
                // The shot's script falls through callback 20 to its end
                // ([`crate::held::fake_end`]; the press update its tick 0).
                *elapsed += TICK_SECONDS;
                *tick += 1;
                let vm = i32::from(*tick) - 1;
                if crate::held::fake_end(usize::from(*script)).is_none_or(|end| vm >= end) {
                    self.action = Action::None;
                }
                self.speed = 0.0;
                None
            }
            Action::Passing { elapsed, tick, row } => {
                *elapsed += TICK_SECONDS;
                *tick += 1;
                // A pass on its row's script moves him by it ([`crate::jump`]):
                // from the velocity he pressed with, the row's `91` holding
                // his walk until it gives it back (from the air, a row with
                // flag 0x400, until he is down too).
                let (row, elapsed) = (*row, *elapsed);
                if crate::jump::scripted(&self.action) {
                    let script = crate::timing::PASS_ROWS[usize::from(row)].script;
                    let air = crate::timing::PASS_ROWS[usize::from(row)].flags & 0x400 != 0;
                    if !air && self.flight.script != script {
                        self.begin_flight();
                    }
                    self.speed = 0.0;
                    self.gait = Gait::Stand;
                    return None;
                }
                // A passout to himself: his row's script stops him (`97`)
                // and holds him until it ends ([`crate::passout`]).
                if crate::passout::toss(row).is_some() {
                    self.speed = 0.0;
                    self.gait = Gait::Stand;
                    return None;
                }
                if elapsed >= PASS_TIME {
                    self.action = Action::None;
                }
                self.walk(input, 0.5, true, None);
                None
            }
            Action::Stealing {
                elapsed,
                counter,
                swipe,
                ..
            } => {
                let before = *elapsed;
                *elapsed += TICK_SECONDS;
                let t = *elapsed;
                let counter = *counter;
                if counter {
                    // The counter stance's script holds him; the clip's
                    // root moves him ([`crate::World::counter_tick`]).
                    self.speed = 0.0;
                    self.gait = Gait::Stand;
                    None
                } else if let Some(s) = swipe {
                    // The swipe's script holds him: no velocity (actor+0x40
                    // stays 0 through every captured swipe), the clip's
                    // root moves him ([`crate::World::swipe_tick`]). Its
                    // last update past the script's ([`Swipe::span`]) is
                    // the step that clears bit 6.
                    let over = s.tick > s.span();
                    self.speed = 0.0;
                    self.gait = Gait::Stand;
                    if over {
                        self.action = Action::None;
                    }
                    None
                } else {
                    if t >= STEAL_TIME {
                        self.action = Action::None;
                    }
                    self.walk(input, 0.4, false, None);
                    crossed(before, t, SWIPE_AT).then_some(Moment::Swipe(None))
                }
            }
            Action::Jumping {
                elapsed,
                block: Some(j),
                ..
            } => {
                // The block jump's script (`crate::block`): standing until
                // callback 35, then flying its velocity as the tip-off's
                // jump flies, until the script's landing wait has run out.
                *elapsed += TICK_SECONDS;
                self.speed = 0.0;
                self.gait = Gait::Stand;
                if j.up && (self.height > 0.0 || j.flown == 0) {
                    let g = crate::toss::JUMP_GRAVITY;
                    self.height += j.vy - 0.5 * g;
                    j.vy -= g;
                    self.position[0] += j.drift[0];
                    self.position[1] += j.drift[1];
                    j.flown = j.flown.saturating_add(1);
                    if self.height <= 0.0 {
                        self.height = 0.0;
                        j.vy = 0.0;
                        j.drift = [0.0; 2];
                    }
                }
                let over = j.done() && self.height <= 0.0;
                if over {
                    self.action = Action::None;
                    self.height = 0.0;
                }
                self.clamp_to_court();
                None
            }
            Action::Jumping { elapsed, high, .. } => {
                *elapsed += TICK_SECONDS;
                let (t, high) = (*elapsed, *high);
                let peak = if high { TURBO_JUMP_HEIGHT } else { JUMP_HEIGHT };
                self.height = jump_height(t, 0.0, JUMP_PEAK, JUMP_TIME, peak);
                // Momentum carries on through the air.
                self.speed *= 0.97;
                self.advance();
                if t >= JUMP_TIME {
                    self.action = Action::None;
                    self.height = 0.0;
                }
                None
            }
            Action::Trick { .. } => {
                // His script's update moved him
                // ([`crate::World::trick_tick`]); the stick does not.
                self.gait = Gait::Stand;
                None
            }
            Action::Stumbling {
                elapsed,
                reaction: Some(_),
                ..
            } => {
                // The reaction's script moves him
                // ([`crate::World::reaction_tick`]).
                *elapsed += TICK_SECONDS;
                self.gait = Gait::Stand;
                None
            }
            Action::Stumbling {
                elapsed, duration, ..
            } => {
                *elapsed += TICK_SECONDS;
                if *elapsed >= *duration {
                    self.action = Action::None;
                }
                self.speed *= 0.9;
                self.gait = Gait::Stand;
                self.advance();
                None
            }
            // The scoop's script holds him ([`crate::World::scoop_tick`]).
            Action::Scoop { .. } => {
                self.speed = 0.0;
                self.gait = Gait::Stand;
                None
            }
            // The dive's script moves him ([`crate::jump`]); it is over
            // when the script is ([`crate::World::flight_tick`]).
            Action::Dive { tick, .. } => {
                *tick = tick.saturating_add(1);
                self.speed = 0.0;
                self.gait = Gait::Stand;
                None
            }
            Action::TipJump {
                tick,
                vy,
                drift,
                up,
                ..
            } => {
                *tick = tick.saturating_add(1);
                self.speed = 0.0;
                self.gait = Gait::Stand;
                if *up {
                    // As the ball flies (`FUN_0014d1f8`): the step, then
                    // half the gravity off the height and all of it off
                    // the rise.
                    let g = crate::toss::JUMP_GRAVITY;
                    self.height += *vy - 0.5 * g;
                    *vy -= g;
                    self.position[0] += drift[0];
                    self.position[1] += drift[1];
                    self.clamp_to_court();
                    if self.height <= 0.0 {
                        self.height = 0.0;
                        self.action = Action::None;
                    }
                }
                None
            }
            Action::ReboundJump {
                script,
                tick,
                vy,
                drift,
                up,
                ..
            } => {
                *tick = tick.saturating_add(1);
                let (tick, flying, drift) = (*tick, *up, *drift);
                let tail = crate::rebound::SCRIPTS[usize::from(*script)].tail;
                // As the tip-off's jump flies.
                let g = crate::toss::JUMP_GRAVITY;
                let rise = *vy;
                if flying {
                    *vy -= g;
                }
                self.speed = 0.0;
                self.gait = Gait::Stand;
                let mut landed = false;
                if flying {
                    self.height += rise - 0.5 * g;
                    self.position[0] += drift[0];
                    self.position[1] += drift[1];
                    self.clamp_to_court();
                    if self.height <= 0.0 {
                        self.height = 0.0;
                        landed = true;
                    }
                }
                if let Action::ReboundJump { up, end, .. } = &mut self.action
                    && landed
                {
                    *up = false;
                    *end = tick.saturating_add(u16::from(tail));
                }
                if let Action::ReboundJump { end, .. } = self.action
                    && tick >= end
                {
                    self.action = Action::None;
                }
                None
            }
        };
        if !matches!(
            self.action,
            Action::None | Action::Passing { .. } | Action::Stealing { .. }
        ) && !crate::jump::scripted(&self.action)
        {
            // A move sets the facing itself; the walk takes it up from
            // there. (A shot's, dunk's or lay-up's script sets the wanted
            // facing and the facing turns to it, [`crate::jump`].)
            self.wanted = self.heading;
        }
        // `FUN_00168ea8` keeps the facings of the last updates (+0x2e8).
        self.guard.history.rotate_right(1);
        self.guard.history[0] = crate::timing::facing_word(self.heading);
        self.rise = self.height - lift;
        moment
    }

    /// Steer by the stick, at up to `limit` of the usual top speeds. Turbo
    /// speed needs turbo in the bar and uses it up. A guard (`stance`,
    /// [`crate::guard`]) faces the ball's holder and moves at his guarding
    /// record's speed.
    pub(crate) fn walk(
        &mut self,
        input: &crate::Input,
        limit: f32,
        held: bool,
        stance: Option<crate::guard::Stance>,
    ) {
        self.height = 0.0;
        use crate::input::{
            GaitFamily, combine_input_intensity, compute_physical_motion,
            evaluate_gait_family_hysteresis,
        };
        use tuning::*;
        let [sx, sz] = input.stick();
        // The stick's strength (actor+0x90; `FUN_00163d40` moves at 16 or
        // more and caps it at 64).
        let strength = input.strength().max(0) as u32;
        let moves = input.moves();
        let turbo = input.turbo() && self.turbo > 0.0;
        let (_requested, capped_intensity) = combine_input_intensity(strength, 0);

        // The family as the native channel keeps it: Jog plays the walk
        // clip, but keeps its own 39/61 band.
        let family = evaluate_gait_family_hysteresis(self.gait_channel.family, capped_intensity);
        self.gait_channel.family = if moves { family as u32 } else { 0 };

        let speed_rating = (self.ratings.speed * 99.0).round() as u32;
        // The motion goes by the speed rating only for a run outside a
        // restart (`0x167b98..0x167bd4`); otherwise by the stick.
        let motion_family = if self.restart_walk {
            GaitFamily::Walk
        } else {
            family
        };
        // The playing gait record's motion range (per native update, so
        // times 60 a second); without the records, generic bounds.
        let native = self.gait_channel.record.is_some();
        let (min_m, max_m) = match family {
            _ if native => (
                self.gait_channel.motion[0] * crate::TICK_HZ as f32,
                self.gait_channel.motion[1] * crate::TICK_HZ as f32,
            ),
            GaitFamily::Run => (RUN_SPEED * 0.9, RUN_SPEED * 1.1),
            _ => (0.0, WALK_SPEED),
        };
        let gait = if limit < 1.0 {
            // The rewrite's own walk while a pass winds up or a swipe
            // reaches without the disc's animations (the original's
            // locomotion does not run in those scripts: the passer's
            // velocity is 0 in 2,465 of the played captures' 2,939 windup
            // updates while his pass clip's root carries him): turning
            // toward the stick and gathering speed, along his facing.
            let target = if !moves {
                0.0
            } else if !native
                && family == GaitFamily::Run
                && !turbo
                && (self.ratings.speed - 0.5).abs() < 1e-4
                && strength >= 64
            {
                RUN_SPEED * limit
            } else {
                compute_physical_motion(
                    motion_family,
                    capped_intensity,
                    speed_rating,
                    min_m,
                    max_m,
                    turbo,
                ) * limit
            };
            if moves {
                let turn = wrap_angle(math::atan2(sx, sz) - self.heading);
                let max = TURN_RATE * TICK_SECONDS;
                self.heading = wrap_angle(self.heading + turn.clamp(-max, max));
            }
            let rate = if target > self.speed {
                ACCELERATION
            } else {
                DECELERATION
            } * TICK_SECONDS;
            self.speed += (target - self.speed).clamp(-rate, rate);
            self.course = self.heading;
            self.wanted = self.heading;
            if !moves {
                Gait::Stand
            } else if turbo {
                Gait::Turbo
            } else if family == GaitFamily::Run {
                Gait::Run
            } else {
                Gait::Walk
            }
        } else if !moves {
            // The stand path sets no velocity (`FUN_00163d40` below 16):
            // actor+0x40 is 0 from the update the stick drops in all 246 of
            // the played captures' stops. What carries him on is his stand
            // script's planted foot through its blend from the gait's frame
            // ([`World::stand_tick`]).
            self.speed = 0.0;
            Gait::Stand
        } else {
            // `FUN_00163d40`'s moving path (`0x167b70..0x167dc0`): the
            // record's motion at once (a guard's by the stick's strength,
            // not his speed rating), along the stick.
            let phys = if !native
                && stance.is_none()
                && family == GaitFamily::Run
                && !turbo
                && (self.ratings.speed - 0.5).abs() < 1e-4
                && strength >= 64
            {
                RUN_SPEED
            } else if stance.is_some() {
                let f = (capped_intensity as f32 / 64.0).min(1.0);
                min_m + (max_m - min_m) * f
            } else {
                compute_physical_motion(
                    motion_family,
                    capped_intensity,
                    speed_rating,
                    min_m,
                    max_m,
                    turbo,
                )
            };
            self.speed = phys * limit;
            self.course = math::atan2(sx, sz);
            // The wanted facing: the stick's, or a guard's holder (steadied
            // by `FUN_00168a10` after the mode's first update).
            self.wanted = match stance {
                Some(s) => crate::restart::heading_radians(s.toward),
                None => self.course,
            };
            if turbo {
                Gait::Turbo
            } else if family == GaitFamily::Run && stance.is_none() {
                Gait::Run
            } else {
                Gait::Walk
            }
        };
        // `FUN_00168ea8`: the facing turns toward the wanted facing, at
        // most 46 units an update; 23 for a posting holder (bit 11 and
        // +0x70, `0x168f38..0x168f60`), 11 on the post defence's rows
        // within 57 of it (`0x168f9c..0x168fec`).
        let (now, to) = (
            crate::timing::facing_word(self.heading),
            crate::timing::facing_word(self.wanted),
        );
        let d = crate::guard::turn(to, now);
        let row = self.stand.and_then(|s| s.run.shown().record);
        let rate = if self.posting && held {
            crate::guard::POST_TURN
        } else if matches!(row, Some(718 | 719)) && d.abs() <= f32::from_bits(0x4265_8e0f) as i32 {
            crate::guard::STANCE_TURN
        } else {
            crate::guard::TURN
        };
        if d.abs() > rate {
            self.heading = crate::restart::heading_radians(now + rate * d.signum());
        } else if d != 0 {
            self.heading = self.wanted;
        }
        self.gait = if self.speed < 0.5 && gait == Gait::Stand {
            Gait::Stand
        } else if gait == Gait::Stand {
            // Slowing to a stop keeps the last moving gait until nearly still.
            if self.speed > RUN_SPEED * 0.5 {
                Gait::Run
            } else {
                Gait::Walk
            }
        } else {
            gait
        };
        let moving = self.gait != Gait::Stand;
        // The skeleton's scale (`FUN_0019cd48` on actor+0x108, its first
        // word): `0x167cd0..0x167ce8` divides the record's root distance
        // by the motion and multiplies by it (a 6'6" player's 1.2; a
        // stand-in of height / 6.5 had cycled the legs a fifth too fast).
        let body_scale_x = self.body().height;
        self.gait_channel.update(
            held,
            moving,
            self.speed,
            &mut self.position,
            [self.heading, self.course],
            body_scale_x,
        );
        self.clamp_to_court();
    }

    /// Move along the course at the current speed, staying on the court.
    fn advance(&mut self) {
        let [fx, fz] = [math::sin(self.course), math::cos(self.course)];
        let step = self.speed * TICK_SECONDS;
        self.position[0] += fx * step;
        self.position[1] += fz * step;
        self.clamp_to_court();
    }

    pub(crate) fn clamp_to_court(&mut self) {
        use tuning::*;
        self.position[0] = self.position[0].clamp(-COURT_HALF_LENGTH, COURT_HALF_LENGTH);
        self.position[1] = self.position[1].clamp(-COURT_HALF_WIDTH, COURT_HALF_WIDTH);
    }
}

/// Height of a jump `t` seconds into an action: a parabola from `takeoff` to
/// `landing`, `peak_height` at `peak`.
fn jump_height(t: f32, takeoff: f32, peak: f32, landing: f32, peak_height: f32) -> f32 {
    if !(takeoff..landing).contains(&t) {
        return 0.0;
    }
    let half = if t < peak {
        peak - takeoff
    } else {
        landing - peak
    };
    let u = (t - peak) / half;
    peak_height * (1.0 - u * u)
}

#[cfg(test)]
mod native_trick_tests {
    use super::*;
    use crate::button;

    #[test]
    fn direct_slot_masks_prioritize_three_and_four_turbos_and_ignore_action_bits() {
        assert_eq!(
            NativeTricks::slot(button::TRICK | button::L2 | button::R2),
            Some(10)
        );
        assert_eq!(
            NativeTricks::slot(button::L1 | button::L2 | button::R2),
            Some(3)
        );
        assert_eq!(
            NativeTricks::slot(button::L1 | button::R1 | button::L2 | button::R2),
            Some(0)
        );
        assert_eq!(NativeTricks::slot(button::TRICK | button::L2), None);
        assert_eq!(NativeTricks::slot(button::TRICK), None);
    }
}
